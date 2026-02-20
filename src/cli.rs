//! CLI argument definitions.

use std::path::PathBuf;

use clap::{
    Parser,
    Subcommand,
};

use crate::config::{
    SortDirection,
    SortOrder,
};

/// Fast TUI file viewer with tree view and Neovim integration.
#[derive(Debug, Parser)]
#[command(name = "trev", version, about)]
#[expect(clippy::struct_excessive_bools, reason = "CLI flags are naturally boolean")]
pub struct Args {
    /// Directory to open.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Show hidden files.
    #[arg(short = 'a', long)]
    pub show_hidden: bool,

    /// Show gitignored files.
    #[arg(long)]
    pub show_ignored: bool,

    /// Disable preview panel.
    #[arg(long)]
    pub no_preview: bool,

    /// Show root directory as a tree node.
    #[arg(long)]
    pub show_root: bool,

    /// Sort order (name, size, mtime, type, extension).
    #[arg(long)]
    pub sort_order: Option<SortOrder>,

    /// Sort direction (asc, desc).
    #[arg(long)]
    pub sort_direction: Option<SortDirection>,

    /// Do not sort directories before files.
    #[arg(long)]
    pub no_directories_first: bool,

    /// Disable file icons (Nerd Fonts).
    #[arg(long)]
    pub no_icons: bool,

    /// Run as daemon (enable IPC server).
    #[arg(long)]
    pub daemon: bool,

    /// Emit selected path to stdout on exit.
    #[arg(long)]
    pub emit: bool,

    /// Output format for --emit.
    #[arg(long, default_value = "lines")]
    pub emit_format: EmitFormat,

    /// Default action when opening a file (used with --daemon).
    #[arg(long, default_value = "edit")]
    pub action: OpenAction,

    /// Restore previous session state on startup.
    #[arg(long, conflicts_with = "no_restore")]
    pub restore: bool,

    /// Do not restore previous session state on startup.
    #[arg(long, conflicts_with = "restore")]
    pub no_restore: bool,

    /// Disable git integration.
    #[arg(long)]
    pub no_git: bool,

    /// Reveal a specific path on startup.
    #[arg(long)]
    pub reveal: Option<PathBuf>,

    /// Enable performance profiling (outputs Chrome Trace JSON).
    #[arg(long)]
    pub profile: bool,

    /// Subcommand.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Emit output format.
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum EmitFormat {
    /// One path per line.
    #[default]
    Lines,
    /// Null-separated paths.
    Nul,
    /// JSON array.
    Json,
}

/// Action for opening files in editor.
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum OpenAction {
    /// Open in current window.
    #[default]
    Edit,
    /// Open in horizontal split.
    Split,
    /// Open in vertical split.
    Vsplit,
    /// Open in new tab.
    Tabedit,
}

/// Subcommands for controlling a running daemon.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Control a running trev daemon.
    Ctl {
        /// Direct path to the daemon socket.
        #[arg(long)]
        socket: Option<PathBuf>,
        /// PID of the daemon process.
        #[arg(long)]
        pid: Option<u32>,
        /// Workspace key for targeting a specific daemon.
        #[arg(long)]
        workspace: Option<String>,
        /// Control action to perform.
        #[command(subcommand)]
        action: CtlAction,
    },
    /// List socket paths of running daemons.
    SocketPath {
        /// Filter by workspace path substring.
        #[arg(long)]
        workspace: Option<String>,
    },
    /// Print JSON Schema for the configuration file to stdout.
    Schema,
}

/// Control actions for a running daemon.
#[derive(Debug, Subcommand)]
pub enum CtlAction {
    /// Reveal a file in the tree.
    Reveal {
        /// Path to reveal.
        path: PathBuf,
    },
    /// Ping the daemon.
    Ping,
    /// Quit the daemon.
    Quit,
}

impl From<OpenAction> for crate::ipc::types::EditorAction {
    fn from(action: OpenAction) -> Self {
        match action {
            OpenAction::Edit => Self::Edit,
            OpenAction::Split => Self::Split,
            OpenAction::Vsplit => Self::Vsplit,
            OpenAction::Tabedit => Self::Tabedit,
        }
    }
}

impl Args {
    /// Parse CLI arguments.
    pub fn parse_args() -> Self {
        Self::parse()
    }
}

impl Default for Args {
    fn default() -> Self {
        Self {
            path: PathBuf::from("."),
            show_hidden: false,
            show_ignored: false,
            no_preview: false,
            show_root: false,
            sort_order: None,
            sort_direction: None,
            no_directories_first: false,
            no_icons: false,
            no_git: false,
            restore: false,
            no_restore: false,
            daemon: false,
            emit: false,
            emit_format: EmitFormat::default(),
            action: OpenAction::default(),
            reveal: None,
            profile: false,
            command: None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    // --- T036: --no-git CLI flag parsing ---

    #[rstest]
    fn no_git_flag_defaults_to_false() {
        let args = Args::default();
        assert_that!(args.no_git, eq(false));
    }

    #[rstest]
    fn no_git_flag_parsed_from_cli() {
        let args = Args::try_parse_from(["trev", "--no-git", "."]).unwrap();
        assert_that!(args.no_git, eq(true));
    }
}
