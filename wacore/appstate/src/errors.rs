use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppStateError {
    #[error("missing value MAC of previous SET operation")]
    MissingPreviousSetValueOperation,
    #[error("mismatching LTHash")]
    MismatchingLTHash,
    #[error("mismatching content MAC")]
    MismatchingContentMAC,
    #[error("mismatching index MAC")]
    MismatchingIndexMAC,
    #[error("didn't find app state key")]
    KeyNotFound,
    #[error("missing value blob in record")]
    MissingValueBlob,
    #[error("value blob too short (need at least 48 bytes for IV + MAC)")]
    ValueBlobTooShort,
    #[error("decryption failed")]
    DecryptionFailed,
    #[error("failed to decode protobuf")]
    DecodeFailed,
    #[error("missing index MAC in record")]
    MissingIndexMAC,
    #[error("missing key ID in record")]
    MissingKeyId,
    #[error("snapshot MAC mismatch")]
    SnapshotMACMismatch,
    #[error("patch snapshot MAC mismatch")]
    PatchSnapshotMACMismatch,
    #[error("patch MAC mismatch")]
    PatchMACMismatch,
    #[error("patch version mismatch: expected {expected}, got {got}")]
    PatchVersionMismatch { expected: u64, got: u64 },
    // WA Web (WAWebSyncdValidateMutations) reports SAME_INDEX_FOR_MULTIPLE_MUTATIONS_IN_PATCH
    // and throws a fatal error: a repeated index within one patch is a tampering signal.
    #[error("duplicate index within a single patch")]
    DuplicateIndexInPatch,
    // SyncdOperation is an open enum: a value we don't know can't be folded
    // into the LTHash add/subtract math without corrupting it, so fail the
    // patch instead of guessing (the old closed-enum decode silently read it
    // as SET).
    #[error("unsupported syncd operation {0} in patch")]
    UnsupportedSyncdOperation(i32),
}

pub type Result<T> = std::result::Result<T, AppStateError>;
