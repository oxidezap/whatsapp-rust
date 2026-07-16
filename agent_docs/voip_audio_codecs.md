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

The encoded boundary does not inspect codec bytes. Codec selection still is not arbitrary: the
selected `AudioFormat` fixes the signaling profile, RTP payload type, RTP clock, and packet cadence
for the whole call. A format cannot change in the middle of a call.

## Supported profiles

| Format | `<audio rate>` | Codec PCM | Frame | RTP clock | Timestamp step | RTP PT |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `MLOW_16KHZ_60MS` | 16000 | 16 kHz mono | 960 samples / 60 ms | 16 kHz | 960 | 120 |
| `OPUS_16KHZ_60MS` | 8000 | 16 kHz mono | 960 samples / 60 ms | 48 kHz | 2880 | 111 |
| `OPUS_RFC7587_48KHZ_60MS` | 8000 | 48 kHz mono | 2880 samples / 60 ms | 48 kHz | 2880 | 111 |

The Opus signaling `rate=8000` is a WhatsApp media-profile selector. It is not the PCM sample rate
and does not change RFC 7587's 48 kHz RTP clock. MLOW redundancy packets use PT 121; encoded MLOW
sinks receive the actual PT in `EncodedAudioFrame` so they can distinguish primary and redundant
payloads.

Outgoing calls advertise exactly the selected rate. Incoming calls can select only a rate present
in the offer. A peer that later preaccepts or accepts with a different rate produces
`CallEvent::AudioFormatMismatch` and the call is terminated instead of decoding with the wrong
codec. Payload bytes are deliberately not used to select the call profile because valid Opus and
MLOW TOCs overlap. After MLOW has been negotiated, its own standard-Opus escape remains supported;
the PCM path surfaces those packets as `CallEvent::ForeignAudio` for an optional Opus decoder.

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
    .encoded_audio(AudioFormat::OPUS_16KHZ_60MS, encoded_rx, playout_tx)
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
- A libavcodec integration can read each encoded `AVPacket` directly and pass its payload as one
  `Bytes` item.
- A child-process adapter can define a small length-prefixed IPC protocol around raw packets. This
  avoids a linked codec dependency but adds process supervision, IPC copies, and another failure
  boundary.
- For the built-in MLOW path, FFmpeg can decode or resample input to signed 16-bit, mono, 16 kHz PCM;
  the application chunks exactly 960 samples and sends those frames through `audio(...)`.

Opus cannot be transformed into MLOW by rewriting a header. They are different codecs. Conversion
must decode Opus to PCM and encode that PCM as MLOW:

```text
raw Opus → Opus decoder → 16 kHz mono PCM → MLOW encoder → raw MLOW
```

That conversion costs CPU, adds latency, and introduces another lossy generation. Prefer negotiating
the codec already produced by the source. FFmpeg has standard Opus support but no MLOW codec, so the
last step requires this project's MLOW encoder or another compatible implementation.

## Interoperability evidence

The profile mapping was checked against the locally decompiled Android media split and captured Web
artifacts, without copying proprietary implementation code:

- The Android media registration path distinguishes `use_mlow_codec` from RTP clock selection.
- The MLOW profile registers PT 120 and normally uses a 16 kHz RTP clock; an experiment flag can
  select a 48 kHz clock.
- The standard Opus profile registers PT 111 and uses a 48 kHz RTP clock.
- PT 121 is registered as `mlow-red-1`.
- Audio capability intersection is performed before preaccept/accept, confirming that the signaling
  rate must be preserved rather than discarded.
- Android/Web artifacts contain separate MLOW and standard Opus decoder paths, which confirms that a
  payload-content heuristic is not a valid negotiation mechanism.

The corresponding local research points are in
`wa-apk-re/decompiled-split-v2/04_wa_media.c`, `09_handlers.c`, and the captured WebAssembly/JavaScript
artifacts under the sibling reverse-engineering projects.

## MLOW versus Opus

MLOW keeps the entire PCM codec path pure Rust and matches the newer WhatsApp-specific profile. It
has no external runtime dependency, but it contributes substantially more code and its
analysis-by-synthesis encoder is CPU/allocation heavy.

Standard Opus has a mature tool ecosystem and lets the Rust core omit both codecs when another
component already produces packets. Linking `voip-libopus` is convenient, while an FFmpeg process
keeps the Rust dependency graph smaller at the cost of operational complexity. The `rate=8000`/PT
111 path is supported by the official artifacts, but it should still be canaried against the current
Android/iOS population before becoming the default.

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
