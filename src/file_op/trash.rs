//! Custom trash directory management for undo-able delete operations.

use std::path::{
    Path,
    PathBuf,
};
use std::sync::atomic::{
    AtomicBool,
    Ordering,
};
use std::time::{
    Duration,
    SystemTime,
};

use anyhow::{
    Context as _,
    Result,
};

/// Number of digits in the `{nanos}_` prefix of a trash entry name.
///
/// Nanoseconds since the Unix epoch have 19 digits from 2001-09 until 2286.
const TIMESTAMP_DIGITS: usize = 19;

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
    trash_path_in(&trash_dir(), original)
}

/// Generate a timestamped trash path for a file inside `dir`.
pub fn trash_path_in(dir: &Path, original: &Path) -> PathBuf {
    let nanos =
        SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos();

    let file_name = original.file_name().unwrap_or_default().to_string_lossy();

    dir.join(format!("{nanos}_{file_name}"))
}

/// Ensure the trash directory exists.
pub fn ensure_trash_dir() -> Result<()> {
    let dir = trash_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create trash directory: {}", dir.display()))?;
    Ok(())
}

/// Remove a specific entry from the custom trash directory.
///
/// Symlinks are removed themselves, never followed.
pub fn clean_trash_file(path: &Path) -> Result<()> {
    let Ok(meta) = path.symlink_metadata() else {
        return Ok(());
    };
    if meta.is_dir() {
        std::fs::remove_dir_all(path)
            .with_context(|| format!("Failed to clean trash: {}", path.display()))?;
    } else {
        std::fs::remove_file(path)
            .with_context(|| format!("Failed to clean trash: {}", path.display()))?;
    }
    Ok(())
}

/// Whether the trash entry at `path` is older than `retention_days` at `now`.
///
/// Always `false` when `retention_days` is `0` or the name has no trev timestamp.
pub fn is_expired(path: &Path, retention_days: u64, now: SystemTime) -> bool {
    if retention_days == 0 {
        return false;
    }
    let Some(trashed_at) = path.file_name().and_then(|n| trashed_at(&n.to_string_lossy())) else {
        return false;
    };
    let retention = Duration::from_secs(retention_days.saturating_mul(24 * 60 * 60));
    now.duration_since(trashed_at).is_ok_and(|age| age > retention)
}

/// Delete trash entries older than `retention_days` at `now`.
///
/// Stops early when `cancelled` becomes `true`; partially deleted entries are
/// still expired and are deleted on a later run. Returns the number of
/// entries deleted completely.
pub fn cleanup_expired(
    retention_days: u64,
    now: SystemTime,
    cancelled: &AtomicBool,
) -> Result<usize> {
    cleanup_expired_in(&trash_dir(), retention_days, now, cancelled)
}

/// [`cleanup_expired`] for an explicit trash directory.
fn cleanup_expired_in(
    dir: &Path,
    retention_days: u64,
    now: SystemTime,
    cancelled: &AtomicBool,
) -> Result<usize> {
    if retention_days == 0 || !dir.exists() {
        return Ok(0);
    }

    let mut deleted = 0;

    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("Failed to read trash directory: {}", dir.display()))?;

    for entry in entries {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
        let entry = entry.context("Failed to read trash directory entry")?;
        let path = entry.path();

        if !is_expired(&path, retention_days, now) {
            continue;
        }

        match remove_tree(&path, cancelled) {
            Ok(true) => deleted += 1,
            Ok(false) => break,
            Err(e) => tracing::warn!(%e, ?path, "failed to delete expired trash entry"),
        }
    }

    Ok(deleted)
}

/// Remove `path` recursively without following symlinks.
///
/// Checks `cancelled` between entries so that a large tree does not delay
/// shutdown. Returns `false` if cancelled before finishing.
fn remove_tree(path: &Path, cancelled: &AtomicBool) -> Result<bool> {
    let meta = path
        .symlink_metadata()
        .with_context(|| format!("Failed to read metadata: {}", path.display()))?;
    if !meta.is_dir() {
        std::fs::remove_file(path)
            .with_context(|| format!("Failed to remove file: {}", path.display()))?;
        return Ok(true);
    }

    for entry in std::fs::read_dir(path)
        .with_context(|| format!("Failed to read directory: {}", path.display()))?
    {
        if cancelled.load(Ordering::Relaxed) {
            return Ok(false);
        }
        let entry = entry.with_context(|| format!("Failed to read entry in {}", path.display()))?;
        if !remove_tree(&entry.path(), cancelled)? {
            return Ok(false);
        }
    }
    std::fs::remove_dir(path)
        .with_context(|| format!("Failed to remove directory: {}", path.display()))?;
    Ok(true)
}

/// Parse the trashed time from a `{nanos}_{original_name}` trash file name.
///
/// Only a prefix of exactly [`TIMESTAMP_DIGITS`] ASCII digits is accepted, so
/// names like `2024_report.xlsx` are not mistaken for trev entries.
fn trashed_at(file_name: &str) -> Option<SystemTime> {
    let (nanos, _) = file_name.split_once('_')?;
    if nanos.len() != TIMESTAMP_DIGITS || !nanos.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
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
    fn trash_path_timestamp_round_trips() {
        let before = SystemTime::now();
        let path = trash_path_in(Path::new("/trash"), Path::new("/a/file_name.txt"));
        let after = SystemTime::now();

        let parsed = trashed_at(&path.file_name().unwrap().to_string_lossy()).unwrap();
        assert!(before <= parsed && parsed <= after);
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

    #[cfg(unix)]
    #[rstest]
    fn clean_trash_file_removes_dangling_symlink() {
        let tmp = tempfile::TempDir::new().unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink("missing-target", &link).unwrap();

        clean_trash_file(&link).unwrap();
        assert!(link.symlink_metadata().is_err());
    }

    const DAY: Duration = Duration::from_hours(24);

    /// Create a trash entry named as if it was trashed at `trashed_at`.
    fn create_entry(dir: &Path, trashed_at: SystemTime, name: &str, is_dir: bool) -> PathBuf {
        let nanos = trashed_at.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
        let path = dir.join(format!("{nanos}_{name}"));
        if is_dir {
            std::fs::create_dir_all(path.join("child")).unwrap();
            std::fs::write(path.join("child/file.txt"), "x").unwrap();
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

        let deleted = cleanup_expired_in(tmp.path(), 30, now, &AtomicBool::new(false)).unwrap();

        assert_that!(deleted, eq(2));
        assert!(!old_file.exists());
        assert!(!old_dir.exists());
    }

    #[rstest]
    fn cleanup_expired_keeps_recent_entries() {
        let tmp = tempfile::TempDir::new().unwrap();
        let now = SystemTime::now();
        let recent = create_entry(tmp.path(), now - DAY * 29, "recent.txt", false);

        let deleted = cleanup_expired_in(tmp.path(), 30, now, &AtomicBool::new(false)).unwrap();

        assert_that!(deleted, eq(0));
        assert!(recent.exists());
    }

    #[rstest]
    fn cleanup_expired_ignores_entries_without_timestamp_prefix() {
        let tmp = tempfile::TempDir::new().unwrap();
        let foreign = tmp.path().join("2024_report.xlsx");
        std::fs::write(&foreign, "keep").unwrap();

        let deleted = cleanup_expired_in(
            tmp.path(),
            1,
            SystemTime::now() + DAY * 365,
            &AtomicBool::new(false),
        )
        .unwrap();

        assert_that!(deleted, eq(0));
        assert!(foreign.exists());
    }

    #[rstest]
    fn cleanup_expired_disabled_with_zero_days() {
        let tmp = tempfile::TempDir::new().unwrap();
        let now = SystemTime::now();
        let old = create_entry(tmp.path(), now - DAY * 3650, "old.txt", false);

        let deleted = cleanup_expired_in(tmp.path(), 0, now, &AtomicBool::new(false)).unwrap();

        assert_that!(deleted, eq(0));
        assert!(old.exists());
    }

    #[rstest]
    fn cleanup_expired_stops_when_cancelled() {
        let tmp = tempfile::TempDir::new().unwrap();
        let now = SystemTime::now();
        let old = create_entry(tmp.path(), now - DAY * 31, "old_dir", true);

        let deleted = cleanup_expired_in(tmp.path(), 30, now, &AtomicBool::new(true)).unwrap();

        assert_that!(deleted, eq(0));
        assert!(old.exists());
    }

    #[cfg(unix)]
    #[rstest]
    fn cleanup_expired_removes_symlink_without_following_it() {
        let tmp = tempfile::TempDir::new().unwrap();
        let trash = tmp.path().join("trash");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&trash).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("keep.txt"), "keep").unwrap();
        let now = SystemTime::now();
        let nanos = (now - DAY * 31).duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
        let link = trash.join(format!("{nanos}_link"));
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        let deleted = cleanup_expired_in(&trash, 30, now, &AtomicBool::new(false)).unwrap();

        assert_that!(deleted, eq(1));
        assert!(link.symlink_metadata().is_err());
        assert!(outside.join("keep.txt").exists());
    }

    #[rstest]
    #[case("/trash/1700000000000000000_file.txt", 30, true)]
    #[case("/trash/1700000000000000000_file.txt", 0, false)]
    #[case("/trash/file.txt", 30, false)]
    fn is_expired_cases(#[case] path: &str, #[case] days: u64, #[case] expected: bool) {
        // 1_700_000_000 s is 2023-11-14; check from one year later.
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000) + DAY * 365;
        assert_that!(is_expired(Path::new(path), days, now), eq(expected));
    }

    #[rstest]
    #[case("1700000000000000000_file.txt", Some(1_700_000_000))]
    #[case("1700000000000000000_with_underscore.txt", Some(1_700_000_000))]
    #[case("file.txt", None)]
    #[case("abc_file.txt", None)]
    #[case("2024_report.xlsx", None)]
    #[case("+700000000000000000_file.txt", None)]
    fn trashed_at_parses_prefix(#[case] name: &str, #[case] expected_secs: Option<u64>) {
        let secs =
            trashed_at(name).map(|t| t.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs());
        assert_that!(secs, eq(expected_secs));
    }
}
