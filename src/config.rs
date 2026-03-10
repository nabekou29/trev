//! Configuration file management.
//!
//! Loads settings from `$XDG_CONFIG_HOME/trev/config.yml` (or `~/.config/trev/config.yml`),
//! warns about unknown keys, and supports CLI argument overrides.

use std::collections::HashMap;
use std::path::{
    Path,
    PathBuf,
};

use anyhow::Context as _;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};

use crate::cli::Args;

/// Application configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Config {
    /// Sort settings.
    pub sort: SortConfig,
    /// Display settings.
    pub display: DisplayConfig,
    /// Preview settings.
    pub preview: PreviewConfig,
    /// File operation settings.
    pub file_op: FileOpConfig,
    /// Session persistence settings.
    pub session: SessionConfig,
    /// File system watcher settings.
    pub watcher: WatcherConfig,
    /// Mouse settings.
    pub mouse: MouseConfig,
    /// Keybinding customization.
    pub keybindings: KeybindingConfig,
    /// Git integration settings.
    pub git: GitConfig,
    /// Search settings.
    pub search: SearchConfig,
    /// User-defined menus.
    #[serde(default)]
    pub menus: HashMap<String, MenuDefinition>,
}

/// Keybinding configuration with context-based sections.
///
/// Each section groups bindings by context: `universal` (always active),
/// `file` (cursor on file), `directory` (cursor on directory), and
/// `daemon.*` (daemon mode variants).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct KeybindingConfig {
    /// Disable all default keybindings.
    pub disable_default: bool,
    /// Timeout in milliseconds for multi-key sequences (default: 500).
    pub key_sequence_timeout_ms: u64,
    /// Bindings active regardless of context.
    pub universal: ContextBindings,
    /// Bindings active when cursor is on a file.
    pub file: ContextBindings,
    /// Bindings active when cursor is on a directory.
    pub directory: ContextBindings,
    /// Bindings active in daemon mode.
    pub daemon: DaemonBindings,
}

/// Keybindings for a specific context.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ContextBindings {
    /// Disable default keybindings for this context.
    pub disable_default: bool,
    /// Keybinding entries.
    #[serde(default)]
    pub bindings: Vec<KeyBindingEntry>,
}

/// Daemon-mode keybindings, subdivided by node context.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct DaemonBindings {
    /// Bindings active in daemon mode regardless of node type.
    pub universal: ContextBindings,
    /// Bindings active in daemon mode when cursor is on a file.
    pub file: ContextBindings,
    /// Bindings active in daemon mode when cursor is on a directory.
    pub directory: ContextBindings,
}

/// A single keybinding entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBindingEntry {
    /// Key notation (e.g. `"j"`, `"<C-a>"`, `"G"`, `"<CR>"`).
    pub key: String,
    /// Built-in action name (e.g. `"tree.move_down"`, `"quit"`).
    #[serde(default)]
    pub action: Option<String>,
    /// Shell command to execute (template string).
    #[serde(default)]
    pub run: Option<String>,
    /// IPC notification method name.
    #[serde(default)]
    pub notify: Option<String>,
    /// User-defined menu name to open.
    #[serde(default)]
    pub menu: Option<String>,
    /// Run the shell command in the background without suspending the TUI.
    #[serde(default)]
    pub background: bool,
}

/// A user-defined menu definition.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MenuDefinition {
    /// Display title for the menu.
    pub title: String,
    /// Menu items.
    #[serde(default)]
    pub items: Vec<MenuItemDef>,
}

/// A single item in a user-defined menu.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MenuItemDef {
    /// Shortcut key character.
    pub key: String,
    /// Display label.
    pub label: String,
    /// Built-in action name.
    #[serde(default)]
    pub action: Option<String>,
    /// Shell command to execute (template string).
    #[serde(default)]
    pub run: Option<String>,
    /// IPC notification method name.
    #[serde(default)]
    pub notify: Option<String>,
    /// Run the shell command in the background without suspending the TUI.
    #[serde(default)]
    pub background: bool,
}

impl JsonSchema for KeyBindingEntry {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "KeyBindingEntry".into()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        concat!(module_path!(), "::KeyBindingEntry").into()
    }

    fn json_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let action_names: Vec<serde_json::Value> = crate::action::Action::all_action_names()
            .into_iter()
            .map(|s| serde_json::Value::String(s.to_string()))
            .collect();

        schemars::json_schema!({
            "type": "object",
            "description": "A keybinding entry. Exactly one of 'action', 'run', 'notify', or 'menu' must be set.",
            "required": ["key"],
            "properties": {
                "key": {
                    "type": "string",
                    "description": "Key notation (e.g. \"j\", \"<C-a>\", \"G\", \"<CR>\")"
                },
                "action": {
                    "anyOf": [
                        {
                            "type": "string",
                            "enum": action_names
                        },
                        { "type": "null" }
                    ],
                    "description": "Built-in action name"
                },
                "run": {
                    "anyOf": [
                        { "type": "string" },
                        { "type": "null" }
                    ],
                    "description": "Shell command template. Variables: {path}, {dir}, {name}, {root}"
                },
                "notify": {
                    "anyOf": [
                        { "type": "string" },
                        { "type": "null" }
                    ],
                    "description": "IPC notification method name"
                },
                "menu": {
                    "anyOf": [
                        { "type": "string" },
                        { "type": "null" }
                    ],
                    "description": "User-defined menu name to open"
                },
                "background": {
                    "type": "boolean",
                    "default": false,
                    "description": "Run shell command in background without suspending TUI"
                }
            },
            "additionalProperties": false
        })
    }
}

/// Sort configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "DisplayConfig aggregates independent display flags"
)]
pub struct DisplayConfig {
    /// Show hidden files.
    pub show_hidden: bool,
    /// Show gitignored files.
    pub show_ignored: bool,
    /// Show preview panel.
    pub show_preview: bool,
    /// Show file icons (Nerd Fonts).
    pub show_icons: bool,
    /// Show root directory as a tree node.
    pub show_root_entry: bool,
    /// Columns to display in the tree view (ordered list of column kinds).
    pub columns: Vec<crate::ui::column::ColumnKind>,
    /// Per-column options (e.g., format and mode for `modified_at`).
    pub column_options: crate::ui::column::ColumnOptionsConfig,
    /// Category-based default styles (directory, symlink, gitignored, git status).
    pub styles: CategoryStyles,
    /// Per-file style rules matched by glob pattern (applied in order; later rules win).
    #[serde(default)]
    pub file_styles: Vec<FileStyleRule>,
}

/// Category-based default display styles.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CategoryStyles {
    /// Style for directory names.
    pub directory: StyleConfig,
    /// Style for symbolic link names.
    pub symlink: StyleConfig,
    /// Style for gitignored file names.
    pub gitignored: StyleConfig,
    /// Per-status styles for git-tracked files.
    pub git_status: GitStatusStyles,
}

impl Default for CategoryStyles {
    fn default() -> Self {
        Self {
            directory: StyleConfig {
                fg: Some(ColorConfig(ratatui::style::Color::Blue)),
                bold: true,
                ..StyleConfig::default()
            },
            symlink: StyleConfig {
                fg: Some(ColorConfig(ratatui::style::Color::Cyan)),
                ..StyleConfig::default()
            },
            gitignored: StyleConfig {
                fg: Some(ColorConfig(ratatui::style::Color::DarkGray)),
                ..StyleConfig::default()
            },
            git_status: GitStatusStyles::default(),
        }
    }
}

/// Per-git-status display styles.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct GitStatusStyles {
    /// Style for unstaged modifications.
    pub modified: StyleConfig,
    /// Style for staged-only changes.
    pub staged: StyleConfig,
    /// Style for staged changes with working tree modifications.
    pub staged_modified: StyleConfig,
    /// Style for newly added files.
    pub added: StyleConfig,
    /// Style for deleted files.
    pub deleted: StyleConfig,
    /// Style for renamed files.
    pub renamed: StyleConfig,
    /// Style for untracked files.
    pub untracked: StyleConfig,
    /// Style for conflicted files.
    pub conflicted: StyleConfig,
}

impl Default for GitStatusStyles {
    fn default() -> Self {
        Self {
            modified: StyleConfig {
                fg: Some(ColorConfig(ratatui::style::Color::Yellow)),
                ..StyleConfig::default()
            },
            staged: StyleConfig {
                fg: Some(ColorConfig(ratatui::style::Color::Green)),
                ..StyleConfig::default()
            },
            staged_modified: StyleConfig {
                fg: Some(ColorConfig(ratatui::style::Color::Yellow)),
                ..StyleConfig::default()
            },
            added: StyleConfig {
                fg: Some(ColorConfig(ratatui::style::Color::Green)),
                ..StyleConfig::default()
            },
            deleted: StyleConfig {
                fg: Some(ColorConfig(ratatui::style::Color::Red)),
                ..StyleConfig::default()
            },
            renamed: StyleConfig {
                fg: Some(ColorConfig(ratatui::style::Color::Blue)),
                ..StyleConfig::default()
            },
            untracked: StyleConfig {
                fg: Some(ColorConfig(ratatui::style::Color::Magenta)),
                ..StyleConfig::default()
            },
            conflicted: StyleConfig {
                fg: Some(ColorConfig(ratatui::style::Color::Red)),
                bold: true,
                ..StyleConfig::default()
            },
        }
    }
}

/// A per-file style rule: glob pattern(s) paired with a display style.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStyleRule {
    /// Glob pattern(s) to match against the file name.
    ///
    /// Accepts a single string (e.g., `"*.rs"`) or an array (e.g., `["*_test.go", "*_test.ts"]`).
    #[serde(deserialize_with = "deserialize_one_or_many")]
    pub pattern: Vec<String>,
    /// Style to apply when any pattern matches.
    pub style: StyleConfig,
}

impl JsonSchema for FileStyleRule {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("FileStyleRule")
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        use schemars::json_schema;

        let pattern_schema = json_schema!({
            "description": "A glob pattern or list of glob patterns",
            "anyOf": [
                { "type": "string" },
                { "type": "array", "items": { "type": "string" } }
            ]
        });
        let style_schema = generator.subschema_for::<StyleConfig>();

        json_schema!({
            "type": "object",
            "properties": {
                "pattern": pattern_schema,
                "style": style_schema
            },
            "required": ["pattern", "style"]
        })
    }
}

/// Deserialize a value that can be either a single string or a list of strings.
fn deserialize_one_or_many<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<String>, D::Error> {
    use serde::de;

    /// Visitor for deserializing a single string or a list of strings.
    struct OneOrManyVisitor;

    impl<'de> de::Visitor<'de> for OneOrManyVisitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a string or a list of strings")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Vec<String>, E> {
            Ok(vec![v.to_string()])
        }

        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<String>, A::Error> {
            let mut vec = Vec::with_capacity(seq.size_hint().unwrap_or(0));
            while let Some(item) = seq.next_element()? {
                vec.push(item);
            }
            Ok(vec)
        }
    }

    deserializer.deserialize_any(OneOrManyVisitor)
}

/// Display style configuration for file names.
///
/// All fields are optional/defaulted so partial overrides work naturally in YAML.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "StyleConfig aggregates independent text modifier flags"
)]
pub struct StyleConfig {
    /// Foreground color (named color or hex string like `"#D32F2F"`).
    pub fg: Option<ColorConfig>,
    /// Background color (named color or hex string like `"#D32F2F"`).
    pub bg: Option<ColorConfig>,
    /// Bold text modifier.
    pub bold: bool,
    /// Dim (reduced brightness) text modifier.
    pub dim: bool,
    /// Italic text modifier.
    pub italic: bool,
    /// Underlined text modifier.
    pub underlined: bool,
    /// Strikethrough text modifier.
    pub crossed_out: bool,
}

/// A color value: named (e.g., `"red"`, `"blue"`) or hex (e.g., `"#D32F2F"`).
#[derive(Debug, Clone, Copy)]
pub struct ColorConfig(pub ratatui::style::Color);

/// Named colors supported in configuration files.
const NAMED_COLORS: &[(&str, ratatui::style::Color)] = &[
    ("black", ratatui::style::Color::Black),
    ("red", ratatui::style::Color::Red),
    ("green", ratatui::style::Color::Green),
    ("yellow", ratatui::style::Color::Yellow),
    ("blue", ratatui::style::Color::Blue),
    ("magenta", ratatui::style::Color::Magenta),
    ("cyan", ratatui::style::Color::Cyan),
    ("gray", ratatui::style::Color::Gray),
    ("dark_gray", ratatui::style::Color::DarkGray),
    ("light_red", ratatui::style::Color::LightRed),
    ("light_green", ratatui::style::Color::LightGreen),
    ("light_yellow", ratatui::style::Color::LightYellow),
    ("light_blue", ratatui::style::Color::LightBlue),
    ("light_magenta", ratatui::style::Color::LightMagenta),
    ("light_cyan", ratatui::style::Color::LightCyan),
    ("white", ratatui::style::Color::White),
];

impl Serialize for ColorConfig {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Try to serialize as a named color first, falling back to hex.
        for &(name, color) in NAMED_COLORS {
            if self.0 == color {
                return serializer.serialize_str(name);
            }
        }
        if let ratatui::style::Color::Rgb(r, g, b) = self.0 {
            return serializer.serialize_str(&format!("#{r:02X}{g:02X}{b:02X}"));
        }
        serializer.serialize_str("white")
    }
}

impl<'de> Deserialize<'de> for ColorConfig {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de;

        /// Visitor for deserializing `ColorConfig` from a named color or hex string.
        struct ColorVisitor;

        impl de::Visitor<'_> for ColorVisitor {
            type Value = ColorConfig;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a named color (e.g. \"red\") or hex color (e.g. \"#D32F2F\")")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<ColorConfig, E> {
                // Try named color first.
                for &(name, color) in NAMED_COLORS {
                    if v.eq_ignore_ascii_case(name) {
                        return Ok(ColorConfig(color));
                    }
                }
                // Try hex color.
                let hex = v.strip_prefix('#').unwrap_or(v);
                if hex.len() == 6 {
                    let r = u8::from_str_radix(hex.get(..2).unwrap_or("ff"), 16);
                    let g = u8::from_str_radix(hex.get(2..4).unwrap_or("ff"), 16);
                    let b = u8::from_str_radix(hex.get(4..6).unwrap_or("ff"), 16);
                    if let (Ok(r), Ok(g), Ok(b)) = (r, g, b) {
                        return Ok(ColorConfig(ratatui::style::Color::Rgb(r, g, b)));
                    }
                }
                let names: Vec<&str> = NAMED_COLORS.iter().map(|(n, _)| *n).collect();
                Err(de::Error::custom(format!(
                    "unknown color \"{v}\": expected a named color ({}) or hex (#RRGGBB)",
                    names.join(", ")
                )))
            }
        }

        deserializer.deserialize_str(ColorVisitor)
    }
}

impl JsonSchema for ColorConfig {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ColorConfig".into()
    }

    fn json_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "A color: named (red, blue, cyan, dark_gray, …) or hex (#RRGGBB)",
            "anyOf": [
                {
                    "type": "string",
                    "enum": [
                        "black", "red", "green", "yellow", "blue", "magenta", "cyan", "gray",
                        "dark_gray", "light_red", "light_green", "light_yellow", "light_blue",
                        "light_magenta", "light_cyan", "white"
                    ]
                },
                {
                    "type": "string",
                    "pattern": "^#[0-9a-fA-F]{6}$"
                }
            ]
        })
    }
}

/// Sort order variants.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, clap::ValueEnum,
)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    /// Natural sort with suffix grouping (test files after their base).
    #[default]
    Smart,
    /// Sort by name.
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
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, clap::ValueEnum,
)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    /// Ascending order.
    #[default]
    Asc,
    /// Descending order.
    Desc,
}

/// Preview configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
    /// Tree/preview split percentage in wide layout (default: 50).
    pub split_ratio: u16,
    /// Tree/preview split percentage in narrow layout (default: 60).
    pub narrow_split_ratio: u16,
    /// Width threshold for narrow layout in columns (default: 80).
    pub narrow_width: u16,
    /// Enable word wrap in preview (default: false).
    pub word_wrap: bool,
}

/// External command configuration for preview.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExternalCommand {
    /// Display name for this command provider.
    ///
    /// Defaults to the command binary name when omitted.
    #[serde(default)]
    pub name: Option<String>,
    /// File extensions this command applies to.
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Whether this command handles directories.
    #[serde(default)]
    pub directories: bool,
    /// Priority for ordering (lower = higher priority, default: 100).
    ///
    /// Accepts a number or a named constant: `high` (0), `mid` (100), `low` (1000).
    #[serde(default)]
    pub priority: Priority,
    /// Command name (must be in `$PATH`).
    pub command: String,
    /// Command arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Git status condition: only apply when file has one of these statuses.
    ///
    /// Values: `modified`, `staged`, `added`, `deleted`, `renamed`, `untracked`, `conflicted`.
    /// Empty means no git status filtering.
    #[serde(default)]
    pub git_status: Vec<String>,
}

/// Preview provider priority.
///
/// Can be specified as a raw number or a named constant in YAML:
/// - `high`: 0
/// - `mid`: 100 (default)
/// - `low`: 1000 (same as fallback provider)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Priority(pub u32);

impl Priority {
    /// High priority.
    pub const HIGH: Self = Self(0);
    /// Medium priority (default).
    pub const MID: Self = Self(100);
    /// Low priority (fallback provider).
    pub const LOW: Self = Self(1000);

    /// Get the numeric priority value.
    pub const fn value(self) -> u32 {
        self.0
    }
}

impl Default for Priority {
    fn default() -> Self {
        Self::MID
    }
}

impl Serialize for Priority {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.value().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Priority {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de;

        /// Visitor for deserializing `Priority` from a number or named constant.
        struct PriorityVisitor;

        impl de::Visitor<'_> for PriorityVisitor {
            type Value = Priority;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a number or a named constant (high, mid, low)")
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Priority, E> {
                u32::try_from(v).map(Priority).map_err(de::Error::custom)
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Priority, E> {
                u32::try_from(v).map(Priority).map_err(de::Error::custom)
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Priority, E> {
                match v {
                    "high" => Ok(Priority::HIGH),
                    "mid" => Ok(Priority::MID),
                    "low" => Ok(Priority::LOW),
                    _ => Err(de::Error::unknown_variant(v, &["high", "mid", "low"])),
                }
            }
        }

        deserializer.deserialize_any(PriorityVisitor)
    }
}

impl JsonSchema for Priority {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Priority".into()
    }

    fn json_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "Priority for ordering (lower = higher priority). Number or named constant: high (0), mid (100), low (1000)",
            "default": 100,
            "anyOf": [
                { "type": "integer", "minimum": 0 },
                { "type": "string", "enum": ["high", "mid", "low"] }
            ]
        })
    }
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct FileOpConfig {
    /// Delete mode for the `d` key.
    pub delete_mode: DeleteMode,
    /// Maximum undo stack size.
    pub undo_stack_size: usize,
}

/// Delete mode variants.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeleteMode {
    /// Permanently delete files (undo not possible).
    #[default]
    Permanent,
    /// Move files to custom trash directory (undo possible).
    CustomTrash,
}

/// Session persistence configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SessionConfig {
    /// Whether to restore session by default on startup.
    pub restore_by_default: bool,
    /// Days before a session file expires and is auto-deleted.
    pub expiry_days: u64,
}

/// File system watcher configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct WatcherConfig {
    /// Whether FS watching is enabled.
    pub enabled: bool,
    /// Debounce interval in milliseconds.
    pub debounce_ms: u64,
}

impl Default for KeybindingConfig {
    fn default() -> Self {
        Self {
            disable_default: false,
            key_sequence_timeout_ms: 500,
            universal: ContextBindings::default(),
            file: ContextBindings::default(),
            directory: ContextBindings::default(),
            daemon: DaemonBindings::default(),
        }
    }
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
            cache_size: 50,
            commands: Vec::new(),
            command_timeout: 3,
            split_ratio: 50,
            narrow_split_ratio: 60,
            narrow_width: 80,
            word_wrap: false,
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
            show_icons: true,
            show_root_entry: false,
            columns: crate::ui::column::default_columns(),
            column_options: crate::ui::column::ColumnOptionsConfig::default(),
            styles: CategoryStyles::default(),
            file_styles: Vec::new(),
        }
    }
}

/// Git integration configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct GitConfig {
    /// Whether Git integration is enabled.
    pub enabled: bool,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Mouse input configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct MouseConfig {
    /// Whether mouse support (click, scroll wheel) is enabled.
    pub enabled: bool,
}

impl Default for MouseConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Search configuration.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SearchConfig {}

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
        let deserializer = serde_yaml_ng::Deserializer::from_str(&content);
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
        if args.show_root_entry {
            self.display.show_root_entry = true;
        }
        if args.icons {
            self.display.show_icons = true;
        }
        if args.no_icons {
            self.display.show_icons = false;
        }
        if args.no_git {
            self.git.enabled = false;
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
        Self::config_dir().join("config.yml")
    }

    /// Generate JSON Schema for the configuration file.
    pub fn generate_schema() -> schemars::Schema {
        schemars::schema_for!(Self)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::literal_string_with_formatting_args
)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    // --- config_dir respects XDG_CONFIG_HOME ---

    #[rstest]
    fn config_dir_uses_xdg_config_home() {
        let dir = Config::resolve_config_dir(Some("/tmp/xdg_test"), None);
        assert_eq!(dir, PathBuf::from("/tmp/xdg_test/trev"));
    }

    // --- config_dir falls back to ~/.config/trev ---

    #[rstest]
    fn config_dir_falls_back_to_home_config() {
        let home = PathBuf::from("/home/testuser");
        let dir = Config::resolve_config_dir(None, Some(home.clone()));
        assert_eq!(dir, home.join(".config/trev"));
    }

    // --- config_dir falls back to "." when no home dir ---

    #[rstest]
    fn config_dir_falls_back_to_dot_when_no_home() {
        let dir = Config::resolve_config_dir(None, None);
        assert_eq!(dir, PathBuf::from("./.config/trev"));
    }

    // --- missing config file returns default ---

    #[rstest]
    fn load_missing_file_returns_default() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("nonexistent.yml");

        // load_from should error, but load() skips missing files.
        // Test via load_from with nonexistent path.
        let result = Config::load_from(&path);
        assert!(result.is_err());
    }

    // --- empty config file applies all defaults ---

    #[rstest]
    fn empty_config_applies_defaults() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(&path, "").unwrap();

        let result = Config::load_from(&path).unwrap();
        assert_that!(result.config.sort.order, eq(SortOrder::Smart));
        assert_that!(result.config.sort.directories_first, eq(true));
        assert_that!(result.config.display.show_hidden, eq(false));
        assert_that!(result.config.display.show_preview, eq(true));
        assert_that!(result.config.preview.max_lines, eq(1000));
        assert!(result.warnings.is_empty());
    }

    #[rstest]
    fn yaml_sort_order_smart_parses_correctly() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(&path, "sort:\n  order: smart\n").unwrap();

        let result = Config::load_from(&path).unwrap();
        assert_that!(result.config.sort.order, eq(SortOrder::Smart));
    }

    // --- valid YAML deserializes correctly ---

    #[rstest]
    fn valid_yaml_deserializes_all_fields() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(
            &path,
            r#"
sort:
  order: size
  direction: desc
  directories_first: false

display:
  show_hidden: true
  show_ignored: true
  show_preview: false

preview:
  max_lines: 500
  max_bytes: 5242880
  cache_size: 20
  command_timeout: 5
  commands:
    - extensions: [json]
      command: jq
      args: ["."]
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
        assert_that!(c.display.show_icons, eq(true)); // default when not specified in YAML
        assert_that!(c.preview.max_lines, eq(500));
        assert_that!(c.preview.max_bytes, eq(5_242_880));
        assert_that!(c.preview.cache_size, eq(20));
        assert_that!(c.preview.command_timeout, eq(5));
        assert_that!(c.preview.commands.len(), eq(1));
        assert_that!(c.preview.commands[0].command.as_str(), eq("jq"));
        assert!(result.warnings.is_empty());
    }

    // --- unknown keys produce warnings ---

    #[rstest]
    fn unknown_keys_produce_warnings() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(
            &path,
            r"
sort:
  order: name
  unknown_sort_key: true

display:
  show_hidden: false
  typo_key: oops
",
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

    // --- syntax error includes file path and line info ---

    #[rstest]
    fn syntax_error_includes_file_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(&path, "sort:\n  order: [invalid").unwrap();

        let err = Config::load_from(&path).unwrap_err();
        let msg = format!("{err:#}");

        // Should contain the file path.
        assert!(
            msg.contains(&path.display().to_string()),
            "Error message should contain file path: {msg}"
        );
    }

    // --- permission error returns error ---

    #[rstest]
    #[cfg(unix)]
    fn permission_error_returns_error() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(&path, "sort:\n  order: name").unwrap();

        // Remove read permission.
        let perms = std::fs::Permissions::from_mode(0o000);
        std::fs::set_permissions(&path, perms).unwrap();

        let result = Config::load_from(&path);
        assert!(result.is_err());

        // Restore permissions for cleanup.
        let perms = std::fs::Permissions::from_mode(0o644);
        std::fs::set_permissions(&path, perms).unwrap();
    }

    // --- apply_cli_overrides sort order ---

    #[rstest]
    fn cli_override_sort_order() {
        let mut config = Config::default();
        let args = Args { sort_order: Some(SortOrder::Size), ..Args::default() };
        config.apply_cli_overrides(&args);

        assert_that!(config.sort.order, eq(SortOrder::Size));
    }

    // --- apply_cli_overrides no-preview ---

    #[rstest]
    fn cli_override_no_preview() {
        let mut config = Config::default();
        assert_that!(config.display.show_preview, eq(true));

        let args = Args { no_preview: true, ..Args::default() };
        config.apply_cli_overrides(&args);

        assert_that!(config.display.show_preview, eq(false));
    }

    // --- unset CLI args preserve config values ---

    #[rstest]
    fn unset_cli_args_preserve_config() {
        let mut config = Config::default();
        config.sort.order = SortOrder::Mtime;
        config.display.show_preview = false;

        let args = Args::default();
        config.apply_cli_overrides(&args);

        // Nothing should change.
        assert_that!(config.sort.order, eq(SortOrder::Mtime));
        assert_that!(config.display.show_preview, eq(false));
        assert_that!(config.display.show_icons, eq(true));
    }

    // --- apply_cli_overrides with --no-git ---

    #[rstest]
    fn cli_override_no_git_disables_git() {
        let mut config = Config::default();
        assert_that!(config.git.enabled, eq(true));

        let args = Args { no_git: true, ..Args::default() };
        config.apply_cli_overrides(&args);

        assert_that!(config.git.enabled, eq(false));
    }

    #[rstest]
    fn cli_override_no_git_not_set_preserves_enabled() {
        let mut config = Config::default();
        let args = Args::default();
        config.apply_cli_overrides(&args);

        assert_that!(config.git.enabled, eq(true));
    }

    // --- ExternalCommand display_name ---

    #[rstest]
    fn external_command_display_name_uses_explicit_name() {
        let cmd = ExternalCommand {
            name: Some("Pretty JSON".to_string()),
            extensions: vec!["json".to_string()],
            directories: false,
            priority: Priority::default(),
            command: "jq".to_string(),
            args: vec![".".to_string()],
            git_status: vec![],
        };
        assert_that!(cmd.display_name(), eq("Pretty JSON"));
    }

    #[rstest]
    fn external_command_display_name_defaults_to_command() {
        let cmd = ExternalCommand {
            name: None,
            extensions: vec!["md".to_string()],
            directories: false,
            priority: Priority::default(),
            command: "glow".to_string(),
            args: vec![],
            git_status: vec![],
        };
        assert_that!(cmd.display_name(), eq("glow"));
    }

    #[rstest]
    fn external_command_name_field_is_optional_in_yaml() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(
            &path,
            r#"
preview:
  commands:
    - name: Pretty JSON
      extensions: [json]
      command: jq
      args: ["."]
    - extensions: [md]
      command: glow
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

    // --- apply_cli_overrides --icons ---

    #[rstest]
    fn cli_override_icons() {
        let mut config = Config::default();
        config.display.show_icons = false;
        let args = Args { icons: true, ..Args::default() };
        config.apply_cli_overrides(&args);

        assert_that!(config.display.show_icons, eq(true));
    }

    // --- apply_cli_overrides --no-icons ---

    #[rstest]
    fn cli_override_no_icons() {
        let mut config = Config::default();
        assert_that!(config.display.show_icons, eq(true));

        let args = Args { no_icons: true, ..Args::default() };
        config.apply_cli_overrides(&args);

        assert_that!(config.display.show_icons, eq(false));
    }

    // --- apply_cli_overrides show-ignored ---

    #[rstest]
    fn cli_override_show_ignored() {
        let mut config = Config::default();
        let args = Args { show_ignored: true, ..Args::default() };
        config.apply_cli_overrides(&args);

        assert_that!(config.display.show_ignored, eq(true));
    }

    // --- Keybinding config defaults ---

    #[rstest]
    fn keybinding_config_defaults() {
        let config = KeybindingConfig::default();
        assert!(!config.disable_default);
        assert_that!(config.key_sequence_timeout_ms, eq(500));
        assert!(config.universal.bindings.is_empty());
        assert!(config.file.bindings.is_empty());
        assert!(config.directory.bindings.is_empty());
        assert!(config.daemon.universal.bindings.is_empty());
        assert!(config.daemon.file.bindings.is_empty());
        assert!(config.daemon.directory.bindings.is_empty());
    }

    #[rstest]
    fn keybinding_config_from_yaml() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(
            &path,
            r#"
keybindings:
  disable_default: true
  universal:
    bindings:
      - key: j
        action: tree.move_down
      - key: o
        run: "open {path}"
  file:
    bindings:
      - key: "<CR>"
        notify: open_file
  directory:
    bindings:
      - key: "<CR>"
        action: tree.toggle_expand
  daemon:
    universal:
      bindings:
        - key: "<C-q>"
          notify: quit_request
"#,
        )
        .unwrap();

        let result = Config::load_from(&path).unwrap();
        let kb = &result.config.keybindings;

        assert!(kb.disable_default);

        // Universal bindings.
        assert_that!(kb.universal.bindings.len(), eq(2));
        assert_that!(kb.universal.bindings[0].key.as_str(), eq("j"));
        assert_that!(kb.universal.bindings[0].action.as_deref(), some(eq("tree.move_down")));
        assert!(kb.universal.bindings[0].run.is_none());
        assert!(kb.universal.bindings[0].notify.is_none());

        assert_that!(kb.universal.bindings[1].key.as_str(), eq("o"));
        assert_that!(kb.universal.bindings[1].run.as_deref(), some(eq("open {path}")));

        // File binding.
        assert_that!(kb.file.bindings.len(), eq(1));
        assert_that!(kb.file.bindings[0].key.as_str(), eq("<CR>"));
        assert_that!(kb.file.bindings[0].notify.as_deref(), some(eq("open_file")));

        // Directory binding.
        assert_that!(kb.directory.bindings.len(), eq(1));
        assert_that!(kb.directory.bindings[0].key.as_str(), eq("<CR>"));
        assert_that!(kb.directory.bindings[0].action.as_deref(), some(eq("tree.toggle_expand")));

        // Daemon universal binding.
        assert_that!(kb.daemon.universal.bindings.len(), eq(1));
        assert_that!(kb.daemon.universal.bindings[0].key.as_str(), eq("<C-q>"));
        assert_that!(kb.daemon.universal.bindings[0].notify.as_deref(), some(eq("quit_request")));

        assert!(result.warnings.is_empty());
    }

    #[rstest]
    fn keybinding_section_disable_default() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(
            &path,
            r#"
keybindings:
  file:
    disable_default: true
    bindings:
      - key: "<CR>"
        notify: open_file
  daemon:
    file:
      disable_default: true
"#,
        )
        .unwrap();

        let result = Config::load_from(&path).unwrap();
        let kb = &result.config.keybindings;

        assert!(!kb.disable_default);
        assert!(!kb.universal.disable_default);
        assert!(kb.file.disable_default);
        assert!(!kb.directory.disable_default);
        assert!(!kb.daemon.universal.disable_default);
        assert!(kb.daemon.file.disable_default);
        assert!(!kb.daemon.directory.disable_default);
    }

    #[rstest]
    fn empty_keybindings_uses_defaults() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(&path, "").unwrap();

        let result = Config::load_from(&path).unwrap();
        assert!(!result.config.keybindings.disable_default);
        assert!(result.config.keybindings.universal.bindings.is_empty());
    }

    // --- JSON Schema generation ---

    #[rstest]
    fn schema_generates_valid_json() {
        let schema = Config::generate_schema();
        let json = serde_json::to_string_pretty(&schema).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_object());
        assert!(parsed.get("$schema").is_some());
    }

    #[rstest]
    fn schema_contains_all_top_level_sections() {
        let schema = Config::generate_schema();
        let json = serde_json::to_value(&schema).unwrap();
        let props = json["properties"].as_object().unwrap();

        for key in
            ["sort", "display", "preview", "file_op", "session", "watcher", "keybindings", "git"]
        {
            assert!(props.contains_key(key), "missing top-level property: {key}");
        }
    }

    #[rstest]
    fn schema_sort_order_has_enum_values() {
        let schema = Config::generate_schema();
        let json = serde_json::to_value(&schema).unwrap();
        let sort_order = &json["$defs"]["SortOrder"];
        let json_str = sort_order.to_string();

        for val in ["smart", "name", "size", "mtime", "type", "extension"] {
            assert!(json_str.contains(val), "SortOrder missing variant: {val}");
        }
    }

    #[rstest]
    fn schema_action_field_has_enum_values() {
        let schema = Config::generate_schema();
        let json = serde_json::to_value(&schema).unwrap();
        let entry = &json["$defs"]["KeyBindingEntry"];
        let json_str = entry.to_string();

        for action in [
            "quit",
            "tree.move_down",
            "preview.scroll_down",
            "file_op.yank",
            "filter.hidden",
            "tree.sort.menu",
            "file_op.copy.menu",
        ] {
            assert!(json_str.contains(action), "action enum missing: {action}");
        }
    }

    #[rstest]
    fn schema_keybindings_has_nested_sections() {
        let schema = Config::generate_schema();
        let json = serde_json::to_value(&schema).unwrap();
        let json_str = json.to_string();

        // Top-level keybindings should have nested sections.
        for section in ["disable_default", "universal", "file", "directory", "daemon"] {
            assert!(json_str.contains(section), "keybindings missing section: {section}");
        }
    }

    // --- GitConfig defaults and deserialization ---

    #[rstest]
    fn git_config_defaults_to_enabled() {
        let config = GitConfig::default();
        assert_that!(config.enabled, eq(true));
    }

    #[rstest]
    fn git_config_from_yaml_enabled() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(&path, "git:\n  enabled: true\n").unwrap();

        let result = Config::load_from(&path).unwrap();
        assert_that!(result.config.git.enabled, eq(true));
    }

    #[rstest]
    fn git_config_from_yaml_disabled() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(&path, "git:\n  enabled: false\n").unwrap();

        let result = Config::load_from(&path).unwrap();
        assert_that!(result.config.git.enabled, eq(false));
    }

    // --- ExternalCommand deserialization with git_status ---

    #[rstest]
    fn external_command_git_status_from_yaml() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(
            &path,
            r"
preview:
  commands:
    - command: diff-so-fancy
      extensions: [rs, py]
      git_status: [modified, staged_modified]
",
        )
        .unwrap();

        let result = Config::load_from(&path).unwrap();
        let cmd = &result.config.preview.commands[0];
        assert_that!(cmd.git_status.len(), eq(2));
        assert_that!(cmd.git_status[0].as_str(), eq("modified"));
        assert_that!(cmd.git_status[1].as_str(), eq("staged_modified"));
    }

    #[rstest]
    fn external_command_git_status_defaults_to_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(
            &path,
            r"
preview:
  commands:
    - command: cat
      extensions: [txt]
",
        )
        .unwrap();

        let result = Config::load_from(&path).unwrap();
        let cmd = &result.config.preview.commands[0];
        assert!(cmd.git_status.is_empty());
    }

    #[rstest]
    fn git_config_defaults_when_section_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(&path, "").unwrap();

        let result = Config::load_from(&path).unwrap();
        assert_that!(result.config.git.enabled, eq(true));
    }

    // --- Display columns config ---

    #[rstest]
    fn display_columns_default_has_three_columns() {
        use crate::ui::column::ColumnKind;

        let config = DisplayConfig::default();
        assert_that!(config.columns.len(), eq(3));
        assert_that!(config.columns[0], eq(ColumnKind::GitStatus));
        assert_that!(config.columns[1], eq(ColumnKind::Size));
        assert_that!(config.columns[2], eq(ColumnKind::ModifiedAt));
    }

    #[rstest]
    fn display_columns_yaml_simple_form() {
        use crate::ui::column::ColumnKind;

        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(&path, "display:\n  columns:\n    - size\n").unwrap();

        let result = Config::load_from(&path).unwrap();
        assert_that!(result.config.display.columns.len(), eq(1));
        assert_that!(result.config.display.columns[0], eq(ColumnKind::Size));
    }

    #[rstest]
    fn display_columns_yaml_empty_list() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(&path, "display:\n  columns: []\n").unwrap();

        let result = Config::load_from(&path).unwrap();
        assert!(result.config.display.columns.is_empty());
    }

    #[rstest]
    fn display_columns_yaml_reordered() {
        use crate::ui::column::ColumnKind;

        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(
            &path,
            "display:\n  columns:\n    - git_status\n    - size\n    - modified_at\n",
        )
        .unwrap();

        let result = Config::load_from(&path).unwrap();
        let cols = &result.config.display.columns;
        assert_that!(cols.len(), eq(3));
        assert_that!(cols[0], eq(ColumnKind::GitStatus));
        assert_that!(cols[1], eq(ColumnKind::Size));
        assert_that!(cols[2], eq(ColumnKind::ModifiedAt));
    }

    #[rstest]
    fn display_column_options_yaml() {
        use crate::ui::column::{
            MtimeFormat,
            MtimeMode,
        };

        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(
            &path,
            "display:\n  columns:\n    - modified_at\n  column_options:\n    modified_at:\n      format: absolute\n      mode: recursive_max\n",
        )
        .unwrap();

        let result = Config::load_from(&path).unwrap();
        let opts = &result.config.display.column_options;
        assert_that!(opts.modified_at.format, eq(MtimeFormat::Absolute));
        assert_that!(opts.modified_at.mode, eq(MtimeMode::RecursiveMax));
    }

    #[rstest]
    fn display_column_options_defaults_when_omitted() {
        use crate::ui::column::{
            MtimeFormat,
            MtimeMode,
        };

        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(&path, "display:\n  columns:\n    - modified_at\n").unwrap();

        let result = Config::load_from(&path).unwrap();
        let opts = &result.config.display.column_options;
        assert_that!(opts.modified_at.format, eq(MtimeFormat::Relative));
        assert_that!(opts.modified_at.mode, eq(MtimeMode::OsMtime));
    }

    #[rstest]
    fn display_columns_yaml_unknown_column_errors() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(&path, "display:\n  columns:\n    - unknown_col\n").unwrap();

        let result = Config::load_from(&path);
        assert!(result.is_err());
    }

    // --- Schema display columns ---

    #[rstest]
    fn schema_display_has_columns_and_column_options() {
        let schema = Config::generate_schema();
        let json = serde_json::to_value(&schema).unwrap();
        let json_str = json.to_string();
        assert!(json_str.contains("columns"), "schema should contain columns field");
        assert!(json_str.contains("column_options"), "schema should contain column_options field");
    }

    #[rstest]
    fn preview_split_defaults() {
        let config = PreviewConfig::default();
        assert_that!(config.split_ratio, eq(50));
        assert_that!(config.narrow_split_ratio, eq(60));
        assert_that!(config.narrow_width, eq(80));
    }

    #[rstest]
    fn preview_split_yaml_overrides() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(
            &path,
            "preview:\n  split_ratio: 40\n  narrow_split_ratio: 70\n  narrow_width: 100\n",
        )
        .unwrap();
        let result = Config::load_from(&path).unwrap();
        assert_that!(result.config.preview.split_ratio, eq(40));
        assert_that!(result.config.preview.narrow_split_ratio, eq(70));
        assert_that!(result.config.preview.narrow_width, eq(100));
    }

    #[rstest]
    fn preview_split_partial_override_preserves_defaults() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(&path, "preview:\n  split_ratio: 35\n").unwrap();
        let result = Config::load_from(&path).unwrap();
        assert_that!(result.config.preview.split_ratio, eq(35));
        assert_that!(result.config.preview.narrow_split_ratio, eq(60));
        assert_that!(result.config.preview.narrow_width, eq(80));
    }

    #[rstest]
    fn preview_split_does_not_affect_existing_fields() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(&path, "preview:\n  max_lines: 500\n  split_ratio: 40\n").unwrap();
        let result = Config::load_from(&path).unwrap();
        assert_that!(result.config.preview.max_lines, eq(500));
        assert_that!(result.config.preview.split_ratio, eq(40));
        assert_that!(result.config.preview.cache_size, eq(50));
    }

    // --- MenuDefinition and MenuItemDef deserialization ---

    #[rstest]
    fn menu_definition_from_yaml() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(
            &path,
            r#"
menus:
  my_menu:
    title: My Custom Menu
    items:
      - key: a
        label: Sort by name
        action: tree.sort.by_name
      - key: b
        label: Run command
        run: "echo hello"
      - key: c
        label: Notify
        notify: open_file
"#,
        )
        .unwrap();

        let result = Config::load_from(&path).unwrap();
        let menu = result.config.menus.get("my_menu").unwrap();

        assert_that!(menu.title.as_str(), eq("My Custom Menu"));
        assert_that!(menu.items.len(), eq(3));

        assert_that!(menu.items[0].key.as_str(), eq("a"));
        assert_that!(menu.items[0].label.as_str(), eq("Sort by name"));
        assert_that!(menu.items[0].action.as_deref(), some(eq("tree.sort.by_name")));
        assert!(menu.items[0].run.is_none());
        assert!(menu.items[0].notify.is_none());

        assert_that!(menu.items[1].key.as_str(), eq("b"));
        assert_that!(menu.items[1].run.as_deref(), some(eq("echo hello")));

        assert_that!(menu.items[2].key.as_str(), eq("c"));
        assert_that!(menu.items[2].notify.as_deref(), some(eq("open_file")));
    }

    #[rstest]
    fn menu_definition_defaults_to_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(&path, "").unwrap();

        let result = Config::load_from(&path).unwrap();
        assert!(result.config.menus.is_empty());
    }

    #[rstest]
    fn menu_definition_multiple_menus() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(
            &path,
            r#"
menus:
  sort:
    title: Sort Menu
    items:
      - key: n
        label: By name
        action: tree.sort.by_name
  tools:
    title: Tools
    items:
      - key: g
        label: Git status
        run: "git status"
"#,
        )
        .unwrap();

        let result = Config::load_from(&path).unwrap();
        assert_that!(result.config.menus.len(), eq(2));
        assert!(result.config.menus.contains_key("sort"));
        assert!(result.config.menus.contains_key("tools"));
    }

    // --- KeyBindingEntry with menu field ---

    #[rstest]
    fn keybinding_entry_with_menu_field() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("config.yml");
        std::fs::write(
            &path,
            r"
keybindings:
  universal:
    bindings:
      - key: m
        menu: my_menu
",
        )
        .unwrap();

        let result = Config::load_from(&path).unwrap();
        let bindings = &result.config.keybindings.universal.bindings;
        assert_that!(bindings.len(), eq(1));
        assert_that!(bindings[0].key.as_str(), eq("m"));
        assert_that!(bindings[0].menu.as_deref(), some(eq("my_menu")));
        assert!(bindings[0].action.is_none());
        assert!(bindings[0].run.is_none());
        assert!(bindings[0].notify.is_none());
    }

    // --- Schema includes menus section ---

    #[rstest]
    fn schema_contains_menus_section() {
        let schema = Config::generate_schema();
        let json = serde_json::to_value(&schema).unwrap();
        let props = json["properties"].as_object().unwrap();
        assert!(props.contains_key("menus"), "schema missing menus property");
    }

    #[rstest]
    fn schema_keybinding_entry_has_menu_field() {
        let schema = Config::generate_schema();
        let json = serde_json::to_value(&schema).unwrap();
        let entry = &json["$defs"]["KeyBindingEntry"];
        let json_str = entry.to_string();
        assert!(json_str.contains("menu"), "KeyBindingEntry schema missing menu field");
    }

    #[rstest]
    fn schema_file_is_up_to_date() {
        let schema = Config::generate_schema();
        let generated = serde_json::to_string_pretty(&schema).unwrap() + "\n";

        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let schema_path = Path::new(manifest_dir).join("config.schema.json");
        let on_disk = std::fs::read_to_string(&schema_path).unwrap();

        assert_eq!(
            generated, on_disk,
            "config.schema.json is out of date. Run `mise run schema` to regenerate."
        );
    }
}
