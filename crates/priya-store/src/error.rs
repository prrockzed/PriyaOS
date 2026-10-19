//! What can go wrong in the store, in the shared error model.

use std::path::PathBuf;

use priya_api::error::{ErrorKind, UserFacing};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The database was written by a newer PriyaOS.
    #[error("database schema v{found} is newer than this build supports (v{supported})")]
    NewerSchema { found: i64, supported: i64 },
    #[error("blob {hash} is missing or damaged: {reason}")]
    BadBlob { hash: String, reason: String },
    /// A reference count used wrongly: a bug, not damage.
    #[error("blob {hash}: {reason}")]
    BlobMisuse { hash: String, reason: String },
    #[error("{0}")]
    Format(#[from] time::error::Format),
}

impl StoreError {
    /// An I/O error with the path it happened on.
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Self {
        let path = path.into();
        move |source| Self::Io { path, source }
    }
}

impl UserFacing for StoreError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::Sqlite(rusqlite::Error::SqliteFailure(e, _))
                if e.code == rusqlite::ErrorCode::DatabaseBusy =>
            {
                ErrorKind::Unavailable
            }
            Self::NewerSchema { .. } => ErrorKind::Conflict,
            _ => ErrorKind::Internal,
        }
    }

    fn user_message(&self) -> String {
        match self {
            Self::NewerSchema { .. } => "Your PriyaOS history was saved by a newer version of \
                PriyaOS. Update PriyaOS, or restore a backup from before the update."
                .into(),
            Self::BadBlob { .. } => "Part of your PriyaOS history is damaged.".into(),
            _ => "PriyaOS couldn't read or save its history.".into(),
        }
    }
}

pub type Result<T, E = StoreError> = std::result::Result<T, E>;
