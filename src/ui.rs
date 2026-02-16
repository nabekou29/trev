//! UI layout and rendering.

pub mod inline_input;
pub mod modal;
pub mod preview_view;
pub mod status_bar;
pub mod tree_view;

use ratatui::Frame;
use ratatui::layout::{
    Constraint,
    Direction,
    Layout,
};

use crate::app::AppState;
use crate::input::AppMode;

/// Width threshold below which the layout switches to vertical (tree above, preview below).
///
/// Also used by `app.rs` to auto-hide preview on narrow terminals at startup.
pub const NARROW_WIDTH_THRESHOLD: u16 = 80;

/// Render the entire UI.
///
/// Layout adapts to terminal width:
/// - Wide (> 80 cols): horizontal split — tree (50%) | preview (50%)
/// - Narrow (<= 80 cols): vertical split — tree (60%) / preview (40%)
/// - Preview off: tree only (full area)
pub fn render(frame: &mut Frame<'_>, state: &mut AppState) {
    let chunks = Layout::vertical([
        Constraint::Min(1),    // main content
        Constraint::Length(1), // status bar
    ])
    .split(frame.area());

    let Some(&main_area) = chunks.first() else {
        return;
    };
    let Some(&status_area) = chunks.get(1) else {
        return;
    };

    if state.show_preview {
        let is_narrow = main_area.width <= NARROW_WIDTH_THRESHOLD;

        let (direction, constraints) = if is_narrow {
            // Narrow: vertical split (tree on top, preview on bottom).
            (Direction::Vertical, [Constraint::Percentage(60), Constraint::Percentage(40)])
        } else {
            // Wide: horizontal split (tree left, preview right).
            (Direction::Horizontal, [Constraint::Percentage(50), Constraint::Percentage(50)])
        };

        let content_chunks = Layout::default()
            .direction(direction)
            .constraints(constraints)
            .split(main_area);

        let Some(&tree_area) = content_chunks.first() else {
            return;
        };
        let Some(&preview_area) = content_chunks.get(1) else {
            return;
        };

        state.viewport_height = tree_area.height;

        tree_view::render_tree(frame, tree_area, state);
        preview_view::render_preview(frame, preview_area, &mut state.preview_state, is_narrow);
    } else {
        // Full width tree when preview is disabled.
        state.viewport_height = main_area.height;

        tree_view::render_tree(frame, main_area, state);
    }

    status_bar::render_status(frame, status_area, state);

    // Render modal overlays on top of everything.
    match &state.mode {
        AppMode::Confirm(confirm) => {
            modal::render_confirm_dialog(frame, frame.area(), confirm);
        }
        AppMode::Menu(menu) => {
            modal::render_menu(frame, frame.area(), menu);
        }
        AppMode::Normal | AppMode::Input(_) => {}
    }
}
