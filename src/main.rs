//! trev - A TUI file viewer
//!
//! ファイルシステムを Tree 形式で表示し、プレビューと Git 状態を提供する。

mod app;
mod cli;
mod error;
mod git;
mod highlight;
mod preview;
mod tree;
mod ui;

use clap::Parser;
use tracing::info;

use crate::app::App;
use crate::cli::Args;
use crate::error::Result;

/// アプリケーションのエントリポイント
///
/// # Errors
///
/// 以下の場合にエラーを返す:
/// - 指定されたパスが無効な場合
/// - ターミナルの初期化に失敗した場合
fn main() -> Result<()> {
    // tracing の初期化
    tracing_subscriber::fmt::init();

    // CLI 引数のパース
    let args = Args::parse();

    info!(?args, "trev starting");

    // アプリケーションを実行
    run(&args)
}

/// TUI アプリケーションを実行する
fn run(args: &Args) -> Result<()> {
    // ターミナルを初期化
    let mut terminal = ratatui::init();

    // アプリケーションを作成
    let mut app = App::new(args)?;

    // メインループ
    while app.is_running() {
        // 描画
        terminal.draw(|frame| app.draw(frame))?;

        // イベント処理
        app.handle_events()?;
    }

    // ターミナルを復元
    ratatui::restore();

    // 終了時に選択パスを出力
    if args.emit
        && let Some(path) = &app.selected_path
    {
        emit_path(path);
    }

    Ok(())
}

/// 選択パスを stdout に出力する
#[allow(clippy::print_stdout)]
fn emit_path(path: &std::path::Path) {
    println!("{}", path.display());
}
