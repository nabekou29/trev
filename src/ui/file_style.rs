//! Per-file style matching using glob patterns and category-based defaults.
//!
//! Compiles user-defined file style rules into a `GlobSet` for efficient batch
//! matching, and resolves the final `Style` for each file by layering category
//! defaults, git status colors, and glob-matched overrides.

use anyhow::Context as _;
use globset::{
    Glob,
    GlobSet,
    GlobSetBuilder,
};
use ratatui::style::{
    Color,
    Modifier,
    Style,
};

use crate::config::{
    CategoryStyles,
    FileStyleRule,
    StyleConfig,
};
use crate::git::GitFileStatus;

/// Pre-compiled file style matcher for efficient per-file style lookups.
#[derive(Debug)]
pub struct FileStyleMatcher {
    /// Compiled glob set for batch matching against file names.
    glob_set: GlobSet,
    /// Ordered style configs (index corresponds to `glob_set` match indices).
    styles: Vec<StyleConfig>,
    /// Category-based default styles.
    category_styles: CategoryStyles,
}

impl FileStyleMatcher {
    /// Create a new matcher from user-defined rules and category styles.
    ///
    /// Compiles all glob patterns. Returns an error if any pattern is invalid.
    pub fn new(rules: &[FileStyleRule], category_styles: &CategoryStyles) -> anyhow::Result<Self> {
        let mut builder = GlobSetBuilder::new();
        let mut styles = Vec::with_capacity(rules.len());

        for rule in rules {
            for pattern in &rule.pattern {
                let glob = Glob::new(pattern)
                    .with_context(|| format!("invalid file style pattern: \"{pattern}\""))?;
                builder.add(glob);
                styles.push(rule.style);
            }
        }

        let glob_set = builder.build().context("failed to compile file style glob patterns")?;

        Ok(Self { glob_set, styles, category_styles: *category_styles })
    }

    /// Resolve the display style for a file name.
    ///
    /// Layering (highest priority wins):
    /// 1. Category default (directory / symlink / gitignored / file)
    /// 2. Git status color (fills fg only if not set by category or file rules)
    /// 3. File style rules (glob matches, later rules override earlier)
    pub fn resolve_style(
        &self,
        filename: &str,
        is_dir: bool,
        is_symlink: bool,
        is_ignored: bool,
        is_orphan: bool,
        git_status: Option<GitFileStatus>,
    ) -> Style {
        // 1. Start with category default.
        let category = self.resolve_category_style(is_dir, is_symlink, is_ignored, is_orphan);
        let mut style = apply_style_config(Style::default(), category);

        // 2. Apply git status color (only when category didn't set fg).
        let category_has_fg = category.fg.is_some();
        if !category_has_fg && let Some(status) = git_status {
            let git_style = self.git_status_style(status);
            style = apply_style_config(style, git_style);
        }

        // 3. Apply glob-matched file style rules (later rules win).
        let matches = self.glob_set.matches(filename);
        for &idx in &matches {
            if let Some(rule_style) = self.styles.get(idx) {
                style = apply_style_config(style, rule_style);
            }
        }

        style
    }

    /// Get the category style for a node based on its type flags.
    fn resolve_category_style(
        &self,
        is_dir: bool,
        is_symlink: bool,
        is_ignored: bool,
        is_orphan: bool,
    ) -> &StyleConfig {
        if is_orphan {
            &self.category_styles.orphan_symlink
        } else if is_ignored {
            &self.category_styles.gitignored
        } else if is_dir {
            &self.category_styles.directory
        } else if is_symlink {
            &self.category_styles.symlink
        } else {
            // Default file style: empty (no overrides).
            &DEFAULT_FILE_STYLE
        }
    }

    /// Get the git status style configuration.
    const fn git_status_style(&self, status: GitFileStatus) -> &StyleConfig {
        let gs = &self.category_styles.git_status;
        match status {
            GitFileStatus::Modified => &gs.modified,
            GitFileStatus::Staged => &gs.staged,
            GitFileStatus::StagedModified => &gs.staged_modified,
            GitFileStatus::Added => &gs.added,
            GitFileStatus::Deleted => &gs.deleted,
            GitFileStatus::Renamed => &gs.renamed,
            GitFileStatus::Untracked => &gs.untracked,
            GitFileStatus::Conflicted => &gs.conflicted,
        }
    }
}

/// Default style for regular files (no overrides).
static DEFAULT_FILE_STYLE: StyleConfig = StyleConfig {
    fg: None,
    bg: None,
    bold: false,
    dim: false,
    italic: false,
    underlined: false,
    crossed_out: false,
};

/// Apply a `StyleConfig` on top of an existing `Style`.
///
/// Only fields that are explicitly set in the config override the base style.
const fn apply_style_config(mut style: Style, config: &StyleConfig) -> Style {
    if let Some(ref fg) = config.fg {
        style = style.fg(fg.0);
    }
    if let Some(ref bg) = config.bg {
        style = style.bg(bg.0);
    }
    if config.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if config.dim {
        style = style.add_modifier(Modifier::DIM);
    }
    if config.italic {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if config.underlined {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    if config.crossed_out {
        style = style.add_modifier(Modifier::CROSSED_OUT);
    }
    style
}

/// Parse a hex color string (e.g., `"#D32F2F"`) into a ratatui `Color`.
///
/// Falls back to `Color::White` if the string cannot be parsed.
pub fn parse_hex_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return Color::White;
    }

    let Ok(r) = u8::from_str_radix(hex.get(..2).unwrap_or("ff"), 16) else {
        return Color::White;
    };
    let Ok(g) = u8::from_str_radix(hex.get(2..4).unwrap_or("ff"), 16) else {
        return Color::White;
    };
    let Ok(b) = u8::from_str_radix(hex.get(4..6).unwrap_or("ff"), 16) else {
        return Color::White;
    };

    Color::Rgb(r, g, b)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;
    use crate::config::ColorConfig;

    #[rstest]
    fn empty_rules_directory_gets_default_blue_bold() {
        let category = CategoryStyles::default();
        let matcher = FileStyleMatcher::new(&[], &category).unwrap();
        let style = matcher.resolve_style("src", true, false, false, false, None);
        assert_that!(style.fg.unwrap(), eq(Color::Blue));
        assert_that!(style.add_modifier.contains(Modifier::BOLD), eq(true));
    }

    #[rstest]
    fn empty_rules_symlink_gets_cyan() {
        let category = CategoryStyles::default();
        let matcher = FileStyleMatcher::new(&[], &category).unwrap();
        let style = matcher.resolve_style("link.txt", false, true, false, false, None);
        assert_that!(style.fg.unwrap(), eq(Color::Cyan));
    }

    #[rstest]
    fn empty_rules_ignored_gets_dark_gray() {
        let category = CategoryStyles::default();
        let matcher = FileStyleMatcher::new(&[], &category).unwrap();
        let style = matcher.resolve_style("target", false, false, true, false, None);
        assert_that!(style.fg.unwrap(), eq(Color::DarkGray));
    }

    #[rstest]
    fn empty_rules_regular_file_no_git_is_plain() {
        let category = CategoryStyles::default();
        let matcher = FileStyleMatcher::new(&[], &category).unwrap();
        let style = matcher.resolve_style("main.rs", false, false, false, false, None);
        assert_that!(style.fg, eq(None));
        assert_that!(style.add_modifier, eq(Modifier::empty()));
    }

    #[rstest]
    fn git_status_applies_fg_to_regular_file() {
        let category = CategoryStyles::default();
        let matcher = FileStyleMatcher::new(&[], &category).unwrap();
        let style =
            matcher.resolve_style("main.rs", false, false, false, false, Some(GitFileStatus::Modified));
        assert_that!(style.fg.unwrap(), eq(Color::Yellow));
    }

    #[rstest]
    fn glob_rule_overrides_git_status_fg() {
        let category = CategoryStyles::default();
        let rules = vec![FileStyleRule {
            pattern: vec!["*.lock".to_string()],
            style: StyleConfig {
                fg: Some(ColorConfig(Color::DarkGray)),
                dim: true,
                ..StyleConfig::default()
            },
        }];
        let matcher = FileStyleMatcher::new(&rules, &category).unwrap();
        let style =
            matcher.resolve_style("Cargo.lock", false, false, false, false, Some(GitFileStatus::Modified));
        assert_that!(style.fg.unwrap(), eq(Color::DarkGray));
        assert_that!(style.add_modifier.contains(Modifier::DIM), eq(true));
    }

    #[rstest]
    fn glob_rule_without_fg_keeps_git_status_color() {
        let category = CategoryStyles::default();
        let rules = vec![FileStyleRule {
            pattern: vec!["*.rs".to_string()],
            style: StyleConfig { bold: true, ..StyleConfig::default() },
        }];
        let matcher = FileStyleMatcher::new(&rules, &category).unwrap();
        let style =
            matcher.resolve_style("main.rs", false, false, false, false, Some(GitFileStatus::Modified));
        assert_that!(style.fg.unwrap(), eq(Color::Yellow));
        assert_that!(style.add_modifier.contains(Modifier::BOLD), eq(true));
    }

    #[rstest]
    fn later_glob_rules_override_earlier() {
        let category = CategoryStyles::default();
        let rules = vec![
            FileStyleRule {
                pattern: vec!["*.md".to_string()],
                style: StyleConfig {
                    fg: Some(ColorConfig(Color::Green)),
                    ..StyleConfig::default()
                },
            },
            FileStyleRule {
                pattern: vec!["README*".to_string()],
                style: StyleConfig {
                    fg: Some(ColorConfig(Color::Yellow)),
                    bold: true,
                    ..StyleConfig::default()
                },
            },
        ];
        let matcher = FileStyleMatcher::new(&rules, &category).unwrap();
        let style = matcher.resolve_style("README.md", false, false, false, false, None);
        assert_that!(style.fg.unwrap(), eq(Color::Yellow));
        assert_that!(style.add_modifier.contains(Modifier::BOLD), eq(true));
    }

    #[rstest]
    fn orphan_symlink_gets_red_dim_style() {
        let category = CategoryStyles::default();
        let matcher = FileStyleMatcher::new(&[], &category).unwrap();
        let style = matcher.resolve_style("broken", false, true, false, true, None);
        assert_that!(style.fg.unwrap(), eq(Color::Red));
        assert_that!(style.add_modifier.contains(Modifier::DIM), eq(true));
    }

    #[rstest]
    fn orphan_takes_priority_over_ignored() {
        let category = CategoryStyles::default();
        let matcher = FileStyleMatcher::new(&[], &category).unwrap();
        let style = matcher.resolve_style("broken", false, true, true, true, None);
        // orphan should win over gitignored
        assert_that!(style.fg.unwrap(), eq(Color::Red));
    }

    #[rstest]
    fn parse_hex_color_valid() {
        let color = parse_hex_color("#D32F2F");
        assert_that!(color, eq(Color::Rgb(211, 47, 47)));
    }

    #[rstest]
    fn parse_hex_color_invalid_returns_white() {
        let color = parse_hex_color("invalid");
        assert_that!(color, eq(Color::White));
    }
}
