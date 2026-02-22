//! Tree view widget — renders the file tree with indentation, icons, and cursor highlight.

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
use ratatui::widgets::Paragraph;

use crate::app::AppState;
use crate::file_op::selection::SelectionMode;
use crate::git::GitFileStatus;
use crate::input::AppMode;
use crate::state::tree::{
    ChildrenState,
    VisibleNode,
};
use crate::ui::column::{
    ColumnKind,
    MtimeMode,
    ResolvedColumn,
    format_mtime,
    format_size,
    mtime_color,
    total_columns_width,
    truncate_to_width,
};

/// Render the tree view into the given area.
pub fn render_tree(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let offset = state.scroll.offset();
    let height = area.height as usize;
    let visible = state.tree_state.visible_nodes_in_range(offset, height);
    let cursor = state.tree_state.cursor();
    let area_width = area.width as usize;
    let columns_width = total_columns_width(&state.columns);

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(height);

    for (row, vnode) in visible.iter().enumerate() {
        let global_index = offset + row;
        let is_selected = global_index == cursor;

        let git_status = resolve_git_status(state, &vnode.node.path, vnode.node.is_dir);
        let spans = build_row_spans(vnode, state, area_width, columns_width, git_status);
        let line = Line::from(spans);

        if is_selected {
            lines.push(
                line.style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)),
            );
        } else {
            lines.push(line);
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);

    // Render inline input overlay when in Input mode.
    if let AppMode::Input(ref input) = state.mode {
        render_input_overlay(frame, area, cursor, offset, height, input);
    }
}

/// Build all spans for a single tree row.
///
/// Layout: `[selection][indent][caret][icon][name...][pad][col1][col2][col3]`
fn build_row_spans<'a>(
    vnode: &VisibleNode<'_>,
    state: &AppState,
    area_width: usize,
    columns_width: usize,
    git_status: Option<GitFileStatus>,
) -> Vec<Span<'a>> {
    let mut spans = Vec::new();

    // Selection indicator.
    let in_selection = state.selection.contains(&vnode.node.path);
    let selection_mode = state.selection.mode();
    let selection_indicator = if in_selection {
        match selection_mode {
            Some(SelectionMode::Mark) => Some(("● ", Color::Green)),
            Some(SelectionMode::Cut) => Some(("◆ ", Color::Yellow)),
            Some(SelectionMode::Copy) => Some(("◇ ", Color::Cyan)),
            None => None,
        }
    } else {
        None
    };
    match selection_indicator {
        Some((marker, color)) => spans.push(Span::styled(marker, Style::default().fg(color))),
        None => spans.push(Span::raw("  ")),
    }

    // Indent + directory caret.
    let indent = "  ".repeat(vnode.depth);
    let indicator = if vnode.node.is_dir {
        if vnode.node.is_expanded { "\u{f0d7} " } else { "\u{f0da} " }
    } else {
        "  "
    };
    spans.push(Span::raw(indent));
    spans.push(Span::raw(indicator));

    // File icon (if enabled).
    if state.show_icons {
        push_icon_span(&mut spans, vnode);
    }

    // Name area (truncated to fit) + metadata columns.
    let left_prefix_width: usize = spans.iter().map(Span::width).sum();
    push_name_and_columns(
        &mut spans,
        vnode,
        &state.columns,
        git_status,
        area_width,
        left_prefix_width,
        columns_width,
    );

    spans
}

/// Push the file/directory icon span.
fn push_icon_span(spans: &mut Vec<Span<'_>>, vnode: &VisibleNode<'_>) {
    if vnode.node.is_dir {
        let folder_icon = if vnode.node.is_expanded { "\u{f07c}" } else { "\u{f07b}" };
        spans.push(Span::styled(format!("{folder_icon} "), Style::default().fg(Color::Blue)));
    } else {
        let icon = devicons::icon_for_file(&vnode.node.path, &None);
        let icon_color = parse_hex_color(icon.color);
        spans.push(Span::styled(format!("{} ", icon.icon), Style::default().fg(icon_color)));
    }
}

/// Push the name text (truncated) followed by padding and metadata column spans.
fn push_name_and_columns(
    spans: &mut Vec<Span<'_>>,
    vnode: &VisibleNode<'_>,
    columns: &[ResolvedColumn],
    git_status: Option<GitFileStatus>,
    area_width: usize,
    left_prefix_width: usize,
    columns_width: usize,
) {
    let name = &vnode.node.name;

    // Symlink indicator: " → target".
    let symlink_suffix =
        vnode.node.symlink_target.as_ref().map(|target| format!(" \u{2192} {target}"));

    // Directory status indicator (loading or empty).
    let dir_suffix = if vnode.node.is_dir && vnode.node.is_expanded {
        match &vnode.node.children {
            ChildrenState::Loading => " [Loading...]",
            ChildrenState::Loaded(children) if children.is_empty() => " (empty)",
            _ => "",
        }
    } else {
        ""
    };

    let full_name = format!("{name}{}{dir_suffix}", symlink_suffix.as_deref().unwrap_or(""));

    let name_width = area_width.saturating_sub(left_prefix_width + columns_width);
    let truncated_name = truncate_to_width(&full_name, name_width);
    let name_style = resolve_name_style(vnode.node.is_dir, git_status);

    // Split truncated name into styled segments: filename vs suffix.
    let name_display_len = unicode_width::UnicodeWidthStr::width(name.as_str());
    let truncated_width = unicode_width::UnicodeWidthStr::width(truncated_name.as_str());

    if truncated_width <= name_display_len {
        spans.push(Span::styled(truncated_name, name_style));
    } else {
        spans.push(Span::styled(name.clone(), name_style));
        let remaining = &truncated_name[name.len()..];
        if !remaining.is_empty() {
            spans.push(Span::styled(remaining.to_string(), Style::default().fg(Color::DarkGray)));
        }
    }

    // Pad name area to its fixed width.
    if truncated_width < name_width {
        spans.push(Span::raw(" ".repeat(name_width - truncated_width)));
    }

    // Render metadata columns.
    for col in columns {
        spans.push(Span::raw(" "));
        match col.kind {
            ColumnKind::Size => {
                let text = format_size(vnode.node.size, vnode.node.is_dir);
                spans.push(Span::styled(text, Style::default().fg(Color::DarkGray)));
            }
            ColumnKind::ModifiedAt => {
                let mtime = if vnode.node.is_dir && col.mtime_mode == MtimeMode::RecursiveMax {
                    vnode.node.recursive_max_mtime
                } else {
                    vnode.node.modified
                };
                let text = format_mtime(mtime, col.mtime_format);
                let color = mtime_color(mtime);
                spans.push(Span::styled(text, Style::default().fg(color)));
            }
            ColumnKind::GitStatus => {
                if let Some(status) = git_status {
                    spans.push(git_status_indicator(status));
                } else {
                    spans.push(Span::raw("  "));
                }
            }
        }
    }
}

/// Resolve the display style for a filename.
///
/// Priority (highest wins): git status color → default color.
fn resolve_name_style(is_dir: bool, git_status: Option<GitFileStatus>) -> Style {
    let base = if is_dir {
        Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    if is_dir { base } else { git_status.map_or(base, |status| base.fg(status.color())) }
}

/// Resolve the git status for a node (file or directory).
fn resolve_git_status(
    state: &AppState,
    path: &std::path::Path,
    is_dir: bool,
) -> Option<GitFileStatus> {
    let guard = state.git_state.read().ok()?;
    let git_state = guard.as_ref()?;
    let result =
        if is_dir { git_state.dir_status(path) } else { git_state.file_status(path).copied() };
    drop(guard);
    result
}

/// Create a styled `Span` for a git file status indicator.
fn git_status_indicator(status: GitFileStatus) -> Span<'static> {
    let ch = status.char();
    let color = status.color();
    let mut style = Style::default().fg(color);
    if matches!(status, GitFileStatus::Conflicted) {
        style = style.add_modifier(Modifier::BOLD);
    }
    Span::styled(format!("{ch} "), style)
}

/// Render the inline input overlay below the cursor row.
///
/// The box is positioned below the cursor so the target entry stays visible.
/// Height is 3 lines: top border + input + bottom border.
fn render_input_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    cursor: usize,
    offset: usize,
    height: usize,
    input: &crate::input::InputState,
) {
    let cursor_row = cursor.saturating_sub(offset);
    // Place input box below the cursor row.
    let input_start = cursor_row + 1;
    let box_height: u16 = 3;

    if input_start < height {
        let start_u16 = u16::try_from(input_start).unwrap_or(u16::MAX);
        let available = area.height.saturating_sub(start_u16);

        if available >= box_height {
            let input_area = Rect {
                x: area.x,
                y: area.y.saturating_add(start_u16),
                width: area.width,
                height: box_height,
            };
            crate::ui::inline_input::render_inline_input(frame, input_area, input);
        }
    }
}

/// Parse a hex color string (e.g., "#D32F2F") into a ratatui `Color`.
///
/// Falls back to `Color::White` if the string cannot be parsed.
fn parse_hex_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Color::White;
    }

    let Ok(r) = u8::from_str_radix(hex.get(..2).unwrap_or("ff"), 16) else {
        return Color::White;
    };
    let Ok(g) = u8::from_str_radix(hex.get(2..4).unwrap_or("ff"), 16) else {
        return Color::White;
    };
    let Ok(b) = u8::from_str_radix(hex.get(4..6).unwrap_or("ff"), 16) else {
        return Color::White;
    };

    Color::Rgb(r, g, b)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    // --- T012: git_status_indicator ---

    #[rstest]
    #[case(GitFileStatus::Modified, 'M', Color::Yellow)]
    #[case(GitFileStatus::Staged, 'M', Color::Green)]
    #[case(GitFileStatus::Added, 'A', Color::Green)]
    #[case(GitFileStatus::Deleted, 'D', Color::Red)]
    #[case(GitFileStatus::Renamed, 'R', Color::Blue)]
    #[case(GitFileStatus::Untracked, '?', Color::Magenta)]
    #[case(GitFileStatus::Conflicted, '!', Color::Red)]
    fn git_status_indicator_returns_correct_char_and_color(
        #[case] status: GitFileStatus,
        #[case] expected_char: char,
        #[case] expected_color: Color,
    ) {
        let span = git_status_indicator(status);
        let content = span.content.to_string();
        assert_that!(content.trim().len(), eq(1));
        assert!(content.starts_with(expected_char));
        assert_that!(span.style.fg, some(eq(expected_color)));
    }

    #[rstest]
    fn git_status_indicator_conflicted_is_bold() {
        let span = git_status_indicator(GitFileStatus::Conflicted);
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
    }
}
