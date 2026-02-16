//! UDS server: connection handling, message dispatch, notification writer.

use std::path::{
    Path,
    PathBuf,
};
use std::sync::Arc;

use serde_json::Value;
use tokio::io::{
    AsyncBufReadExt,
    AsyncWriteExt,
    BufReader,
};
use tokio::net::unix::OwnedWriteHalf;
use tokio::net::UnixListener;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{
    Mutex,
    oneshot,
};
use tracing::{
    debug,
    info,
    warn,
};

use super::types::{
    IpcCommand,
    JsonRpcMessage,
    METHOD_NOT_FOUND,
    PARSE_ERROR,
};

/// Shared write half of a Unix socket connection.
type SharedWriter = Arc<Mutex<OwnedWriteHalf>>;

/// IPC server that listens on a Unix Domain Socket.
///
/// Handles JSON-RPC 2.0 messages: dispatches `ping` directly,
/// sends `reveal`/`quit` to the main loop via `mpsc` channel,
/// and manages a persistent client writer for outgoing notifications.
#[derive(Debug)]
pub struct IpcServer {
    /// Path to the Unix socket.
    socket_path: PathBuf,
    /// Writer for sending notifications to the most recently connected client.
    notification_writer: Arc<Mutex<Option<SharedWriter>>>,
}

impl IpcServer {
    /// Start the IPC server listening on the given socket path.
    ///
    /// Spawns a background task to accept connections. Commands are
    /// dispatched to the main event loop via `ipc_tx`.
    ///
    /// # Errors
    ///
    /// Returns an error if the socket cannot be bound.
    pub fn start_on_path(
        socket_path: PathBuf,
        ipc_tx: UnboundedSender<IpcCommand>,
    ) -> std::io::Result<Arc<Self>> {
        // Remove stale socket file from previous run
        let _ = std::fs::remove_file(&socket_path);

        // Ensure parent directory exists
        if let Some(parent) = socket_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let listener = UnixListener::bind(&socket_path)?;

        info!(path = %socket_path.display(), "IPC server listening");

        let notification_writer = Arc::new(Mutex::new(None));
        let server = Arc::new(Self {
            socket_path,
            notification_writer,
        });

        let writer_ref = server.notification_writer.clone();
        tokio::spawn(async move {
            accept_loop(listener, ipc_tx, writer_ref).await;
        });

        Ok(server)
    }

    /// Get the socket path.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Send a JSON-RPC notification to the connected client.
    ///
    /// If no client is connected, the notification is silently dropped.
    pub async fn send_notification(&self, method: &str, params: Value) {
        let msg = JsonRpcMessage::notification(method, params);
        let guard = self.notification_writer.lock().await;
        if let Some(writer) = guard.as_ref() {
            write_message(writer, &msg).await;
        } else {
            debug!(method, "No client connected, dropping notification");
        }
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
        super::paths::remove_meta(&self.socket_path);
    }
}

/// Accept loop: listens for new connections and spawns per-connection handlers.
async fn accept_loop(
    listener: UnixListener,
    ipc_tx: UnboundedSender<IpcCommand>,
    notification_writer: Arc<Mutex<Option<SharedWriter>>>,
) {
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                debug!("New IPC client connected");

                let (read_half, write_half) = stream.into_split();
                let writer = Arc::new(Mutex::new(write_half));

                // Store as the notification target
                *notification_writer.lock().await = Some(writer.clone());

                let ipc_tx = ipc_tx.clone();
                let nw = notification_writer.clone();
                tokio::spawn(async move {
                    handle_connection(read_half, writer, ipc_tx).await;
                    // On disconnect, clear notification writer if it's still ours
                    // (a newer connection may have replaced it)
                    debug!("IPC client disconnected");
                    let mut guard = nw.lock().await;
                    // We can't compare Arc pointers easily, so just clear it.
                    // A new connection will set a fresh writer.
                    *guard = None;
                });
            }
            Err(e) => {
                warn!(error = %e, "Failed to accept IPC connection");
            }
        }
    }
}

/// Handle a single client connection: read lines and dispatch JSON-RPC messages.
async fn handle_connection(
    read_half: tokio::net::unix::OwnedReadHalf,
    writer: SharedWriter,
    ipc_tx: UnboundedSender<IpcCommand>,
) {
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF — client disconnected
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                dispatch_message(trimmed, &writer, &ipc_tx).await;
            }
            Err(e) => {
                warn!(error = %e, "Error reading from IPC client");
                break;
            }
        }
    }
}

/// Parse and dispatch a single JSON-RPC message line.
async fn dispatch_message(
    line: &str,
    writer: &SharedWriter,
    ipc_tx: &UnboundedSender<IpcCommand>,
) {
    let msg: JsonRpcMessage = match serde_json::from_str(line) {
        Ok(m) => m,
        Err(e) => {
            warn!(error = %e, "Failed to parse JSON-RPC message");
            let error_resp =
                JsonRpcMessage::error_response(Value::Null, PARSE_ERROR, "Parse error");
            write_message(writer, &error_resp).await;
            return;
        }
    };

    match msg {
        JsonRpcMessage::Request {
            method, params, id, ..
        } => {
            dispatch_request(&method, params, id, writer, ipc_tx).await;
        }
        JsonRpcMessage::Notification {
            method, params, ..
        } => {
            dispatch_notification(&method, params.as_ref(), ipc_tx);
        }
        JsonRpcMessage::Response { .. } => {
            debug!("Received response from client (ignored)");
        }
    }
}

/// Dispatch a JSON-RPC request (has id, expects response).
async fn dispatch_request(
    method: &str,
    params: Option<Value>,
    id: Value,
    writer: &SharedWriter,
    ipc_tx: &UnboundedSender<IpcCommand>,
) {
    match method {
        "ping" => {
            let resp =
                JsonRpcMessage::success_response(id, serde_json::json!({"ok": true}));
            write_message(writer, &resp).await;
        }
        "reveal" => {
            let path = params
                .as_ref()
                .and_then(|p| p.get("path"))
                .and_then(Value::as_str)
                .map(PathBuf::from);

            if let Some(path) = path {
                let (tx, rx) = oneshot::channel();
                let cmd = IpcCommand::Reveal {
                    path,
                    response_tx: tx,
                };
                if ipc_tx.send(cmd).is_ok()
                    && let Ok(result) = rx.await
                {
                    let resp = JsonRpcMessage::success_response(id, result);
                    write_message(writer, &resp).await;
                }
            } else {
                let resp = JsonRpcMessage::error_response(
                    id,
                    super::types::INVALID_PARAMS,
                    "Missing 'path' parameter",
                );
                write_message(writer, &resp).await;
            }
        }
        "quit" => {
            let (tx, rx) = oneshot::channel();
            let cmd = IpcCommand::Quit { response_tx: tx };
            if ipc_tx.send(cmd).is_ok()
                && let Ok(result) = rx.await
            {
                let resp = JsonRpcMessage::success_response(id, result);
                write_message(writer, &resp).await;
            }
        }
        _ => {
            let resp = JsonRpcMessage::error_response(id, METHOD_NOT_FOUND, "Method not found");
            write_message(writer, &resp).await;
        }
    }
}

/// Dispatch a JSON-RPC notification (no id, no response).
fn dispatch_notification(
    method: &str,
    params: Option<&Value>,
    ipc_tx: &UnboundedSender<IpcCommand>,
) {
    match method {
        "reveal" => {
            let path = params
                .and_then(|p| p.get("path"))
                .and_then(Value::as_str)
                .map(PathBuf::from);

            if let Some(path) = path {
                // For notifications, we don't need a response — use a dummy channel
                let (tx, _rx) = oneshot::channel();
                let cmd = IpcCommand::Reveal {
                    path,
                    response_tx: tx,
                };
                let _ = ipc_tx.send(cmd);
            }
        }
        _ => {
            debug!(method, "Ignoring unknown notification");
        }
    }
}

/// Write a JSON-RPC message to the writer as a newline-delimited JSON line.
async fn write_message(writer: &SharedWriter, msg: &JsonRpcMessage) {
    if let Ok(mut serialized) = serde_json::to_string(msg) {
        serialized.push('\n');
        let mut guard = writer.lock().await;
        if let Err(e) = guard.write_all(serialized.as_bytes()).await {
            warn!(error = %e, "Failed to write IPC message");
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::sync::atomic::{
        AtomicU32,
        Ordering,
    };
    use std::time::Duration;

    use rstest::*;
    use serde_json::json;
    use tokio::sync::mpsc;

    use super::*;

    #[rstest]
    #[tokio::test]
    async fn ping_returns_ok_response() {
        let (ipc_tx, _ipc_rx) = mpsc::unbounded_channel::<IpcCommand>();
        let server = IpcServer::start_on_path(temp_socket_path(), ipc_tx).unwrap();

        let mut stream = tokio::net::UnixStream::connect(server.socket_path())
            .await
            .unwrap();
        let request = r#"{"jsonrpc":"2.0","method":"ping","id":1}"#;
        stream
            .write_all(format!("{request}\n").as_bytes())
            .await
            .unwrap();

        let mut reader = BufReader::new(&mut stream);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();

        assert_eq!(response["result"]["ok"], true);
        assert_eq!(response["id"], 1);
    }

    #[rstest]
    #[tokio::test]
    async fn quit_dispatches_command_and_responds() {
        let (ipc_tx, mut ipc_rx) = mpsc::unbounded_channel::<IpcCommand>();
        let server = IpcServer::start_on_path(temp_socket_path(), ipc_tx).unwrap();

        let mut stream = tokio::net::UnixStream::connect(server.socket_path())
            .await
            .unwrap();
        let request = r#"{"jsonrpc":"2.0","method":"quit","id":2}"#;
        stream
            .write_all(format!("{request}\n").as_bytes())
            .await
            .unwrap();

        // Receive the command on the main loop side
        let cmd = tokio::time::timeout(Duration::from_secs(1), ipc_rx.recv())
            .await
            .unwrap()
            .unwrap();

        // Respond via oneshot
        match cmd {
            IpcCommand::Quit { response_tx } => {
                response_tx.send(json!({"ok": true})).unwrap();
            }
            _ => panic!("Expected Quit command"),
        }

        // Read the JSON-RPC response
        let mut reader = BufReader::new(&mut stream);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();

        assert_eq!(response["result"]["ok"], true);
        assert_eq!(response["id"], 2);
    }

    #[rstest]
    #[tokio::test]
    async fn reveal_dispatches_command_with_path() {
        let (ipc_tx, mut ipc_rx) = mpsc::unbounded_channel::<IpcCommand>();
        let server = IpcServer::start_on_path(temp_socket_path(), ipc_tx).unwrap();

        let mut stream = tokio::net::UnixStream::connect(server.socket_path())
            .await
            .unwrap();
        let request =
            r#"{"jsonrpc":"2.0","method":"reveal","params":{"path":"/tmp/test.rs"},"id":3}"#;
        stream
            .write_all(format!("{request}\n").as_bytes())
            .await
            .unwrap();

        let cmd = tokio::time::timeout(Duration::from_secs(1), ipc_rx.recv())
            .await
            .unwrap()
            .unwrap();

        match cmd {
            IpcCommand::Reveal { path, response_tx } => {
                assert_eq!(path, PathBuf::from("/tmp/test.rs"));
                response_tx.send(json!({"ok": true})).unwrap();
            }
            _ => panic!("Expected Reveal command"),
        }

        let mut reader = BufReader::new(&mut stream);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();

        assert_eq!(response["result"]["ok"], true);
        assert_eq!(response["id"], 3);
    }

    #[rstest]
    #[tokio::test]
    async fn unknown_method_returns_error() {
        let (ipc_tx, _ipc_rx) = mpsc::unbounded_channel::<IpcCommand>();
        let server = IpcServer::start_on_path(temp_socket_path(), ipc_tx).unwrap();

        let mut stream = tokio::net::UnixStream::connect(server.socket_path())
            .await
            .unwrap();
        let request = r#"{"jsonrpc":"2.0","method":"unknown_method","id":4}"#;
        stream
            .write_all(format!("{request}\n").as_bytes())
            .await
            .unwrap();

        let mut reader = BufReader::new(&mut stream);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();

        assert_eq!(response["error"]["code"], -32601);
        assert_eq!(response["id"], 4);
    }

    #[rstest]
    #[tokio::test]
    async fn send_notification_to_connected_client() {
        let (ipc_tx, _ipc_rx) = mpsc::unbounded_channel::<IpcCommand>();
        let server = IpcServer::start_on_path(temp_socket_path(), ipc_tx).unwrap();

        let stream = tokio::net::UnixStream::connect(server.socket_path())
            .await
            .unwrap();

        // Wait for the server to register the client writer
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Send a notification from server to connected client
        server
            .send_notification("open_file", json!({"action": "edit", "path": "/tmp/foo.rs"}))
            .await;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let msg: Value = serde_json::from_str(&line).unwrap();

        assert_eq!(msg["method"], "open_file");
        assert_eq!(msg["params"]["action"], "edit");
        assert_eq!(msg["params"]["path"], "/tmp/foo.rs");
    }

    /// Create a unique temporary socket path for testing.
    fn temp_socket_path() -> PathBuf {
        /// Atomic counter for unique socket paths across parallel tests.
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join("trev-test");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(format!("test-{}-{n}.sock", std::process::id()))
    }
}
