//! Inline input widget for tree view (create/rename operations).
//!
//! Renders a bordered, rounded input box with a title and cursor.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{
    Color,
    Modifier,
    Style,
};
use ratatui::text::{
    Line,
    Span,
};
use ratatui::widgets::{
    Block,
    BorderType,
    Borders,
    Clear,
    Paragraph,
};

use crate::input::InputState;

/// Render an inline input field with a rounded border and title.
///
/// The box has the structure:
/// ```text
/// ╭─ Title ──────────────────────────╮
/// │ input_value█                     │
/// ╰──────────────────────────────────╯
/// ```
pub fn render_inline_input(frame: &mut Frame<'_>, area: Rect, input: &InputState) {
    // Clear the area behind the input box.
    frame.render_widget(Clear, area);

    // Build the bordered block with rounded corners and title.
    let title = input.title();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ));

    // Build the input line with cursor.
    let mut spans = Vec::new();

    // Split value at cursor position to render cursor indicator.
    let (before, after) = input.buffer.value.split_at(input.buffer.cursor_pos);

    spans.push(Span::raw(before.to_string()));

    // Render cursor as inverted character (or space if at end).
    let cursor_style = Style::default().bg(Color::White).fg(Color::Black);

    let mut after_chars = after.chars();
    if let Some(cursor_char) = after_chars.next() {
        spans.push(Span::styled(cursor_char.to_string(), cursor_style));
        let rest: String = after_chars.collect();
        if !rest.is_empty() {
            spans.push(Span::raw(rest));
        }
    } else {
        // Cursor at end of input — show a space block.
        spans.push(Span::styled(" ", cursor_style));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).block(block);
    frame.render_widget(paragraph, area);
}
