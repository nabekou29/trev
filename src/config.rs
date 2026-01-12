//! 設定ファイル
//!
//! `$XDG_CONFIG_HOME/trev/config.toml` から設定を読み込む。

use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;
use serde::Deserialize;

use crate::tree::{
    SortDirection,
    SortOrder,
};

/// アプリケーション設定
#[derive(Debug, Clone, Default)]
pub(crate) struct Config {
    /// デフォルトのソート順
    pub(crate) sort_order: SortOrder,
    /// デフォルトのソート方向
    pub(crate) sort_direction: SortDirection,
    /// ディレクトリを先に表示するか
    pub(crate) directories_first: bool,
    /// 隠しファイルを表示するか
    pub(crate) show_hidden: bool,
    /// gitignore されたファイルを表示するか
    pub(crate) show_ignored: bool,
    /// プレビューを表示するか
    pub(crate) show_preview: bool,
}

/// TOML ファイルの設定構造
#[derive(Debug, Deserialize, Default)]
struct ConfigFile {
    /// ソート設定
    #[serde(default)]
    sort: SortConfigFile,
    /// 表示設定
    #[serde(default)]
    display: DisplayConfigFile,
}

/// ソート設定（TOML）
#[derive(Debug, Deserialize, Default)]
struct SortConfigFile {
    /// ソート順: "name", "size", "mtime", "type", "extension"
    order: Option<String>,
    /// ソート方向: "asc", "desc"
    direction: Option<String>,
    /// ディレクトリを先に表示するか
    directories_first: Option<bool>,
}

/// 表示設定（TOML）
#[derive(Debug, Deserialize, Default)]
struct DisplayConfigFile {
    /// 隠しファイルを表示するか
    show_hidden: Option<bool>,
    /// gitignore されたファイルを表示するか
    show_ignored: Option<bool>,
    /// プレビューを表示するか
    show_preview: Option<bool>,
}

impl Config {
    /// 設定ファイルから読み込む
    ///
    /// 設定ファイルが存在しない場合はデフォルト値を使用する。
    pub(crate) fn load() -> Self {
        let config_path = Self::config_path();

        if let Some(path) = config_path
            && path.exists()
            && let Ok(content) = fs::read_to_string(&path)
            && let Ok(config_file) = toml::from_str::<ConfigFile>(&content)
        {
            return Self::from_file(config_file);
        }

        Self::default_config()
    }

    /// 設定ファイルのパスを取得する
    fn config_path() -> Option<PathBuf> {
        ProjectDirs::from("", "", "trev").map(|dirs| dirs.config_dir().join("config.toml"))
    }

    /// デフォルト設定を返す
    fn default_config() -> Self {
        Self {
            sort_order: SortOrder::Name,
            sort_direction: SortDirection::Ascending,
            directories_first: true,
            show_hidden: false,
            show_ignored: false,
            show_preview: true,
        }
    }

    /// TOML ファイルから Config を生成する
    fn from_file(file: ConfigFile) -> Self {
        let default = Self::default_config();

        Self {
            sort_order: file
                .sort
                .order
                .as_ref()
                .and_then(|s| parse_sort_order(s))
                .unwrap_or(default.sort_order),
            sort_direction: file
                .sort
                .direction
                .as_ref()
                .and_then(|s| parse_sort_direction(s))
                .unwrap_or(default.sort_direction),
            directories_first: file.sort.directories_first.unwrap_or(default.directories_first),
            show_hidden: file.display.show_hidden.unwrap_or(default.show_hidden),
            show_ignored: file.display.show_ignored.unwrap_or(default.show_ignored),
            show_preview: file.display.show_preview.unwrap_or(default.show_preview),
        }
    }
}

/// ソート順文字列をパースする
fn parse_sort_order(s: &str) -> Option<SortOrder> {
    match s.to_lowercase().as_str() {
        "name" => Some(SortOrder::Name),
        "size" => Some(SortOrder::Size),
        "mtime" | "time" | "date" => Some(SortOrder::Mtime),
        "type" => Some(SortOrder::Type),
        "extension" | "ext" => Some(SortOrder::Extension),
        _ => None,
    }
}

/// ソート方向文字列をパースする
fn parse_sort_direction(s: &str) -> Option<SortDirection> {
    match s.to_lowercase().as_str() {
        "asc" | "ascending" => Some(SortDirection::Ascending),
        "desc" | "descending" => Some(SortDirection::Descending),
        _ => None,
    }
}
