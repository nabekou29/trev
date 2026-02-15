//! Configuration file management.
//!
//! Loads settings from `$XDG_CONFIG_HOME/trev/config.toml` (or `~/.config/trev/config.toml`),
//! warns about unknown keys, and supports CLI argument overrides.

use std::path::{
    Path,
    PathBuf,
};

use anyhow::Context as _;
use serde::{
    Deserialize,
    Serialize,
};

use crate::cli::Args;

/// Application configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Sort settings.
    pub sort: SortConfig,
    /// Display settings.
    pub display: DisplayConfig,
    /// Preview settings.
    pub preview: PreviewConfig,
    /// File operation settings.
    pub file_operations: FileOpConfig,
    /// Session persistence settings.
    pub session: SessionConfig,
    /// File system watcher settings.
    pub watcher: WatcherConfig,
}

/// Sort configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SortConfig {
    /// Sort order field.
    pub order: SortOrder,
    /// Sort direction.
    pub direction: SortDirection,
    /// Whether directories come first.
    pub directories_first: bool,
}

/// Display configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    /// Show hidden files.
    pub show_hidden: bool,
    /// Show gitignored files.
    pub show_ignored: bool,
    /// Show preview panel.
    pub show_preview: bool,
}

/// Sort order variants.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    /// Sort by name.
    #[default]
    Name,
    /// Sort by file size.
    Size,
    /// Sort by modification time.
    Mtime,
    /// Sort by file type.
    Type,
    /// Sort by file extension.
    Extension,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    /// Ascending order.
    #[default]
    Asc,
    /// Descending order.
    Desc,
}

/// Preview configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PreviewConfig {
    /// Maximum number of lines to preview (default: 1000).
    pub max_lines: usize,
    /// Maximum bytes to read per file (default: 10 MB).
    pub max_bytes: u64,
    /// LRU cache capacity (default: 10).
    pub cache_size: usize,
    /// External preview commands.
    pub commands: Vec<ExternalCommand>,
    /// Timeout for external commands in seconds (default: 3).
    pub command_timeout: u64,
}

/// External command configuration for preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalCommand {
    /// Display name for this command provider.
    ///
    /// Defaults to the command binary name when omitted.
    #[serde(default)]
    pub name: Option<String>,
    /// File extensions this command applies to.
    pub extensions: Vec<String>,
    /// Command name (must be in `$PATH`).
    pub command: String,
    /// Command arguments.
    #[serde(default)]
    pub args: Vec<String>,
}

impl ExternalCommand {
    /// Get the display name for this command.
    ///
    /// Returns the explicit `name` if set, otherwise the command binary name.
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.command)
    }
}

/// File operation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FileOpConfig {
    /// Delete mode for the `d` key.
    pub delete_mode: DeleteMode,
    /// Maximum undo stack size.
    pub undo_stack_size: usize,
}

/// Delete mode variants.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeleteMode {
    /// Permanently delete files (undo not possible).
    #[default]
    Permanent,
    /// Move files to custom trash directory (undo possible).
    CustomTrash,
}

/// Session persistence configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionConfig {
    /// Whether to restore session by default on startup.
    pub restore_by_default: bool,
    /// Days before a session file expires and is auto-deleted.
    pub expiry_days: u64,
}

/// File system watcher configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WatcherConfig {
    /// Whether FS watching is enabled.
    pub enabled: bool,
    /// Debounce interval in milliseconds.
    pub debounce_ms: u64,
}

impl Default for FileOpConfig {
    fn default() -> Self {
        Self { delete_mode: DeleteMode::default(), undo_stack_size: 100 }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self { restore_by_default: true, expiry_days: 90 }
    }
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self { enabled: true, debounce_ms: 250 }
    }
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            max_lines: 1000,
            max_bytes: 10 * 1024 * 1024, // 10 MB
            cache_size: 10,
            commands: Vec::new(),
            command_timeout: 3,
        }
    }
}

impl Default for SortConfig {
    fn default() -> Self {
        Self {
            order: SortOrder::default(),
            direction: SortDirection::default(),
            directories_first: true,
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self { show_hidden: false, show_ignored: false, show_preview: true }
    }
}

/// Result of loading configuration.
///
/// Contains the parsed config and any warnings (e.g. unknown keys).
#[derive(Debug)]
pub struct ConfigLoadResult {
    /// The loaded (or default) configuration.
    pub config: Config,
    /// Warnings encountered during loading (unknown keys, etc.).
    pub warnings: Vec<String>,
}

impl Config {
    /// Load configuration from the default config file path.
    ///
    /// Returns default config with no warnings if the file does not exist.
    /// On parse error, returns `Err` with file path and line number in the message.
    pub(crate) fn load() -> anyhow::Result<ConfigLoadResult> {
        let path = Self::config_path();
        if path.exists() {
            return Self::load_from(&path);
        }
        Ok(ConfigLoadResult { config: Self::default(), warnings: Vec::new() })
    }

    /// Load configuration from a specific file path.
    ///
    /// Uses `serde_ignored` to detect unknown keys and collect them as warnings.
    pub(crate) fn load_from(path: &Path) -> anyhow::Result<ConfigLoadResult> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let mut unknown_keys = Vec::new();
        let deserializer = toml::Deserializer::parse(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
        let config: Self = serde_ignored::deserialize(deserializer, |key| {
            unknown_keys.push(key.to_string());
        })
        .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        let warnings: Vec<String> = unknown_keys
            .iter()
            .map(|key| format!("Unknown config key '{}' in {}", key, path.display()))
            .collect();

        Ok(ConfigLoadResult { config, warnings })
    }

    /// Apply CLI argument overrides to the configuration.
    ///
    /// Only overrides values for flags that were explicitly provided on the command line.
    pub(crate) const fn apply_cli_overrides(&mut self, args: &Args) {
        if args.show_hidden {
            self.display.show_hidden = true;
        }
        if args.show_ignored {
            self.display.show_ignored = true;
        }
        if args.no_preview {
            self.display.show_preview = false;
        }
        if let Some(order) = args.sort_order {
            self.sort.order = order;
        }
        if let Some(direction) = args.sort_direction {
            self.sort.direction = direction;
        }
        if args.no_directories_first {
            self.sort.directories_first = false;
        }
    }

    /// Get the default configuration directory path (XDG compliant).
    ///
    /// Resolves `$XDG_CONFIG_HOME/trev/` first, falls back to `~/.config/trev/`.
    fn config_dir() -> PathBuf {
        Self::resolve_config_dir(std::env::var("XDG_CONFIG_HOME").ok().as_deref(), dirs::home_dir())
    }

    /// Resolve config directory from explicit XDG and home directory values.
    ///
    /// Extracted for testability (avoids `unsafe` env var manipulation in tests).
    fn resolve_config_dir(xdg_config_home: Option<&str>, home_dir: Option<PathBuf>) -> PathBuf {
        xdg_config_home.map_or_else(
            || home_dir.unwrap_or_else(|| PathBuf::from(".")).join(".config/trev"),
            |xdg| PathBuf::from(xdg).join("trev"),
        )
    }

    /// Get the default configuration file path.
    fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    // --- T002: config_dir respects XDG_CONFIG_HOME ---

    #[rstest]
    fn config_dir_uses_xdg_config_home() {
        let dir = Config::resolve_config_dir(Some("/tmp/xdg_test"), None);
        assert_eq!(dir, PathBuf::from("/tmp/xdg_test/trev"));
    }

    // --- T003: config_dir falls back to ~/.config/trev ---

    #[rstest]
    fn config_dir_falls_back_to_home_config() {
        let home = PathBuf::from("/home/testuser");
        let dir = Config::resolve_config_dir(None, Some(home.clone()));
        assert_eq!(dir, home.join(".config/trev"));
    }

    // --- T003b: config_dir falls back to "." when no home dir ---

    #[rstest]
    fn config_dir_falls_back_to_dot_when_no_home() {
        let dir = Config::resolve_config_dir(None, None);
        assert_eq!(dir, PathBuf::from("./.config/trev"));
    }

    // --- T004: missing config file returns default ---

    #[rstest]
    fn load_missing_file_returns_default() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("nonexistent.toml");

        // load_from should error, but load() skips missing files.
        // Test via load_from with nonexistent path.
        let result = Config::load_from(&path);
        assert!(result.is_err());
    }

    // --- T005: empty config file applies all defaults ---

    #[rstest]
    fn empty_config_applies_defaults() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, "").unwrap();

        let result = Config::load_from(&path).unwrap();
        assert_that!(result.config.sort.directories_first, eq(true));
        assert_that!(result.config.display.show_hidden, eq(false));
        assert_that!(result.config.display.show_preview, eq(true));
        assert_that!(result.config.preview.max_lines, eq(1000));
        assert!(result.warnings.is_empty());
    }

    // --- T006: valid TOML deserializes correctly ---

    #[rstest]
    fn valid_toml_deserializes_all_fields() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[sort]
order = "size"
direction = "desc"
directories_first = false

[display]
show_hidden = true
show_ignored = true
show_preview = false

[preview]
max_lines = 500
max_bytes = 5242880
cache_size = 20
command_timeout = 5

[[preview.commands]]
extensions = ["json"]
command = "jq"
args = ["."]
"#,
        )
        .unwrap();

        let result = Config::load_from(&path).unwrap();
        let c = &result.config;

        assert_that!(c.sort.order, eq(SortOrder::Size));
        assert_that!(c.sort.direction, eq(SortDirection::Desc));
        assert_that!(c.sort.directories_first, eq(false));
        assert_that!(c.display.show_hidden, eq(true));
        assert_that!(c.display.show_ignored, eq(true));
        assert_that!(c.display.show_preview, eq(false));
        assert_that!(c.preview.max_lines, eq(500));
        assert_that!(c.preview.max_bytes, eq(5_242_880));
        assert_that!(c.preview.cache_size, eq(20));
        assert_that!(c.preview.command_timeout, eq(5));
        assert_that!(c.preview.commands.len(), eq(1));
        assert_that!(c.preview.commands[0].command.as_str(), eq("jq"));
        assert!(result.warnings.is_empty());
    }

    // --- T007: unknown keys produce warnings ---

    #[rstest]
    fn unknown_keys_produce_warnings() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[sort]
order = "name"
unknown_sort_key = true

[display]
show_hidden = false
typo_key = "oops"
"#,
        )
        .unwrap();

        let result = Config::load_from(&path).unwrap();

        // Config should still load correctly.
        assert_that!(result.config.sort.order, eq(SortOrder::Name));
        assert_that!(result.config.display.show_hidden, eq(false));

        // Should have 2 warnings.
        assert_that!(result.warnings.len(), eq(2));
        assert!(result.warnings.iter().any(|w| w.contains("unknown_sort_key")));
        assert!(result.warnings.iter().any(|w| w.contains("typo_key")));
    }

    // --- T008: syntax error includes file path and line info ---

    #[rstest]
    fn syntax_error_includes_file_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, "[sort\nbroken = true").unwrap();

        let err = Config::load_from(&path).unwrap_err();
        let msg = format!("{err:#}");

        // Should contain the file path.
        assert!(
            msg.contains(&path.display().to_string()),
            "Error message should contain file path: {msg}"
        );
    }

    // --- T009: permission error returns error ---

    #[rstest]
    #[cfg(unix)]
    fn permission_error_returns_error() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, "[sort]\norder = \"name\"").unwrap();

        // Remove read permission.
        let perms = std::fs::Permissions::from_mode(0o000);
        std::fs::set_permissions(&path, perms).unwrap();

        let result = Config::load_from(&path);
        assert!(result.is_err());

        // Restore permissions for cleanup.
        let perms = std::fs::Permissions::from_mode(0o644);
        std::fs::set_permissions(&path, perms).unwrap();
    }

    // --- T016: apply_cli_overrides sort order ---

    #[rstest]
    fn cli_override_sort_order() {
        let mut config = Config::default();
        let args = Args { sort_order: Some(SortOrder::Size), ..Args::default() };
        config.apply_cli_overrides(&args);

        assert_that!(config.sort.order, eq(SortOrder::Size));
    }

    // --- T017: apply_cli_overrides no-preview ---

    #[rstest]
    fn cli_override_no_preview() {
        let mut config = Config::default();
        assert_that!(config.display.show_preview, eq(true));

        let args = Args { no_preview: true, ..Args::default() };
        config.apply_cli_overrides(&args);

        assert_that!(config.display.show_preview, eq(false));
    }

    // --- T018: unset CLI args preserve TOML values ---

    #[rstest]
    fn unset_cli_args_preserve_toml() {
        let mut config = Config::default();
        config.sort.order = SortOrder::Mtime;
        config.display.show_preview = false;

        let args = Args::default();
        config.apply_cli_overrides(&args);

        // Nothing should change.
        assert_that!(config.sort.order, eq(SortOrder::Mtime));
        assert_that!(config.display.show_preview, eq(false));
    }

    // --- ExternalCommand display_name ---

    #[rstest]
    fn external_command_display_name_uses_explicit_name() {
        let cmd = ExternalCommand {
            name: Some("Pretty JSON".to_string()),
            extensions: vec!["json".to_string()],
            command: "jq".to_string(),
            args: vec![".".to_string()],
        };
        assert_that!(cmd.display_name(), eq("Pretty JSON"));
    }

    #[rstest]
    fn external_command_display_name_defaults_to_command() {
        let cmd = ExternalCommand {
            name: None,
            extensions: vec!["md".to_string()],
            command: "glow".to_string(),
            args: vec![],
        };
        assert_that!(cmd.display_name(), eq("glow"));
    }

    #[rstest]
    fn external_command_name_field_is_optional_in_toml() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
[[preview.commands]]
name = "Pretty JSON"
extensions = ["json"]
command = "jq"
args = ["."]

[[preview.commands]]
extensions = ["md"]
command = "glow"
"#,
        )
        .unwrap();

        let result = Config::load_from(&path).unwrap();
        let commands = &result.config.preview.commands;

        assert_that!(commands.len(), eq(2));
        assert_that!(commands[0].display_name(), eq("Pretty JSON"));
        assert_that!(commands[1].display_name(), eq("glow"));
        assert!(result.warnings.is_empty());
    }

    // --- T019: apply_cli_overrides show-ignored ---

    #[rstest]
    fn cli_override_show_ignored() {
        let mut config = Config::default();
        let args = Args { show_ignored: true, ..Args::default() };
        config.apply_cli_overrides(&args);

        assert_that!(config.display.show_ignored, eq(true));
    }
}
