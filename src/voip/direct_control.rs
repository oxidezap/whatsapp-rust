//! Signaling controls shared by native and injected direct-call hosts.

use wacore::stanza::call::{TerminateParams, build_mute_v2, build_terminate};
use wacore_binary::Jid;

use super::direct::DirectCallHost;
use crate::client::{CallError, voip::TerminateDelivery};

pub(crate) async fn announce_muted_locked(
    host: &dyn DirectCallHost,
    call_id: &str,
    peer: &Jid,
    call_creator: &Jid,
    generation: u64,
    muted: bool,
) -> Result<(), CallError> {
    if host.call_state().registry().generation_of(call_id) != Some(generation) {
        return Err(CallError::Media("call is no longer active"));
    }
    if call_id.is_empty() {
        return Err(CallError::EmptyCallId);
    }
    host.send_node(build_mute_v2(
        call_id,
        peer,
        call_creator,
        &host.generate_request_id(),
        muted,
    ))
    .await
}

pub(crate) async fn terminate(
    host: &dyn DirectCallHost,
    call_id: &str,
    peer: &Jid,
    call_creator: &Jid,
) -> Result<(), CallError> {
    let epoch = host.call_state().epoch();
    let pinned = host.call_state().registry().generation_of(call_id);
    let _teardown = pinned.map(|generation| DirectTeardown {
        host,
        call_id,
        generation,
    });
    let _transition = host.lock_answer_transition(call_id).await;
    host.call_state().ensure_epoch(epoch)?;
    if host.call_state().registry().generation_of(call_id) != pinned {
        return Err(CallError::Media("call is no longer active"));
    }
    terminate_inner(
        host,
        call_id,
        std::slice::from_ref(peer),
        call_creator,
        pinned,
    )
    .await
    .1
}

pub(crate) async fn terminate_for_generation(
    host: &dyn DirectCallHost,
    call_id: &str,
    peers: &[Jid],
    call_creator: &Jid,
    generation: u64,
) -> TerminateDelivery {
    let (notified, result) =
        terminate_inner(host, call_id, peers, call_creator, Some(generation)).await;
    TerminateDelivery {
        notified,
        failure: result.err(),
    }
}

pub(crate) async fn terminate_inner(
    host: &dyn DirectCallHost,
    call_id: &str,
    peers: &[Jid],
    call_creator: &Jid,
    generation: Option<u64>,
) -> (usize, Result<(), CallError>) {
    let epoch = host.call_state().epoch();
    let _teardown = generation.map(|generation| DirectTeardown {
        host,
        call_id,
        generation,
    });
    if call_id.is_empty() {
        return (0, Err(CallError::EmptyCallId));
    }
    let mut notified = 0;
    let mut failure = None;
    for peer in peers {
        if let Err(error) = host.call_state().ensure_epoch(epoch) {
            if failure.is_none() {
                failure = Some(error);
            }
            break;
        }
        let id = host.generate_request_id();
        let stanza = build_terminate(&TerminateParams {
            call_id,
            to: peer,
            id: Some(&id),
            call_creator,
            reason: None,
        });
        match host.send_node(stanza).await {
            Ok(()) => notified += 1,
            Err(error) if failure.is_none() => failure = Some(error),
            Err(_) => {}
        }
    }
    (notified, failure.map_or(Ok(()), Err))
}

struct DirectTeardown<'a> {
    host: &'a dyn DirectCallHost,
    call_id: &'a str,
    generation: u64,
}

impl Drop for DirectTeardown<'_> {
    fn drop(&mut self) {
        super::direct::terminate_local_if_current(self.host, self.call_id, self.generation);
    }
}
