# STUN protobuf attribute payloads

The WhatsApp VoIP relay Allocate request carries three protobuf-encoded STUN attributes. We do not
vendor the upstream `.proto` files or a protobuf runtime: the encoders in `stun.rs`
(`create_voip_sender_subscriptions`, `create_apk_sender_subscriptions`,
`create_apk_stream_descriptors`) emit the wire bytes directly, and the frozen `stun_proto` KATs in
`kats.json` pin them (`stun.rs::protobuf_payloads_match_kat`).

This doc is the field layout those encoders implement, so the format stays reviewable without the
external proto sources. All KAT vectors use `ssrc = 0x12345678` and (where applicable) `pid = 7`,
`layerId = "audio"`.

## `voip.SenderSubscriptions` (WASM client, STUN attr `0x4000`)

One audio sender. `ssrc` is a plain `uint32` varint.

```
message SenderSubscriptions {       // attr payload
  repeated Sender senders = 1;       // one entry
}
message Sender {
  uint32 ssrc          = 3;          // = 0x12345678
  StreamLayer  stream_layer = 5;     // AUDIO  = 0
  PayloadType  payload_type = 6;     // MEDIA  = 0
}
```

KAT `voip_sender_subscriptions = 0a0a18f8acd1910128003000`:
`0a 0a` senders[0] len=10; `18 f8acd1910 1` field3 ssrc; `28 00` field5=0; `30 00` field6=0.

## `wa.voip.SenderSubscriptions` (APK client, STUN attr `0x4025`)

One audio ssrc, with the ssrc carried as a **zigzag `sint64`**. The optional per-participant `pid`
nests a `layerId` string.

```
message SenderSubscriptions {
  repeated Subscription subscriptions = 1;
}
message Subscription {
  SsrcLayers ssrc_layers = 1;
}
message SsrcLayers {
  repeated sint64 ssrcs = 1;         // ssrcs[0] = zigzag(0x12345678)
  repeated Pid    pids  = 2;         // empty in the nopid KAT
}
message Pid {
  sint64 pid      = 1;               // zigzag(7)
  string layer_id = 2;              // "audio"
}
```

KAT `apk_sender_subscriptions_nopid = 0a08 0a06 08f0d9a2a302`:
subscriptions[0] -> ssrc_layers -> ssrcs[0] = zigzag(0x12345678).

KAT `apk_sender_subscriptions_pid` additionally carries `pids[0] = { pid: zigzag(7), layerId:
"audio" }`.

## `wa.voip.StreamDescriptors` (APK client, STUN attr `0x4024`)

One audio/OPUS descriptor. `ssrc` is a zigzag `sint64`.

```
message StreamDescriptors {
  repeated StreamDescriptor stream_descriptors = 1;
}
message StreamDescriptor {
  string stream_layer                 = 1;   // "audio"
  string payload_type                 = 2;   // "OPUS"
  sint64 ssrc                         = 3;   // zigzag(0x12345678)
  bool   is_uplink_prefetch_enabled   = 4;   // false (0)
}
```

KAT `apk_stream_descriptors = 0a15 0a05 617564696f 12 04 4f505553 18 f0d9a2a302 20 00`:
stream_descriptors[0] -> "audio", "OPUS", ssrc, is_uplink_prefetch_enabled=false.

## Provenance

`kats.json` is a frozen fixture. The crypto vectors (the e2e-srtp / hbh-srtp / sframe key schedules and
the RTP/RTCP IVs) were generated once by an independent reference (standard HKDF-SHA256 + the RFC 3711
AES-128-CTR KDF) — the WhatsApp VoIP key schedule is fixed, so they never change. The Rust impl is
pinned against them byte-for-byte by the KAT tests in `mod.rs` / `e2e_srtp.rs` / `sframe.rs` /
`hbh_srtp.rs`; those tests catch drift in the impl, not in the vectors. If a brand-new crypto vector is
ever needed, compute it with an independent crypto tool (not the impl under test, which would be
circular).

The STUN protobuf vectors (`stun`, `ping`, `apk_*`, `attr_token`) are re-encodable by hand from the
field layout documented above: to change `ssrc`/`pid`, re-encode the payload and update `kats.json`;
the Rust encoders + `protobuf_payloads_match_kat` then re-pin it.
