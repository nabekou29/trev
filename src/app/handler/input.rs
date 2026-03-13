//! Input, confirm, and menu mode key handlers.
//!
//! Handles inline text editing (create/rename), delete confirmation dialogs,
//! and selection menu overlays.

use super::file_op::{
    execute_create,
    execute_create_directory,
    execute_delete,
    execute_rename,
};
use crate::app::state::{
    AppContext,
    AppState,
};
use crate::input::AppMode;

/// Handle key events in Confirm mode (delete confirmation dialog).
pub fn handle_confirm_mode_key(
    key: crossterm::event::KeyEvent,
    state: &mut AppState,
    ctx: &AppContext,
) {
    use crossterm::event::KeyCode;

    match key.code {
        // Confirm: execute the delete operation.
        KeyCode::Char('y') | KeyCode::Enter => {
            let AppMode::Confirm(confirm) = std::mem::take(&mut state.mode) else {
                return;
            };
            execute_delete(confirm, state, ctx);
        }
        // Cancel: return to Normal mode.
        KeyCode::Char('n') | KeyCode::Esc => {
            state.mode = AppMode::Normal;
        }
        _ => {}
    }
}

/// Handle key events in Input mode (inline text editing).
pub fn handle_input_mode_key(
    key: crossterm::event::KeyEvent,
    state: &mut AppState,
    ctx: &AppContext,
) {
    // Take ownership of the input state temporarily.
    let AppMode::Input(ref mut input_state) = state.mode else {
        return;
    };

    match input_state.handle_key_event(key) {
        Some(true) => {
            // Confirmed — execute the operation.
            let AppMode::Input(input) = std::mem::take(&mut state.mode) else {
                return;
            };
            let status_msg = status_message_for(&input);
            execute_input_confirm(state, input, ctx);
            if let Some(msg) = status_msg {
                state.set_status(msg);
            }
        }
        Some(false) => {
            // Cancelled — return to Normal mode.
            state.mode = AppMode::Normal;
        }
        None => {
            // Regular editing key — state already mutated.
        }
    }
}

/// Handle key events in Menu mode (selection menu overlay).
///
/// Supports both direct shortcut keys and j/k/arrow navigation with Enter to confirm.
pub fn handle_menu_mode_key(
    key: crossterm::event::KeyEvent,
    state: &mut AppState,
    ctx: &AppContext,
) {
    use crossterm::event::KeyCode;

    let AppMode::Menu(ref mut menu) = state.mode else {
        return;
    };

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.mode = AppMode::Normal;
        }
        // Navigate down.
        KeyCode::Char('j') | KeyCode::Down => {
            let len = menu.items.len();
            if len > 0 {
                menu.cursor = (menu.cursor + 1) % len;
            }
        }
        // Navigate up.
        KeyCode::Char('k') | KeyCode::Up => {
            let len = menu.items.len();
            if len > 0 {
                menu.cursor = (menu.cursor + len - 1) % len;
            }
        }
        // Confirm current selection.
        KeyCode::Enter => {
            let selected_idx = menu.cursor;
            let selected = menu.items.get(selected_idx).map(|i| (i.label.clone(), i.value.clone()));
            let on_select = menu.on_select;

            if matches!(on_select, crate::input::MenuAction::Custom) {
                let action = menu.item_actions.get(selected_idx).cloned();
                state.mode = AppMode::Normal;
                if let Some(action) = action {
                    dispatch_custom_action(action, state, ctx);
                }
            } else {
                state.mode = AppMode::Normal;
                if let Some((label, value)) = selected {
                    dispatch_menu_action(on_select, state, &label, &value);
                }
            }
        }
        // Direct shortcut key.
        KeyCode::Char(ch) => {
            let on_select = menu.on_select;

            if matches!(on_select, crate::input::MenuAction::Custom) {
                let matched = menu
                    .items
                    .iter()
                    .zip(menu.item_actions.iter())
                    .find(|(item, _)| item.key == ch)
                    .map(|(_, action)| action.clone());
                if let Some(action) = matched {
                    state.mode = AppMode::Normal;
                    dispatch_custom_action(action, state, ctx);
                }
            } else {
                let matched = menu
                    .items
                    .iter()
                    .find(|i| i.key == ch)
                    .map(|i| (i.label.clone(), i.value.clone()));
                if let Some((label, value)) = matched {
                    state.mode = AppMode::Normal;
                    dispatch_menu_action(on_select, state, &label, &value);
                }
            }
        }
        _ => {}
    }
}

/// Dispatch the selected menu item based on the menu action type.
fn dispatch_menu_action(
    action: crate::input::MenuAction,
    state: &mut AppState,
    label: &str,
    value: &str,
) {
    use crate::input::MenuAction;

    match action {
        MenuAction::CopyToClipboard => {
            copy_to_clipboard(state, label, value);
        }
        MenuAction::SelectSortOrder => {
            apply_sort_from_menu(state, value);
        }
        MenuAction::Custom => {
            // Custom menu actions are dispatched via dispatch_custom_menu_item.
            // This case should not be reached through the normal flow.
        }
    }
}

/// Dispatch a custom menu action by re-using the main action handlers.
fn dispatch_custom_action(action: crate::action::Action, state: &mut AppState, ctx: &AppContext) {
    use crate::action::Action;

    match action {
        Action::Tree(tree_action) => {
            super::tree::handle_tree_action(tree_action, state, ctx);
        }
        Action::FileOp(file_op_action) => {
            super::file_op::handle_file_op_action(file_op_action, state, ctx);
        }
        Action::Filter(filter_action) => {
            super::handle_filter_action(filter_action, state, ctx);
        }
        Action::Preview(preview_action) => {
            super::preview::handle_preview_action(preview_action, state, ctx);
        }
        Action::Shell { cmd, background } => {
            if background {
                super::handle_shell_background(&cmd, state);
            } else {
                super::handle_shell_action(&cmd, state);
            }
        }
        Action::Notify(method) => {
            super::handle_notify_action(&method, state, ctx);
        }
        Action::Quit => {
            state.should_quit = true;
        }
        Action::OpenEditor => {
            super::handle_open_editor(state);
        }
        Action::Search(_) | Action::OpenMenu(_) | Action::ShowHelp | Action::Noop => {}
    }
}

/// Apply a sort order from the menu selection.
///
/// Parses the value as a `SortOrder`, updates the tree state, and re-sorts.
fn apply_sort_from_menu(state: &mut AppState, value: &str) {
    use clap::ValueEnum;

    use crate::state::tree::SortOrder;

    let Ok(config_order) = crate::config::SortOrder::from_str(value, true) else {
        state.set_status(format!("Unknown sort order: {value}"));
        return;
    };
    let order: SortOrder = config_order.into();
    let direction = state.tree_state.sort_direction();
    let dirs_first = state.tree_state.directories_first();
    state.tree_state.apply_sort(order, direction, dirs_first);
    state.set_status(format!("Sort: {value}"));
}

/// Copy text to the system clipboard and set a status message.
pub(super) fn copy_to_clipboard(state: &mut AppState, label: &str, value: &str) {
    match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(value)) {
        Ok(()) => {
            state.set_status(format!("Copied {label}: {value}"));
        }
        Err(e) => {
            tracing::warn!(%e, "failed to copy to clipboard");
            state.set_status(format!("Clipboard error: {e}"));
        }
    }
}

/// Build a status message for the given input action.
fn status_message_for(input: &crate::input::InputState) -> Option<String> {
    use crate::input::InputAction;

    if input.buffer.value.trim().is_empty() {
        return None;
    }

    match &input.on_confirm {
        InputAction::Create { .. } => Some(format!("Created {}", input.buffer.value)),
        InputAction::CreateDirectory { .. } => {
            Some(format!("Created directory {}", input.buffer.value))
        }
        InputAction::Rename { .. } => Some(format!("Renamed to {}", input.buffer.value)),
    }
}

/// Execute the confirmed input action (create or rename).
fn execute_input_confirm(state: &mut AppState, input: crate::input::InputState, ctx: &AppContext) {
    use crate::input::InputAction;

    if input.buffer.value.trim().is_empty() {
        return;
    }

    match input.on_confirm {
        InputAction::Create { parent_dir } => {
            execute_create(&parent_dir, &input.buffer.value, state, ctx);
        }
        InputAction::CreateDirectory { parent_dir } => {
            execute_create_directory(&parent_dir, &input.buffer.value, state, ctx);
        }
        InputAction::Rename { target } => {
            execute_rename(&target, &input.buffer.value, state, ctx);
        }
    }
}
