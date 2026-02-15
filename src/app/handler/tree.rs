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
use crate::state::tree::TreeState;
use crate::tree::builder::TreeBuilder;

/// Handle a tree action.
pub fn handle_tree_action(
    action: &crate::action::TreeAction,
    state: &mut AppState,
    ctx: &AppContext,
) {
    use crate::action::TreeAction;

    match *action {
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
            state.tree_state.collapse();
        }
        TreeAction::ToggleExpand => {
            let show_hidden = state.show_hidden;
            let show_ignored = state.show_ignored;
            if let Some(result) = state.tree_state.toggle_expand(state.tree_state.cursor()) {
                handle_expand_result(
                    &result,
                    &mut state.tree_state,
                    ctx,
                    show_hidden,
                    show_ignored,
                );
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
    }
}

/// Handle an expand result: spawn loads or prefetch as appropriate.
///
/// Accepts `show_hidden`/`show_ignored` as separate parameters because this
/// function operates on a partial borrow of `TreeState` (not full `AppState`).
/// Callers should extract these values from `state` before the partial borrow.
fn handle_expand_result(
    result: &crate::state::tree::ExpandResult,
    tree_state: &mut TreeState,
    ctx: &AppContext,
    show_hidden: bool,
    show_ignored: bool,
) {
    use crate::state::tree::ExpandResult;

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
        let _ = tx
            .send(ChildrenLoadResult {
                path,
                children,
                prefetch,
            })
            .await;
    });
}

/// Refresh a directory in the tree by triggering a re-read of its children.
///
/// Keeps existing children visible during reload to avoid a visual flash
/// where the directory briefly appears collapsed.
pub fn refresh_directory(
    state: &AppState,
    dir: &Path,
    ctx: &AppContext,
) {
    // Spawn reload without invalidating: old children remain visible until
    // set_children() replaces them with the fresh listing.
    spawn_load_children(&ctx.children_tx, dir.to_path_buf(), state.show_hidden, state.show_ignored, false);
}
