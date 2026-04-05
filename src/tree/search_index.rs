//! Search index for background file system scanning.

use std::path::{
    Path,
    PathBuf,
};
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::{
    AtomicBool,
    AtomicUsize,
    Ordering,
};

use super::search_engine::inject_entry;

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

    /// Append multiple entries to the index in bulk.
    pub fn append_entries(&mut self, entries: &mut Vec<SearchEntry>) {
        self.entries.append(entries);
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

/// Flush threshold: number of entries accumulated per thread before flushing
/// to the shared index.
const FLUSH_THRESHOLD: usize = 4096;

/// Default maximum number of entries in the search index.
/// Limits memory usage for very large directory trees (e.g. `/`).
pub const DEFAULT_MAX_ENTRIES: usize = 1_000_000;

/// Build a search index by performing a parallel scan of the directory tree.
///
/// Uses `ignore::WalkBuilder::build_parallel()` for multi-threaded traversal
/// with `.gitignore` awareness. Entries are flushed incrementally to `index`
/// so that partial results are available for search while scanning continues.
///
/// Stops early when `cancelled` is set to `true` (e.g. on app shutdown) or
/// when the index reaches `max_entries`.
///
/// This function is synchronous and should be called via `tokio::task::spawn_blocking`.
pub fn build_search_index(
    index: &Arc<RwLock<SearchIndex>>,
    root_path: &Path,
    show_hidden: bool,
    show_ignored: bool,
    cancelled: &Arc<AtomicBool>,
    max_entries: usize,
) {
    let span = tracing::info_span!(
        "build_search_index",
        root_path = %root_path.display(),
        entries = tracing::field::Empty,
    );
    let _guard = span.enter();

    let root_owned = root_path.to_path_buf();

    ignore::WalkBuilder::new(root_path)
        .hidden(!show_hidden)
        .git_ignore(!show_ignored)
        .git_global(!show_ignored)
        .git_exclude(!show_ignored)
        .build_parallel()
        .visit(&mut IndexVisitorBuilder { root: &root_owned, index, cancelled, max_entries });

    // Mark complete and record final count.
    if let Ok(mut guard) = index.write() {
        guard.mark_complete();
        span.record("entries", guard.entries().len());
    }
}

/// Builder that creates per-thread [`IndexVisitor`]s.
struct IndexVisitorBuilder<'a> {
    /// Root path to exclude from results.
    root: &'a Path,
    /// Shared index to flush entries into incrementally.
    index: &'a Arc<RwLock<SearchIndex>>,
    /// Cancellation flag checked by each visitor.
    cancelled: &'a Arc<AtomicBool>,
    /// Maximum total entries across all threads.
    max_entries: usize,
}

impl<'s> ignore::ParallelVisitorBuilder<'s> for IndexVisitorBuilder<'s> {
    fn build(&mut self) -> Box<dyn ignore::ParallelVisitor + 's> {
        Box::new(IndexVisitor {
            root: self.root,
            index: Arc::clone(self.index),
            cancelled: Arc::clone(self.cancelled),
            max_entries: self.max_entries,
            local: Vec::with_capacity(FLUSH_THRESHOLD),
        })
    }
}

/// Per-thread visitor that accumulates entries locally and flushes periodically.
struct IndexVisitor<'a> {
    /// Root path to exclude from results.
    root: &'a Path,
    /// Shared index to flush into.
    index: Arc<RwLock<SearchIndex>>,
    /// Cancellation flag — stops walking when `true`.
    cancelled: Arc<AtomicBool>,
    /// Maximum total entries in the shared index.
    max_entries: usize,
    /// Thread-local entry buffer.
    local: Vec<SearchEntry>,
}

impl IndexVisitor<'_> {
    /// Flush the local buffer into the shared index.
    ///
    /// Returns `true` if the index has reached the maximum entry limit.
    fn flush(&mut self) -> bool {
        if self.local.is_empty() {
            return false;
        }
        if let Ok(mut guard) = self.index.write() {
            guard.append_entries(&mut self.local);
            return guard.entries().len() >= self.max_entries;
        }
        false
    }
}

impl ignore::ParallelVisitor for IndexVisitor<'_> {
    fn visit(&mut self, entry: Result<ignore::DirEntry, ignore::Error>) -> ignore::WalkState {
        // Check cancellation.
        if self.cancelled.load(Ordering::Relaxed) {
            return ignore::WalkState::Quit;
        }

        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                tracing::warn!("Search index: skipping entry: {err}");
                return ignore::WalkState::Continue;
            }
        };

        let path = entry.path();

        // Skip the root directory itself.
        if path == self.root {
            return ignore::WalkState::Continue;
        }

        let is_dir = entry.file_type().is_some_and(|ft| ft.is_dir());
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();

        self.local.push(SearchEntry { path: path.to_path_buf(), name, is_dir });

        if self.local.len() >= FLUSH_THRESHOLD {
            let limit_reached = self.flush();
            if limit_reached {
                tracing::info!(max = self.max_entries, "search index entry limit reached");
                return ignore::WalkState::Quit;
            }
        }

        ignore::WalkState::Continue
    }
}

impl Drop for IndexVisitor<'_> {
    fn drop(&mut self) {
        self.flush();
    }
}

// ===========================================================================
// Nucleo Injector — parallel scan that pushes directly into Nucleo<T>
// ===========================================================================

/// Build a search index by injecting entries into a `Nucleo` injector.
///
/// Same parallel directory walk as [`build_search_index`], but pushes entries
/// directly into a lock-free [`nucleo::Injector`] for immediate availability
/// to the async fuzzy matcher.
///
/// This function is synchronous and should be called via `tokio::task::spawn_blocking`.
pub fn inject_into_nucleo(
    injector: &nucleo::Injector<SearchEntry>,
    root_path: &Path,
    show_hidden: bool,
    show_ignored: bool,
    cancelled: &Arc<AtomicBool>,
    max_entries: usize,
) {
    let span = tracing::info_span!(
        "inject_into_nucleo",
        root_path = %root_path.display(),
        entries = tracing::field::Empty,
    );
    let _guard = span.enter();

    let root_owned = root_path.to_path_buf();
    let count = Arc::new(AtomicUsize::new(0));

    ignore::WalkBuilder::new(root_path)
        .hidden(!show_hidden)
        .git_ignore(!show_ignored)
        .git_global(!show_ignored)
        .git_exclude(!show_ignored)
        .build_parallel()
        .visit(&mut NucleoVisitorBuilder {
            root: &root_owned,
            injector,
            cancelled,
            max_entries,
            count: &count,
        });

    span.record("entries", count.load(Ordering::Relaxed));
}

/// Builder that creates per-thread [`NucleoVisitor`]s.
struct NucleoVisitorBuilder<'a> {
    root: &'a Path,
    injector: &'a nucleo::Injector<SearchEntry>,
    cancelled: &'a Arc<AtomicBool>,
    max_entries: usize,
    count: &'a Arc<AtomicUsize>,
}

impl<'s> ignore::ParallelVisitorBuilder<'s> for NucleoVisitorBuilder<'s> {
    fn build(&mut self) -> Box<dyn ignore::ParallelVisitor + 's> {
        Box::new(NucleoVisitor {
            root: self.root,
            injector: self.injector.clone(),
            cancelled: Arc::clone(self.cancelled),
            max_entries: self.max_entries,
            count: Arc::clone(self.count),
        })
    }
}

/// Per-thread visitor that pushes entries directly into the Nucleo injector.
struct NucleoVisitor<'a> {
    root: &'a Path,
    injector: nucleo::Injector<SearchEntry>,
    cancelled: Arc<AtomicBool>,
    max_entries: usize,
    count: Arc<AtomicUsize>,
}

impl ignore::ParallelVisitor for NucleoVisitor<'_> {
    fn visit(&mut self, entry: Result<ignore::DirEntry, ignore::Error>) -> ignore::WalkState {
        if self.cancelled.load(Ordering::Relaxed) {
            return ignore::WalkState::Quit;
        }

        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                tracing::warn!("Nucleo index: skipping entry: {err}");
                return ignore::WalkState::Continue;
            }
        };

        let path = entry.path();

        // Skip the root directory itself.
        if path == self.root {
            return ignore::WalkState::Continue;
        }

        let is_dir = entry.file_type().is_some_and(|ft| ft.is_dir());
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();

        let search_entry = SearchEntry { path: path.to_path_buf(), name, is_dir };
        inject_entry(&self.injector, search_entry, self.root);

        let prev = self.count.fetch_add(1, Ordering::Relaxed);
        if prev + 1 >= self.max_entries {
            tracing::info!(max = self.max_entries, "nucleo index entry limit reached");
            return ignore::WalkState::Quit;
        }

        ignore::WalkState::Continue
    }
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

    /// Helper: build a search index and return the completed index.
    fn build_index(root: &Path, show_hidden: bool, show_ignored: bool) -> SearchIndex {
        let index = Arc::new(RwLock::new(SearchIndex::new()));
        let cancelled = Arc::new(AtomicBool::new(false));
        build_search_index(
            &index,
            root,
            show_hidden,
            show_ignored,
            &cancelled,
            DEFAULT_MAX_ENTRIES,
        );
        Arc::try_unwrap(index).unwrap().into_inner().unwrap()
    }

    #[rstest]
    fn test_build_search_index_scans_all_files() -> Result<()> {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file1.txt"), "").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        fs::write(dir.path().join("subdir/file2.txt"), "").unwrap();

        let index = build_index(dir.path(), false, false);
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

        let index = build_index(dir.path(), false, false);

        let names: Vec<&str> = index.entries().iter().map(|e| e.name.as_str()).collect();
        verify_that!(names.contains(&"visible.txt"), eq(true))?;
        verify_that!(names.contains(&"secret.txt"), eq(false))?;
        verify_that!(names.contains(&"ignored"), eq(false))?;
        Ok(())
    }

    #[rstest]
    fn test_build_index_hidden_false_ignored_true_excludes_hidden_gitignored() -> Result<()> {
        let dir = TempDir::new().unwrap();
        std::process::Command::new("git").args(["init"]).current_dir(dir.path()).output().unwrap();
        fs::write(dir.path().join(".gitignore"), ".vscode/\n").unwrap();
        fs::create_dir(dir.path().join(".vscode")).unwrap();
        fs::write(dir.path().join(".vscode/settings.json"), "{}").unwrap();
        fs::write(dir.path().join("visible.txt"), "").unwrap();

        // show_hidden=false, show_ignored=true → .vscode should NOT appear
        let index = build_index(dir.path(), false, true);
        let names: Vec<&str> = index.entries().iter().map(|e| e.name.as_str()).collect();

        verify_that!(names.contains(&".vscode"), eq(false))?;
        verify_that!(names.contains(&"settings.json"), eq(false))?;
        verify_that!(names.contains(&"visible.txt"), eq(true))?;
        Ok(())
    }

    /// Reproduces the real-world pattern: `.vscode/*` with whitelisted files.
    #[rstest]
    fn test_build_index_hidden_false_ignored_true_whitelist_pattern() -> Result<()> {
        let dir = TempDir::new().unwrap();
        std::process::Command::new("git").args(["init"]).current_dir(dir.path()).output().unwrap();
        // Same pattern as trev's own .gitignore
        fs::write(
            dir.path().join(".gitignore"),
            ".vscode/*\n!.vscode/settings.json\n!.vscode/extensions.json\n",
        )
        .unwrap();
        fs::create_dir(dir.path().join(".vscode")).unwrap();
        fs::write(dir.path().join(".vscode/settings.json"), "{}").unwrap();
        fs::write(dir.path().join(".vscode/other.json"), "{}").unwrap();
        fs::write(dir.path().join("visible.txt"), "").unwrap();

        // show_hidden=false, show_ignored=true → nothing under .vscode should appear
        let index = build_index(dir.path(), false, true);
        let names: Vec<&str> = index.entries().iter().map(|e| e.name.as_str()).collect();

        verify_that!(names.contains(&".vscode"), eq(false))?;
        verify_that!(names.contains(&"settings.json"), eq(false))?;
        verify_that!(names.contains(&"other.json"), eq(false))?;
        verify_that!(names.contains(&"visible.txt"), eq(true))?;
        Ok(())
    }
}
