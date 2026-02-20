//! Preview panel widget — renders file preview content with scroll support.

use ratatui::Frame;
use ratatui::layout::{
    Alignment,
    Rect,
};
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
    Borders,
    Paragraph,
};
use ratatui_image::StatefulImage;

use crate::preview::content::PreviewContent;
use crate::preview::state::PreviewState;

/// Render the preview panel into the given area.
///
/// Dispatches to variant-specific rendering based on the current
/// `PreviewContent` in the `PreviewState`.
///
/// When `is_narrow` is true, the preview is below the tree and uses a top border;
/// otherwise it is beside the tree and uses a left border.
pub fn render_preview(frame: &mut Frame<'_>, area: Rect, state: &mut PreviewState, is_narrow: bool) {
    let title = build_title(state);

    let border = if is_narrow { Borders::TOP } else { Borders::LEFT };
    let block = Block::default().borders(border).title(title).title_alignment(Alignment::Left);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let content_type = state.content.type_name();
    let preview_path = state
        .current_path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let _span = tracing::info_span!("render_content", content_type, preview_path).entered();

    match &mut state.content {
        PreviewContent::HighlightedText { lines, truncated, .. } => {
            render_highlighted_text(
                frame,
                inner,
                lines,
                state.scroll_row,
                state.scroll_col,
                *truncated,
            );
        }
        PreviewContent::PlainText { lines, truncated } => {
            render_plain_text(frame, inner, lines, state.scroll_row, state.scroll_col, *truncated);
        }
        PreviewContent::AnsiText { text } => {
            let paragraph = Paragraph::new(text.clone()).scroll((
                u16::try_from(state.scroll_row).unwrap_or(u16::MAX),
                u16::try_from(state.scroll_col).unwrap_or(u16::MAX),
            ));
            frame.render_widget(paragraph, inner);
        }
        PreviewContent::Loading => {
            render_centered(frame, inner, "Loading...", Style::default().fg(Color::DarkGray));
        }
        PreviewContent::Error { message } => {
            render_centered(frame, inner, message, Style::default().fg(Color::Red));
        }
        PreviewContent::Empty => {
            render_centered(frame, inner, "(empty)", Style::default().fg(Color::DarkGray));
        }
        PreviewContent::Binary { size } => {
            let text = format!("Binary file ({size} bytes)");
            render_centered(frame, inner, &text, Style::default().fg(Color::DarkGray));
        }
        PreviewContent::Directory { entry_count, total_size } => {
            let text = format!("Directory: {entry_count} entries, {total_size} bytes");
            render_centered(frame, inner, &text, Style::default().fg(Color::DarkGray));
        }
        PreviewContent::Image { protocol } => {
            let widget = StatefulImage::default();
            frame.render_stateful_widget(widget, inner, &mut **protocol);
        }
    }
}

/// Build the title line for the preview block.
fn build_title(state: &PreviewState) -> Line<'static> {
    let mut spans = vec![Span::raw(" Preview")];

    if let Some(provider_name) = state.active_provider_name() {
        spans.push(Span::styled(format!(" [{provider_name}]"), Style::default().fg(Color::Cyan)));
    }

    if state.available_providers.len() > 1 {
        spans.push(Span::styled(
            format!(" ({}/{})", state.active_provider_index + 1, state.available_providers.len()),
            Style::default().fg(Color::DarkGray),
        ));
    }

    // Show language for highlighted text.
    if let PreviewContent::HighlightedText { language, .. } = &state.content {
        spans.push(Span::styled(
            format!(" {language}"),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
        ));
    }

    Line::from(spans)
}

/// Render syntax-highlighted text lines with scroll offset.
fn render_highlighted_text(
    frame: &mut Frame<'_>,
    area: Rect,
    lines: &[Line<'static>],
    scroll_row: usize,
    scroll_col: usize,
    truncated: bool,
) {
    let height = area.height as usize;
    let end = (scroll_row + height).min(lines.len());

    let mut visible: Vec<Line<'_>> = lines.get(scroll_row..end).unwrap_or_default().to_vec();

    if truncated && end >= lines.len() {
        visible.push(Line::styled(
            "--- truncated ---",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
        ));
    }

    let paragraph =
        Paragraph::new(visible).scroll((0, u16::try_from(scroll_col).unwrap_or(u16::MAX)));
    frame.render_widget(paragraph, area);
}

/// Render plain text lines with scroll offset.
fn render_plain_text(
    frame: &mut Frame<'_>,
    area: Rect,
    lines: &[String],
    scroll_row: usize,
    scroll_col: usize,
    truncated: bool,
) {
    let height = area.height as usize;
    let end = (scroll_row + height).min(lines.len());

    let mut visible: Vec<Line<'_>> = lines
        .get(scroll_row..end)
        .unwrap_or_default()
        .iter()
        .map(|s| Line::raw(s.as_str()))
        .collect();

    if truncated && end >= lines.len() {
        visible.push(Line::styled(
            "--- truncated ---",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
        ));
    }

    let paragraph =
        Paragraph::new(visible).scroll((0, u16::try_from(scroll_col).unwrap_or(u16::MAX)));
    frame.render_widget(paragraph, area);
}

/// Render centered text in the given area.
fn render_centered(frame: &mut Frame<'_>, area: Rect, text: &str, style: Style) {
    let paragraph =
        Paragraph::new(Line::styled(text.to_string(), style)).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}
