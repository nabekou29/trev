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
use crate::input::AppMode;
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

        // State indicator based on selection mode.
        let in_selection = state.selection.contains(&vnode.node.path);
        let selection_mode = state.selection.mode();

        // Build spans for the line.
        let mut spans = Vec::new();

        if in_selection {
            match selection_mode {
                Some(SelectionMode::Mark) => {
                    spans.push(Span::styled("● ", Style::default().fg(Color::Green)));
                }
                Some(SelectionMode::Cut) => {
                    spans.push(Span::styled("◆ ", Style::default().fg(Color::Yellow)));
                }
                Some(SelectionMode::Copy) => {
                    spans.push(Span::styled("◇ ", Style::default().fg(Color::Cyan)));
                }
                None => {
                    spans.push(Span::raw("  "));
                }
            }
        } else {
            spans.push(Span::raw("  "));
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

        let name_style = if vnode.node.is_dir {
            Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        spans.push(Span::styled(name.clone(), name_style));

        if !dir_suffix.is_empty() {
            spans.push(Span::styled(dir_suffix, Style::default().fg(Color::DarkGray)));
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
    use std::path::{
        Path,
        PathBuf,
    };

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use rstest::*;

    use crate::app::{
        AppState,
        ScrollState,
    };
    use crate::file_op::selection::SelectionBuffer;
    use crate::file_op::undo::UndoHistory;
    use crate::input::AppMode;
    use crate::preview::cache::PreviewCache;
    use crate::preview::provider::PreviewRegistry;
    use crate::preview::state::PreviewState;
    use crate::state::tree::{
        ChildrenState,
        SortDirection,
        SortOrder,
        TreeNode,
        TreeState,
    };

    /// Helper: create a file node.
    fn file_node(name: &str, parent: &Path) -> TreeNode {
        TreeNode {
            name: name.to_string(),
            path: parent.join(name),
            is_dir: false,
            is_symlink: false,
            size: 100,
            modified: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
        }
    }

    /// Helper: create a directory node with children.
    fn dir_node(name: &str, parent: &Path, children: Vec<TreeNode>) -> TreeNode {
        TreeNode {
            name: name.to_string(),
            path: parent.join(name),
            is_dir: true,
            is_symlink: false,
            size: 0,
            modified: None,
            children: ChildrenState::Loaded(children),
            is_expanded: false,
        }
    }

    /// Helper: create a minimal `AppState` from a `TreeState`.
    fn app_state_from_tree(tree_state: TreeState) -> AppState {
        AppState {
            tree_state,
            preview_state: PreviewState::new(),
            preview_cache: PreviewCache::new(10),
            preview_registry: PreviewRegistry::new(vec![]).unwrap(),
            mode: AppMode::Normal,
            selection: SelectionBuffer::new(),
            undo_history: UndoHistory::new(100),
            watcher: None,
            should_quit: false,
            show_icons: true,
            show_preview: false,
            show_hidden: false,
            show_ignored: false,
            viewport_height: 50,
            scroll: ScrollState::new(),
            status_message: None,
            processing: false,
        }
    }

    /// Helper: create a `TreeState` with root expanded.
    fn tree_with_children(children: Vec<TreeNode>) -> TreeState {
        let root = TreeNode {
            name: "root".to_string(),
            path: PathBuf::from("/test/root"),
            is_dir: true,
            is_symlink: false,
            size: 0,
            modified: None,
            children: ChildrenState::Loaded(children),
            is_expanded: true,
        };
        TreeState::new(root, SortOrder::Name, SortDirection::Asc, true)
    }

    #[rstest]
    #[ignore = "Performance test — run with `cargo test -- --ignored`"]
    fn perf_render_tree_100k_flat() {
        let root_path = Path::new("/test/root");
        let children: Vec<TreeNode> =
            (0..100_000).map(|i| file_node(&format!("file{i:06}.txt"), root_path)).collect();
        let state = app_state_from_tree(tree_with_children(children));

        let mut terminal = Terminal::new(TestBackend::new(120, 50)).unwrap();

        // Warmup.
        terminal
            .draw(|frame| {
                super::render_tree(frame, frame.area(), &state);
            })
            .unwrap();

        let start = std::time::Instant::now();
        terminal
            .draw(|frame| {
                super::render_tree(frame, frame.area(), &state);
            })
            .unwrap();
        let elapsed = start.elapsed();

        eprintln!("render_tree 100k flat (viewport 50): {elapsed:?}");
    }

    #[rstest]
    #[ignore = "Performance test — run with `cargo test -- --ignored`"]
    fn perf_render_tree_100k_nested() {
        let root_path = Path::new("/test/root");
        let children: Vec<TreeNode> = (0..100)
            .map(|d| {
                let dir_path = root_path.join(format!("dir{d:03}"));
                let files: Vec<TreeNode> = (0..1000)
                    .map(|f| file_node(&format!("file{f:04}.txt"), &dir_path))
                    .collect();
                let mut d = dir_node(&format!("dir{d:03}"), root_path, files);
                d.is_expanded = true;
                d
            })
            .collect();
        let state = app_state_from_tree(tree_with_children(children));

        let mut terminal = Terminal::new(TestBackend::new(120, 50)).unwrap();

        // Warmup.
        terminal
            .draw(|frame| {
                super::render_tree(frame, frame.area(), &state);
            })
            .unwrap();

        let start = std::time::Instant::now();
        terminal
            .draw(|frame| {
                super::render_tree(frame, frame.area(), &state);
            })
            .unwrap();
        let elapsed = start.elapsed();

        eprintln!("render_tree 100k nested (viewport 50): {elapsed:?}");
    }

    #[rstest]
    #[ignore = "Performance test — run with `cargo test -- --ignored`"]
    fn perf_render_tree_100k_scrolled_to_middle() {
        let root_path = Path::new("/test/root");
        let children: Vec<TreeNode> =
            (0..100_000).map(|i| file_node(&format!("file{i:06}.txt"), root_path)).collect();
        let mut state = app_state_from_tree(tree_with_children(children));

        // Scroll to middle to test non-zero offset.
        state.scroll.clamp_to_cursor(50_000, 50);
        state.tree_state.move_cursor_to(50_000);

        let mut terminal = Terminal::new(TestBackend::new(120, 50)).unwrap();

        // Warmup.
        terminal
            .draw(|frame| {
                super::render_tree(frame, frame.area(), &state);
            })
            .unwrap();

        let start = std::time::Instant::now();
        terminal
            .draw(|frame| {
                super::render_tree(frame, frame.area(), &state);
            })
            .unwrap();
        let elapsed = start.elapsed();

        eprintln!("render_tree 100k scrolled to middle (viewport 50): {elapsed:?}");
    }

    #[rstest]
    #[ignore = "Performance test — run with `cargo test -- --ignored`"]
    fn perf_full_frame_render_100k() {
        let root_path = Path::new("/test/root");
        let children: Vec<TreeNode> =
            (0..100_000).map(|i| file_node(&format!("file{i:06}.txt"), root_path)).collect();
        let mut state = app_state_from_tree(tree_with_children(children));

        let mut terminal = Terminal::new(TestBackend::new(120, 50)).unwrap();

        // Warmup.
        terminal
            .draw(|frame| {
                crate::ui::render(frame, &mut state);
            })
            .unwrap();

        let start = std::time::Instant::now();
        terminal
            .draw(|frame| {
                crate::ui::render(frame, &mut state);
            })
            .unwrap();
        let elapsed = start.elapsed();

        eprintln!("full render 100k (120x50): {elapsed:?}");
    }
}
