//! Syntax highlighting utilities — syntect to ratatui conversion.

use std::sync::LazyLock;

use ratatui::style::{
    Color,
    Modifier,
    Style as RatatuiStyle,
};
use ratatui::text::{
    Line,
    Span,
};
use syntect::easy::HighlightLines;
use syntect::highlighting::{
    FontStyle,
    Style as SyntectStyle,
};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use two_face::theme::{
    EmbeddedLazyThemeSet,
    EmbeddedThemeName,
};

/// Global syntax set with extended language support (200+ languages).
static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(two_face::syntax::extra_newlines);

/// Global theme set with extended themes.
static THEME_SET: LazyLock<EmbeddedLazyThemeSet> = LazyLock::new(two_face::theme::extra);

/// Convert a syntect style to a ratatui style.
///
/// Only applies foreground color and font modifiers.
/// Background is intentionally skipped to let the terminal's
/// native background show through.
fn syntect_to_ratatui_style(style: &SyntectStyle) -> RatatuiStyle {
    let mut s = RatatuiStyle::default().fg(Color::Rgb(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    ));

    if style.font_style.contains(FontStyle::BOLD) {
        s = s.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        s = s.add_modifier(Modifier::ITALIC);
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        s = s.add_modifier(Modifier::UNDERLINED);
    }

    s
}

/// Cached theme name, detected once at first use.
///
/// `terminal_light::luma()` sends an OSC 11 query to the terminal.
/// This must only run once — repeated queries during the event loop
/// produce responses on stdin that crossterm misinterprets as key events.
static AUTO_THEME: LazyLock<EmbeddedThemeName> = LazyLock::new(|| {
    let is_light = terminal_light::luma().is_ok_and(|l| l > 0.6);
    if is_light { EmbeddedThemeName::Base16OceanLight } else { EmbeddedThemeName::Base16OceanDark }
});

/// Force-initialize the theme cache.
///
/// Must be called before entering raw mode, as `terminal_light::luma()`
/// sends an OSC 11 query whose response would pollute stdin in raw mode.
pub fn init_theme() {
    let _ = *AUTO_THEME;
}

/// Get a reference to the global syntax set.
#[allow(dead_code)]
pub fn syntax_set() -> &'static SyntaxSet {
    &SYNTAX_SET
}

/// Highlight source code and return ratatui `Line`s.
///
/// Uses the syntax set to detect language by file extension,
/// falling back to plain text for unknown extensions.
pub fn highlight_lines(code: &str, extension: &str) -> Vec<Line<'static>> {
    let ss = &*SYNTAX_SET;
    // Index is safe: EmbeddedLazyThemeSet always contains all EmbeddedThemeName variants.
    #[allow(clippy::indexing_slicing)]
    let theme = &THEME_SET[*AUTO_THEME];

    let syntax = ss
        .find_syntax_by_extension(extension)
        .or_else(|| ss.find_syntax_by_token(extension))
        .unwrap_or_else(|| ss.find_syntax_plain_text());

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut result = Vec::new();

    for line in LinesWithEndings::from(code) {
        let ranges = highlighter.highlight_line(line, ss).unwrap_or_default();

        let spans: Vec<Span<'static>> = ranges
            .into_iter()
            .map(|(style, text)| Span::styled(text.to_string(), syntect_to_ratatui_style(&style)))
            .collect();

        result.push(Line::from(spans));
    }

    result
}

/// Detect the language name for a given file extension.
///
/// Returns the syntax name (e.g., "Rust", "Python") or "Plain Text"
/// if the extension is not recognized.
pub fn detect_language(extension: &str) -> String {
    let ss = &*SYNTAX_SET;
    ss.find_syntax_by_extension(extension)
        .or_else(|| ss.find_syntax_by_token(extension))
        .map_or_else(|| "Plain Text".to_string(), |s| s.name.clone())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {

    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    #[rstest]
    fn highlight_rust_code_produces_lines() {
        let code = "fn main() {\n    println!(\"hello\");\n}\n";
        let lines = highlight_lines(code, "rs");
        assert_that!(lines.len(), eq(3));
    }

    #[rstest]
    fn highlight_unknown_extension_produces_lines() {
        let code = "some content\n";
        let lines = highlight_lines(code, "zzzzunknown");
        assert_that!(lines.len(), eq(1));
    }

    #[rstest]
    fn detect_language_rust() {
        let lang = detect_language("rs");
        assert_that!(lang.as_str(), eq("Rust"));
    }

    #[rstest]
    fn detect_language_unknown() {
        let lang = detect_language("zzzzunknown");
        assert_that!(lang.as_str(), eq("Plain Text"));
    }

    #[rstest]
    fn detect_language_python() {
        let lang = detect_language("py");
        assert_that!(lang.as_str(), eq("Python"));
    }
}
