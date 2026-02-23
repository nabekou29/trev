//! Column types, formatting, and width calculations for the tree view.

use std::time::SystemTime;

use ratatui::style::Color;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use unicode_width::UnicodeWidthStr;

/// One minute in seconds.
const MINUTE: u64 = 60;
/// One hour in seconds.
const HOUR: u64 = 3600;
/// One day in seconds.
const DAY: u64 = 86400;
/// One week in seconds.
const WEEK: u64 = 7 * DAY;
/// One month (30 days) in seconds.
const MONTH: u64 = 30 * DAY;

/// Kind of column that can be displayed in the tree view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ColumnKind {
    /// File size (human-readable).
    Size,
    /// Last modification time.
    ModifiedAt,
    /// Git status indicator.
    GitStatus,
}

impl ColumnKind {
    /// Fixed display width for this column kind (excluding separator).
    pub const fn width(self) -> usize {
        match self {
            Self::Size => 5,
            Self::ModifiedAt => 10,
            Self::GitStatus => 2,
        }
    }
}

/// Display format for the `ModifiedAt` column.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MtimeFormat {
    /// Tiered relative time: `Xm ago`, `Xh ago`, `Xd ago`, `Xw ago`, then ISO.
    #[default]
    Relative,
    /// Always ISO format (`YYYY-MM-DD`).
    Absolute,
}

/// How directory modification times are computed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MtimeMode {
    /// Use the OS modification time of the directory itself.
    #[default]
    OsMtime,
    /// Use the maximum mtime among all loaded descendant files (GitHub-like).
    RecursiveMax,
}

/// Per-column options for the `ModifiedAt` column.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ModifiedAtColumnOptions {
    /// Display format.
    pub format: MtimeFormat,
    /// How directory mtime is computed.
    pub mode: MtimeMode,
}

/// Per-column options configuration.
///
/// Each field corresponds to a column kind that supports options.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ColumnOptionsConfig {
    /// Options for the `ModifiedAt` column.
    pub modified_at: ModifiedAtColumnOptions,
}

/// A resolved column ready for rendering.
///
/// Created by merging column kind with per-column options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedColumn {
    /// Column kind.
    pub kind: ColumnKind,
    /// Format for `ModifiedAt` columns.
    pub mtime_format: MtimeFormat,
    /// How directory mtime is computed for `ModifiedAt` columns.
    pub mtime_mode: MtimeMode,
}

impl ResolvedColumn {
    /// Fixed display width for this column (delegates to `ColumnKind::width`).
    pub const fn width(self) -> usize {
        self.kind.width()
    }
}

/// Resolve a list of column kinds into resolved columns using the given options.
pub fn resolve_columns(kinds: &[ColumnKind], options: &ColumnOptionsConfig) -> Vec<ResolvedColumn> {
    kinds
        .iter()
        .map(|&kind| {
            let (mtime_format, mtime_mode) = match kind {
                ColumnKind::ModifiedAt => (options.modified_at.format, options.modified_at.mode),
                _ => (MtimeFormat::default(), MtimeMode::default()),
            };
            ResolvedColumn { kind, mtime_format, mtime_mode }
        })
        .collect()
}

/// Calculate the total width needed for all columns (including separators).
///
/// Each column has a 1-char separator before it.
pub fn total_columns_width(columns: &[ResolvedColumn]) -> usize {
    if columns.is_empty() {
        return 0;
    }
    columns.iter().map(|c| c.width() + 1).sum()
}

/// Default column kinds: `[GitStatus, Size, ModifiedAt]`.
pub fn default_columns() -> Vec<ColumnKind> {
    vec![ColumnKind::GitStatus, ColumnKind::Size, ColumnKind::ModifiedAt]
}

// ---------------------------------------------------------------------------
// Formatting functions
// ---------------------------------------------------------------------------

/// Format a float value with a unit suffix to produce a fixed 5-char string.
///
/// - `< 10`: 1 decimal place, right-aligned (e.g., ` 3.6K`)
/// - `10..100`: 1 decimal place, right-aligned (e.g., `15.4M`)
/// - `100..10000`: no decimal, right-aligned (e.g., ` 111K`)
/// - `>= 10000`: clamped to `9999` (e.g., `9999T`)
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_arithmetic
)]
fn format_with_unit(val: f64, unit: char) -> String {
    if val < 100.0 {
        format!("{val:>4.1}{unit}")
    } else {
        let rounded = val.round().min(9999.0) as u64;
        format!("{rounded:>4}{unit}")
    }
}

/// Format a file size as a human-readable string (fixed 5-char, right-aligned).
///
/// Directories should pass `is_dir = true` to get `"    -"`.
#[allow(clippy::cast_precision_loss, clippy::float_arithmetic)]
pub fn format_size(size: u64, is_dir: bool) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    const TB: u64 = 1024 * 1024 * 1024 * 1024;

    if is_dir {
        return "    -".to_string();
    }

    if size < KB {
        format!("{size:>4}B")
    } else if size < MB {
        format_with_unit(size as f64 / KB as f64, 'K')
    } else if size < GB {
        format_with_unit(size as f64 / MB as f64, 'M')
    } else if size < TB {
        format_with_unit(size as f64 / GB as f64, 'G')
    } else {
        format_with_unit(size as f64 / TB as f64, 'T')
    }
}

/// Format a modification time as a human-readable string (fixed 10-char, right-aligned).
///
/// `None` → `"         -"`.
///
/// In `Relative` mode: tiered relative time → ISO for old files.
/// In `Absolute` mode: always ISO (`YYYY-MM-DD`).
#[allow(clippy::float_arithmetic)]
pub fn format_mtime(modified: Option<SystemTime>, fmt: MtimeFormat) -> String {
    let Some(mtime) = modified else {
        return format!("{:>10}", "-");
    };

    let now = SystemTime::now();
    let elapsed = now.duration_since(mtime).unwrap_or_default();
    let secs = elapsed.as_secs();

    match fmt {
        MtimeFormat::Absolute => format_mtime_iso(mtime),
        MtimeFormat::Relative => {
            let relative = if secs < HOUR {
                format!("{}m ago", secs / MINUTE)
            } else if secs < DAY {
                format!("{}h ago", secs / HOUR)
            } else if secs < WEEK {
                format!("{}d ago", secs / DAY)
            } else if secs < MONTH {
                format!("{}w ago", secs / WEEK)
            } else {
                return format_mtime_iso(mtime);
            };
            format!("{relative:>10}")
        }
    }
}

/// Format a `SystemTime` as ISO date `YYYY-MM-DD` (10 chars).
fn format_mtime_iso(mtime: SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Local> = mtime.into();
    datetime.format("%Y-%m-%d").to_string()
}

/// Color for the modification time based on recency.
///
/// Uses a green gradient: brighter for recent files, dimmer for older ones.
/// Files older than 30 days fall back to `DarkGray`.
pub fn mtime_color(modified: Option<SystemTime>) -> Color {
    let Some(mtime) = modified else {
        return Color::DarkGray;
    };
    let elapsed = SystemTime::now().duration_since(mtime).unwrap_or_default();
    let secs = elapsed.as_secs();

    if secs < HOUR {
        Color::Rgb(120, 200, 120)
    } else if secs < DAY {
        Color::Rgb(90, 155, 90)
    } else if secs < WEEK {
        Color::Rgb(65, 115, 65)
    } else if secs < MONTH {
        Color::Rgb(50, 85, 50)
    } else {
        Color::DarkGray
    }
}

/// Truncate a string to fit within `max_width` display columns.
///
/// If the string fits, returns it unchanged. Otherwise, truncates and appends `…`.
/// Handles CJK characters (2-width) correctly.
pub fn truncate_to_width(text: &str, max_width: usize) -> String {
    let text_width = UnicodeWidthStr::width(text);
    if text_width <= max_width {
        return text.to_string();
    }

    if max_width == 0 {
        return String::new();
    }

    // Need at least 1 char width for "…"
    let target_width = max_width.saturating_sub(1);
    let mut current_width: usize = 0;
    let mut result = String::new();

    for ch in text.chars() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + ch_width > target_width {
            break;
        }
        result.push(ch);
        current_width += ch_width;
    }

    // Guard: string-level width may differ from sum of char widths for
    // certain Unicode sequences (e.g. combining marks, variation selectors).
    // Trim characters until the result + "…" fits within max_width.
    while UnicodeWidthStr::width(result.as_str()) + 1 > max_width {
        if result.pop().is_none() {
            break;
        }
    }

    result.push('…');
    result
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::time::Duration;

    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    // =========================================================================
    // --- ColumnKind, MtimeFormat, MtimeMode, ResolvedColumn ---
    // =========================================================================

    #[rstest]
    fn column_kind_widths() {
        assert_that!(ColumnKind::Size.width(), eq(5));
        assert_that!(ColumnKind::ModifiedAt.width(), eq(10));
        assert_that!(ColumnKind::GitStatus.width(), eq(2));
    }

    #[rstest]
    fn total_columns_width_all_defaults() {
        let cols = resolve_columns(&default_columns(), &ColumnOptionsConfig::default());
        // (2+1) + (5+1) + (10+1) = 20
        assert_that!(total_columns_width(&cols), eq(20));
    }

    #[rstest]
    fn total_columns_width_empty() {
        assert_that!(total_columns_width(&[]), eq(0));
    }

    #[rstest]
    fn total_columns_width_single() {
        let cols = resolve_columns(&[ColumnKind::Size], &ColumnOptionsConfig::default());
        assert_that!(total_columns_width(&cols), eq(6)); // 5+1
    }

    #[rstest]
    fn resolve_columns_default_options() {
        let cols = resolve_columns(&[ColumnKind::ModifiedAt], &ColumnOptionsConfig::default());
        assert_that!(cols.len(), eq(1));
        assert_that!(cols[0].kind, eq(ColumnKind::ModifiedAt));
        assert_that!(cols[0].mtime_format, eq(MtimeFormat::Relative));
        assert_that!(cols[0].mtime_mode, eq(MtimeMode::OsMtime));
    }

    #[rstest]
    fn resolve_columns_with_custom_options() {
        let options = ColumnOptionsConfig {
            modified_at: ModifiedAtColumnOptions {
                format: MtimeFormat::Absolute,
                mode: MtimeMode::RecursiveMax,
            },
        };
        let cols = resolve_columns(&[ColumnKind::Size, ColumnKind::ModifiedAt], &options);
        assert_that!(cols.len(), eq(2));
        // Size column ignores mtime options.
        assert_that!(cols[0].mtime_format, eq(MtimeFormat::Relative));
        assert_that!(cols[0].mtime_mode, eq(MtimeMode::OsMtime));
        // ModifiedAt gets custom options.
        assert_that!(cols[1].mtime_format, eq(MtimeFormat::Absolute));
        assert_that!(cols[1].mtime_mode, eq(MtimeMode::RecursiveMax));
    }

    #[rstest]
    fn column_kind_serde_round_trip() {
        for kind in [ColumnKind::Size, ColumnKind::ModifiedAt, ColumnKind::GitStatus] {
            let yaml = serde_yaml_ng::to_string(&kind).unwrap();
            let back: ColumnKind = serde_yaml_ng::from_str(&yaml).unwrap();
            assert_eq!(kind, back);
        }
    }

    #[rstest]
    fn mtime_mode_serde_round_trip() {
        for mode in [MtimeMode::OsMtime, MtimeMode::RecursiveMax] {
            let yaml = serde_yaml_ng::to_string(&mode).unwrap();
            let back: MtimeMode = serde_yaml_ng::from_str(&yaml).unwrap();
            assert_eq!(mode, back);
        }
    }

    #[rstest]
    fn column_options_config_serde() {
        let yaml = "modified_at:\n  format: absolute\n  mode: recursive_max";
        let config: ColumnOptionsConfig = serde_yaml_ng::from_str(yaml).unwrap();
        assert_that!(config.modified_at.format, eq(MtimeFormat::Absolute));
        assert_that!(config.modified_at.mode, eq(MtimeMode::RecursiveMax));
    }

    #[rstest]
    fn column_options_config_defaults() {
        let yaml = "";
        let config: ColumnOptionsConfig = serde_yaml_ng::from_str(yaml).unwrap();
        assert_that!(config.modified_at.format, eq(MtimeFormat::Relative));
        assert_that!(config.modified_at.mode, eq(MtimeMode::OsMtime));
    }

    // =========================================================================
    // --- format_size ---
    // =========================================================================

    #[rstest]
    #[case(0, false, "   0B")]
    #[case(42, false, "  42B")]
    #[case(1023, false, "1023B")]
    #[case(1024, false, " 1.0K")]
    #[case(1536, false, " 1.5K")]
    #[case(114_000, false, " 111K")]
    #[case(16_168_550, false, "15.4M")]
    #[case(1_048_576, false, " 1.0M")]
    #[case(1_073_741_824, false, " 1.0G")]
    #[case(112_742_891_520, false, " 105G")]
    #[case(1_099_511_627_776, false, " 1.0T")]
    fn format_size_values(#[case] size: u64, #[case] is_dir: bool, #[case] expected: &str) {
        assert_that!(format_size(size, is_dir).as_str(), eq(expected));
    }

    #[rstest]
    fn format_size_directory() {
        assert_that!(format_size(0, true).as_str(), eq("    -"));
        assert_that!(format_size(1024, true).as_str(), eq("    -"));
    }

    /// All `format_size` outputs must be exactly 5 chars wide.
    #[rstest]
    #[case(0)]
    #[case(512)]
    #[case(1023)]
    #[case(1024)]
    #[case(10_240)]
    #[case(114_000)]
    #[case(1_048_575)]
    #[case(1_048_576)]
    #[case(104_857_600)]
    #[case(1_073_741_824)]
    #[case(112_742_891_520)]
    #[case(1_099_511_627_776)]
    fn format_size_always_five_chars(#[case] size: u64) {
        let result = format_size(size, false);
        assert_that!(result.len(), eq(5));
    }

    mod prop {
        use proptest::prelude::*;

        use super::*;

        proptest! {
            /// format_size must always produce exactly 5 characters for any file size.
            #[test]
            fn format_size_width_is_always_five(size in 0..=u64::MAX) {
                let result = format_size(size, false);
                prop_assert_eq!(result.len(), 5, "format_size({}) = \"{}\" is {} chars", size, result, result.len());
            }

            /// format_size for directories must always produce exactly 5 characters.
            #[test]
            fn format_size_dir_width_is_always_five(size in 0..=u64::MAX) {
                let result = format_size(size, true);
                prop_assert_eq!(result.len(), 5);
            }

            /// `format_mtime` in Relative mode must always produce exactly 10 characters.
            #[test]
            fn format_mtime_relative_width_is_always_ten(secs in 0..=(365u64 * 24 * 3600 * 100)) {
                let mtime = SystemTime::now() - Duration::from_secs(secs);
                let result = format_mtime(Some(mtime), MtimeFormat::Relative);
                let width = UnicodeWidthStr::width(result.as_str());
                prop_assert_eq!(width, 10, "format_mtime({} secs ago) = \"{}\" is {} chars wide", secs, result, width);
            }

            /// `format_mtime` in Absolute mode must always produce exactly 10 characters.
            #[test]
            fn format_mtime_absolute_width_is_always_ten(secs in 0..=(365u64 * 24 * 3600 * 100)) {
                let mtime = SystemTime::now() - Duration::from_secs(secs);
                let result = format_mtime(Some(mtime), MtimeFormat::Absolute);
                let width = UnicodeWidthStr::width(result.as_str());
                prop_assert_eq!(width, 10, "format_mtime({} secs ago, absolute) = \"{}\" is {} chars wide", secs, result, width);
            }

            /// `truncate_to_width` output display width must never exceed `max_width`.
            #[test]
            fn truncate_never_exceeds_max_width(s in ".{0,200}", max_width in 0..100usize) {
                let result = truncate_to_width(&s, max_width);
                let width = UnicodeWidthStr::width(result.as_str());
                prop_assert!(width <= max_width, "truncate_to_width(\"{}\", {}) = \"{}\" has width {}", s, max_width, result, width);
            }
        }
    }

    // =========================================================================
    // --- format_mtime ---
    // =========================================================================

    #[rstest]
    fn format_mtime_none() {
        assert_that!(format_mtime(None, MtimeFormat::Relative).as_str(), eq("         -"));
    }

    #[rstest]
    fn format_mtime_relative_minutes() {
        let mtime = SystemTime::now() - Duration::from_mins(30);
        let result = format_mtime(Some(mtime), MtimeFormat::Relative);
        assert!(result.contains("m ago"), "expected minutes: {result}");
        assert_that!(result.len(), eq(10));
    }

    #[rstest]
    fn format_mtime_relative_hours() {
        let mtime = SystemTime::now() - Duration::from_hours(2);
        let result = format_mtime(Some(mtime), MtimeFormat::Relative);
        assert!(result.contains("h ago"), "expected hours: {result}");
        assert_that!(result.len(), eq(10));
    }

    #[rstest]
    fn format_mtime_relative_days() {
        let mtime = SystemTime::now() - Duration::from_hours(72);
        let result = format_mtime(Some(mtime), MtimeFormat::Relative);
        assert!(result.contains("d ago"), "expected days: {result}");
        assert_that!(result.len(), eq(10));
    }

    #[rstest]
    fn format_mtime_relative_weeks() {
        let mtime = SystemTime::now() - Duration::from_hours(504);
        let result = format_mtime(Some(mtime), MtimeFormat::Relative);
        assert!(result.contains("w ago"), "expected weeks: {result}");
        assert_that!(result.len(), eq(10));
    }

    #[rstest]
    fn format_mtime_relative_old_is_iso() {
        let mtime = SystemTime::now() - Duration::from_hours(1440);
        let result = format_mtime(Some(mtime), MtimeFormat::Relative);
        // ISO format: YYYY-MM-DD, exactly 10 chars.
        assert_that!(result.len(), eq(10));
        assert!(result.contains('-'), "expected ISO date: {result}");
    }

    #[rstest]
    fn format_mtime_absolute_always_iso() {
        let mtime = SystemTime::now() - Duration::from_mins(1);
        let result = format_mtime(Some(mtime), MtimeFormat::Absolute);
        // Should be ISO even for recent files.
        assert_that!(result.len(), eq(10));
        assert!(result.contains('-'), "expected ISO date: {result}");
    }

    #[rstest]
    fn format_mtime_absolute_none() {
        assert_that!(format_mtime(None, MtimeFormat::Absolute).as_str(), eq("         -"));
    }

    // =========================================================================
    // --- truncate_to_width ---
    // =========================================================================

    #[rstest]
    fn truncate_to_width_fits() {
        assert_that!(truncate_to_width("hello", 10).as_str(), eq("hello"));
    }

    #[rstest]
    fn truncate_to_width_exact_fit() {
        assert_that!(truncate_to_width("hello", 5).as_str(), eq("hello"));
    }

    #[rstest]
    fn truncate_to_width_ascii_truncation() {
        let result = truncate_to_width("hello world", 8);
        assert_that!(result.as_str(), eq("hello w…"));
    }

    #[rstest]
    fn truncate_to_width_cjk() {
        // Each CJK char is 2 width. "日本語" = 6 width.
        let result = truncate_to_width("日本語", 5);
        // max_width=5, target_width=4, can fit "日本" (4 width) + "…" (1 width) = 5.
        assert_that!(result.as_str(), eq("日本…"));
    }

    #[rstest]
    fn truncate_to_width_cjk_boundary() {
        // "日本語test" = 6+4=10 width.
        // Truncate to 7: target=6, "日本語" (6 width) + "…" = 7.
        let result = truncate_to_width("日本語test", 7);
        assert_that!(result.as_str(), eq("日本語…"));
    }

    #[rstest]
    fn truncate_to_width_zero() {
        assert_that!(truncate_to_width("hello", 0).as_str(), eq(""));
    }

    #[rstest]
    fn truncate_to_width_one() {
        // max_width=1, target=0, no chars fit, just "…".
        assert_that!(truncate_to_width("hello", 1).as_str(), eq("…"));
    }

    #[rstest]
    fn truncate_to_width_empty_input() {
        assert_that!(truncate_to_width("", 10).as_str(), eq(""));
    }

    // =========================================================================
    // mtime_color
    // =========================================================================

    #[rstest]
    fn mtime_color_none_returns_dark_gray() {
        assert_that!(mtime_color(None), eq(Color::DarkGray));
    }

    #[rstest]
    fn mtime_color_under_one_hour() {
        let recent = SystemTime::now() - Duration::from_mins(30);
        assert_that!(mtime_color(Some(recent)), eq(Color::Rgb(120, 200, 120)));
    }

    #[rstest]
    fn mtime_color_under_one_day() {
        let today = SystemTime::now() - Duration::from_hours(6);
        assert_that!(mtime_color(Some(today)), eq(Color::Rgb(90, 155, 90)));
    }

    #[rstest]
    fn mtime_color_under_one_week() {
        let this_week = SystemTime::now() - Duration::from_hours(72);
        assert_that!(mtime_color(Some(this_week)), eq(Color::Rgb(65, 115, 65)));
    }

    #[rstest]
    fn mtime_color_under_one_month() {
        let this_month = SystemTime::now() - Duration::from_hours(504);
        assert_that!(mtime_color(Some(this_month)), eq(Color::Rgb(50, 85, 50)));
    }

    #[rstest]
    fn mtime_color_over_one_month() {
        let old = SystemTime::now() - Duration::from_hours(1440);
        assert_that!(mtime_color(Some(old)), eq(Color::DarkGray));
    }
}
