//
// Copyright 2021 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("\"unknown {0} algorithm {1}\"")]
    UnknownAlgorithm(&'static str, String),
    #[error("invalid key size")]
    InvalidKeySize,
    #[error("invalid nonce size")]
    InvalidNonceSize,
    #[error("invalid input size")]
    InvalidInputSize,
    #[error("invalid authentication tag")]
    InvalidTag,
    #[error("output buffer too small: required {required} bytes, provided {provided}")]
    OutputBufferTooSmall { required: usize, provided: usize },
}

pub type Result<T> = std::result::Result<T, Error>;
