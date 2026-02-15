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

/// Render the entire UI.
///
/// Splits the frame into:
/// - Main content area: tree view (50%) | preview (50%) when preview is enabled
/// - Status bar (1 row at the bottom)
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
        // Split main area into tree (50%) and preview (50%).
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_area);

        let Some(&tree_area) = content_chunks.first() else {
            return;
        };
        let Some(&preview_area) = content_chunks.get(1) else {
            return;
        };

        state.viewport_height = tree_area.height;

        tree_view::render_tree(frame, tree_area, state);
        preview_view::render_preview(frame, preview_area, &mut state.preview_state);
    } else {
        // Full width tree when preview is disabled.
        state.viewport_height = main_area.height;

        tree_view::render_tree(frame, main_area, state);
    }

    status_bar::render_status(frame, status_area, state);
}
