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
use crate::state::tree::{
    SortDirection,
    SortOrder,
};

/// Serializable session state for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Canonical root path this session belongs to.
    pub root_path: PathBuf,
    /// Last access timestamp (for expiry cleanup).
    pub last_accessed: SystemTime,
    /// Paths of expanded directories.
    #[serde(default)]
    pub expanded_paths: Vec<PathBuf>,
    /// Path the cursor was on (more robust than index).
    #[serde(default)]
    pub cursor_path: Option<PathBuf>,
    /// Selection buffer paths.
    #[serde(default)]
    pub selection_paths: Vec<PathBuf>,
    /// Selection buffer mode.
    #[serde(default)]
    pub selection_mode: Option<SelectionMode>,
    /// Undo stack.
    #[serde(default)]
    pub undo_stack: Vec<OpGroup>,
    /// Redo stack.
    #[serde(default)]
    pub redo_stack: Vec<OpGroup>,
    /// Scroll offset (first visible row) — deprecated, kept for old sessions.
    #[serde(default)]
    pub scroll_offset: Option<usize>,
    /// Cursor's visual row (on-screen row) for scroll position restoration.
    /// Computed as `cursor_index - scroll_offset` at save time.
    #[serde(default)]
    pub scroll_visual_row: Option<usize>,
    /// Whether hidden (dot) files were shown.
    #[serde(default)]
    pub show_hidden: Option<bool>,
    /// Whether gitignored files were shown.
    #[serde(default)]
    pub show_ignored: Option<bool>,
    /// Whether the preview panel was visible.
    #[serde(default)]
    pub show_preview: Option<bool>,
    /// Sort order at the time of save.
    #[serde(default)]
    pub sort_order: Option<SortOrder>,
    /// Sort direction at the time of save.
    #[serde(default)]
    pub sort_direction: Option<SortDirection>,
    /// Whether directories were sorted before files.
    #[serde(default)]
    pub directories_first: Option<bool>,
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

/// Display/sort settings to persist in the session.
#[derive(Debug, Clone, Copy)]
#[expect(clippy::struct_excessive_bools, reason = "mirrors AppState display flags")]
pub struct DisplaySettings {
    /// Whether hidden (dot) files are shown.
    pub show_hidden: bool,
    /// Whether gitignored files are shown.
    pub show_ignored: bool,
    /// Whether the preview panel is visible.
    pub show_preview: bool,
    /// Current sort order.
    pub sort_order: SortOrder,
    /// Current sort direction.
    pub sort_direction: SortDirection,
    /// Whether directories are sorted before files.
    pub directories_first: bool,
}

/// Build a `SessionState` from application state components.
pub fn build_session_state(
    root_path: &Path,
    expanded_paths: Vec<PathBuf>,
    cursor_path: Option<PathBuf>,
    selection: &SelectionBuffer,
    undo_history: &UndoHistory,
    scroll_visual_row: usize,
    display: &DisplaySettings,
) -> SessionState {
    let (selection_paths, selection_mode) = selection.export();
    let (undo_stack, redo_stack) = undo_history.export_stacks();

    SessionState {
        root_path: root_path.to_path_buf(),
        last_accessed: SystemTime::now(),
        expanded_paths,
        cursor_path,
        selection_paths,
        selection_mode: selection_mode.copied(),
        undo_stack,
        redo_stack,
        scroll_offset: None,
        scroll_visual_row: Some(scroll_visual_row),
        show_hidden: Some(display.show_hidden),
        show_ignored: Some(display.show_ignored),
        show_preview: Some(display.show_preview),
        sort_order: Some(display.sort_order),
        sort_direction: Some(display.sort_direction),
        directories_first: Some(display.directories_first),
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
    cleanup_expired_in(&session_dir()?, expiry_days)
}

/// Delete expired session files in the given directory.
///
/// Returns the number of deleted sessions.
fn cleanup_expired_in(dir: &Path, expiry_days: u64) -> Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }

    let expiry_duration = std::time::Duration::from_secs(expiry_days * 24 * 60 * 60);
    let now = SystemTime::now();
    let mut deleted = 0;

    let entries = std::fs::read_dir(dir).context("failed to read session directory")?;

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
            scroll_offset: None,
            scroll_visual_row: None,
            show_hidden: Some(true),
            show_ignored: None,
            show_preview: Some(false),
            sort_order: Some(SortOrder::Name),
            sort_direction: Some(SortDirection::Desc),
            directories_first: Some(false),
        };

        save(&state).unwrap();

        let restored = restore(&root).unwrap().unwrap();
        assert_eq!(restored.root_path, root);
        assert_that!(restored.expanded_paths.len(), eq(2));
        assert_eq!(restored.cursor_path, Some(file));
        assert_eq!(restored.selection_mode, Some(SelectionMode::Copy));
        assert_eq!(restored.show_hidden, Some(true));
        assert_eq!(restored.show_ignored, None);
        assert_eq!(restored.show_preview, Some(false));
        assert_eq!(restored.sort_order, Some(SortOrder::Name));
        assert_eq!(restored.sort_direction, Some(SortDirection::Desc));
        assert_eq!(restored.directories_first, Some(false));
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
            scroll_offset: None,
            scroll_visual_row: None,
            show_hidden: None,
            show_ignored: None,
            show_preview: None,
            sort_order: None,
            sort_direction: None,
            directories_first: None,
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

        let display = DisplaySettings {
            show_hidden: true,
            show_ignored: false,
            show_preview: true,
            sort_order: SortOrder::Size,
            sort_direction: SortDirection::Desc,
            directories_first: false,
        };

        let state = build_session_state(
            root,
            expanded.clone(),
            cursor.clone(),
            &selection,
            &undo,
            42,
            &display,
        );

        assert_eq!(state.root_path, root);
        assert_eq!(state.expanded_paths, expanded);
        assert_eq!(state.cursor_path, cursor);
        assert_eq!(state.selection_mode, Some(SelectionMode::Cut));
        assert_that!(state.selection_paths.len(), eq(1));
        assert_eq!(state.scroll_offset, None);
        assert_eq!(state.scroll_visual_row, Some(42));
        assert_eq!(state.show_hidden, Some(true));
        assert_eq!(state.show_ignored, Some(false));
        assert_eq!(state.show_preview, Some(true));
        assert_eq!(state.sort_order, Some(SortOrder::Size));
        assert_eq!(state.sort_direction, Some(SortDirection::Desc));
        assert_eq!(state.directories_first, Some(false));
    }

    /// Write a session file directly into a custom directory for isolated testing.
    fn save_session_in(dir: &Path, state: &SessionState) -> PathBuf {
        use sha2::Digest;

        std::fs::create_dir_all(dir).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(state.root_path.to_string_lossy().as_bytes());
        let hash = hasher.finalize();
        let hex = format!("{hash:x}");
        let file_path = dir.join(format!("{}.json", &hex[..16]));

        let json = serde_json::to_string_pretty(state).unwrap();
        std::fs::write(&file_path, json).unwrap();
        file_path
    }

    #[rstest]
    fn cleanup_expired_removes_old_sessions() {
        let tmp = tempfile::TempDir::new().unwrap();
        let session_dir = tmp.path().join("sessions");

        let state = SessionState {
            root_path: PathBuf::from("/test/old_project"),
            last_accessed: SystemTime::UNIX_EPOCH,
            expanded_paths: vec![],
            cursor_path: None,
            selection_paths: vec![],
            selection_mode: None,
            undo_stack: vec![],
            redo_stack: vec![],
            scroll_offset: None,
            scroll_visual_row: None,
            show_hidden: None,
            show_ignored: None,
            show_preview: None,
            sort_order: None,
            sort_direction: None,
            directories_first: None,
        };

        let file_path = save_session_in(&session_dir, &state);
        assert!(file_path.exists());

        // Cleanup with 1 day expiry — should remove the old session.
        let deleted = cleanup_expired_in(&session_dir, 1).unwrap();
        assert_that!(deleted, eq(1));
        assert!(!file_path.exists());
    }

    #[rstest]
    fn cleanup_expired_keeps_recent_sessions() {
        let tmp = tempfile::TempDir::new().unwrap();
        let session_dir = tmp.path().join("sessions");

        let state = SessionState {
            root_path: PathBuf::from("/test/recent_project"),
            last_accessed: SystemTime::now(),
            expanded_paths: vec![],
            cursor_path: None,
            selection_paths: vec![],
            selection_mode: None,
            undo_stack: vec![],
            redo_stack: vec![],
            scroll_offset: None,
            scroll_visual_row: None,
            show_hidden: None,
            show_ignored: None,
            show_preview: None,
            sort_order: None,
            sort_direction: None,
            directories_first: None,
        };

        let file_path = save_session_in(&session_dir, &state);
        assert!(file_path.exists());

        // Cleanup with 90 day expiry — should keep the recent session.
        let deleted = cleanup_expired_in(&session_dir, 90).unwrap();
        assert_that!(deleted, eq(0));
        assert!(file_path.exists());
    }

    #[rstest]
    fn restore_deserializes_minimal_session_with_only_required_fields() {
        // JSON with only the two required fields — all #[serde(default)] fields omitted.
        let json = r#"{
            "root_path": "/test/minimal",
            "last_accessed": { "secs_since_epoch": 1700000000, "nanos_since_epoch": 0 }
        }"#;

        let state: SessionState = serde_json::from_str(json).unwrap();

        assert_eq!(state.root_path, PathBuf::from("/test/minimal"));
        assert!(state.expanded_paths.is_empty());
        assert!(state.cursor_path.is_none());
        assert!(state.selection_paths.is_empty());
        assert!(state.selection_mode.is_none());
        assert!(state.undo_stack.is_empty());
        assert!(state.redo_stack.is_empty());
        assert!(state.scroll_offset.is_none());
        assert!(state.scroll_visual_row.is_none());
        assert!(state.show_hidden.is_none());
        assert!(state.show_ignored.is_none());
        assert!(state.show_preview.is_none());
        assert!(state.sort_order.is_none());
        assert!(state.sort_direction.is_none());
        assert!(state.directories_first.is_none());
    }
}
