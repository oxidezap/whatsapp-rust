# VoIP audio codecs

The media core owns signaling, RTP timing, SRTP/WARP, relay transport, and receive statistics.
Applications may use the built-in PCM/MLOW path or exchange complete codec packets through
`encoded_audio`.

```text
PCM source ── MLOW adapter ──┐
                             ├── RTP + SRTP/WARP ── WhatsApp relay
encoded source/sink ─────────┘
```

The encoded boundary does not transcode. `AudioFormat` fixes the codec profile, payload type, RTP
clock, and 60 ms packet cadence for the call.

## Profiles

| Format | Profile | PCM | RTP clock / step | PT |
| --- | --- | ---: | ---: | ---: |
| `MLOW_16KHZ_60MS` | MLOW | 16 kHz mono | 16 kHz / 960 | 120 |
| `OPUS_MLOW_16KHZ_60MS` | Opus CELT in MLOW | 16 kHz mono | 16 kHz / 960 | 120 |
| `OPUS_16KHZ_60MS` | Native Opus | 16 kHz mono | 16 kHz / 960 | 120 |
| `OPUS_RFC7587_16KHZ_60MS` | Native Opus | 16 kHz mono | 48 kHz / 2880 | 111 |
| `OPUS_RFC7587_48KHZ_60MS` | Native Opus | 48 kHz mono | 48 kHz / 2880 | 111 |

All current profiles signal `<audio enc="opus" rate="16000">`. The rate alone does not select the
RTP profile.

## Negotiation

MLOW capability index 31 controls the codec sent by the peer. Native Opus clears that bit and sends
this minimal uncompressed settings overlay in the answer:

```json
{
  "encode": { "use_mlow_codec_v1": "false" },
  "options": { "enable_48khz_rtp_clock": "false" }
}
```

The capability selects the peer's encoder; `use_mlow_codec_v1` selects its decoder for the reverse
direction. Both are required for full-duplex native Opus. `enable_48khz_rtp_clock=true` independently
selects PT 111 and the 48 kHz RTP clock; the production default is PT 120 at 16 kHz.

Incoming calls reject a locally selected rate absent from the offer. A later incompatible
preaccept/accept emits `CallEvent::AudioFormatMismatch` and terminates the call. Codec detection never
overrides the negotiated RTP profile.

The PT120 native-Opus path was verified against Android/Web implementations and with a live
full-duplex Android call. MLOW remains the compatibility default.

## Cargo features

| Feature | Contents |
| --- | --- |
| `voip-encoded` | Native relay/runtime and encoded I/O; no audio codec |
| `voip-mlow` | Runtime plus the pure-Rust PCM/MLOW adapter |
| `voip-libopus` | Encoded runtime plus the optional libopus adapter |
| `voip` | Compatibility aggregate: MLOW + libopus |
| `wacore/voip` | Runtime-agnostic media engine and encoded I/O |
| `wacore/voip-mlow` | Core media engine plus MLOW |

An application that already produces raw Opus packets needs only `voip-encoded`.

## Encoded API

The source sends one complete raw codec packet per `Bytes`, paced every 60 ms. The sink receives the
decrypted packet and its RTP metadata.

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
```

Container data is not accepted. Ogg pages from `ffmpeg -f opus` must be demuxed; FFmpeg RTP output
must have its RTP header removed because this core creates and protects the WhatsApp RTP packet.
libavcodec integrations can pass each raw `AVPacket` directly.

`OPUS_MLOW_16KHZ_60MS` requires CELT-only Opus. Run each packet through
`packetize_opus_for_mlow`; the reverse path uses `depacketize_opus_from_mlow`. These helpers rewrite
only the packet header. SILK/Hybrid Opus requires decode/re-encode, and arbitrary Opus-to-MLOW
conversion requires full transcoding.

The libopus adapter uses the observed 16 kHz mono, 60 ms, 24 kbps, complexity-5, DTX configuration.
It maps short Opus DTX packets to MLOW SID when the escape profile is selected.

## Tradeoffs

- MLOW is pure Rust and broadly compatible, but its analysis-by-synthesis encoder costs more CPU.
- Native Opus avoids MLOW transcoding and works with FFmpeg/libopus or another packet producer.
- The MLOW Opus escape is asymmetric: a peer may still send proprietary MLOW, so an Opus-only
  application needs an external MLOW decoder for that fallback.

The MLOW hot path reuses VAD, history, range-coder, pitch, and output buffers. The encoded path
preserves `Bytes` ownership until RTP/SRTP framing.

## CLI validation

```bash
WA_AUDIO_CODEC=mlow cargo run -p whatsapp-rust-voip-cli --release -- listen accept
WA_AUDIO_CODEC=opus cargo run -p whatsapp-rust-voip-cli --release -- listen accept

WA_AUDIO_CODEC=mlow cargo run -p whatsapp-rust-voip-cli --release \
  --no-default-features --features voip-mlow -- listen accept
WA_AUDIO_CODEC=opus cargo run -p whatsapp-rust-voip-cli --release \
  --no-default-features --features voip-opus -- listen accept
```

`WA_AUDIO_CODEC=opus` selects native PT120 Opus. `WA_AUDIO_PROFILE=pt111` selects the 48 kHz RTP
variant; `WA_AUDIO_PROFILE=mlow` selects the CELT escape.

## Video

Video already accepts external H.264 Annex-B access units. It is not wire-codec-agnostic: signaling,
packetization, and PT 97 currently target H.264.
