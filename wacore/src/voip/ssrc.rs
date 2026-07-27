//! SSRC derivation and participant-LID helpers for E2E HKDF `info`.

/// Participant / stream SSRC: HKDF-SHA256(salt=slot_word LE32, ikm=call_id, info=lid, 4),
/// read back as a little-endian u32.
pub fn derive_wasm_participant_ssrc(call_id: &str, lid: &str, slot_word: u32) -> u32 {
    let mut okm = [0u8; 4];
    crate::crypto::hkdf_sha256_into(
        call_id.as_bytes(),
        Some(&slot_word.to_le_bytes()),
        lid.as_bytes(),
        &mut okm,
    )
    .expect("4 bytes within HKDF limit");
    u32::from_le_bytes(okm)
}

/// Relay stream slot used to derive the VIDEO SSRC, pinned to the WhatsApp WASM slot grid.
pub const VIDEO_SSRC_SLOT_WORD: u32 = 2;
/// RTC app-data stream slot (reactions and other call controls).
pub const APP_DATA_SSRC_SLOT_WORD: u32 = 6;
/// Hop-by-hop FEC relay stream slots used once more than one remote PID is subscribed.
pub const HBH_FEC_TX_SSRC_SLOT_WORD: u32 = 7;
pub const HBH_FEC_RX_SSRC_SLOT_WORD: u32 = 8;

/// Relay stream order: audio media/FEC/NACK, video media/FEC/NACK,
/// hop-by-hop FEC transmit/receive, app-data.
pub const WASM_RELAY_STREAM_SLOT_WORDS: [u32; 9] = [0, 1, 4, 2, 3, 5, 7, 8, 6];

/// Video-stream SSRC for a participant: same HKDF as audio, video slot word.
pub fn derive_video_participant_ssrc(call_id: &str, lid: &str) -> u32 {
    derive_wasm_participant_ssrc(call_id, lid, VIDEO_SSRC_SLOT_WORD)
}

pub fn derive_wasm_relay_stream_ssrcs(call_id: &str, lid: &str) -> [u32; 9] {
    WASM_RELAY_STREAM_SLOT_WORDS.map(|slot| derive_wasm_participant_ssrc(call_id, lid, slot))
}

/// Device-qualified LID for E2E SRTP HKDF `info`: keep an existing `:N@lid`,
/// bare `@lid` becomes `:0@lid`, everything else passes through. Intentionally a separate protocol
/// surface from SFrame's variant; they coincide today, so both delegate to one helper. Un-shim here
/// if E2E-SRTP ever needs to diverge.
pub fn format_e2e_srtp_participant_id(jid: &str) -> String {
    crate::voip::format_participant_id(jid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip::testkat::kats;

    #[test]
    fn ssrc_matches_kat() {
        let k = kats();
        let call_id = k["inputs"]["callId"].as_str().unwrap();
        let lid = k["inputs"]["peerLid"].as_str().unwrap();
        assert_eq!(
            derive_wasm_participant_ssrc(call_id, lid, 0) as u64,
            k["voip_crypto"]["ssrc_slot0"].as_u64().unwrap()
        );
        assert_eq!(
            derive_wasm_participant_ssrc(call_id, lid, 1) as u64,
            k["voip_crypto"]["ssrc_slot1"].as_u64().unwrap()
        );
    }

    #[test]
    fn video_ssrc_differs_from_audio_ssrc() {
        // Same call/participant must yield distinct per-stream SSRCs, or the
        // engine's demux would collapse audio and video into one stream.
        let audio = derive_wasm_participant_ssrc("CALL-ID-0001", "12345:0@lid", 0);
        let video = derive_video_participant_ssrc("CALL-ID-0001", "12345:0@lid");
        assert_ne!(audio, video);
        assert_eq!(video, 0xf8d7_0484, "WhatsApp WASM video-slot KAT");
    }

    #[test]
    fn relay_stream_bundle_uses_the_protocol_slot_order() {
        assert_eq!(
            WASM_RELAY_STREAM_SLOT_WORDS,
            crate::voip::stun::wasm_stream_slot_words(),
            "the descriptor builder zips these positionally; they must not drift"
        );
        let call_id = "CALL-ID-0001";
        let participant = "12345:0@lid";
        let bundle = derive_wasm_relay_stream_ssrcs(call_id, participant);
        for (index, slot) in WASM_RELAY_STREAM_SLOT_WORDS.iter().copied().enumerate() {
            assert_eq!(
                bundle[index],
                derive_wasm_participant_ssrc(call_id, participant, slot)
            );
        }
        assert_eq!(
            bundle[0],
            derive_wasm_participant_ssrc(call_id, participant, 0)
        );
        assert_eq!(
            bundle[3],
            derive_video_participant_ssrc(call_id, participant)
        );
    }

    #[test]
    fn format_participant_id_rules() {
        assert_eq!(format_e2e_srtp_participant_id("12345@lid"), "12345:0@lid");
        assert_eq!(format_e2e_srtp_participant_id("12345:6@lid"), "12345:6@lid");
        assert_eq!(
            format_e2e_srtp_participant_id("12345@s.whatsapp.net"),
            "12345@s.whatsapp.net"
        );
    }
}
