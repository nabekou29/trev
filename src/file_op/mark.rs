//! Mark set for multi-file selection.

use std::collections::HashSet;
use std::path::{
    Path,
    PathBuf,
};

/// A set of marked file/directory paths for batch operations.
#[derive(Debug, Clone, Default)]
pub struct MarkSet {
    /// The set of marked paths.
    marked: HashSet<PathBuf>,
}

#[allow(dead_code, reason = "Methods used incrementally as file ops are integrated")]
impl MarkSet {
    /// Create a new empty mark set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            marked: HashSet::new(),
        }
    }

    /// Toggle the mark state of a path.
    ///
    /// Returns `true` if the path is now marked, `false` if unmarked.
    pub fn toggle(&mut self, path: PathBuf) -> bool {
        if self.marked.contains(&path) {
            self.marked.remove(&path);
            false
        } else {
            self.marked.insert(path);
            true
        }
    }

    /// Clear all marks.
    pub fn clear(&mut self) {
        self.marked.clear();
    }

    /// Check if a path is marked.
    pub fn is_marked(&self, path: &Path) -> bool {
        self.marked.contains(path)
    }

    /// Get the number of marked paths.
    #[must_use]
    pub fn count(&self) -> usize {
        self.marked.len()
    }

    /// Check if there are no marks.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.marked.is_empty()
    }

    /// Get operation targets: marked paths if any, otherwise the cursor path.
    ///
    /// Returns a `Vec` of paths to operate on.
    #[must_use]
    pub fn targets_or_cursor(&self, cursor_path: &Path) -> Vec<PathBuf> {
        if self.marked.is_empty() {
            vec![cursor_path.to_path_buf()]
        } else {
            self.marked.iter().cloned().collect()
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    #[rstest]
    fn new_mark_set_is_empty() {
        let marks = MarkSet::new();
        assert_that!(marks.count(), eq(0));
        assert_that!(marks.is_empty(), eq(true));
    }

    #[rstest]
    fn toggle_marks_and_unmarks() {
        let mut marks = MarkSet::new();
        let path = PathBuf::from("/tmp/test.txt");

        let marked = marks.toggle(path.clone());
        assert_that!(marked, eq(true));
        assert_that!(marks.is_marked(&path), eq(true));
        assert_that!(marks.count(), eq(1));

        let marked = marks.toggle(path.clone());
        assert_that!(marked, eq(false));
        assert_that!(marks.is_marked(&path), eq(false));
        assert_that!(marks.count(), eq(0));
    }

    #[rstest]
    fn clear_removes_all_marks() {
        let mut marks = MarkSet::new();
        marks.toggle(PathBuf::from("/a"));
        marks.toggle(PathBuf::from("/b"));
        marks.toggle(PathBuf::from("/c"));
        assert_that!(marks.count(), eq(3));

        marks.clear();
        assert_that!(marks.count(), eq(0));
        assert_that!(marks.is_empty(), eq(true));
    }

    #[rstest]
    fn targets_or_cursor_returns_cursor_when_no_marks() {
        let marks = MarkSet::new();
        let cursor = PathBuf::from("/cursor/file.txt");
        let targets = marks.targets_or_cursor(&cursor);
        assert_eq!(targets, vec![cursor]);
    }

    #[rstest]
    fn targets_or_cursor_returns_marks_when_marked() {
        let mut marks = MarkSet::new();
        let a = PathBuf::from("/a");
        let b = PathBuf::from("/b");
        marks.toggle(a.clone());
        marks.toggle(b.clone());

        let cursor = PathBuf::from("/cursor");
        let mut targets = marks.targets_or_cursor(&cursor);
        targets.sort();
        assert_eq!(targets, vec![a, b]);
    }
}
