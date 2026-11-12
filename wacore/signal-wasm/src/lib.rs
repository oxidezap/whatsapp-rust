// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Browser and edge bindings for `wacore_signal_ops`.
//!
//! The page supplies storage and sealing: a `RecordBackend` (IndexedDB in a
//! browser, anything in an edge worker) and a `Sealer` (WebCrypto, so the data
//! key can stay non-extractable). This crate supplies the Signal state machine.
//! Supply an initialized wasm-bindgen module exposing `SignalDevice`, not a
//! bare WebAssembly instance. Keep one device per held scope. Neither this
//! module nor its local Node tests establish a browser transport or grant flow.
//! App-state sharing is a separate default-off choice: enabling it permits
//! contact-name, label and mute/archive/pin metadata decryption. Message
//! plaintext custody does not imply metadata opacity. Protect the caller's
//! store/sealer, renew its lease, and persist application content before
//! `markDelivered`. The host still owns history consent and transport policy.
#![cfg(target_arch = "wasm32")]

mod adapters;
mod convert;
mod send;

use adapters::{JsRecords, JsSealerAdapter};
use convert::{
    address, lease_to_js, ops_error, public_identity, public_prekeys, remote_bundle, set,
    signed_prekey,
};
use js_sys::{Object, Promise, Uint8Array, Uint32Array};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use wacore_recordstore::{Lease, LeaseStore, RecordSignalStore};
use wacore_signal_ops::{EncKind, OpsError, ReceiveKind, ReceiveRequest, Received, SignalOps};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

#[wasm_bindgen(typescript_custom_section)]
const TS_TYPES: &str = r#"
export interface Fence { scope: string; generation: number }
export interface Lease { scope: string; generation: number; holder: string; expiresAtMs: number }
export type WriteOp =
  | { op: "put" | "update" | "insert"; ns: string; key: string; value: Uint8Array }
  | { op: "delete"; ns: string; key: string };

/**
 * Byte storage for one or more scopes. `write` applies every op or none, and
 * only while `fence.generation` is the scope's current lease generation;
 * otherwise it rejects with an error whose `code` is "fence_lost". An
 * "update" op on a missing key is a no-op, never an insert. An "insert" op on
 * an existing key rejects the whole batch with `code` "record_exists".
 */
export interface RecordBackend {
  get(scope: string, ns: string, key: string): Promise<Uint8Array | null | undefined>;
  scanKeys(scope: string, ns: string, prefix: string, limit: number | undefined): Promise<string[]>;
  write(fence: Fence, ops: WriteOp[]): Promise<void>;
  acquire(scope: string, holder: string, nowMs: number, ttlMs: number): Promise<Lease | null>;
  renew(lease: Lease, nowMs: number, ttlMs: number): Promise<Lease | null>;
  release(lease: Lease): Promise<void>;
}

/** Authenticated encryption for record values. `aad` must be authenticated, not encrypted. */
export interface Sealer {
  seal(aad: Uint8Array, plaintext: Uint8Array): Promise<Uint8Array>;
  open(aad: Uint8Array, sealed: Uint8Array): Promise<Uint8Array>;
}

export interface RemoteBundle {
  registrationId: number;
  deviceId: number;
  identityKey: Uint8Array;
  signedPrekeyId: number;
  signedPrekey: Uint8Array;
  signedPrekeySignature: Uint8Array;
  prekeyId?: number;
  prekey?: Uint8Array;
}
export interface PublicPreKey { id: number; publicKey: Uint8Array }
export interface PrekeyUploadBatch { batchId: string; preKeys: PublicPreKey[] }
export type RetryRoute =
  | { kind: "direct"; recipient?: string }
  | { kind: "group"; addressingMode: "pn" | "lid" }
  | { kind: "broadcast_list" };
export interface RetryRequest {
  chat: string; messageId: string; requester: string; encryptionJid: string;
  retryCount: number; route: RetryRoute; bundle?: RemoteBundle;
}
export type ReceiveResult =
  | { status: "already_delivered"; receiptKey: string }
  | { status: "message"; receiptKey: string; message: Uint8Array;
      isSkdmOnly: boolean; identityChanged: boolean; redelivered: boolean;
      senderKeyRejected: boolean; appStateKeyShare: Uint8Array };
"#;

type Store = RecordSignalStore<JsRecords, JsSealerAdapter>;

struct Inner {
    ops: SignalOps<Store>,
    records: Arc<JsRecords>,
    lease: RefCell<Lease>,
    store: Arc<Store>,
}

/// One linked device's Signal state, held under a lease.
#[wasm_bindgen]
pub struct SignalDevice {
    inner: Rc<Inner>,
}

fn promise<F>(future: F) -> Promise
where
    F: std::future::Future<Output = Result<JsValue, JsValue>> + 'static,
{
    future_to_promise(future)
}

#[wasm_bindgen]
impl SignalDevice {
    /// Take the scope's lease and open its device. With `create`, a scope
    /// without device keys gets new ones; existing keys are never replaced.
    /// Rejects with code "lease_held" while another holder's lease is live.
    /// Pass the same `backend` object to every open of a scope: opens under
    /// one lease share key caches and locks only when their backend is the
    /// same object.
    #[wasm_bindgen(js_name = open)]
    pub async fn open(
        backend: adapters::RecordBackend,
        sealer: adapters::Sealer,
        scope: String,
        holder: String,
        now_ms: f64,
        ttl_ms: f64,
        create: bool,
    ) -> Result<SignalDevice, JsValue> {
        let records = JsRecords::shared(backend);
        let lease = records
            .acquire(&scope, &holder, now_ms as u64, ttl_ms as u64)
            .await
            .map_err(|e| ops_error(OpsError::Store(e)))?
            .ok_or_else(|| convert::coded_error("lease_held", "another holder owns this scope"))?;
        let store = Arc::new(RecordSignalStore::new(
            records.clone(),
            Arc::new(JsSealerAdapter::new(sealer)),
            lease.fence.clone(),
        ));
        let opened = match SignalOps::open(store.clone()).await {
            Ok(ops) => Ok(ops),
            Err(OpsError::NotInitialised) if create => SignalOps::create(store.clone()).await,
            Err(e) => Err(e),
        };
        let ops = match opened {
            Ok(ops) => ops,
            Err(e) => {
                // No SignalDevice is returned, so nobody else could release
                // this lease; give it up now rather than block other holders
                // until it expires. A failed release changes nothing for the
                // caller, who gets the original error.
                let _ = records.release(&lease).await;
                return Err(ops_error(e));
            }
        };
        Ok(SignalDevice {
            inner: Rc::new(Inner {
                ops,
                records,
                lease: RefCell::new(lease),
                store,
            }),
        })
    }

    /// The current lease, for the caller's renewal timer.
    #[wasm_bindgen(getter)]
    pub fn lease(&self) -> JsValue {
        lease_to_js(&self.inner.lease.borrow())
    }

    /// Extend the lease. Resolves `false` once another holder has taken the
    /// scope; every later write would be refused, so stop using this device.
    #[wasm_bindgen(js_name = renewLease)]
    pub fn renew_lease(&self, now_ms: f64, ttl_ms: f64) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let current = inner.lease.borrow().clone();
            let renewed = inner
                .records
                .renew(&current, now_ms as u64, ttl_ms as u64)
                .await
                .map_err(|e| ops_error(OpsError::Store(e)))?;
            Ok(JsValue::from_bool(match renewed {
                Some(lease) => {
                    *inner.lease.borrow_mut() = lease;
                    true
                }
                None => false,
            }))
        })
    }

    /// Give the scope up so another holder can take it at once.
    pub fn release(&self) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let current = inner.lease.borrow().clone();
            inner
                .records
                .release(&current)
                .await
                .map_err(|e| ops_error(OpsError::Store(e)))?;
            Ok(JsValue::UNDEFINED)
        })
    }

    #[wasm_bindgen(js_name = publicIdentity)]
    pub fn public_identity(&self) -> Promise {
        let inner = self.inner.clone();
        promise(async move { Ok(public_identity(&inner.ops.public_identity().await)) })
    }

    #[wasm_bindgen(js_name = generatePrekeys)]
    pub fn generate_prekeys(&self, count: u32) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let keys = inner.ops.generate_prekeys(count).await.map_err(ops_error)?;
            Ok(public_prekeys(&keys))
        })
    }

    /// Durable public upload batch. Repeated calls/reopening recover the
    /// outstanding live batch; use fresh `generatePrekeys` for retry receipts.
    #[wasm_bindgen(js_name = preparePrekeyUpload, unchecked_return_type = "Promise<PrekeyUploadBatch>")]
    pub fn prepare_prekey_upload(&self, count: f64) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            if count.fract() != 0.0 || !(1.0..=812.0).contains(&count) {
                return Err(convert::coded_error(
                    "invalid_input",
                    "prekey count must be an integer from 1 to 812",
                ));
            }
            let batch = inner
                .ops
                .prepare_prekey_upload(count as u32)
                .await
                .map_err(ops_error)?;
            let out = Object::new();
            set(&out, "batchId", &JsValue::from_str(&batch.batch_id));
            set(&out, "preKeys", &public_prekeys(&batch.pre_keys));
            Ok(out.into())
        })
    }

    /// Confirm a successful upload by its exact batch and member IDs. The
    /// latest exact ACK replays safely after a lost response/reopening.
    #[wasm_bindgen(js_name = acknowledgePrekeyUpload, unchecked_return_type = "Promise<void>")]
    pub fn acknowledge_prekey_upload(&self, batch_id: String, ids: Uint32Array) -> Promise {
        let inner = self.inner.clone();
        let ids = ids.to_vec();
        promise(async move {
            inner
                .ops
                .acknowledge_prekey_upload(&batch_id, &ids)
                .await
                .map_err(ops_error)?;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Local surviving uploaded keys; this does not query the provider pool.
    #[wasm_bindgen(js_name = uploadedPrekeyCount, unchecked_return_type = "Promise<number>")]
    pub fn uploaded_prekey_count(&self) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let count = inner
                .store
                .uploaded_prekey_count()
                .await
                .map_err(|e| ops_error(OpsError::Store(e)))?;
            Ok(JsValue::from_f64(f64::from(count)))
        })
    }

    /// Mark fresh retry keys only. Any original pending upload member rejects
    /// the entire call; use `acknowledgePrekeyUpload` for durable batches.
    #[wasm_bindgen(js_name = markPrekeysUploaded)]
    pub fn mark_prekeys_uploaded(&self, ids: Uint32Array) -> Promise {
        let inner = self.inner.clone();
        let ids = ids.to_vec();
        promise(async move {
            inner
                .ops
                .mark_prekeys_uploaded(&ids)
                .await
                .map_err(ops_error)?;
            Ok(JsValue::UNDEFINED)
        })
    }

    #[wasm_bindgen(js_name = rotateSignedPrekey)]
    pub fn rotate_signed_prekey(&self) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let key = inner.ops.rotate_signed_prekey().await.map_err(ops_error)?;
            Ok(signed_prekey(&key))
        })
    }

    #[wasm_bindgen(js_name = hasSession)]
    pub fn has_session(&self, user: String, device: u32) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let found = inner
                .ops
                .has_session(&address(&user, device)?)
                .await
                .map_err(ops_error)?;
            Ok(JsValue::from_bool(found))
        })
    }

    /// Run X3DH against a bundle the client fetched from the server.
    #[wasm_bindgen(js_name = establishSession)]
    pub fn establish_session(&self, user: String, device: u32, bundle: JsValue) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let bundle = remote_bundle(&bundle)?;
            inner
                .ops
                .establish_session(&address(&user, device)?, &bundle)
                .await
                .map_err(ops_error)?;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Decrypt a pairwise payload. `kind` is the `<enc type>` value.
    pub fn decrypt(&self, user: String, device: u32, kind: String, ciphertext: Vec<u8>) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let kind = EncKind::try_from(kind.as_str()).map_err(|_| {
                convert::coded_error("invalid_input", &format!("unsupported enc type {kind}"))
            })?;
            let result = inner
                .ops
                .decrypt(&address(&user, device)?, kind, &ciphertext)
                .await
                .map_err(ops_error)?;
            let out = Object::new();
            set(
                &out,
                "plaintext",
                &Uint8Array::from(result.plaintext.as_slice()),
            );
            set(
                &out,
                "identityChanged",
                &JsValue::from_bool(result.identity_changed),
            );
            Ok(out.into())
        })
    }

    /// Encrypt padded plaintext for one device with an established session.
    pub fn encrypt(&self, user: String, device: u32, plaintext: Vec<u8>) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let result = inner
                .ops
                .encrypt(&address(&user, device)?, &plaintext)
                .await
                .map_err(ops_error)?;
            let out = Object::new();
            set(&out, "kind", &JsValue::from_str(result.kind.as_str()));
            set(
                &out,
                "ciphertext",
                &Uint8Array::from(result.ciphertext.as_slice()),
            );
            Ok(out.into())
        })
    }

    #[wasm_bindgen(js_name = senderKeyDistribution)]
    pub fn sender_key_distribution(&self, group: String, user: String, device: u32) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let skdm = inner
                .ops
                .sender_key_distribution(&group, &address(&user, device)?)
                .await
                .map_err(ops_error)?;
            Ok(Uint8Array::from(skdm.as_slice()).into())
        })
    }

    #[wasm_bindgen(js_name = groupEncrypt)]
    pub fn group_encrypt(
        &self,
        group: String,
        user: String,
        device: u32,
        plaintext: Vec<u8>,
    ) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let ciphertext = inner
                .ops
                .group_encrypt(&group, &address(&user, device)?, &plaintext)
                .await
                .map_err(ops_error)?;
            Ok(Uint8Array::from(ciphertext.as_slice()).into())
        })
    }

    #[wasm_bindgen(js_name = processSenderKeyDistribution)]
    pub fn process_sender_key_distribution(
        &self,
        group: String,
        user: String,
        device: u32,
        skdm: Vec<u8>,
    ) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            inner
                .ops
                .process_sender_key_distribution(&group, &address(&user, device)?, &skdm)
                .await
                .map_err(ops_error)?;
            Ok(JsValue::UNDEFINED)
        })
    }

    #[wasm_bindgen(js_name = groupDecrypt)]
    pub fn group_decrypt(
        &self,
        group: String,
        user: String,
        device: u32,
        ciphertext: Vec<u8>,
    ) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let plaintext = inner
                .ops
                .group_decrypt(&group, &address(&user, device)?, &ciphertext)
                .await
                .map_err(ops_error)?;
            Ok(Uint8Array::from(plaintext.as_slice()).into())
        })
    }

    /// Decrypt, buffer and classify one received payload. Resolves
    /// `{ status: "message", receiptKey, message, isSkdmOnly, identityChanged,
    /// redelivered, senderKeyRejected, appStateKeyShare }` where `message` is
    /// the encoded, unpadded `waE2E.Message` without app-state key fields,
    /// or `{ status: "already_delivered", receiptKey }`. Call `markDelivered`
    /// once the app has stored the message. `share_app_state_keys` defaults
    /// off; enabling it exposes own-device app-state keys separately, allowing
    /// synchronized metadata decryption. It never enables foreign-device shares.
    #[wasm_bindgen(unchecked_return_type = "Promise<ReceiveResult>")]
    #[allow(clippy::too_many_arguments)]
    pub fn receive(
        &self,
        chat: String,
        user: String,
        device: u32,
        kind: String,
        ciphertext: Vec<u8>,
        padding_version: u8,
        is_from_me: bool,
        share_app_state_keys: Option<adapters::AppStateSharingConsent>,
    ) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let share_app_state_keys = match share_app_state_keys {
                None => false,
                Some(value) => value.as_bool().ok_or_else(|| {
                    convert::coded_error(
                        "invalid_input",
                        "app-state sharing consent must be boolean",
                    )
                })?,
            };
            let kind = ReceiveKind::try_from(kind.as_str()).map_err(|_| {
                convert::coded_error("invalid_input", &format!("unsupported enc type {kind}"))
            })?;
            let sender = address(&user, device)?;
            let received = inner
                .ops
                .receive(ReceiveRequest {
                    chat: &chat,
                    sender: &sender,
                    kind,
                    ciphertext: &ciphertext,
                    padding_version,
                    is_from_me,
                })
                .await
                .map_err(ops_error)?;
            let out = Object::new();
            match received {
                Received::AlreadyDelivered { receipt_key } => {
                    set(&out, "status", &JsValue::from_str("already_delivered"));
                    set(&out, "receiptKey", &JsValue::from_str(&receipt_key));
                }
                Received::Message(mut message) => {
                    set(&out, "status", &JsValue::from_str("message"));
                    set(&out, "receiptKey", &JsValue::from_str(&message.receipt_key));
                    if let Some(protocol) = message.content.message.protocol_message.as_option_mut()
                    {
                        protocol.app_state_sync_key_share = Default::default();
                        protocol.app_state_sync_key_request = Default::default();
                    }
                    message.content.message.sender_key_distribution_message = Default::default();
                    message
                        .content
                        .message
                        .fast_ratchet_key_sender_key_distribution_message = Default::default();
                    let encoded = waproto::codec::message_to_vec(&message.content.message);
                    set(&out, "message", &Uint8Array::from(encoded.as_slice()));
                    set(
                        &out,
                        "isSkdmOnly",
                        &JsValue::from_bool(message.content.is_skdm_only),
                    );
                    set(
                        &out,
                        "identityChanged",
                        &JsValue::from_bool(message.identity_changed),
                    );
                    set(
                        &out,
                        "redelivered",
                        &JsValue::from_bool(message.redelivered),
                    );
                    set(
                        &out,
                        "senderKeyRejected",
                        &JsValue::from_bool(message.sender_key_rejected),
                    );
                    let share = if share_app_state_keys && is_from_me {
                        message
                            .app_state_key_share
                            .as_ref()
                            .map(convert::app_state_share)
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    set(
                        &out,
                        "appStateKeyShare",
                        &Uint8Array::from(share.as_slice()),
                    );
                }
            }
            Ok(out.into())
        })
    }

    /// Drop a delivered message's plaintext from the decrypt buffer.
    #[wasm_bindgen(js_name = markDelivered)]
    pub fn mark_delivered(&self, receipt_key: String) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            inner
                .ops
                .mark_delivered(&receipt_key)
                .await
                .map_err(ops_error)?;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Delete decrypt-buffer entries older than `cutoffMs`; resolves the count.
    #[wasm_bindgen(js_name = pruneDecryptBuffer)]
    pub fn prune_decrypt_buffer(&self, cutoff_ms: f64) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let removed = inner
                .ops
                .prune_decrypt_buffer(cutoff_ms as u64)
                .await
                .map_err(ops_error)?;
            Ok(JsValue::from_f64(removed as f64))
        })
    }

    /// Record this device's own JIDs after pair success. Sends need them.
    #[wasm_bindgen(js_name = setOwnJids)]
    pub fn set_own_jids(&self, pn: String, lid: Option<String>) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            inner
                .ops
                .set_own_jids(&pn, lid.as_deref())
                .await
                .map_err(ops_error)?;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Build a direct-message stanza from an encoded `waE2E.Message`.
    /// Resolves `{ stanza, unreachedDevices, phash }`; transmit `stanza`
    /// through the WhatsApp client unchanged.
    #[wasm_bindgen(js_name = sendDirect)]
    pub fn send_direct(
        &self,
        resolver: send::SendResolver,
        to: String,
        message: Vec<u8>,
        message_id: String,
    ) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let to = convert::jid(&to)?;
            let message = convert::message(&message)?;
            let resolver = send::JsResolver::new(resolver);
            let sent = inner
                .ops
                .send_direct(&send::PageRuntime, &resolver, &to, &message, &message_id)
                .await
                .map_err(ops_error)?;
            let out = Object::new();
            set(&out, "stanza", &Uint8Array::from(sent.stanza.as_slice()));
            set(
                &out,
                "unreachedDevices",
                &convert::jid_list(&sent.unreached_devices),
            );
            set(
                &out,
                "phash",
                &sent.phash.map_or(JsValue::NULL, |p| JsValue::from_str(&p)),
            );
            Ok(out.into())
        })
    }

    /// Build a group stanza. Resolves `{ stanza, distributionTargets,
    /// distributionEpoch, staleDeviceUsers, phash }`; after the server acks
    /// it, pass `distributionTargets` and the `distributionEpoch` bigint to
    /// `markSenderKeyDistributed`.
    #[wasm_bindgen(js_name = sendGroup)]
    pub fn send_group(
        &self,
        resolver: send::SendResolver,
        group: String,
        message: Vec<u8>,
        message_id: String,
    ) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let group = convert::jid(&group)?;
            let message = convert::message(&message)?;
            let resolver = send::JsResolver::new(resolver);
            let sent = inner
                .ops
                .send_group(&send::PageRuntime, &resolver, &group, &message, &message_id)
                .await
                .map_err(ops_error)?;
            let out = Object::new();
            set(&out, "stanza", &Uint8Array::from(sent.stanza.as_slice()));
            set(
                &out,
                "distributionTargets",
                &convert::jid_list(&sent.distribution_targets),
            );
            set(
                &out,
                "distributionEpoch",
                &js_sys::BigInt::from(sent.distribution_epoch),
            );
            let stale: js_sys::Array = sent
                .stale_device_users
                .iter()
                .map(|u| JsValue::from_str(u))
                .collect();
            set(&out, "staleDeviceUsers", &stale);
            set(
                &out,
                "phash",
                &sent.phash.map_or(JsValue::NULL, |p| JsValue::from_str(&p)),
            );
            Ok(out.into())
        })
    }

    /// Record that the server acked a group send carrying the sender key.
    /// Resolves `false`, recording nothing, when the group's sender key has
    /// rotated since the send that returned `epoch`.
    #[wasm_bindgen(js_name = markSenderKeyDistributed)]
    pub fn mark_sender_key_distributed(
        &self,
        group: String,
        devices: Vec<String>,
        epoch: u64,
    ) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let group = convert::jid(&group)?;
            let devices = devices
                .iter()
                .map(|d| convert::jid(d))
                .collect::<Result<Vec<_>, _>>()?;
            let recorded = inner
                .ops
                .mark_sender_key_distributed(&group, &devices, epoch)
                .await
                .map_err(ops_error)?;
            Ok(JsValue::from_bool(recorded))
        })
    }

    /// Atomically rotate this device's group sender keys and clear distribution
    /// marks, so the next send uses a fresh key after membership changes.
    #[wasm_bindgen(js_name = forgetSenderKeyDevices)]
    pub fn forget_sender_key_devices(&self, group: String) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let group = convert::jid(&group)?;
            inner
                .ops
                .forget_sender_key_devices(&group)
                .await
                .map_err(ops_error)?;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Rebuild a stored sent message for a retry receipt. Only ciphertext
    /// stanza bytes leave this method. The caller sends them unchanged and
    /// applies its transport admission/pacing before sending.
    #[wasm_bindgen(unchecked_return_type = "Promise<Uint8Array>")]
    pub fn resend(
        &self,
        resolver: send::SendResolver,
        #[wasm_bindgen(unchecked_param_type = "RetryRequest")] request: JsValue,
    ) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let request = convert::retry_request(&request)?;
            let resolver = send::JsResolver::new(resolver);
            let stanza = inner
                .ops
                .resend(
                    &resolver,
                    wacore_signal_ops::RetryRequest {
                        chat: &request.chat,
                        message_id: &request.message_id,
                        requester: &request.requester,
                        encryption_jid: &request.encryption_jid,
                        route: request.route,
                        retry_count: request.retry_count,
                        bundle: request.bundle.as_ref(),
                    },
                )
                .await
                .map_err(ops_error)?;
            Ok(Uint8Array::from(stanza.as_slice()).into())
        })
    }

    /// Retire encrypted local retry records before this Unix-ms cutoff.
    #[wasm_bindgen(js_name = pruneSentMessages, unchecked_return_type = "Promise<number>")]
    pub fn prune_sent_messages(&self, cutoff_ms: f64) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let cutoff = convert::safe_u64(cutoff_ms, "cutoffMs").map_err(|_| {
                convert::coded_error(
                    "invalid_input",
                    "cutoffMs must be a non-negative safe integer",
                )
            })?;
            let count = inner
                .ops
                .prune_sent_messages(cutoff)
                .await
                .map_err(ops_error)?;
            Ok(JsValue::from_f64(count as f64))
        })
    }

    /// Drop the identity and sessions of every device of a user whose
    /// identity changed, as reported by the server.
    #[wasm_bindgen(js_name = forgetUser)]
    pub fn forget_user(&self, user: String) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let user = convert::jid(&user)?;
            inner.ops.forget_user(&user).await.map_err(ops_error)?;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// Move a user's sessions and identities from phone-number to LID
    /// addresses once the mapping is known. Resolves to how many moved.
    #[wasm_bindgen(js_name = migratePnToLid)]
    pub fn migrate_pn_to_lid(&self, pn: String, lid: String) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let pn = convert::jid(&pn)?;
            let lid = convert::jid(&lid)?;
            let moved = inner
                .ops
                .migrate_pn_to_lid(&pn, &lid)
                .await
                .map_err(ops_error)?;
            Ok(JsValue::from_f64(moved as f64))
        })
    }

    /// Verify the primary's pair-success container and add the device
    /// signature. Rejects with code "pairing_refused" and a `status`.
    #[wasm_bindgen(js_name = signPairing)]
    pub fn sign_pairing(&self, container: Vec<u8>) -> Promise {
        let inner = self.inner.clone();
        promise(async move {
            let signed = inner
                .ops
                .sign_pairing(&container)
                .await
                .map_err(ops_error)?;
            let out = Object::new();
            set(
                &out,
                "signedIdentity",
                &Uint8Array::from(signed.signed_identity.as_slice()),
            );
            set(
                &out,
                "keyIndex",
                &JsValue::from_f64(f64::from(signed.key_index)),
            );
            Ok(out.into())
        })
    }
}
