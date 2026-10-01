use whatsapp_rust::types::events::{EventInterest, EventKind};

// A host deliberately chooses a disposition for each known kind. The wildcard
// preserves source compatibility; the ALL-driven assertion catches additions.
fn disposition(kind: EventKind) -> Option<bool> {
    use EventKind::*;
    match kind {
        Messages | Receipt => Some(true),
        Connected
        | Disconnected
        | PairSuccess
        | PairError
        | LoggedOut
        | PairingQrCode
        | PairingCode
        | PairingCodeRefresh
        | QrScannedWithoutMultidevice
        | ClientOutdated
        | UndecryptableMessage
        | Notification
        | ChatPresence
        | Presence
        | PictureUpdate
        | UserAboutUpdate
        | ContactUpdated
        | ContactNumberChanged
        | ContactSyncRequested
        | GroupUpdate
        | ContactUpdate
        | IncomingCall
        | MissedCall
        | CallEndedElsewhere
        | RetiredPushNameUpdate
        | SelfPushNameUpdated
        | PinUpdate
        | MuteUpdate
        | ArchiveUpdate
        | StarUpdate
        | MarkChatAsReadUpdate
        | DeleteChatUpdate
        | ClearChatUpdate
        | UserStatusMuteUpdate
        | DeleteMessageForMeUpdate
        | LabelEditUpdate
        | LabelAssociationUpdate
        | HistorySync
        | OfflineSyncPreview
        | OfflineSyncCompleted
        | DirtyState
        | DeviceListUpdate
        | IdentityChange
        | BusinessStatusUpdate
        | StreamReplaced
        | TemporaryBan
        | ConnectFailure
        | StreamError
        | DisappearingModeChanged
        | NewsletterLiveUpdate
        | RawNode
        | MexNotification
        | PairPasskeyRequest
        | PairPasskeyConfirmation
        | PairPasskeyError
        | ServerAck
        | PairingQrCodesExhausted
        | PairingCodeError
        | AppStateSyncFailed
        | DecryptedPayload
        | SentFrame
        | MessageLabelAssociationUpdate
        | QuickReplyUpdate
        | DisableLinkPreviewsUpdate
        | ContactRemoved
        | EncDecryptFailed
        | CallLogSync
        | ClientExpirationChanged
        | OfflineSyncInterrupted
        | LockChatUpdate
        | FavoriteStickerUpdate
        | RemoveRecentStickerUpdate
        | FavoritesUpdate
        | StatusPrivacyUpdate
        | ReachoutTimelockUpdate => Some(false),
        _ => None,
    }
}

fn main() {
    let interest = EventInterest::of(&[EventKind::Messages, EventKind::Receipt]);
    for (index, &kind) in EventKind::ALL.iter().enumerate() {
        assert_eq!(kind as u8 as usize, index);
        let wanted = disposition(kind).expect("host must choose a disposition for new kinds");
        assert_eq!(interest.wants(kind), wanted, "{kind:?}");
        assert!(EventInterest::ALL.wants(kind));
    }
    assert_eq!(size_of::<EventKind>(), 1);
    assert_eq!(size_of::<EventInterest>(), 16);
}
