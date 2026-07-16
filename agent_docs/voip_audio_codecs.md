# VoIP audio codec boundary

The VoIP media core owns call signaling, RTP timing, SRTP/WARP protection, relay transport, and
receive statistics. Codec work is optional. An application can either use the built-in PCM/MLOW
path or exchange complete encoded payloads through `encoded_audio`.

```text
PCM application                 Encoded application
     │                                  │ raw codec packet
     ▼                                  ▼
built-in MLOW adapter          EncodedAudioSource / Sink
     │                                  │
     └──────── codec payload ────────────┘
                        │
                  RTP + SRTP/WARP
                        │
                 WhatsApp relay
```

The encoded boundary does not transcode codec bytes. It only validates and classifies the MLOW
profile's standard-Opus escape. Codec selection still is not arbitrary: the selected `AudioFormat`
fixes the signaling rate, RTP profile, payload type, clock, and packet cadence for the whole call.
A format cannot change in the middle of a call.

## Supported profiles

| Format | RTP profile | `<audio rate>` | Codec PCM | Frame | RTP clock | Step | PT |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `MLOW_16KHZ_60MS` | MLOW | 16000 | 16 kHz mono | 960 / 60 ms | 16 kHz | 960 | 120 |
| `OPUS_MLOW_16KHZ_60MS` | MLOW escape | 16000 | 16 kHz mono | 960 / 60 ms | 16 kHz | 960 | 120 |
| `OPUS_16KHZ_60MS` | native Opus | 16000 | 16 kHz mono | 960 / 60 ms | 16 kHz | 960 | 120 |
| `OPUS_RFC7587_16KHZ_60MS` | RFC 7587 Opus | 16000 | 16 kHz mono | 960 / 60 ms | 48 kHz | 2880 | 111 |
| `OPUS_RFC7587_48KHZ_60MS` | RFC 7587 Opus | 16000 | 48 kHz mono | 2880 / 60 ms | 48 kHz | 2880 | 111 |

`<audio enc="opus">` names the codec family, not the RTP payload profile. The signaling rate also
does not determine that profile by itself. Capability v1 index 31 gates the codec the peer sends:
advertising it keeps MLOW available, while omitting it selects the standard-Opus fallback. The
answer's directional `voip_settings.encode.use_mlow_codec_v1` selects the decoder for media sent by
the answerer; both sides must agree for full-duplex native Opus. A separate
`options.enable_48khz_rtp_clock` value selects PT 111 with a 48 kHz RTP clock. When false, both MLOW
and native Opus use PT 120 with a 16 kHz clock. The serialized capability is
`version-count | mask-length | mask`, so disabling MLOW changes only bit `0x80` in byte 5 of the
captured seven-byte blob. MLOW redundancy packets use PT 121. Encoded sinks receive both the actual
PT and the classified per-packet codec in
`EncodedAudioFrame`, because a MLOW-profile peer may send proprietary MLOW while our source sends
standard Opus through its escape.

The server-forwarded 1:1 offer can carry a large `<voip_settings uncompressed="1">` JSON object.
The Android receive path first loads its local defaults and then overlays only fields present in the
peer's answer. A native-Opus accept therefore sends a minimal, static settings object containing
`encode.use_mlow_codec_v1` and `options.enable_48khz_rtp_clock`; copying and reparsing the peer's
rollout settings is unnecessary. The native parser rejects compressed VoIP settings on this path,
so the answer explicitly marks this small JSON as uncompressed.

Outgoing calls advertise exactly the selected rate. Incoming calls can select only a rate present
in the offer. A peer that later preaccepts or accepts with a different rate produces
`CallEvent::AudioFormatMismatch` and the call is terminated instead of decoding with the wrong
codec. Payload bytes are not used to select the call profile. Once MLOW is negotiated, PT 120 bytes
starting with `0b11` are its standard-Opus/CELT escape; PT 121 remains MLOW RED even when its RED
header starts with the same bits. The PCM path surfaces escaped packets as `CallEvent::ForeignAudio`
for an optional Opus decoder.

For the escape, the source must produce CELT-only Opus. MLOW uses its own CELT TOC rather than the
RFC Opus TOC; `packetize_opus_for_mlow` rewrites it without transcoding and without allocating for
the common one-frame and arbitrary-frame packet forms. The inverse `depacketize_opus_from_mlow`
prepares an inbound escape for a stock Opus decoder. The included libopus adapter does both and uses
16 kHz wideband restricted-low-delay mode, avoiding an otherwise unnecessary 16→48 kHz resample.

DTX is part of that profile boundary. Stock libopus may represent a 60 ms DTX interval as the
two-byte RFC packet `BB 03`; sending only a rewritten CELT TOC would set MLOW's VAD bit and make the
receiver consume the RTP speech marker while the payload still contains silence. The packetizer
therefore maps libopus DTX packets of at most two bytes to MLOW's one-byte SID `90`. The RTP stream
re-arms its speech latch on SID/DTX, so the first subsequent coded voice packet carries the marker.
An external Opus producer should enable DTX; passing every packet through
`packetize_opus_for_mlow` applies the same mapping.

## Cargo profiles

The former `voip` feature remains the compatibility profile and includes the native runtime, MLOW,
and the optional libopus helpers. Smaller deployments can select only what they use:

| Feature | Contents |
| --- | --- |
| `voip-encoded` | Native relay/runtime and raw encoded I/O; no audio codec implementation |
| `voip-mlow` | Native relay/runtime plus the pure-Rust PCM/MLOW adapter |
| `voip-libopus` | Encoded runtime plus `WaOpusEncoder`/`WaOpusDecoder` backed by libopus |
| `voip` | Compatibility aggregate: `voip-mlow` + `voip-libopus` |
| `wacore/voip` | Runtime-agnostic media crypto, RTP, engine, and encoded I/O |
| `wacore/voip-mlow` | `wacore/voip` plus the pure-Rust MLOW codec |

An application that already has Opus packets needs only `voip-encoded`; it does not need libopus or
the MLOW implementation in its dependency graph.

## Encoded API

The source sends one complete raw codec packet per item, paced at the format's 60 ms cadence. The
sink receives one decrypted packet plus its RTP metadata.

```rust,ignore
use bytes::Bytes;
use whatsapp_rust::voip::{AudioFormat, EncodedAudioFrame};

let (encoded_tx, encoded_rx) = async_channel::bounded::<Bytes>(3);
let (playout_tx, playout_rx) = async_channel::bounded::<EncodedAudioFrame>(3);

let call = client
    .voip()
    .call(&peer)
    .encoded_audio(AudioFormat::OPUS_MLOW_16KHZ_60MS, encoded_rx, playout_tx)
    .start()
    .await?;

// Producer: encode one PCM frame, then send Bytes containing only the Opus packet.
// Consumer: decode frame.data; frame.timestamp/PT remain available for diagnostics.
```

`Bytes` avoids another copy when ownership crosses the application/core boundary. RTP/SRTP output
still requires a protected packet buffer.

## FFmpeg and external converters

FFmpeg can be used without linking a Rust codec library, but its output framing matters:

- `ffmpeg ... -f opus pipe:1` emits an Ogg Opus stream. Ogg pages must not be sent to
  `encoded_audio`.
- An FFmpeg RTP output already contains RTP headers. The application must parse it and extract each
  raw Opus payload because this core creates the WhatsApp RTP/SRTP packet itself.
- A libavcodec integration can read each encoded `AVPacket` directly. For
  `OPUS_MLOW_16KHZ_60MS`, configure 16 kHz mono CELT/WB with 60 ms packets, call
  `packetize_opus_for_mlow` on each packet, then pass it as `Bytes`.
- A child-process adapter can define a small length-prefixed IPC protocol around raw packets. This
  avoids a linked codec dependency but adds process supervision, IPC copies, and another failure
  boundary.
- For the built-in MLOW path, FFmpeg can decode or resample input to signed 16-bit, mono, 16 kHz PCM;
  the application chunks exactly 960 samples and sends those frames through `audio(...)`.

Compatible CELT Opus needs only the reversible MLOW packet-header rewrite. Arbitrary Opus cannot be
made compatible that way. A SILK/Hybrid packet must be decoded and re-encoded as CELT, or
transcoded all the way to proprietary MLOW:

```text
raw Opus → Opus decoder → 16 kHz mono PCM → MLOW encoder → raw MLOW
```

That conversion costs CPU, adds latency, and introduces another lossy generation. Prefer producing
compatible CELT at the source. FFmpeg has standard Opus support but no MLOW codec, so the last step
requires this project's MLOW encoder or another compatible implementation.

## Interoperability evidence

The profile mapping was checked against the locally decompiled Android media split and captured Web
artifacts, without copying proprietary implementation code:

- The Android media registration path distinguishes `use_mlow_codec` from RTP clock selection.
- Codec registration and RTP-clock selection are independent: disabling MLOW can produce native
  Opus on PT 120 with a 16 kHz clock.
- `enable_48khz_rtp_clock` switches the native Opus path to PT 111 with a 48 kHz RTP clock; the
  server-provided setting was false in the live Android negotiation.
- PT 121 is registered as `mlow-red-1`.
- Audio capability intersection is performed before preaccept/accept, confirming that the signaling
  rate must be preserved rather than discarded.
- Capability v1 index 31 controls `use_mlow_codec`: the official peer clears that media parameter
  when the bit is absent and recreates the audio stream if the effective value changes.
- The current Android `accept` handler stores an incoming settings object before intersecting device
  capabilities, then reapplies the selected VoIP parameters and recreates transport/media state.
- The current Android NetEQ registration chooses `mlow-1` or `opus` directly from the effective
  `use_mlow_codec` value; payload inspection is not the negotiation mechanism.
- Android/Web artifacts contain separate MLOW and standard Opus decoder paths, which confirms that a
  payload-content heuristic is not a valid negotiation mechanism.
- The current Android APK's Ghidra project confirms that MLOW mode selects `mlow_packet_parse`, sets
  the MLOW control on both libopus encoder and decoder, and still decodes escaped packets through
  `opus_decode`.
- The negotiated settings request one-byte DTX frames and a speech-resume RTP marker. The WebAssembly
  receiver independently contains marker-controlled DTX-exit state, matching the SID/marker behavior
  above.
- A live Android call answered with `rate=8000` tore down its audio devices before sending RTP.
  With `rate=16000` and MLOW capability 31 cleared, media remained connected and the peer sent PT
  120. This confirms the native Opus/PT 120 combination selected by the decompiled media path.
- In that connected call the Android RTCP receiver reports acknowledged the outbound audio SSRC
  without loss while playout stayed silent. Together with the decoder-selection path above, this
  distinguishes a directional negotiation mismatch from RTP, SRTP, relay, volume, or routing loss.

The corresponding local research points are in the current `wa-apk-re/ghidra-project` and the
captured WebAssembly/JavaScript artifacts under the sibling reverse-engineering projects.

## MLOW versus Opus

MLOW keeps the entire PCM codec path pure Rust and matches the newer WhatsApp-specific profile. It
has no external runtime dependency, but it contributes substantially more code and its
analysis-by-synthesis encoder is CPU/allocation heavy.

Standard Opus has a mature tool ecosystem and lets the Rust core omit the outbound codec when
another component already produces compatible packets. Linking `voip-libopus` is convenient, while
an FFmpeg process keeps the Rust dependency graph smaller at the cost of operational complexity.
Clearing MLOW capability 31 selects the native Opus codec; it does not itself select PT 111. The
default observed path remains PT 120/16 kHz, while the independent 48 kHz-clock setting selects the
RFC 7587 PT 111 variant. The bundled libopus adapter follows the observed 60 ms, 24 kbps, DTX, and
complexity-5 configuration so its CPU cost and packet behavior track the official sender more
closely.

The escape is asymmetric: a peer that negotiated the MLOW profile can still send proprietary MLOW
inbound. An Opus-only binary keeps the call alive and exposes those packets, but needs an external
MLOW decoder for full-duplex playout.

Transcoding Opus to MLOW combines the disadvantages of both paths and should be a compatibility
fallback, not the normal architecture.

## Video scope

Video already accepts externally encoded access units, so encoding can live in FFmpeg or another
process. It is not codec-agnostic on the wire yet: signaling, packetization, depacketization, and PT
97 currently target H.264 Annex-B. Supporting another video codec requires a negotiated
`VideoFormat` plus a packetizer/depacketizer for that codec; swapping only the external encoder is
not sufficient.

## Production comparison

The VoIP CLI can switch codecs without changing the call flow:

```bash
# One binary containing both variants
WA_AUDIO_CODEC=mlow cargo run -p whatsapp-rust-voip-cli --release -- listen accept
WA_AUDIO_CODEC=opus cargo run -p whatsapp-rust-voip-cli --release -- listen accept
WA_AUDIO_CODEC=opus WA_AUDIO_PROFILE=mlow \
  cargo run -p whatsapp-rust-voip-cli --release -- listen accept # MLOW escape diagnostic

# Binaries proving that the unused codec is absent
WA_AUDIO_CODEC=mlow cargo run -p whatsapp-rust-voip-cli --release \
  --no-default-features --features voip-mlow -- listen accept
WA_AUDIO_CODEC=opus cargo run -p whatsapp-rust-voip-cli --release \
  --no-default-features --features voip-opus -- listen accept
```

Compare at least call-setup success, format mismatches, first-audio latency, CPU time, peak RSS,
microphone drops, playout underruns, RTP loss/jitter, and subjective quality. Keep the codec fixed for
the call; if a cohort fails, roll back on the next call rather than attempting an in-call format
switch.

The MLOW realtime path reuses its sanitization, range-coder, encoded-output, VAD, and history
buffers. The pitch estimator also avoids short-lived per-subframe arrays. The allocation benchmark
for the varied voiced stream moved from 673 allocations / about 494.7 KB per encode to 627
allocations / about 456.5 KB on the engine-style reused-output path. Continue treating CPU numbers
as benchmark-host-specific; the byte-exact and reverse-engineering ground-truth tests guard codec
behavior.
