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
