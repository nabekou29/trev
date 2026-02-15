//! Input and confirm mode key handlers.
//!
//! Handles inline text editing (create/rename) and delete confirmation dialogs.

use super::file_op::{
    execute_create,
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

/// Build a status message for the given input action.
fn status_message_for(input: &crate::input::InputState) -> Option<String> {
    use crate::input::InputAction;

    if input.value.trim().is_empty() {
        return None;
    }

    match &input.on_confirm {
        InputAction::Create { .. } => Some(format!("Created {}", input.value)),
        InputAction::Rename { .. } => Some(format!("Renamed to {}", input.value)),
    }
}

/// Execute the confirmed input action (create or rename).
fn execute_input_confirm(state: &mut AppState, input: crate::input::InputState, ctx: &AppContext) {
    use crate::input::InputAction;

    if input.value.trim().is_empty() {
        return;
    }

    match input.on_confirm {
        InputAction::Create { parent_dir } => {
            execute_create(&parent_dir, &input.value, state, ctx);
        }
        InputAction::Rename { target } => {
            execute_rename(&target, &input.value, state, ctx);
        }
    }
}
