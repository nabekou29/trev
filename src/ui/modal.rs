//! Modal dialog widgets.

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
    List,
    ListItem,
    Paragraph,
};

use crate::input::{
    ConfirmState,
    MenuState,
};

/// Render a confirmation dialog as a centered modal overlay.
///
/// Shows the confirmation message, a list of affected file paths,
/// and a keybinding hint footer.
pub fn render_confirm_dialog(frame: &mut Frame<'_>, area: Rect, confirm: &ConfirmState) {
    let dialog_area = centered_rect(area, 60, 80, confirm.paths.len());

    // Clear the area behind the dialog.
    frame.render_widget(Clear, dialog_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                &confirm.message,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
        .title_alignment(Alignment::Center);

    let inner = block.inner(dialog_area);
    frame.render_widget(block, dialog_area);

    if inner.height < 2 {
        return;
    }

    // Split inner area: file list + footer.
    let chunks = Layout::vertical([
        Constraint::Min(1),    // file list
        Constraint::Length(1), // footer
    ])
    .split(inner);

    let Some(&list_area) = chunks.first() else {
        return;
    };
    let Some(&footer_area) = chunks.get(1) else {
        return;
    };

    // File list.
    let items: Vec<ListItem<'_>> = confirm
        .paths
        .iter()
        .map(|p| {
            let display =
                p.file_name().and_then(|n| n.to_str()).unwrap_or_else(|| p.to_str().unwrap_or("?"));
            ListItem::new(Span::styled(format!("  {display}"), Style::default().fg(Color::Red)))
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, list_area);

    // Footer with keybinding hints.
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" y", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw("/"),
        Span::styled("Enter", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::styled(": confirm  ", Style::default().fg(Color::DarkGray)),
        Span::styled("n", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw("/"),
        Span::styled("Esc", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::styled(": cancel", Style::default().fg(Color::DarkGray)),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(footer, footer_area);
}

/// Render a selection menu as a centered modal overlay.
///
/// Shows the menu title and a list of items with shortcut keys.
pub fn render_menu(frame: &mut Frame<'_>, area: Rect, menu: &MenuState) {
    let dialog_area = centered_rect(area, 50, 80, menu.items.len());

    // Clear the area behind the dialog.
    frame.render_widget(Clear, dialog_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                &menu.title,
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]))
        .title_alignment(Alignment::Center);

    let inner = block.inner(dialog_area);
    frame.render_widget(block, dialog_area);

    if inner.height < 2 {
        return;
    }

    // Split inner area: item list + footer.
    let chunks = Layout::vertical([
        Constraint::Min(1),    // item list
        Constraint::Length(1), // footer
    ])
    .split(inner);

    let Some(&list_area) = chunks.first() else {
        return;
    };
    let Some(&footer_area) = chunks.get(1) else {
        return;
    };

    // Menu items with cursor highlight.
    let items: Vec<ListItem<'_>> = menu
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = i == menu.cursor;
            let marker = if is_selected { ">" } else { " " };
            let label_style = if is_selected {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let bg_style = if is_selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::raw(format!(" {marker} ")),
                Span::styled(
                    format!("[{}]", item.key),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(&item.label, label_style),
            ]).style(bg_style))
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, list_area);

    // Footer.
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("Esc", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::styled(": cancel", Style::default().fg(Color::DarkGray)),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(footer, footer_area);
}

/// Calculate a centered rectangle within the given area.
///
/// `width_pct` and `max_height_pct` control sizing relative to the parent area.
/// Actual height adapts to `item_count` (file list entries + border/footer overhead).
fn centered_rect(area: Rect, width_pct: u16, max_height_pct: u16, item_count: usize) -> Rect {
    let width = area.width.saturating_mul(width_pct) / 100;
    // Border (2) + footer (1) + at least 1 item row.
    let content_height = u16::try_from(item_count).unwrap_or(u16::MAX).saturating_add(3);
    let max_height = area.height.saturating_mul(max_height_pct) / 100;
    let height = content_height.min(max_height).max(4);

    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area.y.saturating_add(area.height.saturating_sub(height) / 2);

    Rect::new(x, y, width.min(area.width), height.min(area.height))
}
