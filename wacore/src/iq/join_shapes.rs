//! Auto-generated response presence-gate shapes (WhatsApp 2.3000.1045368834). DO NOT EDIT.
//!
//! The success shapes of RPCs whose answers carry presence gates, from the
//! whatspec IQ index.
//!
//! Regenerate with `cargo run -p whatspec-codegen`; never edit by hand. To pin
//! another RPC, add it to `WANTED` in the codegen's join-shape emitter.

/// (variant tag, required child tags) for each success variant of
/// `WASmaxGroupsAcceptGroupAddRPC`, in RPC cascade order. A variant with no required children
/// accepts a bare `<iq type="result">`.
pub const ACCEPT_GROUP_ADD_SUCCESS: &[(&str, &[&str])] = &[
    (
        "AcceptGroupAddResponseGroupJoinRequestSuccess",
        &["membership_approval_request"],
    ),
    ("AcceptGroupAddResponseSuccess", &[]),
];

/// (variant tag, required child tags) for each success variant of
/// `WASmaxGroupsJoinLinkedGroupRPC`, in RPC cascade order. A variant with no required children
/// accepts a bare `<iq type="result">`.
pub const JOIN_LINKED_GROUP_SUCCESS: &[(&str, &[&str])] = &[
    (
        "JoinLinkedGroupResponseGroupJoinRequestSuccess",
        &["membership_approval_request"],
    ),
    ("JoinLinkedGroupResponseSuccess", &[]),
];

/// (variant tag, required child tags) for each success variant of
/// `WASmaxPassiveModeActiveIQRPC`, in RPC cascade order. A variant with no required children
/// accepts a bare `<iq type="result">`.
pub const PASSIVE_ACTIVE_SUCCESS: &[(&str, &[&str])] = &[("ActiveIQResponseSuccess", &["active"])];

/// (variant tag, required child tags) for each success variant of
/// `WASmaxPassiveModePassiveIQRPC`, in RPC cascade order. A variant with no required children
/// accepts a bare `<iq type="result">`.
pub const PASSIVE_PASSIVE_SUCCESS: &[(&str, &[&str])] =
    &[("PassiveIQResponseSuccess", &["passive"])];
