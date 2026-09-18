//! Small pure helpers shared by the control plane and the media engine.
//!
//! These were methods of the `voip` module, which made the registry depend on `crate::voip` for a
//! KDF shim and a JID formatter. None of them touches crypto primitives beyond `crypto::hkdf_sha256`
//! or names an engine type, so they live with the contract; `crate::voip` re-exports them.

/// HKDF-SHA256 (extract with `salt`, expand with `info`): the one KDF shape all of
/// WhatsApp's VoIP key derivations reduce to.
#[cfg(feature = "voip")]
pub(crate) fn hkdf_sha256(salt: &[u8], ikm: &[u8], info: &[u8], len: usize) -> Vec<u8> {
    debug_assert!(len <= 255 * 32, "HKDF-SHA256 max output is 8160 bytes");
    crate::crypto::hkdf_sha256(ikm, len, Some(salt), info).expect("HKDF length within bounds")
}

/// Device-qualified participant id used as HKDF `info` for both E2E-SRTP and SFrame: strip the
/// resource, keep an existing `:N@lid` device suffix, give bare `@lid` an implicit `:0`, and pass
/// everything else through unchanged.
pub(crate) fn format_participant_id(jid: &str) -> String {
    let bare = jid.split('/').next().unwrap_or(jid).trim();
    let Some(at) = bare.rfind('@') else {
        return bare.to_string();
    };
    if at == 0 {
        return bare.to_string();
    }
    let user = &bare[..at];
    let domain = &bare[at + 1..];
    if domain == "lid" && !user.contains(':') {
        return format!("{user}:0@{domain}");
    }
    bare.to_string()
}

/// LEB128 varint append (`SFrame` header + DC STUN attributes use the same encoding).
#[cfg(feature = "voip")]
pub(crate) fn encode_varint(out: &mut Vec<u8>, value: u64) {
    let mut v = value;
    while v > 0x7f {
        out.push(((v & 0x7f) | 0x80) as u8);
        v >>= 7;
    }
    out.push((v & 0xff) as u8);
}
