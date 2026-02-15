//! Status bar widget — displays file path and cursor position.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{
    Color,
    Style,
};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::app::AppState;

/// Render the status bar into the given area.
///
/// Shows the current file path and cursor position (N/Total).
pub fn render_status(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let visible_count = state.tree_state.visible_node_count();
    let cursor = state.tree_state.cursor();

    let info = state
        .tree_state
        .current_node_info()
        .map_or_else(String::new, |node_info| node_info.path.display().to_string());

    let position = if visible_count > 0 {
        format!(" {}/{}", cursor + 1, visible_count)
    } else {
        String::new()
    };

    let status_text = format!("{info}{position}");
    let status_line = Line::raw(status_text);

    let paragraph =
        Paragraph::new(status_line).style(Style::default().bg(Color::DarkGray).fg(Color::White));

    frame.render_widget(paragraph, area);
}
