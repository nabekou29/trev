//! Search index for background file system scanning.

use std::path::{
    Path,
    PathBuf,
};

/// A single entry in the search index.
#[derive(Debug, Clone)]
pub struct SearchEntry {
    /// Absolute path to the file or directory.
    pub path: PathBuf,
    /// File/directory name.
    pub name: String,
    /// Whether this entry is a directory.
    pub is_dir: bool,
}

/// An index of all files/directories found during background scanning.
///
/// Built incrementally during background traversal. Can be queried
/// for search results even while scanning is in progress.
#[derive(Debug)]
pub struct SearchIndex {
    /// All discovered entries.
    entries: Vec<SearchEntry>,
    /// Whether the background scan has completed.
    is_complete: bool,
}

impl SearchIndex {
    /// Create a new, empty search index.
    pub const fn new() -> Self {
        Self { entries: Vec::new(), is_complete: false }
    }

    /// Add an entry to the index (called during scanning).
    pub fn add_entry(&mut self, entry: SearchEntry) {
        self.entries.push(entry);
    }

    /// Get all entries in the index.
    pub fn entries(&self) -> &[SearchEntry] {
        &self.entries
    }

    /// Whether the background scan has completed.
    pub const fn is_complete(&self) -> bool {
        self.is_complete
    }

    /// Mark the scan as complete.
    pub const fn mark_complete(&mut self) {
        self.is_complete = true;
    }

    /// Find direct children of the given parent path.
    ///
    /// This can be used as a cache for lazy loading: if the search index
    /// already has entries for a directory, we can skip filesystem IO.
    pub fn find_children(&self, parent: &Path) -> Vec<&SearchEntry> {
        self.entries.iter().filter(|e| e.path.parent() == Some(parent)).collect()
    }
}

impl Default for SearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a search index by performing a full scan of the directory tree.
///
/// Uses `ignore::WalkBuilder` for `.gitignore` awareness.
/// This function is synchronous and should be called via `tokio::task::spawn_blocking`.
pub fn build_search_index(root_path: &Path, show_hidden: bool, show_ignored: bool) -> SearchIndex {
    let mut index = SearchIndex::new();

    let walker = ignore::WalkBuilder::new(root_path)
        .hidden(!show_hidden)
        .git_ignore(!show_ignored)
        .git_global(!show_ignored)
        .git_exclude(!show_ignored)
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                tracing::warn!("Search index: skipping entry: {err}");
                continue;
            }
        };

        let path = entry.path();

        // Skip the root directory itself
        if path == root_path {
            continue;
        }

        let is_dir = entry.file_type().is_some_and(|ft| ft.is_dir());

        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();

        index.add_entry(SearchEntry { path: path.to_path_buf(), name, is_dir });
    }

    index.mark_complete();
    index
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use std::fs;

    use googletest::prelude::*;
    use rstest::*;
    use tempfile::TempDir;

    use super::*;

    #[rstest]
    fn test_add_entry_and_entries() -> Result<()> {
        let mut index = SearchIndex::new();
        index.add_entry(SearchEntry {
            path: PathBuf::from("/test/file.txt"),
            name: "file.txt".to_string(),
            is_dir: false,
        });
        verify_that!(index.entries().len(), eq(1))?;
        verify_that!(index.entries()[0].name.as_str(), eq("file.txt"))?;
        Ok(())
    }

    #[rstest]
    fn test_is_complete_lifecycle() -> Result<()> {
        let mut index = SearchIndex::new();
        verify_that!(index.is_complete(), eq(false))?;
        index.mark_complete();
        verify_that!(index.is_complete(), eq(true))?;
        Ok(())
    }

    #[rstest]
    fn test_find_children_returns_direct_children_only() -> Result<()> {
        let mut index = SearchIndex::new();
        index.add_entry(SearchEntry {
            path: PathBuf::from("/root/a.txt"),
            name: "a.txt".to_string(),
            is_dir: false,
        });
        index.add_entry(SearchEntry {
            path: PathBuf::from("/root/sub/b.txt"),
            name: "b.txt".to_string(),
            is_dir: false,
        });
        index.add_entry(SearchEntry {
            path: PathBuf::from("/root/sub"),
            name: "sub".to_string(),
            is_dir: true,
        });

        let children = index.find_children(Path::new("/root"));
        verify_that!(children.len(), eq(2))?; // a.txt and sub

        let sub_children = index.find_children(Path::new("/root/sub"));
        verify_that!(sub_children.len(), eq(1))?; // b.txt
        Ok(())
    }

    #[rstest]
    fn test_build_search_index_scans_all_files() -> Result<()> {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file1.txt"), "").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        fs::write(dir.path().join("subdir/file2.txt"), "").unwrap();

        let index = build_search_index(dir.path(), false, false);
        verify_that!(index.is_complete(), eq(true))?;
        // Should find: file1.txt, subdir, subdir/file2.txt
        verify_that!(index.entries().len(), eq(3))?;
        Ok(())
    }

    #[rstest]
    fn test_build_search_index_respects_gitignore() -> Result<()> {
        let dir = TempDir::new().unwrap();

        // Initialize git repo
        std::process::Command::new("git").args(["init"]).current_dir(dir.path()).output().unwrap();

        fs::write(dir.path().join(".gitignore"), "ignored/\n").unwrap();
        fs::create_dir(dir.path().join("ignored")).unwrap();
        fs::write(dir.path().join("ignored/secret.txt"), "").unwrap();
        fs::write(dir.path().join("visible.txt"), "").unwrap();

        let index = build_search_index(dir.path(), false, false);

        let names: Vec<&str> = index.entries().iter().map(|e| e.name.as_str()).collect();
        verify_that!(names.contains(&"visible.txt"), eq(true))?;
        verify_that!(names.contains(&"secret.txt"), eq(false))?;
        verify_that!(names.contains(&"ignored"), eq(false))?;
        Ok(())
    }
}
