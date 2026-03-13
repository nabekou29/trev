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
    Wrap,
};
use ratatui_image::StatefulImage;

use crate::preview::content::PreviewContent;
use crate::preview::state::PreviewState;
use crate::ui::column::truncate_to_width;

/// Render the preview panel into the given area.
///
/// Dispatches to variant-specific rendering based on the current
/// `PreviewContent` in the `PreviewState`.
///
/// The preview block uses full borders on all sides.
pub fn render_preview(frame: &mut Frame<'_>, area: Rect, state: &mut PreviewState) {
    let provider_title = build_provider_title(state);
    let provider_width = provider_title.as_ref().map_or(0, Line::width);
    let top_title = build_top_title(state, area.width as usize, provider_width);
    let lang_title = build_language_title(state);

    let mut block = Block::default().borders(Borders::ALL).title_top(top_title);
    if let Some(providers) = provider_title {
        block = block.title_top(providers.right_aligned());
    }
    if let Some(lang) = lang_title {
        block = block.title_bottom(lang.right_aligned());
    }

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

    let word_wrap = state.word_wrap;

    match &mut state.content {
        PreviewContent::HighlightedText { lines, truncated, .. } => {
            render_highlighted_text(
                frame,
                inner,
                lines,
                state.scroll_row,
                state.scroll_col,
                *truncated,
                word_wrap,
            );
        }
        PreviewContent::PlainText { lines, truncated } => {
            render_plain_text(
                frame,
                inner,
                lines,
                state.scroll_row,
                state.scroll_col,
                *truncated,
                word_wrap,
            );
        }
        PreviewContent::AnsiText { text } => {
            let paragraph = Paragraph::new(text.clone());
            let paragraph =
                apply_scroll_or_wrap(paragraph, state.scroll_row, state.scroll_col, word_wrap);
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

/// Build the top-left title: file name, truncated with `…` if needed.
fn build_top_title(
    state: &PreviewState,
    total_width: usize,
    provider_width: usize,
) -> Line<'static> {
    let file_name = state
        .current_path
        .as_ref()
        .and_then(|p| p.file_name())
        .map_or_else(|| "Preview".to_string(), |n| n.to_string_lossy().into_owned());

    // Available space: total - 2 (border corners) - provider_width
    // The padding spaces ` name ` are part of the rendered width, so subtract them too.
    let max_name_len = total_width.saturating_sub(provider_width + 2 + 2);
    let display_name = truncate_to_width(&file_name, max_name_len);

    Line::from(Span::styled(
        format!(" {display_name} "),
        Style::default().add_modifier(Modifier::BOLD),
    ))
}

/// Build the top-right provider indicator title.
///
/// Only shown when multiple providers are available.
/// Active: `● Name` (cyan+bold), inactive: `○ Name` (dim).
fn build_provider_title(state: &PreviewState) -> Option<Line<'static>> {
    if state.available_providers.len() <= 1 {
        return None;
    }
    let mut spans = Vec::new();
    for (i, name) in state.available_providers.iter().enumerate() {
        let is_active = i == state.active_provider_index;
        let (dot_label, style) = if is_active {
            (" ● ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        } else {
            (" ○ ", Style::default().fg(Color::DarkGray))
        };
        spans.push(Span::styled(dot_label, style));
        spans.push(Span::styled(name.clone(), style));
    }
    spans.push(Span::raw(" "));
    Some(Line::from(spans))
}

/// Build the right-aligned language label for syntax-highlighted content.
fn build_language_title(state: &PreviewState) -> Option<Line<'static>> {
    if let PreviewContent::HighlightedText { language, .. } = &state.content {
        Some(Line::from(Span::styled(
            format!("{language} "),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
        )))
    } else {
        None
    }
}

/// Apply word wrap or horizontal scroll to a paragraph.
fn apply_scroll_or_wrap(
    paragraph: Paragraph<'_>,
    scroll_row: usize,
    scroll_col: usize,
    word_wrap: bool,
) -> Paragraph<'_> {
    if word_wrap {
        paragraph
            .wrap(Wrap { trim: false })
            .scroll((u16::try_from(scroll_row).unwrap_or(u16::MAX), 0))
    } else {
        paragraph.scroll((
            u16::try_from(scroll_row).unwrap_or(u16::MAX),
            u16::try_from(scroll_col).unwrap_or(u16::MAX),
        ))
    }
}

/// Render syntax-highlighted text lines with scroll offset.
fn render_highlighted_text(
    frame: &mut Frame<'_>,
    area: Rect,
    lines: &[Line<'static>],
    scroll_row: usize,
    scroll_col: usize,
    truncated: bool,
    word_wrap: bool,
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

    let paragraph = Paragraph::new(visible);
    let paragraph = if word_wrap {
        paragraph.wrap(Wrap { trim: false })
    } else {
        paragraph.scroll((0, u16::try_from(scroll_col).unwrap_or(u16::MAX)))
    };
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
    word_wrap: bool,
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

    let paragraph = Paragraph::new(visible);
    let paragraph = if word_wrap {
        paragraph.wrap(Wrap { trim: false })
    } else {
        paragraph.scroll((0, u16::try_from(scroll_col).unwrap_or(u16::MAX)))
    };
    frame.render_widget(paragraph, area);
}

/// Render centered text in the given area.
fn render_centered(frame: &mut Frame<'_>, area: Rect, text: &str, style: Style) {
    let paragraph =
        Paragraph::new(Line::styled(text.to_string(), style)).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}
