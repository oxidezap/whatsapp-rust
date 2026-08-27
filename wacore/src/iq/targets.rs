//! Auto-generated IQ addressing (WhatsApp 2.3000.1045368834). DO NOT EDIT.
//!
//! How the official client addresses the requests this repository sends.
//!
//! Regenerate with `cargo run -p whatspec-codegen`; never edit by hand. To pin
//! another request, add it to `WANTED` in the codegen's IQ target emitter.

/// Who a request is addressed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IqTarget {
    /// The main server, `s.whatsapp.net`.
    MainServer,
    /// The group server, `g.us`: it answers about groups in general rather than
    /// about one group.
    GroupServer,
    /// The one group the request acts on.
    GroupJid,
}

/// Addressing of `LeaveGroupIq`, from `leaveGroup` in `WAWebGroupExitJob` (`set` in `w:g2`).
pub const LEAVE_GROUP: IqTarget = IqTarget::GroupServer;

/// Addressing of `GroupParticipatingIq`, from `makeGetParticipatingGroupsRequestParticipatingParticipants` in `WASmaxOutGroupsGetParticipatingGroupsRequest` (`get` in `w:g2`).
pub const GROUP_PARTICIPATING: IqTarget = IqTarget::GroupServer;

/// Addressing of `BatchGetGroupInfoIq`, from `makeBatchGetGroupInfoRequestQueryGroup` in `WASmaxOutGroupsBatchGetGroupInfoRequest` (`get` in `w:g2`).
pub const BATCH_GET_GROUP_INFO: IqTarget = IqTarget::GroupServer;

/// Addressing of `GetGroupInviteInfoIq`, from `makeGetInviteGroupInfoRequest` in `WASmaxOutGroupsGetInviteGroupInfoRequest` (`get` in `w:g2`).
pub const GET_GROUP_INVITE_INFO: IqTarget = IqTarget::GroupServer;

/// Addressing of `GroupQueryIq`, from `makeGetGroupInfoRequestQueryAddRequest` in `WASmaxOutGroupsGetGroupInfoRequest` (`get` in `w:g2`).
pub const GROUP_QUERY: IqTarget = IqTarget::GroupJid;

/// Addressing of `SetGroupSubjectIq`, from `makeSetSubjectRequest` in `WASmaxOutGroupsSetSubjectRequest` (`set` in `w:g2`).
pub const SET_GROUP_SUBJECT: IqTarget = IqTarget::GroupJid;

/// Addressing of `SetGroupDescriptionIq`, from `makeSetDescriptionRequestDescriptionBody` in `WASmaxOutGroupsSetDescriptionRequest` (`set` in `w:g2`).
pub const SET_GROUP_DESCRIPTION: IqTarget = IqTarget::GroupJid;

/// Addressing of `SetGroupLockedIq`, from `makeSetPropertyRequestLocked` in `WASmaxOutGroupsSetPropertyRequest` (`set` in `w:g2`).
pub const SET_GROUP_LOCKED: IqTarget = IqTarget::GroupJid;

/// Addressing of `ReportGroupMessagesIq`, from `makeReportMessagesRequest` in `WASmaxOutGroupsReportMessagesRequest` (`set` in `w:g2`).
pub const REPORT_GROUP_MESSAGES: IqTarget = IqTarget::GroupJid;

/// Addressing of `GetReportedGroupMessagesIq`, from `makeGetReportedMessagesRequest` in `WASmaxOutGroupsGetReportedMessagesRequest` (`get` in `w:g2`).
pub const GET_REPORTED_GROUP_MESSAGES: IqTarget = IqTarget::GroupJid;

/// Addressing of `GetMembershipRequestsIq`, from `makeGetMembershipApprovalRequestsRequest` in `WASmaxOutGroupsGetMembershipApprovalRequestsRequest` (`get` in `w:g2`).
pub const GET_MEMBERSHIP_REQUESTS: IqTarget = IqTarget::GroupJid;

/// Addressing of `MembershipRequestActionIq`, from `makeMembershipRequestsActionRequestMembershipRequestsActionApproveParticipant` in `WASmaxOutGroupsMembershipRequestsActionRequest` (`set` in `w:g2`).
pub const MEMBERSHIP_REQUEST_ACTION: IqTarget = IqTarget::GroupJid;
