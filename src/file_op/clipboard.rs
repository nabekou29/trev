//! OS clipboard reading and image-to-file saving.
//!
//! Reads file lists or image data from the system clipboard
//! and provides helpers to save clipboard images as PNG files.

use std::path::{
    Path,
    PathBuf,
};

use anyhow::Result;

use super::conflict::resolve_conflict;

/// Content retrieved from the OS clipboard.
#[derive(Debug)]
pub enum ClipboardContent {
    /// A list of file paths (e.g., from Finder Cmd+C).
    Files(Vec<PathBuf>),
    /// An image as RGBA pixel data (e.g., a screenshot).
    Image {
        /// Image width in pixels.
        width: u32,
        /// Image height in pixels.
        height: u32,
        /// Raw RGBA pixel data.
        bytes: Vec<u8>,
    },
}

/// Read the OS clipboard, trying file list first, then image data.
///
/// Returns `None` if the clipboard is empty or contains only text.
pub fn read_clipboard() -> Option<ClipboardContent> {
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(cb) => cb,
        Err(e) => {
            tracing::warn!(%e, "failed to open clipboard");
            return None;
        }
    };

    // Try file list first.
    if let Ok(files) = clipboard.get().file_list()
        && !files.is_empty()
    {
        return Some(ClipboardContent::Files(files));
    }

    // Try image data.
    match clipboard.get().image() {
        Ok(img) => {
            let width = u32::try_from(img.width).ok()?;
            let height = u32::try_from(img.height).ok()?;
            Some(ClipboardContent::Image { width, height, bytes: img.bytes.into_owned() })
        }
        Err(_) => None,
    }
}

/// Save RGBA image data as a PNG file in the given directory.
///
/// The filename is generated from the current timestamp: `clipboard_YYYYMMDD_HHMMSS.png`.
/// If a file with that name already exists, a conflict suffix is appended.
///
/// Returns the final path of the saved file.
pub fn save_image_as_png(dst_dir: &Path, width: u32, height: u32, bytes: &[u8]) -> Result<PathBuf> {
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("clipboard_{timestamp}.png");
    let desired_path = dst_dir.join(&filename);
    let final_path = resolve_conflict(&desired_path);

    let img = image::RgbaImage::from_raw(width, height, bytes.to_vec())
        .ok_or_else(|| anyhow::anyhow!("invalid image dimensions {width}x{height}"))?;
    img.save(&final_path)?;

    Ok(final_path)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    #[rstest]
    fn save_image_creates_png_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        // 2x2 RGBA image (16 bytes).
        let bytes = vec![255u8; 2 * 2 * 4];
        let result = save_image_as_png(tmp.path(), 2, 2, &bytes);
        assert_that!(result.is_ok(), eq(true));

        let path = result.unwrap();
        assert_that!(path.exists(), eq(true));
        assert_that!(path.extension().and_then(|e| e.to_str()), some(eq("png")));
    }

    #[rstest]
    fn save_image_invalid_dimensions_returns_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Buffer too small for 10x10 image.
        let bytes = vec![0u8; 4];
        let result = save_image_as_png(tmp.path(), 10, 10, &bytes);
        assert_that!(result.is_err(), eq(true));
    }

    #[rstest]
    fn save_image_conflict_resolution() {
        let tmp = tempfile::TempDir::new().unwrap();
        let bytes = vec![255u8; 2 * 2 * 4];

        let path1 = save_image_as_png(tmp.path(), 2, 2, &bytes).unwrap();
        let path2 = save_image_as_png(tmp.path(), 2, 2, &bytes).unwrap();

        assert_that!(path1.exists(), eq(true));
        assert_that!(path2.exists(), eq(true));
        assert_that!(path1 != path2, eq(true));
    }
}
