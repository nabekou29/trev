//! Action handlers dispatched from the main event loop.

mod file_op;
mod input;
pub mod ipc;
mod preview;
mod tree;

use std::sync::Arc;

use crate::action::Action;
use file_op::handle_file_op_action;
use input::{
    handle_confirm_mode_key,
    handle_input_mode_key,
    handle_menu_mode_key,
};
pub use ipc::handle_ipc_command;
use preview::handle_preview_action;
pub use preview::trigger_preview;
use tree::handle_tree_action;
pub use tree::{
    refresh_directory,
    trigger_prefetch,
};

use std::collections::BTreeSet;

use crate::app::keymap::KeyContext;
use crate::app::state::{
    AppContext,
    AppState,
};
use crate::input::AppMode;

/// Handle a key event and update application state.
pub fn handle_key_event(key: crossterm::event::KeyEvent, state: &mut AppState, ctx: &AppContext) {
    // Dispatch based on current application mode.
    match state.mode {
        AppMode::Input(_) => {
            handle_input_mode_key(key, state, ctx);
        }
        AppMode::Confirm(_) => {
            handle_confirm_mode_key(key, state, ctx);
        }
        AppMode::Menu(_) => {
            handle_menu_mode_key(key, state);
        }
        AppMode::Normal => {
            let mut active_contexts = BTreeSet::new();
            if ctx.ipc_server.is_some() {
                active_contexts.insert(KeyContext::Daemon);
            }
            let node_ctx = state
                .tree_state
                .current_node_info()
                .map_or(KeyContext::File, |info| {
                    if info.is_dir {
                        KeyContext::Directory
                    } else {
                        KeyContext::File
                    }
                });
            active_contexts.insert(node_ctx);
            let Some(action) = ctx.keymap.resolve(key, &active_contexts) else {
                return;
            };
            match action {
                Action::Quit => {
                    state.should_quit = true;
                }
                Action::Tree(tree_action) => {
                    handle_tree_action(*tree_action, state, ctx);
                }
                Action::Preview(preview_action) => {
                    handle_preview_action(*preview_action, state, ctx);
                }
                Action::FileOp(file_op_action) => {
                    handle_file_op_action(*file_op_action, state, ctx);
                }
                Action::Shell(cmd) => {
                    handle_shell_action(cmd, state);
                }
                Action::Notify(method) => {
                    handle_notify_action(method, state, ctx);
                }
                Action::Noop => {}
            }
        }
    }
}

/// Execute a shell command with template variable substitution.
///
/// Suspends the TUI, runs the command via `sh -c`, then resumes.
/// Template variables: `{path}`, `{dir}`, `{name}`, `{root}`.
fn handle_shell_action(cmd: &str, state: &mut AppState) {
    let expanded = expand_shell_template(cmd, &state.tree_state);

    // Suspend TUI.
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);

    // Execute command.
    let result = std::process::Command::new("sh")
        .arg("-c")
        .arg(&expanded)
        .status();

    // Resume TUI.
    let _ = crossterm::terminal::enable_raw_mode();
    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen);

    match result {
        Ok(status) if !status.success() => {
            let code = status.code().map_or_else(|| "unknown".to_string(), |c| c.to_string());
            state.set_status(format!("Command exited with code {code}"));
        }
        Err(e) => {
            state.set_status(format!("Failed to execute command: {e}"));
        }
        Ok(_) => {}
    }
}

/// Send an IPC notification to the connected client.
///
/// Includes the cursor file path and editor action in the notification params.
/// Silently drops if no IPC server is running (non-daemon mode).
fn handle_notify_action(method: &str, state: &AppState, ctx: &AppContext) {
    let Some(server) = &ctx.ipc_server else {
        return;
    };
    let path = state.tree_state.cursor_path().unwrap_or_default();
    let params = serde_json::json!({
        "action": ctx.editor_action,
        "path": path,
    });
    let server = Arc::clone(server);
    let method = method.to_string();
    tokio::spawn(async move {
        server.send_notification(&method, params).await;
    });
}

/// Expand template variables in a shell command string.
///
/// Supported variables:
/// - `{path}` — absolute path of the cursor node
/// - `{dir}` — directory path (self for dirs, parent for files)
/// - `{name}` — file name (basename)
/// - `{root}` — workspace root path
#[expect(clippy::literal_string_with_formatting_args, reason = "Uses {path}/{dir}/{name}/{root} as template placeholders, not format args")]
fn expand_shell_template(
    template: &str,
    tree_state: &crate::state::tree::TreeState,
) -> String {
    let info = tree_state.current_node_info();
    let root = tree_state.root_path();

    let path_str = info.as_ref().map_or_else(String::new, |i| i.path.display().to_string());
    let dir_str = tree_state
        .cursor_dir_path()
        .map_or_else(String::new, |p| p.display().to_string());
    let name_str = info.as_ref().map_or_else(String::new, |i| i.name.clone());
    let root_str = root.display().to_string();

    template
        .replace("{path}", &path_str)
        .replace("{dir}", &dir_str)
        .replace("{name}", &name_str)
        .replace("{root}", &root_str)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::literal_string_with_formatting_args
)]
mod tests {
    use std::path::Path;

    use googletest::prelude::*;
    use rstest::*;

    use crate::state::tree::{
        ChildrenState,
        TreeNode,
        TreeOptions,
        TreeState,
    };

    /// Create a minimal tree state with a file child at cursor.
    fn state_with_file(name: &str, root: &Path) -> TreeState {
        let child = TreeNode {
            name: name.to_string(),
            path: root.join(name),
            is_dir: false,
            is_symlink: false,
            symlink_target: None,
            size: 0,
            modified: None,
            recursive_max_mtime: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
        };
        let root_node = TreeNode {
            name: "root".to_string(),
            path: root.to_path_buf(),
            is_dir: true,
            is_symlink: false,
            symlink_target: None,
            size: 0,
            modified: None,
            recursive_max_mtime: None,
            children: ChildrenState::Loaded(vec![child]),
            is_expanded: true,
        };
        let mut state =
            TreeState::new(root_node, TreeOptions::default());
        state.move_cursor_to(0);
        state
    }

    /// Create a tree state with a directory child at cursor.
    fn state_with_dir(name: &str, root: &Path) -> TreeState {
        let child = TreeNode {
            name: name.to_string(),
            path: root.join(name),
            is_dir: true,
            is_symlink: false,
            symlink_target: None,
            size: 0,
            modified: None,
            recursive_max_mtime: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
        };
        let root_node = TreeNode {
            name: "root".to_string(),
            path: root.to_path_buf(),
            is_dir: true,
            is_symlink: false,
            symlink_target: None,
            size: 0,
            modified: None,
            recursive_max_mtime: None,
            children: ChildrenState::Loaded(vec![child]),
            is_expanded: true,
        };
        let mut state =
            TreeState::new(root_node, TreeOptions::default());
        state.move_cursor_to(0);
        state
    }

    #[rstest]
    fn expand_template_replaces_path() {
        let root = Path::new("/test/root");
        let state = state_with_file("test.txt", root);
        let result = super::expand_shell_template("open {path}", &state);
        assert_that!(result.as_str(), eq("open /test/root/test.txt"));
    }

    #[rstest]
    fn expand_template_replaces_name() {
        let root = Path::new("/test/root");
        let state = state_with_file("hello.rs", root);
        let result = super::expand_shell_template("echo {name}", &state);
        assert_that!(result.as_str(), eq("echo hello.rs"));
    }

    #[rstest]
    fn expand_template_replaces_root() {
        let root = Path::new("/test/root");
        let state = state_with_file("test.txt", root);
        let result = super::expand_shell_template("ls {root}", &state);
        assert_that!(result.as_str(), eq("ls /test/root"));
    }

    #[rstest]
    fn expand_template_dir_for_file_returns_parent() {
        let root = Path::new("/test/root");
        let state = state_with_file("test.txt", root);
        let result = super::expand_shell_template("cd {dir}", &state);
        assert_that!(result.as_str(), eq("cd /test/root"));
    }

    #[rstest]
    fn expand_template_dir_for_directory_returns_self() {
        let root = Path::new("/test/root");
        let state = state_with_dir("subdir", root);
        let result = super::expand_shell_template("cd {dir}", &state);
        assert_that!(result.as_str(), eq("cd /test/root/subdir"));
    }

    #[rstest]
    fn expand_template_multiple_vars() {
        let root = Path::new("/test/root");
        let state = state_with_file("test.txt", root);
        let result = super::expand_shell_template("cp {path} {dir}/{name}.bak", &state);
        assert_that!(
            result.as_str(),
            eq("cp /test/root/test.txt /test/root/test.txt.bak")
        );
    }

    #[rstest]
    fn expand_template_no_vars() {
        let root = Path::new("/test/root");
        let state = state_with_file("test.txt", root);
        let result = super::expand_shell_template("echo hello", &state);
        assert_that!(result.as_str(), eq("echo hello"));
    }
}
