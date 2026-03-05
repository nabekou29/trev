//! Search mode handlers: fuzzy search with incremental filtering.
//!
//! The search flow has two phases:
//! - **Typing**: user enters a query; results are computed and the tree is
//!   filtered incrementally on each keystroke.
//! - **Filtered**: query is confirmed; the tree shows only matching entries
//!   and the user can navigate with normal tree keys. Esc clears the filter.

use crossterm::event::{
    KeyCode,
    KeyEvent,
};

use crate::app::state::{
    AppContext,
    AppState,
};
use crate::input::{
    AppMode,
    SearchPhase,
    SearchState,
};
use crate::tree::builder::TreeBuilder;
use crate::tree::search_engine;

/// Transition from Normal mode to Search(Typing) mode.
pub fn open_search(state: &mut AppState) {
    state.mode = AppMode::Search(SearchState::new());
    state.dirty = true;
}

/// Handle key events in Search mode.
///
/// Dispatches based on the current search phase (Typing or Filtered).
pub fn handle_search_mode_key(
    key: KeyEvent,
    state: &mut AppState,
    ctx: &AppContext,
) {
    let AppMode::Search(ref search) = state.mode else {
        return;
    };

    match search.phase {
        SearchPhase::Typing => handle_typing_key(key, state, ctx),
        SearchPhase::Filtered => handle_filtered_key(key, state, ctx),
    }
}

/// Handle key events during the Typing phase.
///
/// - Printable characters: edit query buffer and run incremental search.
/// - Enter: confirm search and transition to Filtered phase.
/// - Esc: cancel search and return to Normal mode.
/// - Up/Down: navigate search history.
fn handle_typing_key(
    key: KeyEvent,
    state: &mut AppState,
    ctx: &AppContext,
) {
    match key.code {
        KeyCode::Esc => {
            // Cancel search, restore normal tree view.
            state.tree_state.clear_search_filter();
            state.search_match_indices.clear();
            state.mode = AppMode::Normal;
            state.dirty = true;
        }
        KeyCode::Enter => {
            confirm_search(state);
        }
        KeyCode::Up => {
            navigate_history(state, -1);
            run_incremental_search(state, ctx);
        }
        KeyCode::Down => {
            navigate_history(state, 1);
            run_incremental_search(state, ctx);
        }
        KeyCode::Tab => {
            // Toggle search mode (Name ↔ Path) and re-run search.
            let AppMode::Search(ref mut search) = state.mode else {
                return;
            };
            search.mode = search.mode.toggle();
            run_incremental_search(state, ctx);
        }
        _ => {
            // Try editing the text buffer.
            let AppMode::Search(ref mut search) = state.mode else {
                return;
            };
            if search.buffer.handle_key_event(key) {
                // Text changed — run incremental search.
                run_incremental_search(state, ctx);
            }
        }
    }
}

/// Handle key events during the Filtered phase.
///
/// Normal tree navigation keys are passed through to the tree handler.
/// Esc clears the filter and returns to Normal mode.
/// `/` opens a new search from the filtered state.
fn handle_filtered_key(
    key: KeyEvent,
    state: &mut AppState,
    ctx: &AppContext,
) {
    match key.code {
        KeyCode::Esc => {
            // Clear filter and return to Normal.
            state.tree_state.clear_search_filter();
            state.search_match_indices.clear();
            state.mode = AppMode::Normal;
            state.dirty = true;
        }
        KeyCode::Char('/') => {
            // Start a new search.
            state.tree_state.clear_search_filter();
            open_search(state);
        }
        _ => {
            // Delegate to normal mode key handling for tree navigation.
            // Temporarily switch to Normal mode for dispatch, then restore.
            let search_state = std::mem::take(&mut state.mode);
            state.mode = AppMode::Normal;
            super::handle_normal_mode_key(key, state, ctx);
            // If the handler didn't change the mode (still Normal), restore Search.
            if matches!(state.mode, AppMode::Normal) {
                state.mode = search_state;
            }
        }
    }
}

/// Run the fuzzy search against the index and update the tree filter.
fn run_incremental_search(state: &mut AppState, ctx: &AppContext) {
    let AppMode::Search(ref search) = state.mode else {
        return;
    };
    let query = &search.buffer.value;
    let mode = search.mode;

    if query.is_empty() {
        // Empty query: clear filter.
        state.tree_state.clear_search_filter();
        state.search_match_indices.clear();
        state.dirty = true;
        return;
    }

    // Read the search index (may be partially built).
    let Ok(index) = ctx.search_index.try_read() else {
        // Index is being written to; skip this update.
        return;
    };

    let root_path = ctx.root_path.clone();
    let results = search_engine::search(
        index.entries(),
        query,
        &root_path,
        ctx.search_max_results,
        mode,
    );

    // Store match indices for highlight rendering.
    state.search_match_indices.clear();
    for r in &results {
        state.search_match_indices.insert(r.path.clone(), r.match_indices.clone());
    }

    let current_path = state.tree_state.cursor_path();
    let visible_paths = search_engine::compute_visible_paths(&results, &root_path);

    // Load NotLoaded directories so collect_visible_filtered can traverse them.
    let builder = TreeBuilder::new(state.show_hidden, state.show_ignored);
    state.tree_state.ensure_filter_paths_loaded(&visible_paths, builder);

    state.tree_state.set_search_filter(visible_paths);

    // When filtered results fit in the viewport, reset scroll to top so the
    // user can see all results at a glance. Search is a "find the target
    // quickly" operation — if a stale scroll offset hides part of a small
    // result set, the user has to scroll manually to discover matches that
    // are already on screen, which defeats the purpose.
    if state.tree_state.visible_node_count() <= state.viewport_height {
        state.scroll.set_offset(0);
    }

    // Keep cursor on the same file if it's still visible in the filtered
    // results; otherwise fall back to the first (highest score) result.
    let preserved = current_path
        .as_ref()
        .is_some_and(|p| state.tree_state.move_cursor_to_path(p));
    if !preserved
        && let Some(first) = results.first()
    {
        state.tree_state.move_cursor_to_path(&first.path);
    }

    state.dirty = true;
}

/// Confirm the search: transition from Typing to Filtered phase.
///
/// Adds the query to search history and expands parent directories of
/// matched entries in the real tree so they remain visible after clearing
/// the filter.
fn confirm_search(state: &mut AppState) {
    let AppMode::Search(ref mut search) = state.mode else {
        return;
    };

    let query = search.buffer.value.clone();
    if query.is_empty() {
        // Empty query: cancel search.
        state.tree_state.clear_search_filter();
        state.mode = AppMode::Normal;
        state.dirty = true;
        return;
    }

    // Add to search history (avoid consecutive duplicates).
    if state.search_history.last().is_none_or(|last| *last != query) {
        state.search_history.push(query);
        // Cap history at 50 entries.
        if state.search_history.len() > 50 {
            state.search_history.remove(0);
        }
    }

    // Expand directories in the filter set so they are truly expanded
    // and can be freely collapsed/expanded in the Filtered phase.
    if let Some(filter) = state.tree_state.search_filter_paths().cloned() {
        state.tree_state.expand_paths(&filter);
    }
    state.tree_state.pin_search_filter();

    search.phase = SearchPhase::Filtered;
    state.dirty = true;
}

/// Navigate search history with Up (older) / Down (newer).
fn navigate_history(state: &mut AppState, direction: i32) {
    let AppMode::Search(ref mut search) = state.mode else {
        return;
    };

    if state.search_history.is_empty() {
        return;
    }

    let history_len = state.search_history.len();

    match direction {
        // Up: go to older entry.
        -1 => {
            match search.history_index {
                None => {
                    // Save current query and jump to most recent history entry.
                    search.original_query = search.buffer.value.clone();
                    let idx = history_len - 1;
                    search.history_index = Some(idx);
                    if let Some(entry) = state.search_history.get(idx) {
                        search.buffer.value.clone_from(entry);
                        search.buffer.cursor_pos = search.buffer.value.len();
                    }
                }
                Some(idx) if idx > 0 => {
                    let new_idx = idx - 1;
                    search.history_index = Some(new_idx);
                    if let Some(entry) = state.search_history.get(new_idx) {
                        search.buffer.value.clone_from(entry);
                        search.buffer.cursor_pos = search.buffer.value.len();
                    }
                }
                Some(_) => {
                    // Already at oldest entry.
                }
            }
        }
        // Down: go to newer entry.
        1 => {
            match search.history_index {
                Some(idx) if idx + 1 < history_len => {
                    let new_idx = idx + 1;
                    search.history_index = Some(new_idx);
                    if let Some(entry) = state.search_history.get(new_idx) {
                        search.buffer.value.clone_from(entry);
                        search.buffer.cursor_pos = search.buffer.value.len();
                    }
                }
                Some(_) => {
                    // Return to the original query.
                    search.history_index = None;
                    search.buffer.value = search.original_query.clone();
                    search.buffer.cursor_pos = search.buffer.value.len();
                }
                None => {
                    // Already at the newest (current input).
                }
            }
        }
        _ => {}
    }

    state.dirty = true;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;
    use crate::input::TextBuffer;

    /// Build a `SearchState` with the given query.
    fn search_with_query(query: &str) -> SearchState {
        SearchState {
            buffer: TextBuffer::with_value(query.to_string(), query.len()),
            phase: SearchPhase::Typing,
            mode: crate::input::SearchMode::Name,
            history_index: None,
            original_query: String::new(),
        }
    }

    #[rstest]
    fn confirm_search_adds_to_history() {
        let mut state = crate::app::state::tests::minimal_app_state();
        state.mode = AppMode::Search(search_with_query("test"));

        confirm_search(&mut state);

        assert_that!(state.search_history.len(), eq(1));
        assert_that!(state.search_history[0].as_str(), eq("test"));
        // Mode should be Search(Filtered).
        let AppMode::Search(ref s) = state.mode else {
            panic!("expected Search mode");
        };
        assert_that!(s.phase, eq(SearchPhase::Filtered));
    }

    #[rstest]
    fn confirm_empty_search_cancels() {
        let mut state = crate::app::state::tests::minimal_app_state();
        state.mode = AppMode::Search(search_with_query(""));

        confirm_search(&mut state);

        assert!(matches!(state.mode, AppMode::Normal));
        assert_that!(state.search_history.len(), eq(0));
    }

    #[rstest]
    fn confirm_search_deduplicates_consecutive() {
        let mut state = crate::app::state::tests::minimal_app_state();
        state.search_history.push("test".to_string());
        state.mode = AppMode::Search(search_with_query("test"));

        confirm_search(&mut state);

        // Should not add duplicate.
        assert_that!(state.search_history.len(), eq(1));
    }

    #[rstest]
    fn navigate_history_up_down() {
        let mut state = crate::app::state::tests::minimal_app_state();
        state.search_history = vec!["aaa".to_string(), "bbb".to_string(), "ccc".to_string()];
        state.mode = AppMode::Search(search_with_query("current"));

        // Up: should go to "ccc" (most recent).
        navigate_history(&mut state, -1);
        let AppMode::Search(ref s) = state.mode else {
            panic!("expected Search mode");
        };
        assert_that!(s.buffer.value.as_str(), eq("ccc"));
        assert_that!(s.history_index, eq(Some(2)));

        // Up again: should go to "bbb".
        navigate_history(&mut state, -1);
        let AppMode::Search(ref s) = state.mode else {
            panic!("expected Search mode");
        };
        assert_that!(s.buffer.value.as_str(), eq("bbb"));

        // Down: should go back to "ccc".
        navigate_history(&mut state, 1);
        let AppMode::Search(ref s) = state.mode else {
            panic!("expected Search mode");
        };
        assert_that!(s.buffer.value.as_str(), eq("ccc"));

        // Down again: should restore original query.
        navigate_history(&mut state, 1);
        let AppMode::Search(ref s) = state.mode else {
            panic!("expected Search mode");
        };
        assert_that!(s.buffer.value.as_str(), eq("current"));
        assert_that!(s.history_index, eq(None));
    }
}
