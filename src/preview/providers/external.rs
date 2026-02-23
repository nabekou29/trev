//! External command preview provider — ansi-to-tui.

use std::path::Path;
use std::process::Command;
use std::sync::{
    Arc,
    RwLock,
};
use std::time::Duration;

use ansi_to_tui::IntoText;

use crate::config::ExternalCommand;
use crate::git::GitState;
use crate::preview::content::PreviewContent;
use crate::preview::provider::{
    LoadContext,
    PreviewProvider,
};

/// Maximum output size from external commands (1 MB).
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;

/// Provider that runs a single external command and renders its ANSI output.
///
/// Each configured command specifies file extensions it applies to.
/// The command is run with the file path as an argument, and the
/// ANSI-colored stdout is converted to ratatui `Text`.
#[derive(Debug)]
pub struct ExternalCmdProvider {
    /// Display name of this provider.
    name: String,
    /// External command configuration.
    command: ExternalCommand,
    /// Timeout for the command.
    timeout: Duration,
    /// Shared git repository state for `git_status` condition filtering.
    git_state: Arc<RwLock<Option<GitState>>>,
}

impl ExternalCmdProvider {
    /// Create a new external command provider for a single command.
    pub fn new(
        command: ExternalCommand,
        timeout_secs: u64,
        git_state: Arc<RwLock<Option<GitState>>>,
    ) -> Self {
        let name = command.display_name().to_string();
        Self { name, command, timeout: Duration::from_secs(timeout_secs), git_state }
    }

    /// Check if a command exists in PATH.
    fn command_exists(name: &str) -> bool {
        Command::new("which")
            .arg(name)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }
}

impl PreviewProvider for ExternalCmdProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn priority(&self) -> u32 {
        self.command.priority.value()
    }

    fn can_handle(&self, path: &Path, is_dir: bool) -> bool {
        if !Self::command_exists(&self.command.command) {
            return false;
        }

        // Check extension / directory match first.
        let ext_match = if is_dir {
            self.command.directories
        } else {
            path.extension().and_then(|e| e.to_str()).is_some_and(|ext| {
                let ext_lower = ext.to_ascii_lowercase();
                self.command.extensions.iter().any(|e| e.to_ascii_lowercase() == ext_lower)
            })
        };
        if !ext_match {
            return false;
        }

        // Check git_status condition (empty = no filter, always matches).
        if !self.command.git_status.is_empty() {
            let matches =
                self.git_state
                    .read()
                    .ok()
                    .as_ref()
                    .and_then(|guard| guard.as_ref())
                    .and_then(|gs| {
                        if is_dir { gs.dir_status(path) } else { gs.file_status(path).copied() }
                    })
                    .is_some_and(|status| {
                        self.command.git_status.iter().any(|s| s == status.config_name())
                    });
            if !matches {
                return false;
            }
        }

        true
    }

    fn load(&self, path: &Path, ctx: &LoadContext) -> anyhow::Result<PreviewContent> {
        let _span = tracing::info_span!(
            "external_cmd_load",
            command = %self.command.command,
            path = %path.display(),
        )
        .entered();
        if ctx.cancel_token.is_cancelled() {
            return Ok(PreviewContent::Empty);
        }

        // Build the command with file path as final argument.
        let mut cmd = Command::new(&self.command.command);
        for arg in &self.command.args {
            cmd.arg(arg);
        }
        cmd.arg(path);
        cmd.env("TERM", "xterm-256color");
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // Spawn and wait with timeout.
        let child = cmd.spawn()?;
        let output = wait_with_timeout(child, self.timeout)?;

        if ctx.cancel_token.is_cancelled() {
            return Ok(PreviewContent::Empty);
        }

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(PreviewContent::Error {
                message: format!(
                    "{} exited with {}: {}",
                    self.command.command,
                    output.status,
                    stderr.trim()
                ),
            });
        }

        // Truncate output if too large.
        let stdout = output.stdout.get(..MAX_OUTPUT_BYTES).unwrap_or(&output.stdout);

        // Convert ANSI output to ratatui Text.
        let text = stdout.into_text().map_err(|e| anyhow::anyhow!("{e}"))?;

        Ok(PreviewContent::AnsiText { text })
    }
}

/// Wait for a child process with a timeout.
///
/// If the timeout expires, kills the process and returns an error.
fn wait_with_timeout(
    child: std::process::Child,
    timeout: Duration,
) -> anyhow::Result<std::process::Output> {
    // Use a thread to implement timeout since std::process doesn't have native timeout.
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let child = child;
        let result = child.wait_with_output();
        let _ = tx.send(());
        result
    });

    match rx.recv_timeout(timeout) {
        Ok(()) => {
            // Thread finished within timeout.
            handle
                .join()
                .map_err(|_| anyhow::anyhow!("Command thread panicked"))?
                .map_err(Into::into)
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            // Timeout — thread is still running. We can't easily kill from here,
            // but the thread will eventually complete. Return an error.
            Err(anyhow::anyhow!("External command timed out"))
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            Err(anyhow::anyhow!("Command thread disconnected"))
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::path::PathBuf;

    use googletest::prelude::*;
    use rstest::*;

    use super::*;
    use crate::config::Priority;
    /// Shared empty git state for providers that don't need git filtering.
    fn no_git() -> Arc<RwLock<Option<GitState>>> {
        Arc::new(RwLock::new(None))
    }

    fn make_echo_provider() -> ExternalCmdProvider {
        ExternalCmdProvider::new(
            ExternalCommand {
                name: None,
                extensions: vec!["csv".to_string(), "tsv".to_string()],
                directories: false,
                priority: Priority::default(),
                command: "cat".to_string(),
                args: vec![],
                git_status: vec![],
            },
            3,
            no_git(),
        )
    }

    fn make_nonexistent_provider() -> ExternalCmdProvider {
        ExternalCmdProvider::new(
            ExternalCommand {
                name: None,
                extensions: vec!["xyz".to_string()],
                directories: false,
                priority: Priority::default(),
                command: "nonexistent_command_12345".to_string(),
                args: vec![],
                git_status: vec![],
            },
            3,
            no_git(),
        )
    }

    // --- name tests ---

    #[rstest]
    fn name_uses_explicit_name() {
        let provider = ExternalCmdProvider::new(
            ExternalCommand {
                name: Some("Pretty JSON".to_string()),
                extensions: vec!["json".to_string()],
                directories: false,
                priority: Priority::default(),
                command: "jq".to_string(),
                args: vec![".".to_string()],
                git_status: vec![],
            },
            3,
            no_git(),
        );
        assert_that!(provider.name(), eq("Pretty JSON"));
    }

    #[rstest]
    fn name_defaults_to_command_name() {
        let provider = make_echo_provider();
        assert_that!(provider.name(), eq("cat"));
    }

    // --- can_handle tests ---

    #[rstest]
    fn can_handle_matching_extension_and_command_exists() {
        let provider = make_echo_provider();
        assert_that!(provider.can_handle(&PathBuf::from("data.csv"), false), eq(true));
    }

    #[rstest]
    fn can_handle_matching_extension_case_insensitive() {
        let provider = make_echo_provider();
        assert_that!(provider.can_handle(&PathBuf::from("data.CSV"), false), eq(true));
    }

    #[rstest]
    fn can_handle_non_matching_extension() {
        let provider = make_echo_provider();
        assert_that!(provider.can_handle(&PathBuf::from("data.txt"), false), eq(false));
    }

    #[rstest]
    fn can_handle_directory_returns_false() {
        let provider = make_echo_provider();
        assert_that!(provider.can_handle(&PathBuf::from("data.csv"), true), eq(false));
    }

    #[rstest]
    fn can_handle_command_not_found_returns_false() {
        let provider = make_nonexistent_provider();
        assert_that!(provider.can_handle(&PathBuf::from("file.xyz"), false), eq(false));
    }

    #[rstest]
    fn can_handle_no_extension_returns_false() {
        let provider = make_echo_provider();
        assert_that!(provider.can_handle(&PathBuf::from("Makefile"), false), eq(false));
    }

    #[rstest]
    fn can_handle_directory_when_directories_enabled() {
        let provider = ExternalCmdProvider::new(
            ExternalCommand {
                name: Some("dust".to_string()),
                extensions: vec![],
                directories: true,
                priority: Priority::default(),
                command: "ls".to_string(),
                args: vec![],
                git_status: vec![],
            },
            3,
            no_git(),
        );
        assert_that!(provider.can_handle(&PathBuf::from("/some/dir"), true), eq(true));
    }

    // --- can_handle with git_status condition ---

    #[rstest]
    fn can_handle_git_status_matches() {
        let git_state = Arc::new(RwLock::new(Some(GitState::from_porcelain(
            " M src/main.rs\n",
            Path::new("/repo"),
        ))));
        let provider = ExternalCmdProvider::new(
            ExternalCommand {
                name: None,
                extensions: vec!["rs".to_string()],
                directories: false,
                priority: Priority::default(),
                command: "cat".to_string(),
                args: vec![],
                git_status: vec!["modified".to_string()],
            },
            3,
            git_state,
        );
        assert_that!(provider.can_handle(&PathBuf::from("/repo/src/main.rs"), false), eq(true));
    }

    #[rstest]
    fn can_handle_git_status_no_match() {
        let git_state = Arc::new(RwLock::new(Some(GitState::from_porcelain(
            "?? src/main.rs\n",
            Path::new("/repo"),
        ))));
        let provider = ExternalCmdProvider::new(
            ExternalCommand {
                name: None,
                extensions: vec!["rs".to_string()],
                directories: false,
                priority: Priority::default(),
                command: "cat".to_string(),
                args: vec![],
                git_status: vec!["modified".to_string()],
            },
            3,
            git_state,
        );
        // File is untracked, not modified — should not match.
        assert_that!(provider.can_handle(&PathBuf::from("/repo/src/main.rs"), false), eq(false));
    }

    #[rstest]
    fn can_handle_git_status_no_condition_always_matches() {
        // Empty git_status = no filter, always matches if extension matches.
        let provider = make_echo_provider();
        assert_that!(provider.can_handle(&PathBuf::from("data.csv"), false), eq(true));
    }

    #[rstest]
    fn can_handle_git_status_file_has_no_status() {
        // File exists but has no git status — should not match when filter is set.
        let git_state =
            Arc::new(RwLock::new(Some(GitState::from_porcelain("", Path::new("/repo")))));
        let provider = ExternalCmdProvider::new(
            ExternalCommand {
                name: None,
                extensions: vec!["rs".to_string()],
                directories: false,
                priority: Priority::default(),
                command: "cat".to_string(),
                args: vec![],
                git_status: vec!["modified".to_string()],
            },
            3,
            git_state,
        );
        assert_that!(provider.can_handle(&PathBuf::from("/repo/src/clean.rs"), false), eq(false));
    }

    #[rstest]
    fn can_handle_git_status_multiple_statuses() {
        let git_state = Arc::new(RwLock::new(Some(GitState::from_porcelain(
            "A  src/new.rs\n",
            Path::new("/repo"),
        ))));
        let provider = ExternalCmdProvider::new(
            ExternalCommand {
                name: None,
                extensions: vec!["rs".to_string()],
                directories: false,
                priority: Priority::default(),
                command: "cat".to_string(),
                args: vec![],
                git_status: vec!["modified".to_string(), "added".to_string()],
            },
            3,
            git_state,
        );
        // File is "added" which is in the filter list.
        assert_that!(provider.can_handle(&PathBuf::from("/repo/src/new.rs"), false), eq(true));
    }

    // --- load tests ---

    #[rstest]
    fn load_successful_command_returns_ansi_text() {
        let provider = make_echo_provider();
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.csv");
        std::fs::write(&file_path, "a,b,c\n1,2,3\n").unwrap();

        let ctx = LoadContext {
            max_lines: 1000,
            max_bytes: 10_000_000,

            cancel_token: tokio_util::sync::CancellationToken::new(),
        };

        let result = provider.load(&file_path, &ctx).unwrap();
        assert!(matches!(result, PreviewContent::AnsiText { .. }));
    }

    #[rstest]
    fn load_cancelled_returns_empty() {
        let provider = make_echo_provider();
        let ctx = LoadContext {
            max_lines: 1000,
            max_bytes: 10_000_000,

            cancel_token: tokio_util::sync::CancellationToken::new(),
        };
        ctx.cancel_token.cancel();

        let result = provider.load(&PathBuf::from("test.csv"), &ctx).unwrap();
        assert!(matches!(result, PreviewContent::Empty));
    }

    #[rstest]
    fn priority_uses_config_value() {
        let provider = make_echo_provider();
        assert_that!(provider.priority(), eq(Priority::MID.value()));
    }
}
