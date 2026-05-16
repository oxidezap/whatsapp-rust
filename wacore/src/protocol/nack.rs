//! Codes sent as the `error` attr on `<ack class="message">` nacks
//! (`Handle/MsgSendAck.js` + `Create/NackFromStanza.js`). Server stops
//! retransmitting on receipt; use vs `<receipt type="retry">` for
//! recoverable errors.

#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::WireEnum)]
#[wire(kind = "int")]
#[allow(dead_code)]
pub enum NackReason {
    #[wire = 421]
    StaleGroupAddressingMode,
    #[wire = 475]
    NewChatMessagesCapped,
    #[wire = 487]
    ParsingError,
    #[wire = 488]
    UnrecognizedStanza,
    #[wire = 489]
    UnrecognizedStanzaClass,
    #[wire = 490]
    UnrecognizedStanzaType,
    /// WA Web pairs with `<meta failure_reason=N>` child.
    #[wire = 491]
    InvalidProtobuf,
    #[wire = 493]
    InvalidHostedCompanionStanza,
    #[wire = 495]
    MissingMessageSecret,
    #[wire = 496]
    SignalErrorOldCounter,
    #[wire = 499]
    MessageDeletedOnPeer,
    #[wire = 500]
    UnhandledError,
    #[wire = 550]
    UnsupportedAdminRevoke,
    #[wire = 551]
    UnsupportedLIDGroup,
    #[wire = 552]
    DBOperationFailed,
    #[wire_fallback]
    Unknown(i32),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pin every code against `Create/NackFromStanza.js` so a renumber
    /// can't silently break server compatibility.
    #[test]
    fn nack_reason_codes_match_wa_web() {
        assert_eq!(NackReason::StaleGroupAddressingMode.code(), 421);
        assert_eq!(NackReason::NewChatMessagesCapped.code(), 475);
        assert_eq!(NackReason::ParsingError.code(), 487);
        assert_eq!(NackReason::UnrecognizedStanza.code(), 488);
        assert_eq!(NackReason::UnrecognizedStanzaClass.code(), 489);
        assert_eq!(NackReason::UnrecognizedStanzaType.code(), 490);
        assert_eq!(NackReason::InvalidProtobuf.code(), 491);
        assert_eq!(NackReason::InvalidHostedCompanionStanza.code(), 493);
        assert_eq!(NackReason::MissingMessageSecret.code(), 495);
        assert_eq!(NackReason::SignalErrorOldCounter.code(), 496);
        assert_eq!(NackReason::MessageDeletedOnPeer.code(), 499);
        assert_eq!(NackReason::UnhandledError.code(), 500);
        assert_eq!(NackReason::UnsupportedAdminRevoke.code(), 550);
        assert_eq!(NackReason::UnsupportedLIDGroup.code(), 551);
        assert_eq!(NackReason::DBOperationFailed.code(), 552);
    }
}
