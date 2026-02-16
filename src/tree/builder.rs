//! File system tree builder using `ignore::WalkBuilder`.

use std::path::Path;

use anyhow::{
    Context,
    Result,
    bail,
};

use crate::state::tree::{
    ChildrenState,
    TreeNode,
};

/// Builds a tree from the file system.
///
/// Uses `ignore::WalkBuilder` to respect `.gitignore` rules.
#[derive(Debug)]
pub struct TreeBuilder {
    /// Whether to show hidden (dot) files.
    show_hidden: bool,
    /// Whether to show gitignored files.
    show_ignored: bool,
}

impl TreeBuilder {
    /// Create a new `TreeBuilder` with display options.
    pub const fn new(show_hidden: bool, show_ignored: bool) -> Self {
        Self { show_hidden, show_ignored }
    }

    /// Build a tree from the given root path.
    ///
    /// Only loads the immediate children (depth=1). Subdirectories
    /// will have `ChildrenState::NotLoaded`.
    pub fn build(&self, root_path: &Path) -> Result<TreeNode> {
        let root_path = root_path
            .canonicalize()
            .with_context(|| format!("Failed to canonicalize path: {}", root_path.display()))?;

        let metadata = root_path
            .metadata()
            .with_context(|| format!("Failed to read metadata: {}", root_path.display()))?;

        if !metadata.is_dir() {
            bail!("Root path is not a directory: {}", root_path.display());
        }

        let name = root_path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();

        let children = self.load_children(&root_path)?;

        Ok(TreeNode {
            name,
            path: root_path,
            is_dir: true,
            is_symlink: false,
            size: 0,
            modified: metadata.modified().ok(),
            children: ChildrenState::Loaded(children),
            is_expanded: true,
        })
    }

    /// Load immediate children of a directory (depth=1).
    #[allow(clippy::unnecessary_wraps)]
    pub fn load_children(&self, dir_path: &Path) -> Result<Vec<TreeNode>> {
        let mut children = Vec::new();

        let walker = ignore::WalkBuilder::new(dir_path)
            .max_depth(Some(1))
            .hidden(!self.show_hidden)
            .git_ignore(!self.show_ignored)
            .git_global(!self.show_ignored)
            .git_exclude(!self.show_ignored)
            .build();

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    tracing::warn!("Skipping entry: {err}");
                    continue;
                }
            };

            let path = entry.path();

            // Skip the root directory itself
            if path == dir_path {
                continue;
            }

            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(err) => {
                    tracing::warn!("Failed to read metadata for {}: {err}", path.display());
                    continue;
                }
            };

            let is_dir = metadata.is_dir();
            let is_symlink = entry.path_is_symlink();

            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();

            children.push(TreeNode {
                name,
                path: path.to_path_buf(),
                is_dir,
                is_symlink,
                size: if is_dir { 0 } else { metadata.len() },
                modified: metadata.modified().ok(),
                children: ChildrenState::NotLoaded,
                is_expanded: false,
            });
        }

        Ok(children)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    use googletest::Result;
    use googletest::prelude::*;
    use rstest::*;
    use tempfile::TempDir;

    use super::*;

    /// Create a test directory structure:
    /// root/
    ///   file1.txt
    ///   file2.rs
    ///   subdir/
    ///     nested.txt
    fn create_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file1.txt"), "hello").unwrap();
        fs::write(dir.path().join("file2.rs"), "fn main() {}").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        fs::write(dir.path().join("subdir/nested.txt"), "nested").unwrap();
        dir
    }

    // --- US1 Tests ---

    #[rstest]
    fn test_build_returns_root_with_children_subdirs_not_loaded() -> Result<()> {
        let dir = create_test_dir();
        let builder = TreeBuilder::new(false, false);
        let root = builder.build(dir.path()).unwrap();

        verify_that!(root.is_dir, eq(true))?;
        verify_that!(root.is_expanded, eq(true))?;

        let children = root.children.as_loaded().unwrap();
        // Should have file1.txt, file2.rs, subdir
        verify_that!(children.len(), ge(3))?;

        // Find subdir — its children should be NotLoaded
        let subdir = children.iter().find(|c| c.name == "subdir").unwrap();
        verify_that!(subdir.is_dir, eq(true))?;
        verify_that!(subdir.children.as_loaded().is_some(), eq(false))?;

        Ok(())
    }

    #[rstest]
    fn test_build_gitignore_excludes_target() -> Result<()> {
        let dir = TempDir::new().unwrap();
        // Create .gitignore and target/
        fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("target/debug.txt"), "").unwrap();
        fs::write(dir.path().join("keep.txt"), "").unwrap();

        // Initialize a git repo so .gitignore is respected
        std::process::Command::new("git").args(["init"]).current_dir(dir.path()).output().unwrap();

        let builder = TreeBuilder::new(false, false);
        let root = builder.build(dir.path()).unwrap();
        let children = root.children.as_loaded().unwrap();

        let names: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
        verify_that!(names.contains(&"target"), eq(false))?;
        verify_that!(names.contains(&"keep.txt"), eq(true))?;
        Ok(())
    }

    #[rstest]
    fn test_build_show_hidden_false_excludes_dotfiles() -> Result<()> {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".hidden"), "").unwrap();
        fs::write(dir.path().join("visible.txt"), "").unwrap();

        let builder = TreeBuilder::new(false, false);
        let root = builder.build(dir.path()).unwrap();
        let children = root.children.as_loaded().unwrap();

        let names: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
        verify_that!(names.contains(&".hidden"), eq(false))?;
        verify_that!(names.contains(&"visible.txt"), eq(true))?;
        Ok(())
    }

    #[rstest]
    fn test_build_show_hidden_true_includes_dotfiles() -> Result<()> {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".hidden"), "").unwrap();
        fs::write(dir.path().join("visible.txt"), "").unwrap();

        let builder = TreeBuilder::new(true, false);
        let root = builder.build(dir.path()).unwrap();
        let children = root.children.as_loaded().unwrap();

        let names: Vec<&str> = children.iter().map(|c| c.name.as_str()).collect();
        verify_that!(names.contains(&".hidden"), eq(true))?;
        verify_that!(names.contains(&"visible.txt"), eq(true))?;
        Ok(())
    }

    #[rstest]
    fn test_build_nonexistent_path_returns_error() -> Result<()> {
        let builder = TreeBuilder::new(false, false);
        let result = builder.build(Path::new("/nonexistent/path/abc123"));
        verify_that!(result.is_err(), eq(true))?;
        Ok(())
    }

    #[cfg(unix)]
    #[rstest]
    fn test_build_symlink_detected() -> Result<()> {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("real.txt"), "content").unwrap();
        symlink(dir.path().join("real.txt"), dir.path().join("link.txt")).unwrap();

        let builder = TreeBuilder::new(false, false);
        let root = builder.build(dir.path()).unwrap();
        let children = root.children.as_loaded().unwrap();

        let link = children.iter().find(|c| c.name == "link.txt").unwrap();
        verify_that!(link.is_symlink, eq(true))?;
        Ok(())
    }
}
