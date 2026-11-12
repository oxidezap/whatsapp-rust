// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Runtime and resolver for sends built in the page.
//!
//! wacore's stanza builders spawn per-device work and ask a
//! `SendContextResolver` for device lists and prekey bundles. In the page the
//! runtime is the JS event loop and the resolver is a JS object that relays to
//! the host's WhatsApp connection.

use crate::convert::remote_bundle;
use async_trait::async_trait;
use js_sys::{Array, Function, Object, Promise, Reflect};
use send_wrapper::SendWrapper;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use wacore::client::context::{GroupRoutingInfo, SendContextResolver, SessionLockGuard};
use wacore::libsignal::protocol::PreKeyBundle;
use wacore::prekeys::PreKeyFetchOutcome;
use wacore::runtime::{AbortHandle, Runtime};
use wacore::types::message::AddressingMode;
use wacore_binary::{CompactString, Jid};
use wacore_signal_ops::{FetchedBundle, SignalResolver, VerifiablePrekeys};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};

#[wasm_bindgen(typescript_custom_section)]
const TS_SEND_TYPES: &str = r#"
/** Answers the device and key queries a send needs, by relaying to the WhatsApp client. */
export interface SendResolver {
  /** Device JIDs for these user JIDs. */
  resolveDevices(jids: string[]): Promise<string[]>;
  /**
   * Prekey bundles by device JID; omit devices without one. A companion
   * device's bundle must carry the `<device-identity>` the server sent with it
   * as `deviceIdentity`; a bundle that does not verify is not used.
   */
  fetchPrekeys(jids: string[]): Promise<Record<string, RemoteBundle>>;
  /**
   * Optional. The LID user (no server) that a phone-number user maps to, if
   * known. Lets a send to a phone-number JID reuse a session migrated to the LID.
   */
  lidForPhone?(user: string): Promise<string | null | undefined>;
  /** Group members and addressing; `lidToPn` maps LID users to phone JIDs. */
  resolveGroup(jid: string): Promise<{
    participants: string[];
    addressingMode: "pn" | "lid";
    lidToPn?: Record<string, string>;
  }>;
}
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "SendResolver")]
    pub type SendResolver;

    #[wasm_bindgen(method, catch, js_name = resolveDevices)]
    fn resolve_devices(this: &SendResolver, jids: Array) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = fetchPrekeys)]
    fn fetch_prekeys(this: &SendResolver, jids: Array) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = lidForPhone)]
    fn lid_for_phone(this: &SendResolver, user: &str) -> Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = resolveGroup)]
    fn resolve_group(this: &SendResolver, jid: &str) -> Result<Promise, JsValue>;
}

/// Runs wacore's tasks on the page's event loop.
pub struct PageRuntime;

fn set_timeout(ms: f64) -> Promise {
    Promise::new(&mut |resolve, _reject| {
        let set_timeout = Reflect::get(&js_sys::global(), &JsValue::from_str("setTimeout"))
            .ok()
            .and_then(|f| f.dyn_into::<Function>().ok());
        match set_timeout {
            Some(f) => {
                let _ = f.call2(&JsValue::NULL, &resolve, &JsValue::from_f64(ms));
            }
            // No timer in this host: resolve at once rather than hang.
            None => {
                let _ = resolve.call0(&JsValue::NULL);
            }
        }
    })
}

impl Runtime for PageRuntime {
    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + 'static>>) -> AbortHandle {
        spawn_local(future);
        AbortHandle::noop()
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()>>> {
        let promise = set_timeout(duration.as_millis() as f64);
        Box::pin(async move {
            let _ = JsFuture::from(promise).await;
        })
    }

    fn spawn_blocking(&self, f: Box<dyn FnOnce() + 'static>) -> Pin<Box<dyn Future<Output = ()>>> {
        Box::pin(async move { f() })
    }

    fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()>>>> {
        None
    }
}

/// A JS `SendResolver` as wacore's resolver. Single-threaded, so the handle is
/// wrapped only to satisfy the trait's `Send + Sync` bounds.
pub struct JsResolver(SendWrapper<SendResolver>);

impl JsResolver {
    pub fn new(resolver: SendResolver) -> Self {
        Self(SendWrapper::new(resolver))
    }
}

fn js_error(context: &str, error: JsValue) -> anyhow::Error {
    let message = Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|m| m.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "JavaScript error".to_owned());
    anyhow::anyhow!("{context}: {message}")
}

async fn call(context: &str, call: Result<Promise, JsValue>) -> anyhow::Result<JsValue> {
    let promise = call.map_err(|e| js_error(context, e))?;
    JsFuture::from(promise)
        .await
        .map_err(|e| js_error(context, e))
}

fn jid_array(jids: &[Jid]) -> Array {
    jids.iter()
        .map(|jid| JsValue::from_str(&jid.to_string()))
        .collect()
}

fn parse_jid(value: &JsValue, context: &str) -> anyhow::Result<Jid> {
    value
        .as_string()
        .ok_or_else(|| anyhow::anyhow!("{context}: expected a JID string"))?
        .parse()
        .map_err(|_| anyhow::anyhow!("{context}: not a JID"))
}

#[async_trait(?Send)]
impl SignalResolver for JsResolver {
    async fn fetch_verifiable_prekeys(&self, jids: &[Jid]) -> anyhow::Result<VerifiablePrekeys> {
        let value = call("fetchPrekeys", self.0.fetch_prekeys(jid_array(jids))).await?;
        let object: Object = value
            .dyn_into()
            .map_err(|_| anyhow::anyhow!("fetchPrekeys must resolve to an object"))?;
        let mut bundles = HashMap::new();
        for entry in Object::entries(&object).iter() {
            let pair: Array = entry.unchecked_into();
            let jid = parse_jid(&pair.get(0), "fetchPrekeys")?;
            let remote =
                remote_bundle(&pair.get(1)).map_err(|e| js_error("fetchPrekeys bundle", e))?;
            let bundle = remote
                .to_prekey_bundle()
                .map_err(|e| anyhow::anyhow!("fetchPrekeys bundle: {e}"))?;
            bundles.insert(
                jid,
                FetchedBundle {
                    bundle,
                    device_identity: remote.device_identity,
                },
            );
        }
        Ok(VerifiablePrekeys {
            bundles,
            rejected: Vec::new(),
        })
    }
}

#[async_trait(?Send)]
impl SendContextResolver for JsResolver {
    // `SignalOps` holds the session lock of every target address for the
    // whole send, so the host needs no second lock set here.
    async fn lock_device_sessions(&self, _device_jids: &[Jid]) -> SessionLockGuard {
        SessionLockGuard::none()
    }

    async fn resolve_devices(&self, jids: &[Jid]) -> anyhow::Result<Vec<Jid>> {
        let value = call("resolveDevices", self.0.resolve_devices(jid_array(jids))).await?;
        let list: Array = value
            .dyn_into()
            .map_err(|_| anyhow::anyhow!("resolveDevices must resolve to an array"))?;
        list.iter()
            .map(|item| parse_jid(&item, "resolveDevices"))
            .collect()
    }

    async fn fetch_prekeys(&self, jids: &[Jid]) -> anyhow::Result<HashMap<Jid, PreKeyBundle>> {
        Ok(self
            .fetch_verifiable_prekeys(jids)
            .await?
            .bundles
            .into_iter()
            .map(|(jid, fetched)| (jid, fetched.bundle))
            .collect())
    }

    async fn fetch_prekeys_for_identity_check(
        &self,
        jids: &[Jid],
    ) -> anyhow::Result<PreKeyFetchOutcome> {
        Ok(PreKeyFetchOutcome {
            bundles: self.fetch_prekeys(jids).await?,
            rejected: Vec::new(),
        })
    }

    async fn get_lid_for_phone(&self, phone_user: &str) -> Option<CompactString> {
        // Optional on the JS side: a resolver without it keeps PN addressing.
        let has = Reflect::get(&self.0, &JsValue::from_str("lidForPhone"))
            .map(|f| f.is_function())
            .unwrap_or(false);
        if !has {
            return None;
        }
        let value = call("lidForPhone", self.0.lid_for_phone(phone_user))
            .await
            .ok()?;
        value
            .as_string()
            .filter(|lid| !lid.is_empty())
            .map(CompactString::from)
    }

    async fn resolve_group_routing_info(&self, jid: &Jid) -> anyhow::Result<Arc<GroupRoutingInfo>> {
        let value = call("resolveGroup", self.0.resolve_group(&jid.to_string())).await?;
        let get =
            |key: &str| Reflect::get(&value, &JsValue::from_str(key)).unwrap_or(JsValue::UNDEFINED);
        let participants: Array = get("participants")
            .dyn_into()
            .map_err(|_| anyhow::anyhow!("resolveGroup: participants must be an array"))?;
        let participants = participants
            .iter()
            .map(|p| parse_jid(&p, "resolveGroup participant"))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let mode = match get("addressingMode").as_string().as_deref() {
            Some("lid") => AddressingMode::Lid,
            Some("pn") => AddressingMode::Pn,
            _ => anyhow::bail!("resolveGroup: addressingMode must be \"pn\" or \"lid\""),
        };
        let lid_to_pn = get("lidToPn");
        if lid_to_pn.is_undefined() || lid_to_pn.is_null() {
            return Ok(Arc::new(GroupRoutingInfo::new(participants, mode)));
        }
        let object: Object = lid_to_pn
            .dyn_into()
            .map_err(|_| anyhow::anyhow!("resolveGroup: lidToPn must be an object"))?;
        let mut map = HashMap::new();
        for entry in Object::entries(&object).iter() {
            let pair: Array = entry.unchecked_into();
            let lid_user = pair
                .get(0)
                .as_string()
                .ok_or_else(|| anyhow::anyhow!("resolveGroup: lidToPn key must be a string"))?;
            map.insert(
                CompactString::from(lid_user),
                parse_jid(&pair.get(1), "resolveGroup lidToPn")?,
            );
        }
        Ok(Arc::new(GroupRoutingInfo::with_lid_to_pn_map(
            participants,
            mode,
            map,
        )))
    }
}
