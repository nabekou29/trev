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
            if let Some(result) = state.tree_state.expand_or_open() {
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
            state.tree_state.half_page_down(state.viewport_height as usize);
        }
        TreeAction::HalfPageUp => {
            state.tree_state.half_page_up(state.viewport_height as usize);
        }
        TreeAction::ExpandAll => {
            handle_expand_all(state, ctx);
        }
        TreeAction::CollapseAll => {
            handle_collapse_all(state);
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
fn handle_collapse_all(state: &mut AppState) {
    let Some(dir_path) = state.tree_state.cursor_dir_path() else {
        return;
    };
    let collapsed = state.tree_state.collapse_subtree(&dir_path);

    // Unwatch collapsed directories.
    for path in &collapsed {
        unwatch_dir(path, &mut state.watcher);
    }

    if !collapsed.is_empty() {
        state.set_status(format!("Collapsed {} directories", collapsed.len()));
    }
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
        ExpandResult::OpenFile(_) => {
            // File opening not implemented yet.
        }
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
