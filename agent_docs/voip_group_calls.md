# VoIP group calls

How a linked device joins a WhatsApp group voice call as an **invitee**: what the call service
actually sends, which code handles each step, and a complete example that bridges the call audio
to another system.

The behaviour below was observed on live calls: an Android phone creates the group call and a
whatsapp-rust linked device is invited. It complements `voip_audio_codecs.md` (codecs and the
encoded API) and `voip_conformance.md` (oracle gates), which do not cover live group joins.

## Join sequence

```text
phone (creator)            call service (<call_id>@call)            this device
      │  <offer group-jid=…><group_info …/></offer>  (no <relay>)         │
      │ ───────────────────────────────────────────────────────────────▶ │  Event::IncomingCall
      │                                                                  │  accept().start():
      │                       ◀── <preaccept>  (preaccept_group_invite) ──│
      │                       ◀── <accept>     (accept_group_invite)  ────│
      │                       ── <group_update tx=N rekey=1><relay …/> ─▶ │  relay + roster
      │                       ── <group_update tx=N+2> (roster only) ───▶ │
      │                       ◀── <preaccept>/<accept> with media ────────│  start() completes
      │ ◀─────────── <enc_rekey> (our epoch, Signal-encrypted) ──────────│  rekey=1 → we fan out
      │ ◀════════════ RTP/RTCP through the relay (WARP/E2E-SRTP) ════════▶│
      │                       ── <terminate reason=group_call_ended> ───▶ │  call ends
```

1. **Offer.** A group offer carries `group_info` (roster, `transaction-id`) and no `<relay>`, so
   `IncomingCall::media()` is `None`. The handler registers it as a ringing group invitation
   (`insert_ringing_group_if_inactive`) and the `IncomingCall` carries that ringing generation.
2. **Answering the invitation.** The call service sends the relay-bearing `group_update` only
   after this device has pre-accepted and accepted the invitation. `AcceptCall::start()` therefore
   calls `Voip::preaccept_group_invite` and `Voip::accept_group_invite` before it waits for the
   relay (`OFFER_ACK_RELAY_TIMEOUT`). Both stay public for applications that answer early, while
   the invitation still rings.
3. **Relay snapshot.** `group_update` arrives from `<call_id>@call`, not from the creator's device.
   `apply_group_control` accepts that address (`is_call_service_sender`) in addition to the
   creator. The snapshot that carries the relay can arrive *after* a newer roster-only one (live:
   `tx=13` with relay after `tx=15` without). `GroupCallState::apply_update` keeps the newer
   roster and adopts the late relay. A relay already held is never replaced by an older one.
4. **Media.** `start()` builds a group engine from the snapshot, connects the relay, and sends
   its own `<preaccept>`/`<accept>` with audio parameters. The receiver subscription in the STUN
   Allocate lists the pids of the connected remote devices.
5. **Epoch.** If the snapshot has `rekey="1"`, this device generates the group epoch and sends it
   to every connected remote device as `<enc_rekey>`. Inbound RTP stays gated until an epoch is
   installed.
6. **Roster updates.** Later `group_update`s omit the pid of devices whose pid did not change. An
   absent pid means *unchanged*: the committed snapshot carries the known pid over, and media
   receives the committed snapshot rather than the raw update. If absent were treated as removed,
   media would re-send the Allocate with an empty receiver subscription and the relay would stop
   forwarding audio after a handful of packets.
7. **End.** The final `terminate` (`reason="group_call_ended"`) also comes from `<call_id>@call`
   and is accepted the same way. `CallHandle::wait_ended()` then resolves.

## Wire facts the code relies on

| Fact | Where it is handled |
| --- | --- |
| `group_update` and the group `terminate` come from `<call_id>@call` (device 0), with no `participant` attribute | `src/handlers/call.rs`: `is_call_service_sender` |
| The relay's `transaction-id` numbers relay allocations (e.g. `1`), independent of the roster's (e.g. `13`) | `voip_control/group.rs`: `valid_group_snapshot` does not compare them |
| The relay can arrive on an older roster transaction than one already applied | `GroupCallState::apply_update`: adopt the relay, keep the roster |
| The call creator is `pid="0"`; pids are unique but may be zero | `valid_group_snapshot`, `validate_group_media_snapshot` |
| Roster updates leave out unchanged pids | `GroupCallState::apply_update` carries them over; `apply_group_control` hands media the committed snapshot |
| No relay arrives until the invitation is pre-accepted and accepted | `AcceptCall::start()` answers the invitation before waiting |
| Phones send MLOW in group calls too; each participant is its own RTP stream | The PCM path decodes and mixes participants into one 16 kHz stream |

## Example: a bot that joins group calls

An application that answers allowed group calls and exchanges audio with another system, such as
a speech model. Audio is 16 kHz mono `i16`, exactly 960 samples (60 ms) per frame, in both
directions (see `voip_audio_codecs.md`).

```rust
use std::sync::Arc;
use std::time::Duration;
use whatsapp_rust::async_channel;
use whatsapp_rust::prelude::*;
use whatsapp_rust::wacore::types::call::{CallAction, IncomingCall};
use whatsapp_rust::wacore::types::events::{Event, EventHandler};

const FRAME: usize = 960; // 60 ms @ 16 kHz

struct GroupCallBot {
    client: Arc<Client>,
}

impl EventHandler for GroupCallBot {
    fn handle_event(&self, event: Arc<Event>) {
        let Event::IncomingCall(call) = &*event else { return };
        // Replayed offers are already over; group offers carry `incoming.group`.
        if call.offline || call.group.is_none() || !matches!(call.action, CallAction::Offer { .. }) {
            return;
        }
        let client = self.client.clone();
        let call: IncomingCall = (**call).clone();
        tokio::spawn(async move { join(client, call).await });
    }
}

async fn join(client: Arc<Client>, call: IncomingCall) {
    // mic: what we say in the call. speaker: everyone else, already mixed.
    let (mic_tx, mic_rx) = async_channel::bounded::<Vec<i16>>(3);
    let (speaker_tx, speaker_rx) = async_channel::bounded::<Vec<i16>>(16);

    // start() answers the invitation, waits for the relay, and connects media.
    let handle = match client.voip().accept(&call).audio(mic_rx, speaker_tx).start().await {
        Ok(handle) => Arc::new(handle),
        Err(error) => return eprintln!("group join failed: {error}"),
    };

    // The source must deliver one frame per 60 ms; send silence when there is nothing to say.
    let pacer = tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_millis(60));
        loop {
            tick.tick().await;
            let frame = next_frame_to_say().unwrap_or_else(|| vec![0; FRAME]);
            if mic_tx.send(frame).await.is_err() {
                break;
            }
        }
    });
    let listener = tokio::spawn(async move {
        while let Ok(frame) = speaker_rx.recv().await {
            on_frame_heard(&frame); // e.g. stream to a speech-to-speech model
        }
    });

    handle.wait_ended().await;
    pacer.abort();
    listener.abort();
    eprintln!("group call ended: {:?}", handle.media_stats());
}

fn next_frame_to_say() -> Option<Vec<i16>> {
    None // pull from your model's output buffer
}

fn on_frame_heard(_frame: &[i16]) {}

// Registration, after building the bot:
// let client = bot.client();
// let _subscription = client.subscribe_handler(Arc::new(GroupCallBot { client: client.clone() }));
```

Notes for applications:

- **Policy.** Check `call_creator` / `caller_pn` before joining. To leave a call to the account's
  other devices, do nothing rather than `Voip::reject`. A reject without a reason is an explicit
  decline by the user (`CallAction::Reject`), not "this device is busy".
- **One stream.** In a group call the speaker channel already carries every participant mixed.
  `EncodedAudioFrame::sender` / `device` / `pid` identify streams only on the encoded path.
- **Health.** `CallHandle::media_stats()` is the quickest live check. `rtp_received` should keep
  growing while someone talks. If it stalls early while the call stays up, the receiver
  subscription (step 6) is the first thing to inspect.

## Troubleshooting

| Symptom | Likely cause |
| --- | --- |
| `start()` fails with `call service request timed out` | The invitation was not answered before the relay wait (step 2), or the relay snapshot was rejected (steps 3 and 6) |
| `rejected group snapshot from non-creator sender` | A `group_update` from `<call_id>@call` reached a build without `is_call_service_sender` |
| `rejected invalid group snapshot` | A snapshot rule rejected it. Typical cases: a relay `transaction-id` compared with the roster's, or a pid of 0 |
| The roster lists the device as connected, but `rtp_received` stops after a few packets | The receiver subscription was re-sent without the remote pids (step 6) |
| Every frame is silent and `rtp_received` grows | The epoch is not installed, or the codec does not match the stream (`voip_audio_codecs.md`) |
