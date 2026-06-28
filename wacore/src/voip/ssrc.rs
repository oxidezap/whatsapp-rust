//! SSRC derivation and participant-LID helpers for E2E HKDF `info`.

use hkdf::Hkdf;
use sha2::Sha256;

/// Participant / stream SSRC: HKDF-SHA256(salt=slot_word LE32, ikm=call_id, info=lid, 4),
/// read back as a little-endian u32.
pub fn derive_wasm_participant_ssrc(call_id: &str, lid: &str, slot_word: u32) -> u32 {
    let hk = Hkdf::<Sha256>::new(Some(&slot_word.to_le_bytes()), call_id.as_bytes());
    let mut okm = [0u8; 4];
    hk.expand(lid.as_bytes(), &mut okm)
        .expect("4 bytes within HKDF limit");
    u32::from_le_bytes(okm)
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
    fn format_participant_id_rules() {
        assert_eq!(format_e2e_srtp_participant_id("12345@lid"), "12345:0@lid");
        assert_eq!(format_e2e_srtp_participant_id("12345:6@lid"), "12345:6@lid");
        assert_eq!(
            format_e2e_srtp_participant_id("12345@s.whatsapp.net"),
            "12345@s.whatsapp.net"
        );
    }
}
