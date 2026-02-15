//! Unified selection buffer for mark, copy, and cut operations.

use std::collections::HashSet;
use std::path::{
    Path,
    PathBuf,
};

use serde::{
    Deserialize,
    Serialize,
};

/// Selection mode: determines how selected paths will be used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionMode {
    /// Marked for batch operations (delete, etc.).
    Mark,
    /// Copied — originals remain after paste.
    Copy,
    /// Cut — originals removed after paste.
    Cut,
}

/// Unified buffer holding selected file paths and their operation mode.
///
/// Replaces the previous separate `MarkSet` and `YankBuffer` types.
/// Only one mode is active at a time; switching mode preserves paths.
#[derive(Debug, Clone, Default)]
pub struct SelectionBuffer {
    /// Selected file paths.
    paths: HashSet<PathBuf>,
    /// Current selection mode (`None` when buffer is empty).
    mode: Option<SelectionMode>,
}

impl SelectionBuffer {
    /// Create a new empty selection buffer.
    #[must_use]
    pub fn new() -> Self {
        Self { paths: HashSet::new(), mode: None }
    }

    /// Toggle a path in Mark mode.
    ///
    /// If the current mode is not Mark, switches to Mark mode while
    /// preserving existing paths. Then toggles the given path.
    pub fn toggle_mark(&mut self, path: PathBuf) {
        // Switch to Mark mode if needed (paths preserved).
        if self.mode.as_ref() != Some(&SelectionMode::Mark) {
            self.mode = Some(SelectionMode::Mark);
        }

        if self.paths.contains(&path) {
            self.paths.remove(&path);
            if self.paths.is_empty() {
                self.mode = None;
            }
        } else {
            self.paths.insert(path);
        }
    }

    /// Set the buffer with the given paths and mode.
    pub fn set(&mut self, paths: Vec<PathBuf>, mode: SelectionMode) {
        self.paths = paths.into_iter().collect();
        self.mode = Some(mode);
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.paths.clear();
        self.mode = None;
    }

    /// Whether the buffer is empty.
    #[allow(dead_code, reason = "API completeness — will be used as selection buffer grows")]
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Number of selected paths.
    pub fn count(&self) -> usize {
        self.paths.len()
    }

    /// Get the current selection mode.
    pub const fn mode(&self) -> Option<&SelectionMode> {
        self.mode.as_ref()
    }

    /// Get the selected paths.
    #[allow(dead_code, reason = "API completeness — direct access for future use")]
    pub const fn paths(&self) -> &HashSet<PathBuf> {
        &self.paths
    }

    /// Check if a specific path is selected.
    pub fn contains(&self, path: &Path) -> bool {
        self.paths.contains(path)
    }

    /// Export the buffer contents for serialization.
    pub fn export(&self) -> (Vec<PathBuf>, Option<&SelectionMode>) {
        let mut paths: Vec<PathBuf> = self.paths.iter().cloned().collect();
        paths.sort();
        (paths, self.mode.as_ref())
    }

    /// Reconstruct a `SelectionBuffer` from previously exported parts.
    pub fn from_parts(paths: Vec<PathBuf>, mode: Option<SelectionMode>) -> Self {
        Self { paths: paths.into_iter().collect(), mode }
    }

    /// Get selected paths with children removed when a parent is also selected.
    ///
    /// If both `/a` and `/a/b` are selected, only `/a` is returned.
    #[must_use]
    pub fn deduplicated_paths(&self) -> Vec<PathBuf> {
        let mut sorted: Vec<&PathBuf> = self.paths.iter().collect();
        sorted.sort();

        let mut result: Vec<PathBuf> = Vec::new();
        for path in sorted {
            let has_ancestor = result.iter().any(|accepted| path.starts_with(accepted));
            if !has_ancestor {
                result.push(path.clone());
            }
        }
        result
    }

    /// Get mark targets or fall back to cursor path.
    ///
    /// Returns deduplicated selected paths only when in Mark mode.
    /// Otherwise returns just the cursor path.
    #[must_use]
    pub fn mark_targets_or_cursor(&self, cursor_path: &Path) -> Vec<PathBuf> {
        if self.mode.as_ref() == Some(&SelectionMode::Mark) && !self.paths.is_empty() {
            self.deduplicated_paths()
        } else {
            vec![cursor_path.to_path_buf()]
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    #[rstest]
    fn new_buffer_is_empty() {
        let buf = SelectionBuffer::new();
        assert_that!(buf.is_empty(), eq(true));
        assert_that!(buf.count(), eq(0));
        assert_that!(buf.mode(), none());
    }

    #[rstest]
    fn toggle_mark_adds_path() {
        let mut buf = SelectionBuffer::new();
        buf.toggle_mark(PathBuf::from("/a"));
        assert_that!(buf.count(), eq(1));
        assert_that!(buf.contains(Path::new("/a")), eq(true));
        assert_that!(buf.mode(), some(eq(&SelectionMode::Mark)));
    }

    #[rstest]
    fn toggle_mark_removes_path() {
        let mut buf = SelectionBuffer::new();
        buf.toggle_mark(PathBuf::from("/a"));
        buf.toggle_mark(PathBuf::from("/a"));
        assert_that!(buf.is_empty(), eq(true));
        assert_that!(buf.mode(), none());
    }

    #[rstest]
    fn toggle_mark_preserves_copy_paths() {
        let mut buf = SelectionBuffer::new();
        buf.set(vec![PathBuf::from("/a"), PathBuf::from("/b")], SelectionMode::Copy);
        // Switch to Mark mode via toggle — paths a,b preserved, c added.
        buf.toggle_mark(PathBuf::from("/c"));
        assert_that!(buf.count(), eq(3));
        assert_that!(buf.contains(Path::new("/a")), eq(true));
        assert_that!(buf.contains(Path::new("/b")), eq(true));
        assert_that!(buf.contains(Path::new("/c")), eq(true));
        assert_that!(buf.mode(), some(eq(&SelectionMode::Mark)));
    }

    #[rstest]
    fn set_replaces_buffer() {
        let mut buf = SelectionBuffer::new();
        buf.toggle_mark(PathBuf::from("/old"));
        buf.set(vec![PathBuf::from("/new1"), PathBuf::from("/new2")], SelectionMode::Cut);
        assert_that!(buf.count(), eq(2));
        assert_that!(buf.mode(), some(eq(&SelectionMode::Cut)));
        assert_that!(buf.contains(Path::new("/old")), eq(false));
    }

    #[rstest]
    fn clear_empties_buffer() {
        let mut buf = SelectionBuffer::new();
        buf.set(vec![PathBuf::from("/a")], SelectionMode::Copy);
        buf.clear();
        assert_that!(buf.is_empty(), eq(true));
        assert_that!(buf.mode(), none());
    }

    #[rstest]
    fn mark_targets_or_cursor_returns_marks() {
        let mut buf = SelectionBuffer::new();
        buf.toggle_mark(PathBuf::from("/a"));
        buf.toggle_mark(PathBuf::from("/b"));
        let mut targets = buf.mark_targets_or_cursor(Path::new("/cursor"));
        targets.sort();
        assert_eq!(targets, vec![PathBuf::from("/a"), PathBuf::from("/b")]);
    }

    #[rstest]
    fn mark_targets_or_cursor_returns_cursor_when_copy_mode() {
        let mut buf = SelectionBuffer::new();
        buf.set(vec![PathBuf::from("/a")], SelectionMode::Copy);
        let targets = buf.mark_targets_or_cursor(Path::new("/cursor"));
        assert_eq!(targets, vec![PathBuf::from("/cursor")]);
    }

    #[rstest]
    fn mark_targets_or_cursor_returns_cursor_when_empty() {
        let buf = SelectionBuffer::new();
        let targets = buf.mark_targets_or_cursor(Path::new("/cursor"));
        assert_eq!(targets, vec![PathBuf::from("/cursor")]);
    }

    #[rstest]
    fn deduplicated_paths_removes_children() {
        let mut buf = SelectionBuffer::new();
        buf.toggle_mark(PathBuf::from("/a"));
        buf.toggle_mark(PathBuf::from("/a/a-1"));
        buf.toggle_mark(PathBuf::from("/a/a-2"));
        let mut result = buf.deduplicated_paths();
        result.sort();
        assert_eq!(result, vec![PathBuf::from("/a")]);
    }

    #[rstest]
    fn deduplicated_paths_keeps_siblings() {
        let mut buf = SelectionBuffer::new();
        buf.toggle_mark(PathBuf::from("/a/a-1"));
        buf.toggle_mark(PathBuf::from("/b/b-1"));
        let mut result = buf.deduplicated_paths();
        result.sort();
        assert_eq!(result, vec![PathBuf::from("/a/a-1"), PathBuf::from("/b/b-1")]);
    }

    #[rstest]
    fn deduplicated_paths_deep_nesting() {
        let mut buf = SelectionBuffer::new();
        buf.toggle_mark(PathBuf::from("/a"));
        buf.toggle_mark(PathBuf::from("/a/b/c/d"));
        let result = buf.deduplicated_paths();
        assert_eq!(result, vec![PathBuf::from("/a")]);
    }

    #[rstest]
    fn export_and_from_parts_roundtrip() {
        let mut buf = SelectionBuffer::new();
        buf.set(vec![PathBuf::from("/b"), PathBuf::from("/a")], SelectionMode::Copy);

        let (paths, mode) = buf.export();
        // Paths are sorted by export.
        assert_eq!(paths, vec![PathBuf::from("/a"), PathBuf::from("/b")]);
        assert_eq!(mode, Some(&SelectionMode::Copy));

        // Reconstruct.
        let restored = SelectionBuffer::from_parts(paths, mode.cloned());
        assert_that!(restored.count(), eq(2));
        assert_that!(restored.contains(Path::new("/a")), eq(true));
        assert_that!(restored.contains(Path::new("/b")), eq(true));
        assert_that!(restored.mode(), some(eq(&SelectionMode::Copy)));
    }

    #[rstest]
    fn export_empty_buffer() {
        let buf = SelectionBuffer::new();
        let (paths, mode) = buf.export();
        assert_that!(paths.len(), eq(0));
        assert_that!(mode, none());
    }
}
