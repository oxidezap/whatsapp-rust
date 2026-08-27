//! Long-lived per-entry structures, on the shapes that actually accumulate.
//!
//! These are the collections a connected client keeps resident for hours: the
//! group snapshot behind every group send, and the device-registry record
//! behind every recipient. Their *size* is pinned by the retained-bytes tests
//! next to each type; what is measured here is the cost of the layouts those
//! tests buy — a sorted slice searched with `binary_search` has to stay at
//! least as fast as the `HashMap` it replaced on the read path, which is where
//! a group send spends one lookup per participant.

use divan::black_box;
use std::collections::HashMap;
use wacore::client::context::GroupInfo;
use wacore::types::message::AddressingMode;
use wacore_binary::CompactString;
use wacore_binary::jid::{Jid, Server};

fn main() {
    divan::main();
}

/// A LID group of `n` members, every member carrying a phone-number mapping —
/// the worst case for the mapping structures and the shape a community group
/// actually has.
fn lid_group(n: usize) -> (Vec<Jid>, HashMap<CompactString, Jid>) {
    let mut participants = Vec::with_capacity(n);
    let mut lid_to_pn = HashMap::with_capacity(n);
    for i in 0..n {
        let lid_user = CompactString::from(format!("1000000{i:08}"));
        let pn_user = CompactString::from(format!("5511{i:09}"));
        participants.push(Jid::new(lid_user.clone(), Server::Lid));
        lid_to_pn.insert(lid_user, Jid::new(pn_user, Server::Pn));
    }
    (participants, lid_to_pn)
}

/// Building the snapshot: paid once per group-metadata fetch, and the cost the
/// sorted layout front-loads in exchange for the read path below.
#[divan::bench(args = [64, 256, 1024])]
fn group_info_build(bencher: divan::Bencher, n: usize) {
    let (participants, lid_to_pn) = lid_group(n);
    bencher
        .with_inputs(|| (participants.clone(), lid_to_pn.clone()))
        .bench_values(|(participants, lid_to_pn)| {
            GroupInfo::with_lid_to_pn_map(participants, AddressingMode::Lid, lid_to_pn)
        });
}

/// One forward lookup per participant: what `resolve_group_devices` does on
/// every group send before it can query devices.
#[divan::bench(args = [64, 256, 1024])]
fn group_info_lookup_forward(bencher: divan::Bencher, n: usize) {
    let (participants, lid_to_pn) = lid_group(n);
    let users: Vec<CompactString> = participants.iter().map(|j| j.user.clone()).collect();
    let info = GroupInfo::with_lid_to_pn_map(participants, AddressingMode::Lid, lid_to_pn);
    bencher.bench(|| {
        for user in &users {
            black_box(info.phone_jid_for_lid_user(user));
        }
    });
}

/// One reverse lookup per participant: the direction that used to cost a whole
/// second `HashMap` and now costs a `u32` index.
#[divan::bench(args = [64, 256, 1024])]
fn group_info_lookup_reverse(bencher: divan::Bencher, n: usize) {
    let (participants, lid_to_pn) = lid_group(n);
    let phone_users: Vec<CompactString> = lid_to_pn.values().map(|j| j.user.clone()).collect();
    let info = GroupInfo::with_lid_to_pn_map(participants, AddressingMode::Lid, lid_to_pn);
    bencher.bench(|| {
        for user in &phone_users {
            black_box(info.lid_user_for_phone_user(user));
        }
    });
}

/// A participant-add notification on a warm group: the write path, which the
/// sorted layout makes a rebuild rather than n hash insertions.
#[divan::bench(args = [64, 256, 1024])]
fn group_info_add_participants(bencher: divan::Bencher, n: usize) {
    let (participants, lid_to_pn) = lid_group(n);
    let info = GroupInfo::with_lid_to_pn_map(participants, AddressingMode::Lid, lid_to_pn);
    let new_lid = Jid::lid("100000099999999");
    let new_pn = Jid::pn("5511999999999");
    bencher
        .with_inputs(|| info.clone())
        .bench_values(|mut info| {
            info.add_participants([(&new_lid, Some(&new_pn))]);
            info
        });
}
