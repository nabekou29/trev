//! trev - Fast TUI file viewer with tree view and Neovim integration.

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let args = trev::cli::Args::parse_args();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    trev::app::run(&args).await
}
