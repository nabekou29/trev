//! Image preview provider — ratatui-image.

use std::path::Path;
use std::sync::Mutex;

use ratatui_image::picker::{
    Capability,
    Picker,
    ProtocolType,
};

use crate::preview::content::PreviewContent;
use crate::preview::provider::{
    LoadContext,
    NodeInfo,
    PreviewProvider,
};

/// Known image file extensions.
const IMAGE_EXTENSIONS: &[&str] =
    &["png", "jpg", "jpeg", "gif", "bmp", "webp", "ico", "tiff", "tif", "svg"];

/// Provider that renders image files via ratatui-image.
///
/// Uses `Picker` to detect terminal graphics protocol capabilities
/// and produces a `StatefulProtocol` for rendering.
pub struct ImagePreviewProvider {
    /// Shared picker for creating image protocols.
    /// Wrapped in Mutex for thread safety (Send + Sync).
    picker: Mutex<Picker>,
}

impl ImagePreviewProvider {
    /// Create a new image preview provider with the given picker.
    pub const fn new(picker: Picker) -> Self {
        Self { picker: Mutex::new(picker) }
    }

    /// Create a fallback Picker using halfblocks protocol.
    ///
    /// Used when terminal query fails. Uses a default font size.
    pub fn fallback_picker() -> Picker {
        Picker::halfblocks()
    }
}

/// Correct the graphics protocol detected by `Picker` when running inside herdr.
///
/// A herdr pane inherits the outer terminal's environment variables, and
/// ratatui-image skips the Kitty and Sixel capability queries whenever
/// `WEZTERM_EXECUTABLE` (or `KONSOLE_VERSION`) is set, falling back to the
/// iTerm2 protocol based on `TERM_PROGRAM`. The protocol then matches the outer
/// terminal rather than the herdr pane that actually draws the output, and
/// iTerm2 image payloads leave the preview empty there.
///
/// herdr's own emulator answers the Kitty capability query and reports its cell
/// size, so Kitty is the protocol to use inside a pane.
pub fn correct_protocol_in_herdr(picker: &mut Picker) {
    let in_herdr = std::env::var_os("HERDR_ENV").is_some_and(|v| !v.is_empty());
    let cell_size_known =
        picker.capabilities().iter().any(|c| matches!(*c, Capability::CellSize(Some(_))));
    let resolved = resolve_protocol(picker.protocol_type(), cell_size_known, in_herdr);
    if resolved != picker.protocol_type() {
        picker.set_protocol_type(resolved);
    }
}

/// Decide the protocol to render with.
///
/// The correction requires a real cell-size response: without one the picker
/// carries a placeholder font size, and Kitty images would be sized in pixels
/// against a made-up cell grid.
const fn resolve_protocol(
    detected: ProtocolType,
    cell_size_known: bool,
    in_herdr: bool,
) -> ProtocolType {
    if in_herdr && cell_size_known { ProtocolType::Kitty } else { detected }
}

impl std::fmt::Debug for ImagePreviewProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImagePreviewProvider").finish_non_exhaustive()
    }
}

impl PreviewProvider for ImagePreviewProvider {
    #[expect(clippy::unnecessary_literal_bound, reason = "Trait requires &str return")]
    fn name(&self) -> &str {
        "Image"
    }

    fn priority(&self) -> u32 {
        crate::config::Priority::MID.value()
    }

    fn is_cacheable(&self) -> bool {
        false
    }

    fn can_handle(&self, path: &Path, node: &NodeInfo) -> bool {
        if node.is_dir() {
            return false;
        }
        path.extension().and_then(|e| e.to_str()).is_some_and(|ext| {
            let ext_lower = ext.to_ascii_lowercase();
            IMAGE_EXTENSIONS.iter().any(|&ie| ie == ext_lower)
        })
    }

    fn load(&self, path: &Path, ctx: &LoadContext) -> anyhow::Result<PreviewContent> {
        let _span = tracing::info_span!("image_load", path = %path.display()).entered();
        if ctx.cancel_token.is_cancelled() {
            return Ok(PreviewContent::Cancelled);
        }

        let dyn_image = load_image(path)?;

        if ctx.cancel_token.is_cancelled() {
            return Ok(PreviewContent::Cancelled);
        }

        let Ok(picker) = self.picker.lock() else {
            return Ok(PreviewContent::Error {
                message: "Failed to acquire image picker lock".to_string(),
            });
        };

        let protocol = picker.new_resize_protocol(dyn_image);

        Ok(PreviewContent::Image { protocol: Box::new(protocol) })
    }
}

/// Load an image file, handling SVG via resvg.
fn load_image(path: &Path) -> anyhow::Result<image::DynamicImage> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();

    if ext == "svg" {
        load_svg(path)
    } else {
        let img = image::ImageReader::open(path)?.decode()?;
        Ok(img)
    }
}

/// Render an SVG file to a `DynamicImage` using resvg.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::float_arithmetic,
    reason = "SVG rendering requires floating point math and integer casts"
)]
fn load_svg(path: &Path) -> anyhow::Result<image::DynamicImage> {
    let _span = tracing::info_span!("svg_load").entered();
    let svg_data = std::fs::read(path)?;

    let tree = resvg::usvg::Tree::from_data(&svg_data, &resvg::usvg::Options::default())?;

    let size = tree.size();
    let width = size.width().ceil() as u32;
    let height = size.height().ceil() as u32;

    // Cap dimensions to avoid excessive memory usage.
    let max_dim = 2048;
    let (w, h) = if width > max_dim || height > max_dim {
        let scale = f64::from(max_dim) / f64::from(width.max(height));
        ((f64::from(width) * scale).ceil() as u32, (f64::from(height) * scale).ceil() as u32)
    } else {
        (width.max(1), height.max(1))
    };

    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| anyhow::anyhow!("Failed to create pixmap for SVG"))?;

    let transform =
        resvg::tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Convert RGBA premultiplied to standard RGBA.
    let rgba_data = pixmap.data().to_vec();
    let img_buf = image::RgbaImage::from_raw(w, h, rgba_data)
        .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer from SVG pixels"))?;

    Ok(image::DynamicImage::ImageRgba8(img_buf))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::path::PathBuf;

    use googletest::prelude::*;
    use rstest::*;

    use super::*;
    use crate::preview::provider::FileType;

    fn make_provider() -> ImagePreviewProvider {
        // Use fallback picker (halfblocks) for testing.
        ImagePreviewProvider::new(ImagePreviewProvider::fallback_picker())
    }

    // --- can_handle tests ---

    #[rstest]
    #[case("image.png", false, true)]
    #[case("image.jpg", false, true)]
    #[case("image.jpeg", false, true)]
    #[case("image.gif", false, true)]
    #[case("image.bmp", false, true)]
    #[case("image.webp", false, true)]
    #[case("image.svg", false, true)]
    #[case("image.ico", false, true)]
    #[case("image.tiff", false, true)]
    #[case("image.PNG", false, true)]
    #[case("code.rs", false, false)]
    #[case("data.txt", false, false)]
    #[case("dir", true, false)]
    fn can_handle_image_extensions(
        #[case] filename: &str,
        #[case] is_dir: bool,
        #[case] expected: bool,
    ) {
        let node =
            NodeInfo { file_type: if is_dir { FileType::Directory } else { FileType::File } };
        let provider = make_provider();
        assert_that!(provider.can_handle(&PathBuf::from(filename), &node), eq(expected));
    }

    // --- protocol correction tests ---

    #[rstest]
    #[case(ProtocolType::Iterm2, true, true, ProtocolType::Kitty)]
    #[case(ProtocolType::Halfblocks, true, true, ProtocolType::Kitty)]
    #[case(ProtocolType::Kitty, true, true, ProtocolType::Kitty)]
    #[case(ProtocolType::Iterm2, false, true, ProtocolType::Iterm2)]
    #[case(ProtocolType::Iterm2, true, false, ProtocolType::Iterm2)]
    #[case(ProtocolType::Sixel, true, false, ProtocolType::Sixel)]
    fn resolve_protocol_cases(
        #[case] detected: ProtocolType,
        #[case] cell_size_known: bool,
        #[case] in_herdr: bool,
        #[case] expected: ProtocolType,
    ) {
        assert_that!(resolve_protocol(detected, cell_size_known, in_herdr), eq(expected));
    }

    #[rstest]
    fn priority_is_builtin() {
        let provider = make_provider();
        assert_that!(provider.priority(), eq(crate::config::Priority::MID.value()));
    }
}
