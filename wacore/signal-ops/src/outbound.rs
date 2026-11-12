// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Outbound messages built inside the Signal service.
//!
//! The application hands the service a `waE2E.Message`; wacore's stanza
//! builder pads it, adds own-device copies and encrypts it per device. The
//! WhatsApp client only answers device-list and prekey queries through the
//! [`SendContextResolver`] and transmits the finished stanza, so it never
//! handles the plaintext. The prekey bundles it supplies are verified before
//! use; see the crate's trust boundary. Session changes and the sent-message record commit before
//! the stanza is returned, matching wacore's rule that Signal state is durable
//! before ciphertext is published.

use crate::error::OpsError;
use crate::ops::RemoteBundle;
use crate::ops::{ServiceStore, SignalOps, chain_key, group_key, session_key};
use crate::trust::{SignalResolver, Verifying};
use std::collections::HashSet;
use wacore::client::context::{GroupRoutingInfo, SendContextResolver};
use wacore::libsignal::protocol::{UsePQRatchet, process_prekey_bundle};
use wacore::libsignal::store::sender_key_name::SenderKeyName;
use wacore::runtime::Runtime;
use wacore::send::{
    DmSignalAddressing, DmStanzaRequest, GroupStanzaRequest, ResolvedDmDevices,
    ResolvedGroupDevices, SenderKeyDistributionPolicy, SignalStores, prepare_dm_stanza,
    prepare_group_stanza_resolved, retain_skdm_distribution_targets,
};
use wacore::send::{PairwiseRetryDestination, PairwiseRetryRequest, prepare_pairwise_retry_stanza};
use wacore::types::jid::JidExt as _;
use wacore::types::message::{AddressingMode, EditAttribute};
use wacore_binary::{Jid, JidExt};
use wacore_recordstore::Namespace;
use waproto::whatsapp as wa;

/// A finished `<message>` stanza for the client to transmit.
#[derive(Debug, Clone)]
pub struct PreparedSend {
    /// The stanza, marshaled as the binary node format the socket writes.
    pub stanza: Vec<u8>,
    /// Devices that were addressed but got no `<enc>`.
    pub unreached_devices: Vec<Jid>,
    /// Device-list hash of the addressed set, for comparing with the server
    /// ack.
    pub phash: Option<String>,
}

fn sent_key(chat: &Jid, message_id: &str) -> String {
    format!("{chat}\u{0}{message_id}")
}

/// Sent-message record: `[inserted ms, u64 BE][has_secret][secret (32 bytes,
/// when set)][audience count, u32 BE]([JID length, u16 BE][JID])*[message]`.
/// The audience is who the message went to: the chat user for a direct
/// message, the group's participants at send time otherwise. A retry is only
/// answered for that audience.
fn encode_sent(message: &[u8], secret: Option<&[u8; 32]>, audience: &[Jid]) -> Vec<u8> {
    let inserted_ms = u64::try_from(wacore::time::now_utc().timestamp_millis()).unwrap_or(0);
    let audience: Vec<String> = audience.iter().map(Jid::to_string).collect();
    let mut out = Vec::with_capacity(
        8 + 1 + 32 + 4 + audience.iter().map(|jid| 2 + jid.len()).sum::<usize>() + message.len(),
    );
    out.extend_from_slice(&inserted_ms.to_be_bytes());
    match secret {
        Some(secret) => {
            out.push(1);
            out.extend_from_slice(secret);
        }
        None => out.push(0),
    }
    let audience: Vec<&String> = audience
        .iter()
        .filter(|jid| u16::try_from(jid.len()).is_ok())
        .collect();
    out.extend_from_slice(
        &u32::try_from(audience.len())
            .unwrap_or(u32::MAX)
            .to_be_bytes(),
    );
    for jid in audience {
        out.extend_from_slice(&(jid.len() as u16).to_be_bytes());
        out.extend_from_slice(jid.as_bytes());
    }
    out.extend_from_slice(message);
    out
}

/// Borrowed fields of a persisted sent-message record.
struct SentRecord<'a> {
    inserted_ms: u64,
    secret: Option<&'a [u8]>,
    audience: Vec<&'a str>,
    body: &'a [u8],
}

fn decode_sent(record: &[u8]) -> Result<SentRecord<'_>, OpsError> {
    let corrupt = || OpsError::InvalidInput("sent-message record is corrupt".into());
    let (inserted, rest) = record.split_at_checked(8).ok_or_else(corrupt)?;
    let inserted_ms = u64::from_be_bytes(inserted.try_into().map_err(|_| corrupt())?);
    let (secret, mut rest) = match rest.first() {
        Some(0) => (None, &rest[1..]),
        Some(1) if rest.len() >= 33 => (Some(&rest[1..33]), &rest[33..]),
        _ => return Err(corrupt()),
    };
    let (count, tail) = rest.split_at_checked(4).ok_or_else(corrupt)?;
    rest = tail;
    let count = u32::from_be_bytes(count.try_into().map_err(|_| corrupt())?);
    let mut audience = Vec::new();
    for _ in 0..count {
        let (len, tail) = rest.split_at_checked(2).ok_or_else(corrupt)?;
        let len = usize::from(u16::from_be_bytes(len.try_into().map_err(|_| corrupt())?));
        let (jid, tail) = tail.split_at_checked(len).ok_or_else(corrupt)?;
        audience.push(std::str::from_utf8(jid).map_err(|_| corrupt())?);
        rest = tail;
    }
    Ok(SentRecord {
        inserted_ms,
        secret,
        audience,
        body: rest,
    })
}

fn dm_lock_jids(devices: &[Jid], preferred: &[Jid]) -> Vec<Jid> {
    let mut jids: Vec<_> = devices.iter().chain(preferred).cloned().collect();
    jids.sort_by_key(|jid| jid.to_protocol_address().as_str().to_owned());
    jids.dedup_by_key(|jid| jid.to_protocol_address().as_str().to_owned());
    jids
}

fn send_error(error: anyhow::Error) -> OpsError {
    OpsError::Send(format!("{error:#}"))
}

/// A finished group `<message>` stanza.
#[derive(Debug, Clone)]
pub struct PreparedGroupSend {
    pub stanza: Vec<u8>,
    /// Devices this send distributed the sender key to. Once the server acks
    /// the stanza, pass them to [`SignalOps::mark_sender_key_distributed`].
    pub distribution_targets: Vec<Jid>,
    /// The group's sender-key epoch this send distributed under. Pass it with
    /// the acknowledgment, so an acknowledgment that arrives after a rotation
    /// cannot mark devices as holding the replacement key.
    pub distribution_epoch: u64,
    /// Users whose devices were reported unregistered while fetching bundles;
    /// the client should refresh their device lists.
    pub stale_device_users: Vec<String>,
    pub phash: Option<String>,
}

/// Where a retried message goes, matching the client's retransmission routes.
#[derive(Debug, Clone)]
pub enum RetryRoute {
    /// A direct message; `recipient` is the original chat user when the
    /// requester is one of this account's own devices.
    Direct {
        recipient: Option<Jid>,
    },
    Group {
        addressing_mode: AddressingMode,
    },
    BroadcastList,
}

/// A retry receipt the client received for a message this device sent.
#[derive(Debug, Clone)]
pub struct RetryRequest<'a> {
    /// The chat the original went to (the recipient user, group or list).
    pub chat: &'a Jid,
    pub message_id: &'a str,
    /// The device that asked, as addressed on the wire.
    pub requester: &'a Jid,
    /// The address to encrypt to (the requester's LID when known).
    pub encryption_jid: &'a Jid,
    pub route: RetryRoute,
    pub retry_count: u8,
    /// Keys the receipt carried, used to start a fresh session.
    pub bundle: Option<&'a RemoteBundle>,
}

struct OwnIdentity {
    pn: Jid,
    lid: Option<Jid>,
    account: Option<wa::ADVSignedDeviceIdentity>,
}

/// The address a device is encrypted to: its LID when the client knows the
/// mapping for a phone-number device, otherwise the device itself.
async fn encryption_jid(resolver: &dyn SendContextResolver, device: &Jid) -> Jid {
    if device.is_pn()
        && let Some(lid_user) = resolver.get_lid_for_phone(&device.user).await
        && let Ok(lid) = format!("{lid_user}@lid").parse::<Jid>()
    {
        return lid.with_device(device.device);
    }
    device.clone()
}

fn sender_key_device_key(group: &str, device: &Jid) -> String {
    format!("{group}\u{0}{device}")
}

/// A group's sender-key epoch. It sits outside the `{group}\0` device
/// prefix, so clearing the distribution marks on rotation leaves it in place.
fn distribution_epoch_key(group: &str) -> String {
    format!("{group}\u{1}epoch")
}

fn new_epoch() -> [u8; 8] {
    use rand::Rng;
    rand::make_rng::<rand::rngs::StdRng>()
        .next_u64()
        .to_be_bytes()
}

fn decode_epoch(bytes: Option<Vec<u8>>) -> u64 {
    bytes
        .and_then(|bytes| <[u8; 8]>::try_from(bytes.as_slice()).ok())
        .map_or(0, u64::from_be_bytes)
}

/// Whether `a` and `b` name the same account, across its PN and LID forms.
async fn same_account(
    resolver: &dyn SendContextResolver,
    group: Option<&GroupRoutingInfo>,
    a: &Jid,
    b: &Jid,
) -> bool {
    if a.is_same_user_as(b) && a.is_pn() == b.is_pn() && a.is_lid() == b.is_lid() {
        return true;
    }
    let (pn, lid) = match (a.is_pn(), b.is_pn(), a.is_lid(), b.is_lid()) {
        (true, _, _, true) => (a, b),
        (_, true, true, _) => (b, a),
        _ => return false,
    };
    if let Some(group) = group
        && group
            .lid_user_for_phone_user(&pn.user)
            .is_some_and(|user| *user == lid.user)
    {
        return true;
    }
    resolver
        .get_lid_for_phone(&pn.user)
        .await
        .is_some_and(|user| user == lid.user)
}

impl<S: ServiceStore> SignalOps<S> {
    /// Refuse a retry for a device outside the original conversation before
    /// any state changes: re-encrypting a stored message to a device the
    /// caller picks would hand out past plaintext. Membership and PN/LID
    /// mappings come from `resolver`, the same source a send trusts for its
    /// device list.
    async fn check_retry_requester(
        &self,
        resolver: &dyn SendContextResolver,
        request: &RetryRequest<'_>,
        own: &OwnIdentity,
        audience: &[Jid],
    ) -> Result<(), OpsError> {
        let refuse = || OpsError::InvalidInput("the retry requester was not a recipient".into());
        let group = match request.route {
            RetryRoute::Direct { .. } => None,
            RetryRoute::Group { .. } | RetryRoute::BroadcastList => Some(
                resolver
                    .resolve_group_routing_info(request.chat)
                    .await
                    .map_err(send_error)?,
            ),
        };
        let group = group.as_deref();
        // The encryption address must belong to the requester itself.
        if !same_account(resolver, group, request.requester, request.encryption_jid).await {
            return Err(refuse());
        }
        for own in std::iter::once(&own.pn).chain(own.lid.as_ref()) {
            if same_account(resolver, group, own, request.requester).await {
                return Ok(());
            }
        }
        // Someone else must have been sent this message, and, for a group,
        // still be a member: a later joiner gets no earlier messages, and a
        // removed member gets nothing more.
        let mut sent_to = false;
        for account in audience {
            if same_account(resolver, group, account, request.requester).await {
                sent_to = true;
                break;
            }
        }
        if !sent_to {
            return Err(refuse());
        }
        if let Some(group) = group {
            for member in &group.participants {
                if same_account(resolver, Some(group), member, request.requester).await {
                    return Ok(());
                }
            }
            return Err(refuse());
        }
        Ok(())
    }

    async fn own_identity(&self) -> Result<OwnIdentity, OpsError> {
        let keys = self.keys().await;
        let pn: Jid = keys
            .own_pn
            .as_deref()
            .ok_or_else(|| OpsError::InvalidInput("device is not paired: own JID unknown".into()))?
            .parse()
            .map_err(|_| OpsError::InvalidInput("stored own JID does not parse".into()))?;
        let lid: Option<Jid> = keys
            .own_lid
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(|_| OpsError::InvalidInput("stored own LID does not parse".into()))?;
        let account = keys
            .account
            .as_deref()
            .map(waproto::codec::adv_signed_device_identity_decode)
            .transpose()
            .map_err(|_| {
                OpsError::InvalidInput("stored account identity does not decode".into())
            })?;
        Ok(OwnIdentity { pn, lid, account })
    }

    /// Build a direct-message stanza for `to`. Devices come from the
    /// resolver (the WhatsApp client); missing sessions are set up from
    /// bundles it fetches.
    pub async fn send_direct(
        &self,
        runtime: &dyn Runtime,
        resolver: &dyn SignalResolver,
        to: &Jid,
        message: &wa::Message,
        message_id: &str,
    ) -> Result<PreparedSend, OpsError> {
        let OwnIdentity {
            pn: own_jid,
            lid: own_lid,
            account,
        } = self.own_identity().await?;

        let mut devices = resolver
            .resolve_devices(&[to.to_non_ad(), own_jid.to_non_ad()])
            .await
            .map_err(send_error)?;
        // Hosted (Cloud API) devices never get a copy, as in the client.
        devices.retain(|device| !device.is_hosted());
        wacore::types::jid::sort_dedup_by_device(&mut devices);
        let resolved = ResolvedDmDevices::new(devices, &own_jid, own_lid.as_ref());

        // Fix each device's Signal address up front (its LID when the client
        // knows one, as wacore's fan-out would) so the locks taken here cover
        // exactly the sessions the encrypt will write.
        let mut encryption = Vec::with_capacity(resolved.devices().len());
        for device in resolved.devices() {
            encryption.push(encryption_jid(resolver, device).await);
        }
        // An existing PN session remains a valid fallback even when the client
        // knows the preferred LID. Both addresses can be written by fan-out.
        let lock_jids = dm_lock_jids(resolved.devices(), &encryption);
        let held_keys: Vec<String> = lock_jids
            .iter()
            .map(|jid| session_key(&jid.to_protocol_address()))
            .collect();
        let _ = resolved.signal_addressing_or_init(DmSignalAddressing::new(encryption, lock_jids));
        let held = self.lock_keys(held_keys).await?;
        held.run(async {
            let staged = std::sync::Arc::new(self.store().staged());
            let stores = self.stores_over(staged.clone()).await;
            let (mut sender_keys, mut sessions, mut identities, mut prekeys, signed_prekeys) = (
                stores.clone(),
                stores.clone(),
                stores.clone(),
                stores.clone(),
                stores,
            );
            let mut signal = SignalStores {
                sender_key_store: &mut sender_keys,
                session_store: &mut sessions,
                identity_store: &mut identities,
                prekey_store: &mut prekeys,
                signed_prekey_store: &signed_prekeys,
            };
            let verifying = Verifying {
                ops: self,
                inner: resolver,
                store: staged.clone(),
            };
            let prepared = prepare_dm_stanza(
                runtime,
                &mut signal,
                &verifying,
                DmStanzaRequest {
                    own_jid: &own_jid,
                    own_lid: own_lid.as_ref(),
                    account: account.as_ref(),
                    to,
                    message,
                    message_id,
                    edit: None,
                    extra_nodes: &[],
                    devices: &resolved,
                    pre_encoded: None,
                },
            )
            .await
            .map_err(send_error)?;

            let encoded = waproto::codec::message_to_vec(message);
            staged
                .put_aux(
                    Namespace::SentMessage,
                    &sent_key(to, message_id),
                    &encode_sent(
                        &encoded,
                        prepared.message_secret.as_ref(),
                        std::slice::from_ref(to),
                    ),
                )
                .await?;
            staged.commit().await?;

            let stanza = wacore_binary::marshal(&prepared.node)
                .map_err(|e| OpsError::Send(format!("stanza did not marshal: {e}")))?;
            Ok(PreparedSend {
                stanza,
                unreached_devices: prepared.unreached_devices,
                phash: prepared.phash.map(|p| p.to_string()),
            })
        })
        .await
    }

    /// The message this device sent to `chat` with `message_id`, if still
    /// kept. Used to answer retry receipts.
    pub async fn sent_message(
        &self,
        chat: &Jid,
        message_id: &str,
    ) -> Result<Option<wa::Message>, OpsError> {
        Ok(self
            .sent_record(chat, message_id)
            .await?
            .map(|(message, _)| message))
    }

    /// A sent message and the audience it went to.
    async fn sent_record(
        &self,
        chat: &Jid,
        message_id: &str,
    ) -> Result<Option<(wa::Message, Vec<Jid>)>, OpsError> {
        let Some(record) = self
            .store()
            .load_aux(Namespace::SentMessage, &sent_key(chat, message_id))
            .await?
        else {
            return Ok(None);
        };
        let record = decode_sent(&record)?;
        let mut message = waproto::codec::message_decode(record.body)
            .map_err(|e| OpsError::InvalidInput(format!("sent message did not decode: {e}")))?;
        if let Some(secret) = record.secret {
            let mut context = message
                .message_context_info
                .as_option()
                .cloned()
                .unwrap_or_default();
            context.message_secret = Some(secret.to_vec());
            message.message_context_info = Some(context).into();
        }
        let audience = record
            .audience
            .iter()
            .map(|jid| jid.parse())
            .collect::<Result<Vec<Jid>, _>>()
            .map_err(|_| OpsError::InvalidInput("sent-message audience is corrupt".into()))?;
        Ok(Some((message, audience)))
    }

    /// Build a group stanza: a sender-key message for the group plus, for
    /// devices that do not yet hold this device's sender key, the key itself
    /// encrypted pairwise. Membership and addressing come from the resolver.
    pub async fn send_group(
        &self,
        runtime: &dyn Runtime,
        resolver: &dyn SignalResolver,
        group: &Jid,
        message: &wa::Message,
        message_id: &str,
    ) -> Result<PreparedGroupSend, OpsError> {
        let OwnIdentity {
            pn: own_jid,
            lid: own_lid,
            account,
        } = self.own_identity().await?;
        let own_lid = own_lid.ok_or_else(|| {
            OpsError::InvalidInput("group sends need this device's LID; call set_own_jids".into())
        })?;
        let group_str = group.to_string();
        // Resolve membership while holding the rotation locks. A send that
        // began before a removal may finish on the old chain, but cannot
        // distribute a replacement chain using its old membership snapshot.
        // g:/k: locks precede every s: lock in the common ordering.
        let held = self
            .lock_keys(vec![
                group_key(&group_str),
                chain_key(&group_str, own_jid.to_protocol_address().as_str()),
                chain_key(&group_str, own_lid.to_protocol_address().as_str()),
            ])
            .await?;
        held.run(async {
            let routing = resolver
                .resolve_group_routing_info(group)
                .await
                .map_err(send_error)?;
            let is_lid_mode = routing.addressing_mode == AddressingMode::Lid;
            let own_sending = if is_lid_mode { &own_lid } else { &own_jid };

            // Same device query as the client's resolve_group_devices: LID-mode
            // members are queried by phone number, then mapped back.
            let mut queries: Vec<Jid> = routing
                .participants
                .iter()
                .map(|jid| {
                    if is_lid_mode
                        && jid.is_lid()
                        && let Some(pn) = routing.phone_jid_for_lid_user(&jid.user)
                    {
                        return pn.to_non_ad();
                    }
                    jid.to_non_ad()
                })
                .collect();
            if !routing
                .participants
                .iter()
                .any(|participant| participant.is_same_user_as(own_sending))
            {
                queries.push(own_jid.to_non_ad());
            }
            let mut devices = resolver
                .resolve_devices(&queries)
                .await
                .map_err(send_error)?;
            if is_lid_mode {
                devices = devices
                    .into_iter()
                    .map(|device| routing.phone_device_jid_into_lid(device))
                    .collect();
            }
            wacore::types::jid::sort_dedup_by_device(&mut devices);
            let addressed = ResolvedGroupDevices::new(devices);

            let mut held_keys = Vec::new();
            let mut signal_addresses = std::collections::HashMap::new();
            for device in addressed.devices() {
                let encryption = encryption_jid(resolver, device).await;
                held_keys.push(session_key(&device.to_protocol_address()));
                held_keys.push(session_key(&encryption.to_protocol_address()));
                signal_addresses.insert(device.clone(), encryption);
            }
            let session_held = self.lock_keys(held_keys).await?;
            session_held
                .run(async {
                    let own_chain = SenderKeyName::from_parts(
                        &group_str,
                        own_sending.to_protocol_address().as_str(),
                    );
                    // A missing chain means a fresh key: everyone needs it, as in the
                    // client's force_skdm path.
                    let force = self
                        .store()
                        .get_sender_key(own_chain.cache_key())
                        .await?
                        .is_none();
                    let targets = if force {
                        let mut all = addressed.devices().to_vec();
                        retain_skdm_distribution_targets(&mut all, own_sending);
                        all
                    } else {
                        let warm: HashSet<String> = self
                            .store()
                            .scan_aux(Namespace::SenderKeyDevices, &format!("{group_str}\u{0}"))
                            .await?
                            .into_iter()
                            .collect();
                        let is_warm =
                            |jid: &Jid| warm.contains(&sender_key_device_key(&group_str, jid));
                        addressed
                            .devices()
                            .iter()
                            .filter(|device| {
                                !(device.is_hosted()
                        || device.user == own_sending.user && device.device == own_sending.device
                        // WA Web treats a companion as warm only if its
                        // primary (device 0) is too.
                        || is_warm(device) && is_warm(&device.with_device(0)))
                            })
                            .cloned()
                            .collect()
                    };

                    let staged = std::sync::Arc::new(self.store().staged());
                    let stores = self.stores_over(staged.clone()).await;
                    let (
                        mut sender_keys,
                        mut sessions,
                        mut identities,
                        mut prekeys,
                        signed_prekeys,
                    ) = (
                        stores.clone(),
                        stores.clone(),
                        stores.clone(),
                        stores.clone(),
                        stores,
                    );
                    let mut signal = SignalStores {
                        sender_key_store: &mut sender_keys,
                        session_store: &mut sessions,
                        identity_store: &mut identities,
                        prekey_store: &mut prekeys,
                        signed_prekey_store: &signed_prekeys,
                    };
                    let verifying = Verifying {
                        ops: self,
                        inner: resolver,
                        store: staged.clone(),
                    };
                    let prepared = prepare_group_stanza_resolved(
                        runtime,
                        &mut signal,
                        &verifying,
                        GroupStanzaRequest {
                            group: &routing,
                            own_jid: &own_jid,
                            own_lid: &own_lid,
                            account: account.as_ref(),
                            to: group,
                            message,
                            message_id,
                            force_distribution: force,
                            distribution_targets: Some(targets),
                            distribution_policy: SenderKeyDistributionPolicy::BestEffort,
                            phash_devices: Some(&addressed),
                            edit: None,
                            extra_nodes: &[],
                            pre_encoded: None,
                        },
                        Some(&signal_addresses),
                    )
                    .await
                    .map_err(send_error)?;

                    let encoded = waproto::codec::message_to_vec(message);
                    staged
                        .put_aux(
                            Namespace::SentMessage,
                            &sent_key(group, message_id),
                            &encode_sent(
                                &encoded,
                                prepared.message_secret.as_ref(),
                                &routing.participants,
                            ),
                        )
                        .await?;
                    staged.commit().await?;

                    let stanza = wacore_binary::marshal(&prepared.node)
                        .map_err(|e| OpsError::Send(format!("stanza did not marshal: {e}")))?;
                    // Read under the group lock that rotation also takes.
                    let distribution_epoch = decode_epoch(
                        self.store()
                            .load_aux(
                                Namespace::SenderKeyDevices,
                                &distribution_epoch_key(&group_str),
                            )
                            .await?,
                    );
                    Ok(PreparedGroupSend {
                        stanza,
                        distribution_targets: prepared.skdm_devices,
                        distribution_epoch,
                        stale_device_users: prepared.stale_device_users,
                        phash: prepared.phash.map(|p| p.to_string()),
                    })
                })
                .await
        })
        .await
    }

    /// Record that the server acked a group send whose stanza carried the
    /// sender key to `devices`. This account's own devices are never recorded
    /// (WA Web's `!isMeDevice`), so they get the key on every send.
    ///
    /// `distribution_epoch` is the send's [`PreparedGroupSend::distribution_epoch`].
    /// Returns `false`, recording nothing, when the group's sender key has
    /// rotated since that send.
    pub async fn mark_sender_key_distributed(
        &self,
        group: &Jid,
        devices: &[Jid],
        distribution_epoch: u64,
    ) -> Result<bool, OpsError> {
        let OwnIdentity { pn, lid, .. } = self.own_identity().await?;
        let group_str = group.to_string();
        let held = self.lock_keys(vec![group_key(&group_str)]).await?;
        held.run(async {
            let current = decode_epoch(
                self.store()
                    .load_aux(
                        Namespace::SenderKeyDevices,
                        &distribution_epoch_key(&group_str),
                    )
                    .await?,
            );
            if current != distribution_epoch {
                return Ok(false);
            }
            for device in devices {
                if device.is_same_user_as(&pn)
                    || lid.as_ref().is_some_and(|l| device.is_same_user_as(l))
                {
                    continue;
                }
                self.store()
                    .put_aux(
                        Namespace::SenderKeyDevices,
                        &sender_key_device_key(&group_str, device),
                        &[1],
                    )
                    .await?;
            }
            Ok(true)
        })
        .await
    }

    /// Rotate this device's PN and LID sender-key chains, clear distribution
    /// marks and start a new distribution epoch in one fenced batch. Use after membership changes so a removed
    /// participant cannot decrypt later messages using the previous chain.
    pub async fn forget_sender_key_devices(&self, group: &Jid) -> Result<(), OpsError> {
        let OwnIdentity { pn, lid, .. } = self.own_identity().await?;
        let group_str = group.to_string();
        let addresses: Vec<_> = std::iter::once(&pn)
            .chain(lid.iter())
            .map(|jid| jid.to_protocol_address())
            .collect();
        let mut held_keys = vec![group_key(&group_str)];
        held_keys.extend(
            addresses
                .iter()
                .map(|address| chain_key(&group_str, address.as_str())),
        );
        let held = self.lock_keys(held_keys).await?;
        held.run(async {
            let staged = self.store().staged();
            for address in addresses {
                let name = SenderKeyName::from_parts(&group_str, address.as_str());
                staged.delete_sender_key(name.cache_key()).await?;
            }
            let prefix = format!("{group}\u{0}");
            let keys = staged
                .scan_aux(Namespace::SenderKeyDevices, &prefix)
                .await?;
            let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
            staged
                .delete_aux(Namespace::SenderKeyDevices, &refs)
                .await?;
            staged
                .put_aux(
                    Namespace::SenderKeyDevices,
                    &distribution_epoch_key(&group_str),
                    &new_epoch(),
                )
                .await?;
            staged.commit().await?;
            Ok(())
        })
        .await
    }

    /// Re-encrypt a sent message for the device that asked for it. Without a
    /// session and without keys in the receipt, a bundle is fetched through
    /// the resolver, as the client does. Status messages are not handled here.
    pub async fn resend(
        &self,
        resolver: &dyn SignalResolver,
        request: RetryRequest<'_>,
    ) -> Result<Vec<u8>, OpsError> {
        let (message, audience) = self
            .sent_record(request.chat, request.message_id)
            .await?
            .ok_or_else(|| OpsError::InvalidInput("no record of that sent message".into()))?;
        let own = self.own_identity().await?;
        self.check_retry_requester(resolver, &request, &own, &audience)
            .await?;
        let OwnIdentity { account, .. } = own;
        let address = request.encryption_jid.to_protocol_address();

        // A group retry also means the requester may have lost this device's
        // sender key: forget its mark so the next group send redistributes it,
        // as WA Web and the client do (mark_requester_for_fresh_skdm).
        let mut held_keys = vec![session_key(&address)];
        if matches!(request.route, RetryRoute::Group { .. }) {
            held_keys.push(group_key(&request.chat.to_string()));
        }
        let held = self.lock_keys(held_keys).await?;
        held.run(async {
            if matches!(request.route, RetryRoute::Group { .. }) {
                let group = request.chat.to_string();
                let keys: Vec<String> = [request.requester, request.encryption_jid]
                    .into_iter()
                    .map(|jid| sender_key_device_key(&group, jid))
                    .collect();
                let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
                // A new epoch first, so an acknowledgment still in flight from
                // an earlier send cannot restore the mark deleted here.
                self.store()
                    .put_aux(
                        Namespace::SenderKeyDevices,
                        &distribution_epoch_key(&group),
                        &new_epoch(),
                    )
                    .await?;
                self.store()
                    .delete_aux(Namespace::SenderKeyDevices, &refs)
                    .await?;
            }
            let staged = std::sync::Arc::new(self.store().staged());
            if let Some(bundle) = request.bundle {
                self.establish_into(&staged, &address, bundle).await?;
            } else if !staged.has_session(address.as_str()).await? {
                let verifying = Verifying {
                    ops: self,
                    inner: resolver,
                    store: staged.clone(),
                };
                let fetched = verifying
                    .fetch_prekeys(std::slice::from_ref(request.encryption_jid))
                    .await
                    .map_err(send_error)?;
                let bundle = fetched.get(request.encryption_jid).ok_or_else(|| {
                    OpsError::Send("no session and no prekey bundle for the requester".into())
                })?;
                let mut sessions = self.stores_over(staged.clone()).await;
                let mut identities = sessions.clone();
                process_prekey_bundle(
                    &address,
                    &mut sessions,
                    &mut identities,
                    bundle,
                    &mut rand::make_rng::<rand::rngs::StdRng>(),
                    UsePQRatchet::No,
                )
                .await?;
            }

            let destination = match request.route {
                RetryRoute::Direct { recipient } => PairwiseRetryDestination::Direct {
                    to: request.requester.clone(),
                    recipient,
                },
                RetryRoute::Group { addressing_mode } => PairwiseRetryDestination::Participant {
                    to: request.chat.clone(),
                    participant: request.requester.clone(),
                    addressing_mode: Some(addressing_mode),
                },
                RetryRoute::BroadcastList => PairwiseRetryDestination::Participant {
                    to: request.chat.clone(),
                    participant: request.requester.clone(),
                    addressing_mode: None,
                },
            };
            let mut sessions = self.stores_over(staged.clone()).await;
            let mut identities = sessions.clone();
            let node = prepare_pairwise_retry_stanza(
                &mut sessions,
                &mut identities,
                PairwiseRetryRequest {
                    destination,
                    encryption_jid: request.encryption_jid.clone(),
                    message: &message,
                    message_id: request.message_id.to_owned(),
                    retry_count: request.retry_count,
                    account: account.as_ref(),
                    edit: EditAttribute::infer_from_message(&message),
                    pre_encoded: None,
                },
            )
            .await
            .map_err(send_error)?;
            staged.commit().await?;
            wacore_binary::marshal(&node)
                .map_err(|e| OpsError::Send(format!("stanza did not marshal: {e}")))
        })
        .await
    }

    /// Delete sent-message records inserted before `cutoff_ms`; a retry
    /// receipt for an older message can no longer be answered. Returns how
    /// many were removed.
    pub async fn prune_sent_messages(&self, cutoff_ms: u64) -> Result<usize, OpsError> {
        let store = self.store();
        let mut expired = Vec::new();
        for key in store.scan_aux(Namespace::SentMessage, "").await? {
            // A record that no longer decodes cannot serve a resend; delete it
            // rather than let one corrupt entry stop every later prune.
            if let Some(record) = store.load_aux(Namespace::SentMessage, &key).await?
                && decode_sent(&record).map_or(true, |record| record.inserted_ms < cutoff_ms)
            {
                expired.push(key);
            }
        }
        let refs: Vec<&str> = expired.iter().map(String::as_str).collect();
        store.delete_aux(Namespace::SentMessage, &refs).await?;
        Ok(expired.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_message_locks_cover_the_pn_fallback_and_preferred_lid() {
        let pn: Jid = "222:1@s.whatsapp.net".parse().unwrap();
        let lid: Jid = "900222:1@lid".parse().unwrap();
        assert_eq!(
            dm_lock_jids(std::slice::from_ref(&pn), std::slice::from_ref(&lid)),
            vec![pn.clone(), lid]
        );
        assert_eq!(
            dm_lock_jids(std::slice::from_ref(&pn), std::slice::from_ref(&pn)),
            vec![pn]
        );
    }
}
