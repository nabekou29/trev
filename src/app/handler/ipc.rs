//! IPC command handlers dispatched from the main event loop.

use crate::app::AppContext;
use crate::app::state::AppState;
use crate::ipc::types::IpcCommand;
use crate::tree::builder::TreeBuilder;

/// Handle an IPC command received from the IPC server.
///
/// Dispatches `Quit`, `Reveal`, `GetState`, and `Action` commands, sending a
/// JSON-RPC response back through the oneshot channel.
pub fn handle_ipc_command(cmd: IpcCommand, state: &mut AppState, ctx: &AppContext) {
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
        IpcCommand::GetState { response_tx } => {
            let _ = response_tx.send(build_state_json(state));
        }
        IpcCommand::Action { action, response_tx } => {
            super::dispatch_action(&action, state, ctx);
            let _ = response_tx.send(serde_json::json!({"ok": true}));
        }
    }
}

/// Build the JSON representation of the current application state.
fn build_state_json(state: &AppState) -> serde_json::Value {
    let preview = if state.show_preview {
        let area = state.layout_areas.preview_area;
        serde_json::json!({
            "path": state.preview_state.current_path.as_ref().map(|p| p.display().to_string()),
            "provider": state.preview_state.active_provider_name(),
            "x": area.x,
            "y": area.y,
            "width": area.width,
            "height": area.height,
            "scroll": state.preview_state.scroll_row,
        })
    } else {
        serde_json::json!({ "path": null })
    };

    let info = state.tree_state.current_node_info();
    let dir_str = info.as_ref().and_then(|i| {
        if i.is_dir {
            Some(i.path.display().to_string())
        } else {
            i.path.parent().map(|p| p.display().to_string())
        }
    });

    let cursor = serde_json::json!({
        "path": info.as_ref().map(|i| i.path.display().to_string()),
        "name": info.as_ref().map(|i| i.name.as_str()),
        "dir": dir_str,
        "is_dir": info.as_ref().is_some_and(|i| i.is_dir),
    });

    let mode = match state.mode {
        crate::input::AppMode::Normal => "normal",
        crate::input::AppMode::Input(_) => "input",
        crate::input::AppMode::Confirm(_) => "confirm",
        crate::input::AppMode::Menu(_) => "menu",
        crate::input::AppMode::Search(_) => "search",
        crate::input::AppMode::Help(_) => "help",
    };

    serde_json::json!({
        "preview": preview,
        "cursor": cursor,
        "root": state.tree_state.root_path().display().to_string(),
        "show_preview": state.show_preview,
        "show_hidden": state.show_hidden,
        "show_ignored": state.show_ignored,
        "mode": mode,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::sync::{
        Arc,
        RwLock,
    };

    use rstest::*;
    use serde_json::json;
    use tempfile::TempDir;
    use tokio::sync::oneshot;

    use crate::app::keymap::{
        ActionKeyLookup,
        KeyMap,
    };
    use crate::app::state::{
        AppContext,
        AppState,
        ScrollState,
    };
    use crate::config::KeybindingConfig;
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
    use crate::tree::search_index::SearchIndex;

    /// Create a minimal `AppContext` for testing.
    fn test_context(root: &std::path::Path) -> AppContext {
        let (children_tx, _) = tokio::sync::mpsc::channel(1);
        let (preview_tx, _) = tokio::sync::mpsc::channel(1);
        let (git_tx, _) = tokio::sync::mpsc::channel(1);
        let (rebuild_tx, _) = tokio::sync::mpsc::channel(1);
        let (search_index_ready_tx, _) = tokio::sync::mpsc::channel(1);
        let (stat_tx, _) = tokio::sync::mpsc::channel(1);
        let keymap = KeyMap::from_config(&KeybindingConfig::default(), &HashMap::new());
        let action_key_lookup = ActionKeyLookup::from_keymap(&keymap);
        AppContext {
            children_tx,
            preview_tx,
            preview_config: crate::config::PreviewConfig::default(),
            file_op_config: crate::config::FileOpConfig::default(),
            keymap,
            action_key_lookup,
            suppressed: Arc::new(AtomicBool::new(false)),
            ipc_server: None,
            git_tx,
            git_enabled: false,
            root_path: root.to_path_buf(),
            rebuild_tx,
            menus: HashMap::new(),
            search_index: Arc::new(RwLock::new(SearchIndex::new())),
            search_index_ready_tx,
            stat_tx,
            custom_actions: HashMap::new(),
        }
    }

    /// Create a minimal `AppState` for testing with a real filesystem tree.
    fn test_state(root: &std::path::Path) -> AppState {
        let builder = TreeBuilder::new(true, true);
        let root_node = builder.build(root).unwrap();
        let tree_state = TreeState::new(root_node, TreeOptions::default());
        let registry = PreviewRegistry::new(vec![Arc::new(FallbackProvider::new())]).unwrap();

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
            modal_avoid_preview: false,
            show_hidden: true,
            show_ignored: true,
            git_enabled: true,
            viewport_height: 20,
            scroll: ScrollState::new(),
            status_message: None,
            processing: false,
            git_state: Arc::new(RwLock::new(None)),
            rebuild_generation: 0,
            columns: crate::ui::column::resolve_columns(
                &crate::ui::column::default_columns(),
                &crate::ui::column::ColumnOptionsConfig::default(),
            ),
            layout_split_ratio: 50,
            layout_narrow_split_ratio: 60,
            layout_narrow_width: 80,
            pending_keys: crate::app::pending_keys::PendingKeys::new(
                std::time::Duration::from_millis(500),
            ),
            needs_redraw: false,
            dirty: true,
            file_style_matcher: crate::ui::file_style::FileStyleMatcher::new(
                &[],
                &crate::config::CategoryStyles::default(),
            )
            .unwrap(),
            preview_debounce: None,
            layout_areas: crate::app::LayoutAreas::default(),
            deferred_expansion: None,
            search_history: vec![],
            search_match_indices: HashMap::new(),
            search_pending_loads: None,
            search_index_cancelled: Arc::new(AtomicBool::new(false)),
            search_engine: crate::tree::search_engine::NucleoSearchEngine::new(Arc::new(|| {})),
        }
    }

    #[rstest]
    fn quit_sets_should_quit_and_sends_response() {
        let tmp = TempDir::new().unwrap();
        let (tx, mut rx) = oneshot::channel();
        let cmd = IpcCommand::Quit { response_tx: tx };

        let mut state = test_state(tmp.path());
        let ctx = test_context(tmp.path());
        super::handle_ipc_command(cmd, &mut state, &ctx);

        assert!(state.should_quit);
        let response = rx.try_recv().unwrap();
        assert_eq!(response, json!({"ok": true}));
    }

    #[rstest]
    fn reveal_nonexistent_path_returns_false() {
        let tmp = TempDir::new().unwrap();
        let (tx, mut rx) = oneshot::channel();
        let cmd =
            IpcCommand::Reveal { path: PathBuf::from("/nonexistent/file.rs"), response_tx: tx };

        let mut state = test_state(tmp.path());
        let ctx = test_context(tmp.path());
        super::handle_ipc_command(cmd, &mut state, &ctx);

        assert!(!state.should_quit);
        let response = rx.try_recv().unwrap();
        assert_eq!(response, json!({"ok": false}));
    }

    #[rstest]
    fn get_state_returns_expected_fields() {
        let tmp = TempDir::new().unwrap();
        let (tx, mut rx) = oneshot::channel();
        let cmd = IpcCommand::GetState { response_tx: tx };

        let mut state = test_state(tmp.path());
        let ctx = test_context(tmp.path());
        state.show_preview = false;
        state.show_hidden = true;
        state.show_ignored = false;
        super::handle_ipc_command(cmd, &mut state, &ctx);

        let response = rx.try_recv().unwrap();
        // Preview off → path is null.
        assert_eq!(response["preview"]["path"], json!(null));
        // Top-level flags.
        assert_eq!(response["show_preview"], false);
        assert_eq!(response["show_hidden"], true);
        assert_eq!(response["show_ignored"], false);
        assert_eq!(response["mode"], "normal");
        // Root matches the temp dir.
        assert!(response["root"].as_str().is_some());
        // Cursor object exists.
        assert!(response["cursor"].is_object());
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
        let cmd = IpcCommand::Reveal { path: target.clone(), response_tx: tx };

        let mut state = test_state(tmp.path());
        let ctx = test_context(tmp.path());
        // Initially cursor is at 0, subdir is not expanded.
        assert_eq!(state.tree_state.cursor(), 0);

        super::handle_ipc_command(cmd, &mut state, &ctx);

        let response = rx.try_recv().unwrap();
        assert_eq!(response, json!({"ok": true}));

        // Cursor should have moved to target.txt.
        let visible = state.tree_state.visible_nodes();
        let cursor = state.tree_state.cursor();
        let canonical_target = std::fs::canonicalize(&target).unwrap();
        assert_eq!(visible.get(cursor).unwrap().node.path, canonical_target);
    }
}
