//! IPC モジュール
//!
//! Unix Domain Socket を使用した IPC サーバー/クライアントを提供する。
//! daemon モードで起動した trev に対して、外部から `reveal` などの
//! コマンドを送信できる。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::error::{AppError, Result};

/// IPC リクエスト
///
/// クライアントからサーバーへ送信されるコマンド。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", content = "args")]
pub(crate) enum IpcRequest {
    /// 指定パスを表示・選択
    #[serde(rename = "reveal")]
    Reveal {
        /// 表示するパス
        path: PathBuf,
    },
    /// 疎通確認
    #[serde(rename = "ping")]
    Ping {},
    /// 終了要求
    #[serde(rename = "quit")]
    Quit {},
}

/// IPC レスポンス
///
/// サーバーからクライアントへ送信される応答。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IpcResponse {
    /// 成功したかどうか
    pub(crate) ok: bool,
    /// エラーメッセージ（失敗時のみ）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

impl IpcResponse {
    /// 成功レスポンスを作成
    pub(crate) fn ok() -> Self {
        Self {
            ok: true,
            error: None,
        }
    }

    /// エラーレスポンスを作成
    pub(crate) fn err(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(message.into()),
        }
    }
}

/// IPC コマンド（メインループへ通知用）
///
/// IPC サーバーからメインループへ送信されるイベント。
#[derive(Debug, Clone)]
pub(crate) enum IpcCommand {
    /// reveal コマンド
    Reveal(PathBuf),
    /// 終了要求
    Quit,
}

/// ソケットパスを取得する
///
/// `$XDG_RUNTIME_DIR/trev/<workspace_key>.sock` 形式のパスを返す。
pub(crate) fn socket_path(workspace_key: &str) -> Result<PathBuf> {
    let runtime_dir = directories::BaseDirs::new()
        .and_then(|dirs| dirs.runtime_dir().map(Path::to_path_buf))
        .unwrap_or_else(std::env::temp_dir);

    let socket_dir = runtime_dir.join("trev");
    std::fs::create_dir_all(&socket_dir)?;

    Ok(socket_dir.join(format!("{}.sock", workspace_key)))
}

/// コマンドファイルのパスを取得する
///
/// daemon モードで trev からエディタへイベントを通知するためのファイル。
pub(crate) fn command_file_path(workspace_key: &str) -> Result<PathBuf> {
    let runtime_dir = directories::BaseDirs::new()
        .and_then(|dirs| dirs.runtime_dir().map(Path::to_path_buf))
        .unwrap_or_else(std::env::temp_dir);

    let cmd_dir = runtime_dir.join("trev");
    std::fs::create_dir_all(&cmd_dir)?;

    Ok(cmd_dir.join(format!("{}.cmd", workspace_key)))
}

/// エディタへのコマンド
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EditorCommand {
    /// アクション種別
    pub(crate) action: EditorAction,
    /// 対象パス
    pub(crate) path: PathBuf,
}

/// エディタアクション種別
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EditorAction {
    /// バッファで開く
    Edit,
    /// 分割して開く
    Split,
    /// 縦分割して開く
    Vsplit,
    /// 新しいタブで開く
    Tabedit,
}

/// コマンドファイルに書き込む
pub(crate) fn write_editor_command(workspace_key: &str, command: &EditorCommand) -> Result<()> {
    let path = command_file_path(workspace_key)?;
    let json = serde_json::to_string(command)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// workspace キーを取得する
///
/// Git リポジトリのルートディレクトリ名、または指定パスのディレクトリ名を返す。
pub(crate) fn workspace_key(path: &Path) -> String {
    // パスを正規化（相対パス対応）
    let abs_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    // Git root を探す
    let mut current = abs_path.clone();
    loop {
        if current.join(".git").exists() {
            return current
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "default".to_string());
        }
        if !current.pop() {
            break;
        }
    }

    // Git root が見つからない場合は指定パスのディレクトリ名
    abs_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "default".to_string())
}

/// IPC サーバーを起動する
///
/// Unix Domain Socket でリッスンし、受信したコマンドを `tx` へ送信する。
pub(crate) async fn serve(
    socket_path: &Path,
    tx: mpsc::UnboundedSender<IpcCommand>,
) -> Result<()> {
    // 既存のソケットファイルを削除
    if socket_path.exists() {
        std::fs::remove_file(socket_path)?;
    }

    let listener = UnixListener::bind(socket_path)?;
    info!(?socket_path, "IPC server started");

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let tx = tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream, tx).await {
                        error!(?e, "Error handling IPC client");
                    }
                });
            }
            Err(e) => {
                error!(?e, "Error accepting IPC connection");
            }
        }
    }
}

/// クライアント接続を処理する
async fn handle_client(
    stream: UnixStream,
    tx: mpsc::UnboundedSender<IpcCommand>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    // 1行読み取り
    let n = reader.read_line(&mut line).await?;
    if n == 0 {
        return Ok(());
    }

    debug!(?line, "Received IPC request");

    // JSON パース
    let response = match serde_json::from_str::<IpcRequest>(&line) {
        Ok(request) => process_request(request, &tx),
        Err(e) => {
            warn!(?e, "Failed to parse IPC request");
            IpcResponse::err(format!("Invalid request: {}", e))
        }
    };

    // レスポンス送信
    let response_json = serde_json::to_string(&response)?;
    writer.write_all(response_json.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;

    Ok(())
}

/// IPC リクエストを処理する
fn process_request(
    request: IpcRequest,
    tx: &mpsc::UnboundedSender<IpcCommand>,
) -> IpcResponse {
    match request {
        IpcRequest::Reveal { path } => {
            if tx.send(IpcCommand::Reveal(path)).is_err() {
                IpcResponse::err("Failed to send command to main loop")
            } else {
                IpcResponse::ok()
            }
        }
        IpcRequest::Ping {} => IpcResponse::ok(),
        IpcRequest::Quit {} => {
            let _ = tx.send(IpcCommand::Quit);
            IpcResponse::ok()
        }
    }
}

/// IPC クライアント: コマンドを送信する
pub(crate) async fn send_command(socket_path: &Path, request: &IpcRequest) -> Result<IpcResponse> {
    let stream = UnixStream::connect(socket_path).await.map_err(|e| {
        AppError::Ipc(format!(
            "Failed to connect to {}: {}",
            socket_path.display(),
            e
        ))
    })?;

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // リクエスト送信
    let request_json = serde_json::to_string(request)?;
    writer.write_all(request_json.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;

    // レスポンス受信
    let mut line = String::new();
    reader.read_line(&mut line).await?;

    let response: IpcResponse = serde_json::from_str(&line)?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_key() {
        // Git リポジトリがない場合はディレクトリ名
        let key = workspace_key(Path::new("/tmp/test"));
        assert_eq!(key, "test");
    }

    #[test]
    fn test_ipc_request_serialize() {
        let request = IpcRequest::Reveal {
            path: PathBuf::from("/path/to/file"),
        };
        let json = serde_json::to_string(&request).ok();
        assert!(json.is_some());

        let ping = IpcRequest::Ping {};
        let json = serde_json::to_string(&ping).ok();
        assert!(json.is_some());
    }

    #[test]
    fn test_ipc_response() {
        let ok = IpcResponse::ok();
        assert!(ok.ok);
        assert!(ok.error.is_none());

        let err = IpcResponse::err("test error");
        assert!(!err.ok);
        assert_eq!(err.error.as_deref(), Some("test error"));
    }
}
