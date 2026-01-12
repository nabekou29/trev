//! コマンドライン引数の定義
//!
//! clap を使用して CLI 引数を定義する。

use std::path::PathBuf;

use clap::{
    Parser,
    Subcommand,
    ValueEnum,
};

/// TUI ファイルビューワ
#[derive(Debug, Parser)]
#[command(name = "trev", version, about = "A TUI file viewer")]
pub(crate) struct Args {
    /// サブコマンド
    #[command(subcommand)]
    pub command: Option<Command>,

    /// 開始ディレクトリ（デフォルト: カレントディレクトリ）
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// daemon モードで起動（IPC サーバーを有効化）
    #[arg(long)]
    pub daemon: bool,

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

    /// 起動時に指定パスを選択状態にする
    #[arg(long)]
    pub reveal: Option<PathBuf>,
}

/// サブコマンド
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// daemon モードの trev を制御する
    Ctl {
        /// 制御コマンド
        #[command(subcommand)]
        action: CtlAction,
    },
}

/// ctl サブコマンドのアクション
#[derive(Debug, Subcommand)]
pub(crate) enum CtlAction {
    /// 指定パスをツリー上で表示・選択
    Reveal {
        /// 表示するパス
        path: PathBuf,

        /// workspace キー（省略時は path から自動検出）
        #[arg(long)]
        workspace: Option<String>,
    },
    /// 疎通確認
    Ping {
        /// workspace キー（省略時はカレントディレクトリから検出）
        #[arg(long)]
        workspace: Option<String>,
    },
    /// 終了要求
    Quit {
        /// workspace キー（省略時はカレントディレクトリから検出）
        #[arg(long)]
        workspace: Option<String>,
    },
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
    /// バッファで開く
    Edit,
    /// 分割して開く
    Split,
    /// 縦分割して開く
    Vsplit,
    /// 新しいタブで開く
    Tabedit,
}
