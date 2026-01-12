//! trev - A TUI file viewer
//!
//! ファイルシステムを Tree 形式で表示し、プレビューと Git 状態を提供する。

mod app;
mod cli;
mod config;
mod error;
mod git;
mod highlight;
mod preview;
mod tree;
mod ui;

use std::path::Path;
use std::process::Command;

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
        // ファイルを開く要求があれば処理
        if let Some(path) = app.take_open_file_request() {
            // ターミナルを一時的に復元
            ratatui::restore();

            // ファイルを開く
            open_file(&path);

            // ターミナルを再初期化
            terminal = ratatui::init();
            continue;
        }

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

/// ファイルを外部エディタで開く
fn open_file(path: &Path) {
    // $EDITOR 環境変数を優先
    if let Ok(editor) = std::env::var("EDITOR") {
        let _ = Command::new(&editor).arg(path).status();
        return;
    }

    // $VISUAL 環境変数を次に試す
    if let Ok(visual) = std::env::var("VISUAL") {
        let _ = Command::new(&visual).arg(path).status();
        return;
    }

    // macOS の場合は open コマンドを使用
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(path).status();
    }

    // Linux の場合は xdg-open を使用
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("xdg-open").arg(path).status();
    }
}

/// 選択パスを stdout に出力する
#[allow(clippy::print_stdout)]
fn emit_path(path: &Path) {
    println!("{}", path.display());
}
