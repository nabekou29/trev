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
use super::fingerprint::Fingerprint;
use super::trash;

/// A single undoable file system operation with its reverse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoOp {
    /// Forward operation (what was done).
    pub forward: FsOp,
    /// Reverse operation (how to undo it).
    pub reverse: FsOp,
    /// Snapshot of the entry the last executed direction produced.
    ///
    /// The next undo (or redo) consumes that same path, and refuses to touch
    /// it if it is no longer the entry trev left there. `None` for operations
    /// recorded by older versions, which skip this check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<Fingerprint>,
}

impl UndoOp {
    /// Build an `UndoOp` for a forward operation that has just been executed.
    ///
    /// Captures the fingerprint of the entry the forward operation produced.
    pub fn new(forward: FsOp, reverse: FsOp) -> Self {
        let fingerprint = produced_path(&forward).and_then(Fingerprint::of);
        Self { forward, reverse, fingerprint }
    }
}

/// A group of operations performed as a single user action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpGroup {
    /// Human-readable description for status messages.
    pub description: String,
    /// Individual operations in execution order.
    pub ops: Vec<UndoOp>,
}

/// Result of an undo or redo step.
#[derive(Debug)]
pub enum Step<'a> {
    /// The history in that direction is empty.
    Nothing,
    /// The group was applied and moved to the opposite stack.
    Done(&'a OpGroup),
    /// A precondition failed, so nothing was changed on disk.
    ///
    /// The group is removed from history. Entries it still references in the
    /// trash are left for the retention cleanup instead of being deleted, so
    /// a recoverable conflict never destroys data.
    Skipped {
        /// The removed group.
        group: OpGroup,
        /// Why the precondition failed.
        error: anyhow::Error,
    },
    /// Execution failed after some operations were applied.
    ///
    /// The group is removed from history; the applied operations are not
    /// rolled back.
    Failed {
        /// The removed group.
        group: OpGroup,
        /// The execution error.
        error: anyhow::Error,
    },
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

    /// Undo the most recent operation group by running its reverse operations
    /// through `exec`.
    ///
    /// Older groups stay undoable when this group is skipped or fails. Every
    /// later step re-checks its preconditions and, for operations recorded with
    /// a fingerprint, the identity of the entry it consumes, so it does not
    /// act on entries that changed in the meantime.
    pub fn undo(&mut self, exec: impl FnMut(&FsOp) -> Result<()>) -> Step<'_> {
        let Some(mut group) = self.undo_stack.pop_back() else {
            return Step::Nothing;
        };

        if let Err(error) =
            group.ops.iter().try_for_each(|op| validate_step(&op.reverse, op.fingerprint.as_ref()))
        {
            return Step::Skipped { group, error };
        }
        if let Err(error) = group.ops.iter().rev().map(|op| &op.reverse).try_for_each(exec) {
            return Step::Failed { group, error };
        }

        for op in &mut group.ops {
            op.fingerprint = produced_path(&op.reverse).and_then(Fingerprint::of);
        }
        self.redo_stack.push_back(group);
        self.redo_stack.back().map_or(Step::Nothing, Step::Done)
    }

    /// Redo the most recently undone operation group by running its forward
    /// operations through `exec`.
    ///
    /// Deletes into the custom trash get a fresh trash name, so their
    /// retention period starts again from the redo.
    pub fn redo(&mut self, exec: impl FnMut(&FsOp) -> Result<()>) -> Step<'_> {
        let Some(mut group) = self.redo_stack.pop_back() else {
            return Step::Nothing;
        };

        let trash_dir = trash::trash_dir();
        for op in &mut group.ops {
            retarget_trash_move(op, &trash_dir);
        }

        if let Err(error) =
            group.ops.iter().try_for_each(|op| validate_step(&op.forward, op.fingerprint.as_ref()))
        {
            return Step::Skipped { group, error };
        }
        if let Err(error) = group.ops.iter().map(|op| &op.forward).try_for_each(exec) {
            return Step::Failed { group, error };
        }

        for op in &mut group.ops {
            op.fingerprint = produced_path(&op.forward).and_then(Fingerprint::of);
        }
        self.undo_stack.push_back(group);
        self.undo_stack.back().map_or(Step::Nothing, Step::Done)
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

/// How strictly a consumed entry must match its recorded fingerprint.
#[derive(Debug, Clone, Copy)]
enum Match {
    /// Same inode; content may differ (the entry is moved, not destroyed).
    Entry,
    /// Same inode and unmodified content (the entry is deleted).
    Content,
}

/// The path an operation removes from its current location, if any.
fn consumed_path(op: &FsOp) -> Option<(&Path, Match)> {
    match op {
        FsOp::Move { src, .. } => Some((src, Match::Entry)),
        FsOp::RemoveFile { path } | FsOp::RemoveDir { path } => Some((path, Match::Content)),
        FsOp::Copy { .. }
        | FsOp::CreateFile { .. }
        | FsOp::CreateDir { .. }
        | FsOp::CreateSymlink { .. }
        | FsOp::CreateHardlink { .. } => None,
    }
}

/// The path an operation creates, if any.
fn produced_path(op: &FsOp) -> Option<&Path> {
    match op {
        FsOp::Copy { dst, .. } | FsOp::Move { dst, .. } => Some(dst),
        FsOp::CreateFile { path } | FsOp::CreateDir { path } => Some(path),
        FsOp::CreateSymlink { link, .. } | FsOp::CreateHardlink { link, .. } => Some(link),
        FsOp::RemoveFile { .. } | FsOp::RemoveDir { .. } => None,
    }
}

/// Validate an operation's preconditions and the identity of the entry it consumes.
fn validate_step(op: &FsOp, expected: Option<&Fingerprint>) -> Result<()> {
    validate_op_precondition(op)?;

    if let (Some((path, mode)), Some(expected)) = (consumed_path(op), expected) {
        let matches = Fingerprint::of(path).is_some_and(|current| match mode {
            Match::Entry => expected.same_entry(&current),
            Match::Content => expected.same_content(&current),
        });
        if !matches {
            bail!("Changed outside trev since the operation: {}", path.display());
        }
    }
    Ok(())
}

/// Whether anything exists at `path`, without following symlinks.
fn path_exists(path: &Path) -> bool {
    path.symlink_metadata().is_ok()
}

/// Validate that a file system operation's preconditions are met.
fn validate_op_precondition(op: &FsOp) -> Result<()> {
    match op {
        FsOp::Copy { src, dst } | FsOp::Move { src, dst } => {
            if !path_exists(src) {
                bail!("Source does not exist: {}", src.display());
            }
            if path_exists(dst) {
                bail!("Destination already exists: {}", dst.display());
            }
        }
        FsOp::CreateFile { path } | FsOp::CreateDir { path } => {
            if path_exists(path) {
                bail!("Path already exists: {}", path.display());
            }
        }
        FsOp::RemoveFile { path } | FsOp::RemoveDir { path } => {
            if !path_exists(path) {
                bail!("Path does not exist: {}", path.display());
            }
        }
        FsOp::CreateSymlink { link, .. } | FsOp::CreateHardlink { link, .. } => {
            if path_exists(link) {
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

/// Whether `path` is a directory, without following symlinks.
fn is_real_dir(path: &Path) -> bool {
    path.symlink_metadata().is_ok_and(|m| m.is_dir())
}

/// Build the reverse operation for a forward `FsOp`.
///
/// Returns `None` for operations that cannot be meaningfully reversed
/// (caller should handle this by not adding to undo history).
pub fn reverse_op(forward: &FsOp) -> Option<FsOp> {
    match forward {
        FsOp::Copy { dst, .. } => Some(reverse_for_copy(dst)),
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

/// Convenience: build an `UndoOp` from a forward operation that has just been executed.
///
/// Returns `None` if the operation has no meaningful reverse.
pub fn build_undo_op(forward: FsOp) -> Option<UndoOp> {
    let reverse = reverse_op(&forward)?;
    Some(UndoOp::new(forward, reverse))
}

/// Convenience: determine the appropriate reverse for a copied path.
///
/// Checks whether `dst` is a directory at call time (without following
/// symlinks) to choose `RemoveFile` vs `RemoveDir`.
pub fn reverse_for_copy(dst: &Path) -> FsOp {
    if is_real_dir(dst) {
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

    use super::super::executor::execute;
    use super::*;

    /// Build a group from `(forward, reverse)` pairs without fingerprints.
    fn group(desc: &str, ops: Vec<(FsOp, FsOp)>) -> OpGroup {
        OpGroup {
            description: desc.to_string(),
            ops: ops
                .into_iter()
                .map(|(forward, reverse)| UndoOp { forward, reverse, fingerprint: None })
                .collect(),
        }
    }

    fn sample_group(desc: &str) -> OpGroup {
        group(
            desc,
            vec![(
                FsOp::CreateFile { path: PathBuf::from("/tmp/test") },
                FsOp::RemoveFile { path: PathBuf::from("/tmp/test") },
            )],
        )
    }

    /// Execute `forward` and push it as a single-op group, as the handlers do.
    fn run_and_push(history: &mut UndoHistory, desc: &str, forward: FsOp) {
        execute(&forward).unwrap();
        let op = build_undo_op(forward).unwrap();
        history.push(OpGroup { description: desc.to_string(), ops: vec![op] });
    }

    fn description(step: &Step<'_>) -> Option<String> {
        match step {
            Step::Done(group) => Some(group.description.clone()),
            Step::Nothing | Step::Skipped { .. } | Step::Failed { .. } => None,
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
        let mut history = UndoHistory::new(10);
        run_and_push(
            &mut history,
            "Created test.txt",
            FsOp::CreateFile { path: file_path.clone() },
        );

        // Undo: remove the file.
        let step = history.undo(execute);
        assert_that!(description(&step), some(eq("Created test.txt")));
        assert_that!(file_path.exists(), eq(false));
        assert_that!(history.can_undo(), eq(false));
        assert_that!(history.can_redo(), eq(true));

        // Redo: recreate the file.
        let step = history.redo(execute);
        assert_that!(description(&step), some(eq("Created test.txt")));
        assert_that!(file_path.exists(), eq(true));
        assert_that!(history.can_undo(), eq(true));
        assert_that!(history.can_redo(), eq(false));
    }

    #[rstest]
    fn undo_returns_nothing_when_empty() {
        let mut history = UndoHistory::new(10);
        assert!(matches!(history.undo(execute), Step::Nothing));
    }

    #[rstest]
    fn redo_returns_nothing_when_empty() {
        let mut history = UndoHistory::new(10);
        assert!(matches!(history.redo(execute), Step::Nothing));
    }

    #[rstest]
    fn undo_skips_group_on_precondition_violation_without_touching_disk() {
        let tmp = tempfile::TempDir::new().unwrap();
        let a = tmp.path().join("a.txt");
        let b = tmp.path().join("b.txt");
        let mut history = UndoHistory::new(10);
        std::fs::write(&a, "data").unwrap();
        run_and_push(&mut history, "Rename", FsOp::Move { src: a.clone(), dst: b.clone() });
        // Something recreates the original name.
        std::fs::write(&a, "new").unwrap();

        let step = history.undo(execute);

        assert!(matches!(step, Step::Skipped { .. }));
        assert_that!(std::fs::read_to_string(&a).unwrap(), eq("new"));
        assert_that!(std::fs::read_to_string(&b).unwrap(), eq("data"));
        assert_that!(history.can_undo(), eq(false));
        assert_that!(history.can_redo(), eq(false));
    }

    #[rstest]
    fn undo_after_skip_does_not_delete_recreated_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let notes = tmp.path().join("notes.md");
        let notes2 = tmp.path().join("notes2.md");
        let mut history = UndoHistory::new(10);
        run_and_push(&mut history, "Create", FsOp::CreateFile { path: notes.clone() });
        run_and_push(
            &mut history,
            "Rename",
            FsOp::Move { src: notes.clone(), dst: notes2.clone() },
        );
        // An editor recreates notes.md with user content.
        std::fs::write(&notes, "user content").unwrap();

        assert!(matches!(history.undo(execute), Step::Skipped { .. }));
        let step = history.undo(execute);

        assert!(matches!(step, Step::Skipped { .. }));
        assert_that!(std::fs::read_to_string(&notes).unwrap(), eq("user content"));
        assert!(notes2.exists());
    }

    #[rstest]
    fn undo_refuses_to_delete_modified_copy() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = tmp.path().join("src.txt");
        let dst = tmp.path().join("dst.txt");
        std::fs::write(&src, "data").unwrap();
        let mut history = UndoHistory::new(10);
        run_and_push(&mut history, "Copy", FsOp::Copy { src, dst: dst.clone() });
        std::fs::write(&dst, "edited after copy").unwrap();

        let step = history.undo(execute);

        assert!(matches!(step, Step::Skipped { .. }));
        assert_that!(std::fs::read_to_string(&dst).unwrap(), eq("edited after copy"));
    }

    #[rstest]
    fn undo_after_skip_reaches_older_independent_group() {
        let tmp = tempfile::TempDir::new().unwrap();
        let older = tmp.path().join("older.txt");
        let newer = tmp.path().join("newer.txt");
        let mut history = UndoHistory::new(10);
        run_and_push(&mut history, "Create older", FsOp::CreateFile { path: older.clone() });
        run_and_push(&mut history, "Create newer", FsOp::CreateFile { path: newer.clone() });
        std::fs::remove_file(&newer).unwrap();

        assert!(matches!(history.undo(execute), Step::Skipped { .. }));
        let step = history.undo(execute);

        assert_that!(description(&step), some(eq("Create older")));
        assert!(!older.exists());
    }

    #[rstest]
    fn redo_refuses_to_move_replaced_source() {
        let tmp = tempfile::TempDir::new().unwrap();
        let a = tmp.path().join("a.txt");
        let b = tmp.path().join("b.txt");
        std::fs::write(&a, "data").unwrap();
        let mut history = UndoHistory::new(10);
        run_and_push(&mut history, "Rename", FsOp::Move { src: a.clone(), dst: b.clone() });
        assert!(matches!(history.undo(execute), Step::Done(_)));
        // Replace the restored file with an unrelated one at the same path.
        std::fs::rename(&a, tmp.path().join("aside.txt")).unwrap();
        std::fs::write(&a, "unrelated").unwrap();

        let step = history.redo(execute);

        assert!(matches!(step, Step::Skipped { .. }));
        assert_that!(std::fs::read_to_string(&a).unwrap(), eq("unrelated"));
        assert!(!b.exists());
    }

    #[cfg(unix)]
    #[rstest]
    fn undo_restores_dangling_symlink_moved_into_trash() {
        let tmp = tempfile::TempDir::new().unwrap();
        let trash = tmp.path().join("trash");
        std::fs::create_dir_all(&trash).unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink("file.txt", &link).unwrap();
        let mut history = UndoHistory::new(10);
        let trashed = trash::trash_path_in(&trash, &link);
        run_and_push(&mut history, "Delete", FsOp::Move { src: link.clone(), dst: trashed });

        let step = history.undo(execute);

        assert!(matches!(step, Step::Done(_)));
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
    }

    #[rstest]
    fn undo_failure_during_execution_drops_group() {
        let mut history = UndoHistory::new(10);
        history.push(sample_group("older"));
        history.push(group(
            "test",
            vec![(
                FsOp::CreateDir { path: PathBuf::from("/nonexistent/dir") },
                FsOp::CreateDir { path: PathBuf::from("/nonexistent/dir") },
            )],
        ));

        let step = history.undo(|_| bail!("disk error"));

        assert!(matches!(step, Step::Failed { .. }));
        assert_that!(history.undo_stack.len(), eq(1));
        assert_that!(history.can_redo(), eq(false));
    }

    #[rstest]
    fn legacy_ops_without_fingerprint_still_undo() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("legacy.txt");
        std::fs::write(&path, "x").unwrap();
        let mut history = UndoHistory::new(10);
        history.push(group(
            "legacy",
            vec![(
                FsOp::CreateFile { path: path.clone() },
                FsOp::RemoveFile { path: path.clone() },
            )],
        ));

        assert!(matches!(history.undo(execute), Step::Done(_)));
        assert!(!path.exists());
    }

    #[rstest]
    fn legacy_session_json_without_fingerprint_deserializes() {
        let json =
            r#"{"forward":{"CreateFile":{"path":"/a"}},"reverse":{"RemoveFile":{"path":"/a"}}}"#;
        let op: UndoOp = serde_json::from_str(json).unwrap();
        assert_that!(op.fingerprint, none());
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
        group(
            desc,
            vec![(
                FsOp::Move { src: original.clone(), dst: trashed.clone() },
                FsOp::Move { src: trashed, dst: original },
            )],
        )
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
            fingerprint: None,
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
                    fingerprint: None,
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
            fingerprint: None,
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
