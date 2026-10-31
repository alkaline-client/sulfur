use std::path::PathBuf;

use sulfur_download::DownloadError;

#[derive(Debug, thiserror::Error)]
pub enum McError {
    #[error(transparent)]
    Download(#[from] DownloadError),
    #[error("invalid metadata in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("I/O error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("unknown version {0}")]
    UnknownVersion(String),
    #[error("version {0} is not installed")]
    NotInstalled(String),
    #[error("no suitable Java runtime found (Java {0} is required)")]
    NoJava(u32),
    #[error("failed to start the game: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("{0}")]
    Invalid(String),
}

impl McError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Download(DownloadError::Cancelled))
    }
}

/// Reads and parses a JSON file with path-aware errors.
pub(crate) fn read_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> crate::Result<T> {
    let bytes = std::fs::read(path).map_err(|e| McError::io(path, e))?;
    serde_json::from_slice(&bytes).map_err(|source| McError::Json {
        path: path.to_owned(),
        source,
    })
}
