//! trev - Fast TUI file viewer with tree view and Neovim integration.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{
    Result,
    bail,
};

#[tokio::main]
async fn main() -> Result<()> {
    let args = trev::cli::Args::parse_args();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    // Handle ctl subcommands before entering TUI mode.
    if let Some(trev::cli::Command::Ctl { action }) = &args.command {
        return handle_ctl(action).await;
    }

    trev::app::run(&args).await
}

/// Handle `trev ctl` subcommands by connecting to a running daemon.
async fn handle_ctl(action: &trev::cli::CtlAction) -> Result<()> {
    use trev::cli::CtlAction;

    let (method, params, workspace) = match action {
        CtlAction::Ping { workspace } => ("ping", None, workspace.as_deref()),
        CtlAction::Quit { workspace } => ("quit", None, workspace.as_deref()),
        CtlAction::Reveal { path, workspace } => {
            let abs_path = std::fs::canonicalize(path)?;
            let params = serde_json::json!({"path": abs_path.to_string_lossy()});
            ("reveal", Some(params), workspace.as_deref())
        }
    };

    let socket = find_socket(workspace)?;
    let response =
        trev::ipc::client::send_request(&socket, method, params, Duration::from_secs(5)).await?;

    // Print result for the user.
    #[allow(clippy::print_stdout)]
    if let Some(result) = response.get("result") {
        println!("{result}");
    } else if let Some(error) = response.get("error") {
        bail!("server error: {error}");
    }

    Ok(())
}

/// Find a daemon socket file, optionally filtering by workspace key.
///
/// Looks for `<workspace>-<pid>.sock` files in the runtime directory.
fn find_socket(workspace: Option<&str>) -> Result<PathBuf> {
    let runtime_dir = trev::ipc::paths::runtime_dir();
    if !runtime_dir.is_dir() {
        bail!("no trev daemons running (runtime dir not found)");
    }

    let mut entries: Vec<PathBuf> = std::fs::read_dir(&runtime_dir)?
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|ext| ext == "sock")
                && workspace.is_none_or(|ws| {
                    p.file_stem()
                        .and_then(|s| s.to_str())
                        .is_some_and(|name| name.starts_with(&format!("{ws}-")))
                })
        })
        .collect();

    match entries.len() {
        0 => bail!(
            "no trev daemon found{}",
            workspace.map_or_else(String::new, |w| format!(" for workspace '{w}'"))
        ),
        1 => Ok(entries.swap_remove(0)),
        n => {
            // Multiple daemons — use the first one.
            tracing::warn!(count = n, "multiple trev daemons found, using first match");
            Ok(entries.swap_remove(0))
        }
    }
}
