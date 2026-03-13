//! Status bar widget — displays indicators, status messages, file path, and cursor position.

use ratatui::Frame;
use ratatui::layout::{
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
use ratatui::widgets::Paragraph;

use crossterm::event::{
    KeyCode,
    KeyModifiers,
};

use crate::app::AppState;
use crate::app::keymap::ActionKeyLookup;
use crate::app::pending_keys::key_display;
use crate::file_op::selection::SelectionMode;
use crate::input::{
    AppMode,
    SearchPhase,
};

/// Click-target areas for the filter indicators in the status bar.
#[derive(Debug, Clone, Copy)]
pub struct FilterAreas {
    /// Area of the hidden-filter indicator.
    pub hidden: Rect,
    /// Area of the ignored-filter indicator.
    pub ignored: Rect,
}

/// Render the status bar into the given area.
///
/// Layout: `[indicators | center (fill) | filter | position]`
///
/// Returns the click-target areas for the filter indicators so the caller
/// can store them for mouse hit-testing.
///
/// `visible_count` is the pre-computed total visible node count for this frame.
#[expect(clippy::cast_possible_truncation, reason = "Terminal dimensions fit in u16")]
pub fn render_status(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &AppState,
    visible_count: usize,
    lookup: &ActionKeyLookup,
) -> FilterAreas {
    let base_style = Style::default();

    // --- Build spans for each section ---
    let indicator_spans = build_indicator_spans(state);
    let indicator_width: u16 = indicator_spans.iter().map(|s| s.width() as u16).sum();

    let filter = build_filter_spans(state);
    let filter_width: u16 = filter.spans.iter().map(|s| s.width() as u16).sum();

    let cursor = state.tree_state.cursor();
    let position = if visible_count > 0 {
        format!(" {}/{} ", cursor + 1, visible_count)
    } else {
        String::new()
    };
    let position_width = position.len() as u16;

    // --- Layout: [indicators | center (fill) | filter | position] ---
    let chunks = Layout::horizontal([
        Constraint::Length(indicator_width),
        Constraint::Fill(1),
        Constraint::Length(filter_width),
        Constraint::Length(position_width),
    ])
    .split(area);

    let Some((&indicator_area, rest)) = chunks.split_first() else {
        return FilterAreas { hidden: Rect::default(), ignored: Rect::default() };
    };
    let Some((&center_area, rest)) = rest.split_first() else {
        return FilterAreas { hidden: Rect::default(), ignored: Rect::default() };
    };
    let Some((&filter_area, rest)) = rest.split_first() else {
        return FilterAreas { hidden: Rect::default(), ignored: Rect::default() };
    };
    let Some(&position_area) = rest.first() else {
        return FilterAreas { hidden: Rect::default(), ignored: Rect::default() };
    };

    // Compute per-filter click areas within filter_area.
    let filter_areas = FilterAreas {
        hidden: Rect::new(filter_area.x, filter_area.y, filter.hidden_width, filter_area.height),
        ignored: Rect::new(
            filter_area.x.saturating_add(filter.hidden_width),
            filter_area.y,
            filter.ignored_width,
            filter_area.height,
        ),
    };

    // Render indicators (left).
    if !indicator_spans.is_empty() {
        let indicator_line = Line::from(indicator_spans);
        frame.render_widget(Paragraph::new(indicator_line).style(base_style), indicator_area);
    }

    // Render center content (key hints, message, or pending keys).
    let center_spans = build_center_spans(state, lookup);
    let center_line = Line::from(center_spans);
    frame.render_widget(Paragraph::new(center_line).style(base_style), center_area);

    // Render filter state (right, before position).
    if !filter.spans.is_empty() {
        let filter_line = Line::from(filter.spans);
        frame.render_widget(Paragraph::new(filter_line).style(base_style), filter_area);
    }

    // Render position (right).
    let position_line = Line::raw(position);
    frame.render_widget(Paragraph::new(position_line).style(base_style), position_area);

    filter_areas
}

/// Build indicator spans for mode and selection state (left side).
fn build_indicator_spans(state: &AppState) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    // --- Mode badge ---
    let (mode_label, mode_style) = match &state.mode {
        AppMode::Search(search) if search.phase == SearchPhase::Typing => (
            " SEARCH ",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        AppMode::Search(search) if search.phase == SearchPhase::Filtered => (
            " FILTER ",
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ),
        AppMode::Input(_) => (
            " INPUT ",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ),
        _ => (
            " NORMAL ",
            Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
        ),
    };
    spans.push(Span::styled(mode_label, mode_style));

    // --- Selection badges ---
    let count = state.selection.count();

    if count > 0 {
        match state.selection.mode() {
            Some(SelectionMode::Copy) => {
                spans.push(Span::styled(format!(" [Y:{count}]"), Style::default().fg(Color::Cyan)));
            }
            Some(SelectionMode::Cut) => {
                spans.push(Span::styled(
                    format!(" [X:{count}]"),
                    Style::default().fg(Color::Yellow),
                ));
            }
            Some(SelectionMode::Mark) => {
                spans
                    .push(Span::styled(format!(" [M:{count}]"), Style::default().fg(Color::Green)));
            }
            None => {}
        }
    }

    spans
}

/// Filter span group with per-filter widths for click hit-testing.
struct FilterSpans {
    /// All spans to render.
    spans: Vec<Span<'static>>,
    /// Character width of the hidden-filter portion.
    hidden_width: u16,
    /// Character width of the ignored-filter portion.
    ignored_width: u16,
}

/// Build filter state spans (right side, before position).
///
/// Uses eye icons with symbols: `󰈈 .*` (hidden), `󰈈 .git` (ignored).
/// Active filters (files shown) are bright; inactive (files hidden) are dimmed.
fn build_filter_spans(state: &AppState) -> FilterSpans {
    let mut spans = Vec::new();

    let hidden_width = push_filter_indicator(
        &mut spans,
        state.show_hidden,
        ".*",
        Color::Yellow,
        state.show_icons,
    );
    let ignored_width = push_filter_indicator(
        &mut spans,
        state.show_ignored,
        ".git",
        Color::Cyan,
        state.show_icons,
    );

    spans.push(Span::raw(" "));

    FilterSpans { spans, hidden_width, ignored_width }
}

/// Push spans for a single filter indicator and return its total character width.
///
/// When `active` is true (files visible), the indicator is bright with `color`.
/// When false, it is dimmed with `DarkGray`.
#[expect(clippy::cast_possible_truncation, reason = "Span widths fit in u16")]
fn push_filter_indicator(
    spans: &mut Vec<Span<'static>>,
    active: bool,
    label: &'static str,
    color: Color,
    show_icons: bool,
) -> u16 {
    let style = if active {
        Style::default().fg(color)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    if show_icons {
        let icon = if active { " \u{f06d0} " } else { " \u{f06d1} " };
        let icon_span = Span::styled(icon, style);
        let label_span = Span::styled(label, style);
        let width = (icon_span.width() + label_span.width()) as u16;
        spans.push(icon_span);
        spans.push(label_span);
        width
    } else {
        let span = Span::styled(format!(" {label}"), style);
        let width = span.width() as u16;
        spans.push(span);
        width
    }
}

/// Build center spans: processing > status message > pending keys > contextual key hints.
fn build_center_spans(state: &AppState, lookup: &ActionKeyLookup) -> Vec<Span<'static>> {
    if state.processing {
        return vec![Span::raw(" Processing...")];
    }

    if let Some(msg) = &state.status_message {
        return vec![Span::raw(format!(" {}", msg.text))];
    }

    // Show pending key sequence indicator.
    if state.pending_keys.is_pending() {
        return vec![Span::raw(format!(" {}", state.pending_keys.display_string()))];
    }

    // Contextual key hints based on current mode.
    build_key_hints(state, lookup)
}

/// Build contextual key hints for the current mode.
fn build_key_hints(state: &AppState, lookup: &ActionKeyLookup) -> Vec<Span<'static>> {
    match &state.mode {
        AppMode::Normal if state.selection.count() > 0 => selection_hints(state, lookup),
        AppMode::Normal => normal_hints(lookup),
        AppMode::Search(s) if s.phase == SearchPhase::Filtered => {
            filtered_hints(&s.buffer.value, lookup)
        }
        AppMode::Search(_) => vec![], // Typing phase: search_input.rs handles display
        AppMode::Input(_) => input_hints(),
        AppMode::Confirm(_) => confirm_hints(),
        AppMode::Menu(_) => menu_hints(),
        AppMode::Help(_) => help_hints(),
    }
}

/// Key hint pair: highlighted key + dim description.
fn hint(key: &str, desc: &str) -> [Span<'static>; 2] {
    [
        Span::styled(format!(" {key}"), Style::default().fg(Color::Yellow)),
        Span::styled(format!(":{desc}"), Style::default().fg(Color::Gray)),
    ]
}

/// Key hints when files are selected (yank/cut/mark).
fn selection_hints(state: &AppState, lookup: &ActionKeyLookup) -> Vec<Span<'static>> {
    match state.selection.mode() {
        Some(SelectionMode::Copy | SelectionMode::Cut) => {
            let paste = lookup.key_for("file_op.paste").unwrap_or("p");
            let clear = lookup.key_for("file_op.clear_selections").unwrap_or("<Esc>");
            let pairs = [hint(paste, "paste"), hint(clear, "clear")];
            pairs.into_iter().flatten().collect()
        }
        Some(SelectionMode::Mark) => {
            let yank = lookup.key_for("file_op.yank").unwrap_or("y");
            let cut = lookup.key_for("file_op.cut").unwrap_or("x");
            let delete = lookup.key_for("file_op.delete").unwrap_or("d");
            let clear = lookup.key_for("file_op.clear_selections").unwrap_or("<Esc>");
            let pairs =
                [hint(yank, "yank"), hint(cut, "cut"), hint(delete, "delete"), hint(clear, "clear")];
            pairs.into_iter().flatten().collect()
        }
        None => normal_hints(lookup),
    }
}

/// Key hints for Normal mode.
fn normal_hints(lookup: &ActionKeyLookup) -> Vec<Span<'static>> {
    let nav = lookup.key_pair("tree.move_down", "tree.move_up");
    let expand = lookup.key_pair("tree.expand", "tree.collapse");
    let search = lookup.key_for("search.open").unwrap_or("/");
    let hidden = lookup.key_for("filter.hidden").unwrap_or(".");
    let help = lookup.key_for("help").unwrap_or("?");
    let pairs = [
        hint(&nav, "navigate"),
        hint(&expand, "expand/collapse"),
        hint(search, "search"),
        hint(hidden, "hidden"),
        hint(help, "help"),
    ];
    pairs.into_iter().flatten().collect()
}

/// Key hints for Search Filtered mode.
fn filtered_hints(query: &str, lookup: &ActionKeyLookup) -> Vec<Span<'static>> {
    let esc = key_display(KeyCode::Esc, KeyModifiers::NONE);
    let nav = lookup.key_pair("tree.move_down", "tree.move_up");
    let mut spans = vec![Span::raw(format!(" /{query}"))];
    for s in hint(&esc, "clear") {
        spans.push(s);
    }
    for s in hint(&nav, "navigate") {
        spans.push(s);
    }
    spans
}

/// Key hints for Input mode.
fn input_hints() -> Vec<Span<'static>> {
    let enter = key_display(KeyCode::Enter, KeyModifiers::NONE);
    let esc = key_display(KeyCode::Esc, KeyModifiers::NONE);
    let pairs = [hint(&enter, "confirm"), hint(&esc, "cancel")];
    pairs.into_iter().flatten().collect()
}

/// Key hints for Confirm mode.
fn confirm_hints() -> Vec<Span<'static>> {
    let enter = key_display(KeyCode::Enter, KeyModifiers::NONE);
    let esc = key_display(KeyCode::Esc, KeyModifiers::NONE);
    let pairs = [
        hint(&format!("y/{enter}"), "confirm"),
        hint(&format!("n/{esc}"), "cancel"),
    ];
    pairs.into_iter().flatten().collect()
}

/// Key hints for Menu mode.
fn menu_hints() -> Vec<Span<'static>> {
    let enter = key_display(KeyCode::Enter, KeyModifiers::NONE);
    let esc = key_display(KeyCode::Esc, KeyModifiers::NONE);
    let pairs = [
        hint("j/k", "navigate"),
        hint(&enter, "select"),
        hint(&esc, "cancel"),
    ];
    pairs.into_iter().flatten().collect()
}

/// Key hints for Help mode.
fn help_hints() -> Vec<Span<'static>> {
    let esc = key_display(KeyCode::Esc, KeyModifiers::NONE);
    let pairs = [hint("j/k", "scroll"), hint(&format!("q/{esc}"), "close")];
    pairs.into_iter().flatten().collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;
    use crate::config::KeybindingConfig;

    /// Build an `ActionKeyLookup` with default keybindings for testing.
    fn default_lookup() -> ActionKeyLookup {
        use crate::app::keymap::KeyMap;
        let keymap = KeyMap::from_config(&KeybindingConfig::default(), &std::collections::HashMap::new());
        ActionKeyLookup::from_keymap(&keymap)
    }

    // =========================================================================
    // --- push_filter_indicator ---
    // =========================================================================

    #[rstest]
    fn push_filter_indicator_active_with_icons() {
        let mut spans = Vec::new();
        let width = push_filter_indicator(&mut spans, true, ".*", Color::Yellow, true);

        assert_that!(spans.len(), eq(2));
        // Icon span should contain the eye-open icon (U+F06D0).
        let icon_content = spans[0].content.as_ref();
        assert!(icon_content.contains('\u{f06d0}'), "expected eye-open icon, got: {icon_content}");
        // Label span.
        assert_that!(spans[1].content.as_ref(), eq(".*"));
        // Active uses the provided color.
        assert_that!(spans[0].style.fg, eq(Some(Color::Yellow)));
        assert_that!(spans[1].style.fg, eq(Some(Color::Yellow)));
        // Width should match the sum of both spans.
        let expected_width = u16::try_from(spans[0].width() + spans[1].width()).unwrap();
        assert_that!(width, eq(expected_width));
    }

    #[rstest]
    fn push_filter_indicator_inactive_with_icons() {
        let mut spans = Vec::new();
        let width = push_filter_indicator(&mut spans, false, ".*", Color::Yellow, true);

        assert_that!(spans.len(), eq(2));
        // Icon span should contain the eye-closed icon (U+F06D1).
        let icon_content = spans[0].content.as_ref();
        assert!(icon_content.contains('\u{f06d1}'), "expected eye-closed icon, got: {icon_content}");
        // Inactive uses DarkGray.
        assert_that!(spans[0].style.fg, eq(Some(Color::DarkGray)));
        assert_that!(spans[1].style.fg, eq(Some(Color::DarkGray)));
        let expected_width = u16::try_from(spans[0].width() + spans[1].width()).unwrap();
        assert_that!(width, eq(expected_width));
    }

    #[rstest]
    fn push_filter_indicator_active_without_icons() {
        let mut spans = Vec::new();
        let width = push_filter_indicator(&mut spans, true, ".*", Color::Cyan, false);

        assert_that!(spans.len(), eq(1));
        assert_that!(spans[0].content.as_ref(), eq(" .*"));
        assert_that!(spans[0].style.fg, eq(Some(Color::Cyan)));
        let expected_width = u16::try_from(spans[0].width()).unwrap();
        assert_that!(width, eq(expected_width));
    }

    #[rstest]
    fn push_filter_indicator_inactive_without_icons() {
        let mut spans = Vec::new();
        let width = push_filter_indicator(&mut spans, false, ".git", Color::Cyan, false);

        assert_that!(spans.len(), eq(1));
        assert_that!(spans[0].content.as_ref(), eq(" .git"));
        assert_that!(spans[0].style.fg, eq(Some(Color::DarkGray)));
        let expected_width = u16::try_from(spans[0].width()).unwrap();
        assert_that!(width, eq(expected_width));
    }

    #[rstest]
    fn push_filter_indicator_width_is_positive() {
        let mut spans = Vec::new();
        let width = push_filter_indicator(&mut spans, true, ".*", Color::Yellow, true);
        assert!(width > 0, "width should be positive, got: {width}");

        let mut spans2 = Vec::new();
        let width2 = push_filter_indicator(&mut spans2, false, ".git", Color::Cyan, false);
        assert!(width2 > 0, "width should be positive, got: {width2}");
    }

    // =========================================================================
    // --- hint ---
    // =========================================================================

    #[rstest]
    fn hint_returns_key_in_yellow_and_desc_in_gray() {
        let [key_span, desc_span] = hint("q", "quit");

        assert_that!(key_span.content.as_ref(), eq(" q"));
        assert_that!(key_span.style.fg, eq(Some(Color::Yellow)));

        assert_that!(desc_span.content.as_ref(), eq(":quit"));
        assert_that!(desc_span.style.fg, eq(Some(Color::Gray)));
    }

    #[rstest]
    fn hint_with_complex_key() {
        let [key_span, desc_span] = hint("<C-a>", "select all");

        assert_that!(key_span.content.as_ref(), eq(" <C-a>"));
        assert_that!(desc_span.content.as_ref(), eq(":select all"));
    }

    // =========================================================================
    // --- input_hints ---
    // =========================================================================

    #[rstest]
    fn input_hints_is_non_empty() {
        let spans = input_hints();
        assert!(!spans.is_empty(), "input_hints should return non-empty spans");
    }

    #[rstest]
    fn input_hints_contains_enter_and_esc() {
        let spans = input_hints();
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("Enter"), "expected Enter in input_hints, got: {text}");
        assert!(text.contains("Esc"), "expected Esc in input_hints, got: {text}");
    }

    #[rstest]
    fn input_hints_contains_confirm_and_cancel() {
        let spans = input_hints();
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("confirm"), "expected 'confirm' in input_hints, got: {text}");
        assert!(text.contains("cancel"), "expected 'cancel' in input_hints, got: {text}");
    }

    // =========================================================================
    // --- confirm_hints ---
    // =========================================================================

    #[rstest]
    fn confirm_hints_is_non_empty() {
        let spans = confirm_hints();
        assert!(!spans.is_empty(), "confirm_hints should return non-empty spans");
    }

    #[rstest]
    fn confirm_hints_contains_y_and_n() {
        let spans = confirm_hints();
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains('y'), "expected 'y' in confirm_hints, got: {text}");
        assert!(text.contains('n'), "expected 'n' in confirm_hints, got: {text}");
    }

    #[rstest]
    fn confirm_hints_contains_confirm_and_cancel() {
        let spans = confirm_hints();
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("confirm"), "expected 'confirm' in confirm_hints, got: {text}");
        assert!(text.contains("cancel"), "expected 'cancel' in confirm_hints, got: {text}");
    }

    // =========================================================================
    // --- menu_hints ---
    // =========================================================================

    #[rstest]
    fn menu_hints_is_non_empty() {
        let spans = menu_hints();
        assert!(!spans.is_empty(), "menu_hints should return non-empty spans");
    }

    #[rstest]
    fn menu_hints_contains_navigate_select_cancel() {
        let spans = menu_hints();
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("j/k"), "expected 'j/k' in menu_hints, got: {text}");
        assert!(text.contains("navigate"), "expected 'navigate' in menu_hints, got: {text}");
        assert!(text.contains("select"), "expected 'select' in menu_hints, got: {text}");
        assert!(text.contains("cancel"), "expected 'cancel' in menu_hints, got: {text}");
    }

    // =========================================================================
    // --- help_hints ---
    // =========================================================================

    #[rstest]
    fn help_hints_is_non_empty() {
        let spans = help_hints();
        assert!(!spans.is_empty(), "help_hints should return non-empty spans");
    }

    #[rstest]
    fn help_hints_contains_scroll_and_close() {
        let spans = help_hints();
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("j/k"), "expected 'j/k' in help_hints, got: {text}");
        assert!(text.contains("scroll"), "expected 'scroll' in help_hints, got: {text}");
        assert!(text.contains("close"), "expected 'close' in help_hints, got: {text}");
        assert!(text.contains('q'), "expected 'q' in help_hints, got: {text}");
    }

    // =========================================================================
    // --- filtered_hints ---
    // =========================================================================

    #[rstest]
    fn filtered_hints_contains_query_string() {
        let lookup = default_lookup();
        let spans = filtered_hints("hello", &lookup);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("/hello"), "expected query '/hello' in filtered_hints, got: {text}");
    }

    #[rstest]
    fn filtered_hints_contains_esc() {
        let lookup = default_lookup();
        let spans = filtered_hints("test", &lookup);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("Esc"), "expected 'Esc' in filtered_hints, got: {text}");
    }

    #[rstest]
    fn filtered_hints_contains_clear() {
        let lookup = default_lookup();
        let spans = filtered_hints("test", &lookup);
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("clear"), "expected 'clear' in filtered_hints, got: {text}");
    }
}
