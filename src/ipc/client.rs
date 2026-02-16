//! UDS client for `trev ctl` commands.
//!
//! Connects to a running trev daemon via Unix Domain Socket,
//! sends JSON-RPC 2.0 requests, and reads responses.

use std::path::Path;
use std::time::Duration;

use anyhow::{
    Context,
    Result,
    bail,
};
use serde_json::Value;
use tokio::io::{
    AsyncBufReadExt,
    AsyncWriteExt,
    BufReader,
};
use tokio::net::UnixStream;

/// Send a JSON-RPC 2.0 request to a trev daemon and wait for the response.
///
/// # Errors
///
/// Returns an error if the connection fails, the write fails,
/// or the response is not received within the timeout.
pub async fn send_request(
    socket_path: &Path,
    method: &str,
    params: Option<Value>,
    timeout: Duration,
) -> Result<Value> {
    let stream = UnixStream::connect(socket_path)
        .await
        .with_context(|| format!("failed to connect to {}", socket_path.display()))?;

    let (read_half, mut write_half) = stream.into_split();

    // Build JSON-RPC request.
    let request = params.map_or_else(
        || {
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": method,
                "id": 1
            })
        },
        |p| {
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": method,
                "params": p,
                "id": 1
            })
        },
    );

    let mut serialized = serde_json::to_string(&request)?;
    serialized.push('\n');
    write_half
        .write_all(serialized.as_bytes())
        .await
        .context("failed to write request")?;

    // Read response with timeout.
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    tokio::time::timeout(timeout, reader.read_line(&mut line))
        .await
        .context("response timed out")?
        .context("failed to read response")?;

    if line.trim().is_empty() {
        bail!("empty response from server");
    }

    let response: Value = serde_json::from_str(line.trim())?;
    Ok(response)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{
        AtomicU32,
        Ordering,
    };
    use std::time::Duration;

    use rstest::*;
    use serde_json::json;
    use tokio::sync::mpsc;

    use crate::ipc::server::IpcServer;
    use crate::ipc::types::IpcCommand;

    #[rstest]
    #[tokio::test]
    async fn send_request_ping() {
        let (ipc_tx, _ipc_rx) = mpsc::unbounded_channel::<IpcCommand>();
        let server = IpcServer::start_on_path(temp_socket_path(), ipc_tx).unwrap();

        let response = super::send_request(
            server.socket_path(),
            "ping",
            None,
            Duration::from_secs(2),
        )
        .await
        .unwrap();

        assert_eq!(response["result"]["ok"], true);
    }

    #[rstest]
    #[tokio::test]
    async fn send_request_with_params() {
        let (ipc_tx, mut ipc_rx) = mpsc::unbounded_channel::<IpcCommand>();
        let server = IpcServer::start_on_path(temp_socket_path(), ipc_tx).unwrap();

        // Spawn a task to handle the reveal command.
        tokio::spawn(async move {
            if let Some(IpcCommand::Reveal { response_tx, .. }) = ipc_rx.recv().await {
                let _ = response_tx.send(json!({"ok": true}));
            }
        });

        let response = super::send_request(
            server.socket_path(),
            "reveal",
            Some(json!({"path": "/tmp/test.rs"})),
            Duration::from_secs(2),
        )
        .await
        .unwrap();

        assert_eq!(response["result"]["ok"], true);
    }

    #[rstest]
    #[tokio::test]
    async fn send_request_timeout_on_no_response() {
        let (ipc_tx, _ipc_rx) = mpsc::unbounded_channel::<IpcCommand>();
        let server = IpcServer::start_on_path(temp_socket_path(), ipc_tx).unwrap();

        // Send a quit request but don't respond — should timeout.
        let result = super::send_request(
            server.socket_path(),
            "quit",
            None,
            Duration::from_millis(100),
        )
        .await;

        assert!(result.is_err());
    }

    #[rstest]
    #[tokio::test]
    async fn send_request_connection_refused() {
        let result = super::send_request(
            std::path::Path::new("/tmp/trev-nonexistent-socket.sock"),
            "ping",
            None,
            Duration::from_secs(1),
        )
        .await;

        assert!(result.is_err());
    }

    /// Create a unique temporary socket path for testing.
    fn temp_socket_path() -> PathBuf {
        /// Atomic counter for unique socket paths across parallel tests.
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join("trev-test");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(format!("client-test-{}-{n}.sock", std::process::id()))
    }
}
