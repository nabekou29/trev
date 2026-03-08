//! Action handlers dispatched from the main event loop.

mod file_op;
mod input;
pub mod ipc;
mod mouse;
pub(super) mod preview;
mod search;
mod tree;

use std::collections::BTreeSet;
use std::sync::Arc;

use file_op::handle_file_op_action;
use input::{
    handle_confirm_mode_key,
    handle_input_mode_key,
    handle_menu_mode_key,
};
pub use ipc::handle_ipc_command;
pub use mouse::handle_mouse_event;
use preview::handle_preview_action;
pub use preview::trigger_preview;
use tree::handle_tree_action;
pub use tree::{
    refresh_directory,
    spawn_load_children,
    trigger_prefetch,
};

use crate::action::Action;
use crate::app::key_parse::KeyBinding;
use crate::app::key_trie::TrieLookup;
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
            handle_menu_mode_key(key, state, ctx);
        }
        AppMode::Normal => {
            handle_normal_mode_key(key, state, ctx);
        }
        AppMode::Search(_) => {
            search::handle_search_mode_key(key, state, ctx);
        }
    }
}

/// Handle a key event in Normal mode with multi-key sequence support.
pub(super) fn handle_normal_mode_key(key: crossterm::event::KeyEvent, state: &mut AppState, ctx: &AppContext) {
    let active_contexts = build_active_contexts(state, ctx);
    let kb: KeyBinding = (key.code, key.modifiers);

    state.pending_keys.push(kb);

    match ctx.keymap.lookup(state.pending_keys.keys(), &active_contexts) {
        TrieLookup::Resolved(action) => {
            let action = action.clone();
            state.pending_keys.clear();
            dispatch_action(&action, state, ctx);
        }
        TrieLookup::PendingWithFallback(_) | TrieLookup::Pending => {
            // Wait for more keys or timeout.
        }
        TrieLookup::NoMatch => {
            // Current sequence doesn't match. Clear and retry the new key alone.
            state.pending_keys.clear();
            // Only retry if the failed sequence had more than 1 key.
            let single = ctx.keymap.lookup(&[kb], &active_contexts);
            match single {
                TrieLookup::Resolved(action) => {
                    let action = action.clone();
                    dispatch_action(&action, state, ctx);
                }
                TrieLookup::PendingWithFallback(_) | TrieLookup::Pending => {
                    state.pending_keys.push(kb);
                }
                TrieLookup::NoMatch => {}
            }
        }
    }
}

/// Handle a pending key sequence timeout.
///
/// If the pending sequence has a fallback action, execute it.
/// Otherwise, clear the pending state.
pub fn handle_pending_timeout(state: &mut AppState, ctx: &AppContext) {
    let active_contexts = build_active_contexts(state, ctx);
    let lookup = ctx.keymap.lookup(state.pending_keys.keys(), &active_contexts);

    match lookup {
        TrieLookup::PendingWithFallback(action) => {
            let action = action.clone();
            state.pending_keys.clear();
            dispatch_action(&action, state, ctx);
        }
        _ => {
            state.pending_keys.clear();
        }
    }
}

/// Build the set of active key contexts from the current state.
fn build_active_contexts(state: &AppState, ctx: &AppContext) -> BTreeSet<KeyContext> {
    let mut active_contexts = BTreeSet::new();
    if ctx.ipc_server.is_some() {
        active_contexts.insert(KeyContext::Daemon);
    }
    let node_ctx = state.tree_state.current_node_info().map_or(KeyContext::File, |info| {
        if info.is_dir { KeyContext::Directory } else { KeyContext::File }
    });
    active_contexts.insert(node_ctx);
    active_contexts
}

/// Dispatch a resolved action.
fn dispatch_action(action: &Action, state: &mut AppState, ctx: &AppContext) {
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
        Action::Filter(filter_action) => {
            handle_filter_action(*filter_action, state, ctx);
        }
        Action::Shell { cmd, background } => {
            if *background {
                handle_shell_background(cmd, state);
            } else {
                handle_shell_action(cmd, state);
            }
        }
        Action::Notify(method) => {
            handle_notify_action(method, state, ctx);
        }
        Action::OpenMenu(name) => {
            handle_open_menu(name, state, ctx);
        }
        Action::Search(search_action) => match search_action {
            crate::action::SearchAction::Open => {
                search::open_search(state);
            }
        },
        Action::Noop => {}
    }
}

/// Handle a filter action (toggle hidden/ignored visibility).
fn handle_filter_action(
    action: crate::action::FilterAction,
    state: &mut AppState,
    ctx: &AppContext,
) {
    use crate::action::FilterAction;

    match action {
        FilterAction::Hidden => {
            state.show_hidden = !state.show_hidden;
            tree::rebuild_tree(state, ctx);
            let label = if state.show_hidden { "shown" } else { "hidden" };
            state.set_status(format!("Hidden files: {label}"));
        }
        FilterAction::Ignored => {
            state.show_ignored = !state.show_ignored;
            tree::rebuild_tree(state, ctx);
            let label = if state.show_ignored { "shown" } else { "hidden" };
            state.set_status(format!("Ignored files: {label}"));
        }
    }
}

/// Open a user-defined menu by name.
///
/// Looks up the menu definition from the config, resolves each item's
/// action/run/notify to an `Action`, and opens a `Menu` overlay.
fn handle_open_menu(name: &str, state: &mut AppState, ctx: &AppContext) {
    let Some(menu_def) = ctx.menus.get(name) else {
        tracing::warn!(name, "unknown menu name");
        state.set_status(format!("Unknown menu: {name}"));
        return;
    };

    if menu_def.items.is_empty() {
        return;
    }

    let mut items = Vec::with_capacity(menu_def.items.len());
    let mut item_actions = Vec::with_capacity(menu_def.items.len());

    for item_def in &menu_def.items {
        let resolved = resolve_menu_item_action(item_def);
        let Some(action) = resolved else {
            tracing::warn!(key = %item_def.key, label = %item_def.label, "skipping menu item with invalid action");
            continue;
        };
        let key = item_def.key.chars().next().unwrap_or(' ');
        items.push(crate::input::MenuItem {
            key,
            label: item_def.label.clone(),
            value: String::new(),
        });
        item_actions.push(action);
    }

    if items.is_empty() {
        return;
    }

    state.mode = AppMode::Menu(crate::input::MenuState {
        title: menu_def.title.clone(),
        items,
        cursor: 0,
        on_select: crate::input::MenuAction::Custom,
        item_actions,
    });
}

/// Resolve a menu item definition to an `Action`.
///
/// Returns `None` if the item has no valid action/run/notify or if parsing fails.
fn resolve_menu_item_action(item: &crate::config::MenuItemDef) -> Option<Action> {
    if let Some(ref action_str) = item.action {
        return action_str.parse::<Action>().ok();
    }
    if let Some(ref cmd) = item.run {
        return Some(Action::Shell { cmd: cmd.clone(), background: item.background });
    }
    if let Some(ref method) = item.notify {
        return Some(Action::Notify(method.clone()));
    }
    None
}

/// Execute a shell command with template variable substitution.
///
/// Suspends the TUI (alternate screen + raw mode + keyboard enhancement),
/// runs the command via `sh -c`, waits for the user to press Enter,
/// then resumes the TUI and requests a full redraw.
///
/// Template variables: `{path}`, `{dir}`, `{name}`, `{root}`.
fn handle_shell_action(cmd: &str, state: &mut AppState) {
    use std::io::Write;

    use crossterm::event::{
        DisableMouseCapture,
        EnableMouseCapture,
        KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    };

    let expanded = expand_shell_template(cmd, &state.tree_state);

    // Suspend TUI.
    let _ = crossterm::execute!(std::io::stdout(), PopKeyboardEnhancementFlags);
    let _ = crossterm::execute!(std::io::stdout(), DisableMouseCapture);
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);

    // Execute command.
    let result = std::process::Command::new("sh").arg("-c").arg(&expanded).status();

    // Show result and wait for user to press Enter.
    match &result {
        Ok(status) if !status.success() => {
            let code = status.code().map_or_else(|| "unknown".to_string(), |c| c.to_string());
            let _ = writeln!(std::io::stdout(), "\nProcess exited with code {code}");
        }
        Err(e) => {
            let _ = writeln!(std::io::stdout(), "\nFailed to execute command: {e}");
        }
        Ok(_) => {}
    }
    let _ = write!(std::io::stdout(), "\nPress ENTER to continue...");
    let _ = std::io::stdout().flush();
    let _ = std::io::stdin().read_line(&mut String::new());

    // Resume TUI.
    let _ = crossterm::terminal::enable_raw_mode();
    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen);
    let _ = crossterm::execute!(std::io::stdout(), EnableMouseCapture);
    let _ = crossterm::execute!(
        std::io::stdout(),
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    );

    state.needs_redraw = true;

    if let Ok(status) = &result {
        if !status.success() {
            let code = status.code().map_or_else(|| "unknown".to_string(), |c| c.to_string());
            state.set_status(format!("Command exited with code {code}"));
        }
    } else if let Err(e) = &result {
        state.set_status(format!("Failed to execute command: {e}"));
    }
}

/// Execute a shell command in the background without suspending the TUI.
///
/// The command runs detached with stdout/stderr discarded. Success or failure
/// is reported via the status bar message.
fn handle_shell_background(cmd: &str, state: &mut AppState) {
    let expanded = expand_shell_template(cmd, &state.tree_state);

    match std::process::Command::new("sh")
        .arg("-c")
        .arg(&expanded)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(_) => {
            state.set_status(format!("Running: {expanded}"));
        }
        Err(e) => {
            state.set_status(format!("Failed to execute command: {e}"));
        }
    }
}

/// Send an IPC notification to the connected client.
///
/// Includes the cursor file path in the notification params.
/// Silently drops if no IPC server is running (non-daemon mode).
fn handle_notify_action(method: &str, state: &AppState, ctx: &AppContext) {
    let Some(server) = &ctx.ipc_server else {
        return;
    };
    let path = state.tree_state.cursor_path().unwrap_or_default();
    let params = serde_json::json!({
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
#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "Uses {path}/{dir}/{name}/{root} as template placeholders, not format args"
)]
fn expand_shell_template(template: &str, tree_state: &crate::state::tree::TreeState) -> String {
    let info = tree_state.current_node_info();
    let root = tree_state.root_path();

    let path_str = info.as_ref().map_or_else(String::new, |i| i.path.display().to_string());
    let dir_str =
        tree_state.cursor_dir_path().map_or_else(String::new, |p| p.display().to_string());
    let name_str = info.as_ref().map_or_else(String::new, |i| i.name.clone());
    let root_str = root.display().to_string();

    template
        .replace("{path}", &path_str)
        .replace("{dir}", &dir_str)
        .replace("{name}", &name_str)
        .replace("{root}", &root_str)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::literal_string_with_formatting_args)]
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
            is_ignored: false,
            is_root: false,
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
            is_ignored: false,
            is_root: true,
        };
        let mut state = TreeState::new(root_node, TreeOptions::default());
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
            is_ignored: false,
            is_root: false,
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
            is_ignored: false,
            is_root: true,
        };
        let mut state = TreeState::new(root_node, TreeOptions::default());
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
        assert_that!(result.as_str(), eq("cp /test/root/test.txt /test/root/test.txt.bak"));
    }

    #[rstest]
    fn expand_template_no_vars() {
        let root = Path::new("/test/root");
        let state = state_with_file("test.txt", root);
        let result = super::expand_shell_template("echo hello", &state);
        assert_that!(result.as_str(), eq("echo hello"));
    }

    // --- resolve_menu_item_action ---

    #[rstest]
    fn resolve_menu_item_action_with_action() {
        let item = crate::config::MenuItemDef {
            key: "n".to_string(),
            label: "Sort by name".to_string(),
            action: Some("tree.sort.by_name".to_string()),
            run: None,
            notify: None,
            background: false,
        };
        let result = super::resolve_menu_item_action(&item);
        assert!(result.is_some());
        let action = result.unwrap();
        assert_that!(action.to_string().as_str(), eq("tree.sort.by_name"));
    }

    #[rstest]
    fn resolve_menu_item_action_with_run() {
        let item = crate::config::MenuItemDef {
            key: "g".to_string(),
            label: "Git status".to_string(),
            action: None,
            run: Some("git status".to_string()),
            notify: None,
            background: false,
        };
        let result = super::resolve_menu_item_action(&item);
        assert!(result.is_some());
        assert_that!(result.unwrap().to_string().as_str(), eq("shell:git status"));
    }

    #[rstest]
    fn resolve_menu_item_action_with_notify() {
        let item = crate::config::MenuItemDef {
            key: "o".to_string(),
            label: "Open file".to_string(),
            action: None,
            run: None,
            notify: Some("open_file".to_string()),
            background: false,
        };
        let result = super::resolve_menu_item_action(&item);
        assert!(result.is_some());
        assert_that!(result.unwrap().to_string().as_str(), eq("notify:open_file"));
    }

    #[rstest]
    fn resolve_menu_item_action_with_invalid_action() {
        let item = crate::config::MenuItemDef {
            key: "x".to_string(),
            label: "Invalid".to_string(),
            action: Some("not_a_real_action".to_string()),
            run: None,
            notify: None,
            background: false,
        };
        let result = super::resolve_menu_item_action(&item);
        assert!(result.is_none());
    }

    #[rstest]
    fn resolve_menu_item_action_with_no_fields() {
        let item = crate::config::MenuItemDef {
            key: "x".to_string(),
            label: "Nothing".to_string(),
            action: None,
            run: None,
            notify: None,
            background: false,
        };
        let result = super::resolve_menu_item_action(&item);
        assert!(result.is_none());
    }

    #[rstest]
    fn resolve_menu_item_action_prefers_action_over_run() {
        let item = crate::config::MenuItemDef {
            key: "a".to_string(),
            label: "Both".to_string(),
            action: Some("quit".to_string()),
            run: Some("echo hello".to_string()),
            notify: None,
            background: false,
        };
        let result = super::resolve_menu_item_action(&item);
        assert!(result.is_some());
        assert_that!(result.unwrap().to_string().as_str(), eq("quit"));
    }

    // --- handle_open_menu builds MenuState in config order ---

    /// Build a `MenuDefinition` with items in a specific order.
    fn sample_menu_definition() -> crate::config::MenuDefinition {
        crate::config::MenuDefinition {
            title: "Test Menu".to_string(),
            items: vec![
                crate::config::MenuItemDef {
                    key: "a".to_string(),
                    label: "Alpha".to_string(),
                    action: Some("quit".to_string()),
                    run: None,
                    notify: None,
                    background: false,
                },
                crate::config::MenuItemDef {
                    key: "b".to_string(),
                    label: "Beta".to_string(),
                    run: Some("echo beta".to_string()),
                    action: None,
                    notify: None,
                    background: false,
                },
                crate::config::MenuItemDef {
                    key: "c".to_string(),
                    label: "Charlie".to_string(),
                    notify: Some("do_thing".to_string()),
                    action: None,
                    run: None,
                    background: false,
                },
            ],
        }
    }

    /// Verify that building menu items from a `MenuDefinition` preserves order.
    #[rstest]
    fn open_menu_builds_items_in_config_order() {
        let menu_def = sample_menu_definition();
        let mut items = Vec::new();
        let mut item_actions = Vec::new();

        for item_def in &menu_def.items {
            let Some(action) = super::resolve_menu_item_action(item_def) else {
                continue;
            };
            let key = item_def.key.chars().next().unwrap_or(' ');
            items.push(crate::input::MenuItem {
                key,
                label: item_def.label.clone(),
                value: String::new(),
            });
            item_actions.push(action);
        }

        assert_that!(items.len(), eq(3));
        assert_that!(items[0].key, eq('a'));
        assert_that!(items[0].label.as_str(), eq("Alpha"));
        assert_that!(items[1].key, eq('b'));
        assert_that!(items[1].label.as_str(), eq("Beta"));
        assert_that!(items[2].key, eq('c'));
        assert_that!(items[2].label.as_str(), eq("Charlie"));

        assert_that!(item_actions.len(), eq(3));
        assert_that!(item_actions[0].to_string().as_str(), eq("quit"));
        assert_that!(item_actions[1].to_string().as_str(), eq("shell:echo beta"));
        assert_that!(item_actions[2].to_string().as_str(), eq("notify:do_thing"));
    }

    /// Verify that invalid items are skipped during menu build.
    #[rstest]
    fn open_menu_skips_invalid_items() {
        let menu_def = crate::config::MenuDefinition {
            title: "Test".to_string(),
            items: vec![
                crate::config::MenuItemDef {
                    key: "a".to_string(),
                    label: "Valid".to_string(),
                    action: Some("quit".to_string()),
                    run: None,
                    notify: None,
                    background: false,
                },
                crate::config::MenuItemDef {
                    key: "b".to_string(),
                    label: "Invalid".to_string(),
                    action: Some("not_real".to_string()),
                    run: None,
                    notify: None,
                    background: false,
                },
                crate::config::MenuItemDef {
                    key: "c".to_string(),
                    label: "Also valid".to_string(),
                    run: Some("ls".to_string()),
                    action: None,
                    notify: None,
                    background: false,
                },
            ],
        };

        let mut items = Vec::new();
        for item_def in &menu_def.items {
            let Some(_action) = super::resolve_menu_item_action(item_def) else {
                continue;
            };
            items.push(item_def.label.clone());
        }

        assert_that!(items.len(), eq(2));
        assert_that!(items[0].as_str(), eq("Valid"));
        assert_that!(items[1].as_str(), eq("Also valid"));
    }
}
