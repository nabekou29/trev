//! Fuzzy search engine for the file tree.
//!
//! Wraps `nucleo::Matcher` to perform fuzzy matching against [`SearchIndex`]
//! entries.

use std::cmp::Reverse;
use std::collections::HashSet;
use std::path::{
    Path,
    PathBuf,
};

use nucleo::Matcher;
use nucleo::pattern::{
    CaseMatching,
    Normalization,
    Pattern,
};

use super::search_index::SearchEntry;
use crate::input::SearchMode;

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
    results.sort_by_key(|r| Reverse(r.score));
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
