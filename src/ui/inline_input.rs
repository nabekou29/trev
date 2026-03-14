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
use ratatui::text::Line;
use ratatui::text::Span;
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
pub fn render_inline_input(frame: &mut Frame<'_>, area: Rect, input: &mut InputState) {
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

    // Compute inner width for viewport-aware text clipping.
    let inner = block.inner(area);
    let viewport_width = inner.width as usize;

    // Build the input line with cursor (clipped to viewport).
    let mut spans = Vec::new();
    input.buffer.push_viewport_cursor_spans(&mut spans, viewport_width);

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).block(block);
    frame.render_widget(paragraph, area);
}
