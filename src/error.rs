use thiserror::Error;

/// Application-level errors with actionable messages for operators.
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("permission denied reading SSH/journal logs: {0}")]
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    PermissionDenied(String),

    #[error("log source disconnected: {0}")]
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    LogSourceDisconnected(String),

    #[error("API request failed: {0}")]
    Api(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, AgentError>;
