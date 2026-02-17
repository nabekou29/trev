//! JSON-RPC 2.0 message types and IPC command definitions.

use std::path::PathBuf;

use serde::{
    Deserialize,
    Serialize,
};
use serde_json::Value;
use tokio::sync::oneshot;

/// JSON-RPC 2.0 error code: Parse error.
pub const PARSE_ERROR: i64 = -32700;

/// JSON-RPC 2.0 error code: Method not found.
pub const METHOD_NOT_FOUND: i64 = -32601;

/// JSON-RPC 2.0 error code: Invalid params.
pub const INVALID_PARAMS: i64 = -32602;

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonRpcError {
    /// Error code.
    pub code: i64,
    /// Error message.
    pub message: String,
}

/// JSON-RPC 2.0 message (request, notification, or response).
///
/// Uses serde untagged to discriminate:
/// - Request: has `id` + `method`
/// - Notification: has `method` only (no `id`)
/// - Response: has `id` + `result`/`error`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    /// Request with an id (expects a response).
    Request {
        /// JSON-RPC version (must be "2.0").
        jsonrpc: String,
        /// Method name.
        method: String,
        /// Optional parameters.
        #[serde(skip_serializing_if = "Option::is_none")]
        params: Option<Value>,
        /// Request ID.
        id: Value,
    },
    /// Notification without an id (no response expected).
    Notification {
        /// JSON-RPC version.
        jsonrpc: String,
        /// Method name.
        method: String,
        /// Optional parameters.
        #[serde(skip_serializing_if = "Option::is_none")]
        params: Option<Value>,
    },
    /// Response to a request.
    Response {
        /// JSON-RPC version.
        jsonrpc: String,
        /// Success result (mutually exclusive with `error`).
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<Value>,
        /// Error object (mutually exclusive with `result`).
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<JsonRpcError>,
        /// ID of the request this responds to.
        id: Value,
    },
}

impl JsonRpcMessage {
    /// Create a success response.
    pub fn success_response(id: Value, result: Value) -> Self {
        Self::Response {
            jsonrpc: "2.0".to_owned(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create an error response.
    pub fn error_response(id: Value, code: i64, message: &str) -> Self {
        Self::Response {
            jsonrpc: "2.0".to_owned(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_owned(),
            }),
            id,
        }
    }

    /// Create a notification (no response expected).
    pub fn notification(method: &str, params: Value) -> Self {
        Self::Notification {
            jsonrpc: "2.0".to_owned(),
            method: method.to_owned(),
            params: Some(params),
        }
    }
}

/// Internal command dispatched from IPC server to the main event loop.
///
/// Each variant includes a `oneshot` response channel so the server
/// can send the JSON-RPC response back to the client after processing.
#[derive(Debug)]
pub enum IpcCommand {
    /// Reveal a file in the tree.
    Reveal {
        /// Absolute path to the file to reveal.
        path: PathBuf,
        /// Channel to send the JSON-RPC result value back.
        response_tx: oneshot::Sender<Value>,
    },
    /// Quit the application.
    Quit {
        /// Channel to send the JSON-RPC result value back.
        response_tx: oneshot::Sender<Value>,
    },
}

/// Action for opening files in the editor.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EditorAction {
    /// Open in current window.
    Edit,
    /// Open in horizontal split.
    Split,
    /// Open in vertical split.
    Vsplit,
    /// Open in new tab.
    Tabedit,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;
    use serde_json::json;

    use super::*;

    // --- JsonRpcMessage deserialization ---

    #[rstest]
    fn deserialize_request() {
        let json_str = r#"{"jsonrpc":"2.0","method":"ping","id":1}"#;
        let msg: JsonRpcMessage = serde_json::from_str(json_str).unwrap();
        assert_eq!(
            msg,
            JsonRpcMessage::Request {
                jsonrpc: "2.0".to_owned(),
                method: "ping".to_owned(),
                params: None,
                id: json!(1),
            }
        );
    }

    #[rstest]
    fn deserialize_request_with_params() {
        let json_str =
            r#"{"jsonrpc":"2.0","method":"reveal","params":{"path":"/tmp/foo"},"id":2}"#;
        let msg: JsonRpcMessage = serde_json::from_str(json_str).unwrap();
        assert_eq!(
            msg,
            JsonRpcMessage::Request {
                jsonrpc: "2.0".to_owned(),
                method: "reveal".to_owned(),
                params: Some(json!({"path": "/tmp/foo"})),
                id: json!(2),
            }
        );
    }

    #[rstest]
    fn deserialize_notification() {
        let json_str = r#"{"jsonrpc":"2.0","method":"open_file","params":{"action":"edit","path":"/tmp/foo"}}"#;
        let msg: JsonRpcMessage = serde_json::from_str(json_str).unwrap();
        assert_eq!(
            msg,
            JsonRpcMessage::Notification {
                jsonrpc: "2.0".to_owned(),
                method: "open_file".to_owned(),
                params: Some(json!({"action": "edit", "path": "/tmp/foo"})),
            }
        );
    }

    #[rstest]
    fn deserialize_success_response() {
        let json_str = r#"{"jsonrpc":"2.0","result":{"ok":true},"id":1}"#;
        let msg: JsonRpcMessage = serde_json::from_str(json_str).unwrap();
        assert_eq!(
            msg,
            JsonRpcMessage::Response {
                jsonrpc: "2.0".to_owned(),
                result: Some(json!({"ok": true})),
                error: None,
                id: json!(1),
            }
        );
    }

    #[rstest]
    fn deserialize_error_response() {
        let json_str =
            r#"{"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found"},"id":1}"#;
        let msg: JsonRpcMessage = serde_json::from_str(json_str).unwrap();
        assert_eq!(
            msg,
            JsonRpcMessage::Response {
                jsonrpc: "2.0".to_owned(),
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: "Method not found".to_owned(),
                }),
                id: json!(1),
            }
        );
    }

    // --- JsonRpcMessage serialization ---

    #[rstest]
    fn serialize_success_response() {
        let msg = JsonRpcMessage::success_response(json!(1), json!({"ok": true}));
        let serialized = serde_json::to_string(&msg).unwrap();
        let parsed: Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["result"], json!({"ok": true}));
        assert_eq!(parsed["id"], 1);
    }

    #[rstest]
    fn serialize_error_response() {
        let msg = JsonRpcMessage::error_response(json!(1), METHOD_NOT_FOUND, "Method not found");
        let serialized = serde_json::to_string(&msg).unwrap();
        let parsed: Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["error"]["code"], -32601);
        assert_eq!(parsed["error"]["message"], "Method not found");
    }

    #[rstest]
    fn serialize_notification() {
        let msg =
            JsonRpcMessage::notification("open_file", json!({"action": "edit", "path": "/foo"}));
        let serialized = serde_json::to_string(&msg).unwrap();
        let parsed: Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed["method"], "open_file");
        assert_eq!(parsed["params"]["action"], "edit");
        // Notification must NOT have id
        assert_that!(parsed.get("id"), none());
    }

    // --- EditorAction serialization ---

    #[rstest]
    #[case(EditorAction::Edit, "edit")]
    #[case(EditorAction::Split, "split")]
    #[case(EditorAction::Vsplit, "vsplit")]
    #[case(EditorAction::Tabedit, "tabedit")]
    fn serialize_editor_action(#[case] action: EditorAction, #[case] expected: &str) {
        let serialized = serde_json::to_string(&action).unwrap();
        assert_eq!(serialized, format!("\"{expected}\""));
    }
}
