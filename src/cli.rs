//! CLI argument definitions.

use std::path::PathBuf;

use clap::{
    CommandFactory,
    Parser,
    Subcommand,
};
use clap_complete::Shell;

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
    pub show_root_entry: bool,

    /// Sort order (name, size, mtime, type, extension).
    #[arg(long)]
    pub sort_order: Option<SortOrder>,

    /// Sort direction (asc, desc).
    #[arg(long)]
    pub sort_direction: Option<SortDirection>,

    /// Do not sort directories before files.
    #[arg(long)]
    pub no_directories_first: bool,

    /// Enable file icons (Nerd Fonts).
    #[arg(long, conflicts_with = "no_icons")]
    pub icons: bool,

    /// Disable file icons (Nerd Fonts).
    #[arg(long, conflicts_with = "icons")]
    pub no_icons: bool,

    /// Enable IPC server.
    #[arg(long)]
    pub ipc: bool,

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

    /// Use a specific config file instead of the default.
    ///
    /// Replaces the base config entirely. CLI overrides are still applied on top.
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Additional config file to merge on top of the base config.
    ///
    /// Used by editor plugins to inject keybindings and custom actions at startup.
    #[arg(long)]
    pub config_override: Option<PathBuf>,

    /// Enable performance profiling (outputs Chrome Trace JSON).
    #[arg(long)]
    pub profile: bool,

    /// Subcommand.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Subcommands for controlling a running trev instance.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Control a running trev instance.
    Ctl {
        /// Direct path to the IPC socket.
        #[arg(long)]
        socket: Option<PathBuf>,
        /// PID of the trev process.
        #[arg(long)]
        pid: Option<u32>,
        /// Workspace key for targeting a specific instance.
        #[arg(long)]
        workspace: Option<String>,
        /// Control action to perform.
        #[command(subcommand)]
        action: CtlAction,
    },
    /// List socket paths of running trev instances.
    SocketPath {
        /// Filter by workspace path substring.
        #[arg(long)]
        workspace: Option<String>,
    },
    /// Print JSON Schema for the configuration file to stdout.
    #[cfg(feature = "dev")]
    Schema,
    /// Generate shell completion scripts.
    Completions {
        /// Target shell.
        shell: Shell,
    },
    /// Generate documentation (default keybindings / action reference).
    #[cfg(feature = "dev")]
    Docs,
}

/// Control actions for a running trev instance.
#[derive(Debug, Subcommand)]
pub enum CtlAction {
    /// Reveal a file in the tree.
    Reveal {
        /// Path to reveal.
        path: PathBuf,
    },
    /// Ping the trev instance.
    Ping,
    /// Quit the trev instance.
    Quit,
}

impl Args {
    /// Parse CLI arguments.
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Write shell completion script to stdout.
    pub fn print_completions(shell: Shell) {
        clap_complete::generate(shell, &mut Self::command(), "trev", &mut std::io::stdout());
    }
}

impl Default for Args {
    fn default() -> Self {
        Self {
            path: PathBuf::from("."),
            show_hidden: false,
            show_ignored: false,
            no_preview: false,
            show_root_entry: false,
            sort_order: None,
            sort_direction: None,
            no_directories_first: false,
            icons: false,
            no_icons: false,
            no_git: false,
            restore: false,
            no_restore: false,
            ipc: false,
            reveal: None,
            config: None,
            config_override: None,
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

    // --- --no-git CLI flag parsing ---

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

    // --- --config-override CLI flag parsing ---

    #[rstest]
    fn config_override_defaults_to_none() {
        let args = Args::default();
        assert_that!(args.config_override, none());
    }

    #[rstest]
    fn config_override_parsed_from_cli() {
        let args = Args::try_parse_from(["trev", "--config-override", "/tmp/x.yml", "."]).unwrap();
        assert_eq!(args.config_override, Some(PathBuf::from("/tmp/x.yml")));
    }
}
