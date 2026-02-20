//! trev - Fast TUI file viewer with tree view and Neovim integration.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{
    Result,
    bail,
};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() -> Result<()> {
    let args = trev::cli::Args::parse_args();

    let is_tui_mode = args.command.is_none();

    // In TUI mode, redirect logs to a file so they don't corrupt the display.
    // In subcommand mode, log to stderr as usual.
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));

    // Chrome trace guard must live until program exit to flush trace data.
    let mut chrome_guard: Option<tracing_chrome::FlushGuard> = None;
    let mut profile_path: Option<PathBuf> = None;

    if is_tui_mode {
        let log_dir = dirs::state_dir()
            .or_else(dirs::data_dir)
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("trev");

        let file_appender = tracing_appender::rolling::daily(&log_dir, "trev.log");
        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_writer(file_appender)
            .with_ansi(false)
            .with_filter(env_filter);

        let chrome_layer = if args.profile {
            let output = log_dir.join("profile.json");
            let (layer, guard) = tracing_chrome::ChromeLayerBuilder::new()
                .trace_style(tracing_chrome::TraceStyle::Async)
                .file(output.clone())
                .include_args(true)
                .build();
            chrome_guard = Some(guard);
            profile_path = Some(output);
            Some(layer.with_filter(tracing_subscriber::filter::LevelFilter::INFO))
        } else {
            None
        };

        tracing_subscriber::registry()
            .with(fmt_layer)
            .with(chrome_layer)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .init();
    }

    // Handle subcommands before entering TUI mode.
    match &args.command {
        Some(trev::cli::Command::Ctl {
            socket,
            pid,
            workspace,
            action,
        }) => {
            return handle_ctl(action, socket.as_deref(), *pid, workspace.as_deref()).await;
        }
        Some(trev::cli::Command::SocketPath { workspace }) => {
            return handle_socket_path(workspace.as_deref());
        }
        Some(trev::cli::Command::Schema) => {
            return handle_schema();
        }
        None => {}
    }

    trev::app::run(&args).await?;

    // Flush profile data and notify user of the output path.
    drop(chrome_guard);
    if let Some(path) = &profile_path {
        notify_profile_path(path);
    }

    Ok(())
}

/// Print profile output path to stderr after terminal restore.
#[expect(clippy::print_stderr, reason = "post-exit user notification")]
fn notify_profile_path(path: &std::path::Path) {
    eprintln!("[trev] profile written to {}", path.display());
}

/// Handle `trev ctl` subcommands by connecting to a running daemon.
async fn handle_ctl(
    action: &trev::cli::CtlAction,
    socket: Option<&std::path::Path>,
    pid: Option<u32>,
    workspace: Option<&str>,
) -> Result<()> {
    use trev::cli::CtlAction;

    let (method, params) = match action {
        CtlAction::Ping => ("ping", None),
        CtlAction::Quit => ("quit", None),
        CtlAction::Reveal { path } => {
            let abs_path = std::fs::canonicalize(path)?;
            let params = serde_json::json!({"path": abs_path.to_string_lossy()});
            ("reveal", Some(params))
        }
    };

    let socket_path = find_socket(socket, pid, workspace)?;
    let response =
        trev::ipc::client::send_request(&socket_path, method, params, Duration::from_secs(5))
            .await?;

    // Print result for the user.
    #[expect(clippy::print_stdout, reason = "CLI output to stdout is intentional")]
    if let Some(result) = response.get("result") {
        println!("{result}");
    } else if let Some(error) = response.get("error") {
        bail!("server error: {error}");
    }

    Ok(())
}

/// List socket paths of running daemons.
#[expect(clippy::print_stdout, reason = "CLI output to stdout is intentional")]
fn handle_socket_path(workspace: Option<&str>) -> Result<()> {
    let runtime_dir = trev::ipc::paths::runtime_dir();
    if !runtime_dir.is_dir() {
        return Ok(());
    }

    let mut entries: Vec<_> = std::fs::read_dir(&runtime_dir)?
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "sock"))
        .filter(|p| {
            workspace.is_none_or(|ws| {
                trev::ipc::paths::read_meta(p)
                    .map(|path| path.to_string_lossy().into_owned())
                    .is_some_and(|path| path.contains(ws))
            })
        })
        .collect();
    entries.sort();

    for sock in &entries {
        let meta = trev::ipc::paths::read_meta(sock);
        if let Some(workspace_path) = meta {
            println!("{}\t{}", sock.display(), workspace_path.display());
        } else {
            println!("{}", sock.display());
        }
    }

    Ok(())
}

/// Print JSON Schema for the configuration file to stdout.
#[expect(clippy::print_stdout, reason = "CLI output to stdout is intentional")]
fn handle_schema() -> Result<()> {
    let schema = trev::config::Config::generate_schema();
    let json = serde_json::to_string_pretty(&schema)?;
    println!("{json}");
    Ok(())
}

/// Find a daemon socket file using the provided targeting options.
///
/// Priority: `--socket` (direct path) > `--pid` + `--workspace` (filter).
/// Socket filenames follow the pattern `<workspace>-<pid>.sock`.
fn find_socket(
    socket: Option<&std::path::Path>,
    pid: Option<u32>,
    workspace: Option<&str>,
) -> Result<PathBuf> {
    // Direct socket path — use as-is.
    if let Some(path) = socket {
        if !path.exists() {
            bail!("socket not found: {}", path.display());
        }
        return Ok(path.to_path_buf());
    }

    let runtime_dir = trev::ipc::paths::runtime_dir();
    if !runtime_dir.is_dir() {
        bail!("no trev daemons running (runtime dir not found)");
    }

    let pid_str = pid.map(|p| p.to_string());

    let mut entries: Vec<PathBuf> = std::fs::read_dir(&runtime_dir)?
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else {
                return false;
            };
            if p.extension().is_none_or(|ext| ext != "sock") {
                return false;
            }
            // Filter by PID (from socket filename).
            if !pid_str
                .as_deref()
                .is_none_or(|pid| stem.ends_with(&format!("-{pid}")))
            {
                return false;
            }
            // Filter by workspace (from metadata file).
            if let Some(ws) = workspace {
                let workspace_path = trev::ipc::paths::read_meta(p)
                    .map(|path| path.to_string_lossy().into_owned());
                return workspace_path.is_some_and(|path| path.contains(ws));
            }
            true
        })
        .collect();

    match entries.len() {
        0 => bail!("no trev daemon found (try --socket, --pid, or --workspace to target)"),
        1 => Ok(entries.swap_remove(0)),
        n => bail!(
            "{n} trev daemons found; specify --socket, --pid, or --workspace to target one"
        ),
    }
}
