//! Poll vote encryption/decryption.
//!
//! Thin wrapper over [`secret_enc_addon`] specialised for the
//! `PollVoteMessage` proto and the `"Poll Vote"` use-case.

use anyhow::{Result, anyhow};
use sha2::{Digest, Sha256};

use crate::secret_enc_addon::{
    AddonContext, ModificationType, build_aad, decrypt_addon, encrypt_addon,
};

const GCM_IV_SIZE: usize = 12;
const GCM_TAG_SIZE: usize = 16;

fn poll_vote_addon_ctx<'a>(
    stanza_id: &'a str,
    poll_creator_jid: &'a str,
    voter_jid: &'a str,
) -> AddonContext<'a> {
    AddonContext {
        stanza_id,
        parent_msg_original_sender: poll_creator_jid,
        modification_sender: voter_jid,
        modification_type: ModificationType::PollVote,
    }
}

/// Votes reference options by SHA-256 hash, not by name.
pub fn compute_option_hash(option_name: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(option_name.as_bytes());
    hasher.finalize().into()
}

/// HKDF-SHA256: info = stanzaId || pollCreator || voter || "Poll Vote", no salt.
///
/// Kept as a public API for backwards compatibility; new code should call
/// [`secret_enc_addon::derive_use_case_secret`] with `ModificationType::PollVote`.
pub fn derive_vote_encryption_key(
    message_secret: &[u8],
    stanza_id: &str,
    poll_creator_jid: &str,
    voter_jid: &str,
) -> Result<[u8; 32]> {
    crate::secret_enc_addon::derive_use_case_secret(
        message_secret,
        &poll_vote_addon_ctx(stanza_id, poll_creator_jid, voter_jid),
    )
}

/// Encrypt a poll vote with a pre-derived 32-byte key, symmetric to
/// [`decrypt_poll_vote`]. Returns `(payload_with_tag, iv)`.
///
/// Kept for callers that built their own key via [`derive_vote_encryption_key`].
/// New code should prefer [`encrypt_poll_vote_with_secret`], which derives
/// the key in a single step from the parent poll's `messageSecret`.
pub fn encrypt_poll_vote(
    selected_option_hashes: &[Vec<u8>],
    encryption_key: &[u8; 32],
    stanza_id: &str,
    voter_jid: &str,
) -> Result<(Vec<u8>, [u8; GCM_IV_SIZE])> {
    use crate::libsignal::crypto::aes_256_gcm_encrypt;
    use rand::Rng;

    let plaintext = encode_selected_options(selected_option_hashes);

    let mut iv = [0u8; GCM_IV_SIZE];
    rand::make_rng::<rand::rngs::StdRng>().fill_bytes(&mut iv);

    // poll_creator_jid is not part of the AAD; supply an empty placeholder.
    let aad = build_aad(&poll_vote_addon_ctx(stanza_id, "", voter_jid));

    let mut payload = Vec::with_capacity(plaintext.len() + GCM_TAG_SIZE);
    aes_256_gcm_encrypt(encryption_key, &iv, &aad, &plaintext, &mut payload)
        .map_err(|e| anyhow!("AES-GCM encrypt failed: {e}"))?;

    Ok((payload, iv))
}

/// Encrypt a poll vote given the parent poll's `messageSecret`. Returns
/// `(payload_with_tag, iv)`.
pub fn encrypt_poll_vote_with_secret(
    selected_option_hashes: &[Vec<u8>],
    message_secret: &[u8],
    stanza_id: &str,
    poll_creator_jid: &str,
    voter_jid: &str,
) -> Result<(Vec<u8>, [u8; GCM_IV_SIZE])> {
    let plaintext = encode_selected_options(selected_option_hashes);

    encrypt_addon(
        &plaintext,
        message_secret,
        &poll_vote_addon_ctx(stanza_id, poll_creator_jid, voter_jid),
    )
}

fn encode_selected_options(selected_option_hashes: &[Vec<u8>]) -> Vec<u8> {
    use buffa::encoding::{Tag, WireType};

    let encoded_len = selected_option_hashes
        .iter()
        .map(|hash| 1 + buffa::types::bytes_encoded_len(hash))
        .sum();
    let mut plaintext = Vec::with_capacity(encoded_len);
    for hash in selected_option_hashes {
        Tag::new(1, WireType::LengthDelimited).encode(&mut plaintext);
        buffa::types::encode_bytes(hash, &mut plaintext);
    }
    plaintext
}

/// Returns the selected option hashes (each 32 bytes).
///
/// Kept for backwards compatibility with callers that pre-derived the key.
/// New code should call [`decrypt_poll_vote_with_secret`].
pub fn decrypt_poll_vote(
    enc_payload: &[u8],
    iv: &[u8],
    encryption_key: &[u8; 32],
    stanza_id: &str,
    voter_jid: &str,
) -> Result<Vec<Vec<u8>>> {
    use crate::libsignal::crypto::aes_256_gcm_decrypt;

    let nonce: &[u8; GCM_IV_SIZE] = iv
        .try_into()
        .map_err(|_| anyhow!("Invalid IV size: expected {GCM_IV_SIZE}, got {}", iv.len()))?;
    if enc_payload.len() < GCM_TAG_SIZE {
        return Err(anyhow!(
            "Encrypted payload too short: need at least {GCM_TAG_SIZE} bytes for tag, got {}",
            enc_payload.len()
        ));
    }

    // poll_creator_jid is not part of the AAD; supply an empty placeholder.
    let aad = build_aad(&poll_vote_addon_ctx(stanza_id, "", voter_jid));

    let mut plaintext = Vec::with_capacity(enc_payload.len().saturating_sub(GCM_TAG_SIZE));
    aes_256_gcm_decrypt(encryption_key, nonce, &aad, enc_payload, &mut plaintext)
        .map_err(|_| anyhow!("Poll vote GCM tag verification failed"))?;

    decode_selected_options(&plaintext)
}

/// Creator + voter JIDs (non-AD) that key the poll-vote HKDF and AAD.
#[derive(Debug, Clone, Copy)]
pub struct PollVoteAddressing<'a> {
    pub poll_creator_jid: &'a str,
    pub voter_jid: &'a str,
}

/// The encrypted vote payload and its GCM IV, paired so the two same-typed
/// byte slices can't be transposed at a call site.
#[derive(Debug, Clone, Copy)]
pub struct PollVoteCiphertext<'a> {
    pub enc_payload: &'a [u8],
    pub enc_iv: &'a [u8],
}

/// Decrypt a poll vote, retrying once under the alternate addressing.
///
/// The creator and voter JIDs key the derivation, so a vote authored under LID
/// only opens under LID. As contacts migrate to LID a vote can arrive in a
/// different namespace than the parent poll was learned in, so `fallback`
/// retries with both JIDs swapped together (never mixed), matching WA Web
/// `WAWebAddonEncryption.decryptAddOn`.
pub fn decrypt_poll_vote_with_fallback(
    ciphertext: PollVoteCiphertext<'_>,
    message_secret: &[u8],
    stanza_id: &str,
    primary: PollVoteAddressing<'_>,
    fallback: Option<PollVoteAddressing<'_>>,
) -> Result<Vec<Vec<u8>>> {
    match decrypt_poll_vote_with_secret(
        ciphertext,
        message_secret,
        stanza_id,
        primary.poll_creator_jid,
        primary.voter_jid,
    ) {
        Ok(v) => Ok(v),
        Err(primary_err) => match fallback {
            Some(fb) => decrypt_poll_vote_with_secret(
                ciphertext,
                message_secret,
                stanza_id,
                fb.poll_creator_jid,
                fb.voter_jid,
            )
            .map_err(|fb_err| {
                anyhow!("poll vote decrypt failed: primary={primary_err}; fallback={fb_err}")
            }),
            None => Err(primary_err),
        },
    }
}

/// Decrypts a poll vote and visits borrowed selected option hashes.
///
/// This avoids copying each selected hash when the caller can consume it before
/// the decrypted plaintext buffer is dropped.
pub fn visit_decrypted_poll_vote_with_fallback<F>(
    enc_payload: &[u8],
    iv: &[u8],
    message_secret: &[u8],
    stanza_id: &str,
    primary: PollVoteAddressing<'_>,
    fallback: Option<PollVoteAddressing<'_>>,
    mut visit: F,
) -> Result<()>
where
    F: FnMut(&[u8]),
{
    match visit_poll_vote_with_secret(
        enc_payload,
        iv,
        message_secret,
        stanza_id,
        primary.poll_creator_jid,
        primary.voter_jid,
        &mut visit,
    ) {
        Ok(()) => Ok(()),
        Err(primary_err) => match fallback {
            Some(fb) => visit_poll_vote_with_secret(
                enc_payload,
                iv,
                message_secret,
                stanza_id,
                fb.poll_creator_jid,
                fb.voter_jid,
                &mut visit,
            )
            .map_err(|fb_err| {
                anyhow!("poll vote decrypt failed: primary={primary_err}; fallback={fb_err}")
            }),
            None => Err(primary_err),
        },
    }
}

/// Decrypt a poll vote given the poll's `messageSecret` directly. Preferred
/// over the legacy two-step path that splits derive+decrypt.
pub fn decrypt_poll_vote_with_secret(
    ciphertext: PollVoteCiphertext<'_>,
    message_secret: &[u8],
    stanza_id: &str,
    poll_creator_jid: &str,
    voter_jid: &str,
) -> Result<Vec<Vec<u8>>> {
    let plaintext = decrypt_poll_vote_payload_with_secret(
        ciphertext,
        message_secret,
        stanza_id,
        poll_creator_jid,
        voter_jid,
    )?;
    decode_selected_options(&plaintext)
}

/// Decrypt a poll vote and return its encoded `PollVoteMessage` payload.
///
/// This is useful when the caller owns protobuf decoding or needs to preserve
/// fields unknown to this version of the core.
pub fn decrypt_poll_vote_payload_with_secret(
    ciphertext: PollVoteCiphertext<'_>,
    message_secret: &[u8],
    stanza_id: &str,
    poll_creator_jid: &str,
    voter_jid: &str,
) -> Result<Vec<u8>> {
    decrypt_addon(
        ciphertext.enc_payload,
        ciphertext.enc_iv,
        message_secret,
        &poll_vote_addon_ctx(stanza_id, poll_creator_jid, voter_jid),
    )
}

fn visit_poll_vote_with_secret<F>(
    enc_payload: &[u8],
    iv: &[u8],
    message_secret: &[u8],
    stanza_id: &str,
    poll_creator_jid: &str,
    voter_jid: &str,
    visit: &mut F,
) -> Result<()>
where
    F: FnMut(&[u8]),
{
    let plaintext = decrypt_poll_vote_payload_with_secret(
        PollVoteCiphertext {
            enc_payload,
            enc_iv: iv,
        },
        message_secret,
        stanza_id,
        poll_creator_jid,
        voter_jid,
    )?;
    // Validate the entire plaintext BEFORE emitting anything: `visit` has
    // observable side effects in the caller and the caller has a fallback path
    // that re-runs this on failure. Without the dry first pass, a parse error
    // partway through would leave the caller with partial emissions plus a
    // fallback re-emit (duplicated/corrupt votes). The first scan makes this
    // visit all-or-nothing; the plaintext is small so the double pass is cheap.
    scan_selected_options(&plaintext, |_| {})?;
    scan_selected_options(&plaintext, visit)
}

fn decode_selected_options(plaintext: &[u8]) -> Result<Vec<Vec<u8>>> {
    let mut selected_options = Vec::new();
    scan_selected_options(plaintext, |selected| {
        selected_options.push(selected.to_vec());
    })?;
    Ok(selected_options)
}

fn scan_selected_options<'a, F>(plaintext: &'a [u8], mut visit: F) -> Result<()>
where
    F: FnMut(&'a [u8]),
{
    use buffa::encoding::{Tag, WireType, skip_field_depth};

    let mut cur = plaintext;
    while !cur.is_empty() {
        let tag = Tag::decode(&mut cur)?;
        match tag.field_number() {
            1 => {
                if tag.wire_type() != WireType::LengthDelimited {
                    return Err(buffa::DecodeError::WireTypeMismatch {
                        field_number: 1,
                        expected: WireType::LengthDelimited as u8,
                        actual: tag.wire_type() as u8,
                    }
                    .into());
                }
                visit(buffa::types::borrow_bytes(&mut cur)?);
            }
            _ => skip_field_depth(tag, &mut cur, buffa::RECURSION_LIMIT)?,
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    #[test]
    fn option_hash_deterministic() {
        let h1 = compute_option_hash("Option A");
        let h2 = compute_option_hash("Option A");
        let h3 = compute_option_hash("Option B");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
        assert_eq!(h1.len(), 32);
    }

    #[test]
    fn vote_encrypt_decrypt_roundtrip() {
        use buffa::Message;

        let secret = [0xCDu8; 32];
        let stanza_id = "3EB0ABCD1234";
        let creator = "creator@s.whatsapp.net";
        let voter = "voter@s.whatsapp.net";

        let hashes = vec![
            compute_option_hash("Yes").to_vec(),
            compute_option_hash("No").to_vec(),
        ];

        let (enc, iv) =
            encrypt_poll_vote_with_secret(&hashes, &secret, stanza_id, creator, voter).unwrap();
        let out = decrypt_poll_vote_with_secret(
            PollVoteCiphertext {
                enc_payload: &enc,
                enc_iv: &iv,
            },
            &secret,
            stanza_id,
            creator,
            voter,
        )
        .unwrap();
        assert_eq!(out, hashes);

        let plaintext = decrypt_poll_vote_payload_with_secret(
            PollVoteCiphertext {
                enc_payload: &enc,
                enc_iv: &iv,
            },
            &secret,
            stanza_id,
            creator,
            voter,
        )
        .unwrap();
        let vote_message = waproto::whatsapp::message::PollVoteMessage {
            selected_options: hashes,
        };
        assert_eq!(plaintext, vote_message.encode_to_vec());
    }

    #[test]
    fn selected_option_encoding_matches_message_encoding() {
        use buffa::Message;

        let hashes = vec![
            compute_option_hash("Yes").to_vec(),
            compute_option_hash("No").to_vec(),
        ];
        let vote_msg = waproto::whatsapp::message::PollVoteMessage {
            selected_options: hashes.clone(),
        };

        assert_eq!(encode_selected_options(&hashes), vote_msg.encode_to_vec());
    }

    #[test]
    fn legacy_decrypt_path_still_works() {
        let secret = [0xCDu8; 32];
        let stanza_id = "3EB0ABCD1234";
        let creator = "creator@s.whatsapp.net";
        let voter = "voter@s.whatsapp.net";

        let hashes = vec![compute_option_hash("Yes").to_vec()];
        let (enc, iv) =
            encrypt_poll_vote_with_secret(&hashes, &secret, stanza_id, creator, voter).unwrap();

        let key = derive_vote_encryption_key(&secret, stanza_id, creator, voter).unwrap();
        let out = decrypt_poll_vote(&enc, &iv, &key, stanza_id, voter).unwrap();
        assert_eq!(out, hashes);
    }

    #[test]
    fn wrong_voter_fails() {
        let secret = [0xEFu8; 32];
        let (enc, iv) = encrypt_poll_vote_with_secret(
            &[compute_option_hash("Yes").to_vec()],
            &secret,
            "id",
            "c@s.whatsapp.net",
            "v@s.whatsapp.net",
        )
        .unwrap();

        assert!(
            decrypt_poll_vote_with_secret(
                PollVoteCiphertext {
                    enc_payload: &enc,
                    enc_iv: &iv,
                },
                &secret,
                "id",
                "c@s.whatsapp.net",
                "wrong@s.whatsapp.net"
            )
            .is_err()
        );
    }

    #[test]
    fn fallback_recovers_across_addressing() {
        // Vote encrypted under the PN pair (creator + voter both PN)...
        let secret = [0x33u8; 32];
        let stanza_id = "3EB0FALLBACK";
        let creator_pn = "5511999999999@s.whatsapp.net";
        let voter_pn = "5511888888888@s.whatsapp.net";
        let (enc, iv) = encrypt_poll_vote_with_secret(
            &[compute_option_hash("Yes").to_vec()],
            &secret,
            stanza_id,
            creator_pn,
            voter_pn,
        )
        .unwrap();

        // ...consumer first guesses the LID pair (primary), which fails, then
        // recovers via the PN fallback. Mirrors WA Web's homogeneous retry.
        let creator_lid = "111111111111111@lid";
        let voter_lid = "222222222222222@lid";
        let out = decrypt_poll_vote_with_fallback(
            PollVoteCiphertext {
                enc_payload: &enc,
                enc_iv: &iv,
            },
            &secret,
            stanza_id,
            PollVoteAddressing {
                poll_creator_jid: creator_lid,
                voter_jid: voter_lid,
            },
            Some(PollVoteAddressing {
                poll_creator_jid: creator_pn,
                voter_jid: voter_pn,
            }),
        )
        .unwrap();
        assert_eq!(out, vec![compute_option_hash("Yes").to_vec()]);
    }

    #[test]
    fn visit_fallback_yields_selected_hashes() {
        let secret = [0x34u8; 32];
        let stanza_id = "3EB0VISIT";
        let creator_pn = "5511999999999@s.whatsapp.net";
        let voter_pn = "5511888888888@s.whatsapp.net";
        let expected = compute_option_hash("Yes");
        let (enc, iv) = encrypt_poll_vote_with_secret(
            &[expected.to_vec()],
            &secret,
            stanza_id,
            creator_pn,
            voter_pn,
        )
        .unwrap();

        let mut visited = Vec::new();
        visit_decrypted_poll_vote_with_fallback(
            &enc,
            &iv,
            &secret,
            stanza_id,
            PollVoteAddressing {
                poll_creator_jid: "111111111111111@lid",
                voter_jid: "222222222222222@lid",
            },
            Some(PollVoteAddressing {
                poll_creator_jid: creator_pn,
                voter_jid: voter_pn,
            }),
            |hash| visited.push(<[u8; 32]>::try_from(hash).unwrap()),
        )
        .unwrap();

        assert_eq!(visited, vec![expected]);
    }

    #[test]
    fn fallback_primary_succeeds_without_using_fallback() {
        let secret = [0x44u8; 32];
        let stanza_id = "id";
        let creator = "c@s.whatsapp.net";
        let voter = "v@s.whatsapp.net";
        let (enc, iv) = encrypt_poll_vote_with_secret(
            &[compute_option_hash("A").to_vec()],
            &secret,
            stanza_id,
            creator,
            voter,
        )
        .unwrap();

        // Primary matches; a deliberately-wrong fallback must never be reached.
        let out = decrypt_poll_vote_with_fallback(
            PollVoteCiphertext {
                enc_payload: &enc,
                enc_iv: &iv,
            },
            &secret,
            stanza_id,
            PollVoteAddressing {
                poll_creator_jid: creator,
                voter_jid: voter,
            },
            Some(PollVoteAddressing {
                poll_creator_jid: "wrong@lid",
                voter_jid: "wrong@lid",
            }),
        )
        .unwrap();
        assert_eq!(out, vec![compute_option_hash("A").to_vec()]);
    }

    #[test]
    fn fallback_combined_error_when_both_fail() {
        let secret = [0x55u8; 32];
        let stanza_id = "id";
        let (enc, iv) = encrypt_poll_vote_with_secret(
            &[compute_option_hash("A").to_vec()],
            &secret,
            stanza_id,
            "c@s.whatsapp.net",
            "v@s.whatsapp.net",
        )
        .unwrap();

        let err = decrypt_poll_vote_with_fallback(
            PollVoteCiphertext {
                enc_payload: &enc,
                enc_iv: &iv,
            },
            &secret,
            stanza_id,
            PollVoteAddressing {
                poll_creator_jid: "x@lid",
                voter_jid: "y@lid",
            },
            Some(PollVoteAddressing {
                poll_creator_jid: "x@s.whatsapp.net",
                voter_jid: "y@s.whatsapp.net",
            }),
        )
        .unwrap_err();
        let s = err.to_string();
        assert!(s.contains("primary="), "got: {s}");
        assert!(s.contains("fallback="), "got: {s}");
    }

    #[test]
    fn fallback_none_propagates_primary_error() {
        let secret = [0x66u8; 32];
        let (enc, iv) = encrypt_poll_vote_with_secret(
            &[compute_option_hash("A").to_vec()],
            &secret,
            "id",
            "c@s.whatsapp.net",
            "v@s.whatsapp.net",
        )
        .unwrap();
        assert!(
            decrypt_poll_vote_with_fallback(
                PollVoteCiphertext {
                    enc_payload: &enc,
                    enc_iv: &iv,
                },
                &secret,
                "id",
                PollVoteAddressing {
                    poll_creator_jid: "wrong@lid",
                    voter_jid: "wrong@lid",
                },
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn empty_vote_roundtrip() {
        let secret = [0xEFu8; 32];
        let (enc, iv) = encrypt_poll_vote_with_secret(
            &[],
            &secret,
            "id",
            "c@s.whatsapp.net",
            "v@s.whatsapp.net",
        )
        .unwrap();
        let out = decrypt_poll_vote_with_secret(
            PollVoteCiphertext {
                enc_payload: &enc,
                enc_iv: &iv,
            },
            &secret,
            "id",
            "c@s.whatsapp.net",
            "v@s.whatsapp.net",
        )
        .unwrap();
        assert!(out.is_empty());
    }
}
