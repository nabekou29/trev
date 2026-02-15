//! File system change detection using notify with debouncing.
//!
//! Provides [`FsWatcher`] which monitors expanded directories for external changes
//! and sends debounced events to the main event loop.

use std::collections::HashSet;
use std::path::{
    Path,
    PathBuf,
};
use std::sync::Arc;
use std::sync::atomic::{
    AtomicBool,
    Ordering,
};
use std::time::Duration;

use anyhow::Result;
use notify::RecursiveMode;
use notify_debouncer_mini::{
    DebounceEventResult,
    DebouncedEvent,
    Debouncer,
    new_debouncer,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::config::WatcherConfig;

/// File system change watcher with debouncing and self-operation suppression.
pub struct FsWatcher {
    /// Debounced file watcher (kept alive for the watcher lifetime).
    debouncer: Debouncer<notify::RecommendedWatcher>,
    /// Set of currently watched directory paths.
    watched_dirs: HashSet<PathBuf>,
    /// Flag to suppress events during self-initiated file operations.
    suppressed: Arc<AtomicBool>,
}

impl std::fmt::Debug for FsWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FsWatcher")
            .field("watched_dirs", &self.watched_dirs)
            .field("suppressed", &self.suppressed)
            .finish_non_exhaustive()
    }
}

impl FsWatcher {
    /// Create a new file system watcher.
    ///
    /// Events are sent through `tx` after debouncing.
    /// Self-operation suppression is applied before sending.
    pub fn new(config: &WatcherConfig, tx: UnboundedSender<Vec<DebouncedEvent>>) -> Result<Self> {
        let suppressed = Arc::new(AtomicBool::new(false));
        let suppressed_clone = suppressed.clone();

        let debouncer = new_debouncer(
            Duration::from_millis(config.debounce_ms),
            move |result: DebounceEventResult| {
                if suppressed_clone.load(Ordering::SeqCst) {
                    return;
                }
                if let Ok(events) = result {
                    let _ = tx.send(events);
                }
            },
        )?;

        Ok(Self { debouncer, watched_dirs: HashSet::new(), suppressed })
    }

    /// Start watching a directory (`NonRecursive`).
    ///
    /// Idempotent: watching an already-watched directory is a no-op.
    pub fn watch(&mut self, path: &Path) -> Result<()> {
        if self.watched_dirs.contains(path) {
            return Ok(());
        }
        self.debouncer.watcher().watch(path, RecursiveMode::NonRecursive)?;
        self.watched_dirs.insert(path.to_path_buf());
        Ok(())
    }

    /// Stop watching a directory.
    ///
    /// Idempotent: unwatching a non-watched directory is a no-op.
    pub fn unwatch(&mut self, path: &Path) -> Result<()> {
        if !self.watched_dirs.remove(path) {
            return Ok(());
        }
        self.debouncer.watcher().unwatch(path)?;
        Ok(())
    }

    /// Get a clone of the suppression flag for sharing with file operation handlers.
    pub fn suppressed(&self) -> Arc<AtomicBool> {
        self.suppressed.clone()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    fn create_test_watcher() -> FsWatcher {
        let config = WatcherConfig::default();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        FsWatcher::new(&config, tx).unwrap()
    }

    #[rstest]
    fn watch_adds_to_tracked_dirs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut watcher = create_test_watcher();
        watcher.watch(tmp.path()).unwrap();
        assert_that!(watcher.watched_dirs.contains(tmp.path()), eq(true));
    }

    #[rstest]
    fn double_watch_is_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut watcher = create_test_watcher();
        watcher.watch(tmp.path()).unwrap();
        watcher.watch(tmp.path()).unwrap();
        assert_that!(watcher.watched_dirs.len(), eq(1));
    }

    #[rstest]
    fn unwatch_removes_from_tracked_dirs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut watcher = create_test_watcher();
        watcher.watch(tmp.path()).unwrap();
        watcher.unwatch(tmp.path()).unwrap();
        assert_that!(watcher.watched_dirs.contains(tmp.path()), eq(false));
    }

    #[rstest]
    fn unwatch_nonexistent_is_noop() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut watcher = create_test_watcher();
        // Should not error on unwatching a non-watched path.
        watcher.unwatch(tmp.path()).unwrap();
    }

    #[rstest]
    fn suppressed_flag_is_shared() {
        let watcher = create_test_watcher();
        let flag = watcher.suppressed();
        assert_that!(flag.load(Ordering::SeqCst), eq(false));
        flag.store(true, Ordering::SeqCst);
        assert_that!(watcher.suppressed.load(Ordering::SeqCst), eq(true));
    }

    #[rstest]
    fn watch_multiple_dirs() {
        let tmp1 = tempfile::TempDir::new().unwrap();
        let tmp2 = tempfile::TempDir::new().unwrap();
        let mut watcher = create_test_watcher();
        watcher.watch(tmp1.path()).unwrap();
        watcher.watch(tmp2.path()).unwrap();
        assert_that!(watcher.watched_dirs.len(), eq(2));

        watcher.unwatch(tmp1.path()).unwrap();
        assert_that!(watcher.watched_dirs.len(), eq(1));
        assert_that!(watcher.watched_dirs.contains(tmp2.path()), eq(true));
    }
}
