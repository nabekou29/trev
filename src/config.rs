//! Configuration file management.

use std::path::{
    Path,
    PathBuf,
};

use serde::{
    Deserialize,
    Serialize,
};

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
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
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
    /// File extensions this command applies to.
    pub extensions: Vec<String>,
    /// Command name (must be in `$PATH`).
    pub command: String,
    /// Command arguments.
    #[serde(default)]
    pub args: Vec<String>,
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
        Self {
            show_hidden: false,
            show_ignored: false,
            show_preview: true,
        }
    }
}

impl Config {
    /// Load configuration from the default config file path.
    ///
    /// Returns default config if the file does not exist.
    pub(crate) fn load() -> anyhow::Result<Self> {
        let path = Self::config_path();
        if let Some(path) = path.filter(|p| p.exists()) {
            return Self::load_from(&path);
        }
        Ok(Self::default())
    }

    /// Load configuration from a specific file path.
    pub(crate) fn load_from(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    /// Get the default configuration file path.
    fn config_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "trev")
            .map(|dirs| dirs.config_dir().join("config.toml"))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    #[rstest]
    fn test_default_config() -> Result<()> {
        let config = Config::default();
        verify_that!(config.sort.directories_first, eq(true))?;
        verify_that!(config.display.show_hidden, eq(false))?;
        verify_that!(config.display.show_preview, eq(true))?;
        Ok(())
    }

    #[rstest]
    fn test_config_deserialize() -> Result<()> {
        let toml_str = r#"
[sort]
order = "size"
direction = "desc"
directories_first = false

[display]
show_hidden = true
show_ignored = true
show_preview = false
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        verify_that!(config.sort.directories_first, eq(false))?;
        verify_that!(config.display.show_hidden, eq(true))?;
        verify_that!(config.display.show_preview, eq(false))?;
        Ok(())
    }
}
