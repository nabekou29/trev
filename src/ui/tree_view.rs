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
use crate::input::{
    AppMode,
    SearchMode,
};
use crate::state::tree::{
    ChildrenState,
    VisibleNode,
};
use crate::ui::column::{
    ColumnKind,
    MtimeMode,
    format_mtime,
    format_size,
    mtime_color,
    total_columns_width,
    truncate_to_width,
};

/// Render the tree view into the given area.
///
/// `visible_count` is the pre-computed total visible node count for this frame,
/// avoiding redundant full tree walks.
pub fn render_tree(frame: &mut Frame<'_>, area: Rect, state: &mut AppState, visible_count: usize) {
    // When in Search(Typing) mode, reserve the bottom row for the search input bar.
    let (tree_area, search_bar_area) = if let AppMode::Search(ref search) = state.mode {
        if search.phase == crate::input::SearchPhase::Typing && area.height > 1 {
            let tree = Rect { height: area.height - 1, ..area };
            let bar = Rect { y: area.y.saturating_add(area.height - 1), height: 1, ..area };
            (tree, Some(bar))
        } else {
            (area, None)
        }
    } else {
        (area, None)
    };

    let offset = state.scroll.offset();
    let height = tree_area.height as usize;
    let visible = state.tree_state.visible_nodes_in_range(offset, height);
    let cursor = state.tree_state.cursor();
    let area_width = tree_area.width as usize;
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
    frame.render_widget(paragraph, tree_area);

    // Render inline input overlay when in Input mode.
    if let AppMode::Input(ref mut input) = state.mode {
        render_input_overlay(frame, tree_area, cursor, offset, height, input);
    }

    // Render search input bar.
    if let (Some(bar_area), AppMode::Search(search)) = (search_bar_area, &state.mode) {
        let index_complete = true; // TODO: check ctx.search_index.is_complete()
        let match_count = Some(visible_count);
        crate::ui::search_input::render_search_input(
            frame,
            bar_area,
            &search.buffer,
            search.mode,
            match_count,
            !index_complete,
        );
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
        state,
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
        let icon_color = crate::ui::file_style::parse_hex_color(icon.color);
        spans.push(Span::styled(format!("{} ", icon.icon), Style::default().fg(icon_color)));
    }
}

/// Push the name text (truncated) followed by padding and metadata column spans.
fn push_name_and_columns(
    spans: &mut Vec<Span<'_>>,
    vnode: &VisibleNode<'_>,
    state: &AppState,
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
            ChildrenState::Loaded(children)
                if state
                    .tree_state
                    .search_filter_paths()
                    .is_some_and(|filter| !children.iter().any(|c| filter.contains(&c.path))) =>
            {
                " (no results)"
            }
            _ => "",
        }
    } else {
        ""
    };

    let full_name = format!("{name}{}{dir_suffix}", symlink_suffix.as_deref().unwrap_or(""));

    let name_width = area_width.saturating_sub(left_prefix_width + columns_width);
    let name_style = state.file_style_matcher.resolve_style(
        name,
        crate::ui::file_style::NodeStyleFlags {
            is_dir: vnode.node.is_dir,
            is_symlink: vnode.node.is_symlink,
            is_ignored: vnode.node.is_ignored,
            is_orphan: vnode.node.is_orphan,
        },
        git_status,
    );

    // Render the name with optional search match highlighting.
    let truncated_width =
        push_name_with_highlight(spans, name, &full_name, name_width, name_style, vnode, state);

    // Pad name area to its fixed width.
    if truncated_width < name_width {
        spans.push(Span::raw(" ".repeat(name_width - truncated_width)));
    }

    // Render metadata columns.
    push_metadata_columns(spans, vnode, state, git_status);
}

/// Render the name text with optional search match highlighting.
///
/// Returns the display width of the rendered name (used for padding).
fn push_name_with_highlight(
    spans: &mut Vec<Span<'_>>,
    name: &str,
    full_name: &str,
    name_width: usize,
    name_style: Style,
    vnode: &VisibleNode<'_>,
    state: &AppState,
) -> usize {
    let name_display_len = unicode_width::UnicodeWidthStr::width(name);
    let full_name_width = unicode_width::UnicodeWidthStr::width(full_name);

    // Check for search match highlight indices.
    let search_indices = state.search_match_indices.get(&vnode.node.path).filter(|v| !v.is_empty());

    if let Some(raw_indices) = search_indices {
        let name_indices = adjust_match_indices_for_name(
            raw_indices,
            name,
            &vnode.node.path,
            &state.mode,
            state.tree_state.root_path(),
        );

        if full_name_width > name_width {
            push_highlighted_name(spans, name, Some(name_width), &name_indices, name_style);
        } else {
            push_highlighted_name(spans, name, None, &name_indices, name_style);
            if let Some(suffix) = full_name.get(name.len()..)
                && !suffix.is_empty()
            {
                spans.push(Span::styled(suffix.to_string(), Style::default().fg(Color::DarkGray)));
            }
        }
    } else {
        let truncated_name = truncate_to_width(full_name, name_width);
        let truncated_width = unicode_width::UnicodeWidthStr::width(truncated_name.as_str());

        if truncated_width <= name_display_len {
            spans.push(Span::styled(truncated_name, name_style));
        } else {
            spans.push(Span::styled(name.to_string(), name_style));
            if let Some(remaining) = truncated_name.get(name.len()..)
                && !remaining.is_empty()
            {
                spans.push(Span::styled(
                    remaining.to_string(),
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }

        return truncated_width;
    }

    // For highlighted path, compute rendered width.
    full_name_width.min(name_width)
}

/// Push metadata column spans (size, mtime, git status).
fn push_metadata_columns(
    spans: &mut Vec<Span<'_>>,
    vnode: &VisibleNode<'_>,
    state: &AppState,
    git_status: Option<GitFileStatus>,
) {
    for col in &state.columns {
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

/// Adjust search match indices from the matched haystack to the displayed filename.
///
/// In `Name` mode, indices already reference the filename and are used as-is.
/// In `Path` mode, indices reference the relative path; this function remaps
/// them to character positions within the filename portion.
fn adjust_match_indices_for_name(
    indices: &[u32],
    name: &str,
    path: &std::path::Path,
    mode: &AppMode,
    root_path: &std::path::Path,
) -> Vec<u32> {
    let search_mode = match mode {
        AppMode::Search(s) => s.mode,
        _ => return indices.to_vec(),
    };

    match search_mode {
        SearchMode::Name => indices.to_vec(),
        SearchMode::Path => {
            let Ok(rel) = path.strip_prefix(root_path) else {
                return Vec::new();
            };
            let rel_str = rel.to_string_lossy();
            let rel_char_count = rel_str.chars().count();
            let name_char_count = name.chars().count();
            let offset = rel_char_count.saturating_sub(name_char_count);

            #[expect(
                clippy::cast_possible_truncation,
                reason = "char index fits in u32 since name length is bounded"
            )]
            indices
                .iter()
                .filter_map(|&i| {
                    (i as usize)
                        .checked_sub(offset)
                        .filter(|&adj| adj < name_char_count)
                        .map(|adj| adj as u32)
                })
                .collect()
        }
    }
}

/// Push name spans with character-level match highlighting and optional truncation.
///
/// Characters at positions listed in `match_indices` are rendered with an
/// underline modifier added to `base_style`. When `max_width` is `Some`,
/// truncates the name and appends "…".
fn push_highlighted_name(
    spans: &mut Vec<Span<'_>>,
    name: &str,
    max_width: Option<usize>,
    match_indices: &[u32],
    base_style: Style,
) {
    use unicode_width::UnicodeWidthChar;

    let highlight_style = base_style.add_modifier(Modifier::UNDERLINED);

    // If truncating, reserve 1 column for "…".
    let target_width = max_width.map(|w| w.saturating_sub(1));
    let mut current_width = 0usize;
    let mut buf = String::new();
    let mut prev_highlighted: Option<bool> = None;
    let mut truncated = false;

    for (i, ch) in name.chars().enumerate() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if let Some(tw) = target_width
            && current_width + ch_width > tw
        {
            truncated = true;
            break;
        }

        #[expect(
            clippy::cast_possible_truncation,
            reason = "char index in a filename always fits in u32"
        )]
        let highlighted = match_indices.binary_search(&(i as u32)).is_ok();
        if let Some(prev) = prev_highlighted
            && highlighted != prev
            && !buf.is_empty()
        {
            let style = if prev { highlight_style } else { base_style };
            spans.push(Span::styled(std::mem::take(&mut buf), style));
        }
        prev_highlighted = Some(highlighted);
        buf.push(ch);
        current_width += ch_width;
    }

    if !buf.is_empty() {
        let style = match prev_highlighted {
            Some(true) => highlight_style,
            _ => base_style,
        };
        spans.push(Span::styled(buf, style));
    }

    if truncated {
        spans.push(Span::styled("…".to_string(), base_style));
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
    input: &mut crate::input::InputState,
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    // --- git_status_indicator ---

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

    // --- Helper ---

    /// Create a `SearchState` with the given mode for testing.
    fn make_search_state(mode: SearchMode) -> crate::input::SearchState {
        crate::input::SearchState {
            buffer: crate::input::TextBuffer::new(),
            phase: crate::input::SearchPhase::Typing,
            mode,
            history_index: None,
            original_query: String::new(),
        }
    }

    // --- adjust_match_indices_for_name ---

    #[rstest]
    fn adjust_indices_name_mode_returns_as_is() {
        let mode = AppMode::Search(make_search_state(SearchMode::Name));
        let indices = vec![0, 1, 2];
        let result = adjust_match_indices_for_name(
            &indices,
            "foo",
            std::path::Path::new("/project/src/foo"),
            &mode,
            std::path::Path::new("/project"),
        );
        assert_eq!(result, vec![0u32, 1, 2]);
    }

    #[rstest]
    fn adjust_indices_path_mode_simple_remap() {
        let mode = AppMode::Search(make_search_state(SearchMode::Path));
        // path "src/foo.rs", root "/project", name "foo.rs"
        // rel = "src/foo.rs" (10 chars), name = "foo.rs" (6 chars), offset = 4
        let indices = vec![4, 5, 6];
        let result = adjust_match_indices_for_name(
            &indices,
            "foo.rs",
            std::path::Path::new("/project/src/foo.rs"),
            &mode,
            std::path::Path::new("/project"),
        );
        assert_eq!(result, vec![0u32, 1, 2]);
    }

    #[rstest]
    fn adjust_indices_path_mode_directory_prefix_filtered_out() {
        let mode = AppMode::Search(make_search_state(SearchMode::Path));
        // rel = "src/foo.rs", offset = 4
        // indices [0, 1, 2, 3] are all in the "src/" prefix → filtered out
        let indices = vec![0, 1, 2, 3];
        let result = adjust_match_indices_for_name(
            &indices,
            "foo.rs",
            std::path::Path::new("/project/src/foo.rs"),
            &mode,
            std::path::Path::new("/project"),
        );
        assert_eq!(result, Vec::<u32>::new());
    }

    #[rstest]
    fn adjust_indices_path_mode_mixed_indices() {
        let mode = AppMode::Search(make_search_state(SearchMode::Path));
        // rel = "src/foo.rs" (10 chars), name = "foo.rs" (6 chars), offset = 4
        // index 0 → in prefix → filtered
        // index 4 → 4-4=0 → kept
        // index 9 → 9-4=5 → kept (within name_char_count=6)
        let indices = vec![0, 4, 9];
        let result = adjust_match_indices_for_name(
            &indices,
            "foo.rs",
            std::path::Path::new("/project/src/foo.rs"),
            &mode,
            std::path::Path::new("/project"),
        );
        assert_eq!(result, vec![0u32, 5]);
    }

    #[rstest]
    fn adjust_indices_path_mode_nested_directory() {
        let mode = AppMode::Search(make_search_state(SearchMode::Path));
        // path "src/app/handler.rs", root "/project", name "handler.rs"
        // rel = "src/app/handler.rs" (18 chars), name = "handler.rs" (10 chars), offset = 8
        let indices = vec![8, 9, 10];
        let result = adjust_match_indices_for_name(
            &indices,
            "handler.rs",
            std::path::Path::new("/project/src/app/handler.rs"),
            &mode,
            std::path::Path::new("/project"),
        );
        assert_eq!(result, vec![0u32, 1, 2]);
    }

    #[rstest]
    fn adjust_indices_path_mode_root_file() {
        let mode = AppMode::Search(make_search_state(SearchMode::Path));
        // path "Cargo.toml", root "/project", name "Cargo.toml"
        // rel = "Cargo.toml" (10 chars), name = "Cargo.toml" (10 chars), offset = 0
        let indices = vec![0, 1, 2];
        let result = adjust_match_indices_for_name(
            &indices,
            "Cargo.toml",
            std::path::Path::new("/project/Cargo.toml"),
            &mode,
            std::path::Path::new("/project"),
        );
        assert_eq!(result, vec![0u32, 1, 2]);
    }

    #[rstest]
    fn adjust_indices_normal_mode_returns_as_is() {
        let mode = AppMode::Normal;
        let indices = vec![0, 1, 2];
        let result = adjust_match_indices_for_name(
            &indices,
            "foo",
            std::path::Path::new("/project/src/foo"),
            &mode,
            std::path::Path::new("/project"),
        );
        assert_eq!(result, vec![0u32, 1, 2]);
    }

    #[rstest]
    fn adjust_indices_path_mode_strip_prefix_failure_returns_empty() {
        let mode = AppMode::Search(make_search_state(SearchMode::Path));
        // path does not start with root → strip_prefix fails → empty vec
        let indices = vec![0, 1, 2];
        let result = adjust_match_indices_for_name(
            &indices,
            "foo.rs",
            std::path::Path::new("/other/src/foo.rs"),
            &mode,
            std::path::Path::new("/project"),
        );
        assert_eq!(result, Vec::<u32>::new());
    }

    // --- push_highlighted_name ---

    #[rstest]
    fn highlighted_name_no_matches() {
        let mut spans = Vec::new();
        push_highlighted_name(&mut spans, "hello", None, &[], Style::default());
        assert_that!(spans.len(), eq(1));
        assert_that!(spans[0].content.as_ref(), eq("hello"));
        // No underline modifier
        assert!(!spans[0].style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[rstest]
    fn highlighted_name_all_matched() {
        let mut spans = Vec::new();
        push_highlighted_name(&mut spans, "abc", None, &[0, 1, 2], Style::default());
        // All characters highlighted → single span with UNDERLINED
        assert_that!(spans.len(), eq(1));
        assert_that!(spans[0].content.as_ref(), eq("abc"));
        assert!(spans[0].style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[rstest]
    fn highlighted_name_partial_match() {
        let mut spans = Vec::new();
        // "hello" with indices [1, 3]:
        // h(normal), e(highlight), l(normal), l(highlight), o(normal)
        push_highlighted_name(&mut spans, "hello", None, &[1, 3], Style::default());
        assert_that!(spans.len(), eq(5));
        assert_that!(spans[0].content.as_ref(), eq("h"));
        assert!(!spans[0].style.add_modifier.contains(Modifier::UNDERLINED));
        assert_that!(spans[1].content.as_ref(), eq("e"));
        assert!(spans[1].style.add_modifier.contains(Modifier::UNDERLINED));
        assert_that!(spans[2].content.as_ref(), eq("l"));
        assert!(!spans[2].style.add_modifier.contains(Modifier::UNDERLINED));
        assert_that!(spans[3].content.as_ref(), eq("l"));
        assert!(spans[3].style.add_modifier.contains(Modifier::UNDERLINED));
        assert_that!(spans[4].content.as_ref(), eq("o"));
        assert!(!spans[4].style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[rstest]
    fn highlighted_name_truncation() {
        let mut spans = Vec::new();
        // max_width=3 on "hello" → reserves 1 for "…", so 2 chars fit → "he" + "…"
        push_highlighted_name(&mut spans, "hello", Some(3), &[], Style::default());
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_that!(text.as_str(), eq("he…"));
    }

    #[rstest]
    fn highlighted_name_truncation_with_highlight() {
        let mut spans = Vec::new();
        // max_width=3, "hello" with indices [0, 1]
        // Target width = 2, so "h"(highlighted) + "e"(highlighted) + "…"
        push_highlighted_name(&mut spans, "hello", Some(3), &[0, 1], Style::default());
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_that!(text.as_str(), eq("he…"));
        // "he" should be underlined, "…" should not
        assert!(spans[0].style.add_modifier.contains(Modifier::UNDERLINED));
        // Last span is the ellipsis
        let last = &spans[spans.len() - 1];
        assert_that!(last.content.as_ref(), eq("…"));
        assert!(!last.style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[rstest]
    fn highlighted_name_empty() {
        let mut spans = Vec::new();
        push_highlighted_name(&mut spans, "", None, &[], Style::default());
        assert_that!(spans.len(), eq(0));
    }

    #[rstest]
    fn highlighted_name_unicode_width_truncation() {
        let mut spans = Vec::new();
        // "日本語" each CJK char is 2 columns wide → total width 6
        // max_width=4 → target_width=3, "日" takes 2 cols (fits), "本" takes 2 cols (2+2=4 > 3, doesn't fit)
        // Result: "日" + "…"
        push_highlighted_name(&mut spans, "日本語", Some(4), &[0], Style::default());
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_that!(text.as_str(), eq("日…"));
        // "日" should be underlined (index 0 matched)
        assert!(spans[0].style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[rstest]
    fn highlighted_name_consecutive_matches_grouped() {
        let mut spans = Vec::new();
        // "abcde" with indices [1, 2, 3]:
        // a(normal), b+c+d(highlighted), e(normal)
        push_highlighted_name(&mut spans, "abcde", None, &[1, 2, 3], Style::default());
        assert_that!(spans.len(), eq(3));
        assert_that!(spans[0].content.as_ref(), eq("a"));
        assert!(!spans[0].style.add_modifier.contains(Modifier::UNDERLINED));
        assert_that!(spans[1].content.as_ref(), eq("bcd"));
        assert!(spans[1].style.add_modifier.contains(Modifier::UNDERLINED));
        assert_that!(spans[2].content.as_ref(), eq("e"));
        assert!(!spans[2].style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[rstest]
    fn highlighted_name_base_style_preserved() {
        let mut spans = Vec::new();
        let base = Style::default().fg(Color::Red);
        push_highlighted_name(&mut spans, "ab", None, &[1], base);
        // "a" should have base style (fg Red, no underline)
        assert_that!(spans[0].style.fg, some(eq(Color::Red)));
        assert!(!spans[0].style.add_modifier.contains(Modifier::UNDERLINED));
        // "b" should have base style + underline
        assert_that!(spans[1].style.fg, some(eq(Color::Red)));
        assert!(spans[1].style.add_modifier.contains(Modifier::UNDERLINED));
    }
}
