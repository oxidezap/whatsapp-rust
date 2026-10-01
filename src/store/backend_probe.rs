//! Test-only destructor/parked-I/O backend, retaining the backend's overrides.

use super::release::tests::ProbeBackend;
use super::traits::*;
use bytes::Bytes;
use std::sync::Arc;
use wacore::appstate::hash::HashState;
use wacore::appstate::processor::AppStateMutationMAC;
use wacore::runtime::BoxFuture;
use wacore::store::error::Result;

// Keep every override of the test's inner backend, while allowing the test
// to park chosen I/O methods. This decorator is never compiled in production.
macro_rules! forward_methods {
    ($(fn $method:ident<$($lt:lifetime),+>($receiver:ident: &$self_lt:lifetime Self $(, $arg:ident: $ty:ty)*) -> $out:ty;)+) => {
        $(fn $method<$($lt,)+ 'async_trait>(&$self_lt self $(, $arg: $ty)*) -> BoxFuture<'async_trait, $out>
        where $($lt: 'async_trait,)+ Self: 'async_trait {
            Box::pin(async move {
                self.before_call(stringify!($method)).await;
                self.backend.$method($($arg),*).await
            })
        })+
    };
}

macro_rules! forward_domains {
    ($owner:ty) => {
        impl SignalStore for $owner {
            forward_methods! {
                fn put_identity<'s, 'a>(this: &'s Self, address: &'a str, key: [u8; 32]) -> Result<()>;
                fn put_identities_batch<'s, 'a>(this: &'s Self, identities: &'a [(Arc<str>, [u8; 32])]) -> Result<()>;
                fn load_identity<'s, 'a>(this: &'s Self, address: &'a str) -> Result<Option<[u8; 32]>>;
                fn delete_identity<'s, 'a>(this: &'s Self, address: &'a str) -> Result<()>;
                fn delete_identities_batch<'s, 'a>(this: &'s Self, addresses: &'a [Arc<str>]) -> Result<()>;
                fn get_session<'s, 'a>(this: &'s Self, address: &'a str) -> Result<Option<Bytes>>;
                fn put_session<'s, 'a, 'b>(this: &'s Self, address: &'a str, session: &'b [u8]) -> Result<()>;
                fn put_sessions_batch<'s, 'a>(this: &'s Self, sessions: &'a [(Arc<str>, Bytes)]) -> Result<()>;
                fn get_sessions_batch<'s, 'a>(this: &'s Self, addresses: &'a [Arc<str>]) -> Result<Vec<(Arc<str>, Bytes)>>;
                fn delete_session<'s, 'a>(this: &'s Self, address: &'a str) -> Result<()>;
                fn delete_sessions_batch<'s, 'a>(this: &'s Self, addresses: &'a [Arc<str>]) -> Result<()>;
                fn has_session<'s, 'a>(this: &'s Self, address: &'a str) -> Result<bool>;
                fn has_signal_state_for_user<'s, 'a>(this: &'s Self, user: &'a str) -> Result<bool>;
                fn store_prekey<'s, 'a>(this: &'s Self, id: u32, record: &'a [u8], uploaded: bool) -> Result<()>;
                fn store_prekeys_batch<'s, 'a>(this: &'s Self, keys: &'a [(u32, Bytes)], uploaded: bool) -> Result<()>;
                fn load_prekey<'s>(this: &'s Self, id: u32) -> Result<Option<Bytes>>;
                fn load_prekeys_batch<'s, 'a>(this: &'s Self, ids: &'a [u32]) -> Result<Vec<(u32, Bytes)>>;
                fn mark_prekeys_uploaded<'s, 'a>(this: &'s Self, ids: &'a [u32]) -> Result<()>;
                fn remove_prekey<'s>(this: &'s Self, id: u32) -> Result<()>;
                fn remove_prekeys_batch<'s, 'a>(this: &'s Self, ids: &'a [u32]) -> Result<()>;
                fn get_max_prekey_id<'s>(this: &'s Self) -> Result<u32>;
                fn store_signed_prekey<'s, 'a>(this: &'s Self, id: u32, record: &'a [u8]) -> Result<()>;
                fn load_signed_prekey<'s>(this: &'s Self, id: u32) -> Result<Option<Vec<u8>>>;
                fn load_all_signed_prekeys<'s>(this: &'s Self) -> Result<Vec<(u32, Vec<u8>)>>;
                fn remove_signed_prekey<'s>(this: &'s Self, id: u32) -> Result<()>;
                fn put_sender_key<'s, 'a, 'b>(this: &'s Self, address: &'a str, record: &'b [u8]) -> Result<()>;
                fn put_sender_keys_batch<'s, 'a>(this: &'s Self, sender_keys: &'a [(Arc<str>, Bytes)]) -> Result<()>;
                fn get_sender_key<'s, 'a>(this: &'s Self, address: &'a str) -> Result<Option<Vec<u8>>>;
                fn delete_sender_key<'s, 'a>(this: &'s Self, address: &'a str) -> Result<()>;
                fn delete_sender_keys_batch<'s, 'a>(this: &'s Self, addresses: &'a [Arc<str>]) -> Result<()>;
            }
        }
        impl AppSyncStore for $owner {
            forward_methods! {
                fn get_sync_key<'s, 'a>(this: &'s Self, key_id: &'a [u8]) -> Result<Option<AppStateSyncKey>>;
                fn set_sync_key<'s, 'a>(this: &'s Self, key_id: &'a [u8], key: AppStateSyncKey) -> Result<()>;
                fn get_version<'s, 'a>(this: &'s Self, name: &'a str) -> Result<Option<HashState>>;
                fn delete_version<'s, 'a>(this: &'s Self, name: &'a str) -> Result<()>;
                fn set_version<'s, 'a>(this: &'s Self, name: &'a str, state: HashState) -> Result<()>;
                fn put_mutation_macs<'s, 'a, 'b>(this: &'s Self, name: &'a str, version: u64, mutations: &'b [AppStateMutationMAC]) -> Result<()>;
                fn get_mutation_mac<'s, 'a, 'b>(this: &'s Self, name: &'a str, index_mac: &'b [u8]) -> Result<Option<Vec<u8>>>;
                fn get_mutation_macs<'s, 'a, 'b>(this: &'s Self, name: &'a str, index_macs: &'b [[u8; 32]]) -> Result<std::collections::HashMap<[u8; 32], Vec<u8>>>;
                fn delete_mutation_macs<'s, 'a, 'b>(this: &'s Self, name: &'a str, index_macs: &'b [Vec<u8>]) -> Result<()>;
                fn commit_patch<'s, 'a, 'b, 'c>(this: &'s Self, name: &'a str, state: HashState, removed_index_macs: &'b [Vec<u8>], added: &'c [AppStateMutationMAC]) -> Result<()>;
                fn clear_mutation_macs<'s, 'a>(this: &'s Self, name: &'a str) -> Result<()>;
                fn get_latest_sync_key_id<'s>(this: &'s Self) -> Result<Option<Vec<u8>>>;
            }
        }
        impl ProtocolStore for $owner {
            forward_methods! {
                fn get_sender_key_devices<'s, 'a>(this: &'s Self, group_jid: &'a str) -> Result<Vec<(String, bool)>>;
                fn set_sender_key_status<'s, 'a, 'b, 'c>(this: &'s Self, group_jid: &'a str, entries: &'b [(&'c str, bool)]) -> Result<()>;
                fn clear_sender_key_devices<'s, 'a>(this: &'s Self, group_jid: &'a str) -> Result<()>;
                fn delete_sender_key_device_rows<'s, 'a, 'b>(this: &'s Self, device_jids: &'a [&'b str]) -> Result<()>;
                fn clear_all_sender_key_devices<'s>(this: &'s Self) -> Result<()>;
                fn get_lid_mapping<'s, 'a>(this: &'s Self, lid: &'a str) -> Result<Option<LidPnMappingEntry>>;
                fn get_pn_mapping<'s, 'a>(this: &'s Self, phone: &'a str) -> Result<Option<LidPnMappingEntry>>;
                fn put_lid_mapping<'s, 'a>(this: &'s Self, entry: &'a LidPnMappingEntry) -> Result<()>;
                fn put_lid_mappings<'s, 'a>(this: &'s Self, entries: &'a [LidPnMappingEntry]) -> Result<()>;
                fn get_all_lid_mappings<'s>(this: &'s Self) -> Result<Vec<LidPnMappingEntry>>;
                fn save_base_key<'s, 'a, 'b, 'c>(this: &'s Self, address: &'a str, message_id: &'b str, base_key: &'c [u8]) -> Result<()>;
                fn has_same_base_key<'s, 'a, 'b, 'c>(this: &'s Self, address: &'a str, message_id: &'b str, current_base_key: &'c [u8]) -> Result<bool>;
                fn delete_base_key<'s, 'a, 'b>(this: &'s Self, address: &'a str, message_id: &'b str) -> Result<()>;
                fn delete_expired_base_keys<'s>(this: &'s Self, cutoff_timestamp: i64) -> Result<u32>;
                fn update_device_list<'s>(this: &'s Self, record: DeviceListRecord) -> Result<()>;
                fn update_device_lists<'s>(this: &'s Self, records: Vec<DeviceListRecord>) -> Result<()>;
                fn get_devices<'s, 'a>(this: &'s Self, user: &'a str) -> Result<Option<DeviceListRecord>>;
                fn get_devices_batch<'s, 'a, 'b>(this: &'s Self, users: &'a [&'b str]) -> Result<Vec<DeviceListRecord>>;
                fn delete_devices<'s, 'a>(this: &'s Self, user: &'a str) -> Result<()>;
                fn get_group_metadata<'s, 'a>(this: &'s Self, group_jid: &'a str) -> Result<Option<Vec<u8>>>;
                fn put_group_metadata<'s, 'a, 'b>(this: &'s Self, group_jid: &'a str, blob: &'b [u8]) -> Result<()>;
                fn delete_group_metadata<'s, 'a>(this: &'s Self, group_jid: &'a str) -> Result<()>;
                fn get_tc_token<'s, 'a>(this: &'s Self, jid: &'a str) -> Result<Option<TcTokenEntry>>;
                fn get_tc_tokens<'s, 'a>(this: &'s Self, jids: &'a [String]) -> Result<Vec<Option<TcTokenEntry>>>;
                fn put_tc_token<'s, 'a, 'b>(this: &'s Self, jid: &'a str, entry: &'b TcTokenEntry) -> Result<()>;
                fn delete_tc_token<'s, 'a>(this: &'s Self, jid: &'a str) -> Result<()>;
                fn get_all_tc_token_jids<'s>(this: &'s Self) -> Result<Vec<String>>;
                fn delete_expired_tc_tokens<'s>(this: &'s Self, token_cutoff: i64, sender_cutoff: i64) -> Result<u32>;
                fn touch_tc_token_sender_timestamp<'s, 'a>(this: &'s Self, jid: &'a str, sender_timestamp: i64) -> Result<()>;
                fn store_received_tc_token<'s, 'a, 'b>(this: &'s Self, jid: &'a str, token: &'b [u8], token_timestamp: i64) -> Result<()>;
                fn store_sent_message<'s, 'a, 'b, 'c>(this: &'s Self, chat_jid: &'a str, message_id: &'b str, payload: &'c [u8]) -> Result<()>;
                fn get_sent_message<'s, 'a, 'b>(this: &'s Self, chat_jid: &'a str, message_id: &'b str) -> Result<Option<Vec<u8>>>;
                fn take_sent_message<'s, 'a, 'b>(this: &'s Self, chat_jid: &'a str, message_id: &'b str) -> Result<Option<Vec<u8>>>;
                fn delete_expired_sent_messages<'s>(this: &'s Self, cutoff_timestamp: i64) -> Result<u32>;
                fn store_pending_inbound<'s, 'a, 'b, 'c, 'd>(this: &'s Self, chat: &'a str, sender: &'b str, id: &'c str, message: &'d [u8]) -> Result<()>;
                fn get_pending_inbound<'s, 'a, 'b, 'c>(this: &'s Self, chat: &'a str, sender: &'b str, id: &'c str) -> Result<Option<Vec<u8>>>;
                fn delete_pending_inbound<'s, 'a, 'b, 'c>(this: &'s Self, chat: &'a str, sender: &'b str, id: &'c str) -> Result<()>;
                fn delete_expired_pending_inbound<'s>(this: &'s Self, cutoff_timestamp: i64) -> Result<u32>;
                fn store_pending_inbound_batch<'s, 'a, 'b>(this: &'s Self, rows: &'a [PendingInboundRow<'b>]) -> Result<()>;
                fn delete_pending_inbound_batch<'s, 'a, 'b>(this: &'s Self, keys: &'a [PendingInboundKey<'b>]) -> Result<()>;
            }
        }
        impl MsgSecretStore for $owner {
            forward_methods! {
                fn put_msg_secret<'s, 'a, 'b, 'c, 'd>(this: &'s Self, chat: &'a str, sender: &'b str, msg_id: &'c str, secret: &'d MessageSecret) -> Result<()>;
                fn put_msg_secrets<'s>(this: &'s Self, entries: Vec<MsgSecretEntry>) -> Result<usize>;
                fn get_msg_secret<'s, 'a, 'b, 'c>(this: &'s Self, chat: &'a str, sender: &'b str, msg_id: &'c str) -> Result<Option<Vec<u8>>>;
                fn get_msg_secret_with_ts<'s, 'a, 'b, 'c>(this: &'s Self, chat: &'a str, sender: &'b str, msg_id: &'c str) -> Result<Option<(Vec<u8>, i64)>>;
                fn delete_expired_msg_secrets<'s>(this: &'s Self, cutoff_timestamp: i64) -> Result<u32>;
            }
        }
    };
}

forward_domains!(ProbeBackend);
