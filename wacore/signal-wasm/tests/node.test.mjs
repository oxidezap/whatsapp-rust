// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

// End-to-end check of the wasm32 bindings from JavaScript.
//
// Build first, from the repository root:
//   RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo build -p wacore-signal-wasm \
//     --lib --release --target wasm32-unknown-unknown
//   wasm-bindgen --target nodejs --out-dir target/signal-wasm-node \
//     target/wasm32-unknown-unknown/release/wacore_signal_wasm.wasm
// Then: node --test wacore/signal-wasm/tests/node.test.mjs
//
// The backend below is an in-memory reference for the RecordBackend contract;
// the sealer is WebCrypto AES-GCM with a non-extractable key, as in a browser.

import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { mkdir, open } from "node:fs/promises";
import { join } from "node:path";
import { uploadPublicPrekeys } from "../examples/upload.mjs";

const require = createRequire(import.meta.url);
const pkg =
  process.env.SIGNAL_WASM_PKG ??
  fileURLToPath(
    new URL(
      "../../../target/signal-wasm-node/wacore_signal_wasm.js",
      import.meta.url,
    ),
  );
const { SignalDevice } = require(pkg);

function fixture(input) {
  assert.equal(
    typeof process.env.SIGNAL_WASM_FIXTURE,
    "string",
    "build the native synthetic fixture helper first",
  );
  const result = spawnSync(process.env.SIGNAL_WASM_FIXTURE, [], {
    input: JSON.stringify(input),
    maxBuffer: 2 * 1024 * 1024,
  });
  assert.equal(result.status, 0, "synthetic fixture helper failed");
  return JSON.parse(result.stdout);
}
async function pair(signal) {
  const identity = await signal.publicIdentity();
  const { container } = fixture({
    op: "pairing",
    identityKey: [...identity.identityKey],
    advSecret: [...identity.advSecret],
  });
  await signal.signPairing(Uint8Array.from(container));
}
const stanza = (bytes) => fixture({ op: "stanza", stanza: [...bytes] });
function find(node, tag) {
  return node.tag === tag
    ? node
    : node.children.map((n) => find(n, tag)).find(Boolean);
}
async function receiveStanza(signal, encoded, chat = ALICE) {
  const enc = find(stanza(encoded), "enc");
  assert.ok(enc, "ciphertext stanza has enc");
  return signal.receive(
    chat,
    ALICE,
    1,
    enc.attrs.type,
    Uint8Array.from(enc.data),
    Number(enc.attrs.v),
    false,
  );
}
async function persistThenMark(signal, result) {
  const dir = join(
    process.env.SIGNAL_WASM_TEST_OUTPUT ?? "target/signal-wasm-node-proof",
    crypto.randomUUID(),
  );
  await mkdir(dir, { recursive: true });
  const file = await open(join(dir, "message.pb"), "wx");
  try {
    await file.writeFile(result.message);
    await file.sync();
  } finally {
    await file.close();
  }
  const directory = await open(dir, "r");
  try {
    await directory.sync();
  } finally {
    await directory.close();
  }
  await signal.markDelivered(result.receiptKey);
}
function resolverFor(bundle, group = "120363000000001@g.us") {
  return {
    async resolveDevices() {
      return ["222:1@s.whatsapp.net"];
    },
    async fetchPrekeys(jids) {
      return Object.fromEntries(jids.map((jid) => [jid, bundle]));
    },
    async resolveGroup(jid) {
      assert.equal(jid, group);
      return { participants: [ALICE, BOB], addressingMode: "pn" };
    },
  };
}

const TTL = 60_000;

class MemoryBackend {
  records = new Map();
  leases = new Map();

  #slot(scope, ns, key) {
    return `${scope}\u0000${ns}\u0000${key}`;
  }

  async get(scope, ns, key) {
    return this.records.get(this.#slot(scope, ns, key)) ?? null;
  }

  async scanKeys(scope, ns, prefix, limit) {
    const head = `${scope}\u0000${ns}\u0000`;
    const keys = [...this.records.keys()]
      .filter((slot) => slot.startsWith(head + prefix))
      .map((slot) => slot.slice(head.length))
      .sort();
    return limit === undefined ? keys : keys.slice(0, limit);
  }

  async write(fence, ops) {
    if (this.leases.get(fence.scope)?.generation !== fence.generation) {
      throw Object.assign(new Error("fence lost"), { code: "fence_lost" });
    }
    for (const op of ops) {
      if (
        op.op === "insert" &&
        this.records.has(this.#slot(fence.scope, op.ns, op.key))
      ) {
        throw Object.assign(new Error("record exists"), {
          code: "record_exists",
        });
      }
    }
    for (const op of ops) {
      const slot = this.#slot(fence.scope, op.ns, op.key);
      if (op.op === "put" || op.op === "insert")
        this.records.set(slot, op.value.slice());
      else if (op.op === "update") {
        if (this.records.has(slot)) this.records.set(slot, op.value.slice());
      } else this.records.delete(slot);
    }
  }

  async acquire(scope, holder, nowMs, ttlMs) {
    const row = this.leases.get(scope);
    let generation;
    if (!row) generation = 1;
    else if (row.holder === holder) generation = row.generation;
    else if (row.expiresAtMs > nowMs) return null;
    else generation = row.generation + 1;
    const lease = { scope, generation, holder, expiresAtMs: nowMs + ttlMs };
    this.leases.set(scope, lease);
    return { ...lease };
  }

  async renew(lease, nowMs, ttlMs) {
    const row = this.leases.get(lease.scope);
    if (
      !row ||
      row.generation !== lease.generation ||
      row.holder !== lease.holder
    )
      return null;
    row.expiresAtMs = nowMs + ttlMs;
    return { ...row };
  }

  async release(lease) {
    const row = this.leases.get(lease.scope);
    if (
      row &&
      row.generation === lease.generation &&
      row.holder === lease.holder
    ) {
      row.expiresAtMs = 0;
      row.holder = "";
    }
  }

  containsBytes(needle) {
    const hex = Buffer.from(needle).toString("hex");
    return [...this.records.values()].some((v) =>
      Buffer.from(v).toString("hex").includes(hex),
    );
  }
}

class FaultBackend extends MemoryBackend {
  failBefore;
  failAfter;
  async write(fence, ops) {
    if (this.failBefore?.(ops)) {
      this.failBefore = undefined;
      throw new Error("synthetic commit refused");
    }
    await super.write(fence, ops);
    if (this.failAfter?.(ops)) {
      this.failAfter = undefined;
      throw new Error("synthetic response lost");
    }
  }
}

async function webCryptoSealer() {
  const key = await crypto.subtle.generateKey(
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"],
  );
  return {
    async seal(aad, plaintext) {
      const iv = crypto.getRandomValues(new Uint8Array(12));
      const sealed = new Uint8Array(
        await crypto.subtle.encrypt(
          { name: "AES-GCM", iv, additionalData: aad },
          key,
          plaintext,
        ),
      );
      const out = new Uint8Array(12 + sealed.length);
      out.set(iv);
      out.set(sealed, 12);
      return out;
    },
    async open(aad, sealed) {
      return new Uint8Array(
        await crypto.subtle.decrypt(
          { name: "AES-GCM", iv: sealed.subarray(0, 12), additionalData: aad },
          key,
          sealed.subarray(12),
        ),
      );
    },
  };
}

async function device(
  scope,
  { backend, sealer, holder = "tab-a", now = 0 } = {},
) {
  backend ??= new MemoryBackend();
  sealer ??= await webCryptoSealer();
  const signal = await SignalDevice.open(
    backend,
    sealer,
    scope,
    holder,
    now,
    TTL,
    true,
  );
  return { signal, backend, sealer };
}

async function bundleFor(signal, withPrekey = true) {
  const identity = await signal.publicIdentity();
  const bundle = {
    registrationId: identity.registrationId,
    deviceId: 1,
    identityKey: identity.identityKey,
    signedPrekeyId: identity.signedPrekey.id,
    signedPrekey: identity.signedPrekey.publicKey,
    signedPrekeySignature: identity.signedPrekey.signature,
  };
  if (withPrekey) {
    const [prekey] = await signal.generatePrekeys(1);
    bundle.prekeyId = prekey.id;
    bundle.prekey = prekey.publicKey;
  }
  return bundle;
}

const ALICE = "111@s.whatsapp.net";
const BOB = "222@s.whatsapp.net";
const text = (s) => new TextEncoder().encode(s);
const read = (b) => new TextDecoder().decode(b);

test("a session runs both ways and nothing readable reaches storage", async () => {
  const alice = await device("alice");
  const bob = await device("bob");
  await alice.signal.establishSession(BOB, 1, await bundleFor(bob.signal));

  const first = await alice.signal.encrypt(BOB, 1, text("hello bob"));
  assert.equal(first.kind, "pkmsg");
  const opened = await bob.signal.decrypt(
    ALICE,
    1,
    first.kind,
    first.ciphertext,
  );
  assert.equal(read(opened.plaintext), "hello bob");

  const reply = await bob.signal.encrypt(ALICE, 1, text("hi alice"));
  assert.equal(reply.kind, "msg");
  assert.equal(
    read(
      (await alice.signal.decrypt(BOB, 1, reply.kind, reply.ciphertext))
        .plaintext,
    ),
    "hi alice",
  );

  for (let round = 0; round < 5; round++) {
    const out = await alice.signal.encrypt(BOB, 1, text(`a${round}`));
    assert.equal(
      read(
        (await bob.signal.decrypt(ALICE, 1, out.kind, out.ciphertext))
          .plaintext,
      ),
      `a${round}`,
    );
  }

  assert.equal(bob.backend.containsBytes(text("hello bob")), false);
  const identity = await bob.signal.publicIdentity();
  assert.equal(bob.backend.containsBytes(identity.advSecret), false);
});

test("a second tab is refused while the lease is live, then fences the first out", async () => {
  const alice = await device("alice-failover");
  const backend = new MemoryBackend();
  const sealer = await webCryptoSealer();
  const bobA = await device("bob-failover", {
    backend,
    sealer,
    holder: "tab-a",
    now: 0,
  });
  await alice.signal.establishSession(BOB, 1, await bundleFor(bobA.signal));
  const first = await alice.signal.encrypt(BOB, 1, text("before"));
  await bobA.signal.decrypt(ALICE, 1, first.kind, first.ciphertext);

  await assert.rejects(
    SignalDevice.open(backend, sealer, "bob-failover", "tab-b", 1, TTL, false),
    (error) => error.code === "lease_held",
  );

  const bobB = await SignalDevice.open(
    backend,
    sealer,
    "bob-failover",
    "tab-b",
    TTL + 1,
    TTL,
    false,
  );
  const next = await alice.signal.encrypt(BOB, 1, text("after"));
  await assert.rejects(
    bobA.signal.decrypt(ALICE, 1, next.kind, next.ciphertext),
    (error) => error.code === "fence_lost",
  );
  assert.equal(await bobA.signal.renewLease(TTL + 2, TTL), false);
  assert.equal(
    read((await bobB.decrypt(ALICE, 1, next.kind, next.ciphertext)).plaintext),
    "after",
  );
});

test("opens of one scope by the same holder share keys and locks", async () => {
  const backend = new MemoryBackend();
  const sealer = await webCryptoSealer();
  const first = await device("same-holder", { backend, sealer, now: 0 });
  // Renewal by the same holder keeps the fence, so only a shared authority
  // keeps the two opens from writing over each other.
  const second = await SignalDevice.open(
    backend,
    sealer,
    "same-holder",
    "tab-a",
    1,
    TTL,
    false,
  );
  const [a, b] = await Promise.all([
    first.signal.rotateSignedPrekey(),
    second.rotateSignedPrekey(),
  ]);
  assert.notEqual(a.id, b.id, "rotations under one fence must not reuse an id");
  const latest = (await first.signal.publicIdentity()).signedPrekey;
  assert.deepEqual((await second.publicIdentity()).signedPrekey, latest);
});

test("group messages decrypt with a distributed sender key", async () => {
  const alice = await device("alice-group");
  const bob = await device("bob-group");
  const group = "120363000000001@g.us";
  await alice.signal.establishSession(BOB, 1, await bundleFor(bob.signal));

  const skdm = await alice.signal.senderKeyDistribution(group, ALICE, 1);
  const carried = await alice.signal.encrypt(BOB, 1, skdm);
  const received = await bob.signal.decrypt(
    ALICE,
    1,
    carried.kind,
    carried.ciphertext,
  );
  await bob.signal.processSenderKeyDistribution(
    group,
    ALICE,
    1,
    received.plaintext,
  );

  const skmsg = await alice.signal.groupEncrypt(
    group,
    ALICE,
    1,
    text("hello group"),
  );
  assert.equal(
    read(await bob.signal.groupDecrypt(group, ALICE, 1, skmsg)),
    "hello group",
  );
});

test("a sealer with the wrong key cannot open the device, and the lease is released", async () => {
  const backend = new MemoryBackend();
  const first = await device("bob-key", { backend });
  await first.signal.release();
  await assert.rejects(
    SignalDevice.open(
      backend,
      await webCryptoSealer(),
      "bob-key",
      "tab-b",
      1,
      TTL,
      false,
    ),
    (error) => error.code === "store",
  );
  // The failed open gave its lease back, so another tab can take the scope.
  await SignalDevice.open(
    backend,
    first.sealer,
    "bob-key",
    "tab-c",
    2,
    TTL,
    false,
  );
});

test("pairing and input errors carry codes", async () => {
  const bob = await device("bob-errors");
  await assert.rejects(
    bob.signal.signPairing(text("not a container")),
    (error) =>
      error.code === "pairing_refused" && typeof error.status === "number",
  );
  await assert.rejects(
    bob.signal.decrypt(ALICE, 1, "skmsg", new Uint8Array(4)),
    (error) => error.code === "invalid_input",
  );
  await assert.rejects(
    bob.signal.establishSession(ALICE, 1, { registrationId: 1 }),
    (error) => error.code === "invalid_input",
  );
});

// A `waE2E.Message` with only `conversation` (field 1), padded as WhatsApp
// pads (n bytes of value n, 1..=16).
function paddedConversation(body) {
  const utf8 = text(body);
  const pad = 1 + (utf8.length % 16);
  const out = new Uint8Array(2 + utf8.length + pad);
  out[0] = 0x0a;
  out[1] = utf8.length;
  out.set(utf8, 2);
  out.fill(pad, 2 + utf8.length);
  return out;
}

function conversationOf(message) {
  assert.equal(message[0], 0x0a);
  return read(message.subarray(2, 2 + message[1]));
}

test("receive buffers until delivered and recognises redelivery after", async () => {
  const alice = await device("alice-receive");
  const bob = await device("bob-receive");
  await alice.signal.establishSession(BOB, 1, await bundleFor(bob.signal));
  const sent = await alice.signal.encrypt(
    BOB,
    1,
    paddedConversation("hello from js"),
  );
  const args = [ALICE, ALICE, 1, sent.kind, sent.ciphertext, 2, false];

  const first = await bob.signal.receive(...args);
  assert.equal(first.status, "message");
  assert.equal(first.redelivered, false);
  assert.equal(conversationOf(first.message), "hello from js");

  const again = await bob.signal.receive(...args);
  assert.equal(again.redelivered, true);
  assert.equal(again.receiptKey, first.receiptKey);

  await bob.signal.markDelivered(first.receiptKey);
  const after = await bob.signal.receive(...args);
  assert.equal(after.status, "already_delivered");
  assert.equal(bob.backend.containsBytes(text("hello from js")), false);
});

function conversation(body) {
  const utf8 = text(body);
  return Uint8Array.from([0x0a, utf8.length, ...utf8]);
}

test("sends are built in the page and only ask the resolver for devices and keys", async () => {
  const alice = await device("alice-send-js");
  const bob = await device("bob-send-js");
  await alice.signal.setOwnJids("111:1@s.whatsapp.net", "900111:1@lid");
  const bobDevice = "222:1@s.whatsapp.net";
  const bobBundle = await bundleFor(bob.signal);
  const calls = { devices: [], prekeys: [] };
  const resolver = {
    async resolveDevices(jids) {
      calls.devices.push(jids);
      return [bobDevice];
    },
    async fetchPrekeys(jids) {
      calls.prekeys.push(jids);
      return Object.fromEntries(
        jids.filter((j) => j === bobDevice).map((j) => [j, bobBundle]),
      );
    },
    async resolveGroup() {
      return {
        participants: ["111@s.whatsapp.net", "222@s.whatsapp.net"],
        addressingMode: "pn",
      };
    },
  };

  const first = await alice.signal.sendDirect(
    resolver,
    "222@s.whatsapp.net",
    conversation("hi"),
    "J1",
  );
  assert.ok(first.stanza instanceof Uint8Array && first.stanza.length > 0);
  assert.deepEqual(first.unreachedDevices, []);
  assert.deepEqual(calls.devices[0].sort(), [
    "111@s.whatsapp.net",
    "222@s.whatsapp.net",
  ]);
  assert.deepEqual(calls.prekeys, [[bobDevice]]);

  await alice.signal.sendDirect(
    resolver,
    "222@s.whatsapp.net",
    conversation("again"),
    "J2",
  );
  assert.equal(
    calls.prekeys.length,
    1,
    "an established session needs no new bundle",
  );
  assert.equal(alice.backend.containsBytes(text("again")), false);

  const group = await alice.signal.sendGroup(
    resolver,
    "120363000000001@g.us",
    conversation("g"),
    "G1",
  );
  assert.ok(group.stanza.length > 0);
  assert.deepEqual(group.distributionTargets, [bobDevice]);

  const unpaired = await device("unpaired-js");
  await assert.rejects(
    unpaired.signal.sendDirect(
      resolver,
      "222@s.whatsapp.net",
      conversation("x"),
      "J3",
    ),
    (error) => error.code === "invalid_input",
  );
});

test("an identity change forgets every session with that user", async () => {
  const alice = await device("alice-forget");
  const bob = await device("bob-forget");
  await alice.signal.establishSession(BOB, 1, await bundleFor(bob.signal));
  assert.equal(await alice.signal.hasSession(BOB, 1), true);

  await alice.signal.forgetUser(BOB);
  assert.equal(await alice.signal.hasSession(BOB, 1), false);
  await alice.signal.establishSession(BOB, 1, await bundleFor(bob.signal));
  assert.equal(await alice.signal.hasSession(BOB, 1), true);
});

test("sessions follow a user from phone number to LID", async () => {
  const alice = await device("alice-migrate");
  const bob = await device("bob-migrate");
  const BOB_LID = "999@lid";
  await alice.signal.establishSession(BOB, 1, await bundleFor(bob.signal));

  assert.equal(await alice.signal.migratePnToLid(BOB, BOB_LID), 1);
  assert.equal(await alice.signal.hasSession(BOB, 1), false);
  assert.equal(await alice.signal.hasSession(BOB_LID, 1), true);
  assert.equal(await alice.signal.migratePnToLid(BOB, BOB_LID), 0);
});

test("durable public uploads recover failed commits and lost prepare or ACK responses", async () => {
  const backend = new FaultBackend();
  const bob = await device("bob-upload-durable", { backend });
  backend.failBefore = (ops) => ops.some((op) => op.ns === "prekey");
  await assert.rejects(
    bob.signal.preparePrekeyUpload(3),
    (error) => error.code === "store",
  );
  assert.equal(
    [...backend.records.keys()].filter((key) => key.includes("\0prekey\0"))
      .length,
    0,
  );
  backend.failAfter = (ops) => ops.some((op) => op.key === "prekey-upload-v1");
  await assert.rejects(
    bob.signal.preparePrekeyUpload(3),
    (error) => error.code === "store",
  );
  const batch = await bob.signal.preparePrekeyUpload(3);
  assert.deepEqual(Object.keys(batch).sort(), ["batchId", "preKeys"]);
  assert.match(batch.batchId, /^[0-9a-f]{32}$/);
  for (const key of batch.preKeys) {
    assert.deepEqual(Object.keys(key).sort(), ["id", "publicKey"]);
    assert.equal(key.publicKey.length, 32);
  }
  for (let i = 0; i < 12; i++)
    assert.deepEqual(await bob.signal.preparePrekeyUpload(812), batch);
  const fresh = await bob.signal.generatePrekeys(1);
  assert.equal(fresh[0].id, batch.preKeys.at(-1).id + 1);
  assert.equal(await bob.signal.uploadedPrekeyCount(), 0);
  const ids = Uint32Array.from(batch.preKeys.map((key) => key.id));
  await assert.rejects(
    bob.signal.markPrekeysUploaded(Uint32Array.of(fresh[0].id, ids[0])),
    (e) => e.code === "invalid_input",
  );
  assert.equal(await bob.signal.uploadedPrekeyCount(), 0);
  await assert.rejects(
    bob.signal.acknowledgePrekeyUpload(batch.batchId, ids.subarray(0, 1)),
    (e) => e.code === "invalid_input",
  );
  await assert.rejects(
    bob.signal.acknowledgePrekeyUpload("foreign", ids),
    (e) => e.code === "invalid_input",
  );
  backend.failAfter = (ops) =>
    ops.some((op) => op.key === "prekey-upload-ack-v1");
  await assert.rejects(
    bob.signal.acknowledgePrekeyUpload(batch.batchId, ids),
    (e) => e.code === "store",
  );
  await bob.signal.release();
  bob.signal.free();
  const reopened = await SignalDevice.open(
    backend,
    bob.sealer,
    "bob-upload-durable",
    "tab-b",
    1,
    TTL,
    false,
  );
  await reopened.acknowledgePrekeyUpload(batch.batchId, ids);
  assert.equal(await reopened.uploadedPrekeyCount(), 3);
  const next = await reopened.preparePrekeyUpload(1);
  await reopened.acknowledgePrekeyUpload(batch.batchId, ids);
  assert.deepEqual(await reopened.preparePrekeyUpload(1), next);
  assert.equal(
    await reopened.uploadedPrekeyCount(),
    3,
    "fresh retry key was not marked uploaded",
  );
  await reopened.markPrekeysUploaded(Uint32Array.of(fresh[0].id));
  assert.equal(await reopened.uploadedPrekeyCount(), 4);
  await reopened.release();
  reopened.free();
});

test("uploaded counts and recovered batches exclude keys consumed by actual encrypted receive", async () => {
  const alice = await device("alice-upload-consumption");
  const bob = await device("bob-upload-consumption");
  const batch = await bob.signal.preparePrekeyUpload(2);
  const bundle = await bundleFor(bob.signal, false);
  bundle.prekeyId = batch.preKeys[0].id;
  bundle.prekey = batch.preKeys[0].publicKey;
  await alice.signal.establishSession(BOB, 1, bundle);
  const cipher = await alice.signal.encrypt(
    BOB,
    1,
    paddedConversation("durable upload message"),
  );
  const result = await bob.signal.receive(
    ALICE,
    ALICE,
    1,
    cipher.kind,
    cipher.ciphertext,
    2,
    false,
  );
  assert.equal(conversationOf(result.message), "durable upload message");
  assert.equal(result.senderKeyRejected, false);
  assert.equal(result.appStateKeyShare.length, 0);
  const retry = await bob.signal.receive(
    ALICE,
    ALICE,
    1,
    cipher.kind,
    cipher.ciphertext,
    2,
    false,
  );
  assert.equal(retry.redelivered, true);
  assert.equal(retry.receiptKey, result.receiptKey);
  await persistThenMark(bob.signal, result);
  assert.equal(
    (
      await bob.signal.receive(
        ALICE,
        ALICE,
        1,
        cipher.kind,
        cipher.ciphertext,
        2,
        false,
      )
    ).status,
    "already_delivered",
  );
  const live = await bob.signal.preparePrekeyUpload(1);
  assert.equal(live.batchId, batch.batchId);
  assert.deepEqual(live.preKeys, batch.preKeys.slice(1));
  await assert.rejects(
    bob.signal.markPrekeysUploaded(Uint32Array.of(batch.preKeys[0].id)),
    (e) => e.code === "invalid_input",
  );
  await bob.signal.acknowledgePrekeyUpload(
    batch.batchId,
    Uint32Array.from(batch.preKeys.map((key) => key.id)),
  );
  assert.equal(await bob.signal.uploadedPrekeyCount(), 1);
  const other = await alice.signal.encrypt(
    BOB,
    1,
    paddedConversation("not another prekey"),
  );
  await bob.signal.receive(
    ALICE,
    ALICE,
    1,
    other.kind,
    other.ciphertext,
    2,
    false,
  );
  assert.equal(await bob.signal.uploadedPrekeyCount(), 1);
  await bob.signal.release();
  bob.signal.free();
  const reopened = await SignalDevice.open(
    bob.backend,
    bob.sealer,
    "bob-upload-consumption",
    "tab-b",
    1,
    TTL,
    false,
  );
  await reopened.acknowledgePrekeyUpload(
    batch.batchId,
    Uint32Array.from(batch.preKeys.map((key) => key.id)),
  );
  assert.equal(await reopened.uploadedPrekeyCount(), 1);
  await reopened.release();
  reopened.free();
});

test("uploaded counts and durable upload acknowledgments reject stale holders", async () => {
  const bob = await device("bob-upload-fenced");
  const batch = await bob.signal.preparePrekeyUpload(2);
  const next = await SignalDevice.open(
    bob.backend,
    bob.sealer,
    "bob-upload-fenced",
    "tab-b",
    TTL + 1,
    TTL,
    false,
  );
  for (const operation of [
    () => bob.signal.uploadedPrekeyCount(),
    () => bob.signal.preparePrekeyUpload(2),
    () =>
      bob.signal.acknowledgePrekeyUpload(
        batch.batchId,
        Uint32Array.from(batch.preKeys.map((key) => key.id)),
      ),
  ])
    await assert.rejects(operation(), (e) => e.code === "fence_lost");
  assert.deepEqual(await next.preparePrekeyUpload(1), batch);
  await next.release();
  next.free();
});

test("actual WASM retry stanzas decrypt at the peer, including fresh keys after a reset", async () => {
  const alice = await device("alice-resend");
  const bob = await device("bob-resend");
  await alice.signal.setOwnJids("111:1@s.whatsapp.net", undefined);
  await pair(alice.signal);
  const resolver = resolverFor(await bundleFor(bob.signal));
  await alice.signal.sendDirect(
    resolver,
    BOB,
    conversation("lost once"),
    "RETRY1",
  );
  const request = {
    chat: BOB,
    messageId: "RETRY1",
    requester: "222:1@s.whatsapp.net",
    encryptionJid: "222:1@s.whatsapp.net",
    retryCount: 1,
    route: { kind: "direct" },
  };
  const first = await alice.signal.resend(resolver, request);
  const tree = stanza(first);
  assert.equal(tree.attrs.id, "RETRY1");
  assert.equal(find(tree, "enc").attrs.count, "1");
  const delivered = await receiveStanza(bob.signal, first);
  assert.equal(conversationOf(delivered.message), "lost once");
  await persistThenMark(bob.signal, delivered);
  const reset = await device("bob-resend-after-reset");
  const freshBundle = await bundleFor(reset.signal);
  const second = await alice.signal.resend(resolver, {
    ...request,
    retryCount: 2,
    bundle: freshBundle,
  });
  assert.equal(find(stanza(second), "enc").attrs.count, "2");
  const recovered = await receiveStanza(reset.signal, second);
  assert.equal(conversationOf(recovered.message), "lost once");
  await persistThenMark(reset.signal, recovered);
  await assert.rejects(
    alice.signal.resend(resolver, { ...request, messageId: "missing" }),
    (e) => e.code === "invalid_input",
  );
  assert.equal(alice.backend.containsBytes(text("lost once")), false);
  assert.equal(
    await alice.signal.pruneSentMessages(Number.MAX_SAFE_INTEGER),
    1,
  );
  await assert.rejects(
    alice.signal.resend(resolver, request),
    (e) => e.code === "invalid_input",
  );
});

test("a group retry clears only the requester distribution acknowledgment", async () => {
  const group = "120363000000001@g.us";
  const alice = await device("alice-group-resend");
  const bob = await device("bob-group-resend");
  await alice.signal.setOwnJids("111:1@s.whatsapp.net", "900111:1@lid");
  await pair(alice.signal);
  const primary = await device("bob-primary-group-resend");
  const primaryBundle = { ...(await bundleFor(primary.signal)), deviceId: 0 };
  const companionBundle = await bundleFor(bob.signal);
  const resolver = resolverFor(companionBundle, group);
  resolver.resolveDevices = async () => [BOB, "222:1@s.whatsapp.net"];
  resolver.fetchPrekeys = async (jids) =>
    Object.fromEntries(
      jids.map((jid) => [jid, jid === BOB ? primaryBundle : companionBundle]),
    );
  const initial = await alice.signal.sendGroup(
    resolver,
    group,
    conversation("group retry"),
    "GROUP1",
  );
  assert.deepEqual(
    [...initial.distributionTargets].sort(),
    [BOB, "222:1@s.whatsapp.net"].sort(),
  );
  assert.equal(typeof initial.distributionEpoch, "bigint");
  assert.equal(
    await alice.signal.markSenderKeyDistributed(
      group,
      initial.distributionTargets,
      initial.distributionEpoch,
    ),
    true,
  );
  assert.deepEqual(
    (
      await alice.signal.sendGroup(
        resolver,
        group,
        conversation("warm"),
        "GROUP2",
      )
    ).distributionTargets,
    [],
  );
  const retried = await alice.signal.resend(resolver, {
    chat: group,
    messageId: "GROUP1",
    requester: "222:1@s.whatsapp.net",
    encryptionJid: "222:1@s.whatsapp.net",
    retryCount: 1,
    route: { kind: "group", addressingMode: "pn" },
  });
  assert.equal(stanza(retried).attrs.to, group);
  const received = await receiveStanza(bob.signal, retried, group);
  assert.equal(conversationOf(received.message), "group retry");
  await persistThenMark(bob.signal, received);
  // A late acknowledgment of the send before the retry must not restore the
  // requester's cleared mark.
  assert.equal(
    await alice.signal.markSenderKeyDistributed(
      group,
      initial.distributionTargets,
      initial.distributionEpoch,
    ),
    false,
  );
  assert.deepEqual(
    (
      await alice.signal.sendGroup(
        resolver,
        group,
        conversation("redistribute"),
        "GROUP3",
      )
    ).distributionTargets,
    ["222:1@s.whatsapp.net"],
  );
});

// Synthetic protobuf fixtures follow waproto/src/whatsapp.proto field numbers.
function varint(value) {
  const bytes = [];
  do {
    bytes.push((value & 127) | (value > 127 ? 128 : 0));
    value = Math.floor(value / 128);
  } while (value);
  return Uint8Array.from(bytes);
}
function field(number, bytes) {
  return Uint8Array.from([
    ...varint(number * 8 + 2),
    ...varint(bytes.length),
    ...bytes,
  ]);
}
function pad(bytes) {
  const count = 1 + (bytes.length % 16);
  return Uint8Array.from([...bytes, ...new Array(count).fill(count)]);
}

test("receive exposes rejected sender keys without losing ordinary application content", async () => {
  const alice = await device("alice-rejected-skdm");
  const bob = await device("bob-rejected-skdm");
  await alice.signal.establishSession(BOB, 1, await bundleFor(bob.signal));
  const badSkdm = field(2, [
    ...field(1, text("120363000000001@g.us")),
    ...field(2, [1, 2, 3]),
  ]);
  const bytes = pad(
    Uint8Array.from([...conversation("keep the text"), ...badSkdm]),
  );
  const cipher = await alice.signal.encrypt(BOB, 1, bytes);
  const received = await bob.signal.receive(
    "120363000000001@g.us",
    ALICE,
    1,
    cipher.kind,
    cipher.ciphertext,
    2,
    false,
  );
  assert.equal(received.senderKeyRejected, true);
  assert.equal(conversationOf(received.message), "keep the text");
  assert.equal(
    Buffer.from(received.message).includes(Buffer.from(badSkdm)),
    false,
  );
  await persistThenMark(bob.signal, received);
});

test("app-state sharing requires separate explicit consent and own-device evidence", async () => {
  const share = field(1, [
    ...field(1, field(1, new Uint8Array(32).fill(3))),
    ...field(2, field(1, new Uint8Array(32).fill(4))),
  ]);
  const protocol = field(12, [...varint(2 * 8), 6, ...field(7, share)]);
  for (const [consent, fromMe] of [
    [undefined, true],
    [false, true],
    [true, true],
    [true, false],
  ]) {
    const alice = await device(`alice-share-${consent}-${fromMe}`);
    const bob = await device(`bob-share-${consent}-${fromMe}`);
    await alice.signal.establishSession(BOB, 1, await bundleFor(bob.signal));
    const cipher = await alice.signal.encrypt(
      BOB,
      1,
      pad(
        Uint8Array.from([
          ...conversation("mixed own-device content"),
          ...protocol,
        ]),
      ),
    );
    const received = await bob.signal.receive(
      ALICE,
      ALICE,
      1,
      cipher.kind,
      cipher.ciphertext,
      2,
      fromMe,
      consent,
    );
    assert.equal(conversationOf(received.message), "mixed own-device content");
    assert.equal(
      Buffer.from(received.message).includes(Buffer.from(share)),
      false,
      "application bytes must omit key material",
    );
    assert.equal(
      received.appStateKeyShare.length !== 0,
      consent === true && fromMe,
    );
    if (consent === true && fromMe)
      assert.ok(
        Buffer.from(received.appStateKeyShare).equals(Buffer.from(share)),
        "separate share bytes match the synthetic protocol fixture",
      );
    const retry = await bob.signal.receive(
      ALICE,
      ALICE,
      1,
      cipher.kind,
      cipher.ciphertext,
      2,
      fromMe,
    );
    assert.equal(retry.redelivered, true);
    assert.equal(retry.appStateKeyShare.length, 0);
    await persistThenMark(bob.signal, received);
    assert.equal(
      (
        await bob.signal.receive(
          ALICE,
          ALICE,
          1,
          cipher.kind,
          cipher.ciphertext,
          2,
          fromMe,
          true,
        )
      ).status,
      "already_delivered",
    );
  }
});

test("new WASM input validation refuses malformed counts, routes and retry metadata", async () => {
  const bob = await device("bob-new-inputs");
  for (const count of [0, -1, 1.5, 813, NaN, Infinity, 2 ** 32 + 1])
    await assert.rejects(
      bob.signal.preparePrekeyUpload(count),
      (e) => e.code === "invalid_input",
    );
  for (const value of [-1, 0.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1])
    await assert.rejects(
      bob.signal.pruneSentMessages(value),
      (e) => e.code === "invalid_input",
    );
  for (const value of ["false", "true", 1, {}])
    await assert.rejects(
      bob.signal.receive(
        ALICE,
        ALICE,
        1,
        "pkmsg",
        new Uint8Array(),
        2,
        true,
        value,
      ),
      (e) => e.code === "invalid_input",
    );
  const request = {
    chat: BOB,
    messageId: "id",
    requester: "222:1@s.whatsapp.net",
    encryptionJid: "222:1@s.whatsapp.net",
    retryCount: 1,
    route: { kind: "direct" },
  };
  const resolver = resolverFor(await bundleFor(bob.signal));
  for (const patch of [
    { retryCount: 256 },
    { retryCount: 0.5 },
    { route: { kind: "unknown" } },
    { route: { kind: "group", addressingMode: "unknown" } },
    { bundle: { registrationId: 1 } },
    { requester: "invalid jid" },
  ])
    await assert.rejects(
      bob.signal.resend(resolver, { ...request, ...patch }),
      (e) => e.code === "invalid_input",
    );
});

test("initialized-module upload example keeps pairing secrets out of its callback", async () => {
  const backend = new MemoryBackend();
  const sealer = await webCryptoSealer();
  const options = {
    backend,
    sealer,
    scope: "example-upload",
    holder: "example-a",
    nowMs: 0,
    ttlMs: TTL,
    count: 2,
  };
  let first;
  await assert.rejects(
    uploadPublicPrekeys({ SignalDevice }, options, async (upload) => {
      first = upload;
      throw new Error("synthetic provider response lost");
    }),
  );
  assert.deepEqual(Object.keys(first).sort(), [
    "batchId",
    "identityKey",
    "preKeys",
    "registrationId",
    "signedPrekey",
  ]);
  const id = await uploadPublicPrekeys(
    { SignalDevice },
    { ...options, holder: "example-b", nowMs: 1 },
    async (upload) => assert.deepEqual(upload, first),
  );
  assert.equal(id, first.batchId);
  const reopened = await SignalDevice.open(
    backend,
    sealer,
    "example-upload",
    "example-c",
    2,
    TTL,
    false,
  );
  assert.equal(await reopened.uploadedPrekeyCount(), 2);
  await reopened.release();
  reopened.free();
  const raw = new WebAssembly.Instance(
    new WebAssembly.Module(Uint8Array.from([0, 97, 115, 109, 1, 0, 0, 0])),
  );
  await assert.rejects(
    uploadPublicPrekeys(raw, options, async () => {}),
    TypeError,
  );
});

class SlowReads extends MemoryBackend {
  async get(...args) {
    await new Promise((resolve) => setTimeout(resolve, 5));
    return super.get(...args);
  }
}

test("a failed open keeps the lease that a concurrent open by the same holder shares", async () => {
  const backend = new SlowReads();
  const sealer = await webCryptoSealer();
  const alice = await device("alice-live-lease");
  const seed = await SignalDevice.open(
    backend,
    sealer,
    "bob-live-lease",
    "tab-a",
    0,
    TTL,
    true,
  );
  await seed.release();
  seed.free();
  // Same holder, same lease: the first open reads with the wrong key and
  // fails while the second is still starting. The failure must not release
  // the lease the second one is about to use.
  const opens = await Promise.allSettled([
    SignalDevice.open(
      backend,
      await webCryptoSealer(),
      "bob-live-lease",
      "tab-a",
      1,
      TTL,
      false,
    ),
    SignalDevice.open(backend, sealer, "bob-live-lease", "tab-a", 1, TTL, false),
  ]);
  assert.equal(opens[0].status, "rejected");
  assert.equal(opens[1].status, "fulfilled");
  const bob = opens[1].value;
  await assert.rejects(
    SignalDevice.open(backend, sealer, "bob-live-lease", "tab-b", 2, TTL, false),
    (error) => error.code === "lease_held",
  );
  await alice.signal.establishSession(BOB, 1, await bundleFor(bob));
  const sent = await alice.signal.encrypt(BOB, 1, text("still live"));
  const got = await bob.decrypt(ALICE, 1, sent.kind, sent.ciphertext);
  assert.equal(read(got.plaintext), "still live");
  const reply = await bob.encrypt(ALICE, 1, text("reply"));
  assert.ok(reply.ciphertext.length > 0);
});

test("a second open with the wrong sealer does not disturb the live device", async () => {
  const backend = new MemoryBackend();
  const sealer = await webCryptoSealer();
  const bob = await device("bob-wrong-key", { backend, sealer, now: 0 });
  await SignalDevice.open(
    backend,
    await webCryptoSealer(),
    "bob-wrong-key",
    "tab-a",
    1,
    TTL,
    false,
  ).catch(() => {});
  await assert.rejects(
    SignalDevice.open(backend, sealer, "bob-wrong-key", "tab-b", 2, TTL, false),
    (error) => error.code === "lease_held",
  );
  assert.ok((await bob.signal.publicIdentity()).identityKey.length > 0);
});

test("pruneDecryptBuffer refuses cutoffs that are not safe integers", async () => {
  const bob = await device("bob-prune-input");
  for (const bad of [Infinity, NaN, -1, 1.5]) {
    await assert.rejects(
      bob.signal.pruneDecryptBuffer(bad),
      (error) => error.code === "invalid_input",
    );
  }
});

test("buffered records carry the wall clock, so a yesterday cutoff keeps them", async () => {
  const alice = await device("alice-clock");
  const bob = await device("bob-clock");
  await alice.signal.establishSession(BOB, 1, await bundleFor(bob.signal));
  const sent = await alice.signal.encrypt(BOB, 1, paddedConversation("fresh"));
  const args = [ALICE, ALICE, 1, sent.kind, sent.ciphertext, 2, false];
  const first = await bob.signal.receive(...args);
  assert.equal(first.status, "message");
  assert.equal(await bob.signal.pruneDecryptBuffer(Date.now() - 86_400_000), 0);
  const again = await bob.signal.receive(...args);
  assert.equal(again.redelivered, true);
  assert.equal(conversationOf(again.message), "fresh");
});

test("a send to a phone number reuses the session migrated to its LID", async () => {
  const alice = await device("alice-lid-send");
  const bob = await device("bob-lid-send");
  const BOB_LID = "999@lid";
  await alice.signal.setOwnJids("111:1@s.whatsapp.net", undefined);
  await alice.signal.establishSession(BOB, 1, await bundleFor(bob.signal));
  assert.equal(await alice.signal.migratePnToLid(BOB, BOB_LID), 1);
  const lookups = [];
  const resolver = {
    async resolveDevices() {
      return ["222:1@s.whatsapp.net"];
    },
    async fetchPrekeys() {
      return {};
    },
    async lidForPhone(user) {
      lookups.push(user);
      return user === "222" ? "999" : undefined;
    },
    async resolveGroup() {
      throw new Error("not a group send");
    },
  };
  const sent = await alice.signal.sendDirect(
    resolver,
    BOB,
    conversation("over lid"),
    "LID1",
  );
  assert.deepEqual(sent.unreachedDevices, []);
  assert.ok(lookups.includes("222"));
  const delivered = await receiveStanza(bob.signal, sent.stanza);
  assert.equal(conversationOf(delivered.message), "over lid");
});
