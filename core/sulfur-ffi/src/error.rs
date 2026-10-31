use sulfur_download::DownloadError;
use sulfur_minecraft::McError;

/// Errors surfaced to Kotlin as `CoreException` subclasses carrying a message.
#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum CoreError {
    #[error("{0}")]
    Network(String),
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    NotFound(String),
    #[error("cancelled")]
    Cancelled,
    #[error("{0}")]
    Java(String),
    /// The account cannot be used here (offline accounts outside mainland China).
    #[error("{0}")]
    Account(String),
    #[error("{0}")]
    Other(String),
}

impl From<McError> for CoreError {
    fn from(e: McError) -> Self {
        let message = e.to_string();
        match e {
            McError::Download(DownloadError::Cancelled) => Self::Cancelled,
            McError::Download(DownloadError::Io { .. }) | McError::Io { .. } => Self::Io(message),
            McError::Download(_) => Self::Network(message),
            McError::UnknownVersion(_) | McError::NotInstalled(_) => Self::NotFound(message),
            McError::NoJava(_) | McError::Spawn(_) => Self::Java(message),
            McError::Json { .. } | McError::Invalid(_) => Self::Other(message),
        }
    }
}

impl From<DownloadError> for CoreError {
    fn from(e: DownloadError) -> Self {
        McError::from(e).into()
    }
}

impl From<sulfur_core::SettingsError> for CoreError {
    fn from(e: sulfur_core::SettingsError) -> Self {
        Self::Io(e.to_string())
    }
}

impl From<tokio::task::JoinError> for CoreError {
    fn from(e: tokio::task::JoinError) -> Self {
        Self::Other(format!("background task failed: {e}"))
    }
}

impl From<sulfur_mods::ModsError> for CoreError {
    fn from(e: sulfur_mods::ModsError) -> Self {
        match e {
            sulfur_mods::ModsError::Download(d) => d.into(),
            other => Self::Other(other.to_string()),
        }
    }
}
