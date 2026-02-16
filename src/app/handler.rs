//! Action handlers dispatched from the main event loop.

mod file_op;
mod input;
mod preview;
mod tree;

use file_op::handle_file_op_action;
use input::{
    handle_confirm_mode_key,
    handle_input_mode_key,
};
use preview::handle_preview_action;
pub use preview::trigger_preview;
use tree::handle_tree_action;
pub use tree::{
    refresh_directory,
    trigger_prefetch,
};

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
        AppMode::Normal => {
            let Some(action) = ctx.keymap.resolve(key) else {
                return;
            };
            match *action {
                crate::action::Action::Quit => {
                    state.should_quit = true;
                }
                crate::action::Action::Tree(tree_action) => {
                    handle_tree_action(tree_action, state, ctx);
                }
                crate::action::Action::Preview(preview_action) => {
                    handle_preview_action(preview_action, state, ctx);
                }
                crate::action::Action::FileOp(file_op_action) => {
                    handle_file_op_action(file_op_action, state, ctx);
                }
            }
        }
    }
}
