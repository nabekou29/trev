//! Lightweight file identity snapshots used to detect external changes.
//!
//! Undo/redo replays operations recorded earlier, but other programs can
//! modify the file system in between. A [`Fingerprint`] captures `lstat`
//! metadata so a later step can confirm it is acting on the same entry it
//! produced, without reading file contents.

use std::path::Path;
use std::time::SystemTime;

use serde::{
    Deserialize,
    Serialize,
};

/// Kind of a file system entry (symlinks are not followed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
    /// Regular file (or any other non-directory, non-symlink entry).
    File,
    /// Directory.
    Dir,
    /// Symbolic link.
    Symlink,
}

/// Snapshot of an entry's identity and content metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingerprint {
    /// Entry kind.
    pub kind: EntryKind,
    /// Device ID (`0` where unavailable).
    pub dev: u64,
    /// Inode number (`0` where unavailable).
    pub ino: u64,
    /// Size in bytes.
    pub size: u64,
    /// Last modification time.
    pub mtime: Option<SystemTime>,
}

impl Fingerprint {
    /// Capture the fingerprint of `path` without following symlinks.
    ///
    /// Returns `None` if nothing exists at `path`.
    pub fn of(path: &Path) -> Option<Self> {
        let meta = path.symlink_metadata().ok()?;
        let file_type = meta.file_type();
        let kind = if file_type.is_symlink() {
            EntryKind::Symlink
        } else if file_type.is_dir() {
            EntryKind::Dir
        } else {
            EntryKind::File
        };
        let (dev, ino) = dev_ino(&meta);
        Some(Self { kind, dev, ino, size: meta.len(), mtime: meta.modified().ok() })
    }

    /// Whether `other` is the same entry (same inode), regardless of content.
    ///
    /// A rename keeps the inode, so this holds across moves on one file system.
    pub fn same_entry(&self, other: &Self) -> bool {
        self.kind == other.kind && self.dev == other.dev && self.ino == other.ino
    }

    /// Whether `other` is the same entry and its content looks unmodified.
    ///
    /// For a directory, size and mtime change when entries are added or
    /// removed directly inside it; changes deeper in the tree are not detected.
    pub fn same_content(&self, other: &Self) -> bool {
        self.same_entry(other) && self.size == other.size && self.mtime == other.mtime
    }
}

/// Device and inode numbers of the entry.
#[cfg(unix)]
fn dev_ino(meta: &std::fs::Metadata) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt as _;
    (meta.dev(), meta.ino())
}

/// Device and inode numbers of the entry (unavailable on this platform).
#[cfg(not(unix))]
const fn dev_ino(_meta: &std::fs::Metadata) -> (u64, u64) {
    (0, 0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    #[rstest]
    fn of_returns_none_for_missing_path() {
        assert_that!(Fingerprint::of(Path::new("/nonexistent/path")), none());
    }

    #[rstest]
    fn rename_keeps_same_entry() {
        let tmp = tempfile::TempDir::new().unwrap();
        let a = tmp.path().join("a.txt");
        let b = tmp.path().join("b.txt");
        std::fs::write(&a, "data").unwrap();
        let before = Fingerprint::of(&a).unwrap();

        std::fs::rename(&a, &b).unwrap();

        let after = Fingerprint::of(&b).unwrap();
        assert!(before.same_entry(&after));
        assert!(before.same_content(&after));
    }

    #[rstest]
    fn recreated_file_is_different_entry() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("a.txt");
        std::fs::write(&path, "data").unwrap();
        let before = Fingerprint::of(&path).unwrap();
        // Keep the old inode alive so the new file cannot reuse its number.
        let moved = tmp.path().join("old.txt");
        std::fs::rename(&path, &moved).unwrap();

        std::fs::write(&path, "data").unwrap();

        assert!(!before.same_entry(&Fingerprint::of(&path).unwrap()));
    }

    #[rstest]
    fn modified_file_is_same_entry_but_different_content() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("a.txt");
        std::fs::write(&path, "data").unwrap();
        let before = Fingerprint::of(&path).unwrap();

        std::fs::write(&path, "more data").unwrap();

        let after = Fingerprint::of(&path).unwrap();
        assert!(before.same_entry(&after));
        assert!(!before.same_content(&after));
    }

    #[cfg(unix)]
    #[rstest]
    fn dangling_symlink_is_fingerprinted_as_symlink() {
        let tmp = tempfile::TempDir::new().unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink("missing-target", &link).unwrap();

        let fp = Fingerprint::of(&link).unwrap();
        assert_that!(fp.kind, eq(EntryKind::Symlink));
    }
}
