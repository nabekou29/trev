//! Workspace key derivation and socket path utilities.

use std::path::{
    Path,
    PathBuf,
};

/// Derive a workspace key from a directory path.
///
/// Walks up the directory tree looking for a `.git` directory.
/// If found, returns the Git root directory name.
/// Otherwise, returns the given directory name as a fallback.
pub fn workspace_key(path: &Path) -> String {
    // Walk up looking for .git
    let mut current = Some(path);
    while let Some(dir) = current {
        if dir.join(".git").is_dir() && let Some(name) = dir.file_name() {
            return name.to_string_lossy().into_owned();
        }
        current = dir.parent();
    }

    // Fallback: directory name of the given path
    path.file_name()
        .map_or_else(|| "trev".to_owned(), |n| n.to_string_lossy().into_owned())
}

/// Get the runtime directory for trev sockets.
///
/// Uses `$XDG_RUNTIME_DIR/trev`, falling back to `$TMPDIR/trev` or `/tmp/trev`.
pub fn runtime_dir() -> PathBuf {
    let base = dirs::runtime_dir().unwrap_or_else(std::env::temp_dir);
    base.join("trev")
}

/// Compute the socket path for the current process and workspace.
///
/// Format: `<runtime_dir>/trev/<workspace_key>-<pid>.sock`
pub fn socket_path(workspace_dir: &Path) -> PathBuf {
    let key = workspace_key(workspace_dir);
    let pid = std::process::id();
    runtime_dir().join(format!("{key}-{pid}.sock"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rstest::*;
    use tempfile::TempDir;

    use super::*;

    // --- workspace_key ---

    #[rstest]
    fn workspace_key_from_git_root() {
        let tmp = TempDir::new().unwrap();
        let repo_dir = tmp.path().join("my-project");
        std::fs::create_dir(&repo_dir).unwrap();
        std::fs::create_dir(repo_dir.join(".git")).unwrap();

        let key = workspace_key(&repo_dir);
        assert_eq!(key, "my-project");
    }

    #[rstest]
    fn workspace_key_from_git_subdir() {
        let tmp = TempDir::new().unwrap();
        let repo_dir = tmp.path().join("my-repo");
        std::fs::create_dir_all(repo_dir.join(".git")).unwrap();
        let sub_dir = repo_dir.join("src").join("deep");
        std::fs::create_dir_all(&sub_dir).unwrap();

        let key = workspace_key(&sub_dir);
        assert_eq!(key, "my-repo");
    }

    #[rstest]
    fn workspace_key_falls_back_to_dir_name() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("plain-dir");
        std::fs::create_dir(&dir).unwrap();

        let key = workspace_key(&dir);
        assert_eq!(key, "plain-dir");
    }

    // --- runtime_dir ---

    #[rstest]
    fn runtime_dir_returns_path_with_trev_subdir() {
        let dir = runtime_dir();
        assert!(dir.ends_with("trev"));
    }

    // --- socket_path ---

    #[rstest]
    fn socket_path_contains_workspace_key_and_pid() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("test-project");
        std::fs::create_dir(&dir).unwrap();

        let path = socket_path(&dir);
        let filename = path.file_name().unwrap().to_str().unwrap();

        assert!(filename.starts_with("test-project-"));
        assert!(filename.ends_with(".sock"));

        // PID part should be numeric
        let pid_part = filename
            .strip_prefix("test-project-")
            .unwrap()
            .strip_suffix(".sock")
            .unwrap();
        assert!(pid_part.parse::<u32>().is_ok());
    }

    #[rstest]
    fn socket_path_is_in_runtime_dir() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("proj");
        std::fs::create_dir(&dir).unwrap();

        let path = socket_path(&dir);
        let parent = path.parent().unwrap();
        assert!(parent.ends_with("trev"));
    }
}
