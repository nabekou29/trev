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
use crate::watcher::SuppressGuard;

/// Handle a file operation action.
#[expect(clippy::too_many_lines, reason = "match dispatch for all file op variants")]
pub fn handle_file_op_action(
    action: crate::action::FileOpAction,
    state: &mut AppState,
    ctx: &AppContext,
) {
    use crate::action::FileOpAction;
    use crate::input::{
        ConfirmAction,
        ConfirmState,
        InputState,
    };

    match action {
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
            if let Some(dir) = cursor_parent_dir(state) {
                state.mode = AppMode::Input(InputState::for_create(dir));
            }
        }
        FileOpAction::CreateDirectory => {
            if let Some(dir) = cursor_parent_dir(state) {
                state.mode = AppMode::Input(InputState::for_create_directory(dir));
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
                if ctx.file_op_config.clipboard_sync {
                    sync_file_list_to_clipboard(&targets);
                }
                state.selection.set(targets, SelectionMode::Copy);
                state.set_status(format!("Yanked {count} file(s)"));
            }
        }
        FileOpAction::Cut => {
            if let Some(info) = state.tree_state.current_node_info() {
                let targets = state.selection.mark_targets_or_cursor(&info.path);
                let count = targets.len();
                if ctx.file_op_config.clipboard_sync {
                    sync_file_list_to_clipboard(&targets);
                }
                state.selection.set(targets, SelectionMode::Cut);
                state.set_status(format!("Cut {count} file(s)"));
            }
        }
        FileOpAction::Paste => {
            execute_paste(state, ctx);
        }
        FileOpAction::PasteFromClipboard => {
            execute_clipboard_paste(state, ctx);
        }
        FileOpAction::CopyToClipboard => {
            if let Some(info) = state.tree_state.current_node_info() {
                let targets = state.selection.mark_targets_or_cursor(&info.path);
                let count = targets.len();
                sync_file_list_to_clipboard(&targets);
                state.set_status(format!("Copied {count} file(s) to clipboard"));
            }
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
        FileOpAction::PasteMenu => {
            open_paste_menu(state);
        }
        FileOpAction::PasteAsSymlink => {
            execute_paste_as_link(state, ctx, LinkKind::Symlink);
        }
        FileOpAction::PasteAsHardlink => {
            execute_paste_as_link(state, ctx, LinkKind::Hardlink);
        }
        FileOpAction::Copy(copy_action) => {
            use crate::action::CopyAction;
            match copy_action {
                CopyAction::Menu => open_copy_menu(state),
                CopyAction::AbsolutePath
                | CopyAction::RelativePath
                | CopyAction::FileName
                | CopyAction::Stem
                | CopyAction::ParentDir => {
                    copy_direct(state, copy_action);
                }
            }
        }
    }
}

/// Write a list of file paths to the OS clipboard as a file list.
///
/// This allows pasting the files in external applications (e.g. Finder).
/// Failures are logged but do not interrupt the operation.
fn sync_file_list_to_clipboard(paths: &[PathBuf]) {
    if let Err(e) = arboard::Clipboard::new().and_then(|mut cb| cb.set().file_list(paths)) {
        tracing::warn!(%e, "failed to write file list to clipboard");
    }
}

/// Get the parent directory for the cursor node.
///
/// If the cursor is on a directory, returns that directory.
/// Otherwise, returns the parent directory of the file.
fn cursor_parent_dir(state: &AppState) -> Option<PathBuf> {
    let info = state.tree_state.current_node_info()?;
    Some(if info.is_dir {
        info.path
    } else {
        info.path.parent().map_or_else(|| info.path.clone(), Path::to_path_buf)
    })
}

/// Build and open the copy-to-clipboard menu for the current cursor node.
fn open_copy_menu(state: &mut AppState) {
    use crate::action::{
        Action,
        CopyAction,
        FileOpAction,
    };
    use crate::input::{
        MenuAction,
        MenuItem,
        MenuState,
    };

    if state.tree_state.current_node_info().is_none() {
        return;
    }

    let items = vec![
        MenuItem { key: 'p', label: "Absolute path".to_string(), value: String::new() },
        MenuItem { key: 'r', label: "Relative path".to_string(), value: String::new() },
        MenuItem { key: 'n', label: "File name".to_string(), value: String::new() },
        MenuItem { key: 's', label: "Stem".to_string(), value: String::new() },
        MenuItem { key: 'd', label: "Parent directory".to_string(), value: String::new() },
        MenuItem { key: 'f', label: "Files".to_string(), value: String::new() },
    ];
    let item_actions = vec![
        Action::FileOp(FileOpAction::Copy(CopyAction::AbsolutePath)),
        Action::FileOp(FileOpAction::Copy(CopyAction::RelativePath)),
        Action::FileOp(FileOpAction::Copy(CopyAction::FileName)),
        Action::FileOp(FileOpAction::Copy(CopyAction::Stem)),
        Action::FileOp(FileOpAction::Copy(CopyAction::ParentDir)),
        Action::FileOp(FileOpAction::CopyToClipboard),
    ];

    state.mode = AppMode::Menu(MenuState {
        title: "Copy to clipboard".to_string(),
        items,
        cursor: 0,
        on_select: MenuAction::Custom,
        item_actions,
    });
}

/// Build and open the paste options menu.
fn open_paste_menu(state: &mut AppState) {
    use crate::action::{
        Action,
        FileOpAction,
    };
    use crate::input::{
        MenuAction,
        MenuItem,
        MenuState,
    };

    let items = vec![
        MenuItem { key: 'p', label: "Paste".to_string(), value: String::new() },
        MenuItem { key: 's', label: "Symlink".to_string(), value: String::new() },
        MenuItem { key: 'h', label: "Hard link".to_string(), value: String::new() },
    ];
    let item_actions = vec![
        Action::FileOp(FileOpAction::Paste),
        Action::FileOp(FileOpAction::PasteAsSymlink),
        Action::FileOp(FileOpAction::PasteAsHardlink),
    ];

    state.mode = AppMode::Menu(MenuState {
        title: "Paste".to_string(),
        items,
        cursor: 0,
        on_select: MenuAction::Custom,
        item_actions,
    });
}

/// Kind of link to create when pasting.
#[derive(Clone, Copy)]
enum LinkKind {
    /// Symbolic link.
    Symlink,
    /// Hard link.
    Hardlink,
}

/// Execute paste-as-link operation: create symlinks or hard links to yanked paths.
fn execute_paste_as_link(state: &mut AppState, ctx: &AppContext, kind: LinkKind) {
    let mode = state.selection.mode().copied();

    // Only Copy mode makes sense for link creation.
    if !matches!(mode, Some(SelectionMode::Copy)) {
        state.set_status("Nothing to paste (yank files first)");
        return;
    }

    let _guard = SuppressGuard::new(&ctx.suppressed);

    let Some(info) = state.tree_state.current_node_info() else {
        return;
    };

    let dst_dir = if info.is_dir {
        info.path
    } else {
        info.path.parent().map_or_else(|| info.path.clone(), Path::to_path_buf)
    };

    let mut undo_ops: Vec<UndoOp> = Vec::new();
    let sources = state.selection.deduplicated_paths();
    let kind_name = match kind {
        LinkKind::Symlink => "symlink",
        LinkKind::Hardlink => "hard link",
    };

    for src in &sources {
        let Some(file_name) = src.file_name() else {
            tracing::warn!(?src, "skipping: no file name");
            continue;
        };

        let desired_dst = dst_dir.join(file_name);
        let final_dst = crate::file_op::conflict::resolve_conflict(&desired_dst);

        let op = match kind {
            LinkKind::Symlink => {
                FsOp::CreateSymlink { target: src.clone(), link: final_dst.clone() }
            }
            LinkKind::Hardlink => {
                if src.is_dir() {
                    tracing::warn!(?src, "skipping: cannot create hard link to directory");
                    state.set_status("Cannot create hard link to a directory");
                    continue;
                }
                FsOp::CreateHardlink { original: src.clone(), link: final_dst.clone() }
            }
        };

        if let Err(e) = execute(&op) {
            tracing::error!(%e, ?src, "paste as {kind_name} failed");
            continue;
        }

        if let Some(undo_op) = build_undo_op(op) {
            undo_ops.push(undo_op);
        }
    }

    let paste_count = undo_ops.len();
    state.selection.clear();

    if !undo_ops.is_empty() {
        state.undo_history.push(OpGroup {
            description: format!("Paste {paste_count} {kind_name}(s)"),
            ops: undo_ops,
        });
    }

    refresh_directory(state, &dst_dir, ctx);
    state.set_status(format!("Created {paste_count} {kind_name}(s)"));
}

/// Copy a specific value to the clipboard directly (without opening the menu).
fn copy_direct(state: &mut AppState, action: crate::action::CopyAction) {
    use crate::action::CopyAction;

    let Some(info) = state.tree_state.current_node_info() else {
        return;
    };

    let (label, value) = match action {
        CopyAction::AbsolutePath => ("absolute path", info.path.to_string_lossy().to_string()),
        CopyAction::RelativePath => {
            let rel = info
                .path
                .strip_prefix(state.tree_state.root_path())
                .unwrap_or(&info.path)
                .to_string_lossy()
                .to_string();
            ("relative path", rel)
        }
        CopyAction::FileName => ("file name", info.name),
        CopyAction::Stem => {
            let stem = info.path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            ("stem", stem)
        }
        CopyAction::ParentDir => {
            let parent =
                info.path.parent().unwrap_or_else(|| Path::new("")).to_string_lossy().to_string();
            ("parent directory", parent)
        }
        CopyAction::Menu => return, // Handled separately.
    };

    super::input::copy_to_clipboard(state, label, &value);
}

/// Execute the confirmed delete operation.
pub fn execute_delete(confirm: crate::input::ConfirmState, state: &mut AppState, ctx: &AppContext) {
    use crate::input::ConfirmAction;

    let _guard = SuppressGuard::new(&ctx.suppressed);
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

/// Validate that a user-supplied name does not escape the parent directory.
///
/// Rejects absolute paths and `..` components to prevent path traversal attacks.
fn validate_name(name: &str) -> Result<(), &'static str> {
    let path = Path::new(name);
    if path.is_absolute() {
        return Err("Absolute paths are not allowed");
    }
    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err("Path traversal (..) is not allowed");
        }
    }
    Ok(())
}

/// Execute file/directory creation.
pub fn execute_create(parent_dir: &Path, name: &str, state: &mut AppState, ctx: &AppContext) {
    if let Err(msg) = validate_name(name) {
        state.set_status(msg.to_string());
        return;
    }
    let _guard = SuppressGuard::new(&ctx.suppressed);
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

/// Execute directory creation.
///
/// Always creates a directory regardless of trailing slash.
pub fn execute_create_directory(
    parent_dir: &Path,
    name: &str,
    state: &mut AppState,
    ctx: &AppContext,
) {
    if let Err(msg) = validate_name(name) {
        state.set_status(msg.to_string());
        return;
    }
    let _guard = SuppressGuard::new(&ctx.suppressed);
    let new_path = parent_dir.join(name);

    let op = FsOp::CreateDir { path: new_path };

    if let Err(e) = execute(&op) {
        tracing::error!(%e, "failed to create directory");
        return;
    }

    if let Some(undo_op) = build_undo_op(op) {
        state
            .undo_history
            .push(OpGroup { description: format!("Create directory {name}"), ops: vec![undo_op] });
    }

    refresh_directory(state, parent_dir, ctx);
}

/// Execute file/directory rename.
pub fn execute_rename(target: &Path, new_name: &str, state: &mut AppState, ctx: &AppContext) {
    if let Err(msg) = validate_name(new_name) {
        state.set_status(msg.to_string());
        return;
    }
    let _guard = SuppressGuard::new(&ctx.suppressed);
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
///
/// Falls back to OS clipboard when the internal selection buffer is empty.
fn execute_paste(state: &mut AppState, ctx: &AppContext) {
    use crate::file_op::executor::is_ancestor;

    let mode = state.selection.mode().copied();
    let is_cut = matches!(mode, Some(SelectionMode::Cut));

    // Fall back to OS clipboard when the internal buffer is empty.
    if !matches!(mode, Some(SelectionMode::Copy | SelectionMode::Cut)) {
        execute_clipboard_paste(state, ctx);
        return;
    }

    let _guard = SuppressGuard::new(&ctx.suppressed);

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

/// Handle a paste event from the terminal (Cmd+V / bracketed paste).
///
/// Only acts in Normal mode to avoid interfering with text input.
pub fn handle_clipboard_paste(state: &mut AppState, ctx: &AppContext) {
    if !matches!(state.mode, AppMode::Normal) {
        return;
    }
    handle_file_op_action(crate::action::FileOpAction::PasteFromClipboard, state, ctx);
}

/// Execute paste from the OS clipboard.
///
/// Reads the clipboard content (file list or image data) and pastes it
/// into the directory at the cursor position.
fn execute_clipboard_paste(state: &mut AppState, ctx: &AppContext) {
    use crate::file_op::clipboard::{
        ClipboardContent,
        read_clipboard,
    };
    use crate::file_op::executor::is_ancestor;

    let Some(content) = read_clipboard() else {
        state.set_status("Clipboard is empty");
        return;
    };

    let Some(dst_dir) = cursor_parent_dir(state) else {
        return;
    };

    match content {
        ClipboardContent::Files(paths) => {
            let _guard = SuppressGuard::new(&ctx.suppressed);
            let mut undo_ops: Vec<UndoOp> = Vec::new();
            let mut paste_count: usize = 0;

            for src in &paths {
                if !src.exists() {
                    tracing::warn!(?src, "clipboard file does not exist, skipping");
                    continue;
                }

                // Prevent copying a directory into itself.
                if src.is_dir() && is_ancestor(src, &dst_dir) {
                    tracing::warn!(?src, ?dst_dir, "skipping: cannot copy directory into itself");
                    continue;
                }

                let Some(file_name) = src.file_name() else {
                    tracing::warn!(?src, "skipping: no file name");
                    continue;
                };

                let desired_dst = dst_dir.join(file_name);
                let final_dst = crate::file_op::conflict::resolve_conflict(&desired_dst);
                let op = FsOp::Copy { src: src.clone(), dst: final_dst.clone() };

                if let Err(e) = execute(&op) {
                    tracing::error!(%e, ?src, "clipboard paste failed");
                    continue;
                }

                let reverse = reverse_for_copy(&final_dst);
                undo_ops.push(UndoOp { forward: op, reverse });
                paste_count += 1;
            }

            if !undo_ops.is_empty() {
                state.undo_history.push(OpGroup {
                    description: format!("Paste {paste_count} file(s) from clipboard"),
                    ops: undo_ops,
                });
            }

            refresh_directory(state, &dst_dir, ctx);
            state.set_status(format!("Pasted {paste_count} file(s) from clipboard"));
        }
        ClipboardContent::Image { width, height, bytes } => {
            let _guard = SuppressGuard::new(&ctx.suppressed);

            match crate::file_op::clipboard::save_image_as_png(&dst_dir, width, height, &bytes) {
                Ok(saved_path) => {
                    // Build undo op: reverse is removing the created file.
                    let forward = FsOp::CreateFile { path: saved_path.clone() };
                    let reverse = FsOp::RemoveFile { path: saved_path.clone() };
                    let file_name = saved_path.file_name().unwrap_or_default().to_string_lossy();
                    state.undo_history.push(OpGroup {
                        description: format!("Paste clipboard image as {file_name}"),
                        ops: vec![UndoOp { forward, reverse }],
                    });

                    refresh_directory(state, &dst_dir, ctx);
                    state.set_status(format!("Saved clipboard image: {file_name}"));
                }
                Err(e) => {
                    tracing::error!(%e, "failed to save clipboard image");
                    state.set_status(format!("Failed to save image: {e}"));
                }
            }
        }
    }
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

    {
        let _guard = SuppressGuard::new(&ctx.suppressed);
        for op in &reverse_ops {
            if let Err(e) = execute(op) {
                tracing::error!(%e, "undo operation failed");
                state.set_status(format!("Undo failed: {e}"));
                return;
            }
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

    {
        let _guard = SuppressGuard::new(&ctx.suppressed);
        for op in &forward_ops {
            if let Err(e) = execute(op) {
                tracing::error!(%e, "redo operation failed");
                state.set_status(format!("Redo failed: {e}"));
                return;
            }
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
            FsOp::CreateSymlink { link, .. } | FsOp::CreateHardlink { link, .. } => {
                if let Some(p) = link.parent() {
                    parents.insert(p.to_path_buf());
                }
            }
        }
    }
    parents
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    #[rstest]
    fn validate_name_allows_simple_name() {
        assert_that!(validate_name("file.txt"), ok(eq(())));
    }

    #[rstest]
    fn validate_name_allows_nested_path() {
        assert_that!(validate_name("dir/file.txt"), ok(eq(())));
    }

    #[rstest]
    fn validate_name_rejects_parent_traversal() {
        assert_that!(validate_name("../evil.txt"), err(anything()));
    }

    #[rstest]
    fn validate_name_rejects_nested_parent_traversal() {
        assert_that!(validate_name("foo/../../evil.txt"), err(anything()));
    }

    #[rstest]
    fn validate_name_rejects_absolute_path() {
        assert_that!(validate_name("/etc/passwd"), err(anything()));
    }
}
