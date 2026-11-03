//! The replacement validates constructors eagerly; the oracle validates on use.
use wacore_libsignal::protocol::{SignalProtocolError, SignedPreKeyRecord};
pub trait ValidatedSignedPreKey {
    fn validated(self) -> SignedPreKeyRecord;
}
impl ValidatedSignedPreKey for SignedPreKeyRecord {
    fn validated(self) -> SignedPreKeyRecord {
        self
    }
}
impl ValidatedSignedPreKey for Result<SignedPreKeyRecord, SignalProtocolError> {
    fn validated(self) -> SignedPreKeyRecord {
        self.expect("valid synthetic signed prekey")
    }
}
