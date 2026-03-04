//! Fuzzy search engine for the file tree.
//!
//! Wraps `nucleo::Matcher` to perform fuzzy matching against [`SearchIndex`]
//! entries. Uses a fixed-size min-heap for O(n log k) top-k selection.

use std::cmp::Reverse;
use std::collections::{
    BinaryHeap,
    HashSet,
};
use std::path::{
    Path,
    PathBuf,
};

use nucleo::Matcher;
use nucleo::pattern::{
    AtomKind,
    CaseMatching,
    Normalization,
    Pattern,
};

use super::search_index::SearchEntry;

/// A single search result with score and match indices.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Absolute path to the matched entry.
    pub path: PathBuf,
    /// Whether this entry is a directory.
    pub is_dir: bool,
    /// Fuzzy match score (higher is better).
    pub score: u32,
    /// Byte indices into the file/directory name where characters matched.
    pub match_indices: Vec<u32>,
}

/// Comparable wrapper for min-heap: sorts by score ascending so we can
/// pop the lowest-scoring entry when the heap is full.
#[derive(Debug)]
struct HeapEntry {
    /// Match score.
    score: u32,
    /// Index into the entries slice (avoids cloning `PathBuf` during heap ops).
    index: usize,
    /// Matched character indices.
    match_indices: Vec<u32>,
}

impl PartialEq for HeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}
impl Eq for HeapEntry {}
impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score.cmp(&other.score)
    }
}

/// Search the given entries for the query using fuzzy matching.
///
/// Returns up to `max_results` results, sorted by score descending.
/// Matching is performed on the file/directory name (not the full path).
pub fn search(
    entries: &[SearchEntry],
    query: &str,
    root_path: &Path,
    max_results: usize,
) -> Vec<SearchResult> {
    let _ = root_path; // Reserved for future use (e.g. path-based matching).
    if query.is_empty() || max_results == 0 {
        return Vec::new();
    }

    let mut matcher = Matcher::new(nucleo::Config::DEFAULT);
    let pattern = Pattern::new(query, CaseMatching::Smart, Normalization::Smart, AtomKind::Fuzzy);

    // Min-heap (Reverse) to keep top-k highest scores.
    let mut heap: BinaryHeap<Reverse<HeapEntry>> = BinaryHeap::with_capacity(max_results + 1);
    let mut indices_buf = Vec::new();
    let mut utf32_buf = Vec::new();

    for (i, entry) in entries.iter().enumerate() {
        let haystack = nucleo::Utf32Str::new(&entry.name, &mut utf32_buf);

        indices_buf.clear();
        let score = pattern.indices(haystack, &mut matcher, &mut indices_buf);

        if let Some(score) = score {
            let he = HeapEntry { score, index: i, match_indices: indices_buf.clone() };
            heap.push(Reverse(he));
            if heap.len() > max_results {
                heap.pop(); // Remove lowest score.
            }
        }
    }

    // Drain heap into results, sorted by score descending.
    // `into_sorted_vec()` on `BinaryHeap<Reverse<HeapEntry>>` yields
    // ascending `Reverse` order = descending score order.
    heap.into_sorted_vec()
        .into_iter()
        .filter_map(|Reverse(he)| {
            let entry = entries.get(he.index)?;
            Some(SearchResult {
                path: entry.path.clone(),
                is_dir: entry.is_dir,
                score: he.score,
                match_indices: he.match_indices,
            })
        })
        .collect()
}

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

    #[rstest]
    fn search_empty_query_returns_empty() {
        let entries = vec![make_entry("/root/foo.txt", false)];
        let results = search(&entries, "", Path::new("/root"), 100);
        assert_that!(results.len(), eq(0));
    }

    #[rstest]
    fn search_matches_file_name() {
        let entries = vec![
            make_entry("/root/foo.txt", false),
            make_entry("/root/bar.txt", false),
            make_entry("/root/foobar.rs", false),
        ];
        let results = search(&entries, "foo", Path::new("/root"), 100);
        assert_that!(results.len(), ge(1));
        // "foo.txt" should be a match.
        assert!(results.iter().any(|r| r.path == Path::new("/root/foo.txt")));
    }

    #[rstest]
    fn search_respects_max_results() {
        let entries: Vec<SearchEntry> =
            (0..100).map(|i| make_entry(&format!("/root/file{i}.txt"), false)).collect();
        let results = search(&entries, "file", Path::new("/root"), 5);
        assert_that!(results.len(), eq(5));
    }

    #[rstest]
    fn search_results_sorted_by_score_descending() {
        let entries = vec![
            make_entry("/root/abcdef.txt", false),
            make_entry("/root/abc.txt", false),
            make_entry("/root/sub/abc_file.txt", false),
        ];
        let results = search(&entries, "abc", Path::new("/root"), 100);
        // Scores should be in descending order.
        for window in results.windows(2) {
            assert!(window[0].score >= window[1].score);
        }
    }

    #[rstest]
    fn search_matches_name_only() {
        let entries = vec![make_entry("/root/src/main.rs", false)];
        // Matches the file name "main.rs", not the full path.
        let results = search(&entries, "main", Path::new("/root"), 100);
        assert_that!(results.len(), eq(1));
        // Path-based query should not match.
        let results = search(&entries, "src/main", Path::new("/root"), 100);
        assert_that!(results.len(), eq(0));
    }

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
}
