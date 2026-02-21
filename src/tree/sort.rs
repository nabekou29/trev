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

/// Extract the lowercase extension from a filename, or empty string.
fn extension_of(name: &str) -> String {
    std::path::Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
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
}
