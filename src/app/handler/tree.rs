//! Tree navigation action handlers.
//!
//! Handles expand, collapse, cursor movement, prefetch, and directory reload.

use std::path::{
    Path,
    PathBuf,
};

use crate::app::state::{
    AppContext,
    AppState,
    ChildrenLoadResult,
};
use crate::state::tree::{
    ExpandResult,
    TreeState,
};
use crate::tree::builder::TreeBuilder;

/// Handle a tree action.
pub fn handle_tree_action(
    action: crate::action::TreeAction,
    state: &mut AppState,
    ctx: &AppContext,
) {
    use crate::action::TreeAction;

    match action {
        TreeAction::MoveDown => {
            state.tree_state.move_cursor(1);
        }
        TreeAction::MoveUp => {
            state.tree_state.move_cursor(-1);
        }
        TreeAction::Expand => {
            let show_hidden = state.show_hidden;
            let show_ignored = state.show_ignored;
            if let Some(result) = state.tree_state.expand_dir() {
                // Watch newly expanded directory.
                watch_if_expand(&result, &mut state.watcher);
                handle_expand_result(
                    &result,
                    &mut state.tree_state,
                    ctx,
                    show_hidden,
                    show_ignored,
                );
            }
        }
        TreeAction::Collapse => {
            // Check if current node is an expanded directory (will be collapsed).
            let visible = state.tree_state.visible_nodes();
            let cursor = state.tree_state.cursor();
            let collapse_path = visible
                .get(cursor)
                .filter(|vn| vn.node.is_dir && vn.node.is_expanded)
                .map(|vn| vn.node.path.clone());
            drop(visible);

            state.tree_state.collapse();

            // Unwatch the collapsed directory.
            if let Some(path) = collapse_path {
                unwatch_dir(&path, &mut state.watcher);
            }
        }
        TreeAction::ToggleExpand => {
            let show_hidden = state.show_hidden;
            let show_ignored = state.show_ignored;
            let cursor = state.tree_state.cursor();

            // Capture collapse path before toggle (toggle may collapse expanded dir).
            let visible = state.tree_state.visible_nodes();
            let collapse_path = visible
                .get(cursor)
                .filter(|vn| vn.node.is_dir && vn.node.is_expanded)
                .map(|vn| vn.node.path.clone());
            drop(visible);

            if let Some(result) = state.tree_state.toggle_expand(cursor) {
                // Expanded — watch.
                watch_if_expand(&result, &mut state.watcher);
                handle_expand_result(
                    &result,
                    &mut state.tree_state,
                    ctx,
                    show_hidden,
                    show_ignored,
                );
                // Emit mode: accumulate file path and quit.
                if let ExpandResult::OpenFile(ref path) = result {
                    handle_emit_open(path, state);
                }
            } else if let Some(path) = collapse_path {
                // Collapsed — unwatch.
                unwatch_dir(&path, &mut state.watcher);
            }
        }
        TreeAction::JumpFirst => {
            state.tree_state.jump_to_first();
        }
        TreeAction::JumpLast => {
            state.tree_state.jump_to_last();
        }
        TreeAction::HalfPageDown => {
            state.tree_state.half_page_down(state.viewport_height);
        }
        TreeAction::HalfPageUp => {
            state.tree_state.half_page_up(state.viewport_height);
        }
        TreeAction::ExpandAll => {
            handle_expand_all(state, ctx);
        }
        TreeAction::CollapseAll => {
            handle_collapse_all(state);
        }
        TreeAction::ToggleHidden => {
            state.show_hidden = !state.show_hidden;
            rebuild_tree(state, ctx);
            let label = if state.show_hidden { "shown" } else { "hidden" };
            state.set_status(format!("Hidden files: {label}"));
        }
        TreeAction::ToggleIgnored => {
            state.show_ignored = !state.show_ignored;
            rebuild_tree(state, ctx);
            let label = if state.show_ignored { "shown" } else { "hidden" };
            state.set_status(format!("Ignored files: {label}"));
        }
        TreeAction::Refresh => {
            rebuild_tree(state, ctx);
            // Re-fetch git status.
            if ctx.git_enabled {
                crate::app::trigger_git_status(ctx);
            }
            state.set_status("Refreshed");
        }
    }
}

/// Maximum number of directories to expand in a single expand-all operation.
const EXPAND_ALL_LIMIT: usize = 300;

/// Handle expand-all: recursively expand the subtree under cursor.
fn handle_expand_all(state: &mut AppState, ctx: &AppContext) {
    let Some(dir_path) = state.tree_state.cursor_dir_path() else {
        return;
    };
    let show_hidden = state.show_hidden;
    let show_ignored = state.show_ignored;
    let result = state.tree_state.expand_subtree(&dir_path, EXPAND_ALL_LIMIT);

    // Spawn loads and watch directories that need loading.
    for path in &result.needs_load {
        spawn_load_children(&ctx.children_tx, path.clone(), show_hidden, show_ignored, false);
        watch_dir(path, &mut state.watcher);
    }

    // Status message.
    if result.hit_limit {
        state.set_status(format!(
            "Expanded {} directories (limit: {EXPAND_ALL_LIMIT})",
            result.expanded
        ));
    } else if result.expanded > 0 {
        state.set_status(format!("Expanded {} directories", result.expanded));
    }
}

/// Handle collapse-all: collapse the subtree under cursor.
///
/// If the cursor is on a file or a collapsed directory, collapses the parent
/// directory instead. Moves the cursor to the collapsed directory afterwards.
fn handle_collapse_all(state: &mut AppState) {
    let Some(info) = state.tree_state.current_node_info() else {
        return;
    };

    // Determine which directory to collapse:
    // - Expanded directory → collapse it directly
    // - File or collapsed directory → collapse the parent
    let is_expanded_dir = info.is_dir
        && state
            .tree_state
            .visible_nodes()
            .get(state.tree_state.cursor())
            .is_some_and(|vn| vn.node.is_expanded);

    let dir_path = if is_expanded_dir {
        info.path
    } else {
        let Some(parent) = info.path.parent() else {
            return;
        };
        parent.to_path_buf()
    };

    let collapsed = state.tree_state.collapse_subtree(&dir_path);

    // Move cursor to the collapsed directory.
    state.tree_state.move_cursor_to_path(&dir_path);

    // Unwatch collapsed directories.
    for path in &collapsed {
        unwatch_dir(path, &mut state.watcher);
    }

    if !collapsed.is_empty() {
        state.set_status(format!("Collapsed {} directories", collapsed.len()));
    }
}

/// Rebuild the entire tree with the current `show_hidden` / `show_ignored` settings.
///
/// Re-expands previously expanded directories and restores the cursor position.
fn rebuild_tree(state: &mut AppState, ctx: &AppContext) {
    let expanded = state.tree_state.expanded_paths();
    let cursor_path = state.tree_state.cursor_path();
    let order = state.tree_state.sort_order();
    let direction = state.tree_state.sort_direction();
    let dirs_first = state.tree_state.directories_first();
    let root_path = state.tree_state.root_path().to_path_buf();

    let builder = TreeBuilder::new(state.show_hidden, state.show_ignored);
    let Ok(root) = builder.build(&root_path) else {
        return;
    };

    let options = crate::state::tree::TreeOptions {
        sort_order: order,
        sort_direction: direction,
        directories_first: dirs_first,
        show_root: state.tree_state.show_root(),
    };
    let mut new_tree = TreeState::new(root, options);

    // Re-expand directories (shortest paths first so parents load before children).
    let mut sorted_expanded = expanded;
    sorted_expanded.sort_by_key(|p| p.as_os_str().len());
    for path in &sorted_expanded {
        if let Ok(children) = builder.load_children(path) {
            new_tree.set_children(path, children, true);
        }
    }

    new_tree.apply_sort(order, direction, dirs_first);

    // Restore cursor position.
    if let Some(ref cp) = cursor_path {
        new_tree.move_cursor_to_path(cp);
    }

    state.tree_state = new_tree;

    // Re-trigger prefetch for visible directories.
    trigger_prefetch(
        &mut state.tree_state,
        &root_path,
        ctx,
        state.show_hidden,
        state.show_ignored,
    );
}

/// Watch a directory if the expand result indicates expansion.
fn watch_if_expand(result: &ExpandResult, watcher: &mut Option<crate::watcher::FsWatcher>) {
    match result {
        ExpandResult::NeedsLoad(path) | ExpandResult::AlreadyLoaded(path) => {
            if let Some(w) = watcher
                && let Err(e) = w.watch(path)
            {
                tracing::warn!(%e, ?path, "failed to watch directory");
            }
        }
        ExpandResult::OpenFile(_) => {}
    }
}

/// Watch a directory.
fn watch_dir(path: &Path, watcher: &mut Option<crate::watcher::FsWatcher>) {
    if let Some(w) = watcher
        && let Err(e) = w.watch(path)
    {
        tracing::warn!(%e, ?path, "failed to watch directory");
    }
}

/// Unwatch a directory.
fn unwatch_dir(path: &Path, watcher: &mut Option<crate::watcher::FsWatcher>) {
    if let Some(w) = watcher
        && let Err(e) = w.unwatch(path)
    {
        tracing::warn!(%e, ?path, "failed to unwatch directory");
    }
}

/// Handle an expand result: spawn loads or prefetch as appropriate.
///
/// Accepts `show_hidden`/`show_ignored` as separate parameters because this
/// function operates on a partial borrow of `TreeState` (not full `AppState`).
/// Callers should extract these values from `state` before the partial borrow.
fn handle_expand_result(
    result: &ExpandResult,
    tree_state: &mut TreeState,
    ctx: &AppContext,
    show_hidden: bool,
    show_ignored: bool,
) {
    match *result {
        ExpandResult::NeedsLoad(ref path) => {
            spawn_load_children(&ctx.children_tx, path.clone(), show_hidden, show_ignored, false);
        }
        ExpandResult::AlreadyLoaded(ref path) => {
            // Directory was prefetched — trigger prefetch for its children.
            trigger_prefetch(tree_state, path, ctx, show_hidden, show_ignored);
        }
        ExpandResult::OpenFile(ref path) => {
            send_open_file_notification(ctx, path);
        }
    }
}

/// Handle file open in emit mode: accumulate the path and quit.
fn handle_emit_open(path: &Path, state: &mut AppState) {
    if let Some(ref mut paths) = state.emit_paths {
        paths.push(path.to_path_buf());
        state.should_quit = true;
    }
}

/// Send an `open_file` notification to the connected Neovim client.
///
/// Only sends if the IPC server is running (daemon mode). The notification
/// includes the configured `EditorAction` and the file path.
fn send_open_file_notification(ctx: &AppContext, path: &Path) {
    if let Some(server) = &ctx.ipc_server {
        let server = server.clone();
        let action = ctx.editor_action;
        let path = path.to_path_buf();
        tokio::spawn(async move {
            server
                .send_notification(
                    "open_file",
                    serde_json::json!({"action": action, "path": path.to_string_lossy()}),
                )
                .await;
        });
    }
}

/// Prefetch child directories one level ahead.
///
/// Transitions `NotLoaded` child directories to `Loading` and spawns
/// background tasks to load their children.
///
/// Accepts `show_hidden`/`show_ignored` as separate parameters because this
/// function operates on a partial borrow of `TreeState` (not full `AppState`).
pub fn trigger_prefetch(
    tree_state: &mut TreeState,
    parent_path: &Path,
    ctx: &AppContext,
    show_hidden: bool,
    show_ignored: bool,
) {
    let prefetch_paths = tree_state.start_prefetch(parent_path);
    for path in prefetch_paths {
        spawn_load_children(&ctx.children_tx, path, show_hidden, show_ignored, true);
    }
}

/// Spawn a blocking task to load directory children asynchronously.
fn spawn_load_children(
    tx: &tokio::sync::mpsc::Sender<ChildrenLoadResult>,
    path: PathBuf,
    show_hidden: bool,
    show_ignored: bool,
    prefetch: bool,
) {
    let tx = tx.clone();
    tokio::spawn(async move {
        let load_path = path.clone();
        let result = tokio::task::spawn_blocking(move || {
            let builder = TreeBuilder::new(show_hidden, show_ignored);
            builder.load_children(&load_path)
        })
        .await;

        let children = match result {
            Ok(Ok(children)) => Ok(children),
            Ok(Err(err)) => Err(err.to_string()),
            Err(err) => Err(err.to_string()),
        };

        // Ignore send error (receiver dropped = app is shutting down).
        let _ = tx.send(ChildrenLoadResult { path, children, prefetch }).await;
    });
}

/// Refresh a directory in the tree by triggering a re-read of its children.
///
/// Keeps existing children visible during reload to avoid a visual flash
/// where the directory briefly appears collapsed.
pub fn refresh_directory(state: &AppState, dir: &Path, ctx: &AppContext) {
    // Spawn reload without invalidating: old children remain visible until
    // set_children() replaces them with the fresh listing.
    spawn_load_children(
        &ctx.children_tx,
        dir.to_path_buf(),
        state.show_hidden,
        state.show_ignored,
        false,
    );
}
