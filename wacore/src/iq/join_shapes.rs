//! Auto-generated group-join success shapes (WhatsApp 2.3000.1045368834). DO NOT EDIT.
//!
//! The success shapes of the group-join RPC, from the whatspec IQ index.
//!
//! Regenerate with `cargo run -p whatspec-codegen`; never edit by hand. To pin
//! another join RPC, extend the codegen's join-shape emitter.

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
