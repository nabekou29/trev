//! Undo/redo history management with operation groups.

use std::collections::VecDeque;
use std::path::Path;
use std::time::SystemTime;

use anyhow::{
    Result,
    bail,
};
use serde::{
    Deserialize,
    Serialize,
};

use super::executor::FsOp;
use super::trash;

/// A single undoable file system operation with its reverse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoOp {
    /// Forward operation (what was done).
    pub forward: FsOp,
    /// Reverse operation (how to undo it).
    pub reverse: FsOp,
}

/// A group of operations performed as a single user action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpGroup {
    /// Human-readable description for status messages.
    pub description: String,
    /// Individual operations in execution order.
    pub ops: Vec<UndoOp>,
}

/// Bounded undo/redo history.
#[derive(Debug)]
pub struct UndoHistory {
    /// Completed operations (undo target). Back = most recent.
    undo_stack: VecDeque<OpGroup>,
    /// Undone operations (redo target). Back = most recent undo.
    redo_stack: VecDeque<OpGroup>,
    /// Maximum undo stack size.
    max_size: usize,
}

impl UndoHistory {
    /// Create a new undo history with the given maximum stack size.
    pub const fn new(max_size: usize) -> Self {
        Self { undo_stack: VecDeque::new(), redo_stack: VecDeque::new(), max_size }
    }

    /// Push a completed operation group onto the undo stack.
    ///
    /// Clears the redo stack (new action invalidates redo history).
    /// Evicts the oldest entry if the stack exceeds `max_size`,
    /// cleaning up any trash files referenced by the evicted group.
    pub fn push(&mut self, group: OpGroup) {
        self.redo_stack.clear();
        self.undo_stack.push_back(group);

        // Evict oldest if over capacity.
        if self.undo_stack.len() > self.max_size {
            if let Some(evicted) = self.undo_stack.front() {
                cleanup_trash_refs(evicted, &trash::trash_dir());
            }
            self.undo_stack.pop_front();
        }
    }

    /// Pop the most recent operation group for undo.
    ///
    /// Validates file system preconditions before returning.
    /// On success, moves the group to the redo stack and returns it.
    pub fn undo(&mut self) -> Result<Option<&OpGroup>> {
        let Some(group) = self.undo_stack.back() else {
            return Ok(None);
        };

        // Validate reverse ops can be executed.
        for undo_op in &group.ops {
            validate_op_precondition(&undo_op.reverse)?;
        }

        // Safe: validated that undo_stack.back() is Some above.
        let Some(group) = self.undo_stack.pop_back() else {
            return Ok(None);
        };
        self.redo_stack.push_back(group);
        Ok(self.redo_stack.back())
    }

    /// Pop the most recent undone operation group for redo.
    ///
    /// Validates file system preconditions before returning.
    /// On success, moves the group back to the undo stack and returns it.
    ///
    /// Deletes into the custom trash get a fresh trash name, so their
    /// retention period starts again from the redo.
    pub fn redo(&mut self) -> Result<Option<&OpGroup>> {
        let Some(group) = self.redo_stack.back_mut() else {
            return Ok(None);
        };

        let trash_dir = trash::trash_dir();
        for undo_op in &mut group.ops {
            retarget_trash_move(undo_op, &trash_dir);
        }

        // Validate forward ops can be re-executed.
        for undo_op in &group.ops {
            validate_op_precondition(&undo_op.forward)?;
        }

        // Safe: validated that redo_stack.back() is Some above.
        let Some(group) = self.redo_stack.pop_back() else {
            return Ok(None);
        };
        self.undo_stack.push_back(group);
        Ok(self.undo_stack.back())
    }

    /// Drop groups whose custom trash entries have expired, plus all older groups.
    ///
    /// Uses the same rule as the retention cleanup of the trash directory, so
    /// restored history never points at entries that cleanup deletes. Older
    /// groups go too because undo replays history newest first. Returns the
    /// number of dropped groups.
    ///
    /// Trash entries are left on disk: the retention cleanup deletes them in
    /// the background once they expire, so this stays cheap on startup.
    pub fn prune_expired_trash(
        &mut self,
        trash_dir: &Path,
        retention_days: u64,
        now: SystemTime,
    ) -> usize {
        let Some(last_expired) = self.undo_stack.iter().rposition(|group| {
            trash_refs(group, trash_dir).any(|path| trash::is_expired(path, retention_days, now))
        }) else {
            return 0;
        };

        self.undo_stack.drain(..=last_expired).count()
    }

    /// Whether there are operations to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Whether there are operations to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Export undo and redo stacks for serialization.
    pub fn export_stacks(&self) -> (Vec<OpGroup>, Vec<OpGroup>) {
        (self.undo_stack.iter().cloned().collect(), self.redo_stack.iter().cloned().collect())
    }

    /// Reconstruct an `UndoHistory` from previously exported stacks.
    pub fn from_stacks(
        undo_stack: Vec<OpGroup>,
        redo_stack: Vec<OpGroup>,
        max_size: usize,
    ) -> Self {
        Self { undo_stack: undo_stack.into(), redo_stack: redo_stack.into(), max_size }
    }
}

/// Validate that a file system operation's preconditions are met.
fn validate_op_precondition(op: &FsOp) -> Result<()> {
    match op {
        FsOp::Copy { src, dst } | FsOp::Move { src, dst } => {
            if !src.exists() {
                bail!("Source does not exist: {}", src.display());
            }
            if dst.exists() {
                bail!("Destination already exists: {}", dst.display());
            }
        }
        FsOp::CreateFile { path } | FsOp::CreateDir { path } => {
            if path.exists() {
                bail!("Path already exists: {}", path.display());
            }
        }
        FsOp::RemoveFile { path } | FsOp::RemoveDir { path } => {
            if !path.exists() {
                bail!("Path does not exist: {}", path.display());
            }
        }
        FsOp::CreateSymlink { link, .. } | FsOp::CreateHardlink { link, .. } => {
            if link.exists() {
                bail!("Link path already exists: {}", link.display());
            }
        }
    }
    Ok(())
}

/// Custom trash entries a group restores on undo.
fn trash_refs<'a>(group: &'a OpGroup, trash_dir: &'a Path) -> impl Iterator<Item = &'a Path> {
    group.ops.iter().filter_map(move |undo_op| match &undo_op.reverse {
        FsOp::Move { src, .. } if src.starts_with(trash_dir) => Some(src.as_path()),
        _ => None,
    })
}

/// Delete the custom trash entries a group restores on undo.
///
/// Used when the group is evicted from history, since nothing else can
/// restore those entries.
fn cleanup_trash_refs(group: &OpGroup, trash_dir: &Path) {
    for src in trash_refs(group, trash_dir) {
        if let Err(e) = trash::clean_trash_file(src) {
            tracing::warn!(%e, ?src, "failed to clean trash file of evicted undo group");
        }
    }
}

/// Point a delete-into-trash operation at a fresh trash path.
///
/// The trash name encodes when the entry was trashed, so a redo needs a new name.
fn retarget_trash_move(undo_op: &mut UndoOp, trash_dir: &Path) {
    let FsOp::Move { src, dst } = &undo_op.forward else {
        return;
    };
    if !dst.starts_with(trash_dir) {
        return;
    }
    let src = src.clone();
    let new_dst = trash::trash_path_in(trash_dir, &src);
    undo_op.reverse = FsOp::Move { src: new_dst.clone(), dst: src.clone() };
    undo_op.forward = FsOp::Move { src, dst: new_dst };
}

/// Build the reverse operation for a forward `FsOp`.
///
/// Returns `None` for operations that cannot be meaningfully reversed
/// (caller should handle this by not adding to undo history).
pub fn reverse_op(forward: &FsOp) -> Option<FsOp> {
    match forward {
        FsOp::Copy { dst, .. } => {
            // Undo copy = remove the copied file/dir.
            if dst.is_dir() {
                Some(FsOp::RemoveDir { path: dst.clone() })
            } else {
                Some(FsOp::RemoveFile { path: dst.clone() })
            }
        }
        FsOp::Move { src, dst } => {
            // Undo move = move back.
            Some(FsOp::Move { src: dst.clone(), dst: src.clone() })
        }
        FsOp::CreateFile { path } => Some(FsOp::RemoveFile { path: path.clone() }),
        FsOp::CreateDir { path } => Some(FsOp::RemoveDir { path: path.clone() }),
        FsOp::RemoveFile { path } => {
            // Cannot reverse a permanent file removal (content lost).
            // This should not be reached in normal flow — permanent deletes
            // are not pushed to undo history.
            tracing::warn!(?path, "cannot reverse permanent file removal");
            None
        }
        FsOp::RemoveDir { path } => {
            tracing::warn!(?path, "cannot reverse permanent directory removal");
            None
        }
        FsOp::CreateSymlink { link, .. } => {
            // Undo symlink creation = remove the symlink file.
            Some(FsOp::RemoveFile { path: link.clone() })
        }
        FsOp::CreateHardlink { link, .. } => {
            // Undo hard link creation = remove the hard link.
            Some(FsOp::RemoveFile { path: link.clone() })
        }
    }
}

/// Convenience: build an `UndoOp` from a forward operation.
///
/// Returns `None` if the operation has no meaningful reverse.
pub fn build_undo_op(forward: FsOp) -> Option<UndoOp> {
    let reverse = reverse_op(&forward)?;
    Some(UndoOp { forward, reverse })
}

/// Convenience: determine the appropriate reverse for a copied path.
///
/// Checks whether `dst` is a file or directory at call time
/// to choose `RemoveFile` vs `RemoveDir`.
pub fn reverse_for_copy(dst: &Path) -> FsOp {
    if dst.is_dir() {
        FsOp::RemoveDir { path: dst.to_path_buf() }
    } else {
        FsOp::RemoveFile { path: dst.to_path_buf() }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    fn sample_group(desc: &str) -> OpGroup {
        OpGroup {
            description: desc.to_string(),
            ops: vec![UndoOp {
                forward: FsOp::CreateFile { path: PathBuf::from("/tmp/test") },
                reverse: FsOp::RemoveFile { path: PathBuf::from("/tmp/test") },
            }],
        }
    }

    #[rstest]
    fn new_history_is_empty() {
        let history = UndoHistory::new(10);
        assert_that!(history.can_undo(), eq(false));
        assert_that!(history.can_redo(), eq(false));
    }

    #[rstest]
    fn push_enables_undo() {
        let mut history = UndoHistory::new(10);
        history.push(sample_group("create"));
        assert_that!(history.can_undo(), eq(true));
        assert_that!(history.can_redo(), eq(false));
    }

    #[rstest]
    fn push_clears_redo_stack() {
        let mut history = UndoHistory::new(10);
        history.push(sample_group("first"));

        // Simulate undo manually (move from undo to redo).
        let group = history.undo_stack.pop_back().unwrap();
        history.redo_stack.push_back(group);
        assert_that!(history.can_redo(), eq(true));

        // New push should clear redo.
        history.push(sample_group("second"));
        assert_that!(history.can_redo(), eq(false));
    }

    #[rstest]
    fn evict_oldest_when_over_capacity() {
        let mut history = UndoHistory::new(2);
        history.push(sample_group("first"));
        history.push(sample_group("second"));
        history.push(sample_group("third"));

        assert_that!(history.undo_stack.len(), eq(2));
        assert_eq!(history.undo_stack[0].description, "second");
        assert_eq!(history.undo_stack[1].description, "third");
    }

    #[rstest]
    fn undo_redo_cycle_with_real_fs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let file_path = tmp.path().join("test.txt");

        // Create the file.
        std::fs::write(&file_path, "hello").unwrap();

        let mut history = UndoHistory::new(10);
        history.push(OpGroup {
            description: "Created test.txt".to_string(),
            ops: vec![UndoOp {
                forward: FsOp::CreateFile { path: file_path.clone() },
                reverse: FsOp::RemoveFile { path: file_path.clone() },
            }],
        });

        // Undo: remove the file.
        let group = history.undo().unwrap().unwrap();
        for op in &group.ops {
            super::super::executor::execute(&op.reverse).unwrap();
        }
        assert_that!(file_path.exists(), eq(false));
        assert_that!(history.can_undo(), eq(false));
        assert_that!(history.can_redo(), eq(true));

        // Redo: recreate the file.
        let group = history.redo().unwrap().unwrap();
        for op in &group.ops {
            super::super::executor::execute(&op.forward).unwrap();
        }
        assert_that!(file_path.exists(), eq(true));
        assert_that!(history.can_undo(), eq(true));
        assert_that!(history.can_redo(), eq(false));
    }

    #[rstest]
    fn undo_fails_on_precondition_violation() {
        let mut history = UndoHistory::new(10);
        // Reverse op tries to remove a non-existent file.
        history.push(OpGroup {
            description: "test".to_string(),
            ops: vec![UndoOp {
                forward: FsOp::CreateFile { path: PathBuf::from("/nonexistent/file.txt") },
                reverse: FsOp::RemoveFile { path: PathBuf::from("/nonexistent/file.txt") },
            }],
        });

        let result = history.undo();
        assert!(result.is_err());
        // Group stays on undo stack (not moved to redo).
        assert_that!(history.can_undo(), eq(true));
        assert_that!(history.can_redo(), eq(false));
    }

    #[rstest]
    fn undo_returns_none_when_empty() {
        let mut history = UndoHistory::new(10);
        let result = history.undo().unwrap();
        assert!(result.is_none());
    }

    #[rstest]
    fn redo_returns_none_when_empty() {
        let mut history = UndoHistory::new(10);
        let result = history.redo().unwrap();
        assert!(result.is_none());
    }

    /// Source and destination of a `Move`.
    fn move_paths(op: &FsOp) -> Option<(&PathBuf, &PathBuf)> {
        match op {
            FsOp::Move { src, dst } => Some((src, dst)),
            _ => None,
        }
    }

    const DAY: Duration = Duration::from_hours(24);

    /// A delete-into-trash group whose trash entry name encodes `trashed_at`.
    fn trash_group(desc: &str, trash_dir: &Path, trashed_at: SystemTime) -> OpGroup {
        let original = PathBuf::from(format!("/work/{desc}"));
        let trashed = trash::trash_path_at(trash_dir, &original, trashed_at);
        OpGroup {
            description: desc.to_string(),
            ops: vec![UndoOp {
                forward: FsOp::Move { src: original.clone(), dst: trashed.clone() },
                reverse: FsOp::Move { src: trashed, dst: original },
            }],
        }
    }

    #[rstest]
    fn prune_expired_trash_drops_expired_group_and_everything_older() {
        let trash_dir = Path::new("/state/trev/trash");
        let now = SystemTime::now();
        let mut history = UndoHistory::new(10);
        history.push(sample_group("oldest"));
        history.push(trash_group("expired", trash_dir, now - DAY * 31));
        history.push(sample_group("rename"));
        history.push(trash_group("recent", trash_dir, now - DAY));

        let pruned = history.prune_expired_trash(trash_dir, 30, now);

        assert_that!(pruned, eq(2));
        let (undo, _) = history.export_stacks();
        let names: Vec<_> = undo.iter().map(|g| g.description.as_str()).collect();
        assert_that!(names, elements_are![eq(&"rename"), eq(&"recent")]);
    }

    #[rstest]
    fn prune_expired_trash_leaves_trash_entries_to_the_cleanup() {
        let tmp = tempfile::TempDir::new().unwrap();
        let now = SystemTime::now();
        let mut history = UndoHistory::new(10);
        let expired = trash_group("expired", tmp.path(), now - DAY * 31);
        let trashed = move_paths(&expired.ops[0].reverse).unwrap().0.clone();
        std::fs::write(&trashed, "x").unwrap();
        history.push(expired);

        history.prune_expired_trash(tmp.path(), 30, now);

        assert_that!(history.can_undo(), eq(false));
        assert!(trashed.exists());
    }

    #[rstest]
    fn prune_expired_trash_disabled_with_zero_days() {
        let trash_dir = Path::new("/state/trev/trash");
        let now = SystemTime::now();
        let mut history = UndoHistory::new(10);
        history.push(trash_group("ancient", trash_dir, now - DAY * 3650));

        assert_that!(history.prune_expired_trash(trash_dir, 0, now), eq(0));
        assert_that!(history.can_undo(), eq(true));
    }

    #[rstest]
    fn retarget_trash_move_assigns_fresh_trash_name() {
        let trash_dir = Path::new("/state/trev/trash");
        let original = PathBuf::from("/work/report.pdf");
        let old = trash_dir.join("1000000000000000000_report.pdf");
        let mut op = UndoOp {
            forward: FsOp::Move { src: original.clone(), dst: old.clone() },
            reverse: FsOp::Move { src: old.clone(), dst: original.clone() },
        };

        retarget_trash_move(&mut op, trash_dir);

        let (src, dst) = move_paths(&op.forward).unwrap();
        assert_that!(src, eq(&original));
        assert!(dst.starts_with(trash_dir) && *dst != old);
        assert!(!trash::is_expired(dst, 1, SystemTime::now()));
        assert_eq!(op.reverse, FsOp::Move { src: dst.clone(), dst: original });
    }

    #[rstest]
    fn retarget_trash_move_gives_same_named_files_distinct_paths() {
        let trash_dir = Path::new("/state/trev/trash");
        let mut ops: Vec<UndoOp> = ["/a/x.txt", "/b/x.txt"]
            .iter()
            .map(|original| {
                let original = PathBuf::from(original);
                let old = trash_dir.join("1000000000000000000_x.txt");
                UndoOp {
                    forward: FsOp::Move { src: original.clone(), dst: old.clone() },
                    reverse: FsOp::Move { src: old, dst: original },
                }
            })
            .collect();

        for op in &mut ops {
            retarget_trash_move(op, trash_dir);
        }

        let dst_a = move_paths(&ops[0].forward).unwrap().1;
        let dst_b = move_paths(&ops[1].forward).unwrap().1;
        assert_ne!(dst_a, dst_b);
    }

    #[rstest]
    fn retarget_trash_move_ignores_other_moves() {
        let trash_dir = Path::new("/state/trev/trash");
        let forward = FsOp::Move { src: PathBuf::from("/a"), dst: PathBuf::from("/b") };
        let mut op = UndoOp {
            forward: forward.clone(),
            reverse: FsOp::Move { src: PathBuf::from("/b"), dst: PathBuf::from("/a") },
        };

        retarget_trash_move(&mut op, trash_dir);

        assert_eq!(op.forward, forward);
    }

    #[rstest]
    fn reverse_op_for_copy_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dst = tmp.path().join("copied.txt");
        std::fs::write(&dst, "data").unwrap();

        let rev = reverse_op(&FsOp::Copy { src: PathBuf::from("/src"), dst: dst.clone() });
        assert_eq!(rev, Some(FsOp::RemoveFile { path: dst }));
    }

    #[rstest]
    fn reverse_op_for_move() {
        let rev = reverse_op(&FsOp::Move { src: PathBuf::from("/a"), dst: PathBuf::from("/b") });
        assert_eq!(rev, Some(FsOp::Move { src: PathBuf::from("/b"), dst: PathBuf::from("/a") }));
    }

    #[rstest]
    fn reverse_op_for_create_file() {
        let rev = reverse_op(&FsOp::CreateFile { path: PathBuf::from("/a.txt") });
        assert_eq!(rev, Some(FsOp::RemoveFile { path: PathBuf::from("/a.txt") }));
    }

    #[rstest]
    fn reverse_op_for_remove_returns_none() {
        let rev = reverse_op(&FsOp::RemoveFile { path: PathBuf::from("/a.txt") });
        assert!(rev.is_none());
    }

    #[rstest]
    fn build_undo_op_returns_pair() {
        let op = build_undo_op(FsOp::CreateFile { path: PathBuf::from("/a") });
        assert!(op.is_some());
        let op = op.unwrap();
        assert_eq!(op.forward, FsOp::CreateFile { path: PathBuf::from("/a") });
        assert_eq!(op.reverse, FsOp::RemoveFile { path: PathBuf::from("/a") });
    }

    #[rstest]
    fn reverse_op_for_create_symlink() {
        let rev = reverse_op(&FsOp::CreateSymlink {
            target: PathBuf::from("/target"),
            link: PathBuf::from("/link"),
        });
        assert_eq!(rev, Some(FsOp::RemoveFile { path: PathBuf::from("/link") }));
    }

    #[rstest]
    fn reverse_op_for_create_hardlink() {
        let rev = reverse_op(&FsOp::CreateHardlink {
            original: PathBuf::from("/original"),
            link: PathBuf::from("/link"),
        });
        assert_eq!(rev, Some(FsOp::RemoveFile { path: PathBuf::from("/link") }));
    }

    #[rstest]
    fn validate_precondition_rejects_existing_symlink_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let existing = tmp.path().join("exists.txt");
        std::fs::write(&existing, "data").unwrap();

        let result = validate_op_precondition(&FsOp::CreateSymlink {
            target: PathBuf::from("/target"),
            link: existing,
        });
        assert!(result.is_err());
    }

    #[rstest]
    fn validate_precondition_accepts_nonexistent_symlink_path() {
        let result = validate_op_precondition(&FsOp::CreateSymlink {
            target: PathBuf::from("/target"),
            link: PathBuf::from("/nonexistent/link"),
        });
        assert!(result.is_ok());
    }

    #[rstest]
    fn validate_precondition_rejects_existing_hardlink_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let existing = tmp.path().join("exists.txt");
        std::fs::write(&existing, "data").unwrap();

        let result = validate_op_precondition(&FsOp::CreateHardlink {
            original: PathBuf::from("/original"),
            link: existing,
        });
        assert!(result.is_err());
    }

    #[rstest]
    fn export_and_from_stacks_roundtrip() {
        let mut history = UndoHistory::new(10);
        history.push(sample_group("first"));
        history.push(sample_group("second"));

        // Simulate an undo to populate redo stack.
        let group = history.undo_stack.pop_back().unwrap();
        history.redo_stack.push_back(group);

        let (undo, redo) = history.export_stacks();
        assert_that!(undo.len(), eq(1));
        assert_that!(redo.len(), eq(1));
        assert_eq!(undo[0].description, "first");
        assert_eq!(redo[0].description, "second");

        // Reconstruct.
        let restored = UndoHistory::from_stacks(undo, redo, 10);
        assert_that!(restored.can_undo(), eq(true));
        assert_that!(restored.can_redo(), eq(true));

        let (undo2, redo2) = restored.export_stacks();
        assert_eq!(undo2[0].description, "first");
        assert_eq!(redo2[0].description, "second");
    }
}
