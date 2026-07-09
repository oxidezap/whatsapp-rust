use thiserror::Error;
use wacore::store::error::StoreError;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ChatStoreError {
    #[error("storage error")]
    Store(#[from] StoreError),

    #[error("invalid full-text search query")]
    InvalidSearchQuery,
}

pub type Result<T> = std::result::Result<T, ChatStoreError>;

pub(crate) fn db_err(e: diesel::result::Error) -> StoreError {
    StoreError::Database(Box::new(e))
}
