//! trev - A TUI file viewer
//!
//! ファイルシステムを Tree 形式で表示し、プレビューと Git 状態を提供する。

mod app;
mod cli;
mod config;
mod error;
mod git;
mod highlight;
mod input;
mod ipc;
mod preview;
mod tree;
mod ui;

use std::path::Path;
use std::process::Command as ProcessCommand;
use std::sync::mpsc as std_mpsc;

use clap::Parser;
use tokio::sync::mpsc as tokio_mpsc;
use tracing::{error, info};

use crate::app::App;
use crate::cli::{Args, Command as CliCommand, CtlAction};
use crate::error::Result;
use crate::ipc::{IpcCommand, IpcRequest};

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

    // サブコマンドがあれば処理
    if let Some(CliCommand::Ctl { action }) = &args.command {
        return run_ctl(action);
    }

    // TUI アプリケーションを実行
    run(&args)
}

/// ctl サブコマンドを実行する
fn run_ctl(action: &CtlAction) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        match action {
            CtlAction::Reveal { path, workspace } => {
                let abs_path = path.canonicalize()?;
                let ws_key = workspace
                    .clone()
                    .unwrap_or_else(|| ipc::workspace_key(&abs_path));
                let socket = ipc::socket_path(&ws_key)?;

                let request = IpcRequest::Reveal { path: abs_path };
                let response = ipc::send_command(&socket, &request).await?;
                print_response(&response);
            }
            CtlAction::Ping { workspace } => {
                let ws_key = workspace.clone().unwrap_or_else(|| {
                    ipc::workspace_key(&std::env::current_dir().unwrap_or_default())
                });
                let socket = ipc::socket_path(&ws_key)?;

                let request = IpcRequest::Ping {};
                let response = ipc::send_command(&socket, &request).await?;
                print_response(&response);
            }
            CtlAction::Quit { workspace } => {
                let ws_key = workspace.clone().unwrap_or_else(|| {
                    ipc::workspace_key(&std::env::current_dir().unwrap_or_default())
                });
                let socket = ipc::socket_path(&ws_key)?;

                let request = IpcRequest::Quit {};
                let response = ipc::send_command(&socket, &request).await?;
                print_response(&response);
            }
        }
        Ok(())
    })
}

/// IPC レスポンスを出力する
#[allow(clippy::print_stdout)]
fn print_response(response: &ipc::IpcResponse) {
    if response.ok {
        println!("ok");
    } else if let Some(err) = &response.error {
        println!("error: {}", err);
    }
}

/// TUI アプリケーションを実行する
fn run(args: &Args) -> Result<()> {
    // IPC チャンネル（daemon モードの場合のみ使用）
    let ipc_rx: Option<std_mpsc::Receiver<IpcCommand>> = if args.daemon {
        let root_path = args.path.canonicalize()?;
        let ws_key = ipc::workspace_key(&root_path);
        let socket_path = ipc::socket_path(&ws_key)?;

        // tokio mpsc → std mpsc への橋渡し
        let (std_tx, std_rx) = std_mpsc::channel::<IpcCommand>();

        // IPC サーバーを別スレッドで起動
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    error!(?e, "Failed to create tokio runtime");
                    return;
                }
            };

            rt.block_on(async {
                let (tokio_tx, mut tokio_rx) = tokio_mpsc::unbounded_channel::<IpcCommand>();

                // IPC サーバータスク
                let server_task = tokio::spawn(async move {
                    if let Err(e) = ipc::serve(&socket_path, tokio_tx).await {
                        error!(?e, "IPC server error");
                    }
                });

                // tokio mpsc → std mpsc フォワーダー
                let forward_task = tokio::spawn(async move {
                    while let Some(cmd) = tokio_rx.recv().await {
                        if std_tx.send(cmd).is_err() {
                            break;
                        }
                    }
                });

                let _ = tokio::join!(server_task, forward_task);
            });
        });

        info!("Daemon mode enabled");
        Some(std_rx)
    } else {
        None
    };

    // ターミナルを初期化
    let mut terminal = ratatui::init();

    // アプリケーションを作成
    let mut app = App::new(args)?;

    // 起動時に reveal が指定されていれば実行
    if let Some(reveal_path) = &args.reveal {
        if let Err(e) = app.reveal(reveal_path) {
            error!(?e, ?reveal_path, "Failed to reveal path on startup");
        }
    }

    // メインループ
    while app.is_running() {
        // IPC コマンドをチェック（daemon モードの場合）
        if let Some(rx) = &ipc_rx {
            // 非ブロッキングで受信をチェック
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    IpcCommand::Reveal(path) => {
                        if let Err(e) = app.reveal(&path) {
                            error!(?e, ?path, "Failed to reveal path");
                        }
                    }
                    IpcCommand::Quit => {
                        app.quit();
                    }
                }
            }
        }

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
        let _ = ProcessCommand::new(&editor).arg(path).status();
        return;
    }

    // $VISUAL 環境変数を次に試す
    if let Ok(visual) = std::env::var("VISUAL") {
        let _ = ProcessCommand::new(&visual).arg(path).status();
        return;
    }

    // macOS の場合は open コマンドを使用
    #[cfg(target_os = "macos")]
    {
        let _ = ProcessCommand::new("open").arg(path).status();
    }

    // Linux の場合は xdg-open を使用
    #[cfg(target_os = "linux")]
    {
        let _ = ProcessCommand::new("xdg-open").arg(path).status();
    }
}

/// 選択パスを stdout に出力する
#[allow(clippy::print_stdout)]
fn emit_path(path: &Path) {
    println!("{}", path.display());
}
