//! Fuzzy search engine for the file tree.
//!
//! Provides two search backends:
//! - [`search()`]: Synchronous single-threaded fuzzy matching (used by benchmarks).
//! - [`NucleoSearchEngine`]: Async parallel matching powered by `Nucleo<T>`,
//!   designed for responsive TUI search with 10M+ entries.

use std::collections::HashSet;
use std::fmt;
use std::path::{
    Path,
    PathBuf,
};
use std::sync::Arc;

use nucleo::Matcher;
use nucleo::pattern::{
    CaseMatching,
    Normalization,
    Pattern,
};

use super::search_index::SearchEntry;
use crate::input::SearchMode;

/// Maximum number of search results used for tree filtering.
///
/// Limits the cost of [`compute_visible_paths`] and tree filter operations
/// while still showing the most relevant results.
pub const MAX_SEARCH_RESULTS: usize = 10_000;

/// Column index for the file/directory name (used in Name mode).
const COL_NAME: usize = 0;
/// Column index for the relative path from root (used in Path mode).
const COL_PATH: usize = 1;
/// Number of columns in the Nucleo matcher.
const NUM_COLUMNS: u32 = 2;

/// A single search result with score and match indices.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Absolute path to the matched entry.
    pub path: PathBuf,
    /// Whether this entry is a directory.
    pub is_dir: bool,
    /// Fuzzy match score (higher is better).
    pub score: u32,
    /// Byte indices into the matched string where characters matched.
    ///
    /// The matched string is the file/directory name in `Name` mode,
    /// or the relative path in `Path` mode.
    pub match_indices: Vec<u32>,
}

// ===========================================================================
// NucleoSearchEngine — async parallel search backend
// ===========================================================================

/// Async parallel search engine powered by `nucleo::Nucleo<T>`.
///
/// Items are injected via [`Injector`](nucleo::Injector) from background threads.
/// Pattern updates and result retrieval happen on the main thread via
/// [`update_pattern`](Self::update_pattern) and [`collect_results`](Self::collect_results).
/// Call [`tick`](Self::tick) regularly (e.g. each frame) to process background work.
pub struct NucleoSearchEngine {
    /// The nucleo parallel matcher instance.
    nucleo: nucleo::Nucleo<SearchEntry>,
    /// Reusable matcher for computing match indices on demand.
    indices_matcher: Matcher,
    /// Reusable buffer for match index computation.
    indices_buf: Vec<u32>,
}

impl fmt::Debug for NucleoSearchEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let snapshot = self.nucleo.snapshot();
        f.debug_struct("NucleoSearchEngine")
            .field("item_count", &snapshot.item_count())
            .field("matched_count", &snapshot.matched_item_count())
            .finish_non_exhaustive()
    }
}

impl NucleoSearchEngine {
    /// Create a new search engine.
    ///
    /// `notify` is called from worker threads whenever results change.
    /// Use it to wake the event loop for a redraw.
    pub fn new(notify: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            nucleo: nucleo::Nucleo::new(nucleo::Config::DEFAULT, notify, None, NUM_COLUMNS),
            indices_matcher: Matcher::new(nucleo::Config::DEFAULT),
            indices_buf: Vec::new(),
        }
    }

    /// Get a thread-safe injector for pushing items from background threads.
    pub fn injector(&self) -> nucleo::Injector<SearchEntry> {
        self.nucleo.injector()
    }

    /// Update the search pattern.
    ///
    /// Sets the pattern on the active column (name or path) and clears the
    /// inactive column. Supports fzf-style syntax (`^foo`, `foo$`, `!foo`).
    pub fn update_pattern(&mut self, query: &str, mode: SearchMode) {
        let (active, inactive) = match mode {
            SearchMode::Name => (COL_NAME, COL_PATH),
            SearchMode::Path => (COL_PATH, COL_NAME),
        };
        self.nucleo.pattern.reparse(
            active,
            query,
            CaseMatching::Smart,
            Normalization::Smart,
            false,
        );
        self.nucleo.pattern.reparse(inactive, "", CaseMatching::Smart, Normalization::Smart, false);
    }

    /// Process pending background work.
    ///
    /// Returns status indicating whether results changed and whether workers
    /// are still running. Should be called regularly (e.g. each frame).
    pub fn tick(&mut self, timeout: u64) -> nucleo::Status {
        self.nucleo.tick(timeout)
    }

    /// Clear all items and reset the matcher.
    ///
    /// Used when rebuilding the index (e.g. after toggling hidden/ignored).
    /// Old injectors are disconnected; call [`injector`](Self::injector) to get
    /// a new one connected to the fresh item set.
    pub fn restart(&mut self) {
        self.nucleo.restart(true);
    }

    /// Number of items matching the current pattern.
    pub fn matched_item_count(&self) -> u32 {
        self.nucleo.snapshot().matched_item_count()
    }

    /// Total number of injected items.
    pub fn item_count(&self) -> u32 {
        self.nucleo.snapshot().item_count()
    }

    /// Collect search results from the current snapshot.
    ///
    /// Returns at most `max_results` items, sorted by score (best first).
    /// Match indices are computed only for the returned items (not all matches).
    pub fn collect_results(&mut self, mode: SearchMode, max_results: usize) -> Vec<SearchResult> {
        let col = match mode {
            SearchMode::Name => COL_NAME,
            SearchMode::Path => COL_PATH,
        };

        // Borrow checker requires destructuring: `nucleo` is borrowed immutably
        // (snapshot + pattern) while `indices_matcher` and `indices_buf` are mutable.
        let Self { nucleo, indices_matcher, indices_buf } = self;
        let snapshot = nucleo.snapshot();
        let count = (snapshot.matched_item_count() as usize).min(max_results);
        if count == 0 {
            return Vec::new();
        }
        let pattern = nucleo.pattern.column_pattern(col);

        let mut results = Vec::with_capacity(count);
        #[allow(clippy::cast_possible_truncation)]
        for item in snapshot.matched_items(0..count as u32) {
            indices_buf.clear();
            if let Some(haystack) = item.matcher_columns.get(col) {
                let _ = pattern.indices(haystack.slice(..), indices_matcher, indices_buf);
            }

            results.push(SearchResult {
                path: item.data.path.clone(),
                is_dir: item.data.is_dir,
                score: 0, // Nucleo sorts results internally; rank is implicit.
                match_indices: indices_buf.clone(),
            });
        }

        results
    }
}

/// Inject a single entry into the Nucleo engine.
///
/// Fills column 0 (name) and column 1 (relative path from `root_path`).
pub fn inject_entry(
    injector: &nucleo::Injector<SearchEntry>,
    entry: SearchEntry,
    root_path: &Path,
) {
    injector.push(entry, |e, cols| {
        if let Some(col) = cols.get_mut(COL_NAME) {
            *col = e.name.as_str().into();
        }
        if let Some(col) = cols.get_mut(COL_PATH)
            && let Ok(rel) = e.path.strip_prefix(root_path)
        {
            *col = rel.to_string_lossy().as_ref().into();
        }
    });
}

// ===========================================================================
// Synchronous search — kept for benchmarks and backward compatibility
// ===========================================================================

/// Search the given entries for the query using fuzzy matching.
///
/// Returns all matching results sorted by score descending.
/// In `Name` mode, matches against the file/directory name.
/// In `Path` mode, matches against the relative path from `root_path`.
pub fn search(
    entries: &[SearchEntry],
    query: &str,
    root_path: &Path,
    mode: SearchMode,
) -> Vec<SearchResult> {
    if query.is_empty() {
        return Vec::new();
    }

    // Use `parse` instead of `new` to support fzf-style syntax:
    // 'foo (substring), ^foo (prefix), foo$ (suffix), !foo (negation).
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);

    let mut ctx = MatchContext {
        matcher: Matcher::new(nucleo::Config::DEFAULT),
        indices_buf: Vec::new(),
        utf32_buf: Vec::new(),
        path_buf: String::new(),
    };

    let mut results: Vec<SearchResult> = entries
        .iter()
        .filter_map(|entry| {
            let (score, indices) = match_entry(entry, &pattern, &mut ctx, root_path, mode)?;
            Some(SearchResult {
                path: entry.path.clone(),
                is_dir: entry.is_dir,
                score,
                match_indices: indices,
            })
        })
        .collect();
    results.sort_by_key(|r| std::cmp::Reverse(r.score));
    results
}

/// Reusable buffers for fuzzy matching, avoiding per-entry allocation.
struct MatchContext {
    /// The nucleo matcher instance.
    matcher: Matcher,
    /// Buffer for match indices.
    indices_buf: Vec<u32>,
    /// Buffer for UTF-32 conversion.
    utf32_buf: Vec<char>,
    /// Buffer for relative path strings.
    path_buf: String,
}

/// Try to match a single entry against the pattern.
///
/// Returns `Some((score, match_indices))` if the entry matches, `None`
/// otherwise.
fn match_entry(
    entry: &SearchEntry,
    pattern: &Pattern,
    ctx: &mut MatchContext,
    root_path: &Path,
    mode: SearchMode,
) -> Option<(u32, Vec<u32>)> {
    let haystack_str = match mode {
        SearchMode::Name => &entry.name,
        SearchMode::Path => {
            ctx.path_buf.clear();
            let rel = entry.path.strip_prefix(root_path).ok()?;
            ctx.path_buf.push_str(&rel.to_string_lossy());
            &ctx.path_buf
        }
    };
    let haystack = nucleo::Utf32Str::new(haystack_str, &mut ctx.utf32_buf);

    ctx.indices_buf.clear();
    let score = pattern.indices(haystack, &mut ctx.matcher, &mut ctx.indices_buf)?;
    Some((score, ctx.indices_buf.clone()))
}

// ===========================================================================
// Shared utilities
// ===========================================================================

/// Compute the set of paths that should be visible when filtering the tree.
///
/// Includes all matched paths plus their ancestor directories up to (but not
/// including) `root_path`.
pub fn compute_visible_paths(results: &[SearchResult], root_path: &Path) -> HashSet<PathBuf> {
    let mut visible = HashSet::new();

    for result in results {
        visible.insert(result.path.clone());

        // Walk up to root, adding each ancestor.
        let mut ancestor = result.path.as_path();
        while let Some(parent) = ancestor.parent() {
            if parent == root_path || !visible.insert(parent.to_path_buf()) {
                break;
            }
            ancestor = parent;
        }
    }

    visible
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    fn make_entry(path: &str, is_dir: bool) -> SearchEntry {
        let p = PathBuf::from(path);
        let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
        SearchEntry { path: p, name, is_dir }
    }

    // ===================================================================
    // Synchronous search tests
    // ===================================================================

    #[rstest]
    fn search_empty_query_returns_empty() {
        let entries = vec![make_entry("/root/foo.txt", false)];
        let results = search(&entries, "", Path::new("/root"), SearchMode::Name);
        assert_that!(results.len(), eq(0));
    }

    #[rstest]
    fn search_matches_file_name() {
        let entries = vec![
            make_entry("/root/foo.txt", false),
            make_entry("/root/bar.txt", false),
            make_entry("/root/foobar.rs", false),
        ];
        let results = search(&entries, "foo", Path::new("/root"), SearchMode::Name);
        assert_that!(results.len(), ge(1));
        // "foo.txt" should be a match.
        assert!(results.iter().any(|r| r.path == Path::new("/root/foo.txt")));
    }

    #[rstest]
    fn search_returns_all_matches() {
        let entries: Vec<SearchEntry> =
            (0..100).map(|i| make_entry(&format!("/root/file{i}.txt"), false)).collect();
        let results = search(&entries, "file", Path::new("/root"), SearchMode::Name);
        assert_that!(results.len(), eq(100));
    }

    #[rstest]
    fn search_results_sorted_by_score_descending() {
        let entries = vec![
            make_entry("/root/abcdef.txt", false),
            make_entry("/root/abc.txt", false),
            make_entry("/root/sub/abc_file.txt", false),
        ];
        let results = search(&entries, "abc", Path::new("/root"), SearchMode::Name);
        // Scores should be in descending order.
        for window in results.windows(2) {
            assert!(window[0].score >= window[1].score);
        }
    }

    #[rstest]
    fn search_name_mode_does_not_match_path() {
        let entries = vec![make_entry("/root/src/main.rs", false)];
        // Name mode matches the file name only.
        let results = search(&entries, "main", Path::new("/root"), SearchMode::Name);
        assert_that!(results.len(), eq(1));
        // Path-based query should not match in Name mode.
        let results = search(&entries, "src/main", Path::new("/root"), SearchMode::Name);
        assert_that!(results.len(), eq(0));
    }

    #[rstest]
    fn search_path_mode_matches_relative_path() {
        let entries = vec![make_entry("/root/src/main.rs", false)];
        let results = search(&entries, "src/main", Path::new("/root"), SearchMode::Path);
        assert_that!(results.len(), eq(1));
    }

    // ===================================================================
    // compute_visible_paths tests
    // ===================================================================

    #[rstest]
    fn compute_visible_paths_includes_ancestors() {
        let results = vec![SearchResult {
            path: PathBuf::from("/root/a/b/c.txt"),
            is_dir: false,
            score: 100,
            match_indices: vec![],
        }];
        let visible = compute_visible_paths(&results, Path::new("/root"));
        assert!(visible.contains(Path::new("/root/a/b/c.txt")));
        assert!(visible.contains(Path::new("/root/a/b")));
        assert!(visible.contains(Path::new("/root/a")));
        // root itself is NOT included.
        assert!(!visible.contains(Path::new("/root")));
    }

    #[rstest]
    fn compute_visible_paths_deduplicates() {
        let results = vec![
            SearchResult {
                path: PathBuf::from("/root/a/x.txt"),
                is_dir: false,
                score: 100,
                match_indices: vec![],
            },
            SearchResult {
                path: PathBuf::from("/root/a/y.txt"),
                is_dir: false,
                score: 90,
                match_indices: vec![],
            },
        ];
        let visible = compute_visible_paths(&results, Path::new("/root"));
        // /root/a should appear once (deduplicated).
        assert!(visible.contains(Path::new("/root/a")));
        assert_that!(visible.len(), eq(3)); // x.txt, y.txt, a
    }

    // ===================================================================
    // NucleoSearchEngine tests
    // ===================================================================

    /// Helper: inject a single entry into the engine.
    fn inject(injector: &nucleo::Injector<SearchEntry>, root: &Path, path: &str, is_dir: bool) {
        let entry = make_entry(path, is_dir);
        inject_entry(injector, entry, root);
    }

    /// Helper: tick until processing is done.
    fn tick_until_done(engine: &mut NucleoSearchEngine) {
        for _ in 0..100 {
            let status = engine.tick(50);
            if !status.running {
                break;
            }
        }
    }

    #[rstest]
    fn nucleo_basic_name_search() {
        let engine_notify = Arc::new(|| {});
        let mut engine = NucleoSearchEngine::new(engine_notify);
        let injector = engine.injector();
        let root = Path::new("/root");

        inject(&injector, root, "/root/foo.txt", false);
        inject(&injector, root, "/root/bar.txt", false);
        inject(&injector, root, "/root/foobar.rs", false);

        engine.update_pattern("foo", SearchMode::Name);
        tick_until_done(&mut engine);

        let results = engine.collect_results(SearchMode::Name, 100);
        assert_that!(results.len(), ge(1));
        assert!(results.iter().any(|r| r.path == Path::new("/root/foo.txt")));
    }

    #[rstest]
    fn nucleo_empty_pattern_matches_all() {
        // In nucleo (like fzf), an empty pattern matches all items.
        // The application handler layer clears the filter for empty queries.
        let engine_notify = Arc::new(|| {});
        let mut engine = NucleoSearchEngine::new(engine_notify);
        let injector = engine.injector();
        let root = Path::new("/root");

        inject(&injector, root, "/root/foo.txt", false);
        engine.update_pattern("", SearchMode::Name);
        tick_until_done(&mut engine);

        let results = engine.collect_results(SearchMode::Name, 100);
        assert_that!(results.len(), eq(1));
    }

    #[rstest]
    fn nucleo_path_mode_search() {
        let engine_notify = Arc::new(|| {});
        let mut engine = NucleoSearchEngine::new(engine_notify);
        let injector = engine.injector();
        let root = Path::new("/root");

        inject(&injector, root, "/root/src/main.rs", false);
        inject(&injector, root, "/root/test/main.rs", false);

        engine.update_pattern("src/main", SearchMode::Path);
        tick_until_done(&mut engine);

        let results = engine.collect_results(SearchMode::Path, 100);
        assert_that!(results.len(), eq(1));
        assert_that!(results[0].path.to_str().unwrap(), eq("/root/src/main.rs"));
    }

    #[rstest]
    fn nucleo_pattern_update() {
        let engine_notify = Arc::new(|| {});
        let mut engine = NucleoSearchEngine::new(engine_notify);
        let injector = engine.injector();
        let root = Path::new("/root");

        inject(&injector, root, "/root/foo.txt", false);
        inject(&injector, root, "/root/bar.txt", false);

        // First search: "foo"
        engine.update_pattern("foo", SearchMode::Name);
        tick_until_done(&mut engine);
        let results = engine.collect_results(SearchMode::Name, 100);
        assert_that!(results.len(), ge(1));
        assert!(results.iter().all(|r| r.path != Path::new("/root/bar.txt")));

        // Update to "bar"
        engine.update_pattern("bar", SearchMode::Name);
        tick_until_done(&mut engine);
        let results = engine.collect_results(SearchMode::Name, 100);
        assert_that!(results.len(), ge(1));
        assert!(results.iter().any(|r| r.path == Path::new("/root/bar.txt")));
    }

    #[rstest]
    fn nucleo_restart_clears_items() {
        let engine_notify = Arc::new(|| {});
        let mut engine = NucleoSearchEngine::new(engine_notify);
        let injector = engine.injector();
        let root = Path::new("/root");

        inject(&injector, root, "/root/foo.txt", false);
        engine.update_pattern("foo", SearchMode::Name);
        tick_until_done(&mut engine);
        assert_that!(engine.matched_item_count(), ge(1));

        // Restart clears everything.
        engine.restart();
        tick_until_done(&mut engine);
        assert_that!(engine.item_count(), eq(0));
        assert_that!(engine.matched_item_count(), eq(0));
    }

    #[rstest]
    fn nucleo_collect_results_has_match_indices() {
        let engine_notify = Arc::new(|| {});
        let mut engine = NucleoSearchEngine::new(engine_notify);
        let injector = engine.injector();
        let root = Path::new("/root");

        inject(&injector, root, "/root/hello.txt", false);
        engine.update_pattern("hel", SearchMode::Name);
        tick_until_done(&mut engine);

        let results = engine.collect_results(SearchMode::Name, 100);
        assert_that!(results.len(), eq(1));
        // Match indices should be non-empty for a matching entry.
        assert!(!results[0].match_indices.is_empty());
    }

    #[rstest]
    fn nucleo_max_results_limits_output() {
        let engine_notify = Arc::new(|| {});
        let mut engine = NucleoSearchEngine::new(engine_notify);
        let injector = engine.injector();
        let root = Path::new("/root");

        for i in 0..100 {
            inject(&injector, root, &format!("/root/file{i}.txt"), false);
        }

        engine.update_pattern("file", SearchMode::Name);
        tick_until_done(&mut engine);

        let results = engine.collect_results(SearchMode::Name, 10);
        assert_that!(results.len(), eq(10));
        // But total matched count should be higher.
        assert_that!(engine.matched_item_count(), eq(100));
    }

    #[rstest]
    fn nucleo_collect_results_empty_when_no_match() {
        let engine_notify = Arc::new(|| {});
        let mut engine = NucleoSearchEngine::new(engine_notify);
        let injector = engine.injector();
        let root = Path::new("/root");

        inject(&injector, root, "/root/foo.txt", false);
        engine.update_pattern("zzzzz_no_match", SearchMode::Name);
        tick_until_done(&mut engine);

        let results = engine.collect_results(SearchMode::Name, 100);
        assert_that!(results.len(), eq(0));
        assert_that!(engine.matched_item_count(), eq(0));
    }

    #[rstest]
    fn nucleo_debug_format() {
        let engine_notify = Arc::new(|| {});
        let engine = NucleoSearchEngine::new(engine_notify);

        let debug = format!("{engine:?}");
        assert!(debug.contains("NucleoSearchEngine"));
        assert!(debug.contains("item_count"));
        assert!(debug.contains("matched_count"));
    }

    #[rstest]
    fn inject_entry_populates_name_and_path_columns() {
        let engine_notify = Arc::new(|| {});
        let mut engine = NucleoSearchEngine::new(engine_notify);
        let injector = engine.injector();
        let root = Path::new("/root");

        inject(&injector, root, "/root/src/main.rs", false);
        tick_until_done(&mut engine);

        // Name mode should match "main.rs".
        engine.update_pattern("main.rs", SearchMode::Name);
        tick_until_done(&mut engine);
        let results = engine.collect_results(SearchMode::Name, 10);
        assert_that!(results.len(), eq(1));

        // Path mode should match "src/main.rs".
        engine.update_pattern("src/main", SearchMode::Path);
        tick_until_done(&mut engine);
        let results = engine.collect_results(SearchMode::Path, 10);
        assert_that!(results.len(), eq(1));
    }

    #[rstest]
    fn nucleo_directory_entries_flagged_correctly() {
        let engine_notify = Arc::new(|| {});
        let mut engine = NucleoSearchEngine::new(engine_notify);
        let injector = engine.injector();
        let root = Path::new("/root");

        inject(&injector, root, "/root/mydir", true);
        inject(&injector, root, "/root/myfile.txt", false);

        engine.update_pattern("my", SearchMode::Name);
        tick_until_done(&mut engine);

        let results = engine.collect_results(SearchMode::Name, 100);
        assert_that!(results.len(), eq(2));
        let dir_result = results.iter().find(|r| r.path == Path::new("/root/mydir")).unwrap();
        let file_result = results.iter().find(|r| r.path == Path::new("/root/myfile.txt")).unwrap();
        assert!(dir_result.is_dir);
        assert!(!file_result.is_dir);
    }
}
