//! Search mode handlers: fuzzy search with incremental filtering.
//!
//! The search flow has two phases:
//! - **Typing**: user enters a query; results are computed and the tree is
//!   filtered incrementally on each keystroke.
//! - **Filtered**: query is confirmed; the tree shows only matching entries
//!   and the user can navigate with normal tree keys. Esc clears the filter.

use std::path::Path;
use std::time::{
    Duration,
    Instant,
};

use crossterm::event::{
    KeyCode,
    KeyEvent,
};

use crate::app::handler::tree::spawn_load_children;
use crate::app::state::{
    AppContext,
    AppState,
    LoadKind,
};
use crate::input::{
    AppMode,
    SearchPhase,
    SearchState,
};
use crate::tree::search_engine;

/// Maximum number of search history entries to retain.
const MAX_SEARCH_HISTORY: usize = 50;

/// Transition from Normal mode to Search(Typing) mode.
pub fn open_search(state: &mut AppState) {
    state.mode = AppMode::Search(SearchState::new());
    state.dirty = true;
}

/// Handle key events in Search mode.
///
/// Dispatches based on the current search phase (Typing or Filtered).
pub fn handle_search_mode_key(key: KeyEvent, state: &mut AppState, ctx: &AppContext) {
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
fn handle_typing_key(key: KeyEvent, state: &mut AppState, ctx: &AppContext) {
    match key.code {
        KeyCode::Esc => {
            // Cancel search, restore normal tree view.
            state.search_debounce = None;
            state.clear_search();
            state.mode = AppMode::Normal;
            state.dirty = true;
        }
        KeyCode::Enter => {
            // Flush pending debounce so results are up-to-date before confirming.
            flush_search_debounce(state, ctx);
            confirm_search(state);
        }
        KeyCode::Up => {
            navigate_history(state, HistoryDirection::Older);
            run_incremental_search(state);
        }
        KeyCode::Down => {
            navigate_history(state, HistoryDirection::Newer);
            run_incremental_search(state);
        }
        KeyCode::Tab => {
            // Toggle search mode (Name ↔ Path) and re-run search.
            let AppMode::Search(ref mut search) = state.mode else {
                return;
            };
            search.mode = search.mode.toggle();
            run_incremental_search(state);
        }
        _ => {
            // Try editing the text buffer.
            let AppMode::Search(ref mut search) = state.mode else {
                return;
            };
            if search.buffer.handle_key_event(key) {
                // Text changed — run incremental search.
                run_incremental_search(state);
            }
        }
    }
}

/// Handle key events during the Filtered phase.
///
/// Normal tree navigation keys are passed through to the tree handler.
/// Esc clears the filter and returns to Normal mode.
/// `/` opens a new search from the filtered state.
fn handle_filtered_key(key: KeyEvent, state: &mut AppState, ctx: &AppContext) {
    match key.code {
        KeyCode::Esc => {
            // Clear filter and return to Normal.
            state.clear_search();
            state.mode = AppMode::Normal;
            state.dirty = true;
        }
        KeyCode::Char('/') => {
            // Start a new search.
            state.clear_search();
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

/// Re-apply the current search filter after a tree rebuild.
///
/// Called when the tree state is replaced (e.g. after toggling hidden/ignored
/// visibility) while a search is active, to restore the filtered view.
///
/// Instead of re-running a search (the index may be mid-rebuild and empty),
/// reconstructs the filter from the still-valid `search_match_indices`.
pub fn reapply_search(state: &mut AppState, ctx: &AppContext, original_cursor_path: Option<&Path>) {
    let phase = match state.mode {
        AppMode::Search(ref search) => {
            if search.buffer.value.is_empty() {
                return;
            }
            search.phase
        }
        _ => return,
    };

    if state.search_match_indices.is_empty() {
        return;
    }

    // Reconstruct visible paths from stored match results (matched paths + ancestors).
    let mut visible = std::collections::HashSet::new();
    for path in state.search_match_indices.keys() {
        visible.insert(path.clone());
        let mut ancestor = path.as_path();
        while let Some(parent) = ancestor.parent() {
            if parent == ctx.root_path.as_path() || !visible.insert(parent.to_path_buf()) {
                break;
            }
            ancestor = parent;
        }
    }

    // Register pending loads for ancestor directories in the new tree.
    let mut pending: Vec<std::path::PathBuf> = visible.iter().cloned().collect();
    pending.sort_by_key(|p| p.components().count());
    state.search_pending_loads = Some(pending);

    // set_search_filter enables virtual expansion, keeping all filter paths
    // visible even while directories are being loaded asynchronously.
    state.tree_state.set_search_filter(visible);
    schedule_search_loads(state, ctx);

    // Restore cursor to previous position if still visible.
    // Prefer the original cursor path (from before tree rebuild) over the
    // current one, which may point to a fallback node.
    let restore_path =
        original_cursor_path.map(Path::to_path_buf).or_else(|| state.tree_state.cursor_path());
    let restored = restore_path.as_ref().is_some_and(|cp| state.tree_state.move_cursor_to_path(cp));
    if !restored {
        // Path not found — clamp cursor to valid range so it doesn't vanish.
        state.tree_state.move_cursor_to(state.tree_state.cursor());
    }

    // For Filtered phase, expand loaded directories immediately but defer
    // pinning (disabling virtual expansion) until all pending loads complete.
    // This ensures newly revealed directories appear expanded while loading.
    // finalize_search_filter() is called from schedule_search_loads() once done.
    if phase == SearchPhase::Filtered {
        if let Some(filter) = state.tree_state.search_filter_paths().cloned() {
            state.tree_state.expand_paths(&filter);
        }
        if state.search_pending_loads.is_none() {
            state.tree_state.pin_search_filter();
        }
    }

    state.dirty = true;
}

/// Re-run the active search against the (newly rebuilt) index.
///
/// Called when the background search index build completes to replace stale
/// results from the previous visibility settings.
/// No-op when no search is active.
pub fn refresh_search(state: &mut AppState) {
    if !matches!(state.mode, AppMode::Search(_)) {
        return;
    }
    run_incremental_search(state);
}

/// Flush any pending search debounce, applying results immediately.
///
/// Called before actions that depend on up-to-date results (e.g. confirming
/// search with Enter) so the tree filter reflects the latest query.
fn flush_search_debounce(state: &mut AppState, ctx: &AppContext) {
    if state.search_debounce.take().is_some() {
        state.search_engine.tick(10);
        apply_nucleo_results(state, ctx);
    }
}

/// Debounce duration for search result application.
///
/// During rapid typing, intermediate Nucleo results are deferred until input
/// settles. Each keystroke resets the deadline, so only the final query's
/// results trigger the expensive `apply_nucleo_results` pipeline.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(100);

/// Run the fuzzy search: update the Nucleo pattern and trigger background matching.
///
/// Only updates the pattern (instant, ~42µs) and returns immediately so that
/// key input is never blocked. Sets a debounce deadline; the actual results
/// are applied in [`apply_nucleo_results`] once the deadline expires and
/// Nucleo workers have produced results.
fn run_incremental_search(state: &mut AppState) {
    let AppMode::Search(ref search) = state.mode else {
        return;
    };
    let query = &search.buffer.value;
    let mode = search.mode;

    if query.is_empty() {
        // Empty query: clear filter.
        state.search_debounce = None;
        state.clear_search();
        state.dirty = true;
        return;
    }

    // Update the Nucleo pattern — matching happens asynchronously in worker threads.
    state.search_engine.update_pattern(query, mode);

    // tick(0) dispatches work to workers without blocking. Without this call
    // the pattern change is only recorded and workers never start.
    state.search_engine.tick(0);

    // Reset debounce — results will be applied after input settles.
    state.search_debounce = Some(Instant::now() + SEARCH_DEBOUNCE);
    state.dirty = true;
}

/// Apply current Nucleo search results to the tree filter.
///
/// Called when Nucleo workers notify that results have changed, or immediately
/// after a pattern update. Reads the snapshot, updates match indices and the
/// tree filter.
pub fn apply_nucleo_results(state: &mut AppState, ctx: &AppContext) {
    let AppMode::Search(ref search) = state.mode else {
        return;
    };
    let mode = search.mode;

    if search.buffer.value.is_empty() {
        return;
    }

    let results = state.search_engine.collect_results(mode, usize::MAX);

    let current_path = state.tree_state.cursor_path();
    let first_result_path = results.first().map(|r| r.path.clone());
    let visible_paths = search_engine::compute_visible_paths(&results, &ctx.root_path);

    // Store match indices for highlight rendering (consume results by value
    // to avoid re-cloning the PathBuf and Vec<u32> that collect_results already owns).
    state.search_match_indices.clear();
    for r in results {
        state.search_match_indices.insert(r.path, r.match_indices);
    }

    // Register unloaded ancestor directories for progressive async loading.
    let mut pending: Vec<std::path::PathBuf> = visible_paths.iter().cloned().collect();
    pending.sort_by_key(|p| p.components().count());
    state.search_pending_loads = Some(pending);

    // Set filter immediately — already-loaded nodes are shown right away,
    // and more results appear progressively as async loads complete.
    state.tree_state.set_search_filter(visible_paths);

    // Schedule loads for directories whose parents are already loaded.
    schedule_search_loads(state, ctx);

    // When filtered results fit in the viewport, reset scroll to top so the
    // user can see all results at a glance.
    if state.tree_state.visible_node_count() <= state.viewport_height {
        state.scroll.set_offset(0);
    }

    // Keep cursor on the same file if it's still visible in the filtered
    // results; otherwise fall back to the first (highest score) result.
    let preserved = current_path.as_ref().is_some_and(|p| state.tree_state.move_cursor_to_path(p));
    if !preserved && let Some(ref first) = first_result_path {
        state.tree_state.move_cursor_to_path(first);
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
        state.clear_search();
        state.mode = AppMode::Normal;
        state.dirty = true;
        return;
    }

    // Add to search history (avoid consecutive duplicates).
    if state.search_history.last().is_none_or(|last| *last != query) {
        state.search_history.push(query);
        if state.search_history.len() > MAX_SEARCH_HISTORY {
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

/// Schedule async loads for search filter ancestor directories.
///
/// Iterates the pending load list (sorted shallowest-first) and spawns
/// async loads for directories whose parent is already loaded in the tree.
/// Directories whose parent is not yet loaded are kept in the pending list
/// and retried when their parent's load completes.
pub fn schedule_search_loads(state: &mut AppState, ctx: &AppContext) {
    let _span = tracing::info_span!("schedule_search_loads").entered();

    let Some(ref mut pending) = state.search_pending_loads else {
        return;
    };

    let show_hidden = state.show_hidden;
    let show_ignored = state.show_ignored;
    let total_pending = pending.len();

    // Phase 1: Batch transition NotLoaded → Loading (single find_node_mut per parent).
    let transitioned = {
        let _span = tracing::info_span!("prepare_transitions", total_pending).entered();
        state.tree_state.prepare_async_loads_batch(pending, false)
    };

    // Phase 2: Spawn async load tasks.
    {
        let _span = tracing::info_span!("spawn_tasks", count = transitioned.len()).entered();
        for load_path in &transitioned {
            spawn_load_children(
                &ctx.children_tx,
                load_path.clone(),
                show_hidden,
                show_ignored,
                LoadKind::SearchFilter,
            );
        }
    }

    let scheduled = transitioned;

    let scheduled_count = scheduled.len();
    let remaining = total_pending - scheduled_count;
    if !scheduled.is_empty() {
        let _span =
            tracing::info_span!("retain_pending", scheduled = scheduled_count, remaining).entered();
        let scheduled_set: std::collections::HashSet<std::path::PathBuf> =
            scheduled.into_iter().collect();
        pending.retain(|p| !scheduled_set.contains(p));
    }
    tracing::info!(
        scheduled = scheduled_count,
        remaining,
        total_pending,
        "schedule_search_loads complete"
    );
    if pending.is_empty() {
        state.search_pending_loads = None;
        // All ancestor loads are done. If in Filtered phase with virtual
        // expansion still active, finalize: expand all filter dirs and pin.
        finalize_search_filter(state);
    }
}

/// Finalize the search filter after all pending loads complete.
///
/// Expands all directories in the filter set and disables virtual expansion
/// so user collapse/expand is respected in the Filtered phase.
/// No-op if not in Filtered phase or virtual expansion is already off.
fn finalize_search_filter(state: &mut AppState) {
    if !matches!(state.mode, AppMode::Search(ref s) if s.phase == SearchPhase::Filtered) {
        return;
    }
    if !state.tree_state.has_search_virtual_expand() {
        return;
    }
    if let Some(filter) = state.tree_state.search_filter_paths().cloned() {
        state.tree_state.expand_paths(&filter);
    }
    state.tree_state.pin_search_filter();
}

/// Direction for search history navigation.
#[derive(Debug, Clone, Copy)]
enum HistoryDirection {
    /// Go to an older (previous) entry.
    Older,
    /// Go to a newer (next) entry.
    Newer,
}

/// Navigate search history with Up (older) / Down (newer).
fn navigate_history(state: &mut AppState, direction: HistoryDirection) {
    let AppMode::Search(ref mut search) = state.mode else {
        return;
    };

    if state.search_history.is_empty() {
        return;
    }

    let history_len = state.search_history.len();

    match direction {
        HistoryDirection::Older => {
            match search.history_index {
                None => {
                    // Save current query and jump to most recent history entry.
                    search.original_query = search.buffer.value.clone();
                    let idx = history_len - 1;
                    search.history_index = Some(idx);
                    if let Some(entry) = state.search_history.get(idx) {
                        search.buffer.set_value(entry);
                    }
                }
                Some(idx) if idx > 0 => {
                    let new_idx = idx - 1;
                    search.history_index = Some(new_idx);
                    if let Some(entry) = state.search_history.get(new_idx) {
                        search.buffer.set_value(entry);
                    }
                }
                Some(_) => {
                    // Already at oldest entry.
                }
            }
        }
        HistoryDirection::Newer => {
            match search.history_index {
                Some(idx) if idx + 1 < history_len => {
                    let new_idx = idx + 1;
                    search.history_index = Some(new_idx);
                    if let Some(entry) = state.search_history.get(new_idx) {
                        search.buffer.set_value(entry);
                    }
                }
                Some(_) => {
                    // Return to the original query.
                    search.history_index = None;
                    search.buffer.set_value(&search.original_query.clone());
                }
                None => {
                    // Already at the newest (current input).
                }
            }
        }
    }

    state.dirty = true;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use std::collections::HashMap;
    use std::sync::RwLock;
    use std::sync::atomic::AtomicBool;

    use googletest::prelude::*;
    use rstest::*;

    use super::*;
    use crate::app::keymap::{
        ActionKeyLookup,
        KeyMap,
    };
    use crate::app::state::AppContext;
    use crate::config::KeybindingConfig;
    use crate::input::TextBuffer;
    use crate::tree::search_engine::NucleoSearchEngine;
    use crate::tree::search_index::SearchIndex;

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

    /// Create a minimal `AppContext` for search tests.
    fn test_context(root: &Path) -> AppContext {
        let (children_tx, _) = tokio::sync::mpsc::channel(1);
        let (preview_tx, _) = tokio::sync::mpsc::channel(1);
        let (git_tx, _) = tokio::sync::mpsc::channel(1);
        let (rebuild_tx, _) = tokio::sync::mpsc::channel(1);
        let (search_index_ready_tx, _) = tokio::sync::mpsc::channel(1);
        let (stat_tx, _) = tokio::sync::mpsc::channel(1);
        let keymap = KeyMap::from_config(&KeybindingConfig::default(), &HashMap::new());
        let action_key_lookup = ActionKeyLookup::from_keymap(&keymap);
        AppContext {
            children_tx,
            preview_tx,
            preview_config: crate::config::PreviewConfig::default(),
            file_op_config: crate::config::FileOpConfig::default(),
            keymap,
            action_key_lookup,
            suppressed: std::sync::Arc::new(AtomicBool::new(false)),
            ipc_server: None,
            git_tx,
            git_enabled: false,
            root_path: root.to_path_buf(),
            rebuild_tx,
            menus: HashMap::new(),
            search_index: std::sync::Arc::new(RwLock::new(SearchIndex::new())),
            search_index_ready_tx,
            stat_tx,
            custom_actions: HashMap::new(),
        }
    }

    /// Helper: inject entries into the search engine and tick until done.
    fn inject_and_tick(engine: &mut NucleoSearchEngine, root: &Path, paths: &[&str]) {
        use crate::tree::search_engine::inject_entry;
        use crate::tree::search_index::SearchEntry;

        let injector = engine.injector();
        for &p in paths {
            let path = std::path::PathBuf::from(p);
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let entry = SearchEntry { path, name, is_dir: false };
            inject_entry(&injector, entry, root);
        }
        for _ in 0..100 {
            let status = engine.tick(50);
            if !status.running {
                break;
            }
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
        navigate_history(&mut state, HistoryDirection::Older);
        let AppMode::Search(ref s) = state.mode else {
            panic!("expected Search mode");
        };
        assert_that!(s.buffer.value.as_str(), eq("ccc"));
        assert_that!(s.history_index, eq(Some(2)));

        // Up again: should go to "bbb".
        navigate_history(&mut state, HistoryDirection::Older);
        let AppMode::Search(ref s) = state.mode else {
            panic!("expected Search mode");
        };
        assert_that!(s.buffer.value.as_str(), eq("bbb"));

        // Down: should go back to "ccc".
        navigate_history(&mut state, HistoryDirection::Newer);
        let AppMode::Search(ref s) = state.mode else {
            panic!("expected Search mode");
        };
        assert_that!(s.buffer.value.as_str(), eq("ccc"));

        // Down again: should restore original query.
        navigate_history(&mut state, HistoryDirection::Newer);
        let AppMode::Search(ref s) = state.mode else {
            panic!("expected Search mode");
        };
        assert_that!(s.buffer.value.as_str(), eq("current"));
        assert_that!(s.history_index, eq(None));
    }

    // ===================================================================
    // run_incremental_search tests
    // ===================================================================

    #[rstest]
    fn run_incremental_search_sets_debounce_for_nonempty_query() {
        let mut state = crate::app::state::tests::minimal_app_state();
        let root = Path::new("/test/root");

        // Inject items so the engine has something to match.
        inject_and_tick(&mut state.search_engine, root, &["/test/root/foo.txt"]);

        state.mode = AppMode::Search(search_with_query("foo"));
        assert!(state.search_debounce.is_none());

        run_incremental_search(&mut state);

        // Debounce should be set.
        assert!(state.search_debounce.is_some());
        assert!(state.dirty);
    }

    #[rstest]
    fn run_incremental_search_clears_filter_for_empty_query() {
        let mut state = crate::app::state::tests::minimal_app_state();

        // Set up some prior search state.
        state
            .search_match_indices
            .insert(std::path::PathBuf::from("/test/root/old.txt"), vec![0, 1]);
        state.mode = AppMode::Search(search_with_query(""));

        run_incremental_search(&mut state);

        // Debounce should NOT be set for empty query.
        assert!(state.search_debounce.is_none());
        // Match indices should be cleared.
        assert!(state.search_match_indices.is_empty());
    }

    #[rstest]
    fn run_incremental_search_noop_when_not_in_search_mode() {
        let mut state = crate::app::state::tests::minimal_app_state();
        state.mode = AppMode::Normal;

        run_incremental_search(&mut state);

        assert!(state.search_debounce.is_none());
    }

    // ===================================================================
    // flush_search_debounce tests
    // ===================================================================

    #[rstest]
    fn flush_search_debounce_noop_when_no_debounce() {
        let mut state = crate::app::state::tests::minimal_app_state();
        let ctx = test_context(Path::new("/test/root"));
        state.mode = AppMode::Search(search_with_query("foo"));
        state.search_debounce = None;

        flush_search_debounce(&mut state, &ctx);

        // Nothing should change.
        assert!(state.search_debounce.is_none());
        assert!(state.search_match_indices.is_empty());
    }

    #[rstest]
    fn flush_search_debounce_applies_results_immediately() {
        let mut state = crate::app::state::tests::minimal_app_state();
        let root = Path::new("/test/root");
        let ctx = test_context(root);

        // Inject items and set up a pending search.
        inject_and_tick(
            &mut state.search_engine,
            root,
            &["/test/root/foo.txt", "/test/root/bar.txt"],
        );

        state.mode = AppMode::Search(search_with_query("foo"));
        state.search_engine.update_pattern("foo", crate::input::SearchMode::Name);
        state.search_engine.tick(10);
        state.search_debounce = Some(Instant::now() + Duration::from_secs(10));

        flush_search_debounce(&mut state, &ctx);

        // Debounce should be cleared.
        assert!(state.search_debounce.is_none());
        // Results should have been applied (match indices populated).
        assert!(!state.search_match_indices.is_empty());
    }

    // ===================================================================
    // apply_nucleo_results tests
    // ===================================================================

    #[rstest]
    fn apply_nucleo_results_populates_match_indices() {
        let mut state = crate::app::state::tests::minimal_app_state();
        let root = Path::new("/test/root");
        let ctx = test_context(root);

        inject_and_tick(
            &mut state.search_engine,
            root,
            &["/test/root/foo.txt", "/test/root/bar.txt"],
        );

        state.mode = AppMode::Search(search_with_query("foo"));
        state.search_engine.update_pattern("foo", crate::input::SearchMode::Name);
        state.search_engine.tick(10);

        apply_nucleo_results(&mut state, &ctx);

        assert!(!state.search_match_indices.is_empty());
        assert!(state.search_match_indices.contains_key(Path::new("/test/root/foo.txt")));
        assert!(state.dirty);
    }

    #[rstest]
    fn apply_nucleo_results_noop_for_empty_query() {
        let mut state = crate::app::state::tests::minimal_app_state();
        let ctx = test_context(Path::new("/test/root"));
        state.mode = AppMode::Search(search_with_query(""));

        apply_nucleo_results(&mut state, &ctx);

        assert!(state.search_match_indices.is_empty());
    }

    #[rstest]
    fn apply_nucleo_results_noop_when_not_in_search_mode() {
        let mut state = crate::app::state::tests::minimal_app_state();
        let ctx = test_context(Path::new("/test/root"));
        state.mode = AppMode::Normal;

        apply_nucleo_results(&mut state, &ctx);

        assert!(state.search_match_indices.is_empty());
    }

    // ===================================================================
    // refresh_search tests
    // ===================================================================

    #[rstest]
    fn refresh_search_noop_when_not_in_search_mode() {
        let mut state = crate::app::state::tests::minimal_app_state();
        state.mode = AppMode::Normal;

        refresh_search(&mut state);

        assert!(state.search_debounce.is_none());
    }

    #[rstest]
    fn refresh_search_triggers_incremental_search() {
        let mut state = crate::app::state::tests::minimal_app_state();
        let root = Path::new("/test/root");
        inject_and_tick(&mut state.search_engine, root, &["/test/root/foo.txt"]);

        state.mode = AppMode::Search(search_with_query("foo"));

        refresh_search(&mut state);

        // Should have set the debounce (via run_incremental_search).
        assert!(state.search_debounce.is_some());
    }
}
