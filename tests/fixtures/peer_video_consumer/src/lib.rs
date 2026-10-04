//! Independent consumer of the canonical peer video event, without an engine dependency.
//!
//! The positive control uses the same public path as the removed-variant check:
//!
//! ```
//! use whatsapp_rust::voip::CallEvent;
//! fn current(event: &CallEvent) {
//!     if let CallEvent::PeerVideoStateChanged { source, call_creator, state,
//!         orientation, upgrade_token, .. } = event
//!     {
//!         let _ = (source, call_creator, state, orientation, upgrade_token);
//!     }
//! }
//! ```
//!
//! ```compile_fail,E0599
//! use whatsapp_rust::voip::CallEvent;
//! fn legacy(event: &CallEvent) {
//!     if let CallEvent::VideoStateChanged { .. } = event {}
//! }
//! ```
//!
//! Stable rustdoc accepts any compilation failure, so also check the opt-in
//! `removed-video-state` binary's diagnostic: it must report E0599 for the
//! missing `VideoStateChanged` variant, not a feature/import/privacy error.

use whatsapp_rust::Jid;
use whatsapp_rust::voip::{CallEvent, VideoState, VideoUpgradeToken};

pub fn peer_video(
    source: Jid,
    call_creator: Jid,
    state: VideoState,
    orientation: Option<u8>,
    upgrade_token: Option<VideoUpgradeToken>,
) -> CallEvent {
    CallEvent::PeerVideoStateChanged {
        source,
        call_creator,
        state,
        orientation,
        upgrade_token,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumer_can_construct_and_read_identity_and_video_payload() {
        let source: Jid = "222222222222222:2@lid".parse().unwrap();
        let creator: Jid = "111111111111111@lid".parse().unwrap();
        let event = peer_video(
            source.clone(),
            creator.clone(),
            VideoState::Stopped,
            Some(3),
            None,
        );
        match event {
            CallEvent::PeerVideoStateChanged {
                source: actual_source,
                call_creator,
                state,
                orientation,
                upgrade_token,
                ..
            } => {
                assert_eq!(actual_source, source);
                assert_eq!(call_creator, creator);
                assert_eq!(state, VideoState::Stopped);
                assert_eq!(orientation, Some(3));
                assert_eq!(upgrade_token, None);
            }
            _ => panic!("unexpected event"),
        }
    }
}
