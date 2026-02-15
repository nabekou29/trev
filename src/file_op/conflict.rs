//! Auto-rename conflict resolution for file name collisions.

use std::ffi::OsString;
use std::path::{
    Path,
    PathBuf,
};

/// Resolve a file name conflict by appending `_N` before the extension.
///
/// If `dst` does not exist, returns it unchanged.
/// If `dst` exists, tries `name_1.ext`, `name_2.ext`, etc. until a unique name is found.
///
/// For directories (no extension), appends `_N` to the name.
pub fn resolve_conflict(dst: &Path) -> PathBuf {
    if !dst.exists() {
        return dst.to_path_buf();
    }

    let parent = dst.parent().unwrap_or_else(|| Path::new(""));
    let stem = dst.file_stem().unwrap_or_default();
    let extension = dst.extension();

    let mut counter = 1u64;
    loop {
        let mut new_name = OsString::from(stem);
        new_name.push(format!("_{counter}"));
        if let Some(ext) = extension {
            new_name.push(".");
            new_name.push(ext);
        }

        let candidate = parent.join(new_name);
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    #[rstest]
    fn no_conflict_returns_original() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dst = tmp.path().join("file.txt");
        assert_eq!(resolve_conflict(&dst), dst);
    }

    #[rstest]
    fn single_conflict_appends_1() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dst = tmp.path().join("file.txt");
        std::fs::write(&dst, "").unwrap();

        let resolved = resolve_conflict(&dst);
        assert_eq!(resolved, tmp.path().join("file_1.txt"));
    }

    #[rstest]
    fn multiple_conflicts_increment() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dst = tmp.path().join("file.txt");
        std::fs::write(&dst, "").unwrap();
        std::fs::write(tmp.path().join("file_1.txt"), "").unwrap();
        std::fs::write(tmp.path().join("file_2.txt"), "").unwrap();

        let resolved = resolve_conflict(&dst);
        assert_eq!(resolved, tmp.path().join("file_3.txt"));
    }

    #[rstest]
    fn conflict_directory_no_extension() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dst = tmp.path().join("mydir");
        std::fs::create_dir(&dst).unwrap();

        let resolved = resolve_conflict(&dst);
        assert_eq!(resolved, tmp.path().join("mydir_1"));
    }

    #[rstest]
    fn conflict_dotfile() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dst = tmp.path().join(".gitignore");
        std::fs::write(&dst, "").unwrap();

        let resolved = resolve_conflict(&dst);
        assert_that!(resolved.file_name().unwrap().to_str().unwrap(), eq(".gitignore_1"));
    }
}
