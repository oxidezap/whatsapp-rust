// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! End-to-end check of the wasm32 bindings, driven from Rust inside a
//! wasm-bindgen host (Node by default).
//!
//! Run from the repository root:
//!   RUSTFLAGS='--cfg getrandom_backend="wasm_js"' \
//!   CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
//!   cargo test -p wacore-signal-wasm --target wasm32-unknown-unknown
//!
//! The backend below is an in-memory reference for the `RecordBackend`
//! contract and the resolvers are scripted `SendResolver`s. Both are exported
//! with `#[wasm_bindgen]` so the crate reaches them through the same JS
//! method calls it makes on a host's objects. The sealer is AES-256-GCM with
//! the record's AAD authenticated, as a WebCrypto sealer would be.
#![cfg(target_arch = "wasm32")]

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit};
use js_sys::{Array, Object, Promise, Reflect, Uint8Array, Uint32Array};
use rand::Rng;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::rc::Rc;
use std::task::Poll;
use wacore_signal_wasm::SignalDevice;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::wasm_bindgen_test;

const TTL: f64 = 60_000.0;
const ALICE: &str = "111@s.whatsapp.net";
const BOB: &str = "222@s.whatsapp.net";
const BOB_DEVICE: &str = "222:1@s.whatsapp.net";
const GROUP: &str = "120363000000001@g.us";

// ---- JS value helpers -------------------------------------------------

fn get(value: &JsValue, key: &str) -> JsValue {
    Reflect::get(value, &JsValue::from_str(key)).unwrap_or(JsValue::UNDEFINED)
}

fn set(object: &Object, key: &str, value: &JsValue) {
    Reflect::set(object, &JsValue::from_str(key), value).unwrap();
}

fn bytes_of(value: &JsValue) -> Vec<u8> {
    Uint8Array::new(value).to_vec()
}

fn string_of(value: &JsValue) -> String {
    value.as_string().expect("a string")
}

fn strings_of(value: &JsValue) -> Vec<String> {
    Array::from(value).iter().map(|v| string_of(&v)).collect()
}

fn number_of(value: &JsValue) -> f64 {
    value.as_f64().expect("a number")
}

/// Structural equality for plain data: JSON text of both sides.
fn same(a: &JsValue, b: &JsValue) -> bool {
    js_sys::JSON::stringify(a).unwrap().as_string()
        == js_sys::JSON::stringify(b).unwrap().as_string()
}

fn code_of(error: &JsValue) -> Option<String> {
    get(error, "code").as_string()
}

fn message_of(error: &JsValue) -> String {
    get(error, "message")
        .as_string()
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"))
}

fn coded_error(code: &str, message: &str) -> JsValue {
    let error = js_sys::Error::new(message);
    Reflect::set(&error, &JsValue::from_str("code"), &JsValue::from_str(code)).unwrap();
    error.into()
}

/// Await a binding call that must resolve.
async fn run(promise: Promise) -> JsValue {
    match JsFuture::from(promise).await {
        Ok(value) => value,
        Err(error) => panic!("binding call rejected: {}", message_of(&error)),
    }
}

/// Await a binding call that must reject, and return the rejection.
async fn rejected(promise: Promise) -> JsValue {
    match JsFuture::from(promise).await {
        Ok(value) => panic!("binding call resolved with {value:?}"),
        Err(error) => error,
    }
}

async fn rejects_with(promise: Promise, code: &str) {
    let error = rejected(promise).await;
    assert_eq!(
        code_of(&error).as_deref(),
        Some(code),
        "{}",
        message_of(&error)
    );
}

fn expect_err<T>(result: Result<T, JsValue>) -> JsValue {
    match result {
        Ok(_) => panic!("expected the call to reject"),
        Err(error) => error,
    }
}

async fn sleep_ms(ms: u32) {
    let promise = Promise::new(&mut |resolve, _reject| {
        let set_timeout: js_sys::Function = Reflect::get(&js_sys::global(), &"setTimeout".into())
            .unwrap()
            .dyn_into()
            .unwrap();
        set_timeout
            .call2(&JsValue::NULL, &resolve, &JsValue::from_f64(f64::from(ms)))
            .unwrap();
    });
    JsFuture::from(promise).await.unwrap();
}

/// Poll two futures to completion together, as `Promise.allSettled` would.
async fn join2<A: Future, B: Future>(a: A, b: B) -> (A::Output, B::Output) {
    let mut a = Box::pin(a);
    let mut b = Box::pin(b);
    let (mut out_a, mut out_b) = (None, None);
    std::future::poll_fn(|cx| {
        if out_a.is_none()
            && let Poll::Ready(value) = a.as_mut().poll(cx)
        {
            out_a = Some(value);
        }
        if out_b.is_none()
            && let Poll::Ready(value) = b.as_mut().poll(cx)
        {
            out_b = Some(value);
        }
        if out_a.is_some() && out_b.is_some() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
    .await;
    (out_a.unwrap(), out_b.unwrap())
}

// ---- RecordBackend ----------------------------------------------------

/// One write op as the backend received it.
struct Op {
    kind: String,
    ns: String,
    key: String,
    value: Vec<u8>,
}

struct LeaseRow {
    generation: f64,
    holder: String,
    expires_at_ms: f64,
}

type OpsPredicate = fn(&[Op]) -> bool;

#[derive(Default)]
struct State {
    /// Keyed `scope \0 ns \0 key`, so a scan is a sorted prefix walk.
    records: RefCell<BTreeMap<String, Vec<u8>>>,
    leases: RefCell<HashMap<String, LeaseRow>>,
    /// Refuse a write before it commits, once.
    fail_before: Cell<Option<OpsPredicate>>,
    /// Commit a write, then report it lost, once.
    fail_after: Cell<Option<OpsPredicate>>,
    /// Delay every `get`, so concurrent opens interleave.
    read_delay_ms: Cell<u32>,
}

fn slot(scope: &str, ns: &str, key: &str) -> String {
    format!("{scope}\0{ns}\0{key}")
}

impl State {
    fn contains_bytes(&self, needle: &[u8]) -> bool {
        self.records
            .borrow()
            .values()
            .any(|value| value.windows(needle.len()).any(|w| w == needle))
    }

    fn keys_in_namespace(&self, ns: &str) -> usize {
        let marker = format!("\0{ns}\0");
        self.records
            .borrow()
            .keys()
            .filter(|key| key.contains(&marker))
            .count()
    }
}

fn lease_object(scope: &str, row: &LeaseRow) -> JsValue {
    let out = Object::new();
    set(&out, "scope", &JsValue::from_str(scope));
    set(&out, "generation", &JsValue::from_f64(row.generation));
    set(&out, "holder", &JsValue::from_str(&row.holder));
    set(&out, "expiresAtMs", &JsValue::from_f64(row.expires_at_ms));
    out.into()
}

fn parse_ops(ops: &Array) -> Vec<Op> {
    ops.iter()
        .map(|op| Op {
            kind: string_of(&get(&op, "op")),
            ns: string_of(&get(&op, "ns")),
            key: string_of(&get(&op, "key")),
            value: {
                let value = get(&op, "value");
                if value.is_undefined() {
                    Vec::new()
                } else {
                    bytes_of(&value)
                }
            },
        })
        .collect()
}

/// An in-memory `RecordBackend`, exported so the crate calls it by JS name.
#[wasm_bindgen]
pub struct MemoryBackend {
    state: Rc<State>,
}

#[wasm_bindgen]
impl MemoryBackend {
    pub async fn get(&self, scope: String, ns: String, key: String) -> Result<JsValue, JsValue> {
        let delay = self.state.read_delay_ms.get();
        if delay > 0 {
            sleep_ms(delay).await;
        }
        Ok(self
            .state
            .records
            .borrow()
            .get(&slot(&scope, &ns, &key))
            .map_or(JsValue::NULL, |value| {
                Uint8Array::from(value.as_slice()).into()
            }))
    }

    #[wasm_bindgen(js_name = scanKeys)]
    pub async fn scan_keys(
        &self,
        scope: String,
        ns: String,
        prefix: String,
        limit: Option<u32>,
    ) -> Result<JsValue, JsValue> {
        let head = format!("{scope}\0{ns}\0");
        let wanted = format!("{head}{prefix}");
        let keys: Array = self
            .state
            .records
            .borrow()
            .keys()
            .filter(|slot| slot.starts_with(&wanted))
            .map(|slot| JsValue::from_str(&slot[head.len()..]))
            .take(limit.map_or(usize::MAX, |l| l as usize))
            .collect();
        Ok(keys.into())
    }

    pub async fn write(&self, fence: JsValue, ops: Array) -> Result<JsValue, JsValue> {
        let state = &self.state;
        let ops = parse_ops(&ops);
        if let Some(fails) = state.fail_before.get()
            && fails(&ops)
        {
            state.fail_before.set(None);
            return Err(js_sys::Error::new("synthetic commit refused").into());
        }
        let scope = string_of(&get(&fence, "scope"));
        let generation = number_of(&get(&fence, "generation"));
        if state.leases.borrow().get(&scope).map(|row| row.generation) != Some(generation) {
            return Err(coded_error("fence_lost", "fence lost"));
        }
        {
            let mut records = state.records.borrow_mut();
            if ops.iter().any(|op| {
                op.kind == "insert" && records.contains_key(&slot(&scope, &op.ns, &op.key))
            }) {
                return Err(coded_error("record_exists", "record exists"));
            }
            for op in &ops {
                let at = slot(&scope, &op.ns, &op.key);
                match op.kind.as_str() {
                    "put" | "insert" => {
                        records.insert(at, op.value.clone());
                    }
                    "update" => {
                        if let Some(existing) = records.get_mut(&at) {
                            *existing = op.value.clone();
                        }
                    }
                    _ => {
                        records.remove(&at);
                    }
                }
            }
        }
        if let Some(fails) = state.fail_after.get()
            && fails(&ops)
        {
            state.fail_after.set(None);
            return Err(js_sys::Error::new("synthetic response lost").into());
        }
        Ok(JsValue::UNDEFINED)
    }

    pub async fn acquire(
        &self,
        scope: String,
        holder: String,
        now_ms: f64,
        ttl_ms: f64,
    ) -> Result<JsValue, JsValue> {
        let mut leases = self.state.leases.borrow_mut();
        let generation = match leases.get(&scope) {
            None => 1.0,
            Some(row) if row.holder == holder => row.generation,
            Some(row) if row.expires_at_ms > now_ms => return Ok(JsValue::NULL),
            Some(row) => row.generation + 1.0,
        };
        let row = LeaseRow {
            generation,
            holder,
            expires_at_ms: now_ms + ttl_ms,
        };
        let out = lease_object(&scope, &row);
        leases.insert(scope, row);
        Ok(out)
    }

    pub async fn renew(
        &self,
        lease: JsValue,
        now_ms: f64,
        ttl_ms: f64,
    ) -> Result<JsValue, JsValue> {
        let scope = string_of(&get(&lease, "scope"));
        let mut leases = self.state.leases.borrow_mut();
        match leases.get_mut(&scope) {
            Some(row)
                if row.generation == number_of(&get(&lease, "generation"))
                    && row.holder == string_of(&get(&lease, "holder")) =>
            {
                row.expires_at_ms = now_ms + ttl_ms;
                Ok(lease_object(&scope, row))
            }
            _ => Ok(JsValue::NULL),
        }
    }

    pub async fn release(&self, lease: JsValue) -> Result<JsValue, JsValue> {
        let scope = string_of(&get(&lease, "scope"));
        if let Some(row) = self.state.leases.borrow_mut().get_mut(&scope)
            && row.generation == number_of(&get(&lease, "generation"))
            && row.holder == string_of(&get(&lease, "holder"))
        {
            row.expires_at_ms = 0.0;
            row.holder = String::new();
        }
        Ok(JsValue::UNDEFINED)
    }
}

// ---- Sealer -----------------------------------------------------------

/// AES-256-GCM with a random nonce prepended and the AAD authenticated.
#[wasm_bindgen]
pub struct TestSealer {
    cipher: Aes256Gcm,
}

impl TestSealer {
    fn new() -> Self {
        let mut key = [0u8; 32];
        rand::make_rng::<rand::rngs::StdRng>().fill_bytes(&mut key);
        Self {
            cipher: Aes256Gcm::new(&key.into()),
        }
    }
}

#[wasm_bindgen]
impl TestSealer {
    pub async fn seal(
        &self,
        aad: Uint8Array,
        plaintext: Uint8Array,
    ) -> Result<Uint8Array, JsValue> {
        let mut iv = [0u8; 12];
        rand::make_rng::<rand::rngs::StdRng>().fill_bytes(&mut iv);
        let sealed = self
            .cipher
            .encrypt(
                &iv.into(),
                Payload {
                    msg: &plaintext.to_vec(),
                    aad: &aad.to_vec(),
                },
            )
            .map_err(|_| js_sys::Error::new("seal failed"))?;
        let mut out = iv.to_vec();
        out.extend_from_slice(&sealed);
        Ok(Uint8Array::from(out.as_slice()))
    }

    pub async fn open(&self, aad: Uint8Array, sealed: Uint8Array) -> Result<Uint8Array, JsValue> {
        let sealed = sealed.to_vec();
        if sealed.len() < 12 {
            return Err(js_sys::Error::new("sealed value too short").into());
        }
        let (iv, body) = sealed.split_at(12);
        let iv: [u8; 12] = iv.try_into().unwrap();
        let plain = self
            .cipher
            .decrypt(
                &iv.into(),
                Payload {
                    msg: body,
                    aad: &aad.to_vec(),
                },
            )
            .map_err(|_| js_sys::Error::new("authentication failed"))?;
        Ok(Uint8Array::from(plain.as_slice()))
    }
}

// ---- SendResolver -----------------------------------------------------

/// What a scripted resolver answers, and what it was asked.
#[derive(Default)]
struct ResolverState {
    devices: Vec<String>,
    /// Bundle by exact device JID, then `fallback` for any other JID.
    bundles: Vec<(String, JsValue)>,
    fallback: Option<JsValue>,
    /// The group `resolveGroup` expects and its members; none means a
    /// direct-only resolver whose `resolveGroup` fails.
    group: Option<(String, Vec<String>)>,
    /// `(phone user, LID user)` answered by `lidForPhone`.
    lid: Option<(String, String)>,
    device_calls: RefCell<Vec<Vec<String>>>,
    prekey_calls: RefCell<Vec<Vec<String>>>,
    lid_lookups: RefCell<Vec<String>>,
}

impl ResolverState {
    fn resolve_devices(&self, jids: Array) -> Result<Array, JsValue> {
        let mut asked = strings_of(&jids.into());
        asked.sort();
        self.device_calls.borrow_mut().push(asked);
        Ok(self.devices.iter().map(|d| JsValue::from_str(d)).collect())
    }

    fn fetch_prekeys(&self, jids: Array) -> Result<Object, JsValue> {
        let asked = strings_of(&jids.into());
        self.prekey_calls.borrow_mut().push(asked.clone());
        let out = Object::new();
        for jid in asked {
            let bundle = self
                .bundles
                .iter()
                .find(|(known, _)| *known == jid)
                .map(|(_, bundle)| bundle)
                .or(self.fallback.as_ref());
            if let Some(bundle) = bundle {
                set(&out, &jid, bundle);
            }
        }
        Ok(out)
    }

    fn resolve_group(&self, jid: &str) -> Result<Object, JsValue> {
        let Some((group, participants)) = &self.group else {
            return Err(js_sys::Error::new("not a group send").into());
        };
        if jid != group {
            return Err(js_sys::Error::new("unexpected group").into());
        }
        let out = Object::new();
        let participants: Array = participants.iter().map(|p| JsValue::from_str(p)).collect();
        set(&out, "participants", &participants);
        set(&out, "addressingMode", &JsValue::from_str("pn"));
        Ok(out)
    }
}

/// A `SendResolver` without the optional `lidForPhone`.
#[wasm_bindgen]
pub struct Resolver {
    state: Rc<ResolverState>,
}

#[wasm_bindgen]
impl Resolver {
    #[wasm_bindgen(js_name = resolveDevices)]
    pub async fn resolve_devices(&self, jids: Array) -> Result<Array, JsValue> {
        self.state.resolve_devices(jids)
    }

    #[wasm_bindgen(js_name = fetchPrekeys)]
    pub async fn fetch_prekeys(&self, jids: Array) -> Result<Object, JsValue> {
        self.state.fetch_prekeys(jids)
    }

    #[wasm_bindgen(js_name = resolveGroup)]
    pub async fn resolve_group(&self, jid: String) -> Result<Object, JsValue> {
        self.state.resolve_group(&jid)
    }
}

/// A `SendResolver` that also answers `lidForPhone`.
#[wasm_bindgen]
pub struct LidResolver {
    state: Rc<ResolverState>,
}

#[wasm_bindgen]
impl LidResolver {
    #[wasm_bindgen(js_name = resolveDevices)]
    pub async fn resolve_devices(&self, jids: Array) -> Result<Array, JsValue> {
        self.state.resolve_devices(jids)
    }

    #[wasm_bindgen(js_name = fetchPrekeys)]
    pub async fn fetch_prekeys(&self, jids: Array) -> Result<Object, JsValue> {
        self.state.fetch_prekeys(jids)
    }

    #[wasm_bindgen(js_name = lidForPhone)]
    pub async fn lid_for_phone(&self, user: String) -> Result<JsValue, JsValue> {
        self.state.lid_lookups.borrow_mut().push(user.clone());
        Ok(match &self.state.lid {
            Some((phone, lid)) if *phone == user => JsValue::from_str(lid),
            _ => JsValue::UNDEFINED,
        })
    }

    #[wasm_bindgen(js_name = resolveGroup)]
    pub async fn resolve_group(&self, jid: String) -> Result<Object, JsValue> {
        self.state.resolve_group(&jid)
    }
}

fn plain_resolver(state: &Rc<ResolverState>) -> JsValue {
    Resolver {
        state: state.clone(),
    }
    .into()
}

fn lid_resolver(state: &Rc<ResolverState>) -> JsValue {
    LidResolver {
        state: state.clone(),
    }
    .into()
}

/// Every device JID gets `bundle`, and `GROUP` has Alice and Bob.
fn resolver_for(bundle: JsValue) -> Rc<ResolverState> {
    Rc::new(ResolverState {
        devices: vec![BOB_DEVICE.into()],
        fallback: Some(bundle),
        group: Some((GROUP.into(), vec![ALICE.into(), BOB.into()])),
        ..ResolverState::default()
    })
}

// ---- Devices ----------------------------------------------------------

/// A backend and sealer pair. Cloning shares the same JS objects, which is
/// what the crate keys its per-backend authority on.
#[derive(Clone)]
struct Harness {
    state: Rc<State>,
    backend: JsValue,
    sealer: JsValue,
}

impl Harness {
    fn new() -> Self {
        let state = Rc::new(State::default());
        Self {
            backend: MemoryBackend {
                state: state.clone(),
            }
            .into(),
            sealer: TestSealer::new().into(),
            state,
        }
    }

    async fn open(
        &self,
        scope: &str,
        holder: &str,
        now_ms: f64,
        create: bool,
    ) -> Result<SignalDevice, JsValue> {
        open_with(&self.backend, &self.sealer, scope, holder, now_ms, create).await
    }
}

async fn open_with(
    backend: &JsValue,
    sealer: &JsValue,
    scope: &str,
    holder: &str,
    now_ms: f64,
    create: bool,
) -> Result<SignalDevice, JsValue> {
    SignalDevice::open(
        backend.clone().unchecked_into(),
        sealer.clone().unchecked_into(),
        scope.into(),
        holder.into(),
        now_ms,
        TTL,
        create,
    )
    .await
}

struct Dev {
    signal: SignalDevice,
    h: Harness,
}

async fn device(scope: &str) -> Dev {
    let h = Harness::new();
    let signal = h.open(scope, "tab-a", 0.0, true).await.ok().unwrap();
    Dev { signal, h }
}

async fn bundle_for(signal: &SignalDevice, with_prekey: bool) -> JsValue {
    let identity = run(signal.public_identity()).await;
    let signed = get(&identity, "signedPrekey");
    let bundle = Object::new();
    set(&bundle, "registrationId", &get(&identity, "registrationId"));
    set(&bundle, "deviceId", &JsValue::from_f64(1.0));
    set(&bundle, "identityKey", &get(&identity, "identityKey"));
    set(&bundle, "signedPrekeyId", &get(&signed, "id"));
    set(&bundle, "signedPrekey", &get(&signed, "publicKey"));
    set(&bundle, "signedPrekeySignature", &get(&signed, "signature"));
    if with_prekey {
        let prekeys = run(signal.generate_prekeys(1)).await;
        let prekey = Array::from(&prekeys).get(0);
        set(&bundle, "prekeyId", &get(&prekey, "id"));
        set(&bundle, "prekey", &get(&prekey, "publicKey"));
    }
    bundle.into()
}

async fn establish(from: &SignalDevice, to: &str, bundle: JsValue) {
    run(from.establish_session(to.into(), 1, bundle)).await;
}

async fn encrypt(from: &SignalDevice, to: &str, plaintext: &[u8]) -> (String, Vec<u8>) {
    let out = run(from.encrypt(to.into(), 1, plaintext.to_vec())).await;
    (
        string_of(&get(&out, "kind")),
        bytes_of(&get(&out, "ciphertext")),
    )
}

async fn decrypt(at: &SignalDevice, from: &str, kind: &str, ciphertext: &[u8]) -> Vec<u8> {
    let out = run(at.decrypt(from.into(), 1, kind.into(), ciphertext.to_vec())).await;
    bytes_of(&get(&out, "plaintext"))
}

#[allow(clippy::too_many_arguments)]
async fn receive_with(
    at: &SignalDevice,
    chat: &str,
    kind: &str,
    ciphertext: &[u8],
    padding_version: u8,
    from_me: bool,
    consent: Option<JsValue>,
) -> Result<JsValue, JsValue> {
    JsFuture::from(at.receive(
        chat.into(),
        ALICE.into(),
        1,
        kind.into(),
        ciphertext.to_vec(),
        padding_version,
        from_me,
        consent.map(JsCast::unchecked_into),
    ))
    .await
}

/// Receive as a message from Alice with padding version 2.
async fn receive(at: &SignalDevice, kind: &str, ciphertext: &[u8]) -> JsValue {
    receive_with(at, ALICE, kind, ciphertext, 2, false, None)
        .await
        .unwrap_or_else(|e| panic!("receive rejected: {}", message_of(&e)))
}

fn ids_of(prekeys: &JsValue) -> Vec<u32> {
    Array::from(prekeys)
        .iter()
        .map(|key| number_of(&get(&key, "id")) as u32)
        .collect()
}

fn id_array(ids: &[u32]) -> Uint32Array {
    Uint32Array::from(ids)
}

async fn count(d: &SignalDevice) -> f64 {
    number_of(&run(d.uploaded_prekey_count()).await)
}

/// The application persists the message before acknowledging delivery. The
/// bytes are held here in place of a durable store; what matters to the
/// bindings is that `markDelivered` follows persistence.
async fn persist_then_mark(d: &SignalDevice, result: &JsValue) {
    let persisted = bytes_of(&get(result, "message"));
    assert!(!persisted.is_empty(), "nothing to persist");
    run(d.mark_delivered(string_of(&get(result, "receiptKey")))).await;
}

// ---- Wire fixtures ----------------------------------------------------

fn text(s: &str) -> Vec<u8> {
    s.as_bytes().to_vec()
}

/// A `waE2E.Message` with only `conversation` (field 1), padded as WhatsApp
/// pads (n bytes of value n, 1..=16).
fn padded_conversation(body: &str) -> Vec<u8> {
    pad(&conversation(body))
}

fn conversation(body: &str) -> Vec<u8> {
    let mut out = vec![0x0a, body.len() as u8];
    out.extend_from_slice(body.as_bytes());
    out
}

fn conversation_of(message: &[u8]) -> String {
    assert_eq!(message[0], 0x0a);
    String::from_utf8(message[2..2 + message[1] as usize].to_vec()).unwrap()
}

// Synthetic protobuf fixtures follow waproto/src/whatsapp.proto field numbers.
fn varint(mut value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let low = (value & 127) as u8;
        value >>= 7;
        if value == 0 {
            out.push(low);
            return out;
        }
        out.push(low | 128);
    }
}

fn field(number: u64, bytes: &[u8]) -> Vec<u8> {
    let mut out = varint(number * 8 + 2);
    out.extend(varint(bytes.len() as u64));
    out.extend_from_slice(bytes);
    out
}

fn pad(bytes: &[u8]) -> Vec<u8> {
    let count = 1 + bytes.len() % 16;
    let mut out = bytes.to_vec();
    out.extend(std::iter::repeat_n(count as u8, count));
    out
}

fn contains_run(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// A synthetic primary-device pair-success container for `device`'s public
/// identity: signed with a throwaway account key and MACed with its ADV
/// secret.
// Test-only protos have no pinned codec root, as in the native pairing tests.
#[allow(clippy::disallowed_methods)]
fn pairing_container(identity_key: &[u8], adv_secret: &[u8]) -> Vec<u8> {
    use buffa::Message as _;
    use hmac::{KeyInit, Mac};
    use wacore::libsignal::protocol::{KeyPair, PublicKey};
    use waproto::whatsapp as wa;
    let public: [u8; 32] = identity_key.try_into().unwrap();
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let account = KeyPair::generate(&mut rng);
    let device = KeyPair::new(
        PublicKey::from_djb_public_key_bytes(&public).unwrap(),
        KeyPair::generate(&mut rng).private_key,
    );
    let mut details = wa::ADVDeviceIdentity::default();
    details.raw_id = Some(1);
    details.timestamp = Some(0);
    details.key_index = Some(1);
    details.account_type = Some(wa::ADVEncryptionType::E2EE);
    details.device_type = Some(wa::ADVEncryptionType::E2EE);
    let details = details.encode_to_vec();
    let signed = wacore::adv::test_util::signed_identity(
        &account,
        &device,
        &details,
        wacore::adv::test_util::account_prefix(Some(wa::ADVEncryptionType::E2EE)),
        None,
        true,
    )
    .encode_to_vec();
    let mut mac = <hmac::Hmac<sha2::Sha256> as KeyInit>::new_from_slice(adv_secret).unwrap();
    mac.update(&signed);
    let mut container = wa::ADVSignedDeviceIdentityHMAC::default();
    container.details = Some(signed);
    container.hmac = Some(mac.finalize().into_bytes().to_vec());
    container.account_type = Some(wa::ADVEncryptionType::E2EE);
    container.encode_to_vec()
}

async fn pair(signal: &SignalDevice) {
    let identity = run(signal.public_identity()).await;
    let container = pairing_container(
        &bytes_of(&get(&identity, "identityKey")),
        &bytes_of(&get(&identity, "advSecret")),
    );
    run(signal.sign_pairing(container)).await;
}

fn stanza(bytes: &[u8]) -> wacore_binary::Node {
    wacore_binary::marshal::unmarshal_packed_ref(bytes)
        .unwrap()
        .to_owned()
}

fn attr(node: &wacore_binary::Node, name: &str) -> Option<String> {
    node.attrs
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.to_string())
}

fn find<'a>(node: &'a wacore_binary::Node, tag: &str) -> Option<&'a wacore_binary::Node> {
    if node.tag == tag {
        return Some(node);
    }
    node.children()?.iter().find_map(|child| find(child, tag))
}

/// Receive the `enc` payload of a built stanza as a message from Alice.
async fn receive_stanza(at: &SignalDevice, encoded: &[u8], chat: &str) -> JsValue {
    let tree = stanza(encoded);
    let enc = find(&tree, "enc").expect("ciphertext stanza has enc");
    let Some(wacore_binary::NodeContent::Bytes(data)) = &enc.content else {
        panic!("enc carries bytes");
    };
    let version: u8 = attr(enc, "v").unwrap().parse().unwrap();
    receive_with(
        at,
        chat,
        &attr(enc, "type").unwrap(),
        data,
        version,
        false,
        None,
    )
    .await
    .unwrap_or_else(|e| panic!("receive rejected: {}", message_of(&e)))
}

// ---- Tests (ported from the former tests/node.test.mjs) ---------------

#[wasm_bindgen_test]
async fn a_session_runs_both_ways_and_nothing_readable_reaches_storage() {
    let alice = device("alice").await;
    let bob = device("bob").await;
    establish(&alice.signal, BOB, bundle_for(&bob.signal, true).await).await;

    let (kind, ciphertext) = encrypt(&alice.signal, BOB, b"hello bob").await;
    assert_eq!(kind, "pkmsg");
    assert_eq!(
        decrypt(&bob.signal, ALICE, &kind, &ciphertext).await,
        b"hello bob"
    );

    let (kind, ciphertext) = encrypt(&bob.signal, ALICE, b"hi alice").await;
    assert_eq!(kind, "msg");
    assert_eq!(
        decrypt(&alice.signal, BOB, &kind, &ciphertext).await,
        b"hi alice"
    );

    for round in 0..5 {
        let body = format!("a{round}");
        let (kind, ciphertext) = encrypt(&alice.signal, BOB, body.as_bytes()).await;
        assert_eq!(
            decrypt(&bob.signal, ALICE, &kind, &ciphertext).await,
            body.as_bytes()
        );
    }

    assert!(!bob.h.state.contains_bytes(b"hello bob"));
    let identity = run(bob.signal.public_identity()).await;
    assert!(
        !bob.h
            .state
            .contains_bytes(&bytes_of(&get(&identity, "advSecret")))
    );
}

#[wasm_bindgen_test]
async fn a_second_tab_is_refused_while_the_lease_is_live_then_fences_the_first_out() {
    let alice = device("alice-failover").await;
    let h = Harness::new();
    let bob_a = h
        .open("bob-failover", "tab-a", 0.0, true)
        .await
        .ok()
        .unwrap();
    establish(&alice.signal, BOB, bundle_for(&bob_a, true).await).await;
    let (kind, ciphertext) = encrypt(&alice.signal, BOB, b"before").await;
    decrypt(&bob_a, ALICE, &kind, &ciphertext).await;

    let refused = expect_err(h.open("bob-failover", "tab-b", 1.0, false).await);
    assert_eq!(code_of(&refused).as_deref(), Some("lease_held"));

    let bob_b = h
        .open("bob-failover", "tab-b", TTL + 1.0, false)
        .await
        .ok()
        .unwrap();
    let (kind, ciphertext) = encrypt(&alice.signal, BOB, b"after").await;
    rejects_with(
        bob_a.decrypt(ALICE.into(), 1, kind.clone(), ciphertext.clone()),
        "fence_lost",
    )
    .await;
    assert_eq!(run(bob_a.renew_lease(TTL + 2.0, TTL)).await, JsValue::FALSE);
    assert_eq!(decrypt(&bob_b, ALICE, &kind, &ciphertext).await, b"after");
}

#[wasm_bindgen_test]
async fn opens_of_one_scope_by_the_same_holder_share_keys_and_locks() {
    let h = Harness::new();
    let first = h
        .open("same-holder", "tab-a", 0.0, true)
        .await
        .ok()
        .unwrap();
    // Renewal by the same holder keeps the fence, so only a shared authority
    // keeps the two opens from writing over each other.
    let second = h
        .open("same-holder", "tab-a", 1.0, false)
        .await
        .ok()
        .unwrap();
    let both = run(Promise::all(&Array::of2(
        &first.rotate_signed_prekey(),
        &second.rotate_signed_prekey(),
    )))
    .await;
    let both = Array::from(&both);
    assert_ne!(
        number_of(&get(&both.get(0), "id")),
        number_of(&get(&both.get(1), "id")),
        "rotations under one fence must not reuse an id"
    );
    let latest = get(&run(first.public_identity()).await, "signedPrekey");
    assert!(same(
        &get(&run(second.public_identity()).await, "signedPrekey"),
        &latest
    ));
}

#[wasm_bindgen_test]
async fn group_messages_decrypt_with_a_distributed_sender_key() {
    let alice = device("alice-group").await;
    let bob = device("bob-group").await;
    establish(&alice.signal, BOB, bundle_for(&bob.signal, true).await).await;

    let skdm = bytes_of(
        &run(alice
            .signal
            .sender_key_distribution(GROUP.into(), ALICE.into(), 1))
        .await,
    );
    let (kind, ciphertext) = encrypt(&alice.signal, BOB, &skdm).await;
    let received = decrypt(&bob.signal, ALICE, &kind, &ciphertext).await;
    run(bob
        .signal
        .process_sender_key_distribution(GROUP.into(), ALICE.into(), 1, received))
    .await;

    let skmsg = bytes_of(
        &run(alice
            .signal
            .group_encrypt(GROUP.into(), ALICE.into(), 1, text("hello group")))
        .await,
    );
    let plain = run(bob
        .signal
        .group_decrypt(GROUP.into(), ALICE.into(), 1, skmsg))
    .await;
    assert_eq!(bytes_of(&plain), b"hello group");
}

#[wasm_bindgen_test]
async fn a_sealer_with_the_wrong_key_cannot_open_the_device_and_the_lease_is_released() {
    let first = device("bob-key").await;
    run(first.signal.release()).await;
    let wrong: JsValue = TestSealer::new().into();
    let error =
        expect_err(open_with(&first.h.backend, &wrong, "bob-key", "tab-b", 1.0, false).await);
    assert_eq!(code_of(&error).as_deref(), Some("store"));
    // The failed open gave its lease back, so another tab can take the scope.
    first
        .h
        .open("bob-key", "tab-c", 2.0, false)
        .await
        .ok()
        .unwrap();
}

#[wasm_bindgen_test]
async fn pairing_and_input_errors_carry_codes() {
    let bob = device("bob-errors").await;
    let error = rejected(bob.signal.sign_pairing(text("not a container"))).await;
    assert_eq!(code_of(&error).as_deref(), Some("pairing_refused"));
    assert!(get(&error, "status").as_f64().is_some());
    rejects_with(
        bob.signal
            .decrypt(ALICE.into(), 1, "skmsg".into(), vec![0; 4]),
        "invalid_input",
    )
    .await;
    let partial = Object::new();
    set(&partial, "registrationId", &JsValue::from_f64(1.0));
    rejects_with(
        bob.signal
            .establish_session(ALICE.into(), 1, partial.into()),
        "invalid_input",
    )
    .await;
}

#[wasm_bindgen_test]
async fn receive_buffers_until_delivered_and_recognises_redelivery_after() {
    let alice = device("alice-receive").await;
    let bob = device("bob-receive").await;
    establish(&alice.signal, BOB, bundle_for(&bob.signal, true).await).await;
    let (kind, ciphertext) =
        encrypt(&alice.signal, BOB, &padded_conversation("hello from js")).await;

    let first = receive(&bob.signal, &kind, &ciphertext).await;
    assert_eq!(string_of(&get(&first, "status")), "message");
    assert_eq!(get(&first, "redelivered"), JsValue::FALSE);
    assert_eq!(
        conversation_of(&bytes_of(&get(&first, "message"))),
        "hello from js"
    );

    let again = receive(&bob.signal, &kind, &ciphertext).await;
    assert_eq!(get(&again, "redelivered"), JsValue::TRUE);
    assert_eq!(get(&again, "receiptKey"), get(&first, "receiptKey"));

    run(bob
        .signal
        .mark_delivered(string_of(&get(&first, "receiptKey"))))
    .await;
    let after = receive(&bob.signal, &kind, &ciphertext).await;
    assert_eq!(string_of(&get(&after, "status")), "already_delivered");
    assert!(!bob.h.state.contains_bytes(b"hello from js"));
}

#[wasm_bindgen_test]
async fn sends_are_built_in_the_page_and_only_ask_the_resolver_for_devices_and_keys() {
    let alice = device("alice-send-js").await;
    let bob = device("bob-send-js").await;
    run(alice
        .signal
        .set_own_jids("111:1@s.whatsapp.net".into(), Some("900111:1@lid".into())))
    .await;
    let bundle = bundle_for(&bob.signal, true).await;
    let resolver = Rc::new(ResolverState {
        devices: vec![BOB_DEVICE.into()],
        bundles: vec![(BOB_DEVICE.into(), bundle)],
        group: Some((GROUP.into(), vec![ALICE.into(), BOB.into()])),
        ..ResolverState::default()
    });

    let first = run(alice.signal.send_direct(
        plain_resolver(&resolver).unchecked_into(),
        BOB.into(),
        conversation("hi"),
        "J1".into(),
    ))
    .await;
    assert!(!bytes_of(&get(&first, "stanza")).is_empty());
    assert_eq!(Array::from(&get(&first, "unreachedDevices")).length(), 0);
    assert_eq!(
        resolver.device_calls.borrow()[0],
        vec![ALICE.to_owned(), BOB.to_owned()]
    );
    assert_eq!(
        *resolver.prekey_calls.borrow(),
        vec![vec![BOB_DEVICE.to_owned()]]
    );

    run(alice.signal.send_direct(
        plain_resolver(&resolver).unchecked_into(),
        BOB.into(),
        conversation("again"),
        "J2".into(),
    ))
    .await;
    assert_eq!(
        resolver.prekey_calls.borrow().len(),
        1,
        "an established session needs no new bundle"
    );
    assert!(!alice.h.state.contains_bytes(b"again"));

    let group = run(alice.signal.send_group(
        plain_resolver(&resolver).unchecked_into(),
        GROUP.into(),
        conversation("g"),
        "G1".into(),
    ))
    .await;
    assert!(!bytes_of(&get(&group, "stanza")).is_empty());
    assert_eq!(
        strings_of(&get(&group, "distributionTargets")),
        vec![BOB_DEVICE]
    );

    let unpaired = device("unpaired-js").await;
    rejects_with(
        unpaired.signal.send_direct(
            plain_resolver(&resolver).unchecked_into(),
            BOB.into(),
            conversation("x"),
            "J3".into(),
        ),
        "invalid_input",
    )
    .await;
}

#[wasm_bindgen_test]
async fn an_identity_change_forgets_every_session_with_that_user() {
    let alice = device("alice-forget").await;
    let bob = device("bob-forget").await;
    establish(&alice.signal, BOB, bundle_for(&bob.signal, true).await).await;
    assert_eq!(
        run(alice.signal.has_session(BOB.into(), 1)).await,
        JsValue::TRUE
    );

    run(alice.signal.forget_user(BOB.into())).await;
    assert_eq!(
        run(alice.signal.has_session(BOB.into(), 1)).await,
        JsValue::FALSE
    );
    establish(&alice.signal, BOB, bundle_for(&bob.signal, true).await).await;
    assert_eq!(
        run(alice.signal.has_session(BOB.into(), 1)).await,
        JsValue::TRUE
    );
}

#[wasm_bindgen_test]
async fn sessions_follow_a_user_from_phone_number_to_lid() {
    let alice = device("alice-migrate").await;
    let bob = device("bob-migrate").await;
    let bob_lid = "999@lid";
    establish(&alice.signal, BOB, bundle_for(&bob.signal, true).await).await;

    let moved = run(alice.signal.migrate_pn_to_lid(BOB.into(), bob_lid.into())).await;
    assert_eq!(moved.as_f64(), Some(1.0));
    assert_eq!(
        run(alice.signal.has_session(BOB.into(), 1)).await,
        JsValue::FALSE
    );
    assert_eq!(
        run(alice.signal.has_session(bob_lid.into(), 1)).await,
        JsValue::TRUE
    );
    let moved = run(alice.signal.migrate_pn_to_lid(BOB.into(), bob_lid.into())).await;
    assert_eq!(moved.as_f64(), Some(0.0));
}

#[wasm_bindgen_test]
async fn durable_public_uploads_recover_failed_commits_and_lost_prepare_or_ack_responses() {
    let bob = device("bob-upload-durable").await;
    let state = bob.h.state.clone();
    state
        .fail_before
        .set(Some(|ops| ops.iter().any(|op| op.ns == "prekey")));
    rejects_with(bob.signal.prepare_prekey_upload(3.0), "store").await;
    assert_eq!(state.keys_in_namespace("prekey"), 0);
    state.fail_after.set(Some(|ops| {
        ops.iter().any(|op| op.key == "prekey-upload-v1")
    }));
    rejects_with(bob.signal.prepare_prekey_upload(3.0), "store").await;
    let batch = run(bob.signal.prepare_prekey_upload(3.0)).await;
    let mut keys: Vec<String> = Object::keys(&batch.clone().into())
        .iter()
        .map(|k| string_of(&k))
        .collect();
    keys.sort();
    assert_eq!(keys, ["batchId", "preKeys"]);
    let batch_id = string_of(&get(&batch, "batchId"));
    assert!(
        batch_id.len() == 32
            && batch_id
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    );
    for key in Array::from(&get(&batch, "preKeys")).iter() {
        let mut shape: Vec<String> = Object::keys(&key.clone().into())
            .iter()
            .map(|k| string_of(&k))
            .collect();
        shape.sort();
        assert_eq!(shape, ["id", "publicKey"]);
        assert_eq!(bytes_of(&get(&key, "publicKey")).len(), 32);
    }
    for _ in 0..12 {
        assert!(same(
            &run(bob.signal.prepare_prekey_upload(812.0)).await,
            &batch
        ));
    }
    let ids = ids_of(&get(&batch, "preKeys"));
    let fresh = ids_of(&run(bob.signal.generate_prekeys(1)).await);
    assert_eq!(fresh[0], ids.last().unwrap() + 1);
    assert_eq!(count(&bob.signal).await, 0.0);
    rejects_with(
        bob.signal
            .mark_prekeys_uploaded(id_array(&[fresh[0], ids[0]])),
        "invalid_input",
    )
    .await;
    assert_eq!(count(&bob.signal).await, 0.0);
    rejects_with(
        bob.signal
            .acknowledge_prekey_upload(batch_id.clone(), id_array(&ids[..1])),
        "invalid_input",
    )
    .await;
    rejects_with(
        bob.signal
            .acknowledge_prekey_upload("foreign".into(), id_array(&ids)),
        "invalid_input",
    )
    .await;
    state.fail_after.set(Some(|ops| {
        ops.iter().any(|op| op.key == "prekey-upload-ack-v1")
    }));
    rejects_with(
        bob.signal
            .acknowledge_prekey_upload(batch_id.clone(), id_array(&ids)),
        "store",
    )
    .await;
    run(bob.signal.release()).await;
    drop(bob.signal);
    let reopened = bob
        .h
        .open("bob-upload-durable", "tab-b", 1.0, false)
        .await
        .ok()
        .unwrap();
    run(reopened.acknowledge_prekey_upload(batch_id.clone(), id_array(&ids))).await;
    assert_eq!(count(&reopened).await, 3.0);
    let next = run(reopened.prepare_prekey_upload(1.0)).await;
    run(reopened.acknowledge_prekey_upload(batch_id.clone(), id_array(&ids))).await;
    assert!(same(&run(reopened.prepare_prekey_upload(1.0)).await, &next));
    assert_eq!(
        count(&reopened).await,
        3.0,
        "fresh retry key was not marked uploaded"
    );
    run(reopened.mark_prekeys_uploaded(id_array(&fresh[..1]))).await;
    assert_eq!(count(&reopened).await, 4.0);
    run(reopened.release()).await;
}

#[wasm_bindgen_test]
async fn uploaded_counts_and_recovered_batches_exclude_keys_consumed_by_actual_encrypted_receive() {
    let alice = device("alice-upload-consumption").await;
    let bob = device("bob-upload-consumption").await;
    let batch = run(bob.signal.prepare_prekey_upload(2.0)).await;
    let prekeys = Array::from(&get(&batch, "preKeys"));
    let batch_id = string_of(&get(&batch, "batchId"));
    let ids = ids_of(&prekeys.clone().into());
    let bundle = bundle_for(&bob.signal, false).await;
    set(
        bundle.unchecked_ref(),
        "prekeyId",
        &get(&prekeys.get(0), "id"),
    );
    set(
        bundle.unchecked_ref(),
        "prekey",
        &get(&prekeys.get(0), "publicKey"),
    );
    establish(&alice.signal, BOB, bundle).await;
    let (kind, ciphertext) = encrypt(
        &alice.signal,
        BOB,
        &padded_conversation("durable upload message"),
    )
    .await;
    let result = receive(&bob.signal, &kind, &ciphertext).await;
    assert_eq!(
        conversation_of(&bytes_of(&get(&result, "message"))),
        "durable upload message"
    );
    assert_eq!(get(&result, "senderKeyRejected"), JsValue::FALSE);
    assert_eq!(bytes_of(&get(&result, "appStateKeyShare")).len(), 0);
    let retry = receive(&bob.signal, &kind, &ciphertext).await;
    assert_eq!(get(&retry, "redelivered"), JsValue::TRUE);
    assert_eq!(get(&retry, "receiptKey"), get(&result, "receiptKey"));
    persist_then_mark(&bob.signal, &result).await;
    assert_eq!(
        string_of(&get(
            &receive(&bob.signal, &kind, &ciphertext).await,
            "status"
        )),
        "already_delivered"
    );
    let live = run(bob.signal.prepare_prekey_upload(1.0)).await;
    assert_eq!(string_of(&get(&live, "batchId")), batch_id);
    let remaining = Array::new();
    remaining.push(&prekeys.get(1));
    assert!(same(&get(&live, "preKeys"), &remaining));
    rejects_with(
        bob.signal.mark_prekeys_uploaded(id_array(&ids[..1])),
        "invalid_input",
    )
    .await;
    run(bob
        .signal
        .acknowledge_prekey_upload(batch_id.clone(), id_array(&ids)))
    .await;
    assert_eq!(count(&bob.signal).await, 1.0);
    let (kind, ciphertext) = encrypt(
        &alice.signal,
        BOB,
        &padded_conversation("not another prekey"),
    )
    .await;
    receive(&bob.signal, &kind, &ciphertext).await;
    assert_eq!(count(&bob.signal).await, 1.0);
    run(bob.signal.release()).await;
    drop(bob.signal);
    let reopened = bob
        .h
        .open("bob-upload-consumption", "tab-b", 1.0, false)
        .await
        .ok()
        .unwrap();
    run(reopened.acknowledge_prekey_upload(batch_id, id_array(&ids))).await;
    assert_eq!(count(&reopened).await, 1.0);
    run(reopened.release()).await;
}

#[wasm_bindgen_test]
async fn uploaded_counts_and_durable_upload_acknowledgments_reject_stale_holders() {
    let bob = device("bob-upload-fenced").await;
    let batch = run(bob.signal.prepare_prekey_upload(2.0)).await;
    let ids = ids_of(&get(&batch, "preKeys"));
    let next = bob
        .h
        .open("bob-upload-fenced", "tab-b", TTL + 1.0, false)
        .await
        .ok()
        .unwrap();
    rejects_with(bob.signal.uploaded_prekey_count(), "fence_lost").await;
    rejects_with(bob.signal.prepare_prekey_upload(2.0), "fence_lost").await;
    rejects_with(
        bob.signal
            .acknowledge_prekey_upload(string_of(&get(&batch, "batchId")), id_array(&ids)),
        "fence_lost",
    )
    .await;
    assert!(same(&run(next.prepare_prekey_upload(1.0)).await, &batch));
    run(next.release()).await;
}

fn direct_request(chat: &str, message_id: &str) -> Object {
    let request = Object::new();
    set(&request, "chat", &chat.into());
    set(&request, "messageId", &message_id.into());
    set(&request, "requester", &BOB_DEVICE.into());
    set(&request, "encryptionJid", &BOB_DEVICE.into());
    set(&request, "retryCount", &JsValue::from_f64(1.0));
    let route = Object::new();
    set(&route, "kind", &"direct".into());
    set(&request, "route", &route);
    request
}

#[wasm_bindgen_test]
async fn actual_wasm_retry_stanzas_decrypt_at_the_peer_including_fresh_keys_after_a_reset() {
    let alice = device("alice-resend").await;
    let bob = device("bob-resend").await;
    run(alice
        .signal
        .set_own_jids("111:1@s.whatsapp.net".into(), None))
    .await;
    pair(&alice.signal).await;
    let resolver = resolver_for(bundle_for(&bob.signal, true).await);
    run(alice.signal.send_direct(
        plain_resolver(&resolver).unchecked_into(),
        BOB.into(),
        conversation("lost once"),
        "RETRY1".into(),
    ))
    .await;
    let request = direct_request(BOB, "RETRY1");
    let first = bytes_of(
        &run(alice.signal.resend(
            plain_resolver(&resolver).unchecked_into(),
            request.clone().into(),
        ))
        .await,
    );
    let tree = stanza(&first);
    assert_eq!(attr(&tree, "id").as_deref(), Some("RETRY1"));
    assert_eq!(
        attr(find(&tree, "enc").unwrap(), "count").as_deref(),
        Some("1")
    );
    let delivered = receive_stanza(&bob.signal, &first, ALICE).await;
    assert_eq!(
        conversation_of(&bytes_of(&get(&delivered, "message"))),
        "lost once"
    );
    persist_then_mark(&bob.signal, &delivered).await;

    let reset = device("bob-resend-after-reset").await;
    let fresh_bundle = bundle_for(&reset.signal, true).await;
    let second_request = Object::assign(&Object::new(), &request);
    set(&second_request, "retryCount", &JsValue::from_f64(2.0));
    set(&second_request, "bundle", &fresh_bundle);
    let second = bytes_of(
        &run(alice.signal.resend(
            plain_resolver(&resolver).unchecked_into(),
            second_request.into(),
        ))
        .await,
    );
    assert_eq!(
        attr(find(&stanza(&second), "enc").unwrap(), "count").as_deref(),
        Some("2")
    );
    let recovered = receive_stanza(&reset.signal, &second, ALICE).await;
    assert_eq!(
        conversation_of(&bytes_of(&get(&recovered, "message"))),
        "lost once"
    );
    persist_then_mark(&reset.signal, &recovered).await;

    let missing = Object::assign(&Object::new(), &request);
    set(&missing, "messageId", &"missing".into());
    rejects_with(
        alice
            .signal
            .resend(plain_resolver(&resolver).unchecked_into(), missing.into()),
        "invalid_input",
    )
    .await;
    assert!(!alice.h.state.contains_bytes(b"lost once"));
    let pruned = run(alice.signal.prune_sent_messages(9_007_199_254_740_991.0)).await;
    assert_eq!(pruned.as_f64(), Some(1.0));
    rejects_with(
        alice
            .signal
            .resend(plain_resolver(&resolver).unchecked_into(), request.into()),
        "invalid_input",
    )
    .await;
}

#[wasm_bindgen_test]
async fn a_group_retry_clears_only_the_requester_distribution_acknowledgment() {
    let alice = device("alice-group-resend").await;
    let bob = device("bob-group-resend").await;
    run(alice
        .signal
        .set_own_jids("111:1@s.whatsapp.net".into(), Some("900111:1@lid".into())))
    .await;
    pair(&alice.signal).await;
    let primary = device("bob-primary-group-resend").await;
    let primary_bundle = bundle_for(&primary.signal, true).await;
    set(
        primary_bundle.unchecked_ref(),
        "deviceId",
        &JsValue::from_f64(0.0),
    );
    let resolver = Rc::new(ResolverState {
        devices: vec![BOB.into(), BOB_DEVICE.into()],
        bundles: vec![(BOB.into(), primary_bundle)],
        fallback: Some(bundle_for(&bob.signal, true).await),
        group: Some((GROUP.into(), vec![ALICE.into(), BOB.into()])),
        ..ResolverState::default()
    });
    let initial = run(alice.signal.send_group(
        plain_resolver(&resolver).unchecked_into(),
        GROUP.into(),
        conversation("group retry"),
        "GROUP1".into(),
    ))
    .await;
    let mut targets = strings_of(&get(&initial, "distributionTargets"));
    targets.sort();
    let mut expected = [BOB, BOB_DEVICE];
    expected.sort();
    assert_eq!(targets, expected);
    let epoch = get(&initial, "distributionEpoch");
    assert_eq!(epoch.js_typeof().as_string().as_deref(), Some("bigint"));
    let epoch = u64::try_from(epoch).unwrap();
    let mark = |epoch| {
        alice.signal.mark_sender_key_distributed(
            GROUP.into(),
            strings_of(&get(&initial, "distributionTargets")),
            epoch,
        )
    };
    assert_eq!(run(mark(epoch)).await, JsValue::TRUE);
    let warm = run(alice.signal.send_group(
        plain_resolver(&resolver).unchecked_into(),
        GROUP.into(),
        conversation("warm"),
        "GROUP2".into(),
    ))
    .await;
    assert!(strings_of(&get(&warm, "distributionTargets")).is_empty());

    let request = direct_request(GROUP, "GROUP1");
    let route = Object::new();
    set(&route, "kind", &"group".into());
    set(&route, "addressingMode", &"pn".into());
    set(&request, "route", &route);
    let retried = bytes_of(
        &run(alice
            .signal
            .resend(plain_resolver(&resolver).unchecked_into(), request.into()))
        .await,
    );
    assert_eq!(attr(&stanza(&retried), "to").as_deref(), Some(GROUP));
    let received = receive_stanza(&bob.signal, &retried, GROUP).await;
    assert_eq!(
        conversation_of(&bytes_of(&get(&received, "message"))),
        "group retry"
    );
    persist_then_mark(&bob.signal, &received).await;
    // A late acknowledgment of the send before the retry must not restore the
    // requester's cleared mark.
    assert_eq!(run(mark(epoch)).await, JsValue::FALSE);
    let again = run(alice.signal.send_group(
        plain_resolver(&resolver).unchecked_into(),
        GROUP.into(),
        conversation("redistribute"),
        "GROUP3".into(),
    ))
    .await;
    assert_eq!(
        strings_of(&get(&again, "distributionTargets")),
        [BOB_DEVICE]
    );
}

#[wasm_bindgen_test]
async fn receive_exposes_rejected_sender_keys_without_losing_ordinary_application_content() {
    let alice = device("alice-rejected-skdm").await;
    let bob = device("bob-rejected-skdm").await;
    establish(&alice.signal, BOB, bundle_for(&bob.signal, true).await).await;
    let bad_skdm = field(
        2,
        &[field(1, GROUP.as_bytes()), field(2, &[1, 2, 3])].concat(),
    );
    let plain = pad(&[conversation("keep the text"), bad_skdm.clone()].concat());
    let (kind, ciphertext) = encrypt(&alice.signal, BOB, &plain).await;
    let received = receive_with(&bob.signal, GROUP, &kind, &ciphertext, 2, false, None)
        .await
        .ok()
        .unwrap();
    assert_eq!(get(&received, "senderKeyRejected"), JsValue::TRUE);
    let message = bytes_of(&get(&received, "message"));
    assert_eq!(conversation_of(&message), "keep the text");
    assert!(!contains_run(&message, &bad_skdm));
    persist_then_mark(&bob.signal, &received).await;
}

#[wasm_bindgen_test]
async fn app_state_sharing_requires_separate_explicit_consent_and_own_device_evidence() {
    let share = field(
        1,
        &[field(1, &field(1, &[3; 32])), field(2, &field(1, &[4; 32]))].concat(),
    );
    let protocol = field(12, &[varint(2 * 8), vec![6], field(7, &share)].concat());
    for (consent, from_me) in [
        (None, true),
        (Some(false), true),
        (Some(true), true),
        (Some(true), false),
    ] {
        let alice = device(&format!("alice-share-{consent:?}-{from_me}")).await;
        let bob = device(&format!("bob-share-{consent:?}-{from_me}")).await;
        establish(&alice.signal, BOB, bundle_for(&bob.signal, true).await).await;
        let plain = pad(&[conversation("mixed own-device content"), protocol.clone()].concat());
        let (kind, ciphertext) = encrypt(&alice.signal, BOB, &plain).await;
        let received = receive_with(
            &bob.signal,
            ALICE,
            &kind,
            &ciphertext,
            2,
            from_me,
            consent.map(JsValue::from_bool),
        )
        .await
        .ok()
        .unwrap();
        let message = bytes_of(&get(&received, "message"));
        assert_eq!(conversation_of(&message), "mixed own-device content");
        assert!(
            !contains_run(&message, &share),
            "application bytes must omit key material"
        );
        let released = bytes_of(&get(&received, "appStateKeyShare"));
        assert_eq!(!released.is_empty(), consent == Some(true) && from_me);
        if consent == Some(true) && from_me {
            assert_eq!(
                released, share,
                "separate share bytes match the synthetic protocol fixture"
            );
        }
        let retry = receive_with(&bob.signal, ALICE, &kind, &ciphertext, 2, from_me, None)
            .await
            .ok()
            .unwrap();
        assert_eq!(get(&retry, "redelivered"), JsValue::TRUE);
        assert!(bytes_of(&get(&retry, "appStateKeyShare")).is_empty());
        persist_then_mark(&bob.signal, &received).await;
        let after = receive_with(
            &bob.signal,
            ALICE,
            &kind,
            &ciphertext,
            2,
            from_me,
            Some(JsValue::TRUE),
        )
        .await
        .ok()
        .unwrap();
        assert_eq!(string_of(&get(&after, "status")), "already_delivered");
    }
}

#[wasm_bindgen_test]
async fn new_wasm_input_validation_refuses_malformed_counts_routes_and_retry_metadata() {
    let bob = device("bob-new-inputs").await;
    for count in [
        0.0,
        -1.0,
        1.5,
        813.0,
        f64::NAN,
        f64::INFINITY,
        4_294_967_297.0,
    ] {
        rejects_with(bob.signal.prepare_prekey_upload(count), "invalid_input").await;
    }
    for value in [-1.0, 0.5, f64::NAN, f64::INFINITY, 9_007_199_254_740_992.0] {
        rejects_with(bob.signal.prune_sent_messages(value), "invalid_input").await;
    }
    for consent in [
        JsValue::from_str("false"),
        JsValue::from_str("true"),
        JsValue::from_f64(1.0),
        Object::new().into(),
    ] {
        let error = receive_with(&bob.signal, ALICE, "pkmsg", &[], 2, true, Some(consent))
            .await
            .err()
            .unwrap();
        assert_eq!(code_of(&error).as_deref(), Some("invalid_input"));
    }
    let resolver = resolver_for(bundle_for(&bob.signal, true).await);
    let patches: [fn(&Object); 6] = [
        |r| set(r, "retryCount", &JsValue::from_f64(256.0)),
        |r| set(r, "retryCount", &JsValue::from_f64(0.5)),
        |r| {
            let route = Object::new();
            set(&route, "kind", &"unknown".into());
            set(r, "route", &route);
        },
        |r| {
            let route = Object::new();
            set(&route, "kind", &"group".into());
            set(&route, "addressingMode", &"unknown".into());
            set(r, "route", &route);
        },
        |r| {
            let bundle = Object::new();
            set(&bundle, "registrationId", &JsValue::from_f64(1.0));
            set(r, "bundle", &bundle);
        },
        |r| set(r, "requester", &"invalid jid".into()),
    ];
    for patch in patches {
        let request = direct_request(BOB, "id");
        patch(&request);
        rejects_with(
            bob.signal
                .resend(plain_resolver(&resolver).unchecked_into(), request.into()),
            "invalid_input",
        )
        .await;
    }
}

/// The host-side upload sequence described on `preparePrekeyUpload`: take the
/// scope, prepare the durable batch, hand only public material to the
/// provider callback, acknowledge on success and release the scope either way.
async fn upload_public_prekeys(
    h: &Harness,
    scope: &str,
    holder: &str,
    now_ms: f64,
    upload: impl FnOnce(JsValue) -> Result<(), String>,
) -> Result<String, String> {
    let signal = h
        .open(scope, holder, now_ms, true)
        .await
        .map_err(|e| message_of(&e))?;
    let batch = run(signal.prepare_prekey_upload(2.0)).await;
    let identity = run(signal.public_identity()).await;
    // ADV pairing material is absent from the upload callback. A failed or
    // lost response leaves the durable batch available when the caller
    // reopens it.
    let payload = Object::new();
    set(
        &payload,
        "registrationId",
        &get(&identity, "registrationId"),
    );
    set(&payload, "identityKey", &get(&identity, "identityKey"));
    set(&payload, "signedPrekey", &get(&identity, "signedPrekey"));
    set(&payload, "batchId", &get(&batch, "batchId"));
    set(&payload, "preKeys", &get(&batch, "preKeys"));
    let sent = upload(payload.into());
    let outcome = match sent {
        Ok(()) => {
            let ids = ids_of(&get(&batch, "preKeys"));
            run(signal
                .acknowledge_prekey_upload(string_of(&get(&batch, "batchId")), id_array(&ids)))
            .await;
            Ok(string_of(&get(&batch, "batchId")))
        }
        Err(error) => Err(error),
    };
    run(signal.release()).await;
    outcome
}

#[wasm_bindgen_test]
async fn upload_sequence_keeps_pairing_secrets_out_of_its_callback() {
    let h = Harness::new();
    let first = Rc::new(RefCell::new(JsValue::UNDEFINED));
    let seen = first.clone();
    let lost = upload_public_prekeys(&h, "example-upload", "example-a", 0.0, move |upload| {
        *seen.borrow_mut() = upload;
        Err("synthetic provider response lost".into())
    })
    .await;
    assert!(lost.is_err());
    let first = first.borrow().clone();
    let mut keys: Vec<String> = Object::keys(&first.clone().into())
        .iter()
        .map(|k| string_of(&k))
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "batchId",
            "identityKey",
            "preKeys",
            "registrationId",
            "signedPrekey"
        ]
    );
    // The identity the callback was built from does carry the secret, so the
    // absence above is a real filter, and nothing in the payload holds it.
    let probe = h
        .open("example-upload", "example-b", 1.0, false)
        .await
        .ok()
        .unwrap();
    let secret = bytes_of(&get(&run(probe.public_identity()).await, "advSecret"));
    assert!(!secret.is_empty());
    run(probe.release()).await;
    drop(probe);
    assert!(
        !js_sys::JSON::stringify(&first)
            .unwrap()
            .as_string()
            .unwrap()
            .contains("advSecret")
    );

    let expected = first.clone();
    let id = upload_public_prekeys(&h, "example-upload", "example-b", 2.0, move |upload| {
        assert!(
            same(&upload, &expected),
            "a reopened scope recovers the same batch"
        );
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(id, string_of(&get(&first, "batchId")));
    let reopened = h
        .open("example-upload", "example-c", 3.0, false)
        .await
        .ok()
        .unwrap();
    assert_eq!(count(&reopened).await, 2.0);
    run(reopened.release()).await;
}

#[wasm_bindgen_test]
async fn a_failed_open_keeps_the_lease_that_a_concurrent_open_by_the_same_holder_shares() {
    let h = Harness::new();
    h.state.read_delay_ms.set(5);
    let alice = device("alice-live-lease").await;
    let seed = h
        .open("bob-live-lease", "tab-a", 0.0, true)
        .await
        .ok()
        .unwrap();
    run(seed.release()).await;
    drop(seed);
    // Same holder, same lease: the first open reads with the wrong key and
    // fails while the second is still starting. The failure must not release
    // the lease the second one is about to use.
    let wrong: JsValue = TestSealer::new().into();
    let (failed, started) = join2(
        open_with(&h.backend, &wrong, "bob-live-lease", "tab-a", 1.0, false),
        h.open("bob-live-lease", "tab-a", 1.0, false),
    )
    .await;
    assert!(failed.is_err());
    let bob = started.expect("the concurrent open succeeds");
    let refused = expect_err(h.open("bob-live-lease", "tab-b", 2.0, false).await);
    assert_eq!(code_of(&refused).as_deref(), Some("lease_held"));
    establish(&alice.signal, BOB, bundle_for(&bob, true).await).await;
    let (kind, ciphertext) = encrypt(&alice.signal, BOB, b"still live").await;
    assert_eq!(
        decrypt(&bob, ALICE, &kind, &ciphertext).await,
        b"still live"
    );
    let (_, reply) = encrypt(&bob, ALICE, b"reply").await;
    assert!(!reply.is_empty());
}

#[wasm_bindgen_test]
async fn a_second_open_with_the_wrong_sealer_does_not_disturb_the_live_device() {
    let bob = device("bob-wrong-key").await;
    let wrong: JsValue = TestSealer::new().into();
    let _ = open_with(&bob.h.backend, &wrong, "bob-wrong-key", "tab-a", 1.0, false).await;
    let refused = expect_err(bob.h.open("bob-wrong-key", "tab-b", 2.0, false).await);
    assert_eq!(code_of(&refused).as_deref(), Some("lease_held"));
    let identity = run(bob.signal.public_identity()).await;
    assert!(!bytes_of(&get(&identity, "identityKey")).is_empty());
}

#[wasm_bindgen_test]
async fn prune_decrypt_buffer_refuses_cutoffs_that_are_not_safe_integers() {
    let bob = device("bob-prune-input").await;
    for bad in [f64::INFINITY, f64::NAN, -1.0, 1.5] {
        rejects_with(bob.signal.prune_decrypt_buffer(bad), "invalid_input").await;
    }
}

#[wasm_bindgen_test]
async fn buffered_records_carry_the_wall_clock_so_a_yesterday_cutoff_keeps_them() {
    let alice = device("alice-clock").await;
    let bob = device("bob-clock").await;
    establish(&alice.signal, BOB, bundle_for(&bob.signal, true).await).await;
    let (kind, ciphertext) = encrypt(&alice.signal, BOB, &padded_conversation("fresh")).await;
    let first = receive(&bob.signal, &kind, &ciphertext).await;
    assert_eq!(string_of(&get(&first, "status")), "message");
    let removed = run(bob
        .signal
        .prune_decrypt_buffer(js_sys::Date::now() - 86_400_000.0))
    .await;
    assert_eq!(removed.as_f64(), Some(0.0));
    let again = receive(&bob.signal, &kind, &ciphertext).await;
    assert_eq!(get(&again, "redelivered"), JsValue::TRUE);
    assert_eq!(conversation_of(&bytes_of(&get(&again, "message"))), "fresh");
}

#[wasm_bindgen_test]
async fn a_send_to_a_phone_number_reuses_the_session_migrated_to_its_lid() {
    let alice = device("alice-lid-send").await;
    let bob = device("bob-lid-send").await;
    run(alice
        .signal
        .set_own_jids("111:1@s.whatsapp.net".into(), None))
    .await;
    establish(&alice.signal, BOB, bundle_for(&bob.signal, true).await).await;
    let moved = run(alice.signal.migrate_pn_to_lid(BOB.into(), "999@lid".into())).await;
    assert_eq!(moved.as_f64(), Some(1.0));
    let resolver = Rc::new(ResolverState {
        devices: vec![BOB_DEVICE.into()],
        lid: Some(("222".into(), "999".into())),
        ..ResolverState::default()
    });
    let sent = run(alice.signal.send_direct(
        lid_resolver(&resolver).unchecked_into(),
        BOB.into(),
        conversation("over lid"),
        "LID1".into(),
    ))
    .await;
    assert_eq!(Array::from(&get(&sent, "unreachedDevices")).length(), 0);
    assert!(
        resolver
            .lid_lookups
            .borrow()
            .iter()
            .any(|user| user == "222")
    );
    let delivered = receive_stanza(&bob.signal, &bytes_of(&get(&sent, "stanza")), ALICE).await;
    assert_eq!(
        conversation_of(&bytes_of(&get(&delivered, "message"))),
        "over lid"
    );
}

#[wasm_bindgen_test]
async fn invalid_lease_times_are_refused_before_any_backend_call_and_strand_nothing() {
    let h = Harness::new();
    let open = |holder: &'static str, now: f64, ttl: f64| {
        let (backend, sealer) = (h.backend.clone(), h.sealer.clone());
        async move {
            SignalDevice::open(
                backend.unchecked_into(),
                sealer.unchecked_into(),
                "lease-times".into(),
                holder.into(),
                now,
                ttl,
                true,
            )
            .await
        }
    };
    for (now, ttl) in [
        (0.0, f64::INFINITY),
        (0.0, f64::NAN),
        (0.0, -1.0),
        (0.5, 1.0),
        (f64::INFINITY, 1.0),
        (9_007_199_254_740_991.0, 1.0),
    ] {
        let error = expect_err(open("tab-a", now, ttl).await);
        assert_eq!(
            code_of(&error).as_deref(),
            Some("invalid_input"),
            "{now} {ttl}"
        );
    }
    assert!(
        h.state.leases.borrow().is_empty(),
        "no backend call was made"
    );
    // Another holder acquires at once: nothing was left held.
    let live = open("tab-b", 0.0, TTL).await.ok().unwrap();
    for (now, ttl) in [
        (1.0, f64::INFINITY),
        (1.0, f64::NAN),
        (9_007_199_254_740_991.0, 1.0),
    ] {
        rejects_with(live.renew_lease(now, ttl), "invalid_input").await;
    }
    assert_eq!(h.state.leases.borrow()["lease-times"].expires_at_ms, TTL);
    assert_eq!(run(live.renew_lease(1.0, TTL)).await, JsValue::TRUE);
    run(live.release()).await;
    open("tab-c", 2.0, TTL).await.ok().unwrap();
}
