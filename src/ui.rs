//! UI layout and rendering.

pub mod modal;
pub mod preview_view;
pub mod status_bar;
pub mod tree_view;

use ratatui::Frame;
use ratatui::layout::{
    Constraint,
    Layout,
};

use crate::app::AppState;

/// Render the entire UI.
///
/// Splits the frame into a tree view area (all remaining space) and
/// a status bar area (1 row at the bottom).
pub fn render(frame: &mut Frame, state: &mut AppState) {
    let chunks = Layout::vertical([
        Constraint::Min(1),    // tree view
        Constraint::Length(1), // status bar
    ])
    .split(frame.area());

    let Some(&tree_area) = chunks.first() else {
        return;
    };
    let Some(&status_area) = chunks.get(1) else {
        return;
    };

    // Update viewport height from actual layout.
    state.viewport_height = tree_area.height;

    tree_view::render_tree(frame, tree_area, state);
    status_bar::render_status(frame, status_area, state);
}
