//! Preview display state management.

use std::path::PathBuf;

use tokio_util::sync::CancellationToken;

use super::content::PreviewContent;

/// State of the preview panel.
///
/// Tracks the current content, scroll position, active provider,
/// and cancellation token for in-flight loads.
#[derive(Debug)]
pub struct PreviewState {
    /// Current preview content being displayed.
    pub content: PreviewContent,
    /// Vertical scroll offset (line index).
    pub scroll_row: usize,
    /// Horizontal scroll offset (column index).
    pub scroll_col: usize,
    /// Path of the file currently being previewed.
    pub current_path: Option<PathBuf>,
    /// Cancellation token for the current load operation.
    pub cancel_token: CancellationToken,
    /// Index of the active provider in the available list.
    pub active_provider_index: usize,
    /// Names of providers available for the current file.
    pub available_providers: Vec<String>,
    /// Whether word wrap is enabled for text preview.
    pub word_wrap: bool,
}

impl PreviewState {
    /// Create a new preview state with no content.
    pub fn new() -> Self {
        Self {
            content: PreviewContent::Empty,
            scroll_row: 0,
            scroll_col: 0,
            current_path: None,
            cancel_token: CancellationToken::new(),
            active_provider_index: 0,
            available_providers: Vec::new(),
            word_wrap: false,
        }
    }

    /// Request a preview for a new file.
    ///
    /// Cancels any in-flight load, resets scroll and provider index,
    /// and sets content to `Loading`.
    pub fn request_preview(&mut self, path: PathBuf) {
        self.active_provider_index = 0;
        self.begin_load(path);
    }

    /// Reload the current file with a different provider.
    ///
    /// Preserves the provider index (already updated by `cycle_provider`),
    /// but cancels the in-flight load and resets scroll.
    pub fn reload_preview(&mut self, path: PathBuf) {
        self.begin_load(path);
    }

    /// Common loading setup: cancel in-flight, reset scroll, set Loading.
    fn begin_load(&mut self, path: PathBuf) {
        self.cancel_token.cancel();
        self.cancel_token = CancellationToken::new();
        self.scroll_row = 0;
        self.scroll_col = 0;
        self.current_path = Some(path);
        self.content = PreviewContent::Loading;
    }

    /// Set the preview content (called when loading completes).
    pub fn set_content(&mut self, content: PreviewContent) {
        self.content = content;
    }

    /// Set the list of available provider names for the current file.
    pub fn set_available_providers(&mut self, providers: Vec<String>) {
        self.available_providers = providers;
    }

    /// Get the total number of content lines for scroll calculations.
    pub const fn content_line_count(&self) -> usize {
        match &self.content {
            PreviewContent::HighlightedText { lines, .. } => lines.len(),
            PreviewContent::PlainText { lines, .. } => lines.len(),
            PreviewContent::AnsiText { text } => text.lines.len(),
            _ => 0,
        }
    }

    /// Scroll down by `n` lines, clamping to content bounds.
    pub fn scroll_down(&mut self, n: usize, viewport_height: usize) {
        let max = self.content_line_count().saturating_sub(viewport_height);
        self.scroll_row = (self.scroll_row + n).min(max);
    }

    /// Scroll up by `n` lines, clamping at zero.
    pub const fn scroll_up(&mut self, n: usize) {
        self.scroll_row = self.scroll_row.saturating_sub(n);
    }

    /// Scroll right by `n` columns.
    pub const fn scroll_right(&mut self, n: usize) {
        self.scroll_col = self.scroll_col.saturating_add(n);
    }

    /// Scroll left by `n` columns, clamping at zero.
    pub const fn scroll_left(&mut self, n: usize) {
        self.scroll_col = self.scroll_col.saturating_sub(n);
    }

    /// Cycle to the next available provider (wrap-around).
    ///
    /// Returns `true` if the provider changed.
    pub const fn cycle_next_provider(&mut self) -> bool {
        if self.available_providers.len() <= 1 {
            return false;
        }
        self.active_provider_index =
            (self.active_provider_index + 1) % self.available_providers.len();
        self.scroll_row = 0;
        self.scroll_col = 0;
        true
    }

    /// Cycle to the previous available provider (wrap-around).
    ///
    /// Returns `true` if the provider changed.
    pub const fn cycle_prev_provider(&mut self) -> bool {
        if self.available_providers.len() <= 1 {
            return false;
        }
        let len = self.available_providers.len();
        self.active_provider_index = (self.active_provider_index + len - 1) % len;
        self.scroll_row = 0;
        self.scroll_col = 0;
        true
    }

    /// Get the name of the currently active provider, if any.
    pub fn active_provider_name(&self) -> Option<&str> {
        self.available_providers.get(self.active_provider_index).map(String::as_str)
    }
}

impl Default for PreviewState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    #[rstest]
    fn request_preview_resets_scroll() {
        let mut state = PreviewState::new();
        state.scroll_row = 10;
        state.scroll_col = 5;
        state.request_preview(PathBuf::from("/test.rs"));

        assert_that!(state.scroll_row, eq(0));
        assert_that!(state.scroll_col, eq(0));
    }

    #[rstest]
    fn request_preview_sets_loading() {
        let mut state = PreviewState::new();
        state.request_preview(PathBuf::from("/test.rs"));

        assert!(matches!(state.content, PreviewContent::Loading));
    }

    #[rstest]
    fn scroll_down_clamps_to_content() {
        let mut state = PreviewState::new();
        state.set_content(PreviewContent::PlainText {
            lines: (0..50).map(|i| format!("line {i}")).collect(),
            truncated: false,
        });

        // Viewport of 20 lines, max scroll = 50 - 20 = 30.
        state.scroll_down(100, 20);
        assert_that!(state.scroll_row, eq(30));
    }

    #[rstest]
    fn scroll_up_clamps_at_zero() {
        let mut state = PreviewState::new();
        state.scroll_up(10);
        assert_that!(state.scroll_row, eq(0));
    }

    #[rstest]
    fn scroll_right_and_left() {
        let mut state = PreviewState::new();
        state.scroll_right(5);
        assert_that!(state.scroll_col, eq(5));
        state.scroll_left(3);
        assert_that!(state.scroll_col, eq(2));
        state.scroll_left(10);
        assert_that!(state.scroll_col, eq(0));
    }

    #[rstest]
    fn cycle_provider_wraps_around() {
        let mut state = PreviewState::new();
        state.set_available_providers(vec!["Image".to_string(), "Text".to_string()]);

        assert_that!(state.active_provider_index, eq(0));
        assert_that!(state.cycle_next_provider(), eq(true));
        assert_that!(state.active_provider_index, eq(1));
        assert_that!(state.cycle_next_provider(), eq(true));
        assert_that!(state.active_provider_index, eq(0));
    }

    #[rstest]
    fn cycle_provider_single_provider_no_change() {
        let mut state = PreviewState::new();
        state.set_available_providers(vec!["Text".to_string()]);

        assert_that!(state.cycle_next_provider(), eq(false));
        assert_that!(state.active_provider_index, eq(0));
    }

    #[rstest]
    fn cycle_prev_provider_wraps_around() {
        let mut state = PreviewState::new();
        state.set_available_providers(vec!["Image".to_string(), "Text".to_string()]);

        assert_that!(state.active_provider_index, eq(0));
        assert_that!(state.cycle_prev_provider(), eq(true));
        assert_that!(state.active_provider_index, eq(1));
        assert_that!(state.cycle_prev_provider(), eq(true));
        assert_that!(state.active_provider_index, eq(0));
    }

    #[rstest]
    fn cycle_prev_provider_single_provider_no_change() {
        let mut state = PreviewState::new();
        state.set_available_providers(vec!["Text".to_string()]);

        assert_that!(state.cycle_prev_provider(), eq(false));
        assert_that!(state.active_provider_index, eq(0));
    }

    #[rstest]
    fn active_provider_name_returns_correct_name() {
        let mut state = PreviewState::new();
        state.set_available_providers(vec!["Image".to_string(), "Text".to_string()]);

        assert_that!(state.active_provider_name(), some(eq("Image")));
        state.cycle_next_provider();
        assert_that!(state.active_provider_name(), some(eq("Text")));
    }
}
