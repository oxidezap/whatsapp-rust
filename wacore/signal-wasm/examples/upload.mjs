// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

// The host initializes its own wasm-bindgen module and supplies durable
// fenced storage plus an AAD-authenticating sealer. Keep one device per held
// scope. The callback owns provider transport, retries and deadlines.
export async function uploadPublicPrekeys(
  wasm,
  { backend, sealer, scope, holder, nowMs, ttlMs, count = 812 },
  upload,
) {
  if (typeof wasm?.SignalDevice?.open !== "function")
    throw new TypeError(
      "An initialized wasm-bindgen module exposing SignalDevice is required.",
    );
  const device = await wasm.SignalDevice.open(
    backend,
    sealer,
    scope,
    holder,
    nowMs,
    ttlMs,
    true,
  );
  try {
    const batch = await device.preparePrekeyUpload(count);
    const { registrationId, identityKey, signedPrekey } =
      await device.publicIdentity();
    // ADV pairing material is absent from the upload callback. A failed/lost
    // response leaves the durable batch available when the caller reopens it.
    await upload({
      registrationId,
      identityKey,
      signedPrekey,
      batchId: batch.batchId,
      preKeys: batch.preKeys,
    });
    await device.acknowledgePrekeyUpload(
      batch.batchId,
      Uint32Array.from(batch.preKeys.map((key) => key.id)),
    );
    return batch.batchId;
  } finally {
    try {
      await device.release();
    } finally {
      device.free();
    }
  }
}
