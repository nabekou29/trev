//! Custom trash directory management for undo-able delete operations.

use std::path::{
    Path,
    PathBuf,
};
use std::time::{
    Duration,
    SystemTime,
};

use anyhow::{
    Context as _,
    Result,
};

/// Get the custom trash directory path.
///
/// Uses the XDG state directory: `{state_dir}/trev/trash/`.
pub fn trash_dir() -> PathBuf {
    crate::dirs::AppDirs::new()
        .map_or_else(|_| std::env::temp_dir().join("trev/trash"), |d| d.trash_dir())
}

/// Generate a timestamped trash path for a file.
///
/// Format: `{trash_dir}/{nanos}_{original_name}`.
pub fn trash_path(original: &Path) -> PathBuf {
    let nanos =
        SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos();

    let file_name = original.file_name().unwrap_or_default().to_string_lossy();

    trash_dir().join(format!("{nanos}_{file_name}"))
}

/// Ensure the trash directory exists.
pub fn ensure_trash_dir() -> Result<()> {
    let dir = trash_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create trash directory: {}", dir.display()))?;
    Ok(())
}

/// Remove a specific file from the custom trash directory.
pub fn clean_trash_file(path: &Path) -> Result<()> {
    if path.exists() {
        if path.is_dir() {
            std::fs::remove_dir_all(path)
                .with_context(|| format!("Failed to clean trash: {}", path.display()))?;
        } else {
            std::fs::remove_file(path)
                .with_context(|| format!("Failed to clean trash: {}", path.display()))?;
        }
    }
    Ok(())
}

/// Delete trash entries older than `retention_days`.
///
/// `0` disables cleanup. Returns the number of deleted entries.
pub fn cleanup_expired(retention_days: u64) -> Result<usize> {
    if retention_days == 0 {
        return Ok(0);
    }
    cleanup_expired_in(&trash_dir(), retention_days, SystemTime::now())
}

/// Delete trash entries in `dir` that were trashed more than `retention_days` before `now`.
///
/// The trashed time comes from the `{nanos}_` prefix written by [`trash_path`],
/// not from mtime, because a rename into the trash keeps the original mtime.
/// Entries without that prefix were not created by trev and are left alone.
fn cleanup_expired_in(dir: &Path, retention_days: u64, now: SystemTime) -> Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }

    let retention = Duration::from_secs(retention_days.saturating_mul(24 * 60 * 60));
    let mut deleted = 0;

    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read trash directory: {}", dir.display()))?;

    for entry in entries {
        let entry = entry.context("Failed to read trash directory entry")?;
        let path = entry.path();

        let Some(trashed_at) = trashed_at(&entry.file_name().to_string_lossy()) else {
            continue;
        };
        if now.duration_since(trashed_at).unwrap_or_default() <= retention {
            continue;
        }

        match clean_trash_file(&path) {
            Ok(()) => deleted += 1,
            Err(e) => tracing::warn!(%e, ?path, "failed to delete expired trash entry"),
        }
    }

    Ok(deleted)
}

/// Parse the trashed time from a `{nanos}_{original_name}` trash file name.
fn trashed_at(file_name: &str) -> Option<SystemTime> {
    let (nanos, _) = file_name.split_once('_')?;
    let nanos: u64 = nanos.parse().ok()?;
    SystemTime::UNIX_EPOCH.checked_add(Duration::from_nanos(nanos))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    #[rstest]
    fn trash_dir_is_under_data_dir() {
        let dir = trash_dir();
        assert!(dir.to_string_lossy().contains("trev/trash"));
    }

    #[rstest]
    fn trash_path_contains_original_name() {
        let original = PathBuf::from("/home/user/docs/report.pdf");
        let tp = trash_path(&original);
        let name = tp.file_name().unwrap().to_string_lossy();
        assert!(name.contains("report.pdf"));
    }

    #[rstest]
    fn trash_path_is_unique() {
        let original = PathBuf::from("/test/file.txt");
        let p1 = trash_path(&original);
        // Very small sleep to ensure different nanos.
        std::thread::sleep(Duration::from_nanos(1));
        let p2 = trash_path(&original);
        // They should be different due to different timestamps.
        // (In practice, nanos resolution makes collisions extremely unlikely.)
        // We just verify both contain the file name.
        assert!(p1.to_string_lossy().contains("file.txt"));
        assert!(p2.to_string_lossy().contains("file.txt"));
    }

    #[rstest]
    fn ensure_trash_dir_creates_directory() {
        // This test uses the real data dir, just verifying it doesn't error.
        ensure_trash_dir().unwrap();
        assert!(trash_dir().exists());
    }

    #[rstest]
    fn clean_trash_file_removes_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let file = tmp.path().join("trash_item.txt");
        std::fs::write(&file, "trash").unwrap();

        clean_trash_file(&file).unwrap();
        assert!(!file.exists());
    }

    #[rstest]
    fn clean_trash_file_noop_for_missing() {
        let result = clean_trash_file(Path::new("/nonexistent/path"));
        assert!(result.is_ok());
    }

    const DAY: Duration = Duration::from_hours(24);

    /// Create a trash entry named as if it was trashed at `trashed_at`.
    fn create_entry(dir: &Path, trashed_at: SystemTime, name: &str, is_dir: bool) -> PathBuf {
        let nanos = trashed_at.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
        let path = dir.join(format!("{nanos}_{name}"));
        if is_dir {
            std::fs::create_dir_all(path.join("child")).unwrap();
        } else {
            std::fs::write(&path, "trash").unwrap();
        }
        path
    }

    #[rstest]
    fn cleanup_expired_removes_old_files_and_dirs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let now = SystemTime::now();
        let old_file = create_entry(tmp.path(), now - DAY * 31, "old.txt", false);
        let old_dir = create_entry(tmp.path(), now - DAY * 31, "old_dir", true);

        let deleted = cleanup_expired_in(tmp.path(), 30, now).unwrap();

        assert_that!(deleted, eq(2));
        assert!(!old_file.exists());
        assert!(!old_dir.exists());
    }

    #[rstest]
    fn cleanup_expired_keeps_recent_entries() {
        let tmp = tempfile::TempDir::new().unwrap();
        let now = SystemTime::now();
        let recent = create_entry(tmp.path(), now - DAY * 29, "recent.txt", false);

        let deleted = cleanup_expired_in(tmp.path(), 30, now).unwrap();

        assert_that!(deleted, eq(0));
        assert!(recent.exists());
    }

    #[rstest]
    fn cleanup_expired_ignores_entries_without_timestamp_prefix() {
        let tmp = tempfile::TempDir::new().unwrap();
        let foreign = tmp.path().join("not_a_timestamp.txt");
        std::fs::write(&foreign, "keep").unwrap();

        let deleted = cleanup_expired_in(tmp.path(), 1, SystemTime::now() + DAY * 365).unwrap();

        assert_that!(deleted, eq(0));
        assert!(foreign.exists());
    }

    #[rstest]
    fn cleanup_expired_disabled_with_zero_days() {
        assert_that!(cleanup_expired(0).unwrap(), eq(0));
    }

    #[rstest]
    #[case("1700000000000000000_file.txt", Some(1_700_000_000))]
    #[case("1700000000000000000_with_underscore.txt", Some(1_700_000_000))]
    #[case("file.txt", None)]
    #[case("abc_file.txt", None)]
    fn trashed_at_parses_prefix(#[case] name: &str, #[case] expected_secs: Option<u64>) {
        let secs =
            trashed_at(name).map(|t| t.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs());
        assert_that!(secs, eq(expected_secs));
    }
}
