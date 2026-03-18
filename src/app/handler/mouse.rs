//! Mouse event handlers for tree click and scroll wheel.

use crossterm::event::{
    MouseButton,
    MouseEvent,
    MouseEventKind,
};
use ratatui::layout::Position;

use crate::app::state::{
    AppContext,
    AppState,
};
use crate::input::AppMode;

/// Handle a mouse event and update application state.
///
/// Mouse events are only processed in Normal mode. Scroll wheel events
/// are dispatched to the tree or preview panel based on the cursor position.
/// Left-click in the tree area moves the cursor to the clicked row.
/// Left-click on filter indicators toggles hidden/ignored file visibility.
pub fn handle_mouse_event(event: MouseEvent, state: &mut AppState, ctx: &AppContext) {
    // Only handle mouse in Normal mode.
    if !matches!(state.mode, AppMode::Normal) {
        return;
    }

    let pos = Position::new(event.column, event.row);

    match event.kind {
        MouseEventKind::ScrollDown => {
            handle_scroll(state, pos, ScrollDirection::Down);
        }
        MouseEventKind::ScrollUp => {
            handle_scroll(state, pos, ScrollDirection::Up);
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if handle_filter_click(state, ctx, pos) || handle_provider_click(state, ctx, pos) {
                return;
            }
            handle_left_click(state, ctx, pos);
        }
        _ => {}
    }
}

/// Scroll direction for mouse wheel events.
#[derive(Clone, Copy)]
enum ScrollDirection {
    /// Scroll content downward (wheel away from user).
    Down,
    /// Scroll content upward (wheel toward user).
    Up,
}

/// Handle scroll wheel: dispatches to tree or preview based on mouse position.
fn handle_scroll(state: &mut AppState, pos: Position, direction: ScrollDirection) {
    let areas = &state.layout_areas;

    if areas.tree_area.contains(pos) {
        handle_tree_scroll(state, direction);
    } else if areas.preview_area.contains(pos) {
        handle_preview_scroll(state, direction);
    }
}

/// Scroll the tree view by moving the cursor.
///
/// Moves the cursor by 1 line in the given direction. The scroll offset
/// adjusts automatically via `clamp_to_cursor` in the event loop.
fn handle_tree_scroll(state: &mut AppState, direction: ScrollDirection) {
    let delta = match direction {
        ScrollDirection::Down => 1,
        ScrollDirection::Up => -1,
    };
    state.tree_state.move_cursor(delta);
}

/// Scroll the preview panel by 1 line.
fn handle_preview_scroll(state: &mut AppState, direction: ScrollDirection) {
    let viewport_height = state.layout_areas.preview_area.height.saturating_sub(2) as usize;
    match direction {
        ScrollDirection::Down => {
            state.preview_state.scroll_down(1, viewport_height);
        }
        ScrollDirection::Up => {
            state.preview_state.scroll_up(1);
        }
    }
}

/// Handle a left-click on filter indicators in the status bar.
///
/// Returns `true` if the click was consumed (hit a filter area).
fn handle_filter_click(state: &mut AppState, ctx: &AppContext, pos: Position) -> bool {
    use crate::action::FilterAction;

    let areas = &state.layout_areas;

    if areas.filter_hidden_area.contains(pos) {
        super::handle_filter_action(FilterAction::Hidden, state, ctx);
        return true;
    }
    if areas.filter_ignored_area.contains(pos) {
        super::handle_filter_action(FilterAction::Ignored, state, ctx);
        return true;
    }
    false
}

/// Handle a left-click on a provider indicator in the preview title.
///
/// Returns `true` if the click was consumed (hit a provider area).
fn handle_provider_click(state: &mut AppState, ctx: &AppContext, pos: Position) -> bool {
    let areas = &state.layout_areas;

    for (i, rect) in areas.provider_areas.iter().enumerate() {
        if rect.contains(pos) {
            if i != state.preview_state.active_provider_index {
                state.preview_state.active_provider_index = i;
                state.preview_state.scroll_row = 0;
                state.preview_state.scroll_col = 0;
                super::preview::reload_preview(state, ctx);
            }
            return true;
        }
    }
    false
}

/// Handle a left-click in the tree area: move cursor to the clicked row.
///
/// If the clicked node is a directory, also toggles expand/collapse.
fn handle_left_click(state: &mut AppState, ctx: &AppContext, pos: Position) {
    use crate::action::TreeAction;

    let areas = &state.layout_areas;

    if !areas.tree_area.contains(pos) {
        return;
    }

    // Calculate which row within the tree area was clicked.
    let row_in_area = (pos.y.saturating_sub(areas.tree_area.y)) as usize;
    let target_index = state.scroll.offset().saturating_add(row_in_area);

    // Only move cursor if the target index is within valid bounds.
    let total = state.tree_state.visible_node_count();
    if target_index < total {
        state.tree_state.move_cursor_to(target_index);

        // Toggle expand/collapse if the clicked node is a directory.
        if state.tree_state.current_node_info().is_some_and(|info| info.is_dir) {
            super::tree::handle_tree_action(TreeAction::ToggleExpand, state, ctx);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use ratatui::layout::{
        Position,
        Rect,
    };
    use rstest::*;

    /// Verify that `Rect::contains` behaves as expected for hit-testing.
    #[rstest]
    fn rect_contains_inside() {
        let rect = Rect::new(10, 5, 20, 10);
        assert_that!(rect.contains(Position::new(15, 8)), eq(true));
    }

    #[rstest]
    fn rect_contains_top_left_corner() {
        let rect = Rect::new(10, 5, 20, 10);
        assert_that!(rect.contains(Position::new(10, 5)), eq(true));
    }

    #[rstest]
    fn rect_contains_bottom_right_exclusive() {
        let rect = Rect::new(10, 5, 20, 10);
        // Right edge (x=30) and bottom edge (y=15) are exclusive.
        assert_that!(rect.contains(Position::new(30, 14)), eq(false));
        assert_that!(rect.contains(Position::new(29, 15)), eq(false));
    }

    #[rstest]
    fn rect_contains_outside() {
        let rect = Rect::new(10, 5, 20, 10);
        assert_that!(rect.contains(Position::new(5, 3)), eq(false));
    }

    #[rstest]
    fn rect_contains_zero_size() {
        let rect = Rect::new(10, 5, 0, 0);
        assert_that!(rect.contains(Position::new(10, 5)), eq(false));
    }
}
