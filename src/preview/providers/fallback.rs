//! Fallback preview provider — binary, directory, empty, error.

use std::fs;
use std::path::Path;

use crate::preview::content::PreviewContent;
use crate::preview::provider::{
    LoadContext,
    PreviewProvider,
};

/// Fallback provider for files that no other provider handles.
///
/// Produces `Directory` info for directories, `Binary` info for
/// binary files, and `Empty` otherwise. Only enabled when no
/// other provider can handle the file (sole candidate).
#[derive(Debug, Clone, Copy)]
pub struct FallbackProvider;

impl FallbackProvider {
    /// Create a new fallback provider.
    pub const fn new() -> Self {
        Self
    }
}

impl PreviewProvider for FallbackProvider {
    #[expect(clippy::unnecessary_literal_bound, reason = "Trait requires &str return")]
    fn name(&self) -> &str {
        "Fallback"
    }

    fn priority(&self) -> u32 {
        crate::config::Priority::LOW.value()
    }

    fn can_handle(&self, _path: &Path, _is_dir: bool) -> bool {
        true
    }

    fn is_enabled(&self, available: &[&str]) -> bool {
        available.len() <= 1
    }

    fn load(&self, path: &Path, _ctx: &LoadContext) -> anyhow::Result<PreviewContent> {
        let metadata = fs::metadata(path)?;

        if metadata.is_dir() {
            let mut entry_count = 0usize;
            let mut total_size = 0u64;

            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    entry_count += 1;
                    if let Ok(meta) = entry.metadata() {
                        total_size += meta.len();
                    }
                }
            }

            return Ok(PreviewContent::Directory { entry_count, total_size });
        }

        // Special files (FIFOs, sockets, device nodes) — describe without opening.
        if !metadata.is_file() {
            let kind = describe_special_file(&metadata);
            return Ok(PreviewContent::Error { message: format!("Special file ({kind})") });
        }

        let size = metadata.len();

        if size == 0 {
            return Ok(PreviewContent::Empty);
        }

        // Non-text file → binary.
        Ok(PreviewContent::Binary { size })
    }
}

/// Describe the type of a special (non-regular, non-directory) file.
#[cfg(unix)]
fn describe_special_file(metadata: &fs::Metadata) -> &'static str {
    use std::os::unix::fs::FileTypeExt;

    let ft = metadata.file_type();
    if ft.is_fifo() {
        "pipe"
    } else if ft.is_socket() {
        "socket"
    } else if ft.is_char_device() {
        "char device"
    } else if ft.is_block_device() {
        "block device"
    } else {
        "special file"
    }
}

/// Describe the type of a special (non-regular, non-directory) file.
#[cfg(not(unix))]
fn describe_special_file(_metadata: &fs::Metadata) -> &'static str {
    "special file"
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;
    use tempfile::TempDir;
    use tokio_util::sync::CancellationToken;

    use super::*;

    fn make_ctx() -> LoadContext {
        LoadContext {
            max_lines: 1000,
            max_bytes: 10 * 1024 * 1024,

            cancel_token: CancellationToken::new(),
        }
    }

    #[fixture]
    fn temp_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    // --- can_handle tests ---

    #[rstest]
    fn can_handle_always_true() {
        let provider = FallbackProvider::new();
        assert_that!(provider.can_handle(Path::new("/any/file"), false), eq(true));
        assert_that!(provider.can_handle(Path::new("/any/dir"), true), eq(true));
    }

    // --- is_enabled tests ---

    #[rstest]
    fn is_enabled_when_sole_provider() {
        let provider = FallbackProvider::new();
        assert_that!(provider.is_enabled(&["Fallback"]), eq(true));
    }

    #[rstest]
    fn is_disabled_when_others_exist() {
        let provider = FallbackProvider::new();
        assert_that!(provider.is_enabled(&["Text", "Fallback"]), eq(false));
    }

    // --- load tests ---

    #[rstest]
    fn load_directory_produces_directory_info(temp_dir: TempDir) {
        let dir = temp_dir.path().join("subdir");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("a.txt"), "hello").unwrap();
        fs::write(dir.join("b.txt"), "world!").unwrap();

        let provider = FallbackProvider::new();
        let result = provider.load(&dir, &make_ctx()).unwrap();

        match result {
            PreviewContent::Directory { entry_count, total_size } => {
                assert_that!(entry_count, eq(2));
                assert_that!(total_size, eq(11u64)); // "hello" + "world!" = 5 + 6
            }
            other => panic!("Expected Directory, got {other:?}"),
        }
    }

    #[rstest]
    fn load_binary_file_produces_binary(temp_dir: TempDir) {
        let path = temp_dir.path().join("data.bin");
        fs::write(&path, b"\x00\x01\x02\x03").unwrap();

        let provider = FallbackProvider::new();
        let result = provider.load(&path, &make_ctx()).unwrap();

        match result {
            PreviewContent::Binary { size } => {
                assert_that!(size, eq(4u64));
            }
            other => panic!("Expected Binary, got {other:?}"),
        }
    }

    #[rstest]
    fn load_empty_file_produces_empty(temp_dir: TempDir) {
        let path = temp_dir.path().join("empty.bin");
        fs::write(&path, "").unwrap();

        let provider = FallbackProvider::new();
        let result = provider.load(&path, &make_ctx()).unwrap();

        assert!(matches!(result, PreviewContent::Empty));
    }

    #[cfg(unix)]
    #[rstest]
    fn load_fifo_produces_error(temp_dir: TempDir) {
        use std::os::unix::fs::FileTypeExt;

        let fifo_path = temp_dir.path().join("test.pipe");
        std::process::Command::new("mkfifo").arg(&fifo_path).status().unwrap();

        // Verify it's actually a FIFO.
        let meta = fs::metadata(&fifo_path).unwrap();
        assert!(meta.file_type().is_fifo());

        let provider = FallbackProvider::new();
        let result = provider.load(&fifo_path, &make_ctx()).unwrap();

        match result {
            PreviewContent::Error { message } => {
                assert_that!(message.as_str(), eq("Special file (pipe)"));
            }
            other => panic!("Expected Error, got {other:?}"),
        }
    }
}
