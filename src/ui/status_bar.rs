//! Status bar widget — displays indicators, status messages, file path, and cursor position.

use ratatui::Frame;
use ratatui::layout::{
    Constraint,
    Layout,
    Rect,
};
use ratatui::style::{
    Color,
    Style,
};
use ratatui::text::{
    Line,
    Span,
};
use ratatui::widgets::Paragraph;

use crate::app::AppState;
use crate::file_op::selection::SelectionMode;

/// Render the status bar into the given area.
///
/// Layout: `[indicators] [message or path] ... [position]`
#[allow(clippy::cast_possible_truncation)]
pub fn render_status(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let base_style = Style::default().bg(Color::DarkGray).fg(Color::White);

    // --- Build indicator spans (left side) ---
    let indicator_spans = build_indicator_spans(state);
    let indicator_width: u16 = indicator_spans.iter().map(|s| s.width() as u16).sum();

    // --- Build position string (right side) ---
    let visible_count = state.tree_state.visible_node_count();
    let cursor = state.tree_state.cursor();
    let position = if visible_count > 0 {
        format!(" {}/{} ", cursor + 1, visible_count)
    } else {
        String::new()
    };
    let position_width = position.len() as u16;

    // --- Layout: [indicators | center (fill) | position] ---
    let chunks = Layout::horizontal([
        Constraint::Length(indicator_width),
        Constraint::Fill(1),
        Constraint::Length(position_width),
    ])
    .split(area);

    let Some((&indicator_area, rest)) = chunks.split_first() else {
        return;
    };
    let Some((&center_area, rest)) = rest.split_first() else {
        return;
    };
    let Some(&position_area) = rest.first() else {
        return;
    };

    // Render indicators (left).
    if !indicator_spans.is_empty() {
        let indicator_line = Line::from(indicator_spans);
        frame.render_widget(Paragraph::new(indicator_line).style(base_style), indicator_area);
    }

    // Render center content (message or file path).
    let center_text = build_center_text(state);
    let center_line = Line::raw(center_text);
    frame.render_widget(Paragraph::new(center_line).style(base_style), center_area);

    // Render position (right).
    let position_line = Line::raw(position);
    frame.render_widget(Paragraph::new(position_line).style(base_style), position_area);
}

/// Build indicator span for selection state.
fn build_indicator_spans(state: &AppState) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let count = state.selection.count();

    if count > 0 {
        match state.selection.mode() {
            Some(SelectionMode::Copy) => {
                spans.push(Span::styled(format!(" [Y:{count}]"), Style::default().fg(Color::Cyan)));
            }
            Some(SelectionMode::Cut) => {
                spans.push(Span::styled(
                    format!(" [X:{count}]"),
                    Style::default().fg(Color::Yellow),
                ));
            }
            Some(SelectionMode::Mark) => {
                spans
                    .push(Span::styled(format!(" [M:{count}]"), Style::default().fg(Color::Green)));
            }
            None => {}
        }
    }

    spans
}

/// Determine the center text: processing > status message > file path.
fn build_center_text(state: &AppState) -> String {
    if state.processing {
        return " Processing...".to_string();
    }

    if let Some(msg) = &state.status_message {
        return format!(" {}", msg.text);
    }

    // Default: file path.
    state
        .tree_state
        .current_node_info()
        .map_or_else(String::new, |node_info| format!(" {}", node_info.path.display()))
}
