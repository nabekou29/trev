//! UI layout and rendering.

pub mod column;
pub mod inline_input;
pub mod modal;
pub mod preview_view;
pub mod status_bar;
pub mod tree_view;

use ratatui::Frame;
use ratatui::layout::{
    Constraint,
    Direction,
    Layout,
};

use crate::app::AppState;
use crate::input::AppMode;

/// Render the entire UI.
///
/// Layout adapts to terminal width using configurable thresholds and split ratios
/// from `AppState` (`layout_narrow_width`, `layout_split_ratio`, `layout_narrow_split_ratio`).
///
/// - Wide (> threshold): horizontal split — tree | preview
/// - Narrow (<= threshold): vertical split — tree / preview
/// - Preview off: tree only (full area)
pub fn render(frame: &mut Frame<'_>, state: &mut AppState) {
    let chunks = Layout::vertical([
        Constraint::Min(1),    // main content
        Constraint::Length(1), // status bar
    ])
    .split(frame.area());

    let Some(&main_area) = chunks.first() else {
        return;
    };
    let Some(&status_area) = chunks.get(1) else {
        return;
    };

    if state.show_preview {
        let is_narrow = main_area.width <= state.layout_narrow_width;

        let (direction, tree_pct) = if is_narrow {
            (Direction::Vertical, state.layout_narrow_split_ratio)
        } else {
            (Direction::Horizontal, state.layout_split_ratio)
        };
        let preview_pct = 100_u16.saturating_sub(tree_pct);
        let constraints = [Constraint::Percentage(tree_pct), Constraint::Percentage(preview_pct)];

        let content_chunks =
            Layout::default().direction(direction).constraints(constraints).split(main_area);

        let Some(&tree_area) = content_chunks.first() else {
            return;
        };
        let Some(&preview_area) = content_chunks.get(1) else {
            return;
        };

        state.viewport_height = tree_area.height as usize;

        {
            let visible_count = state.tree_state.visible_node_count();
            let _span = tracing::info_span!("render_tree", visible_count).entered();
            tree_view::render_tree(frame, tree_area, state);
        }
        {
            let _span = tracing::info_span!("render_preview").entered();
            preview_view::render_preview(frame, preview_area, &mut state.preview_state, is_narrow);
        }
    } else {
        // Full width tree when preview is disabled.
        state.viewport_height = main_area.height as usize;

        let visible_count = state.tree_state.visible_nodes().len();
        let _span = tracing::info_span!("render_tree", visible_count).entered();
        tree_view::render_tree(frame, main_area, state);
    }

    status_bar::render_status(frame, status_area, state);

    // Render modal overlays on top of everything.
    match &state.mode {
        AppMode::Confirm(confirm) => {
            modal::render_confirm_dialog(frame, frame.area(), confirm);
        }
        AppMode::Menu(menu) => {
            modal::render_menu(frame, frame.area(), menu);
        }
        AppMode::Normal | AppMode::Input(_) => {}
    }
}
