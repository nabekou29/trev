//! Workspace key derivation and socket path utilities.

use std::path::{
    Path,
    PathBuf,
};

use sha2::{
    Digest,
    Sha256,
};

/// Maximum byte length of the directory-name prefix in a workspace key.
///
/// The socket path lives in `sockaddr_un.sun_path`, which is limited to
/// 104 bytes on macOS. The runtime dir alone (`$TMPDIR/trev`) is ~54 bytes
/// there, so the name prefix must stay short. Uniqueness comes from the
/// hash, not the name — the prefix is only for readability.
const MAX_KEY_NAME_BYTES: usize = 16;

/// Derive a workspace key from a canonical directory path.
///
/// Always uses `<dir_name>-<hash8>` format for a compact, collision-free key.
/// The directory name is truncated to [`MAX_KEY_NAME_BYTES`] so the socket
/// path stays within the platform's `sun_path` limit.
/// The full workspace path is stored in a separate metadata file for reverse lookup.
///
/// # Examples
///
/// - `/Users/foo/bar` → `bar-a1b2c3d4`
/// - `/Users/foo/trev` → `trev-e5f6a7b8`
pub fn workspace_key(path: &Path) -> String {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path_str = canonical.to_string_lossy();

    let dir_name =
        path.file_name().map_or_else(|| "trev".to_owned(), |n| n.to_string_lossy().into_owned());
    let dir_name = truncate_at_char_boundary(&dir_name, MAX_KEY_NAME_BYTES);

    let mut hasher = Sha256::new();
    hasher.update(path_str.as_bytes());
    let digest = hasher.finalize();
    let hash_hex = crate::util::hex_encode(digest.as_ref());
    let short_hash = hash_hex.get(..8).unwrap_or(&hash_hex);

    format!("{dir_name}-{short_hash}")
}

/// Truncate a string to at most `max_bytes` bytes without splitting a character.
fn truncate_at_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let end = s.char_indices().map(|(i, _)| i).take_while(|&i| i <= max_bytes).last().unwrap_or(0);
    s.get(..end).unwrap_or("")
}

/// Get the runtime directory for trev sockets.
///
/// Uses `$XDG_RUNTIME_DIR/trev`, falling back to `$TMPDIR/trev` or `/tmp/trev`.
pub fn runtime_dir() -> PathBuf {
    crate::dirs::AppDirs::new()
        .map_or_else(|_| std::env::temp_dir().join("trev"), |d| d.runtime_dir().to_path_buf())
}

/// Compute the socket path for the current process and workspace.
///
/// Format: `<runtime_dir>/<workspace_key>-<pid>.sock`
pub fn socket_path(workspace_dir: &Path) -> PathBuf {
    let key = workspace_key(workspace_dir);
    let pid = std::process::id();
    runtime_dir().join(format!("{key}-{pid}.sock"))
}

/// Compute the metadata file path corresponding to a socket path.
///
/// Format: same as socket path but with `.json` extension instead of `.sock`.
pub fn meta_path(sock_path: &Path) -> PathBuf {
    sock_path.with_extension("json")
}

/// Write workspace metadata alongside a socket file.
///
/// Stores the canonical workspace path so that `trev ctl` can display
/// and filter instances by their workspace directory.
///
/// # Errors
///
/// Returns an error if the metadata file cannot be written.
pub fn write_meta(sock_path: &Path, workspace_dir: &Path) -> std::io::Result<()> {
    let meta = meta_path(sock_path);
    let canonical =
        std::fs::canonicalize(workspace_dir).unwrap_or_else(|_| workspace_dir.to_path_buf());
    let content = serde_json::json!({ "path": canonical.to_string_lossy() });
    std::fs::write(meta, content.to_string())
}

/// Read workspace path from a socket's metadata file.
///
/// Returns `None` if the metadata file doesn't exist or can't be parsed.
pub fn read_meta(sock_path: &Path) -> Option<PathBuf> {
    let meta = meta_path(sock_path);
    let content = std::fs::read_to_string(meta).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json.get("path")?.as_str().map(PathBuf::from)
}

/// Remove workspace metadata file alongside a socket file.
pub fn remove_meta(sock_path: &Path) {
    let meta = meta_path(sock_path);
    let _ = std::fs::remove_file(meta);
}

/// Remove stale socket and metadata files left by crashed processes.
///
/// Scans the runtime directory for `.sock` files and tries to connect
/// to each one. If the connection fails, the instance is no longer running
/// and the socket (and its metadata) is removed.
///
/// Skips the current process's own socket (by PID in filename).
/// Returns the number of cleaned-up sockets.
pub fn cleanup_stale_sockets() -> usize {
    let dir = runtime_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return 0;
    };

    let own_pid = std::process::id();
    let mut removed = 0;

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "sock") {
            continue;
        }
        // Skip our own socket.
        if extract_pid_from_socket(&path).is_some_and(|pid| pid == own_pid) {
            continue;
        }
        // Try connecting — if it fails, the instance is gone.
        if std::os::unix::net::UnixStream::connect(&path).is_err() {
            let _ = std::fs::remove_file(&path);
            remove_meta(&path);
            removed += 1;
            tracing::debug!(path = %path.display(), "removed stale socket");
        }
    }
    removed
}

/// Extract the PID from a socket filename.
///
/// Expects format `<key>-<pid>.sock`, returns the PID portion.
fn extract_pid_from_socket(path: &Path) -> Option<u32> {
    let stem = path.file_stem()?.to_str()?;
    let pid_str = stem.rsplit('-').next()?;
    pid_str.parse().ok()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rstest::*;
    use tempfile::TempDir;

    use super::*;

    // --- workspace_key ---

    #[rstest]
    fn workspace_key_contains_dir_name_and_hash() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("my-project");
        std::fs::create_dir(&dir).unwrap();

        let key = workspace_key(&dir);

        assert!(key.starts_with("my-project-"), "key should start with dir name: {key}");
        // Hash part: 8 hex chars after the last dash.
        let hash_part = key.strip_prefix("my-project-").unwrap();
        assert_eq!(hash_part.len(), 8, "hash should be 8 chars: {hash_part}");
        assert!(
            hash_part.chars().all(|c| c.is_ascii_hexdigit()),
            "hash should be hex: {hash_part}"
        );
    }

    #[rstest]
    fn workspace_key_is_unique_for_different_paths() {
        let tmp = TempDir::new().unwrap();
        let dir_a = tmp.path().join("project-a");
        let dir_b = tmp.path().join("project-b");
        std::fs::create_dir(&dir_a).unwrap();
        std::fs::create_dir(&dir_b).unwrap();

        assert_ne!(workspace_key(&dir_a), workspace_key(&dir_b));
    }

    #[rstest]
    fn workspace_key_same_name_different_parent() {
        let tmp = TempDir::new().unwrap();
        let dir_a = tmp.path().join("parent-a").join("app");
        let dir_b = tmp.path().join("parent-b").join("app");
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::create_dir_all(&dir_b).unwrap();

        let key_a = workspace_key(&dir_a);
        let key_b = workspace_key(&dir_b);

        // Both start with "app-" but hashes differ.
        assert!(key_a.starts_with("app-"));
        assert!(key_b.starts_with("app-"));
        assert_ne!(key_a, key_b);
    }

    #[rstest]
    fn workspace_key_truncates_long_dir_name() {
        let tmp = TempDir::new().unwrap();
        // 31-char name reproduces the macOS sun_path overflow when untruncated.
        let dir = tmp.path().join("feature-awesome-widget-redesign");
        std::fs::create_dir(&dir).unwrap();

        let key = workspace_key(&dir);

        // name(≤16) + "-" + hash(8)
        assert!(key.len() <= 25, "key should be at most 25 bytes: {key} ({})", key.len());
        assert!(key.starts_with("feature-awesome-"), "key should keep a readable prefix: {key}");
    }

    #[rstest]
    fn workspace_key_truncation_respects_char_boundary() {
        let tmp = TempDir::new().unwrap();
        // Multibyte name: 3 bytes per char, byte 16 falls mid-character.
        let dir = tmp.path().join("日本語のディレクトリ名です");
        std::fs::create_dir(&dir).unwrap();

        let key = workspace_key(&dir);

        assert!(key.len() <= 25, "key should be at most 25 bytes: {key} ({})", key.len());
        // 16 bytes fit 5 whole 3-byte chars (15 bytes); the 6th would split at byte 16.
        assert!(key.starts_with("日本語のデ-"), "prefix should be whole chars: {key}");
    }

    #[rstest]
    fn workspace_key_long_names_with_same_prefix_differ() {
        let tmp = TempDir::new().unwrap();
        let dir_a = tmp.path().join("feature-awesome-widget-redesign");
        let dir_b = tmp.path().join("feature-awesome-widget-redesign-v2");
        std::fs::create_dir(&dir_a).unwrap();
        std::fs::create_dir(&dir_b).unwrap();

        assert_ne!(workspace_key(&dir_a), workspace_key(&dir_b));
    }

    #[rstest]
    fn socket_path_filename_is_bounded_for_long_dir_names() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("a".repeat(100));
        std::fs::create_dir(&dir).unwrap();

        let path = socket_path(&dir);
        let filename = path.file_name().unwrap().to_str().unwrap();

        // name(≤16) + "-" + hash(8) + "-" + pid(≤10) + ".sock"(5) = 41 max.
        assert!(filename.len() <= 41, "filename should stay short: {filename}");
    }

    #[rstest]
    fn workspace_key_deterministic() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("stable");
        std::fs::create_dir(&dir).unwrap();

        assert_eq!(workspace_key(&dir), workspace_key(&dir));
    }

    // --- meta ---

    #[rstest]
    fn write_and_read_meta_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let workspace = tmp.path().join("my-project");
        std::fs::create_dir(&workspace).unwrap();

        let sock = tmp.path().join("test-12345.sock");
        write_meta(&sock, &workspace).unwrap();

        let result = read_meta(&sock);
        let canonical = std::fs::canonicalize(&workspace).unwrap();
        assert_eq!(result, Some(canonical));
    }

    #[rstest]
    fn read_meta_returns_none_when_missing() {
        let tmp = TempDir::new().unwrap();
        let sock = tmp.path().join("nonexistent-12345.sock");

        assert_eq!(read_meta(&sock), None);
    }

    #[rstest]
    fn remove_meta_deletes_file() {
        let tmp = TempDir::new().unwrap();
        let workspace = tmp.path().join("proj");
        std::fs::create_dir(&workspace).unwrap();

        let sock = tmp.path().join("test-12345.sock");
        write_meta(&sock, &workspace).unwrap();
        assert!(meta_path(&sock).exists());

        remove_meta(&sock);
        assert!(!meta_path(&sock).exists());
    }

    #[rstest]
    fn remove_meta_noop_when_missing() {
        let tmp = TempDir::new().unwrap();
        let sock = tmp.path().join("nonexistent.sock");
        // Should not panic.
        remove_meta(&sock);
    }

    // --- runtime_dir ---

    #[rstest]
    fn runtime_dir_returns_path_with_trev_subdir() {
        let dir = runtime_dir();
        assert!(dir.ends_with("trev"));
    }

    // --- socket_path ---

    #[rstest]
    fn socket_path_ends_with_sock() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("test-project");
        std::fs::create_dir(&dir).unwrap();

        let path = socket_path(&dir);
        let filename = path.file_name().unwrap().to_str().unwrap();

        assert!(Path::new(filename).extension().is_some_and(|ext| ext == "sock"));
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

    #[rstest]
    fn meta_path_matches_socket_path() {
        let sock = PathBuf::from("/tmp/trev/proj-a1b2c3d4-12345.sock");
        let meta = meta_path(&sock);
        assert_eq!(meta, PathBuf::from("/tmp/trev/proj-a1b2c3d4-12345.json"));
    }

    // --- extract_pid_from_socket ---

    #[rstest]
    fn extract_pid_from_standard_socket_name() {
        let path = PathBuf::from("/tmp/trev/proj-a1b2c3d4-12345.sock");
        assert_eq!(extract_pid_from_socket(&path), Some(12345));
    }

    #[rstest]
    fn extract_pid_returns_none_for_no_dash() {
        let path = PathBuf::from("/tmp/trev/nodash.sock");
        assert_eq!(extract_pid_from_socket(&path), None);
    }

    #[rstest]
    fn extract_pid_returns_none_for_non_numeric() {
        let path = PathBuf::from("/tmp/trev/proj-abc.sock");
        assert_eq!(extract_pid_from_socket(&path), None);
    }
}
