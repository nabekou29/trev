//! UI layout and rendering.

pub mod column;
pub mod file_style;
pub mod help_view;
pub mod inline_input;
pub mod modal;
pub mod preview_view;
pub mod search_input;
pub mod status_bar;
pub mod tree_view;

use ratatui::Frame;
use ratatui::layout::{
    Constraint,
    Direction,
    Layout,
};

use crate::app::keymap::ActionKeyLookup;
use crate::app::{
    AppState,
    LayoutAreas,
};
use crate::input::AppMode;

/// Render the entire UI.
///
/// Layout adapts to terminal width using configurable thresholds and split ratios
/// from `AppState` (`layout_narrow_width`, `layout_split_ratio`, `layout_narrow_split_ratio`).
///
/// - Wide (> threshold): horizontal split — tree | preview
/// - Narrow (<= threshold): vertical split — tree / preview
/// - Preview off: tree only (full area)
pub fn render(frame: &mut Frame<'_>, state: &mut AppState, key_lookup: &ActionKeyLookup) {
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

    // Compute visible node count once per frame — avoids redundant full tree walks
    // in render_tree (search bar) and render_status (position indicator).
    let visible_count = state.tree_state.visible_node_count();

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
        state.layout_areas = LayoutAreas { tree_area, preview_area, ..LayoutAreas::default() };

        tree_view::render_tree(frame, tree_area, state, visible_count);
        let provider_areas =
            preview_view::render_preview(frame, preview_area, &mut state.preview_state);
        state.layout_areas.provider_areas = provider_areas;
    } else {
        // Full width tree when preview is disabled.
        state.viewport_height = main_area.height as usize;
        state.layout_areas = LayoutAreas { tree_area: main_area, ..LayoutAreas::default() };

        tree_view::render_tree(frame, main_area, state, visible_count);
    }

    let filter_areas =
        status_bar::render_status(frame, status_area, state, visible_count, key_lookup);
    state.layout_areas.filter_hidden_area = filter_areas.hidden;
    state.layout_areas.filter_ignored_area = filter_areas.ignored;

    // Compute the bounding area for modal overlays.
    let modal_area = if state.modal_avoid_preview && state.show_preview {
        state.layout_areas.tree_area
    } else {
        frame.area()
    };

    // Render modal overlays on top of everything.
    match &mut state.mode {
        AppMode::Confirm(confirm) => {
            modal::render_confirm_dialog(frame, modal_area, confirm);
        }
        AppMode::Menu(menu) => {
            modal::render_menu(frame, modal_area, menu);
        }
        AppMode::Help(help) => {
            help_view::render_help(frame, modal_area, help);
        }
        AppMode::Normal | AppMode::Input(_) | AppMode::Search(_) => {}
    }
}
