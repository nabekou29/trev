//! Tree sorting logic.

use crate::state::tree::{
    SortDirection,
    SortOrder,
    TreeNode,
};

/// Sort a slice of tree nodes by the given criteria.
///
/// Sorts in-place. If `dirs_first` is true, directories appear before files.
pub fn sort_children(
    children: &mut [TreeNode],
    order: SortOrder,
    direction: SortDirection,
    dirs_first: bool,
) {
    children.sort_by(|a, b| {
        // 1. Directories first (if enabled)
        if dirs_first {
            match (a.is_dir, b.is_dir) {
                (true, false) => return std::cmp::Ordering::Less,
                (false, true) => return std::cmp::Ordering::Greater,
                _ => {}
            }
        }

        // 2. Sort by key
        let ord = match order {
            SortOrder::Smart => compare_smart(&a.name, &b.name),
            SortOrder::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortOrder::Size => a.size.cmp(&b.size),
            SortOrder::Modified => compare_modified(a.modified, b.modified),
            SortOrder::Type => {
                let a_is_dir = u8::from(!a.is_dir);
                let b_is_dir = u8::from(!b.is_dir);
                a_is_dir
                    .cmp(&b_is_dir)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            }
            SortOrder::Extension => {
                let a_ext = extension_of(&a.name);
                let b_ext = extension_of(&b.name);
                a_ext.cmp(&b_ext).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            }
        };

        // 3. Apply direction
        match direction {
            SortDirection::Asc => ord,
            SortDirection::Desc => ord.reverse(),
        }
    });
}

/// Recursively apply sort to a tree node and all its loaded children.
pub fn apply_sort_recursive(
    node: &mut TreeNode,
    order: SortOrder,
    direction: SortDirection,
    dirs_first: bool,
) {
    if let Some(children) = node.children.as_loaded_mut() {
        sort_children(children, order, direction, dirs_first);
        for child in children.iter_mut() {
            apply_sort_recursive(child, order, direction, dirs_first);
        }
    }
}

/// Compare two `Option<SystemTime>` values. `None` sorts to end.
fn compare_modified(
    a: Option<std::time::SystemTime>,
    b: Option<std::time::SystemTime>,
) -> std::cmp::Ordering {
    match (a, b) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

/// Compare two filenames using smart sort: natural sort with suffix grouping.
///
/// 1. Decompose names into (base, suffix).
/// 2. Compare bases using natural sort.
/// 3. Files without suffix sort before files with suffix.
/// 4. Among suffixed files, compare suffixes using natural sort.
/// 5. Tie-break on full name.
fn compare_smart(a: &str, b: &str) -> std::cmp::Ordering {
    let (base_a, suffix_a) = decompose_name(a);
    let (base_b, suffix_b) = decompose_name(b);

    compare_natural(base_a, base_b)
        .then_with(|| match (suffix_a, suffix_b) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Less,
            (Some(_), None) => std::cmp::Ordering::Greater,
            (Some(sa), Some(sb)) => compare_natural(sa, sb),
        })
        .then_with(|| compare_natural(a, b))
}

/// Extract the lowercase extension from a filename, or empty string.
fn extension_of(name: &str) -> String {
    std::path::Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

/// Decompose a filename into (base, optional suffix) for smart sort grouping.
///
/// Rules:
/// 1. No extension → `(name, None)`
/// 2. Stem ends with `_test` or `_spec` → underscore split
/// 3. Stem contains a dot → last dot segment is suffix
/// 4. Otherwise → `(name, None)` (single-dot file like `main.rs`)
fn decompose_name(name: &str) -> (&str, Option<&str>) {
    // Find the last dot (extension separator).
    let Some(ext_pos) = name.rfind('.') else {
        return (name, None);
    };

    // Dotfiles with no further extension (e.g. ".gitignore") have no suffix.
    if ext_pos == 0 {
        return (name, None);
    }

    let stem = &name[..ext_pos];

    // Check underscore patterns in stem.
    for suffix_pat in ["_test", "_spec"] {
        if let Some(base) = stem.strip_suffix(suffix_pat)
            && !base.is_empty()
        {
            return (base, Some(&name[base.len()..]));
        }
    }

    // Check dot pattern in stem.
    if let Some(dot_pos) = stem.rfind('.')
        && dot_pos > 0
    {
        return (&name[..dot_pos], Some(&name[dot_pos..]));
    }

    // No suffix — base is the stem (without extension).
    (stem, None)
}

/// A chunk of text for natural sort comparison.
#[derive(Debug)]
enum Chunk<'a> {
    /// A run of non-digit characters.
    Text(&'a str),
    /// A run of digit characters (stored as string for arbitrary precision).
    Num(&'a str),
}

/// Split a string into alternating Text/Num chunks for natural sort.
fn chunks(s: &str) -> Vec<Chunk<'_>> {
    let mut result = Vec::new();
    let mut chars = s.char_indices().peekable();

    while let Some(&(start, ch)) = chars.peek() {
        if ch.is_ascii_digit() {
            while chars.peek().is_some_and(|&(_, c)| c.is_ascii_digit()) {
                chars.next();
            }
            let end = chars.peek().map_or(s.len(), |&(i, _)| i);
            result.push(Chunk::Num(&s[start..end]));
        } else {
            while chars.peek().is_some_and(|&(_, c)| !c.is_ascii_digit()) {
                chars.next();
            }
            let end = chars.peek().map_or(s.len(), |&(i, _)| i);
            result.push(Chunk::Text(&s[start..end]));
        }
    }

    result
}

/// Compare two strings using natural sort order.
///
/// - Numeric chunks compare by value (length then lexicographic).
/// - Text chunks compare case-insensitively, with case-sensitive tie-break.
/// - Text vs Num: numbers sort before text.
fn compare_natural(a: &str, b: &str) -> std::cmp::Ordering {
    let a_chunks = chunks(a);
    let b_chunks = chunks(b);

    for (ac, bc) in a_chunks.iter().zip(b_chunks.iter()) {
        let ord = match (ac, bc) {
            (Chunk::Num(an), Chunk::Num(bn)) => {
                // Compare by digit count (shorter = smaller), then lexicographic.
                let an = an.trim_start_matches('0');
                let bn = bn.trim_start_matches('0');
                an.len().cmp(&bn.len()).then_with(|| an.cmp(bn))
            }
            (Chunk::Text(at), Chunk::Text(bt)) => {
                let a_low = at.to_lowercase();
                let b_low = bt.to_lowercase();
                a_low.cmp(&b_low).then_with(|| at.cmp(bt))
            }
            // Numbers sort before text.
            (Chunk::Num(_), Chunk::Text(_)) => std::cmp::Ordering::Less,
            (Chunk::Text(_), Chunk::Num(_)) => std::cmp::Ordering::Greater,
        };
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
    }

    // Shorter sequence of chunks comes first.
    a_chunks.len().cmp(&b_chunks.len())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use std::path::Path;

    use googletest::prelude::*;
    use rstest::*;

    use super::*;
    use crate::state::tree::ChildrenState;

    /// Helper: create a file node for sort tests.
    fn file_node(name: &str, size: u64, modified: Option<std::time::SystemTime>) -> TreeNode {
        TreeNode {
            name: name.to_string(),
            path: Path::new("/test").join(name),
            is_dir: false,
            is_symlink: false,
            symlink_target: None,
            size,
            modified,
            recursive_max_mtime: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
        }
    }

    /// Helper: create a directory node for sort tests.
    fn dir_node_sort(name: &str) -> TreeNode {
        TreeNode {
            name: name.to_string(),
            path: Path::new("/test").join(name),
            is_dir: true,
            is_symlink: false,
            symlink_target: None,
            size: 0,
            modified: None,
            recursive_max_mtime: None,
            children: ChildrenState::Loaded(vec![]),
            is_expanded: false,
        }
    }

    fn names(nodes: &[TreeNode]) -> Vec<&str> {
        nodes.iter().map(|n| n.name.as_str()).collect()
    }

    // --- US3 Tests ---

    #[rstest]
    fn test_sort_name_asc_case_insensitive() -> Result<()> {
        let mut nodes = vec![
            file_node("Charlie.txt", 0, None),
            file_node("alpha.txt", 0, None),
            file_node("Bravo.txt", 0, None),
        ];
        sort_children(&mut nodes, SortOrder::Name, SortDirection::Asc, false);
        verify_that!(names(&nodes), eq(&vec!["alpha.txt", "Bravo.txt", "Charlie.txt"]))?;
        Ok(())
    }

    #[rstest]
    fn test_sort_size_desc() -> Result<()> {
        let mut nodes = vec![
            file_node("small.txt", 10, None),
            file_node("big.txt", 1000, None),
            file_node("medium.txt", 500, None),
        ];
        sort_children(&mut nodes, SortOrder::Size, SortDirection::Desc, false);
        verify_that!(names(&nodes), eq(&vec!["big.txt", "medium.txt", "small.txt"]))?;
        Ok(())
    }

    #[rstest]
    fn test_sort_directories_first() -> Result<()> {
        let mut nodes = vec![
            file_node("z_file.txt", 0, None),
            dir_node_sort("a_dir"),
            file_node("a_file.txt", 0, None),
            dir_node_sort("z_dir"),
        ];
        sort_children(&mut nodes, SortOrder::Name, SortDirection::Asc, true);
        verify_that!(names(&nodes), eq(&vec!["a_dir", "z_dir", "a_file.txt", "z_file.txt"]))?;
        Ok(())
    }

    #[rstest]
    fn test_sort_recursive() -> Result<()> {
        let inner_children = vec![file_node("b.txt", 0, None), file_node("a.txt", 0, None)];
        let mut parent = TreeNode {
            name: "parent".to_string(),
            path: Path::new("/test/parent").to_path_buf(),
            is_dir: true,
            is_symlink: false,
            symlink_target: None,
            size: 0,
            modified: None,
            recursive_max_mtime: None,
            children: ChildrenState::Loaded(inner_children),
            is_expanded: true,
        };

        apply_sort_recursive(&mut parent, SortOrder::Name, SortDirection::Asc, false);

        let children = parent.children.as_loaded().unwrap();
        verify_that!(names(children), eq(&vec!["a.txt", "b.txt"]))?;
        Ok(())
    }

    #[rstest]
    fn test_sort_modified_none_at_end() -> Result<()> {
        let now = std::time::SystemTime::now();
        let earlier = now.checked_sub(std::time::Duration::from_hours(1)).unwrap();
        let mut nodes = vec![
            file_node("no_time.txt", 0, None),
            file_node("newer.txt", 0, Some(now)),
            file_node("older.txt", 0, Some(earlier)),
        ];
        sort_children(&mut nodes, SortOrder::Modified, SortDirection::Asc, false);
        verify_that!(names(&nodes), eq(&vec!["older.txt", "newer.txt", "no_time.txt"]))?;
        Ok(())
    }

    // =========================================================================
    // T004: decompose_name tests
    // =========================================================================

    #[rstest]
    #[case("user.test.ts", "user", Some(".test.ts"))]
    #[case("app.config.ts", "app", Some(".config.ts"))]
    #[case("index.module.css", "index", Some(".module.css"))]
    #[case("handler_test.go", "handler", Some("_test.go"))]
    #[case("handler_spec.rb", "handler", Some("_spec.rb"))]
    #[case("main.rs", "main", None)]
    #[case("README.md", "README", None)]
    #[case("Makefile", "Makefile", None)]
    #[case(".gitignore", ".gitignore", None)]
    #[case("foo.bar.test.ts", "foo.bar", Some(".test.ts"))]
    #[case("foo.bar.baz.ts", "foo.bar", Some(".baz.ts"))]
    #[case("test_utils.ts", "test_utils", None)]
    #[case("_test.go", "_test", None)] // base would be empty for underscore pattern, so falls through
    fn decompose_name_cases(
        #[case] input: &str,
        #[case] expected_base: &str,
        #[case] expected_suffix: Option<&str>,
    ) {
        let (base, suffix) = decompose_name(input);
        assert_eq!(base, expected_base, "base mismatch for {input}");
        assert_eq!(suffix, expected_suffix, "suffix mismatch for {input}");
    }

    // =========================================================================
    // T005: compare_natural tests
    // =========================================================================

    #[rstest]
    #[case("file2", "file10", std::cmp::Ordering::Less)]
    #[case("file1", "file2", std::cmp::Ordering::Less)]
    #[case("file10", "file10", std::cmp::Ordering::Equal)]
    #[case("file10", "file2", std::cmp::Ordering::Greater)]
    fn natural_sort_numeric_ordering(
        #[case] a: &str,
        #[case] b: &str,
        #[case] expected: std::cmp::Ordering,
    ) {
        assert_eq!(compare_natural(a, b), expected, "{a} vs {b}");
    }

    #[rstest]
    fn natural_sort_case_insensitive() {
        // "api" and "Api" should be considered equal ignoring case.
        // Tie-break: lowercase "a" < uppercase "A" in byte order, so "Api" < "api".
        assert_eq!(compare_natural("api", "Api"), std::cmp::Ordering::Greater);
        assert_eq!(compare_natural("Api", "api"), std::cmp::Ordering::Less);
    }

    #[rstest]
    fn natural_sort_pure_numbers() {
        assert_eq!(compare_natural("1", "2"), std::cmp::Ordering::Less);
        assert_eq!(compare_natural("2", "10"), std::cmp::Ordering::Less);
        assert_eq!(compare_natural("10", "10"), std::cmp::Ordering::Equal);
    }

    #[rstest]
    fn natural_sort_long_numbers() {
        // Arbitrary precision: 99999999999999999 < 100000000000000000 (by digit count).
        assert_eq!(
            compare_natural("file99999999999999999", "file100000000000000000"),
            std::cmp::Ordering::Less
        );
    }

    #[rstest]
    fn natural_sort_text_vs_num() {
        // Numbers should sort before text.
        assert_eq!(compare_natural("1abc", "abc"), std::cmp::Ordering::Less);
    }

    #[rstest]
    fn natural_sort_empty_strings() {
        assert_eq!(compare_natural("", ""), std::cmp::Ordering::Equal);
        assert_eq!(compare_natural("", "a"), std::cmp::Ordering::Less);
    }

    // =========================================================================
    // T006: SortOrder::Smart integration tests
    // =========================================================================

    #[rstest]
    fn smart_sort_suffix_grouping_ts() -> Result<()> {
        let mut nodes = vec![
            file_node("user.test.ts", 0, None),
            file_node("admin.ts", 0, None),
            file_node("user.ts", 0, None),
            file_node("user.stories.ts", 0, None),
            file_node("user.mock.ts", 0, None),
        ];
        sort_children(&mut nodes, SortOrder::Smart, SortDirection::Asc, false);
        verify_that!(
            names(&nodes),
            eq(&vec!["admin.ts", "user.ts", "user.mock.ts", "user.stories.ts", "user.test.ts"])
        )?;
        Ok(())
    }

    #[rstest]
    fn smart_sort_go_style() -> Result<()> {
        let mut nodes = vec![
            file_node("server_test.go", 0, None),
            file_node("handler.go", 0, None),
            file_node("handler_test.go", 0, None),
            file_node("server.go", 0, None),
        ];
        sort_children(&mut nodes, SortOrder::Smart, SortDirection::Asc, false);
        verify_that!(
            names(&nodes),
            eq(&vec!["handler.go", "handler_test.go", "server.go", "server_test.go"])
        )?;
        Ok(())
    }

    #[rstest]
    fn smart_sort_natural_numbers() -> Result<()> {
        let mut nodes = vec![
            file_node("file10.txt", 0, None),
            file_node("file2.txt", 0, None),
            file_node("file1.txt", 0, None),
            file_node("file20.txt", 0, None),
            file_node("file100.txt", 0, None),
        ];
        sort_children(&mut nodes, SortOrder::Smart, SortDirection::Asc, false);
        verify_that!(
            names(&nodes),
            eq(&vec!["file1.txt", "file2.txt", "file10.txt", "file20.txt", "file100.txt"])
        )?;
        Ok(())
    }

    #[rstest]
    fn smart_sort_three_dot_files() -> Result<()> {
        let mut nodes =
            vec![file_node("foo.bar.test.ts", 0, None), file_node("foo.bar.ts", 0, None)];
        sort_children(&mut nodes, SortOrder::Smart, SortDirection::Asc, false);
        verify_that!(names(&nodes), eq(&vec!["foo.bar.ts", "foo.bar.test.ts"]))?;
        Ok(())
    }

    #[rstest]
    fn smart_sort_mixed_with_dirs_first() -> Result<()> {
        let mut nodes = vec![
            file_node("user.test.ts", 0, None),
            dir_node_sort("components"),
            file_node("user.ts", 0, None),
            dir_node_sort("utils"),
        ];
        sort_children(&mut nodes, SortOrder::Smart, SortDirection::Asc, true);
        verify_that!(names(&nodes), eq(&vec!["components", "utils", "user.ts", "user.test.ts"]))?;
        Ok(())
    }

    #[rstest]
    fn smart_sort_acceptance_scenario_5() -> Result<()> {
        // From spec: index.tsx (no suffix, first), then suffixed alphabetically.
        let mut nodes = vec![
            file_node("index.test.tsx", 0, None),
            file_node("index.stories.tsx", 0, None),
            file_node("index.module.css", 0, None),
            file_node("index.tsx", 0, None),
        ];
        sort_children(&mut nodes, SortOrder::Smart, SortDirection::Asc, false);
        verify_that!(
            names(&nodes),
            eq(&vec!["index.tsx", "index.module.css", "index.stories.tsx", "index.test.tsx"])
        )?;
        Ok(())
    }

    #[rstest]
    fn smart_sort_desc_reverses_order() -> Result<()> {
        let mut nodes = vec![
            file_node("file1.txt", 0, None),
            file_node("file10.txt", 0, None),
            file_node("file2.txt", 0, None),
        ];
        sort_children(&mut nodes, SortOrder::Smart, SortDirection::Desc, false);
        verify_that!(names(&nodes), eq(&vec!["file10.txt", "file2.txt", "file1.txt"]))?;
        Ok(())
    }

    /// Performance: smart sort with 100,000 nodes must complete within 1 second.
    #[rstest]
    #[ignore = "performance test: run with --ignored"]
    fn smart_sort_100k_nodes_under_1_second() {
        let mut nodes: Vec<TreeNode> =
            (0..100_000).map(|i| file_node(&format!("component{i}.test.tsx"), 0, None)).collect();

        let start = std::time::Instant::now();
        sort_children(&mut nodes, SortOrder::Smart, SortDirection::Asc, true);
        let elapsed = start.elapsed();

        assert!(elapsed.as_secs() < 1, "smart sort of 100k nodes took {elapsed:?}, expected < 1s");
    }
}
