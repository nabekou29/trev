//! trev - Fast TUI file viewer with tree view and Neovim integration.

#![expect(
    unreachable_pub,
    reason = "Binary crate: all modules are private, so pub vs pub(crate) is irrelevant. \
              Conflicts with clippy::redundant_pub_crate from nursery group."
)]

mod action;
mod app;
mod cli;
mod config;
mod error;
mod git;

mod file_op;
mod input;
mod ipc;
mod preview;
mod session;
mod state;
mod terminal;
mod tree;
mod ui;
mod watcher;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Args::parse_args();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    app::run(&args).await
}
