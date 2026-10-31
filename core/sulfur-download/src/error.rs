use std::{io, path::PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("HTTP {status} from {url}")]
    Status { status: u16, url: String },
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("I/O error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("checksum mismatch for {0}")]
    ChecksumMismatch(PathBuf),
    #[error("size mismatch for {path}: expected {expected} bytes, got {actual}")]
    SizeMismatch {
        path: PathBuf,
        expected: u64,
        actual: u64,
    },
    #[error("server returned an unexpected range for {0}")]
    BadRange(String),
    #[error("download cancelled")]
    Cancelled,
    #[error("no download source for {0}")]
    NoSource(PathBuf),
}

impl DownloadError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// The same request may succeed if repeated after a pause.
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Status { status, .. } => matches!(status, 408 | 425 | 429 | 500..=599),
            Self::Network(_) | Self::ChecksumMismatch(_) | Self::SizeMismatch { .. } => true,
            Self::BadRange(_) => true,
            Self::Io { .. } | Self::Cancelled | Self::NoSource(_) => false,
        }
    }

    /// The current source will not serve this file, but another one might.
    pub fn is_source_specific(&self) -> bool {
        matches!(self, Self::Status { status, .. } if (400..500).contains(status) && !self.is_transient())
    }
}
