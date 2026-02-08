//! Application state machine and main event loop.

use anyhow::Result;

use crate::cli::Args;
use crate::config::Config;

/// Run the application.
pub fn run(args: &Args) -> Result<()> {
    let _config = Config::load()?;
    tracing::info!(?args, "starting trev");
    // TODO: Initialize terminal, event loop, and UI
    Ok(())
}
