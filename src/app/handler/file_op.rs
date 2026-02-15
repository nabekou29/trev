//! File operation action handlers.
//!
//! Handles mark, create, rename, yank, cut, paste, delete, and trash operations.

use std::path::{
    Path,
    PathBuf,
};

use super::tree::refresh_directory;
use crate::app::state::{
    AppContext,
    AppState,
};
use crate::file_op::selection::SelectionMode;
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
        // Undo/Redo: will be implemented in US4.
        FileOpAction::Undo | FileOpAction::Redo => {}
    }
}

/// Execute the confirmed delete operation.
pub fn execute_delete(confirm: crate::input::ConfirmState, state: &mut AppState, ctx: &AppContext) {
    use std::collections::HashSet;

    use crate::file_op::executor::{
        FsOp,
        execute,
    };
    use crate::input::ConfirmAction;

    let mut affected_parents: HashSet<PathBuf> = HashSet::new();
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
            }
        }
    }

    state.selection.clear();

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
pub fn execute_create(parent_dir: &Path, name: &str, state: &AppState, ctx: &AppContext) {
    use crate::file_op::executor::{
        FsOp,
        execute,
    };

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

    // Refresh the parent directory in the tree.
    refresh_directory(state, parent_dir, ctx);
}

/// Execute file/directory rename.
pub fn execute_rename(target: &Path, new_name: &str, state: &AppState, ctx: &AppContext) {
    use crate::file_op::executor::{
        FsOp,
        execute,
    };

    let parent = target.parent().unwrap_or_else(|| Path::new(""));
    let new_path = parent.join(new_name);

    let op = FsOp::Move { src: target.to_path_buf(), dst: new_path };

    if let Err(e) = execute(&op) {
        tracing::error!(%e, "failed to rename");
        return;
    }

    // Refresh the parent directory in the tree.
    refresh_directory(state, parent, ctx);
}

/// Execute paste operation: copy or move selected files to the cursor directory.
fn execute_paste(state: &mut AppState, ctx: &AppContext) {
    use std::collections::HashSet;

    use crate::file_op::conflict::resolve_conflict;
    use crate::file_op::executor::{
        FsOp,
        execute,
        is_ancestor,
    };

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
        let final_dst = resolve_conflict(&desired_dst);

        let op = if is_cut {
            FsOp::Move { src: src.clone(), dst: final_dst }
        } else {
            FsOp::Copy { src: src.clone(), dst: final_dst }
        };

        if let Err(e) = execute(&op) {
            tracing::error!(%e, ?src, "paste operation failed");
            continue;
        }

        // Track source parent for cut operations.
        if is_cut && let Some(parent) = src.parent() {
            src_parents.insert(parent.to_path_buf());
        }
    }

    let paste_count = state.selection.count();
    state.selection.clear();

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
