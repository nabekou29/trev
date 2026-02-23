//! Application state, context, and result types.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{
    Duration,
    Instant,
};

use super::keymap::KeyMap;
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
    /// Emit mode: accumulated file paths for stdout output on exit.
    /// `Some(vec)` when `--emit` is active, `None` otherwise.
    pub emit_paths: Option<Vec<PathBuf>>,
    /// Git repository status (None when git is disabled or outside a repo).
    ///
    /// Shared via `Arc<RwLock<…>>` so `ExternalCmdProvider` instances can read
    /// the current status in `can_handle()` without borrowing `AppState`.
    pub git_state: Arc<std::sync::RwLock<Option<GitState>>>,
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
}

impl AppState {
    /// Set a temporary status message (auto-clears after 3 seconds).
    pub fn set_status(&mut self, text: impl Into<String>) {
        self.status_message = Some(StatusMessage::new(text));
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
    /// Shared suppression flag for file system watcher.
    pub suppressed: Arc<AtomicBool>,
    /// IPC server handle (None if not in daemon mode).
    pub ipc_server: Option<Arc<crate::ipc::server::IpcServer>>,
    /// Default editor action for opening files (from `--action` flag).
    pub editor_action: crate::ipc::types::EditorAction,
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
    /// Whether this was a background prefetch (one-level-ahead load).
    pub prefetch: bool,
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
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

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
}
