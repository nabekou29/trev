//! Session state persistence: save and restore application state across restarts.
//!
//! Stores session state (expanded directories, cursor position, selection buffer,
//! undo history) per root path in a JSON file under the application data directory.

use std::path::{
    Path,
    PathBuf,
};
use std::time::SystemTime;

use anyhow::{
    Context as _,
    Result,
};
use serde::{
    Deserialize,
    Serialize,
};
use sha2::{
    Digest,
    Sha256,
};

use crate::file_op::selection::{
    SelectionBuffer,
    SelectionMode,
};
use crate::file_op::undo::{
    OpGroup,
    UndoHistory,
};

/// Serializable session state for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Canonical root path this session belongs to.
    pub root_path: PathBuf,
    /// Last access timestamp (for expiry cleanup).
    pub last_accessed: SystemTime,
    /// Paths of expanded directories.
    pub expanded_paths: Vec<PathBuf>,
    /// Path the cursor was on (more robust than index).
    pub cursor_path: Option<PathBuf>,
    /// Selection buffer paths.
    pub selection_paths: Vec<PathBuf>,
    /// Selection buffer mode.
    pub selection_mode: Option<SelectionMode>,
    /// Undo stack.
    pub undo_stack: Vec<OpGroup>,
    /// Redo stack.
    pub redo_stack: Vec<OpGroup>,
}

/// Get the session storage directory path.
///
/// Returns `{data_dir}/trev/sessions/`.
fn session_dir() -> Result<PathBuf> {
    let data_dir = dirs::data_dir().context("could not determine data directory")?;
    Ok(data_dir.join("trev").join("sessions"))
}

/// Compute the session file path for a given root path.
///
/// Uses the first 16 hex characters of the SHA-256 hash of the root path.
fn session_path(root: &Path) -> Result<PathBuf> {
    let mut hasher = Sha256::new();
    hasher.update(root.to_string_lossy().as_bytes());
    let hash = hasher.finalize();
    let hex = format!("{hash:x}");
    let short_hash = &hex[..16];
    Ok(session_dir()?.join(format!("{short_hash}.json")))
}

/// Save session state to disk with atomic write.
///
/// Writes to a temporary file first, then renames to the final path.
pub fn save(state: &SessionState) -> Result<()> {
    let path = session_path(&state.root_path)?;
    let dir = path.parent().context("session path has no parent")?;
    std::fs::create_dir_all(dir).context("failed to create session directory")?;

    let json = serde_json::to_string_pretty(state).context("failed to serialize session state")?;

    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, json).context("failed to write temporary session file")?;
    std::fs::rename(&tmp_path, &path).context("failed to rename session file")?;

    Ok(())
}

/// Build a `SessionState` from application state components.
pub fn build_session_state(
    root_path: &Path,
    expanded_paths: Vec<PathBuf>,
    cursor_path: Option<PathBuf>,
    selection: &SelectionBuffer,
    undo_history: &UndoHistory,
) -> SessionState {
    let (selection_paths, selection_mode) = selection.export();
    let (undo_stack, redo_stack) = undo_history.export_stacks();

    SessionState {
        root_path: root_path.to_path_buf(),
        last_accessed: SystemTime::now(),
        expanded_paths,
        cursor_path,
        selection_paths,
        selection_mode: selection_mode.cloned(),
        undo_stack: undo_stack.to_vec(),
        redo_stack: redo_stack.to_vec(),
    }
}

/// Restore session state from disk.
///
/// Returns `None` if no session file exists for the given root path.
/// Filters out expanded paths that no longer exist on the filesystem.
pub fn restore(root_path: &Path) -> Result<Option<SessionState>> {
    let path = session_path(root_path)?;
    if !path.exists() {
        return Ok(None);
    }

    let json = std::fs::read_to_string(&path).context("failed to read session file")?;
    let mut state: SessionState =
        serde_json::from_str(&json).context("failed to deserialize session state")?;

    // Filter out expanded paths that no longer exist.
    state.expanded_paths.retain(|p| p.exists());

    // Clear cursor path if it no longer exists.
    if let Some(ref cp) = state.cursor_path
        && !cp.exists()
    {
        state.cursor_path = None;
    }

    Ok(Some(state))
}

/// Delete expired session files.
///
/// Returns the number of deleted sessions.
pub fn cleanup_expired(expiry_days: u64) -> Result<usize> {
    let dir = session_dir()?;
    if !dir.exists() {
        return Ok(0);
    }

    let expiry_duration = std::time::Duration::from_secs(expiry_days * 24 * 60 * 60);
    let now = SystemTime::now();
    let mut deleted = 0;

    let entries = std::fs::read_dir(&dir).context("failed to read session directory")?;

    for entry in entries {
        let entry = entry.context("failed to read directory entry")?;
        let path = entry.path();

        if path.extension().is_some_and(|e| e == "json")
            && let Ok(json) = std::fs::read_to_string(&path)
            && let Ok(state) = serde_json::from_str::<SessionState>(&json)
        {
            let elapsed = now.duration_since(state.last_accessed).unwrap_or_default();
            if elapsed > expiry_duration {
                if let Err(e) = std::fs::remove_file(&path) {
                    tracing::warn!(%e, ?path, "failed to delete expired session");
                } else {
                    deleted += 1;
                }
            }
        }
    }

    Ok(deleted)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    #[rstest]
    fn session_path_is_deterministic() {
        let path1 = session_path(Path::new("/test/root")).unwrap();
        let path2 = session_path(Path::new("/test/root")).unwrap();
        assert_eq!(path1, path2);
    }

    #[rstest]
    fn session_path_differs_for_different_roots() {
        let path1 = session_path(Path::new("/test/root1")).unwrap();
        let path2 = session_path(Path::new("/test/root2")).unwrap();
        assert!(path1 != path2);
    }

    #[rstest]
    fn session_path_ends_with_json() {
        let path = session_path(Path::new("/test/root")).unwrap();
        assert_that!(path.extension().unwrap().to_str().unwrap(), eq("json"));
    }

    #[rstest]
    fn save_and_restore_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("project");
        std::fs::create_dir_all(&root).unwrap();

        let sub = root.join("subdir");
        std::fs::create_dir_all(&sub).unwrap();

        let file = root.join("file.txt");
        std::fs::write(&file, "hello").unwrap();

        let state = SessionState {
            root_path: root.clone(),
            last_accessed: SystemTime::now(),
            expanded_paths: vec![root.clone(), sub],
            cursor_path: Some(file.clone()),
            selection_paths: vec![file.clone()],
            selection_mode: Some(SelectionMode::Copy),
            undo_stack: vec![],
            redo_stack: vec![],
        };

        save(&state).unwrap();

        let restored = restore(&root).unwrap().unwrap();
        assert_eq!(restored.root_path, root);
        assert_that!(restored.expanded_paths.len(), eq(2));
        assert_eq!(restored.cursor_path, Some(file));
        assert_eq!(restored.selection_mode, Some(SelectionMode::Copy));
    }

    #[rstest]
    fn restore_returns_none_when_no_session() {
        let result = restore(Path::new("/nonexistent/path/that/surely/does/not/exist")).unwrap();
        assert!(result.is_none());
    }

    #[rstest]
    fn restore_filters_nonexistent_expanded_paths() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("project");
        std::fs::create_dir_all(&root).unwrap();

        let state = SessionState {
            root_path: root.clone(),
            last_accessed: SystemTime::now(),
            expanded_paths: vec![root.clone(), PathBuf::from("/does/not/exist")],
            cursor_path: Some(PathBuf::from("/also/gone")),
            selection_paths: vec![],
            selection_mode: None,
            undo_stack: vec![],
            redo_stack: vec![],
        };

        save(&state).unwrap();

        let restored = restore(&root).unwrap().unwrap();
        // Non-existent path filtered out.
        assert_that!(restored.expanded_paths.len(), eq(1));
        assert_eq!(restored.expanded_paths[0], root);
        // Non-existent cursor path cleared.
        assert!(restored.cursor_path.is_none());
    }

    #[rstest]
    fn build_session_state_extracts_components() {
        let root = Path::new("/test/root");
        let expanded = vec![PathBuf::from("/test/root"), PathBuf::from("/test/root/sub")];
        let cursor = Some(PathBuf::from("/test/root/file.txt"));

        let mut selection = SelectionBuffer::new();
        selection.set(vec![PathBuf::from("/test/root/a")], SelectionMode::Cut);

        let undo = UndoHistory::new(10);

        let state = build_session_state(root, expanded.clone(), cursor.clone(), &selection, &undo);

        assert_eq!(state.root_path, root);
        assert_eq!(state.expanded_paths, expanded);
        assert_eq!(state.cursor_path, cursor);
        assert_eq!(state.selection_mode, Some(SelectionMode::Cut));
        assert_that!(state.selection_paths.len(), eq(1));
    }

    #[rstest]
    fn cleanup_expired_removes_old_sessions() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("old_project");
        std::fs::create_dir_all(&root).unwrap();

        // Create a session with last_accessed set far in the past.
        let old_time = SystemTime::UNIX_EPOCH;
        let state = SessionState {
            root_path: root.clone(),
            last_accessed: old_time,
            expanded_paths: vec![],
            cursor_path: None,
            selection_paths: vec![],
            selection_mode: None,
            undo_stack: vec![],
            redo_stack: vec![],
        };

        save(&state).unwrap();

        // Verify file exists.
        let path = session_path(&root).unwrap();
        assert!(path.exists());

        // Cleanup with 1 day expiry — should remove the old session.
        let deleted = cleanup_expired(1).unwrap();
        assert_that!(deleted, ge(1));
        assert!(!path.exists());
    }

    #[rstest]
    fn cleanup_expired_keeps_recent_sessions() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("recent_project");
        std::fs::create_dir_all(&root).unwrap();

        let state = SessionState {
            root_path: root.clone(),
            last_accessed: SystemTime::now(),
            expanded_paths: vec![],
            cursor_path: None,
            selection_paths: vec![],
            selection_mode: None,
            undo_stack: vec![],
            redo_stack: vec![],
        };

        save(&state).unwrap();

        let path = session_path(&root).unwrap();
        assert!(path.exists());

        // Cleanup with 90 day expiry — should keep the recent session.
        let deleted = cleanup_expired(90).unwrap();
        // Our session should not be deleted (it was just created).
        assert!(path.exists());
        // Note: other expired sessions from previous tests might also be deleted,
        // so we only assert our file still exists.
        let _ = deleted;
    }
}
