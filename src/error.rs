//! エラー型の定義
//!
//! アプリケーション全体で使用するエラー型を定義する。

use std::path::PathBuf;

/// アプリケーションエラー
#[derive(Debug, thiserror::Error)]
pub(crate) enum AppError {
    /// IO エラー
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// 無効なパス
    #[error("Invalid path: {}", .0.display())]
    InvalidPath(PathBuf),

    /// IPC エラー
    #[error("IPC error: {0}")]
    Ipc(String),

    /// JSON パースエラー
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// アプリケーション用の Result 型エイリアス
pub(crate) type Result<T> = std::result::Result<T, AppError>;
