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
#[allow(clippy::struct_excessive_bools)]
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

    /// Reveal a specific path on startup.
    #[arg(long)]
    pub reveal: Option<PathBuf>,

    /// Subcommand.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Emit output format.
#[derive(Debug, Clone, Default, clap::ValueEnum)]
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
#[derive(Debug, Clone, Default, clap::ValueEnum)]
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
        #[command(subcommand)]
        action: CtlAction,
    },
}

/// Control actions for a running daemon.
#[derive(Debug, Subcommand)]
pub enum CtlAction {
    /// Reveal a file in the tree.
    Reveal {
        /// Path to reveal.
        path: PathBuf,
        /// Workspace key for targeting a specific daemon.
        #[arg(long)]
        workspace: Option<String>,
    },
    /// Ping the daemon.
    Ping {
        /// Workspace key for targeting a specific daemon.
        #[arg(long)]
        workspace: Option<String>,
    },
    /// Quit the daemon.
    Quit {
        /// Workspace key for targeting a specific daemon.
        #[arg(long)]
        workspace: Option<String>,
    },
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
            sort_order: None,
            sort_direction: None,
            no_directories_first: false,
            no_icons: false,
            restore: false,
            no_restore: false,
            daemon: false,
            emit: false,
            emit_format: EmitFormat::default(),
            action: OpenAction::default(),
            reveal: None,
            command: None,
        }
    }
}
