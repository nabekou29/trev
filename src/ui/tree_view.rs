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
use crate::state::tree::ChildrenState;

/// Width reserved for the right-side metadata area (git status indicator).
const METADATA_WIDTH: usize = 2;

/// Render the tree view into the given area.
pub fn render_tree(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let offset = state.scroll.offset();
    let height = area.height as usize;
    let visible = state.tree_state.visible_nodes_in_range(offset, height);
    let cursor = state.tree_state.cursor();
    let area_width = area.width as usize;

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(height);

    for (row, vnode) in visible.iter().enumerate() {
        let global_index = offset + row;
        let is_selected = global_index == cursor;

        let indent = "  ".repeat(vnode.depth);

        // Directory expand/collapse indicator (Nerd Font caret).
        let indicator = if vnode.node.is_dir {
            if vnode.node.is_expanded {
                "\u{f0d7} " //  (expanded)
            } else {
                "\u{f0da} " //  (collapsed)
            }
        } else {
            "  "
        };

        // State indicator based on selection mode.
        let in_selection = state.selection.contains(&vnode.node.path);
        let selection_mode = state.selection.mode();

        // Build spans for the line.
        let mut spans = Vec::new();

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

        spans.push(Span::raw(indent));
        spans.push(Span::raw(indicator));

        // File icon (if enabled).
        if state.show_icons {
            if vnode.node.is_dir {
                // Nerd Font folder icons: open vs closed.
                let folder_icon = if vnode.node.is_expanded {
                    "\u{f07c}" //  folder_open
                } else {
                    "\u{f07b}" //  folder
                };
                spans.push(Span::styled(
                    format!("{folder_icon} "),
                    Style::default().fg(Color::Blue),
                ));
            } else {
                let icon = devicons::icon_for_file(&vnode.node.path, &None);
                let icon_color = parse_hex_color(icon.color);
                spans
                    .push(Span::styled(format!("{} ", icon.icon), Style::default().fg(icon_color)));
            }
        }

        // File/directory name.
        let name = &vnode.node.name;

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

        // Resolve git status (used for both filename color and right-side indicator).
        let git_status = resolve_git_status(state, &vnode.node.path, vnode.node.is_dir);

        let name_style = resolve_name_style(vnode.node.is_dir, git_status);

        spans.push(Span::styled(name.clone(), name_style));

        if !dir_suffix.is_empty() {
            spans.push(Span::styled(dir_suffix, Style::default().fg(Color::DarkGray)));
        }
        if let Some(status) = git_status {
            // Calculate current left-side content width.
            let left_width: usize = spans.iter().map(Span::width).sum();
            let total_needed = left_width + METADATA_WIDTH;
            if area_width > total_needed {
                let padding = area_width - total_needed;
                spans.push(Span::raw(" ".repeat(padding)));
            }
            spans.push(git_status_indicator(status));
        }

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

/// Resolve the display style for a filename.
///
/// Priority (highest wins): git status color → default color.
/// Future: glob pattern color will slot in between.
fn resolve_name_style(is_dir: bool, git_status: Option<GitFileStatus>) -> Style {
    let base = if is_dir {
        Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    if is_dir {
        base
    } else {
        git_status.map_or(base, |status| base.fg(status.color()))
    }
}

/// Resolve the git status for a node (file or directory).
fn resolve_git_status(
    state: &AppState,
    path: &std::path::Path,
    is_dir: bool,
) -> Option<GitFileStatus> {
    let guard = state.git_state.read().ok()?;
    let git_state = guard.as_ref()?;
    let result = if is_dir {
        git_state.dir_status(path)
    } else {
        git_state.file_status(path).copied()
    };
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
