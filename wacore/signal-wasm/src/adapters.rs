// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! `RecordStore`, `LeaseStore` and `Sealer` backed by JS objects.
//!
//! wasm32 is single-threaded here, so the JS handles are wrapped in
//! `SendWrapper` only to satisfy the store traits' `Send + Sync` bounds; they
//! are never touched off the page's thread.

use crate::convert::{fence_to_js, lease_to_js, to_lease};
use async_trait::async_trait;
use js_sys::{Array, Object, Promise, Reflect, Uint8Array};
use send_wrapper::SendWrapper;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use wacore::store::error::{Result, StoreError};
use wacore_recordstore::{
    Fence, FenceLost, Lease, LeaseStore, Namespace, RecordExists, RecordStore, SealError, WriteOp,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "RecordBackend")]
    pub type RecordBackend;

    #[wasm_bindgen(method, catch)]
    fn get(
        this: &RecordBackend,
        scope: &str,
        ns: &str,
        key: &str,
    ) -> std::result::Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch, js_name = scanKeys)]
    fn scan_keys(
        this: &RecordBackend,
        scope: &str,
        ns: &str,
        prefix: &str,
        limit: Option<u32>,
    ) -> std::result::Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch)]
    fn write(
        this: &RecordBackend,
        fence: JsValue,
        ops: Array,
    ) -> std::result::Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch)]
    fn acquire(
        this: &RecordBackend,
        scope: &str,
        holder: &str,
        now_ms: f64,
        ttl_ms: f64,
    ) -> std::result::Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch)]
    fn renew(
        this: &RecordBackend,
        lease: JsValue,
        now_ms: f64,
        ttl_ms: f64,
    ) -> std::result::Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch)]
    fn release(this: &RecordBackend, lease: JsValue) -> std::result::Result<Promise, JsValue>;

    #[wasm_bindgen(typescript_type = "Sealer")]
    pub type Sealer;

    // An opaque JS value keeps bool consent strict while retaining an
    // optional boolean in generated TypeScript (no truthiness coercion).
    #[wasm_bindgen(typescript_type = "boolean")]
    pub type AppStateSharingConsent;

    #[wasm_bindgen(method, catch)]
    fn seal(
        this: &Sealer,
        aad: &Uint8Array,
        plaintext: &Uint8Array,
    ) -> std::result::Result<Promise, JsValue>;

    #[wasm_bindgen(method, catch)]
    fn open(
        this: &Sealer,
        aad: &Uint8Array,
        sealed: &Uint8Array,
    ) -> std::result::Result<Promise, JsValue>;
}

/// A JS failure carried as a store error. Holds only the message, never the
/// value, so record bytes cannot leak into logs through an error.
#[derive(Debug)]
struct JsFailure(String);

impl std::fmt::Display for JsFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for JsFailure {}

fn describe(error: &JsValue) -> String {
    Reflect::get(error, &JsValue::from_str("message"))
        .ok()
        .and_then(|m| m.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "JavaScript error".to_owned())
}

fn error_code(error: &JsValue) -> Option<String> {
    Reflect::get(error, &JsValue::from_str("code"))
        .ok()
        .and_then(|c| c.as_string())
}

fn store_error(error: JsValue) -> StoreError {
    StoreError::Database(Box::new(JsFailure(describe(&error))))
}

async fn settle(
    call: std::result::Result<Promise, JsValue>,
) -> std::result::Result<JsValue, JsValue> {
    JsFuture::from(call?).await
}

/// Opens per (scope, holder): live devices and opens still acquiring.
type LiveDevices = Mutex<HashMap<(Arc<str>, Arc<str>), usize>>;

pub struct JsRecords(SendWrapper<RecordBackend>, LiveDevices);

impl JsRecords {
    /// The adapter for `backend`, shared by every open over the same JS
    /// object. `AuthorityKey` identifies a backend by this allocation, so a
    /// fresh adapter per open would give two opens under one lease fence
    /// separate key caches and locks, and their writes could overwrite each
    /// other.
    pub fn shared(backend: RecordBackend) -> Arc<Self> {
        thread_local! {
            static OPEN: RefCell<Vec<Weak<JsRecords>>> = const { RefCell::new(Vec::new()) };
        }
        OPEN.with(|open| {
            let mut open = open.borrow_mut();
            open.retain(|records| records.strong_count() > 0);
            if let Some(records) = open
                .iter()
                .filter_map(Weak::upgrade)
                .find(|records| Object::is(&records.0, &backend))
            {
                return records;
            }
            let records = Arc::new(Self(SendWrapper::new(backend), Mutex::default()));
            open.push(Arc::downgrade(&records));
            records
        })
    }

    fn live_key(scope: &str, holder: &str) -> (Arc<str>, Arc<str>) {
        (Arc::from(scope), Arc::from(holder))
    }

    /// Count one open of (`scope`, `holder`) before it asks for the lease, so
    /// a concurrent open that fails cannot release a lease this one is about
    /// to share. Every `reserve` is paired with one `untrack`.
    pub fn reserve(&self, scope: &str, holder: &str) {
        if let Ok(mut live) = self.1.lock() {
            *live.entry(Self::live_key(scope, holder)).or_insert(0) += 1;
        }
    }

    /// Drop one open of (`scope`, `holder`). True when it was the last, so
    /// nobody else, live or still acquiring, uses the lease. The count and the
    /// answer are one step under the lock; the caller issues its release call
    /// before awaiting anything, so a later acquire is ordered after it.
    pub fn untrack(&self, scope: &str, holder: &str) -> bool {
        let Ok(mut live) = self.1.lock() else {
            return false;
        };
        let key = Self::live_key(scope, holder);
        let Some(count) = live.get_mut(&key) else {
            return true;
        };
        *count -= 1;
        if *count == 0 {
            live.remove(&key);
            return true;
        }
        false
    }
}

fn op_to_js(op: &WriteOp) -> JsValue {
    let out = Object::new();
    let (kind, ns, key, value) = match op {
        WriteOp::Put { ns, key, value } => ("put", ns, key, Some(value)),
        WriteOp::Update { ns, key, value } => ("update", ns, key, Some(value)),
        WriteOp::Delete { ns, key } => ("delete", ns, key, None),
        WriteOp::Insert { ns, key, value } => ("insert", ns, key, Some(value)),
    };
    crate::convert::set(&out, "op", &JsValue::from_str(kind));
    crate::convert::set(&out, "ns", &JsValue::from_str(ns.tag()));
    crate::convert::set(&out, "key", &JsValue::from_str(key));
    if let Some(value) = value {
        crate::convert::set(&out, "value", &Uint8Array::from(value.as_slice()));
    }
    out.into()
}

fn optional_lease(value: JsValue) -> Result<Option<Lease>> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    to_lease(&value).map(Some).map_err(StoreError::Validation)
}

#[async_trait(?Send)]
impl RecordStore for JsRecords {
    async fn get(&self, scope: &str, ns: Namespace, key: &str) -> Result<Option<Vec<u8>>> {
        let value = settle(self.0.get(scope, ns.tag(), key))
            .await
            .map_err(store_error)?;
        if value.is_null() || value.is_undefined() {
            return Ok(None);
        }
        let array: Uint8Array = value.dyn_into().map_err(|_| {
            StoreError::Validation("record backend returned a non-Uint8Array".into())
        })?;
        Ok(Some(array.to_vec()))
    }

    async fn scan_keys(
        &self,
        scope: &str,
        ns: Namespace,
        prefix: &str,
        limit: Option<usize>,
    ) -> Result<Vec<String>> {
        let limit = limit.map(|l| u32::try_from(l).unwrap_or(u32::MAX));
        let value = settle(self.0.scan_keys(scope, ns.tag(), prefix, limit))
            .await
            .map_err(store_error)?;
        let array: Array = value
            .dyn_into()
            .map_err(|_| StoreError::Validation("scanKeys must resolve to an array".into()))?;
        let mut keys = Vec::with_capacity(array.length() as usize);
        for item in array.iter() {
            keys.push(
                item.as_string().ok_or_else(|| {
                    StoreError::Validation("scanKeys returned a non-string".into())
                })?,
            );
        }
        // The contract is ascending byte order; sort rather than trust it,
        // since prekey ID lookups depend on it.
        keys.sort();
        if let Some(limit) = limit {
            keys.truncate(limit as usize);
        }
        Ok(keys)
    }

    async fn write(&self, fence: &Fence, ops: &[WriteOp]) -> Result<()> {
        let list = Array::new();
        for op in ops {
            list.push(&op_to_js(op));
        }
        settle(self.0.write(fence_to_js(fence), list))
            .await
            .map(|_| ())
            .map_err(|error| match error_code(&error).as_deref() {
                Some("fence_lost") => FenceLost {
                    scope: fence.scope.clone(),
                    generation: fence.generation,
                }
                .into_store_error(),
                Some("record_exists") => {
                    // The backend does not say which insert collided; report the
                    // first one in the batch.
                    let (ns, key) = ops
                        .iter()
                        .find_map(|op| match op {
                            WriteOp::Insert { ns, key, .. } => Some((*ns, key.clone())),
                            _ => None,
                        })
                        .unwrap_or((Namespace::Device, String::new()));
                    RecordExists { ns, key }.into_store_error()
                }
                _ => store_error(error),
            })
    }
}

#[async_trait(?Send)]
impl LeaseStore for JsRecords {
    async fn acquire(
        &self,
        scope: &str,
        holder: &str,
        now_ms: u64,
        ttl_ms: u64,
    ) -> Result<Option<Lease>> {
        let value = settle(self.0.acquire(scope, holder, now_ms as f64, ttl_ms as f64))
            .await
            .map_err(store_error)?;
        optional_lease(value)
    }

    async fn renew(&self, lease: &Lease, now_ms: u64, ttl_ms: u64) -> Result<Option<Lease>> {
        let value = settle(
            self.0
                .renew(lease_to_js(lease), now_ms as f64, ttl_ms as f64),
        )
        .await
        .map_err(store_error)?;
        optional_lease(value)
    }

    async fn release(&self, lease: &Lease) -> Result<()> {
        settle(self.0.release(lease_to_js(lease)))
            .await
            .map(|_| ())
            .map_err(store_error)
    }
}

pub struct JsSealerAdapter(SendWrapper<Sealer>);

impl JsSealerAdapter {
    pub fn new(sealer: Sealer) -> Self {
        Self(SendWrapper::new(sealer))
    }
}

async fn sealer_bytes(
    call: std::result::Result<Promise, JsValue>,
    authenticate: bool,
) -> std::result::Result<Vec<u8>, SealError> {
    let value = settle(call).await.map_err(|error| {
        if authenticate {
            // WebCrypto reports a failed tag check as an OperationError; any
            // rejection while opening means the record cannot be trusted.
            SealError::Authentication
        } else {
            SealError::Backend(describe(&error))
        }
    })?;
    let array: Uint8Array = value
        .dyn_into()
        .map_err(|_| SealError::Backend("sealer must resolve to a Uint8Array".into()))?;
    Ok(array.to_vec())
}

#[async_trait(?Send)]
impl wacore_recordstore::Sealer for JsSealerAdapter {
    async fn seal(&self, aad: &[u8], plaintext: &[u8]) -> std::result::Result<Vec<u8>, SealError> {
        let call = self
            .0
            .seal(&Uint8Array::from(aad), &Uint8Array::from(plaintext));
        sealer_bytes(call, false).await
    }

    async fn open(&self, aad: &[u8], sealed: &[u8]) -> std::result::Result<Vec<u8>, SealError> {
        let call = self
            .0
            .open(&Uint8Array::from(aad), &Uint8Array::from(sealed));
        sealer_bytes(call, true).await
    }
}
