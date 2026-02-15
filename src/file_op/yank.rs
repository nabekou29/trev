//! Yank buffer for copy/cut operations.

use std::path::{
    Path,
    PathBuf,
};

use serde::{
    Deserialize,
    Serialize,
};

/// Whether the yank operation is a copy or cut.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum YankMode {
    /// Copy files (originals remain after paste).
    Copy,
    /// Cut files (originals removed after paste).
    Cut,
}

/// Buffer holding yanked (copied/cut) file paths.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct YankBuffer {
    /// Yanked file paths.
    paths: Vec<PathBuf>,
    /// Yank mode (None when buffer is empty).
    mode: Option<YankMode>,
}

impl YankBuffer {
    /// Create a new empty yank buffer.
    pub const fn new() -> Self {
        Self {
            paths: Vec::new(),
            mode: None,
        }
    }

    /// Set the buffer with the given paths and mode.
    pub fn set(&mut self, paths: Vec<PathBuf>, mode: YankMode) {
        self.paths = paths;
        self.mode = Some(mode);
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.paths.clear();
        self.mode = None;
    }

    /// Whether the buffer is empty.
    pub const fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Get the yanked paths.
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    /// Get the yank mode.
    pub const fn mode(&self) -> Option<&YankMode> {
        self.mode.as_ref()
    }

    /// Number of yanked paths.
    #[allow(dead_code, reason = "Used by future status bar display")]
    pub const fn count(&self) -> usize {
        self.paths.len()
    }

    /// Check if a specific path is in the yank buffer.
    #[allow(dead_code, reason = "Used by future UI mark highlighting")]
    pub fn contains(&self, path: &Path) -> bool {
        self.paths.iter().any(|p| p == path)
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
        let buf = YankBuffer::new();
        assert_that!(buf.is_empty(), eq(true));
        assert_that!(buf.count(), eq(0));
        assert_that!(buf.mode(), none());
    }

    #[rstest]
    fn set_populates_buffer() {
        let mut buf = YankBuffer::new();
        buf.set(
            vec![PathBuf::from("/a/b"), PathBuf::from("/c/d")],
            YankMode::Copy,
        );
        assert_that!(buf.is_empty(), eq(false));
        assert_that!(buf.count(), eq(2));
        assert_that!(buf.paths()[0].to_str().unwrap(), eq("/a/b"));
        assert_that!(buf.paths()[1].to_str().unwrap(), eq("/c/d"));
        assert_that!(buf.mode(), some(eq(&YankMode::Copy)));
    }

    #[rstest]
    fn set_with_cut_mode() {
        let mut buf = YankBuffer::new();
        buf.set(vec![PathBuf::from("/x")], YankMode::Cut);
        assert_that!(buf.mode(), some(eq(&YankMode::Cut)));
    }

    #[rstest]
    fn clear_empties_buffer() {
        let mut buf = YankBuffer::new();
        buf.set(vec![PathBuf::from("/a")], YankMode::Copy);
        buf.clear();
        assert_that!(buf.is_empty(), eq(true));
        assert_that!(buf.mode(), none());
    }

    #[rstest]
    fn set_replaces_previous_contents() {
        let mut buf = YankBuffer::new();
        buf.set(vec![PathBuf::from("/old")], YankMode::Copy);
        buf.set(vec![PathBuf::from("/new1"), PathBuf::from("/new2")], YankMode::Cut);
        assert_that!(buf.count(), eq(2));
        assert_that!(buf.mode(), some(eq(&YankMode::Cut)));
        assert_that!(buf.contains(&PathBuf::from("/old")), eq(false));
    }

    #[rstest]
    fn contains_finds_path() {
        let mut buf = YankBuffer::new();
        buf.set(vec![PathBuf::from("/a"), PathBuf::from("/b")], YankMode::Copy);
        assert_that!(buf.contains(Path::new("/a")), eq(true));
        assert_that!(buf.contains(Path::new("/c")), eq(false));
    }
}
