// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

// Compile against the actual generated bindings after the Node-target build.
import {
  SignalDevice,
  type PrekeyUploadBatch,
  type ReceiveResult,
  type SendResolver,
} from "../../../target/signal-wasm-node/wacore_signal_wasm.js";

declare const device: SignalDevice;
declare const resolver: SendResolver;
const bytes = new Uint8Array();
const batch: PrekeyUploadBatch = await device.preparePrekeyUpload(2);
await device.acknowledgePrekeyUpload(
  batch.batchId,
  Uint32Array.from(batch.preKeys.map((key) => key.id)),
);
const count: number = await device.uploadedPrekeyCount();
const received: ReceiveResult = await device.receive(
  "chat",
  "sender",
  1,
  "pkmsg",
  bytes,
  2,
  false,
);
await device.receive("chat", "sender", 1, "pkmsg", bytes, 2, true, true);
if (received.status === "message") {
  const rejected: boolean = received.senderKeyRejected;
  const share: Uint8Array = received.appStateKeyShare;
  void rejected;
  void share;
}
const request = {
  chat: "chat",
  messageId: "id",
  requester: "peer",
  encryptionJid: "peer",
  retryCount: 1,
};
await device.resend(resolver, { ...request, route: { kind: "direct" } });
await device.resend(resolver, {
  ...request,
  route: { kind: "group", addressingMode: "lid" },
});
await device.resend(resolver, {
  ...request,
  route: { kind: "broadcast_list" },
});
// @ts-expect-error Consent is an explicit boolean, never a truthy string.
await device.receive("chat", "sender", 1, "pkmsg", bytes, 2, true, "true");
// @ts-expect-error Group retries require a declared addressing mode.
await device.resend(resolver, { ...request, route: { kind: "group" } });
await device.resend(resolver, {
  ...request,
  // @ts-expect-error Retry counts remain numeric public metadata.
  retryCount: "1",
  route: { kind: "direct" },
});
const invalid =
  // @ts-expect-error Only a message result contains application bytes.
  received.status === "already_delivered" ? received.message : undefined;
declare function acceptModule(module: {
  SignalDevice: typeof SignalDevice;
}): void;
acceptModule({ SignalDevice });
declare const raw: WebAssembly.Instance;
// @ts-expect-error A raw WebAssembly instance lacks wasm-bindgen's class/glue.
acceptModule(raw);
// lidForPhone is optional on a resolver.
const withLid: SendResolver = {
  resolveDevices: async (jids) => jids,
  fetchPrekeys: async () => ({}),
  lidForPhone: async (user) => (user === "1" ? "2" : undefined),
  resolveGroup: async () => ({ participants: [], addressingMode: "pn" }),
};
void withLid;
void count;
void invalid;
