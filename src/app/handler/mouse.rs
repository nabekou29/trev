//! Mouse event handlers for tree click and scroll wheel.

use crossterm::event::{
    MouseButton,
    MouseEvent,
    MouseEventKind,
};
use ratatui::layout::Rect;

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
pub fn handle_mouse_event(event: MouseEvent, state: &mut AppState, _ctx: &AppContext) {
    // Only handle mouse in Normal mode.
    if !matches!(state.mode, AppMode::Normal) {
        return;
    }

    let col = event.column;
    let row = event.row;

    match event.kind {
        MouseEventKind::ScrollDown => {
            handle_scroll(state, col, row, ScrollDirection::Down);
        }
        MouseEventKind::ScrollUp => {
            handle_scroll(state, col, row, ScrollDirection::Up);
        }
        MouseEventKind::Down(MouseButton::Left) => {
            handle_left_click(state, col, row);
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
fn handle_scroll(state: &mut AppState, col: u16, row: u16, direction: ScrollDirection) {
    let areas = state.layout_areas;

    if rect_contains(areas.tree_area, col, row) {
        handle_tree_scroll(state, direction);
    } else if rect_contains(areas.preview_area, col, row) {
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
    let viewport_height = state.layout_areas.preview_area.height as usize;
    match direction {
        ScrollDirection::Down => {
            state.preview_state.scroll_down(1, viewport_height);
        }
        ScrollDirection::Up => {
            state.preview_state.scroll_up(1);
        }
    }
}

/// Handle a left-click in the tree area: move cursor to the clicked row.
fn handle_left_click(state: &mut AppState, col: u16, row: u16) {
    let areas = state.layout_areas;

    if !rect_contains(areas.tree_area, col, row) {
        return;
    }

    // Calculate which row within the tree area was clicked.
    let row_in_area = (row.saturating_sub(areas.tree_area.y)) as usize;
    let target_index = state.scroll.offset().saturating_add(row_in_area);

    // Only move cursor if the target index is within valid bounds.
    let total = state.tree_state.visible_node_count();
    if target_index < total {
        state.tree_state.move_cursor_to(target_index);
    }
}

/// Check whether a point (col, row) is within a `Rect`.
const fn rect_contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use ratatui::layout::Rect;
    use rstest::*;

    use super::*;

    #[rstest]
    fn rect_contains_inside() {
        let rect = Rect::new(10, 5, 20, 10);
        assert_that!(rect_contains(rect, 15, 8), eq(true));
    }

    #[rstest]
    fn rect_contains_top_left_corner() {
        let rect = Rect::new(10, 5, 20, 10);
        assert_that!(rect_contains(rect, 10, 5), eq(true));
    }

    #[rstest]
    fn rect_contains_bottom_right_exclusive() {
        let rect = Rect::new(10, 5, 20, 10);
        // Right edge (x=30) and bottom edge (y=15) are exclusive.
        assert_that!(rect_contains(rect, 30, 14), eq(false));
        assert_that!(rect_contains(rect, 29, 15), eq(false));
    }

    #[rstest]
    fn rect_contains_outside() {
        let rect = Rect::new(10, 5, 20, 10);
        assert_that!(rect_contains(rect, 5, 3), eq(false));
    }

    #[rstest]
    fn rect_contains_zero_size() {
        let rect = Rect::new(10, 5, 0, 0);
        assert_that!(rect_contains(rect, 10, 5), eq(false));
    }
}
