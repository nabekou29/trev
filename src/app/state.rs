//! Application state, context, and result types.

use std::collections::HashMap;
use std::path::{
    Path,
    PathBuf,
};
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::time::{
    Duration,
    Instant,
};

use ratatui::layout::Rect;

use super::keymap::{
    ActionKeyLookup,
    KeyMap,
};
use super::pending_keys::PendingKeys;
use crate::config::{
    FileOpConfig,
    MenuDefinition,
    PreviewConfig,
};
use crate::file_op::selection::SelectionBuffer;
use crate::file_op::undo::UndoHistory;
use crate::git::{
    GitState,
    GitStatusResult,
};
use crate::input::AppMode;
use crate::preview::cache::PreviewCache;
use crate::preview::content::PreviewContent;
use crate::preview::provider::PreviewRegistry;
use crate::preview::state::PreviewState;
use crate::state::tree::{
    TreeNode,
    TreeState,
};
use crate::ui::column::ResolvedColumn;
use crate::watcher::FsWatcher;

/// Application-wide state wrapping tree state and UI settings.
#[derive(Debug)]
#[expect(clippy::struct_excessive_bools, reason = "AppState aggregates independent feature flags")]
pub struct AppState {
    /// Tree state (cursor, sort, nodes).
    pub tree_state: TreeState,
    /// Preview display state.
    pub preview_state: PreviewState,
    /// Preview content cache (LRU).
    pub preview_cache: PreviewCache,
    /// Preview provider registry.
    pub preview_registry: PreviewRegistry,
    /// Current application mode (Normal, Input, Confirm).
    pub mode: AppMode,
    /// Unified selection buffer for mark, copy, and cut operations.
    pub selection: SelectionBuffer,
    /// Undo/redo history for file operations.
    pub undo_history: UndoHistory,
    /// File system watcher (None if disabled).
    pub watcher: Option<FsWatcher>,
    /// Whether the application should quit.
    pub should_quit: bool,
    /// Whether to show file icons (Nerd Fonts).
    pub show_icons: bool,
    /// Whether the preview panel is visible.
    pub show_preview: bool,
    /// Whether to show hidden (dot) files.
    pub show_hidden: bool,
    /// Whether to show gitignored files.
    pub show_ignored: bool,
    /// Current viewport height (tree area rows).
    pub viewport_height: usize,
    /// Scroll state for the tree view.
    pub scroll: ScrollState,
    /// Temporary status message displayed in the status bar.
    pub status_message: Option<StatusMessage>,
    /// Whether a blocking file operation is in progress.
    pub processing: bool,
    /// Git repository status (None when git is disabled or outside a repo).
    ///
    /// Shared via `Arc<RwLock<…>>` so `ExternalCmdProvider` instances can read
    /// the current status in `can_handle()` without borrowing `AppState`.
    pub git_state: Arc<RwLock<Option<GitState>>>,
    /// Generation counter for tree rebuilds (latest-wins on rapid toggles).
    pub rebuild_generation: u64,
    /// Resolved column definitions for the metadata columns display.
    pub columns: Vec<ResolvedColumn>,
    /// Tree/preview split percentage in wide layout.
    pub layout_split_ratio: u16,
    /// Tree/preview split percentage in narrow layout.
    pub layout_narrow_split_ratio: u16,
    /// Width threshold for narrow layout (columns).
    pub layout_narrow_width: u16,
    /// Pending key presses for multi-key sequence resolution.
    pub pending_keys: PendingKeys,
    /// Whether the terminal needs a full redraw (e.g. after shell command execution).
    pub needs_redraw: bool,
    /// Whether the UI is dirty and needs to be redrawn.
    ///
    /// Set to `true` when state changes (key events, async results, status expiry).
    /// The event loop skips `terminal.draw()` when `false`, reducing CPU usage at idle.
    pub dirty: bool,
    /// Compiled file style matcher for per-file display customization.
    pub file_style_matcher: crate::ui::file_style::FileStyleMatcher,
    /// Debounce deadline for deferred preview loads (cache-miss only).
    ///
    /// When set, a preview load is waiting to fire after the cursor settles.
    /// Resets on each cursor change to avoid loading during rapid navigation.
    pub preview_debounce: Option<Instant>,
    /// Cached layout areas from the last render for mouse hit-testing.
    pub layout_areas: LayoutAreas,
    /// Deferred session restore: expanded directories loaded asynchronously after first render.
    pub deferred_expansion: Option<DeferredExpansion>,
    /// Search history (most recent last).
    pub search_history: Vec<String>,
    /// Per-path match highlight indices from the last search.
    ///
    /// Maps absolute paths to character indices within the matched string
    /// (file name or relative path depending on `SearchMode`).
    pub search_match_indices: HashMap<PathBuf, Vec<u32>>,
    /// Pending search filter directory loads (sorted by depth, shallowest first).
    ///
    /// Set during incremental search when ancestor directories need async loading.
    /// Cleared when search exits or all loads complete.
    pub search_pending_loads: Option<Vec<PathBuf>>,
    /// Cancellation token for the background search index build.
    ///
    /// Set to `true` to cancel the in-flight build before starting a new one
    /// (e.g. when toggling hidden/ignored file visibility).
    pub search_index_cancelled: Arc<AtomicBool>,
}

/// Cached layout areas from the last render, used for mouse hit-testing.
///
/// Stores the tree and preview panel rectangles so the mouse event handler
/// can determine which panel a click or scroll event targets.
#[derive(Debug, Clone, Copy, Default)]
pub struct LayoutAreas {
    /// The tree view area.
    pub tree_area: Rect,
    /// The preview area (zero-sized when preview is off).
    pub preview_area: Rect,
    /// The hidden-filter indicator area in the status bar (for click toggle).
    pub filter_hidden_area: Rect,
    /// The ignored-filter indicator area in the status bar (for click toggle).
    pub filter_ignored_area: Rect,
}

impl AppState {
    /// Clear search filter and associated match highlight indices.
    ///
    /// This is the single exit point for any search cancellation or
    /// completion that should restore normal tree visibility.
    pub fn clear_search(&mut self) {
        self.tree_state.clear_search_filter();
        self.search_match_indices.clear();
        self.search_pending_loads = None;
    }

    /// Set a temporary status message (auto-clears after 3 seconds).
    pub fn set_status(&mut self, text: impl Into<String>) {
        self.status_message = Some(StatusMessage::new(text));
    }

    /// Expanded directory paths, including not-yet-loaded deferred ones.
    ///
    /// Ensures quitting during deferred restore does not lose pending paths.
    pub fn expanded_paths_including_deferred(&self) -> Vec<PathBuf> {
        let mut paths = self.tree_state.expanded_paths();
        if let Some(ref deferred) = self.deferred_expansion {
            paths.extend_from_slice(deferred.remaining_paths());
        }
        paths
    }

    /// Clear the status message if it has expired.
    ///
    /// Returns `true` if a message was actually cleared (UI needs redraw).
    pub fn clear_expired_status(&mut self) -> bool {
        if self.status_message.as_ref().is_some_and(StatusMessage::is_expired) {
            self.status_message = None;
            true
        } else {
            false
        }
    }
}

/// Temporary status message with auto-expiry.
#[derive(Debug)]
pub struct StatusMessage {
    /// Message text.
    pub text: String,
    /// When the message was created.
    created_at: Instant,
}

impl StatusMessage {
    /// Status message display duration.
    const DURATION: Duration = Duration::from_secs(3);

    /// Create a new status message timestamped to now.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), created_at: Instant::now() }
    }

    /// Whether the message has expired.
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() >= Self::DURATION
    }

    /// Time remaining until the message expires.
    ///
    /// Returns `Duration::ZERO` if already expired.
    pub fn remaining(&self) -> Duration {
        Self::DURATION.saturating_sub(self.created_at.elapsed())
    }
}

/// Scroll position management for the tree view.
#[derive(Debug, Clone, Copy)]
pub struct ScrollState {
    /// Scroll offset (first visible row index).
    offset: usize,
}

impl ScrollState {
    /// Create a new scroll state starting at the top.
    pub const fn new() -> Self {
        Self { offset: 0 }
    }

    /// Get the current scroll offset.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Set the scroll offset so the cursor is centered in the viewport.
    pub const fn center_on_cursor(&mut self, cursor: usize, viewport_height: usize) {
        if viewport_height == 0 {
            self.offset = 0;
            return;
        }
        self.offset = cursor.saturating_sub(viewport_height / 2);
    }

    /// Set the scroll offset so the cursor is at the top of the viewport.
    pub const fn scroll_cursor_to_top(&mut self, cursor: usize) {
        self.offset = cursor;
    }

    /// Set the scroll offset so the cursor is at the bottom of the viewport.
    pub const fn scroll_cursor_to_bottom(&mut self, cursor: usize, viewport_height: usize) {
        if viewport_height == 0 {
            self.offset = 0;
            return;
        }
        self.offset = cursor.saturating_sub(viewport_height - 1);
    }

    /// Set the scroll offset directly.
    pub const fn set_offset(&mut self, offset: usize) {
        self.offset = offset;
    }

    /// Clamp the scroll offset so the cursor stays visible.
    ///
    /// Ensures `cursor - viewport_height + 1 <= offset <= cursor`.
    ///
    /// **Note:** this method does not account for the total number of items.
    /// Call [`clamp_to_total`](Self::clamp_to_total) afterwards to prevent
    /// over-scrolling past the end of the list.
    pub const fn clamp_to_cursor(&mut self, cursor: usize, viewport_height: usize) {
        if viewport_height == 0 {
            self.offset = 0;
            return;
        }
        // If cursor is above the current viewport, scroll up.
        if cursor < self.offset {
            self.offset = cursor;
        }
        // If cursor is below the current viewport, scroll down.
        if cursor >= self.offset + viewport_height {
            self.offset = cursor.saturating_sub(viewport_height - 1);
        }
    }

    /// Clamp the scroll offset so it does not exceed the maximum scrollable
    /// position.
    ///
    /// When `total_items <= viewport_height`, the offset is forced to 0
    /// because all items fit on screen. Otherwise the offset is capped at
    /// `total_items - viewport_height` so the bottom of the list aligns with
    /// the bottom of the viewport (no blank space below the last item).
    pub const fn clamp_to_total(&mut self, total_items: usize, viewport_height: usize) {
        let max_offset = total_items.saturating_sub(viewport_height);
        if self.offset > max_offset {
            self.offset = max_offset;
        }
    }
}

/// Snapshot of cursor position and on-screen row before a tree modification.
///
/// Captured before operations that change the tree structure (deferred
/// expansion, rebuild, etc.) and restored afterwards so the cursor stays
/// at the same visual position on screen.
#[derive(Debug, Clone)]
pub struct CursorSnapshot {
    /// Path the cursor was on (used to relocate after tree changes).
    pub path: Option<PathBuf>,
    /// On-screen row (`cursor_index − scroll.offset()`).
    pub visual_row: usize,
}

impl CursorSnapshot {
    /// Capture current cursor position and visual row.
    pub fn capture(tree: &TreeState, scroll: &ScrollState) -> Self {
        Self { path: tree.cursor_path(), visual_row: tree.cursor().saturating_sub(scroll.offset()) }
    }

    /// Restore cursor to the saved path and adjust scroll to preserve visual row.
    ///
    /// When the saved path no longer exists (e.g. the file was deleted), the
    /// cursor is clamped to the current visible range so it stays at the same
    /// index position — effectively selecting the next item, or the last item
    /// if the deleted file was at the end.
    ///
    pub fn restore(&self, tree: &mut TreeState, scroll: &mut ScrollState, viewport_height: usize) {
        let found = self.path.as_ref().is_some_and(|p| tree.move_cursor_to_path(p));
        if !found {
            // Clamp cursor to valid range (same index → next item, or last).
            tree.move_cursor_to(tree.cursor());
        }
        let cursor = tree.cursor();
        scroll.set_offset(cursor.saturating_sub(self.visual_row));
        scroll.clamp_to_cursor(cursor, viewport_height);
        scroll.clamp_to_total(tree.visible_node_count(), viewport_height);
    }
}

/// Immutable runtime context shared across all handlers.
///
/// Contains channel senders, configuration, and key mappings
/// that do not change after initialization.
#[derive(Debug)]
pub struct AppContext {
    /// Sender for async directory children load results.
    pub children_tx: tokio::sync::mpsc::Sender<ChildrenLoadResult>,
    /// Sender for async preview load results.
    pub preview_tx: tokio::sync::mpsc::Sender<PreviewLoadResult>,
    /// Preview configuration.
    pub preview_config: PreviewConfig,
    /// File operation configuration.
    pub file_op_config: FileOpConfig,
    /// Key-to-action mapping.
    pub keymap: KeyMap,
    /// Cached action-to-key-display lookup (built once from keymap).
    pub action_key_lookup: ActionKeyLookup,
    /// Shared suppression flag for file system watcher.
    pub suppressed: Arc<AtomicBool>,
    /// IPC server handle (None if not in daemon mode).
    pub ipc_server: Option<Arc<crate::ipc::server::IpcServer>>,
    /// Sender for async git status results.
    pub git_tx: tokio::sync::mpsc::Sender<GitStatusResult>,
    /// Whether git integration is enabled.
    pub git_enabled: bool,
    /// Absolute path to the workspace root (for git operations).
    pub root_path: PathBuf,
    /// Sender for async tree rebuild results.
    pub rebuild_tx: tokio::sync::mpsc::Sender<TreeRebuildResult>,
    /// User-defined menu definitions (from config).
    pub menus: HashMap<String, MenuDefinition>,
    /// Shared search index (built in background).
    pub search_index: Arc<RwLock<crate::tree::search_index::SearchIndex>>,
    /// Sender to notify when a search index build completes.
    pub search_index_ready_tx: tokio::sync::mpsc::Sender<()>,
    /// Sender for async stat batch results.
    pub stat_tx: tokio::sync::mpsc::Sender<StatLoadResult>,
}

/// Kind of async directory children load operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadKind {
    /// User-initiated expand (triggers prefetch after load).
    UserExpand,
    /// Background prefetch (one-level-ahead, does not trigger further prefetch).
    Prefetch,
    /// Deferred session restore (cursor-prioritized, adjusts scroll to preserve visual row).
    DeferredRestore,
    /// Search filter load (progressive ancestor loading during search).
    SearchFilter,
}

/// Result of an async directory children load operation.
///
/// Sent through an mpsc channel from the blocking task to the event loop.
#[derive(Debug)]
pub struct ChildrenLoadResult {
    /// Path of the directory whose children were loaded.
    pub path: PathBuf,
    /// Loaded children, or an error message.
    pub children: Result<Vec<TreeNode>, String>,
    /// What kind of load operation produced this result.
    pub kind: LoadKind,
}

/// Result of an async stat batch operation.
///
/// Sent through an mpsc channel from the blocking task to the event loop.
#[derive(Debug)]
pub struct StatLoadResult {
    /// Directory whose children's metadata was fetched.
    pub dir_path: PathBuf,
    /// Per-file metadata: `(file_path, size, modified)`.
    pub entries: Vec<(PathBuf, u64, Option<std::time::SystemTime>)>,
}

/// Deferred session restore: expanded directories to load asynchronously after first render.
///
/// Paths are sorted by cursor proximity (common prefix length descending, then depth ascending)
/// so that directories near the cursor are restored first.
#[derive(Debug)]
pub struct DeferredExpansion {
    /// Paths remaining to be loaded, sorted by priority.
    remaining: Vec<PathBuf>,
    /// Number of currently in-flight async loads.
    in_flight: usize,
    /// Maximum number of concurrent async loads.
    max_concurrent: usize,
}

impl DeferredExpansion {
    /// Maximum concurrent deferred loads.
    const DEFAULT_MAX_CONCURRENT: usize = 8;

    /// Create a new deferred expansion queue.
    ///
    /// Paths are sorted by priority: longest common prefix with `cursor_path` first,
    /// then shallowest depth first (to resolve parent-child dependencies naturally).
    pub fn new(mut paths: Vec<PathBuf>, cursor_path: &Path) -> Self {
        paths.sort_by(|a, b| {
            let a_common = common_prefix_len(a, cursor_path);
            let b_common = common_prefix_len(b, cursor_path);
            // Higher common prefix = higher priority (sort first).
            b_common.cmp(&a_common).then_with(|| {
                // Shallower depth = higher priority for same common prefix.
                a.components().count().cmp(&b.components().count())
            })
        });
        Self { remaining: paths, in_flight: 0, max_concurrent: Self::DEFAULT_MAX_CONCURRENT }
    }

    /// Try to schedule the next batch of loads.
    ///
    /// Returns paths that were transitioned to `Loading` state.
    /// Only schedules paths whose parent directory is already loaded in the tree.
    pub fn schedule(&mut self, tree: &mut TreeState) -> Vec<PathBuf> {
        let mut scheduled = Vec::new();
        let mut still_remaining = Vec::new();

        for path in self.remaining.drain(..) {
            if self.in_flight + scheduled.len() >= self.max_concurrent {
                still_remaining.push(path);
                continue;
            }
            if tree.prepare_async_load(&path, true).is_some() {
                scheduled.push(path);
            } else {
                still_remaining.push(path);
            }
        }

        self.in_flight += scheduled.len();
        self.remaining = still_remaining;
        scheduled
    }

    /// Record that one in-flight load has completed.
    pub const fn on_load_complete(&mut self) {
        self.in_flight = self.in_flight.saturating_sub(1);
    }

    /// Whether all deferred loads have completed and no paths remain.
    pub const fn is_done(&self) -> bool {
        self.remaining.is_empty() && self.in_flight == 0
    }

    /// Whether we are stuck: no in-flight loads and remaining paths cannot be scheduled.
    ///
    /// This happens when remaining paths have parents that are not loaded (e.g., deleted
    /// since the session was saved). In this case, give up on the remaining paths.
    pub const fn is_stuck(&self) -> bool {
        self.in_flight == 0 && !self.remaining.is_empty()
    }

    /// Paths that are still waiting to be loaded.
    ///
    /// Used at shutdown to include not-yet-scheduled paths in the session save,
    /// so they are not lost if the user quits before deferred expansion completes.
    pub fn remaining_paths(&self) -> &[PathBuf] {
        &self.remaining
    }
}

/// Count the number of leading path components shared between two paths.
fn common_prefix_len(a: &Path, b: &Path) -> usize {
    a.components().zip(b.components()).take_while(|(ca, cb)| ca == cb).count()
}

/// Result of an async tree rebuild operation.
///
/// Sent through an mpsc channel from the blocking task to the event loop.
/// Used by `ToggleHidden`, `ToggleIgnored`, and `Refresh` to avoid blocking
/// the UI during tree reconstruction.
#[derive(Debug)]
pub struct TreeRebuildResult {
    /// The rebuilt tree state (ready to swap in).
    pub tree_state: TreeState,
    /// Root path (for triggering prefetch after swap).
    pub root_path: PathBuf,
    /// Whether hidden files are shown (needed for post-swap prefetch).
    pub show_hidden: bool,
    /// Whether ignored files are shown (needed for post-swap prefetch).
    pub show_ignored: bool,
    /// Generation counter to discard stale results from superseded rebuilds.
    pub generation: u64,
    /// Cursor's visual row (cursor - scroll offset) before the rebuild.
    ///
    /// Used to restore the scroll position so the cursor stays at the same
    /// screen row after the tree is swapped in.
    pub visual_row: usize,
    /// Original cursor path before the rebuild.
    ///
    /// Used by `reapply_search` to restore cursor position after the search
    /// filter is re-applied, since the rebuilt tree may have fallen back to a
    /// different node if the original was filtered out.
    pub cursor_path: Option<PathBuf>,
}

/// Result of an async preview load operation.
#[derive(Debug)]
pub struct PreviewLoadResult {
    /// Path of the file that was previewed.
    pub path: PathBuf,
    /// Provider name that produced this content.
    pub provider_name: String,
    /// Loaded preview content.
    pub content: PreviewContent,
    /// Whether this was a background prefetch (cache-only, not for display).
    pub prefetch: bool,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::needless_pub_self)]
pub(super) mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    /// Create a minimal `AppState` for unit testing.
    ///
    /// Uses an empty root node with no children. Suitable for testing
    /// handler logic that doesn't need a real filesystem tree.
    pub fn minimal_app_state() -> AppState {
        use crate::config::CategoryStyles;
        use crate::file_op::selection::SelectionBuffer;
        use crate::file_op::undo::UndoHistory;
        use crate::preview::cache::PreviewCache;
        use crate::preview::provider::PreviewRegistry;
        use crate::preview::providers::fallback::FallbackProvider;
        use crate::preview::state::PreviewState;
        use crate::state::tree::{
            ChildrenState,
            TreeNode,
            TreeOptions,
            TreeState,
        };
        use crate::ui::column::{
            ColumnOptionsConfig,
            default_columns,
            resolve_columns,
        };
        use crate::ui::file_style::FileStyleMatcher;

        let root_node = TreeNode {
            name: "root".to_string(),
            path: PathBuf::from("/test/root"),
            is_dir: true,
            is_symlink: false,
            symlink_target: None,
            size: 0,
            modified: None,
            recursive_max_mtime: None,
            children: ChildrenState::Loaded(vec![]),
            is_expanded: true,
            is_ignored: false,
            is_root: true,
        };
        let tree_state = TreeState::new(root_node, TreeOptions::default());
        let registry = PreviewRegistry::new(vec![Arc::new(FallbackProvider::new())]).unwrap();

        AppState {
            tree_state,
            preview_state: PreviewState::new(),
            preview_cache: PreviewCache::new(10),
            preview_registry: registry,
            mode: AppMode::default(),
            selection: SelectionBuffer::new(),
            undo_history: UndoHistory::new(10),
            watcher: None,
            should_quit: false,
            show_icons: false,
            show_preview: false,
            show_hidden: true,
            show_ignored: true,
            viewport_height: 20,
            scroll: ScrollState::new(),
            status_message: None,
            processing: false,
            git_state: Arc::new(RwLock::new(None)),
            rebuild_generation: 0,
            columns: resolve_columns(&default_columns(), &ColumnOptionsConfig::default()),
            layout_split_ratio: 50,
            layout_narrow_split_ratio: 60,
            layout_narrow_width: 80,
            pending_keys: PendingKeys::new(Duration::from_millis(500)),
            needs_redraw: false,
            dirty: true,
            file_style_matcher: FileStyleMatcher::new(&[], &CategoryStyles::default()).unwrap(),
            preview_debounce: None,
            layout_areas: LayoutAreas::default(),
            deferred_expansion: None,
            search_history: vec![],
            search_match_indices: HashMap::new(),
            search_pending_loads: None,
            search_index_cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    #[rstest]
    fn scroll_state_clamp_cursor_below_viewport() {
        let mut scroll = ScrollState::new();
        // Cursor at 15, viewport 10 → offset should move to 6.
        scroll.clamp_to_cursor(15, 10);
        assert_that!(scroll.offset(), eq(6));
    }

    #[rstest]
    fn scroll_state_clamp_cursor_above_viewport() {
        let mut scroll = ScrollState { offset: 10 };
        // Cursor at 5, viewport 10 → offset should move to 5.
        scroll.clamp_to_cursor(5, 10);
        assert_that!(scroll.offset(), eq(5));
    }

    #[rstest]
    fn scroll_state_clamp_cursor_within_viewport() {
        let mut scroll = ScrollState { offset: 5 };
        // Cursor at 8, viewport 10 → offset stays at 5.
        scroll.clamp_to_cursor(8, 10);
        assert_that!(scroll.offset(), eq(5));
    }

    #[rstest]
    fn scroll_state_clamp_zero_viewport() {
        let mut scroll = ScrollState { offset: 5 };
        scroll.clamp_to_cursor(3, 0);
        assert_that!(scroll.offset(), eq(0));
    }

    #[rstest]
    fn scroll_state_center_on_cursor() {
        let mut scroll = ScrollState::new();
        // Cursor at 20, viewport 10 → offset = 20 - 5 = 15.
        scroll.center_on_cursor(20, 10);
        assert_that!(scroll.offset(), eq(15));
    }

    #[rstest]
    fn scroll_state_center_on_cursor_near_top() {
        let mut scroll = ScrollState::new();
        // Cursor at 2, viewport 10 → offset = 0 (saturating_sub).
        scroll.center_on_cursor(2, 10);
        assert_that!(scroll.offset(), eq(0));
    }

    #[rstest]
    fn scroll_state_center_on_cursor_zero_viewport() {
        let mut scroll = ScrollState { offset: 5 };
        scroll.center_on_cursor(10, 0);
        assert_that!(scroll.offset(), eq(0));
    }

    #[rstest]
    fn scroll_cursor_to_top_sets_offset_to_cursor() {
        let mut scroll = ScrollState::new();
        scroll.scroll_cursor_to_top(15);
        assert_that!(scroll.offset(), eq(15));
    }

    #[rstest]
    fn scroll_cursor_to_top_at_zero() {
        let mut scroll = ScrollState { offset: 10 };
        scroll.scroll_cursor_to_top(0);
        assert_that!(scroll.offset(), eq(0));
    }

    #[rstest]
    fn scroll_cursor_to_bottom_normal() {
        let mut scroll = ScrollState::new();
        // Cursor at 20, viewport 10 → offset = 20 - 9 = 11.
        scroll.scroll_cursor_to_bottom(20, 10);
        assert_that!(scroll.offset(), eq(11));
    }

    #[rstest]
    fn scroll_cursor_to_bottom_near_top() {
        let mut scroll = ScrollState::new();
        // Cursor at 3, viewport 10 → offset = 0 (saturating_sub).
        scroll.scroll_cursor_to_bottom(3, 10);
        assert_that!(scroll.offset(), eq(0));
    }

    #[rstest]
    fn scroll_cursor_to_bottom_zero_viewport() {
        let mut scroll = ScrollState { offset: 5 };
        scroll.scroll_cursor_to_bottom(10, 0);
        assert_that!(scroll.offset(), eq(0));
    }

    #[rstest]
    fn scroll_state_set_offset() {
        let mut scroll = ScrollState::new();
        scroll.set_offset(42);
        assert_that!(scroll.offset(), eq(42));
    }

    #[rstest]
    fn scroll_visual_row_preserved_after_rebuild() {
        // Simulate: cursor=30, offset=20, visual_row=10
        // After rebuild: new_cursor=35 (hidden files inserted above)
        // Expected: offset=25 so visual_row stays 10
        let mut scroll = ScrollState { offset: 20 };
        let visual_row = 30_usize.saturating_sub(scroll.offset()); // 10
        let new_cursor: usize = 35;
        let total: usize = 100;
        let viewport_height: usize = 40;
        let max_offset = total.saturating_sub(viewport_height);
        let desired = new_cursor.saturating_sub(visual_row);
        scroll.set_offset(desired.min(max_offset));

        assert_that!(scroll.offset(), eq(25));
        assert_that!(new_cursor - scroll.offset(), eq(visual_row));
    }

    #[rstest]
    fn scroll_clamps_to_tree_end_when_shrinking() {
        // Simulate: cursor=80, offset=70, visual_row=10, viewport=20
        // After rebuild: tree shrinks to 50 nodes, cursor at 40
        // max_offset = 50-20 = 30, desired = 40-10 = 30
        // offset should be 30 (not 30+), cursor at row 10
        let mut scroll = ScrollState { offset: 70 };
        let visual_row = 80_usize.saturating_sub(scroll.offset()); // 10
        let new_cursor: usize = 40;
        let total: usize = 50;
        let viewport_height: usize = 20;
        let max_offset = total.saturating_sub(viewport_height);
        let desired = new_cursor.saturating_sub(visual_row);
        scroll.set_offset(desired.min(max_offset));

        assert_that!(scroll.offset(), eq(30));
    }

    #[rstest]
    fn scroll_clamps_when_tree_smaller_than_viewport() {
        // Tree has 10 nodes, viewport is 40.
        // visual_row=5, new_cursor=8
        // max_offset = 0 (10-40 saturating), desired = 8-5 = 3
        // offset should be 0 (can't scroll in a small tree)
        let scroll_offset = 0;
        let visual_row = 5_usize.saturating_sub(scroll_offset);
        let new_cursor: usize = 8;
        let total: usize = 10;
        let viewport_height: usize = 40;
        let max_offset = total.saturating_sub(viewport_height);
        let desired = new_cursor.saturating_sub(visual_row);
        let mut scroll = ScrollState { offset: scroll_offset };
        scroll.set_offset(desired.min(max_offset));

        assert_that!(scroll.offset(), eq(0));
    }

    #[rstest]
    fn clamp_to_total_caps_offset_when_over_scrolled() {
        let mut scroll = ScrollState { offset: 15 };
        scroll.clamp_to_total(20, 10);
        assert_that!(scroll.offset(), eq(10));
    }

    #[rstest]
    fn clamp_to_total_noop_when_within_bounds() {
        let mut scroll = ScrollState { offset: 5 };
        scroll.clamp_to_total(20, 10);
        assert_that!(scroll.offset(), eq(5));
    }

    #[rstest]
    fn clamp_to_total_forces_zero_when_all_fit() {
        let mut scroll = ScrollState { offset: 3 };
        scroll.clamp_to_total(5, 10);
        assert_that!(scroll.offset(), eq(0));
    }

    #[rstest]
    fn clamp_to_total_zero_items() {
        let mut scroll = ScrollState { offset: 5 };
        scroll.clamp_to_total(0, 10);
        assert_that!(scroll.offset(), eq(0));
    }

    #[rstest]
    fn clamp_to_total_exact_boundary() {
        let mut scroll = ScrollState { offset: 10 };
        scroll.clamp_to_total(20, 10);
        assert_that!(scroll.offset(), eq(10));
    }

    #[rstest]
    fn clamp_to_cursor_then_total_prevents_over_scroll() {
        // cursor=9, viewport=10, total=10 → max_offset=0.
        // clamp_to_cursor keeps offset=5 (cursor 9 in [5..15)),
        // but clamp_to_total should cap to 0.
        let mut scroll = ScrollState { offset: 5 };
        scroll.clamp_to_cursor(9, 10);
        assert_that!(scroll.offset(), eq(5));
        scroll.clamp_to_total(10, 10);
        assert_that!(scroll.offset(), eq(0));
    }
}
