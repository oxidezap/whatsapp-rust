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
RTP profile, and it never discriminates MLOW from Opus: every profile above signals 16000.

PT 121 is MLOW's `mlow-red-1` redundancy. The shipped client also registers PT 122 as a second
`mlow-1` when `enable_mlow_separate_pt_rx` is on. Neither `_rx` prop is gated by the peer's
capability, so a client always registers what it can receive; of the `_tx` pair only
`enable_mlow_separate_pt_tx` (index 38) is gated, while `enable_red_pt_support_tx` is not, and index
36 gates `enable_mlow_red` rather than a `_tx` prop. We announce 36 and not 38, so a peer with parity
never sends us PT 122.

## Negotiation

**PT 120 carries either codec.** The shipped client registers the payload type once at stream setup
with `name = use_mlow_codec ? "mlow-1" : "opus"`, and its decoder factory dispatches on that name
alone. The payload type does not tell you what the bytes are; the negotiation does. This is the
whole of issue #1105: a peer outside the MLow rollout sends standard Opus on PT 120, and reading it
as MLow silences the call in both directions.

**Capability index 31 is `use_mlow_codec_v1`**, and the gate is a mutual AND. Each side announces
what it can do, and the client's `reset_voip_params_if_no_capability` walks every participant: if
any of them fails to announce the index, the parameter drops to its safe default. One boolean drives
both the encoder and the receive-side registration; there is no per-direction pair.

Read a peer blob with `wacore::stanza::call::capability_bit`. It returns three states, and two of
them are treated in **opposite** directions by the client:

| peer `<capability>` | `CapabilityBit` | effect |
| --- | --- | --- |
| carries index 31 | `Set` | MLow survives if we asked for it too |
| valid, without index 31 | `Clear` | MLow drops for both directions |
| **absent** | `Unknown` | the participant is skipped; **nothing resets** |
| **present but unreadable** (`ver` missing, unparseable, or below the index's version) | `Clear` | falls back to a capability that answers false for everything, so **all of it resets** |

Our own video `<accept>` omits the blob, so `Unknown` is not hypothetical.

Where the decision is applied differs by role, and neither needs a mutable `AudioFormat`:

- **callee**: the peer's capability is in the `<offer>`, before the engine exists, so the call simply
  starts on the right codec.
- **caller**: it arrives with `<preaccept>`/`<accept>`, after setup. It rides the channel that
  already carries the answering device's LID (`PeerAnswer`), because both have to be applied before
  the first inbound packet.

**Group calls are not covered.** The client's mutual AND walks every participant; ours reads one
peer, because a 1:1 call has one. A group call with a participant outside the rollout stays on MLOW,
and `GroupCallDevice::capability()` is public so the group path can read it when someone does that
work. Stated here rather than left to be discovered.

Mid-call the codec is swapped with `CallEngine::switch_audio_codec`, which accepts **only** the
MLOW/Opus pair at 16 kHz, 60 ms, PT 120. Those two `AudioFormat`s agree on payload type, clock rate,
timestamp step, sample rate, channels and samples per frame, so the swap changes no RTP header byte
and is not a renegotiation. Anything else is refused: it would move the clock under a live stream.

Native Opus also sends this minimal uncompressed settings overlay in the answer:

```json
{
  "encode": { "use_mlow_codec_v1": "false" },
  "options": { "enable_48khz_rtp_clock": "false" }
}
```

`enable_48khz_rtp_clock=true` independently selects PT 111 and the 48 kHz RTP clock; the production
default is PT 120 at 16 kHz.

Incoming calls reject a locally selected rate absent from the offer. A later incompatible
preaccept/accept emits `CallEvent::AudioFormatMismatch` and terminates the call.

The PT120 native-Opus path was verified against Android/Web implementations and with a live
full-duplex Android call. MLOW remains the default we ask for, subject to the AND above.

### Content is a corroborator, never the source

The shipped client does no content sniffing at all, and neither do we for what we **send**. But the
negotiation has two holes on the receive side: the capability can be absent, and on the caller side
it can lose the race with the first media packet. `voip::codec_probe` closes them without guessing.

It never reads the payload alone. It asks whether two independent statements by the same peer agree:
the duration its Opus header declares (`voip::opus_packet::opus_packet_shape`, a structural RFC 6716
read with no libopus, so it works on wasm32 and ESP32) and the step its RTP timestamps advance by.
Three consecutive agreements switch the decoder, once.

Two conditions have to hold together, and the second is easy to mistake for redundant:

1. **The peer must be pacing at the cadence the call negotiated.** Without this the agreement is not
   evidence at all. MLow reads TOC bits 4:3 as `{10, 20, 60, 120}` ms and an Opus SILK TOC reads the
   *same bits* as `{10, 20, 40, 60}`, so the two grammars agree by construction at 10 and 20 ms — and
   those are exactly the durations the operating-point guard refuses, which means they reach the
   probe precisely because nothing decoded. A genuine MLow stream at either would otherwise be
   promoted to Opus and the call would break in both directions, permanently.
2. **At the negotiated 960-sample step the collision cannot happen.** The TOCs the decoder accepts
   are the 24 bytes with bit 7 clear, bit 5 clear, bit 2 clear and bits 4:3 in `{01, 10, 11}`; each
   reads as an Opus config of 1, 2, 9 or 10, every reachable total is a multiple of 320 or 640
   samples, and 960 is neither.

Tests pin both: one walks the colliding durations at their own cadences and one walks the 60 ms TOCs
at every body length. Removing the cadence gate as "redundant" is what the first one exists to catch.

A switch driven by content emits `CallEvent::AudioCodecSwitched { source: Content }`. That is worth
acting on: it means our model of the peer is wrong, not just that one call was rescued.

## Observability

Every discard on the receive path increments exactly one named counter in
`wacore::voip::CallMediaStats`, readable through `CallHandle::media_stats()`. Two events close the
loop:

- `CallEvent::AudioSilent` when audio RTP keeps arriving and none of it becomes sound, with a
  `dominant_reason` naming the most specific explanation the counters support;
- `CallEvent::AudioReceptionStalled` when none arrives at all.

They are deliberately distinct: one is a codec problem and the other is transport, and conflating
them is how #1105 was mis-triaged for months. `voip::tap::PacketTap` is public: decorating a
`RelayTransportFactory` with `TappedFactory` puts every relay datagram in both directions through
your sink. The runtime does not yet expose a factory injection point for a live call, so that is
reachable from a shell building its own transport rather than from a `CallHandle`.

A build with no decoder for the negotiated codec does not pretend. It reports `AudioSilent` with
`NoDecoderForNegotiatedCodec`, which is the one silence reason a consumer can act on.

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
