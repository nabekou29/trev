//! コマンドライン引数の定義
//!
//! clap を使用して CLI 引数を定義する。

use std::path::PathBuf;

use clap::{
    Parser,
    ValueEnum,
};

/// TUI ファイルビューワ
#[derive(Debug, Parser)]
#[command(name = "trev", version, about = "A TUI file viewer")]
pub(crate) struct Args {
    /// 開始ディレクトリ（デフォルト: カレントディレクトリ）
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// 終了時に選択ファイルのパスを stdout に出力
    #[arg(long)]
    pub emit: bool,

    /// 出力フォーマット
    #[arg(long, value_enum, default_value = "lines")]
    pub emit_format: EmitFormat,

    /// 終了時のアクション種別
    #[arg(long, value_enum)]
    pub action: Option<Action>,

    /// 隠しファイル・ディレクトリを表示
    #[arg(short = 'a', long)]
    pub show_hidden: bool,
}

/// 出力フォーマット
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub(crate) enum EmitFormat {
    /// 改行区切り
    #[default]
    Lines,
    /// ヌル文字区切り
    Nul,
    /// JSON 形式
    Json,
}

/// 終了時のアクション種別
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Action {
    /// ファイルを開く
    Open,
    /// 分割して開く
    Split,
    /// 縦分割して開く
    Vsplit,
    /// 新しいタブで開く
    Tabedit,
}
