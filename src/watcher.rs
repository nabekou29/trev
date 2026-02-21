//! File system change detection using notify with debouncing.
//!
//! Provides [`FsWatcher`] which monitors expanded directories for external changes
//! and sends debounced events to the main event loop.

use std::path::Path;
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

/// RAII guard that sets a shared `AtomicBool` to `true` on creation
/// and resets it to `false` on drop.
///
/// Used to suppress file system watcher events during self-initiated
/// file operations (create, delete, rename, paste, undo, redo).
#[derive(Debug)]
pub struct SuppressGuard(Arc<AtomicBool>);

impl SuppressGuard {
    /// Activate suppression. The flag is set to `true` immediately.
    pub fn new(flag: &Arc<AtomicBool>) -> Self {
        flag.store(true, Ordering::SeqCst);
        Self(Arc::clone(flag))
    }
}

impl Drop for SuppressGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// File system change watcher with debouncing and self-operation suppression.
///
/// Uses a single recursive watch on the workspace root (`FSEvents` on macOS)
/// instead of per-directory watches. This eliminates the overhead of
/// registering/unregistering hundreds of watches during expand-all.
pub struct FsWatcher {
    /// Debounced file watcher (kept alive for the watcher lifetime).
    debouncer: Debouncer<notify::RecommendedWatcher>,
    /// Flag to suppress events during self-initiated file operations.
    suppressed: Arc<AtomicBool>,
}

impl std::fmt::Debug for FsWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FsWatcher")
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

        Ok(Self { debouncer, suppressed })
    }

    /// Start watching the workspace root recursively.
    ///
    /// Should be called once at startup. Uses `RecursiveMode::Recursive`
    /// which maps to a single `FSEvents` stream on macOS.
    pub fn watch_root(&mut self, path: &Path) -> Result<()> {
        self.debouncer.watcher().watch(path, RecursiveMode::Recursive)?;
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
    fn watch_root_succeeds() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut watcher = create_test_watcher();
        watcher.watch_root(tmp.path()).unwrap();
    }

    #[rstest]
    fn suppressed_flag_is_shared() {
        let watcher = create_test_watcher();
        let flag = watcher.suppressed();
        assert_that!(flag.load(Ordering::SeqCst), eq(false));
        flag.store(true, Ordering::SeqCst);
        assert_that!(watcher.suppressed.load(Ordering::SeqCst), eq(true));
    }
}
