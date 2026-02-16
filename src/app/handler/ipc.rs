//! IPC command handlers dispatched from the main event loop.

use crate::ipc::types::IpcCommand;

/// Handle an IPC command received from the IPC server.
///
/// Dispatches `Quit` and `Reveal` commands, sending a JSON-RPC response
/// back through the oneshot channel.
pub fn handle_ipc_command(cmd: IpcCommand, should_quit: &mut bool) {
    match cmd {
        IpcCommand::Quit { response_tx } => {
            *should_quit = true;
            let _ = response_tx.send(serde_json::json!({"ok": true}));
        }
        IpcCommand::Reveal { response_tx, .. } => {
            // Placeholder — full tree navigation in US2.
            let _ = response_tx.send(serde_json::json!({"ok": true}));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::path::PathBuf;

    use rstest::*;
    use serde_json::json;
    use tokio::sync::oneshot;

    use crate::ipc::types::IpcCommand;

    #[rstest]
    fn quit_sets_should_quit_and_sends_response() {
        let (tx, mut rx) = oneshot::channel();
        let cmd = IpcCommand::Quit { response_tx: tx };

        let mut should_quit = false;
        super::handle_ipc_command(cmd, &mut should_quit);

        assert!(should_quit);
        let response = rx.try_recv().unwrap();
        assert_eq!(response, json!({"ok": true}));
    }

    #[rstest]
    fn reveal_sends_response_without_affecting_quit() {
        let (tx, mut rx) = oneshot::channel();
        let cmd = IpcCommand::Reveal {
            path: PathBuf::from("/tmp/test.rs"),
            response_tx: tx,
        };

        let mut should_quit = false;
        super::handle_ipc_command(cmd, &mut should_quit);

        assert!(!should_quit);
        let response = rx.try_recv().unwrap();
        assert_eq!(response, json!({"ok": true}));
    }
}
