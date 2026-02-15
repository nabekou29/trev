//! File operation action handlers.
//!
//! Handles mark, create, rename, yank, cut, paste, delete, trash, undo, and redo operations.

use std::collections::HashSet;
use std::path::{
    Path,
    PathBuf,
};

use super::tree::refresh_directory;
use crate::app::state::{
    AppContext,
    AppState,
};
use crate::file_op::executor::{
    FsOp,
    execute,
};
use crate::file_op::selection::SelectionMode;
use crate::file_op::undo::{
    OpGroup,
    UndoOp,
    build_undo_op,
    reverse_for_copy,
};
use crate::input::AppMode;

/// Handle a file operation action.
pub fn handle_file_op_action(
    action: &crate::action::FileOpAction,
    state: &mut AppState,
    ctx: &AppContext,
) {
    use crate::action::FileOpAction;
    use crate::input::{
        ConfirmAction,
        ConfirmState,
        InputState,
    };

    match *action {
        FileOpAction::ToggleMark => {
            if let Some(info) = state.tree_state.current_node_info() {
                state.selection.toggle_mark(info.path);
                // Move cursor down after toggling mark.
                state.tree_state.move_cursor(1);
            }
        }
        FileOpAction::ClearSelections => {
            state.selection.clear();
        }
        FileOpAction::CreateFile => {
            if let Some(info) = state.tree_state.current_node_info() {
                // If cursor is on a directory, create inside it. Otherwise, use parent.
                let parent_dir = if info.is_dir {
                    info.path
                } else {
                    info.path.parent().map_or_else(|| info.path.clone(), Path::to_path_buf)
                };
                state.mode = AppMode::Input(InputState::for_create(parent_dir));
            }
        }
        FileOpAction::Rename => {
            if let Some(info) = state.tree_state.current_node_info() {
                state.mode = AppMode::Input(InputState::for_rename(info.path));
            }
        }
        FileOpAction::Yank => {
            if let Some(info) = state.tree_state.current_node_info() {
                let targets = state.selection.mark_targets_or_cursor(&info.path);
                let count = targets.len();
                state.selection.set(targets, SelectionMode::Copy);
                state.set_status(format!("Yanked {count} file(s)"));
            }
        }
        FileOpAction::Cut => {
            if let Some(info) = state.tree_state.current_node_info() {
                let targets = state.selection.mark_targets_or_cursor(&info.path);
                let count = targets.len();
                state.selection.set(targets, SelectionMode::Cut);
                state.set_status(format!("Cut {count} file(s)"));
            }
        }
        FileOpAction::Paste => {
            execute_paste(state, ctx);
        }
        FileOpAction::Delete => {
            if let Some(info) = state.tree_state.current_node_info() {
                let targets = state.selection.mark_targets_or_cursor(&info.path);
                let confirm_action = match ctx.file_op_config.delete_mode {
                    crate::config::DeleteMode::Permanent => ConfirmAction::PermanentDelete,
                    crate::config::DeleteMode::CustomTrash => ConfirmAction::CustomTrash,
                };
                let count = targets.len();
                state.mode = AppMode::Confirm(ConfirmState {
                    message: format!("Delete {count} item(s)?"),
                    paths: targets,
                    on_confirm: confirm_action,
                });
            }
        }
        FileOpAction::SystemTrash => {
            if let Some(info) = state.tree_state.current_node_info() {
                let targets = state.selection.mark_targets_or_cursor(&info.path);
                let count = targets.len();
                state.mode = AppMode::Confirm(ConfirmState {
                    message: format!("Move {count} item(s) to system trash?"),
                    paths: targets,
                    on_confirm: ConfirmAction::SystemTrash,
                });
            }
        }
        FileOpAction::Undo => {
            execute_undo(state, ctx);
        }
        FileOpAction::Redo => {
            execute_redo(state, ctx);
        }
    }
}

/// Execute the confirmed delete operation.
pub fn execute_delete(confirm: crate::input::ConfirmState, state: &mut AppState, ctx: &AppContext) {
    use crate::input::ConfirmAction;

    let mut affected_parents: HashSet<PathBuf> = HashSet::new();
    let mut undo_ops: Vec<UndoOp> = Vec::new();
    let crate::input::ConfirmState { paths, on_confirm, .. } = confirm;
    let delete_count = paths.len();
    let is_system_trash = matches!(on_confirm, ConfirmAction::SystemTrash);

    match on_confirm {
        ConfirmAction::PermanentDelete => {
            for path in paths {
                let op = if path.is_dir() {
                    FsOp::RemoveDir { path: path.clone() }
                } else {
                    FsOp::RemoveFile { path: path.clone() }
                };
                if let Err(e) = execute(&op) {
                    tracing::error!(%e, ?path, "permanent delete failed");
                    continue;
                }
                if let Some(parent) = path.parent() {
                    affected_parents.insert(parent.to_path_buf());
                }
                // Permanent delete cannot be undone — do not add to undo_ops.
            }
        }
        ConfirmAction::CustomTrash => {
            if let Err(e) = crate::file_op::trash::ensure_trash_dir() {
                tracing::error!(%e, "failed to create trash directory");
                return;
            }
            for path in paths {
                let trash_dst = crate::file_op::trash::trash_path(&path);
                let op = FsOp::Move { src: path.clone(), dst: trash_dst };
                if let Err(e) = execute(&op) {
                    tracing::error!(%e, ?path, "custom trash delete failed");
                    continue;
                }
                if let Some(parent) = path.parent() {
                    affected_parents.insert(parent.to_path_buf());
                }
                // Custom trash can be undone: reverse is moving back from trash.
                if let Some(undo_op) = build_undo_op(op) {
                    undo_ops.push(undo_op);
                }
            }
        }
        ConfirmAction::SystemTrash => {
            for path in paths {
                if let Err(e) = trash::delete(&path) {
                    tracing::error!(%e, ?path, "system trash delete failed");
                    continue;
                }
                if let Some(parent) = path.parent() {
                    affected_parents.insert(parent.to_path_buf());
                }
                // System trash cannot be undone via our undo system.
            }
        }
    }

    state.selection.clear();

    // Push undo group (only for custom trash deletes).
    if !undo_ops.is_empty() {
        state
            .undo_history
            .push(OpGroup { description: format!("Delete {delete_count} item(s)"), ops: undo_ops });
    }

    for parent in &affected_parents {
        refresh_directory(state, parent, ctx);
    }

    // Status message based on delete mode.
    if is_system_trash {
        state.set_status(format!("Moved {delete_count} item(s) to trash"));
    } else {
        state.set_status(format!("Deleted {delete_count} item(s)"));
    }
}

/// Execute file/directory creation.
pub fn execute_create(parent_dir: &Path, name: &str, state: &mut AppState, ctx: &AppContext) {
    let new_path = parent_dir.join(name);

    // Trailing "/" means create a directory.
    let op = if name.ends_with('/') {
        FsOp::CreateDir { path: new_path }
    } else {
        // Ensure parent directories exist for nested paths (e.g. "foo/bar/baz.txt").
        if let Some(parent) = new_path.parent()
            && !parent.exists()
        {
            let mkdir_op = FsOp::CreateDir { path: parent.to_path_buf() };
            if let Err(e) = execute(&mkdir_op) {
                tracing::error!(%e, "failed to create parent directories");
                return;
            }
        }
        FsOp::CreateFile { path: new_path }
    };

    if let Err(e) = execute(&op) {
        tracing::error!(%e, "failed to create file/directory");
        return;
    }

    // Push to undo history.
    if let Some(undo_op) = build_undo_op(op) {
        state
            .undo_history
            .push(OpGroup { description: format!("Create {name}"), ops: vec![undo_op] });
    }

    // Refresh the parent directory in the tree.
    refresh_directory(state, parent_dir, ctx);
}

/// Execute file/directory rename.
pub fn execute_rename(target: &Path, new_name: &str, state: &mut AppState, ctx: &AppContext) {
    let parent = target.parent().unwrap_or_else(|| Path::new(""));
    let new_path = parent.join(new_name);

    let op = FsOp::Move { src: target.to_path_buf(), dst: new_path };

    if let Err(e) = execute(&op) {
        tracing::error!(%e, "failed to rename");
        return;
    }

    // Push to undo history.
    if let Some(undo_op) = build_undo_op(op) {
        state
            .undo_history
            .push(OpGroup { description: format!("Rename to {new_name}"), ops: vec![undo_op] });
    }

    // Refresh the parent directory in the tree.
    refresh_directory(state, parent, ctx);
}

/// Execute paste operation: copy or move selected files to the cursor directory.
fn execute_paste(state: &mut AppState, ctx: &AppContext) {
    use crate::file_op::executor::is_ancestor;

    let mode = state.selection.mode().cloned();
    let is_cut = matches!(mode, Some(SelectionMode::Cut));

    // Paste only works in Copy or Cut mode.
    if !matches!(mode, Some(SelectionMode::Copy | SelectionMode::Cut)) {
        return;
    }

    let Some(info) = state.tree_state.current_node_info() else {
        return;
    };

    // Determine destination directory.
    let dst_dir = if info.is_dir {
        info.path
    } else {
        info.path.parent().map_or_else(|| info.path.clone(), Path::to_path_buf)
    };

    // Track source parent directories that need refresh (for cut operations).
    let mut src_parents: HashSet<PathBuf> = HashSet::new();
    let mut undo_ops: Vec<UndoOp> = Vec::new();

    let sources = state.selection.deduplicated_paths();
    for src in sources {
        // Self-reference check: prevent copying/moving a directory into itself.
        if src.is_dir() && is_ancestor(&src, &dst_dir) {
            tracing::warn!(?src, ?dst_dir, "skipping: cannot copy/move directory into itself");
            continue;
        }

        let Some(file_name) = src.file_name() else {
            tracing::warn!(?src, "skipping: no file name");
            continue;
        };

        let desired_dst = dst_dir.join(file_name);
        let final_dst = crate::file_op::conflict::resolve_conflict(&desired_dst);

        let op = if is_cut {
            FsOp::Move { src: src.clone(), dst: final_dst.clone() }
        } else {
            FsOp::Copy { src: src.clone(), dst: final_dst.clone() }
        };

        if let Err(e) = execute(&op) {
            tracing::error!(%e, ?src, "paste operation failed");
            continue;
        }

        // Build undo op for this successful operation.
        let reverse = if is_cut {
            FsOp::Move { src: final_dst, dst: src.clone() }
        } else {
            reverse_for_copy(&final_dst)
        };
        undo_ops.push(UndoOp { forward: op, reverse });

        // Track source parent for cut operations.
        if is_cut && let Some(parent) = src.parent() {
            src_parents.insert(parent.to_path_buf());
        }
    }

    let paste_count = state.selection.count();
    state.selection.clear();

    // Push undo group.
    if !undo_ops.is_empty() {
        let action_desc = if is_cut { "Move" } else { "Copy" };
        state.undo_history.push(OpGroup {
            description: format!("{action_desc} {paste_count} file(s)"),
            ops: undo_ops,
        });
    }

    // Refresh destination directory.
    refresh_directory(state, &dst_dir, ctx);

    // Refresh source parent directories (for cut operations).
    for parent in &src_parents {
        if *parent != dst_dir {
            refresh_directory(state, parent, ctx);
        }
    }

    let action = if is_cut { "Moved" } else { "Pasted" };
    state.set_status(format!("{action} {paste_count} file(s)"));
}

/// Execute undo: reverse the most recent operation group.
fn execute_undo(state: &mut AppState, ctx: &AppContext) {
    // Extract undo data before executing (releases borrow on undo_history).
    let (desc, reverse_ops) = {
        match state.undo_history.undo() {
            Ok(Some(group)) => {
                let desc = group.description.clone();
                let ops: Vec<FsOp> = group.ops.iter().rev().map(|op| op.reverse.clone()).collect();
                (desc, ops)
            }
            Ok(None) => {
                state.set_status("Nothing to undo");
                return;
            }
            Err(e) => {
                state.set_status(format!("Cannot undo: {e}"));
                return;
            }
        }
    };

    for op in &reverse_ops {
        if let Err(e) = execute(op) {
            tracing::error!(%e, "undo operation failed");
            state.set_status(format!("Undo failed: {e}"));
            return;
        }
    }

    // Refresh affected directories.
    let parents = collect_affected_parents(&reverse_ops);
    for parent in &parents {
        refresh_directory(state, parent, ctx);
    }

    state.set_status(format!("Undid: {desc}"));
}

/// Execute redo: re-apply the most recently undone operation group.
fn execute_redo(state: &mut AppState, ctx: &AppContext) {
    // Extract redo data before executing (releases borrow on undo_history).
    let (desc, forward_ops) = {
        match state.undo_history.redo() {
            Ok(Some(group)) => {
                let desc = group.description.clone();
                let ops: Vec<FsOp> = group.ops.iter().map(|op| op.forward.clone()).collect();
                (desc, ops)
            }
            Ok(None) => {
                state.set_status("Nothing to redo");
                return;
            }
            Err(e) => {
                state.set_status(format!("Cannot redo: {e}"));
                return;
            }
        }
    };

    for op in &forward_ops {
        if let Err(e) = execute(op) {
            tracing::error!(%e, "redo operation failed");
            state.set_status(format!("Redo failed: {e}"));
            return;
        }
    }

    // Refresh affected directories.
    let parents = collect_affected_parents(&forward_ops);
    for parent in &parents {
        refresh_directory(state, parent, ctx);
    }

    state.set_status(format!("Redid: {desc}"));
}

/// Collect unique parent directories affected by a set of operations.
fn collect_affected_parents(ops: &[FsOp]) -> HashSet<PathBuf> {
    let mut parents = HashSet::new();
    for op in ops {
        match op {
            FsOp::Copy { src, dst } | FsOp::Move { src, dst } => {
                if let Some(p) = src.parent() {
                    parents.insert(p.to_path_buf());
                }
                if let Some(p) = dst.parent() {
                    parents.insert(p.to_path_buf());
                }
            }
            FsOp::CreateFile { path }
            | FsOp::CreateDir { path }
            | FsOp::RemoveFile { path }
            | FsOp::RemoveDir { path } => {
                if let Some(p) = path.parent() {
                    parents.insert(p.to_path_buf());
                }
            }
        }
    }
    parents
}
