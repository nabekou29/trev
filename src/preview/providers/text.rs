//! Text preview provider — syntax highlighting via syntect.

use std::fs;
use std::io::{
    BufRead,
    BufReader,
};
use std::path::Path;

use crate::preview::content::PreviewContent;
use crate::preview::highlight;
use crate::preview::provider::{
    LoadContext,
    PreviewProvider,
};

/// Provider that reads text files and produces syntax-highlighted previews.
///
/// Falls back to plain text when no syntax definition is found.
/// Detects binary files via `content_inspector` and skips them.
#[derive(Debug, Clone, Copy)]
pub struct TextPreviewProvider;

impl TextPreviewProvider {
    /// Create a new text preview provider.
    pub const fn new() -> Self {
        Self
    }
}

impl PreviewProvider for TextPreviewProvider {
    #[expect(clippy::unnecessary_literal_bound, reason = "Trait requires &str return")]
    fn name(&self) -> &str {
        "Text"
    }

    fn priority(&self) -> u32 {
        crate::config::Priority::MID.value()
    }

    fn can_handle(&self, path: &Path, is_dir: bool) -> bool {
        if is_dir {
            return false;
        }

        // Skip special files (FIFOs, sockets, device files) to avoid blocking on open().
        if !fs::metadata(path).is_ok_and(|m| m.is_file()) {
            return false;
        }

        // Read first 1024 bytes for binary detection.
        let Ok(buf) = read_head(path, 1024) else {
            return false;
        };

        // Empty files are handled (will produce Empty variant).
        if buf.is_empty() {
            return true;
        }

        content_inspector::inspect(&buf).is_text()
    }

    fn load(&self, path: &Path, ctx: &LoadContext) -> anyhow::Result<PreviewContent> {
        let _span = tracing::info_span!("text_load", path = %path.display()).entered();
        let metadata = fs::metadata(path)?;
        if metadata.len() == 0 {
            return Ok(PreviewContent::Empty);
        }

        // Read lines with dual limits (max_lines, max_bytes).
        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);

        let mut lines = Vec::new();
        let mut bytes_read: u64 = 0;
        let mut truncated = false;

        for line_result in reader.lines() {
            if ctx.cancel_token.is_cancelled() {
                return Ok(PreviewContent::Empty);
            }

            let line = line_result?;
            bytes_read += line.len() as u64 + 1; // +1 for newline

            if lines.len() >= ctx.max_lines || bytes_read > ctx.max_bytes {
                truncated = true;
                break;
            }

            lines.push(line);
        }

        if lines.is_empty() {
            return Ok(PreviewContent::Empty);
        }

        // Detect syntax by file extension.
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let language = highlight::detect_language(extension);

        if language == "Plain Text" {
            Ok(PreviewContent::PlainText { lines, truncated })
        } else {
            let code = lines.join("\n");
            let highlighted = highlight::highlight_lines(&code, extension);
            Ok(PreviewContent::HighlightedText { lines: highlighted, language, truncated })
        }
    }
}

/// Read up to `limit` bytes from the start of a file.
fn read_head(path: &Path, limit: usize) -> std::io::Result<Vec<u8>> {
    use std::io::Read;

    let mut file = fs::File::open(path)?;
    let mut buf = vec![0u8; limit];
    let n = file.read(&mut buf)?;
    buf.truncate(n);
    Ok(buf)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use std::fmt::Write;

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
    fn can_handle_text_file(temp_dir: TempDir) {
        let path = temp_dir.path().join("hello.rs");
        fs::write(&path, "fn main() {}").unwrap();

        let provider = TextPreviewProvider::new();
        assert_that!(provider.can_handle(&path, false), eq(true));
    }

    #[rstest]
    fn can_handle_binary_file(temp_dir: TempDir) {
        let path = temp_dir.path().join("data.bin");
        // Write binary data with null bytes.
        fs::write(&path, b"\x00\x01\x02\x03\xff\xfe\xfd").unwrap();

        let provider = TextPreviewProvider::new();
        assert_that!(provider.can_handle(&path, false), eq(false));
    }

    #[rstest]
    fn can_handle_directory_returns_false() {
        let provider = TextPreviewProvider::new();
        assert_that!(provider.can_handle(Path::new("/tmp"), true), eq(false));
    }

    #[rstest]
    fn can_handle_empty_file(temp_dir: TempDir) {
        let path = temp_dir.path().join("empty.txt");
        fs::write(&path, "").unwrap();

        let provider = TextPreviewProvider::new();
        assert_that!(provider.can_handle(&path, false), eq(true));
    }

    #[rstest]
    fn can_handle_nonexistent_file() {
        let provider = TextPreviewProvider::new();
        assert_that!(provider.can_handle(Path::new("/nonexistent/file.txt"), false), eq(false));
    }

    #[cfg(unix)]
    #[rstest]
    fn can_handle_fifo_returns_false(temp_dir: TempDir) {
        let fifo_path = temp_dir.path().join("test.pipe");
        std::process::Command::new("mkfifo")
            .arg(&fifo_path)
            .status()
            .unwrap();

        let provider = TextPreviewProvider::new();
        assert_that!(provider.can_handle(&fifo_path, false), eq(false));
    }

    // --- load tests ---

    #[rstest]
    fn load_rust_file_produces_highlighted_text(temp_dir: TempDir) {
        let path = temp_dir.path().join("main.rs");
        fs::write(&path, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

        let provider = TextPreviewProvider::new();
        let result = provider.load(&path, &make_ctx()).unwrap();

        match result {
            PreviewContent::HighlightedText { language, truncated, lines } => {
                assert_that!(language.as_str(), eq("Rust"));
                assert_that!(truncated, eq(false));
                assert_that!(lines.len(), ge(1));
            }
            other => panic!("Expected HighlightedText, got {other:?}"),
        }
    }

    #[rstest]
    fn load_unknown_extension_produces_plain_text(temp_dir: TempDir) {
        let path = temp_dir.path().join("data.zzzzunknown");
        fs::write(&path, "some content\nanother line\n").unwrap();

        let provider = TextPreviewProvider::new();
        let result = provider.load(&path, &make_ctx()).unwrap();

        match result {
            PreviewContent::PlainText { lines, truncated } => {
                assert_that!(lines.len(), eq(2));
                assert_that!(truncated, eq(false));
            }
            other => panic!("Expected PlainText, got {other:?}"),
        }
    }

    #[rstest]
    fn load_empty_file_produces_empty(temp_dir: TempDir) {
        let path = temp_dir.path().join("empty.txt");
        fs::write(&path, "").unwrap();

        let provider = TextPreviewProvider::new();
        let result = provider.load(&path, &make_ctx()).unwrap();

        assert!(matches!(result, PreviewContent::Empty));
    }

    #[rstest]
    fn load_respects_max_lines_limit(temp_dir: TempDir) {
        let path = temp_dir.path().join("big.txt");
        let content: String = (0..100).fold(String::new(), |mut s, i| {
            let _ = writeln!(s, "line {i}");
            s
        });
        fs::write(&path, &content).unwrap();

        let ctx = LoadContext {
            max_lines: 10,
            max_bytes: 10 * 1024 * 1024,

            cancel_token: CancellationToken::new(),
        };

        let provider = TextPreviewProvider::new();
        let result = provider.load(&path, &ctx).unwrap();

        match result {
            PreviewContent::PlainText { lines, truncated } => {
                assert_that!(lines.len(), eq(10));
                assert_that!(truncated, eq(true));
            }
            other => panic!("Expected PlainText, got {other:?}"),
        }
    }

    #[rstest]
    fn load_respects_max_bytes_limit(temp_dir: TempDir) {
        let path = temp_dir.path().join("big.txt");
        // Each line is ~50 bytes, 100 lines = ~5000 bytes
        let content: String = (0..100).fold(String::new(), |mut s, i| {
            let _ = writeln!(s, "line {i:045}");
            s
        });
        fs::write(&path, &content).unwrap();

        let ctx = LoadContext {
            max_lines: 10000,
            max_bytes: 200, // very small byte limit

            cancel_token: CancellationToken::new(),
        };

        let provider = TextPreviewProvider::new();
        let result = provider.load(&path, &ctx).unwrap();

        match result {
            PreviewContent::PlainText { truncated, .. } => {
                assert_that!(truncated, eq(true));
            }
            other => panic!("Expected PlainText, got {other:?}"),
        }
    }

    #[rstest]
    fn load_cancelled_produces_empty(temp_dir: TempDir) {
        let path = temp_dir.path().join("test.txt");
        fs::write(&path, "some content\n").unwrap();

        let ctx = LoadContext {
            max_lines: 1000,
            max_bytes: 10 * 1024 * 1024,

            cancel_token: CancellationToken::new(),
        };
        ctx.cancel_token.cancel();

        let provider = TextPreviewProvider::new();
        let result = provider.load(&path, &ctx).unwrap();

        assert!(matches!(result, PreviewContent::Empty));
    }
}
