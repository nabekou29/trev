//! Application state, context, and result types.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{
    Duration,
    Instant,
};

use super::keymap::KeyMap;
use crate::config::{
    FileOpConfig,
    PreviewConfig,
};
use crate::file_op::selection::SelectionBuffer;
use crate::file_op::undo::UndoHistory;
use crate::input::AppMode;
use crate::preview::cache::PreviewCache;
use crate::preview::content::PreviewContent;
use crate::preview::provider::PreviewRegistry;
use crate::preview::state::PreviewState;
use crate::state::tree::{
    TreeNode,
    TreeState,
};
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
    #[expect(dead_code, reason = "Cache integration pending")]
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
    pub viewport_height: u16,
    /// Scroll state for the tree view.
    pub scroll: ScrollState,
    /// Temporary status message displayed in the status bar.
    pub status_message: Option<StatusMessage>,
    /// Whether a blocking file operation is in progress.
    pub processing: bool,
}

impl AppState {
    /// Set a temporary status message (auto-clears after 3 seconds).
    pub fn set_status(&mut self, text: impl Into<String>) {
        self.status_message = Some(StatusMessage::new(text));
    }

    /// Clear the status message if it has expired.
    pub fn clear_expired_status(&mut self) {
        if self.status_message.as_ref().is_some_and(StatusMessage::is_expired) {
            self.status_message = None;
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
}

/// Scroll position management for the tree view.
#[derive(Debug)]
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

/// Result of an async preview load operation.
pub struct PreviewLoadResult {
    /// Path of the file that was previewed.
    pub path: PathBuf,
    /// Provider name that produced this content.
    #[expect(dead_code, reason = "Used for cache key integration")]
    pub provider_name: String,
    /// Loaded preview content.
    pub content: PreviewContent,
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
}
