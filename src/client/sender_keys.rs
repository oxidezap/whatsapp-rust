//! Sender key tracking and message cache methods for Client.

use anyhow::Result;
use wacore::types::message::ChatMessageId;
use wacore_binary::Jid;
use waproto::whatsapp as wa;

use super::Client;

impl Client {
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.set_sender_key_status", level = "debug", skip_all, fields(count = device_jids.len(), has_key = has_key), err(Debug)))]
    pub(crate) async fn set_sender_key_status_for_devices(
        &self,
        group_jid: &str,
        device_jids: &[Jid],
        has_key: bool,
        exclude_own_devices: bool,
    ) -> Result<()> {
        let snapshot = if exclude_own_devices {
            Some(self.persistence_manager.get_device_snapshot())
        } else {
            None
        };
        let own_lid_user = snapshot
            .as_ref()
            .and_then(|s| s.lid.as_ref())
            .map(|j| j.user.as_str());
        let own_pn_user = snapshot
            .as_ref()
            .and_then(|s| s.pn.as_ref())
            .map(|j| j.user.as_str());

        let kept: Vec<&Jid> = device_jids
            .iter()
            .filter(|jid| {
                !exclude_own_devices
                    || !(own_lid_user.is_some_and(|u| u == jid.user)
                        || own_pn_user.is_some_and(|u| u == jid.user))
            })
            .collect();

        if kept.is_empty() {
            return Ok(());
        }

        let device_ids: Vec<String> = kept.iter().map(|jid| jid.to_string()).collect();
        let entries: Vec<(&str, bool)> = device_ids.iter().map(|s| (s.as_str(), has_key)).collect();
        self.persistence_manager
            .set_sender_key_status(group_jid, &entries)
            .await?;

        if has_key {
            // Marking devices warm can introduce (user, device) pairs the cached
            // map doesn't have yet, so drop it and let the next send rebuild from
            // the DB write above.
            self.sender_key_device_cache.invalidate(group_jid).await;
        } else {
            // Marking cold flips existing entries in place and bumps the map's
            // generation (no whole-group re-read on the next send), matching WA
            // Web's per-device markForgetSenderKey. The generation bump is what
            // the skdm_warm_memo compares, so a warm send re-runs its target
            // filter and re-sends the now-cold device's SKDM — no separate memo
            // invalidation, hence no cross-cache ordering window.
            let jids: Vec<Jid> = kept.into_iter().cloned().collect();
            self.sender_key_device_cache
                .mark_forgotten(group_jid, &jids)
                .await;
        }
        Ok(())
    }

    /// Mark device JIDs as needing fresh SKDM (has_key = false).
    /// Filters out our own devices (WA Web: `!isMeDevice(e)` check).
    /// Called from handle_retry_receipt for group/status messages.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.mark_forget_sender_key", level = "debug", skip_all, fields(count = device_jids.len()), err(Debug)))]
    pub(crate) async fn mark_forget_sender_key(
        &self,
        group_jid: &str,
        device_jids: &[Jid],
    ) -> Result<()> {
        self.set_sender_key_status_for_devices(group_jid, device_jids, false, true)
            .await?;
        Ok(())
    }

    /// Forward-secrecy rotation when participants leave a group. Mirrors WA
    /// Web's `removeParticipantInfo` (`GroupParticipantHelpers.js`): if any
    /// removed user had `has_key=true`, delete the bot's own sender key for
    /// the group and wipe `sender_key_devices` so the next send takes the
    /// `force_skdm=true` path (`!key_exists`) and redistributes to all
    /// remaining participants.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.rotate_sender_key_on_remove", level = "debug", skip_all, fields(removed = removed_user_ids.len())))]
    pub(crate) async fn rotate_sender_key_on_participant_remove(
        &self,
        group_jid: &str,
        removed_user_ids: &[&str],
    ) {
        if removed_user_ids.is_empty() {
            return;
        }

        // Read failure → rotate anyway. Better to pay the redistribute cost
        // than leave the sender key in place after a removal we couldn't audit.
        let (rows, read_failed) = match self
            .persistence_manager
            .get_sender_key_devices(group_jid)
            .await
        {
            Ok(r) => (r, false),
            Err(e) => {
                log::warn!(
                    "rotate_sender_key_on_participant_remove: read failed for {group_jid}: {e} \
                     — rotating conservatively"
                );
                (Vec::new(), true)
            }
        };

        let any_had_key = rows.iter().any(|(jid_str, has_key)| {
            *has_key
                && jid_str
                    .parse::<Jid>()
                    .ok()
                    .is_some_and(|jid| removed_user_ids.iter().any(|u| *u == jid.user.as_str()))
        });
        if !read_failed && !any_had_key {
            return;
        }

        self.force_rotate_own_sender_key(group_jid).await;
    }

    /// Unconditional forward-secrecy rotation: delete the bot's own sender key
    /// for the group and wipe `sender_key_devices` so the next send takes the
    /// `force_skdm=true` path and regenerates + redistributes a fresh key.
    /// Used by the removal audit and by the `<modify>` (number/LID migration)
    /// path, which WA Web (`modifyParticipantInfo`) rotates unconditionally.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.session.force_rotate_sender_key",
            level = "debug",
            skip_all
        )
    )]
    pub(crate) async fn force_rotate_own_sender_key(&self, group_jid: &str) {
        use wacore::libsignal::store::sender_key_name::SenderKeyName;
        use wacore::types::jid::JidExt;
        let snapshot = self.persistence_manager.get_device_snapshot();
        for own_jid in snapshot.lid.iter().chain(snapshot.pn.iter()) {
            let sk_name =
                SenderKeyName::from_parts(group_jid, own_jid.to_protocol_address().as_str());
            self.signal_cache
                .delete_sender_key(sk_name.cache_key())
                .await;
        }
        self.flush_signal_cache_batch_safe_logged("force_rotate_own_sender_key", None)
            .await;

        if let Err(e) = self
            .persistence_manager
            .clear_sender_key_devices(group_jid)
            .await
        {
            log::warn!("force_rotate_own_sender_key: clear DB failed: {e}");
        }
        self.sender_key_device_cache.invalidate(group_jid).await;
    }

    /// Take a sent message for retry handling. Checks L1 cache first (if enabled),
    /// then falls back to DB. On miss, tries an alternate PN/LID key to handle
    /// mapping changes between send time and retry time (WAWebLidMigrationUtils
    /// `getAlternateMsgKey`).
    /// Returns `(message, alternate_chat)`. When the message was found via the
    /// alternate PN/LID key, `alternate_chat` contains the namespace that
    /// matched -- the caller should use it for session operations instead of
    /// `resolve_encryption_jid` (which would map back to the primary).
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.take_recent_message", level = "debug", skip_all, fields(peer = %to.observe())))]
    pub(crate) async fn take_recent_message(
        &self,
        to: &Jid,
        id: &str,
    ) -> Option<(wa::Message, Option<Jid>)> {
        let primary_key = self.make_chat_message_id(to, id).await;
        if let Some(msg) = self.try_take_by_key(&primary_key).await {
            return Some((msg, None));
        }

        // Primary miss -- try alternate PN<->LID key.
        // If resolve_encryption_jid changed the namespace (PN→LID), the
        // original `to` is already the alternate -- skip the cache lookup.
        // Otherwise (LID input), swap via cache to try the PN form.
        let alt_chat = if primary_key.chat.server != to.server {
            Some(to.clone())
        } else {
            self.swap_pn_lid_namespace(&primary_key.chat).await
        };

        if let Some(alt_chat) = alt_chat {
            log::debug!(
                "Primary key miss for {}:{}, trying alternate {}",
                primary_key.chat.observe(),
                id,
                alt_chat.observe()
            );
            let alt_key = ChatMessageId {
                chat: alt_chat,
                id: primary_key.id,
            };
            if let Some(msg) = self.try_take_by_key(&alt_key).await {
                return Some((msg, Some(alt_key.chat)));
            }
        }

        None
    }

    /// Look up and consume a message by exact `ChatMessageId` (L1 cache then DB).
    async fn try_take_by_key(&self, key: &ChatMessageId) -> Option<wa::Message> {
        let chat_str = key.chat.to_string();
        let has_l1_cache = self.cache_config.recent_messages.capacity > 0;

        // L1 cache check (if capacity > 0)
        if has_l1_cache && let Some(bytes) = self.recent_messages.remove(key).await {
            if let Ok(msg) = waproto::codec::message_decode(bytes.as_slice()) {
                // Cache hit — consume the DB row in the background to avoid orphans.
                let backend = self.persistence_manager.backend();
                let mid = key.id.clone();
                self.runtime
                    .spawn(Box::pin(async move {
                        if let Err(e) = backend.take_sent_message(&chat_str, &mid).await {
                            log::warn!("Failed to clean up sent message {chat_str}:{mid}: {e}");
                        }
                    }))
                    .detach();
                return Some(msg);
            }
            log::warn!(
                "Failed to decode cached message for {}:{}, trying DB",
                key.chat.observe(),
                key.id
            );
        }

        // DB path (primary when cache capacity = 0, fallback when cache misses)
        match self
            .persistence_manager
            .backend()
            .take_sent_message(&chat_str, &key.id)
            .await
        {
            Ok(Some(bytes)) => match waproto::codec::message_decode(bytes.as_slice()) {
                Ok(msg) => Some(msg),
                Err(e) => {
                    log::warn!(
                        "Failed to decode DB message for {}:{}: {}",
                        key.chat.observe(),
                        key.id,
                        e
                    );
                    None
                }
            },
            Ok(None) => None,
            Err(e) => {
                log::warn!(
                    "Failed to read sent message from DB for {}:{}: {}",
                    key.chat.observe(),
                    key.id,
                    e
                );
                None
            }
        }
    }

    /// Non-consuming variant of [`Self::take_recent_message`]: returns the cached
    /// message (and the alternate-namespace chat it matched, if any) WITHOUT
    /// removing it from L1 or touching the DB. The retry handler uses this so a
    /// resend doesn't decode-then-re-encode the message and churn the DB (delete +
    /// re-store) on every retry. Returns `None` on an L1 miss (including DB-only
    /// mode, capacity 0); the caller falls back to take + re-add there.
    pub(crate) async fn peek_recent_message(
        &self,
        to: &Jid,
        id: &str,
    ) -> Option<(wa::Message, Option<Jid>)> {
        let primary_key = self.make_chat_message_id(to, id).await;
        if let Some(msg) = self.peek_by_key(&primary_key).await {
            return Some((msg, None));
        }

        let alt_chat = if primary_key.chat.server != to.server {
            Some(to.clone())
        } else {
            self.swap_pn_lid_namespace(&primary_key.chat).await
        };

        if let Some(alt_chat) = alt_chat {
            let alt_key = ChatMessageId {
                chat: alt_chat,
                id: primary_key.id,
            };
            if let Some(msg) = self.peek_by_key(&alt_key).await {
                return Some((msg, Some(alt_key.chat)));
            }
        }

        None
    }

    /// L1-only, non-consuming lookup. Returns `None` when L1 is disabled
    /// (capacity 0) or misses; the DB is intentionally not read here so the caller
    /// can fall back to the consuming take + re-add path.
    async fn peek_by_key(&self, key: &ChatMessageId) -> Option<wa::Message> {
        if self.cache_config.recent_messages.capacity == 0 {
            return None;
        }
        let bytes = self.recent_messages.get(key).await?;
        match waproto::codec::message_decode(bytes.as_slice()) {
            Ok(msg) => Some(msg),
            Err(e) => {
                log::warn!(
                    "Failed to decode cached message for {}:{}: {e}",
                    key.chat.observe(),
                    key.id
                );
                None
            }
        }
    }

    /// Store a sent message for retry handling. Always writes to DB; when L1 cache
    /// is enabled (capacity > 0) also stores in-memory for fast retrieval.
    /// In DB-only mode (capacity = 0), the DB write is awaited to guarantee persistence.
    /// With L1 cache, the DB write is backgrounded since the cache serves reads immediately.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.add_recent_message", level = "debug", skip_all, fields(peer = %to.observe())))]
    pub(crate) async fn add_recent_message(
        &self,
        to: &Jid,
        id: &str,
        msg: &wa::Message,
        // Avoids re-encoding when the send path already serialized `msg`.
        encoded: Option<std::sync::Arc<Vec<u8>>>,
    ) {
        let shared =
            encoded.unwrap_or_else(|| std::sync::Arc::new(waproto::codec::message_to_vec(msg)));
        let has_l1_cache = self.cache_config.recent_messages.capacity > 0;

        if has_l1_cache {
            // L1 cache serves reads immediately; DB write can be backgrounded.
            // Share the serialized bytes via Arc so the cache and the DB task
            // hold the same buffer instead of memcpy-ing the whole message.
            let key = self.make_chat_message_id(to, id).await;
            let chat_str = key.chat.to_string();
            let msg_id = key.id.clone();
            self.recent_messages
                .insert(key, std::sync::Arc::clone(&shared))
                .await;
            let backend = self.persistence_manager.backend();
            self.runtime
                .spawn(Box::pin(async move {
                    if let Err(e) = backend
                        .store_sent_message(&chat_str, &msg_id, &shared)
                        .await
                    {
                        log::warn!("Failed to store sent message to DB: {e}");
                    }
                }))
                .detach();
        } else {
            // DB-only mode: await to guarantee the row exists before returning.
            // Only chat + id are borrowed here, so resolve the chat directly and
            // pass the caller's `id`, skipping make_chat_message_id's owned
            // ChatMessageId whose id.to_owned() would just be borrowed away.
            let chat = self.resolve_encryption_jid(to).await;
            let chat_str = chat.to_string();
            if let Err(e) = self
                .persistence_manager
                .backend()
                .store_sent_message(&chat_str, id, &shared)
                .await
            {
                log::warn!("Failed to store sent message to DB: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::sender_key_device_cache::SenderKeyDeviceMap;
    use crate::test_utils::create_test_client;
    use std::sync::Arc;
    use wacore_binary::Jid;

    // A cold mark flips has_key in place, keeping the same cached_map Arc, so the
    // skdm_warm_memo cannot notice via pointer identity. It bumps the map's
    // generation instead; this asserts the client cold path advances that
    // generation (and flips the device) so a warm send re-runs its target filter
    // and re-sends the now-cold device's SKDM.
    #[tokio::test]
    async fn cold_mark_bumps_map_generation() {
        let client = create_test_client().await;
        let group = "120363000000000001@g.us";

        let map = client
            .sender_key_device_cache
            .get_or_init(group, async {
                Arc::new(SenderKeyDeviceMap::from_db_rows(&[(
                    "111:0@lid".to_string(),
                    true,
                )]))
            })
            .await;
        let gen_before = map.generation();

        let device: Jid = "111:0@lid".parse().unwrap();
        client
            .mark_forget_sender_key(group, std::slice::from_ref(&device))
            .await
            .unwrap();

        // Same Arc (in-place) but a fresh generation, so any memo stamped with
        // gen_before is now detectably stale.
        let map_after = client
            .sender_key_device_cache
            .get_or_init(group, async { panic!("cache should still hold the group") })
            .await;
        assert!(
            Arc::ptr_eq(&map, &map_after),
            "in-place flip keeps the same Arc"
        );
        assert_ne!(
            map_after.generation(),
            gen_before,
            "cold mark must advance the generation"
        );
        assert_eq!(
            map_after.device_has_key("111", 0),
            Some(false),
            "device flipped cold"
        );
    }

    // A retry receipt for a group we're in can name our own device alongside the
    // broken member. WA Web marks fresh SKDM only for other members
    // (`!isMeDevice`), so the cold mark must flip the member and never our own
    // device — otherwise an inbound retry could tear down our own group session.
    #[tokio::test]
    async fn cold_mark_excludes_own_devices() {
        let client = create_test_client().await;
        let own_lid: Jid = "888000888000888:3@lid".parse().unwrap();
        client
            .persistence_manager
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                own_lid.clone(),
            )))
            .await;

        let group = "120363000000000001@g.us";
        let map = client
            .sender_key_device_cache
            .get_or_init(group, async {
                Arc::new(SenderKeyDeviceMap::from_db_rows(&[
                    ("888000888000888:3@lid".to_string(), true), // our own device
                    ("111000111000111:0@lid".to_string(), true), // broken member
                ]))
            })
            .await;
        let gen_before = map.generation();

        let member: Jid = "111000111000111:0@lid".parse().unwrap();
        client
            .mark_forget_sender_key(group, &[own_lid.clone(), member])
            .await
            .unwrap();

        assert_eq!(
            map.device_has_key("111000111000111", 0),
            Some(false),
            "the member device is flipped cold"
        );
        assert_eq!(
            map.device_has_key("888000888000888", 3),
            Some(true),
            "our own device is excluded, never marked cold"
        );
        assert_ne!(
            map.generation(),
            gen_before,
            "the member flip still advances the generation"
        );
    }

    // The WARM mark also excludes own devices (mirrors WA Web's `!isMeDevice` guard,
    // applied to markHasSenderKey too). Our own companions must never be memoized, or
    // the forget path (which also excludes own) could never un-mark one whose single
    // SKDM encryption failed — a permanent orphan. The external member IS marked warm.
    #[tokio::test]
    async fn warm_mark_excludes_own_devices() {
        let client = create_test_client().await;
        let own_lid: Jid = "888000888000888:3@lid".parse().unwrap();
        client
            .persistence_manager
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                own_lid.clone(),
            )))
            .await;

        let group = "120363000000000001@g.us";
        let own_companion: Jid = "888000888000888:5@lid".parse().unwrap();
        let member: Jid = "111000111000111:0@lid".parse().unwrap();

        client
            .set_sender_key_status_for_devices(group, &[own_companion, member], true, true)
            .await
            .unwrap();

        let rows = client
            .persistence_manager
            .get_sender_key_devices(group)
            .await
            .unwrap();
        let map = SenderKeyDeviceMap::from_db_rows(&rows);
        assert_eq!(
            map.device_has_key("111000111000111", 0),
            Some(true),
            "the external member is marked warm"
        );
        assert_eq!(
            map.device_has_key("888000888000888", 5),
            None,
            "our own companion is never memoized"
        );
    }

    // When every named device is our own, nothing is kept: no DB write, no flip,
    // and (crucially) no generation bump that would churn the warm memo.
    #[tokio::test]
    async fn cold_mark_of_only_own_devices_is_noop() {
        let client = create_test_client().await;
        let own_lid: Jid = "888000888000888:3@lid".parse().unwrap();
        client
            .persistence_manager
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                own_lid.clone(),
            )))
            .await;

        let group = "120363000000000001@g.us";
        let map = client
            .sender_key_device_cache
            .get_or_init(group, async {
                Arc::new(SenderKeyDeviceMap::from_db_rows(&[(
                    "888000888000888:3@lid".to_string(),
                    true,
                )]))
            })
            .await;
        let gen_before = map.generation();

        client
            .mark_forget_sender_key(group, std::slice::from_ref(&own_lid))
            .await
            .unwrap();

        assert_eq!(map.device_has_key("888000888000888", 3), Some(true));
        assert_eq!(
            map.generation(),
            gen_before,
            "excluding all named devices leaves the map untouched"
        );
    }
}
