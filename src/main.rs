//! trev - Fast TUI file viewer with tree view and Neovim integration.

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

use anyhow::Result;

fn main() -> Result<()> {
    let args = cli::Args::parse_args();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(app::run(args))
}
