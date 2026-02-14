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
use crate::state::tree::ChildrenState;

/// Render the tree view into the given area.
pub fn render_tree(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let visible = state.tree_state.visible_nodes();
    let offset = state.scroll.offset();
    let height = area.height as usize;
    let cursor = state.tree_state.cursor();

    let mut lines: Vec<Line<'_>> = Vec::with_capacity(height);

    for (row, vnode) in visible.iter().skip(offset).take(height).enumerate() {
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

        // Build spans for the line.
        let mut spans = Vec::new();
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
                spans.push(Span::styled(
                    format!("{} ", icon.icon),
                    Style::default().fg(icon_color),
                ));
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

        let name_style = if vnode.node.is_dir {
            Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        spans.push(Span::styled(name.clone(), name_style));

        if !dir_suffix.is_empty() {
            spans.push(Span::styled(
                dir_suffix,
                Style::default().fg(Color::DarkGray),
            ));
        }

        let line = Line::from(spans);

        if is_selected {
            lines.push(
                line.style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                ),
            );
        } else {
            lines.push(line);
        }
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
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
