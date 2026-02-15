//! File system operation definitions and execution.

use std::path::{
    Path,
    PathBuf,
};

use anyhow::{
    Context as _,
    Result,
    bail,
};
use serde::{
    Deserialize,
    Serialize,
};

/// A reversible file system operation (can be undone).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FsOp {
    /// Copy a file or directory.
    Copy {
        /// Source path.
        src: PathBuf,
        /// Destination path.
        dst: PathBuf,
    },
    /// Move (rename) a file or directory.
    Move {
        /// Source path.
        src: PathBuf,
        /// Destination path.
        dst: PathBuf,
    },
    /// Create an empty file.
    CreateFile {
        /// Path to create.
        path: PathBuf,
    },
    /// Create a directory (with parents).
    CreateDir {
        /// Path to create.
        path: PathBuf,
    },
    /// Remove a file.
    RemoveFile {
        /// Path to remove.
        path: PathBuf,
    },
    /// Remove a directory recursively.
    RemoveDir {
        /// Path to remove.
        path: PathBuf,
    },
}

/// An irreversible file system operation (cannot be undone).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrreversibleOp {
    /// Send to OS system trash.
    SystemTrash {
        /// Path to trash.
        path: PathBuf,
    },
    /// Permanently delete.
    PermanentDelete {
        /// Path to delete.
        path: PathBuf,
    },
}

/// Execute a single reversible file system operation.
pub fn execute(op: &FsOp) -> Result<()> {
    match op {
        FsOp::Copy { src, dst } => copy_path(src, dst),
        FsOp::Move { src, dst } => move_path(src, dst),
        FsOp::CreateFile { path } => create_file(path),
        FsOp::CreateDir { path } => create_dir(path),
        FsOp::RemoveFile { path } => remove_file(path),
        FsOp::RemoveDir { path } => remove_dir(path),
    }
}

/// Check if `ancestor` is an ancestor of (or equal to) `path`.
///
/// Used to prevent copying/moving a directory into itself.
pub fn is_ancestor(ancestor: &Path, path: &Path) -> bool {
    path.starts_with(ancestor)
}

/// Copy a file or directory to a destination.
fn copy_path(src: &Path, dst: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(src)
        .with_context(|| format!("Failed to read metadata: {}", src.display()))?;

    if metadata.is_symlink() {
        copy_symlink(src, dst)
    } else if metadata.is_dir() {
        copy_dir_recursive(src, dst)
    } else {
        std::fs::copy(src, dst)
            .with_context(|| format!("Failed to copy {} → {}", src.display(), dst.display()))?;
        Ok(())
    }
}

/// Copy a symbolic link (recreates the link, does not follow it).
fn copy_symlink(src: &Path, dst: &Path) -> Result<()> {
    let target = std::fs::read_link(src)
        .with_context(|| format!("Failed to read symlink: {}", src.display()))?;

    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, dst)
        .with_context(|| format!("Failed to create symlink: {}", dst.display()))?;

    #[cfg(windows)]
    {
        if target.is_dir() {
            std::os::windows::fs::symlink_dir(&target, dst)
                .with_context(|| format!("Failed to create symlink: {}", dst.display()))?;
        } else {
            std::os::windows::fs::symlink_file(&target, dst)
                .with_context(|| format!("Failed to create symlink: {}", dst.display()))?;
        }
    }

    Ok(())
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)
        .with_context(|| format!("Failed to create directory: {}", dst.display()))?;

    for entry in std::fs::read_dir(src)
        .with_context(|| format!("Failed to read directory: {}", src.display()))?
    {
        let entry =
            entry.with_context(|| format!("Failed to read directory entry in {}", src.display()))?;
        let entry_path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);

        copy_path(&entry_path, &dst_path)?;
    }

    Ok(())
}

/// Move a file or directory, with cross-filesystem fallback.
fn move_path(src: &Path, dst: &Path) -> Result<()> {
    // Try rename first (atomic on same filesystem).
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) => {
            // Cross-filesystem: copy then remove.
            // EXDEV = 18 on Linux/macOS (cross-device link).
            if e.raw_os_error() == Some(18) {
                copy_path(src, dst)?;
                if src.is_dir() {
                    std::fs::remove_dir_all(src).with_context(|| {
                        format!("Failed to remove source directory after move: {}", src.display())
                    })?;
                } else {
                    std::fs::remove_file(src).with_context(|| {
                        format!("Failed to remove source file after move: {}", src.display())
                    })?;
                }
                Ok(())
            } else {
                Err(e).with_context(|| {
                    format!("Failed to move {} → {}", src.display(), dst.display())
                })
            }
        }
    }
}

/// Create an empty file, creating parent directories as needed.
fn create_file(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent directories: {}", parent.display()))?;
    }
    if path.exists() {
        bail!("File already exists: {}", path.display());
    }
    std::fs::File::create(path)
        .with_context(|| format!("Failed to create file: {}", path.display()))?;
    Ok(())
}

/// Create a directory, creating parent directories as needed.
fn create_dir(path: &Path) -> Result<()> {
    if path.exists() {
        bail!("Directory already exists: {}", path.display());
    }
    std::fs::create_dir_all(path)
        .with_context(|| format!("Failed to create directory: {}", path.display()))?;
    Ok(())
}

/// Remove a file.
fn remove_file(path: &Path) -> Result<()> {
    std::fs::remove_file(path)
        .with_context(|| format!("Failed to remove file: {}", path.display()))
}

/// Remove a directory recursively.
fn remove_dir(path: &Path) -> Result<()> {
    std::fs::remove_dir_all(path)
        .with_context(|| format!("Failed to remove directory: {}", path.display()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    #[rstest]
    fn execute_create_file_and_remove() {
        let tmp = tempfile::TempDir::new().unwrap();
        let file_path = tmp.path().join("test.txt");

        execute(&FsOp::CreateFile {
            path: file_path.clone(),
        })
        .unwrap();
        assert_that!(file_path.exists(), eq(true));

        execute(&FsOp::RemoveFile {
            path: file_path.clone(),
        })
        .unwrap();
        assert_that!(file_path.exists(), eq(false));
    }

    #[rstest]
    fn execute_create_dir_and_remove() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir_path = tmp.path().join("subdir");

        execute(&FsOp::CreateDir {
            path: dir_path.clone(),
        })
        .unwrap();
        assert_that!(dir_path.is_dir(), eq(true));

        execute(&FsOp::RemoveDir {
            path: dir_path.clone(),
        })
        .unwrap();
        assert_that!(dir_path.exists(), eq(false));
    }

    #[rstest]
    fn execute_create_nested_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let file_path = tmp.path().join("a/b/c.txt");

        execute(&FsOp::CreateFile {
            path: file_path.clone(),
        })
        .unwrap();
        assert_that!(file_path.exists(), eq(true));
        assert_that!(tmp.path().join("a/b").is_dir(), eq(true));
    }

    #[rstest]
    fn execute_copy_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = tmp.path().join("src.txt");
        let dst = tmp.path().join("dst.txt");
        std::fs::write(&src, "hello").unwrap();

        execute(&FsOp::Copy {
            src: src.clone(),
            dst: dst.clone(),
        })
        .unwrap();

        assert_that!(dst.exists(), eq(true));
        assert_that!(src.exists(), eq(true));
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), "hello");
    }

    #[rstest]
    fn execute_copy_dir_recursive() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src_dir = tmp.path().join("src_dir");
        let dst_dir = tmp.path().join("dst_dir");
        std::fs::create_dir(&src_dir).unwrap();
        std::fs::write(src_dir.join("file.txt"), "content").unwrap();
        std::fs::create_dir(src_dir.join("sub")).unwrap();
        std::fs::write(src_dir.join("sub/nested.txt"), "nested").unwrap();

        execute(&FsOp::Copy {
            src: src_dir.clone(),
            dst: dst_dir.clone(),
        })
        .unwrap();

        assert_that!(dst_dir.join("file.txt").exists(), eq(true));
        assert_that!(dst_dir.join("sub/nested.txt").exists(), eq(true));
        assert_eq!(
            std::fs::read_to_string(dst_dir.join("sub/nested.txt")).unwrap(),
            "nested"
        );
    }

    #[rstest]
    fn execute_move_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = tmp.path().join("src.txt");
        let dst = tmp.path().join("dst.txt");
        std::fs::write(&src, "hello").unwrap();

        execute(&FsOp::Move {
            src: src.clone(),
            dst: dst.clone(),
        })
        .unwrap();

        assert_that!(dst.exists(), eq(true));
        assert_that!(src.exists(), eq(false));
    }

    #[rstest]
    fn is_ancestor_detects_self_reference() {
        let parent = PathBuf::from("/a/b");
        assert_that!(is_ancestor(&parent, &PathBuf::from("/a/b/c")), eq(true));
        assert_that!(is_ancestor(&parent, &PathBuf::from("/a/b")), eq(true));
        assert_that!(is_ancestor(&parent, &PathBuf::from("/a/x")), eq(false));
        assert_that!(is_ancestor(&parent, &PathBuf::from("/a")), eq(false));
    }

    #[rstest]
    fn create_file_fails_if_exists() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("existing.txt");
        std::fs::write(&path, "exists").unwrap();

        let result = execute(&FsOp::CreateFile { path });
        assert!(result.is_err());
    }
}
