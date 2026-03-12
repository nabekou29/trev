//! Keybinding help overlay — displays all actions grouped by category.
//!
//! Inspired by lazygit's keybinding panel: two-column layout with section
//! headers, human-readable descriptions, cursor selection, inline filtering,
//! and Enter-to-execute.

use ratatui::Frame;
use ratatui::layout::{
    Alignment,
    Constraint,
    Layout,
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
    BorderType,
    Borders,
    Clear,
    Paragraph,
};

use crossterm::event::{
    KeyCode,
    KeyModifiers,
};

use crate::app::pending_keys::key_display;
use crate::input::{
    HelpBinding,
    HelpState,
};

/// Category for grouping keybindings in the help view.
#[derive(Debug, Clone, Copy)]
pub struct BindingGroup {
    /// Category display name.
    pub label: &'static str,
    /// Action name prefix used to match bindings to this group.
    pub prefix: &'static str,
}

/// Ordered list of binding categories.
///
/// Used by both the help view (rendering) and `help_group_sort_key` (sorting).
pub const GROUPS: &[BindingGroup] = &[
    BindingGroup { label: "Navigation", prefix: "tree." },
    BindingGroup { label: "File Operations", prefix: "file_op." },
    BindingGroup { label: "Preview", prefix: "preview." },
    BindingGroup { label: "Search", prefix: "search." },
    BindingGroup { label: "Filters", prefix: "filter." },
];

/// Render the keybinding help overlay.
pub fn render_help(frame: &mut Frame<'_>, area: Rect, help: &mut HelpState) {
    let dialog = centered_rect(area, 70, 80);
    frame.render_widget(Clear, dialog);

    // Build content using HelpState's filtered_bindings.
    let filtered = help.filtered_bindings();
    let key_width = compute_key_column_width(&filtered);
    let inner_width = dialog.width.saturating_sub(2) as usize;
    let (mut lines, cursor_line) =
        build_content_lines(&filtered, key_width, inner_width, help.cursor);

    let total_lines = lines.len();

    // --- Block with scroll indicator in title ---
    let title = build_title(help, total_lines);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(title)
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(dialog);
    frame.render_widget(block, dialog);

    if inner.height < 3 {
        return;
    }

    // Layout: [filter bar (optional)] [content] [footer]
    let filter_height = u16::from(help.filtering || !help.filter.value.is_empty());
    let chunks = Layout::vertical([
        Constraint::Length(filter_height),
        Constraint::Min(1),    // content
        Constraint::Length(1), // footer
    ])
    .split(inner);

    let Some(&filter_area) = chunks.first() else {
        return;
    };
    let Some(&content_area) = chunks.get(1) else {
        return;
    };
    let Some(&footer_area) = chunks.get(2) else {
        return;
    };

    // --- Filter bar ---
    if filter_height > 0 {
        render_filter_bar(frame, filter_area, help);
    }

    // --- Auto-scroll to keep cursor visible ---
    let visible_height = content_area.height as usize;
    let scroll_offset =
        cursor_line.map_or(help.scroll_offset, |cl| auto_scroll(help.scroll_offset, cl, visible_height));
    let max_offset = total_lines.saturating_sub(visible_height);
    let scroll_offset = scroll_offset.min(max_offset);

    // Persist computed offset so the next frame starts from the correct position.
    help.scroll_offset = scroll_offset;

    // Pad with empty lines so short content fills the viewport.
    while lines.len() < scroll_offset + visible_height {
        lines.push(Line::raw(""));
    }

    let visible_lines: Vec<Line<'_>> =
        lines.into_iter().skip(scroll_offset).take(visible_height).collect();

    let paragraph = Paragraph::new(visible_lines).style(Style::default().bg(Color::Black));
    frame.render_widget(paragraph, content_area);

    // --- Footer ---
    render_footer(frame, footer_area, help.filtering);
}

/// Build the title line with optional scroll indicator.
fn build_title(help: &HelpState, total_lines: usize) -> Line<'static> {
    if total_lines == 0 {
        return Line::from(" Help ");
    }
    let pos = help.scroll_offset.saturating_add(1).min(total_lines);
    Line::from(format!(" Help ({pos}/{total_lines}) "))
}

/// Compute auto-scroll offset to keep `cursor_line` visible.
const fn auto_scroll(current_offset: usize, cursor_line: usize, visible_height: usize) -> usize {
    if visible_height == 0 {
        return 0;
    }
    if cursor_line < current_offset {
        // Cursor is above the viewport.
        cursor_line
    } else if cursor_line >= current_offset + visible_height {
        // Cursor is below the viewport.
        cursor_line.saturating_sub(visible_height) + 1
    } else {
        current_offset
    }
}

/// Compute the key column width from the longest key display string.
fn compute_key_column_width(bindings: &[&HelpBinding]) -> usize {
    bindings.iter().map(|b| b.key_display.len()).max().unwrap_or(8).max(8)
}

/// Build all content lines: grouped sections with headers and binding rows.
///
/// Returns the lines and the display line index of the cursor (if any).
fn build_content_lines(
    bindings: &[&HelpBinding],
    key_width: usize,
    inner_width: usize,
    cursor: usize,
) -> (Vec<Line<'static>>, Option<usize>) {
    let mut lines: Vec<Line<'_>> = Vec::new();
    let mut cursor_line: Option<usize> = None;
    let mut binding_index: usize = 0;

    for group in GROUPS {
        let group_bindings: Vec<&&HelpBinding> = bindings
            .iter()
            .filter(|b| b.action_name.starts_with(group.prefix))
            .collect();

        if group_bindings.is_empty() {
            continue;
        }

        lines.push(build_section_header(group.label, inner_width));

        for binding in &group_bindings {
            let is_selected = binding_index == cursor;
            if is_selected {
                cursor_line = Some(lines.len());
            }
            lines.push(build_binding_line(binding, key_width, is_selected));
            binding_index += 1;
        }

        lines.push(Line::raw(""));
    }

    // General group (items that don't match any prefix).
    let general: Vec<&&HelpBinding> = bindings
        .iter()
        .filter(|b| !GROUPS.iter().any(|g| b.action_name.starts_with(g.prefix)))
        .collect();

    if !general.is_empty() {
        lines.push(build_section_header("General", inner_width));

        for binding in &general {
            let is_selected = binding_index == cursor;
            if is_selected {
                cursor_line = Some(lines.len());
            }
            lines.push(build_binding_line(binding, key_width, is_selected));
            binding_index += 1;
        }

        lines.push(Line::raw(""));
    }

    // Remove trailing empty line.
    if lines.last().is_some_and(|l| l.spans.is_empty() || l.to_string().trim().is_empty()) {
        lines.pop();
    }

    (lines, cursor_line)
}

/// Build a section header line: `" ── Label ──────────────────"`.
fn build_section_header(label: &str, inner_width: usize) -> Line<'static> {
    let prefix = "── ";
    let suffix_char = '─';
    let label_part = format!("{prefix}{label} ");
    let remaining = inner_width.saturating_sub(label_part.len() + 1);
    let rule: String = std::iter::repeat_n(suffix_char, remaining).collect();

    Line::from(vec![
        Span::styled(
            format!(" {label_part}"),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ),
        Span::styled(rule, Style::default().fg(Color::DarkGray)),
    ])
}

/// Build a single binding line with optional cursor highlight.
///
/// Unbound actions (no keybinding) are dimmed.
fn build_binding_line(binding: &HelpBinding, key_width: usize, is_selected: bool) -> Line<'static> {
    let marker = if is_selected { "▸" } else { " " };

    let (key_style, desc_style, bg_style) = if is_selected {
        (
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            Style::default().bg(Color::DarkGray),
        )
    } else if binding.has_keybinding {
        (
            Style::default().fg(Color::Cyan),
            Style::default(),
            Style::default(),
        )
    } else {
        (
            Style::default().fg(Color::DarkGray),
            Style::default().fg(Color::DarkGray),
            Style::default(),
        )
    };

    let key_text = if binding.key_display.is_empty() {
        format!("{:>width$}", "–", width = key_width)
    } else {
        format!("{:>width$}", binding.key_display, width = key_width)
    };

    Line::from(vec![
        Span::styled(format!("{marker} "), bg_style),
        Span::styled(key_text, key_style),
        Span::styled("  ", bg_style),
        Span::styled(binding.description.clone(), desc_style),
    ])
    .style(bg_style)
}

/// Render the filter input bar.
fn render_filter_bar(frame: &mut Frame<'_>, area: Rect, help: &HelpState) {
    let style = if help.filtering {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let mut spans = vec![Span::styled(" / ", style.add_modifier(Modifier::BOLD))];

    if help.filtering {
        help.filter.push_cursor_spans(&mut spans);
    } else {
        spans.push(Span::styled(&help.filter.value, style));
    }

    let line = Line::from(spans);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(Color::Black)),
        area,
    );
}

/// Render the footer with navigation hints.
fn render_footer(frame: &mut Frame<'_>, area: Rect, filtering: bool) {
    let dim = Style::default().fg(Color::DarkGray);
    let key_style = Style::default().fg(Color::Yellow);

    let enter = key_display(KeyCode::Enter, KeyModifiers::NONE);
    let esc = key_display(KeyCode::Esc, KeyModifiers::NONE);

    let spans = if filtering {
        vec![
            Span::styled(format!(" {enter}"), key_style),
            Span::styled(":apply ", dim),
            Span::styled(esc, key_style),
            Span::styled(":clear", dim),
        ]
    } else {
        vec![
            Span::styled(" /", key_style),
            Span::styled(":filter ", dim),
            Span::styled("j/k", key_style),
            Span::styled(":select ", dim),
            Span::styled(enter, key_style),
            Span::styled(":execute ", dim),
            Span::styled("q", key_style),
            Span::styled(":close", dim),
        ]
    };

    let footer = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(Color::Black))
        .alignment(Alignment::Center);
    frame.render_widget(footer, area);
}

/// Compute a centered rectangle within the given area.
fn centered_rect(area: Rect, width_pct: u16, height_pct: u16) -> Rect {
    let v_chunks = Layout::vertical([
        Constraint::Percentage((100 - height_pct) / 2),
        Constraint::Percentage(height_pct),
        Constraint::Percentage((100 - height_pct) / 2),
    ])
    .split(area);

    let h_area = v_chunks.get(1).copied().unwrap_or(area);
    let h_chunks = Layout::horizontal([
        Constraint::Percentage((100 - width_pct) / 2),
        Constraint::Percentage(width_pct),
        Constraint::Percentage((100 - width_pct) / 2),
    ])
    .split(h_area);

    h_chunks.get(1).copied().unwrap_or(area)
}
