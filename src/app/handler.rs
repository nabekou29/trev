//! Action handlers dispatched from the main event loop.

mod file_op;
mod input;
pub mod ipc;
mod mouse;
pub(super) mod preview;
pub mod search;
pub mod stat;
pub mod tree;

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
    let mode_name = match state.mode {
        AppMode::Input(_) => "input",
        AppMode::Confirm(_) => "confirm",
        AppMode::Menu(_) => "menu",
        AppMode::Normal => "normal",
        AppMode::Search(_) => "search",
        AppMode::Help(_) => "help",
    };
    let _span = tracing::info_span!("handle_key_event", mode = mode_name).entered();

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
        AppMode::Help(_) => {
            handle_help_mode_key(key, state, ctx);
        }
    }
}

/// Handle a key event in Normal mode with multi-key sequence support.
pub(super) fn handle_normal_mode_key(
    key: crossterm::event::KeyEvent,
    state: &mut AppState,
    ctx: &AppContext,
) {
    let _span = tracing::info_span!("handle_normal_mode_key").entered();
    let active_contexts = {
        let _span = tracing::info_span!("build_active_contexts").entered();
        build_active_contexts(state, ctx)
    };
    let kb: KeyBinding = (key.code, key.modifiers);

    state.pending_keys.push(kb);

    let lookup_result = {
        let _span = tracing::info_span!("keymap_lookup").entered();
        ctx.keymap.lookup(state.pending_keys.keys(), &active_contexts)
    };
    match lookup_result {
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
    let _span = tracing::info_span!("dispatch_action", action = %action).entered();
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
        Action::Shell { cmd, run_mode } => {
            handle_shell_with_mode(cmd, *run_mode, state);
        }
        Action::Notify(method) => {
            handle_notify_action(method, state, ctx);
        }
        Action::OpenMenu(name) => {
            handle_open_menu(name, state, ctx);
        }
        Action::OpenEditor => {
            handle_open_editor(state);
        }
        Action::Search(search_action) => match search_action {
            crate::action::SearchAction::Open => {
                search::open_search(state);
            }
        },
        Action::ShowHelp => {
            open_help(state, ctx);
        }
        Action::Noop => {}
    }
}

/// Open the keybinding help overlay.
///
/// Collects all actions (bound and unbound) and stores them in `HelpState`.
/// Bound actions include their key display; unbound actions show no key.
fn open_help(state: &mut AppState, ctx: &AppContext) {
    use crate::input::{
        HelpBinding,
        HelpState,
        TextBuffer,
    };

    let lookup = &ctx.action_key_lookup;

    // Build bindings for ALL static actions using the cached key lookup.
    let mut bindings: Vec<HelpBinding> = Action::all_action_names()
        .into_iter()
        .filter(|name| !matches!(*name, "noop" | "help"))
        .filter_map(|name| {
            let action = name.parse::<Action>().ok()?;
            let (key_display, has_keybinding) = lookup
                .key_for(name)
                .map_or_else(|| (String::new(), false), |k| (k.to_string(), true));
            Some(HelpBinding {
                key_display,
                action_name: name.to_string(),
                description: action.description().to_string(),
                action,
                has_keybinding,
            })
        })
        .collect();

    // Append custom actions from config.
    for (name, def) in &ctx.custom_actions {
        let Ok(action) = crate::config::resolve_custom_action_def(def) else {
            continue;
        };
        let action_display = action.to_string();
        let (key_display, has_keybinding) = lookup
            .key_for(&action_display)
            .map_or_else(|| (String::new(), false), |k| (k.to_string(), true));
        bindings.push(HelpBinding {
            key_display,
            action_name: format!("custom.{name}"),
            description: def.description.clone(),
            action,
            has_keybinding,
        });
    }
    bindings.sort_by(|a, b| {
        help_group_sort_key(&a.action_name)
            .cmp(&help_group_sort_key(&b.action_name))
            .then_with(|| a.action_name.cmp(&b.action_name))
    });

    state.mode = AppMode::Help(HelpState {
        scroll_offset: 0,
        cursor: 0,
        bindings,
        filter: TextBuffer::new(),
        filtering: false,
    });
    state.dirty = true;
}

/// Compute a sort key matching the help view's group display order.
fn help_group_sort_key(action_name: &str) -> usize {
    use crate::ui::help_view::GROUPS;

    GROUPS
        .iter()
        .position(|g| action_name.starts_with(g.prefix))
        .unwrap_or(GROUPS.len())
}

/// Handle a key event in Help mode.
///
/// When the filter input is active, keys go to the text buffer.
/// Otherwise, vim-style navigation keys move the cursor, and Enter
/// executes the selected action.
fn handle_help_mode_key(
    key: crossterm::event::KeyEvent,
    state: &mut AppState,
    ctx: &AppContext,
) {
    use crossterm::event::{
        KeyCode,
        KeyModifiers,
    };

    // Execute selected action on Enter (non-filter mode).
    // Handled before borrowing state.mode to avoid borrow conflict
    // with execute_help_action which needs &mut AppState.
    if matches!(&state.mode, AppMode::Help(h) if !h.filtering)
        && key.code == KeyCode::Enter
    {
        execute_help_action(state, ctx);
        return;
    }

    let AppMode::Help(ref mut help) = state.mode else {
        return;
    };

    // --- Filter input mode ---
    if help.filtering {
        match key.code {
            KeyCode::Esc => {
                help.filter.set_value("");
                help.filtering = false;
            }
            KeyCode::Enter => {
                help.filtering = false;
            }
            _ => {
                help.filter.handle_key_event(key);
            }
        }
        help.cursor = 0;
        help.scroll_offset = 0;
        state.dirty = true;
        return;
    }

    let filtered_count = help.filtered_bindings().len();
    let max_cursor = filtered_count.saturating_sub(1);

    // --- Normal cursor mode ---
    let changed = match (key.code, key.modifiers) {
        (KeyCode::Esc | KeyCode::Char('q' | '?'), _) => {
            state.mode = AppMode::Normal;
            true
        }
        (KeyCode::Char('/'), KeyModifiers::NONE) => {
            help.filtering = true;
            true
        }
        (KeyCode::Char('j') | KeyCode::Down, _) => {
            help.cursor = help.cursor.saturating_add(1).min(max_cursor);
            true
        }
        (KeyCode::Char('k') | KeyCode::Up, _) => {
            help.cursor = help.cursor.saturating_sub(1);
            true
        }
        (KeyCode::Char('g'), KeyModifiers::NONE) => {
            help.cursor = 0;
            true
        }
        (KeyCode::Char('G'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
            help.cursor = max_cursor;
            true
        }
        (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
            help.cursor = help.cursor.saturating_add(10).min(max_cursor);
            true
        }
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            help.cursor = help.cursor.saturating_sub(10);
            true
        }
        _ => false,
    };
    if changed {
        state.dirty = true;
    }
}

/// Execute the action at the current cursor position in the help view.
fn execute_help_action(state: &mut AppState, ctx: &AppContext) {
    let AppMode::Help(ref help) = state.mode else {
        return;
    };
    let filtered = help.filtered_bindings();
    let Some(binding) = filtered.get(help.cursor) else {
        return;
    };
    let action = binding.action.clone();

    // Close help first, then dispatch.
    state.mode = AppMode::Normal;
    state.dirty = true;
    dispatch_action(&action, state, ctx);
}

/// Handle a filter action (toggle hidden/ignored visibility).
pub(super) fn handle_filter_action(
    action: crate::action::FilterAction,
    state: &mut AppState,
    ctx: &AppContext,
) {
    use crate::action::FilterAction;

    match action {
        FilterAction::Hidden => {
            state.show_hidden = !state.show_hidden;
            // Clear stale search results that were collected under old visibility
            // settings. The rebuilt index will provide fresh results via
            // refresh_search().
            state.search_match_indices.clear();
            tree::rebuild_tree(state, ctx);
            rebuild_search_index(state, ctx);
            let label = if state.show_hidden { "shown" } else { "hidden" };
            state.set_status(format!("Hidden files: {label}"));
        }
        FilterAction::Ignored => {
            state.show_ignored = !state.show_ignored;
            // Clear stale search results (same reason as above).
            state.search_match_indices.clear();
            tree::rebuild_tree(state, ctx);
            rebuild_search_index(state, ctx);
            let label = if state.show_ignored { "shown" } else { "hidden" };
            state.set_status(format!("Ignored files: {label}"));
        }
    }
}

/// Cancel the in-flight search index build and start a new one with current
/// `show_hidden` / `show_ignored` settings.
fn rebuild_search_index(state: &mut AppState, ctx: &AppContext) {
    use std::sync::atomic::Ordering;

    use crate::tree::search_index::SearchIndex;

    // Cancel the previous build.
    state.search_index_cancelled.store(true, Ordering::Relaxed);

    // Clear the existing index so stale entries are not returned during rebuild.
    if let Ok(mut guard) = ctx.search_index.write() {
        *guard = SearchIndex::new();
    }

    // Start a new build with updated visibility settings.
    state.search_index_cancelled = crate::app::spawn_search_index_build(
        &ctx.search_index,
        &ctx.root_path,
        state.show_hidden,
        state.show_ignored,
        &ctx.search_index_ready_tx,
    );
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
        let resolved = resolve_menu_item_action(item_def, &ctx.custom_actions);
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
/// Supports `custom.<name>` references to user-defined custom actions.
fn resolve_menu_item_action(
    item: &crate::config::MenuItemDef,
    custom_actions: &std::collections::HashMap<String, crate::config::CustomActionDef>,
) -> Option<Action> {
    if let Some(ref action_str) = item.action {
        if let Some(name) = action_str.strip_prefix("custom.") {
            let def = custom_actions.get(name)?;
            return crate::config::resolve_custom_action_def(def).ok();
        }
        return action_str.parse::<Action>().ok();
    }
    if let Some(ref cmd) = item.run {
        return Some(Action::Shell { cmd: cmd.clone(), run_mode: item.run_mode });
    }
    if let Some(ref method) = item.notify {
        return Some(Action::Notify(method.clone()));
    }
    None
}

/// Open the current cursor path in the user's preferred editor.
///
/// Resolves the editor from `$VISUAL`, then `$EDITOR`, falling back to `vi`.
/// Suspends the TUI, runs the editor, then resumes.
pub(super) fn handle_open_editor(state: &mut AppState) {
    let Some(path) = state.tree_state.cursor_path() else {
        state.set_status("No file selected".to_string());
        return;
    };

    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());

    let cmd = format!("{editor} {}", path.display());
    handle_shell_foreground(&cmd, state, false);
}

/// Execute a shell command using the specified [`ShellMode`].
///
/// - `Foreground`: Suspend TUI, run command, show "Press ENTER to continue...", resume.
/// - `Background`: Run detached without suspending the TUI.
/// - `Interactive`: Suspend TUI, run command, resume immediately (for full-screen TUI apps).
///
/// Template variables: `{path}`, `{dir}`, `{name}`, `{root}`.
pub(super) fn handle_shell_with_mode(
    cmd: &str,
    mode: crate::action::ShellMode,
    state: &mut AppState,
) {
    use crate::action::ShellMode;

    match mode {
        ShellMode::Background => handle_shell_background(cmd, state),
        ShellMode::Foreground => handle_shell_foreground(cmd, state, true),
        ShellMode::Interactive => handle_shell_foreground(cmd, state, false),
    }
}

/// Execute a shell command in the foreground, optionally waiting for ENTER.
///
/// Suspends the TUI, runs the command via `sh -c`, optionally waits for the
/// user to press Enter, then resumes the TUI and requests a full redraw.
fn handle_shell_foreground(cmd: &str, state: &mut AppState, wait_for_enter: bool) {
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

    if wait_for_enter {
        // Show result and wait for user to press Enter.
        match &result {
            Ok(status) if !status.success() => {
                let code =
                    status.code().map_or_else(|| "unknown".to_string(), |c| c.to_string());
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
    }

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
            run_mode: crate::action::ShellMode::default(),
        };
        let result = super::resolve_menu_item_action(&item, &std::collections::HashMap::new());
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
            run_mode: crate::action::ShellMode::default(),
        };
        let result = super::resolve_menu_item_action(&item, &std::collections::HashMap::new());
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
            run_mode: crate::action::ShellMode::default(),
        };
        let result = super::resolve_menu_item_action(&item, &std::collections::HashMap::new());
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
            run_mode: crate::action::ShellMode::default(),
        };
        let result = super::resolve_menu_item_action(&item, &std::collections::HashMap::new());
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
            run_mode: crate::action::ShellMode::default(),
        };
        let result = super::resolve_menu_item_action(&item, &std::collections::HashMap::new());
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
            run_mode: crate::action::ShellMode::default(),
        };
        let result = super::resolve_menu_item_action(&item, &std::collections::HashMap::new());
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
                    run_mode: crate::action::ShellMode::default(),
                },
                crate::config::MenuItemDef {
                    key: "b".to_string(),
                    label: "Beta".to_string(),
                    run: Some("echo beta".to_string()),
                    action: None,
                    notify: None,
                    run_mode: crate::action::ShellMode::default(),
                },
                crate::config::MenuItemDef {
                    key: "c".to_string(),
                    label: "Charlie".to_string(),
                    notify: Some("do_thing".to_string()),
                    action: None,
                    run: None,
                    run_mode: crate::action::ShellMode::default(),
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
            let Some(action) = super::resolve_menu_item_action(item_def, &std::collections::HashMap::new()) else {
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
                    run_mode: crate::action::ShellMode::default(),
                },
                crate::config::MenuItemDef {
                    key: "b".to_string(),
                    label: "Invalid".to_string(),
                    action: Some("not_real".to_string()),
                    run: None,
                    notify: None,
                    run_mode: crate::action::ShellMode::default(),
                },
                crate::config::MenuItemDef {
                    key: "c".to_string(),
                    label: "Also valid".to_string(),
                    run: Some("ls".to_string()),
                    action: None,
                    notify: None,
                    run_mode: crate::action::ShellMode::default(),
                },
            ],
        };

        let mut items = Vec::new();
        for item_def in &menu_def.items {
            let Some(_action) = super::resolve_menu_item_action(item_def, &std::collections::HashMap::new()) else {
                continue;
            };
            items.push(item_def.label.clone());
        }

        assert_that!(items.len(), eq(2));
        assert_that!(items[0].as_str(), eq("Valid"));
        assert_that!(items[1].as_str(), eq("Also valid"));
    }
}
