//! Search input bar widget rendered at the bottom of the tree area.
//!
//! Displays a `/` prompt followed by the search query with a cursor.
//! Shows match count or indexing status on the right side.

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
    Clear,
    Paragraph,
};

use crate::input::{
    SearchMode,
    TextBuffer,
};

/// Render the search input bar into the given area (1 row high).
///
/// Layout: `[mode]/query█ ········· {count} matches`
///
/// In `Name` mode (default), the prompt is just `/`.
/// In `Path` mode, the prompt shows `path/`.
pub fn render_search_input(
    frame: &mut Frame<'_>,
    area: Rect,
    buffer: &TextBuffer,
    mode: SearchMode,
    match_count: Option<usize>,
    is_indexing: bool,
) {
    // Clear the area behind the input bar.
    frame.render_widget(Clear, area);

    let mut spans: Vec<Span<'_>> = Vec::new();

    // Mode indicator + prompt.
    let prompt_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    if mode == SearchMode::Path {
        spans.push(Span::styled("path", Style::default().fg(Color::DarkGray)));
    }
    spans.push(Span::styled("/", prompt_style));

    // Query text with cursor.
    let (before, after) = buffer.value.split_at(buffer.cursor_pos);
    spans.push(Span::raw(before.to_string()));

    // Cursor: inverted character (or space if at end).
    let cursor_style = Style::default().bg(Color::White).fg(Color::Black);
    let mut after_chars = after.chars();
    if let Some(cursor_char) = after_chars.next() {
        spans.push(Span::styled(cursor_char.to_string(), cursor_style));
        let rest: String = after_chars.collect();
        if !rest.is_empty() {
            spans.push(Span::raw(rest));
        }
    } else {
        spans.push(Span::styled(" ", cursor_style));
    }

    // Right-aligned status indicator.
    let status_text = if is_indexing {
        " (indexing...)".to_string()
    } else if let Some(count) = match_count {
        format!(" {count} matches")
    } else {
        String::new()
    };

    // Calculate how much space to pad.
    let prompt_width = if mode == SearchMode::Path { 5 } else { 1 }; // "path/" or "/"
    let used_width: usize = prompt_width + buffer.value.len() + 1; // prompt + query + cursor
    let status_width = status_text.len();
    let available = area.width as usize;
    let pad = available.saturating_sub(used_width + status_width);

    if pad > 0 {
        spans.push(Span::raw(" ".repeat(pad)));
    }
    if !status_text.is_empty() {
        spans.push(Span::styled(status_text, Style::default().fg(Color::DarkGray)));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).style(Style::default().bg(Color::Black));
    frame.render_widget(paragraph, area);
}
