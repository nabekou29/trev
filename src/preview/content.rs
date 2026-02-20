//! Preview content types returned by providers.

use ratatui::text::{
    Line,
    Text,
};

/// Preview content produced by a provider's `load()` method.
///
/// Each variant represents a different kind of preview output.
pub enum PreviewContent {
    /// Syntax-highlighted text (syntect → ratatui Lines).
    HighlightedText {
        /// Highlighted lines ready for rendering.
        lines: Vec<Line<'static>>,
        /// Detected language name (e.g., "Rust", "Python").
        language: String,
        /// Whether the file was truncated due to size limits.
        truncated: bool,
    },
    /// Plain text without syntax highlighting.
    PlainText {
        /// Raw text lines.
        lines: Vec<String>,
        /// Whether the file was truncated due to size limits.
        truncated: bool,
    },
    /// ANSI-colored text from an external command (via ansi-to-tui).
    AnsiText {
        /// Parsed ANSI text ready for rendering.
        text: Text<'static>,
    },
    /// Image content (ratatui-image protocol).
    Image {
        /// Stateful image protocol for rendering.
        protocol: Box<ratatui_image::protocol::StatefulProtocol>,
    },
    /// Binary file — not displayable as text.
    Binary {
        /// File size in bytes.
        size: u64,
    },
    /// Directory information summary.
    Directory {
        /// Number of direct child entries.
        entry_count: usize,
        /// Total size of all entries in bytes.
        total_size: u64,
    },
    /// Loading in progress.
    Loading,
    /// An error occurred during loading.
    Error {
        /// Human-readable error message.
        message: String,
    },
    /// Empty file.
    Empty,
}

impl PreviewContent {
    /// Return a short type label for profiling.
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::HighlightedText { .. } => "highlighted_text",
            Self::PlainText { .. } => "plain_text",
            Self::AnsiText { .. } => "ansi_text",
            Self::Image { .. } => "image",
            Self::Binary { .. } => "binary",
            Self::Directory { .. } => "directory",
            Self::Loading => "loading",
            Self::Error { .. } => "error",
            Self::Empty => "empty",
        }
    }

    /// Try to clone this content.
    ///
    /// Returns `None` for `Image` (which contains a non-cloneable protocol).
    /// All other variants are cloned successfully.
    pub fn try_clone(&self) -> Option<Self> {
        match self {
            Self::HighlightedText { lines, language, truncated } => {
                Some(Self::HighlightedText {
                    lines: lines.clone(),
                    language: language.clone(),
                    truncated: *truncated,
                })
            }
            Self::PlainText { lines, truncated } => {
                Some(Self::PlainText { lines: lines.clone(), truncated: *truncated })
            }
            Self::AnsiText { text } => Some(Self::AnsiText { text: text.clone() }),
            Self::Image { .. } => None,
            Self::Binary { size } => Some(Self::Binary { size: *size }),
            Self::Directory { entry_count, total_size } => {
                Some(Self::Directory { entry_count: *entry_count, total_size: *total_size })
            }
            Self::Loading => Some(Self::Loading),
            Self::Error { message } => Some(Self::Error { message: message.clone() }),
            Self::Empty => Some(Self::Empty),
        }
    }
}

impl std::fmt::Debug for PreviewContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HighlightedText { language, truncated, lines } => f
                .debug_struct("HighlightedText")
                .field("lines_count", &lines.len())
                .field("language", language)
                .field("truncated", truncated)
                .finish(),
            Self::PlainText { truncated, lines } => f
                .debug_struct("PlainText")
                .field("lines_count", &lines.len())
                .field("truncated", truncated)
                .finish(),
            Self::AnsiText { .. } => f.debug_struct("AnsiText").finish_non_exhaustive(),
            Self::Image { .. } => f.debug_struct("Image").finish_non_exhaustive(),
            Self::Binary { size } => f.debug_struct("Binary").field("size", size).finish(),
            Self::Directory { entry_count, total_size } => f
                .debug_struct("Directory")
                .field("entry_count", entry_count)
                .field("total_size", total_size)
                .finish(),
            Self::Loading => write!(f, "Loading"),
            Self::Error { message } => f.debug_struct("Error").field("message", message).finish(),
            Self::Empty => write!(f, "Empty"),
        }
    }
}
