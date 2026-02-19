//! IPC command handlers dispatched from the main event loop.

use crate::app::state::AppState;
use crate::ipc::types::IpcCommand;
use crate::tree::builder::TreeBuilder;

/// Handle an IPC command received from the IPC server.
///
/// Dispatches `Quit` and `Reveal` commands, sending a JSON-RPC response
/// back through the oneshot channel.
pub fn handle_ipc_command(cmd: IpcCommand, state: &mut AppState) {
    match cmd {
        IpcCommand::Quit { response_tx } => {
            state.should_quit = true;
            let _ = response_tx.send(serde_json::json!({"ok": true}));
        }
        IpcCommand::Reveal { path, response_tx } => {
            let builder = TreeBuilder::new(state.show_hidden, state.show_ignored);
            let found = state.tree_state.reveal_path(&path, builder);
            let _ = response_tx.send(serde_json::json!({"ok": found}));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use rstest::*;
    use serde_json::json;
    use tempfile::TempDir;
    use tokio::sync::oneshot;

    use crate::app::state::{
        AppState,
        ScrollState,
    };
    use crate::file_op::selection::SelectionBuffer;
    use crate::file_op::undo::UndoHistory;
    use crate::input::AppMode;
    use crate::ipc::types::IpcCommand;
    use crate::preview::cache::PreviewCache;
    use crate::preview::provider::PreviewRegistry;
    use crate::preview::providers::fallback::FallbackProvider;
    use crate::preview::state::PreviewState;
    use crate::state::tree::{
        TreeOptions,
        TreeState,
    };
    use crate::tree::builder::TreeBuilder;

    /// Create a minimal `AppState` for testing with a real filesystem tree.
    fn test_state(root: &std::path::Path) -> AppState {
        let builder = TreeBuilder::new(true, true);
        let root_node = builder.build(root).unwrap();
        let tree_state = TreeState::new(root_node, TreeOptions::default());
        let registry = PreviewRegistry::new(vec![
            Arc::new(FallbackProvider::new()),
        ])
        .unwrap();

        AppState {
            tree_state,
            preview_state: PreviewState::new(),
            preview_cache: PreviewCache::new(10),
            preview_registry: registry,
            mode: AppMode::default(),
            selection: SelectionBuffer::new(),
            undo_history: UndoHistory::new(10),
            watcher: None,
            should_quit: false,
            show_icons: false,
            show_preview: false,
            show_hidden: true,
            show_ignored: true,
            viewport_height: 20,
            scroll: ScrollState::new(),
            status_message: None,
            processing: false,
            emit_paths: None,
            git_state: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    #[rstest]
    fn quit_sets_should_quit_and_sends_response() {
        let tmp = TempDir::new().unwrap();
        let (tx, mut rx) = oneshot::channel();
        let cmd = IpcCommand::Quit { response_tx: tx };

        let mut state = test_state(tmp.path());
        super::handle_ipc_command(cmd, &mut state);

        assert!(state.should_quit);
        let response = rx.try_recv().unwrap();
        assert_eq!(response, json!({"ok": true}));
    }

    #[rstest]
    fn reveal_nonexistent_path_returns_false() {
        let tmp = TempDir::new().unwrap();
        let (tx, mut rx) = oneshot::channel();
        let cmd = IpcCommand::Reveal {
            path: PathBuf::from("/nonexistent/file.rs"),
            response_tx: tx,
        };

        let mut state = test_state(tmp.path());
        super::handle_ipc_command(cmd, &mut state);

        assert!(!state.should_quit);
        let response = rx.try_recv().unwrap();
        assert_eq!(response, json!({"ok": false}));
    }

    #[rstest]
    fn reveal_existing_file_moves_cursor() {
        let tmp = TempDir::new().unwrap();
        // Create: subdir/target.txt
        let subdir = tmp.path().join("subdir");
        std::fs::create_dir(&subdir).unwrap();
        let target = subdir.join("target.txt");
        std::fs::write(&target, "hello").unwrap();

        let (tx, mut rx) = oneshot::channel();
        let cmd = IpcCommand::Reveal {
            path: target.clone(),
            response_tx: tx,
        };

        let mut state = test_state(tmp.path());
        // Initially cursor is at 0, subdir is not expanded.
        assert_eq!(state.tree_state.cursor(), 0);

        super::handle_ipc_command(cmd, &mut state);

        let response = rx.try_recv().unwrap();
        assert_eq!(response, json!({"ok": true}));

        // Cursor should have moved to target.txt.
        let visible = state.tree_state.visible_nodes();
        let cursor = state.tree_state.cursor();
        let canonical_target = std::fs::canonicalize(&target).unwrap();
        assert_eq!(visible.get(cursor).unwrap().node.path, canonical_target);
    }
}
