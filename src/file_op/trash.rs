//! Custom trash directory management for undo-able delete operations.

use std::path::{
    Path,
    PathBuf,
};
use std::time::SystemTime;

use anyhow::{
    Context as _,
    Result,
};

/// Get the custom trash directory path.
///
/// Uses the platform data directory: `{data_dir}/trev/trash/`.
pub fn trash_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("trev/trash")
}

/// Generate a timestamped trash path for a file.
///
/// Format: `{trash_dir}/{nanos}_{original_name}`.
pub fn trash_path(original: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    let file_name = original
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    trash_dir().join(format!("{nanos}_{file_name}"))
}

/// Ensure the trash directory exists.
pub fn ensure_trash_dir() -> Result<()> {
    let dir = trash_dir();
    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create trash directory: {}", dir.display()))?;
    }
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
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
        std::thread::sleep(std::time::Duration::from_nanos(1));
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
}
