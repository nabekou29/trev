//! Application error types.

/// Application-level errors.
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum AppError {
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Configuration error.
    #[error("config error: {0}")]
    Config(String),

    /// IPC communication error.
    #[error("IPC error: {0}")]
    Ipc(String),

    /// Tree building error.
    #[error("tree error: {0}")]
    Tree(String),
}
