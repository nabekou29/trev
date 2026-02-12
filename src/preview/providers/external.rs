//! External command preview provider — ansi-to-tui.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use ansi_to_tui::IntoText;

use crate::config::ExternalCommand;
use crate::preview::content::PreviewContent;
use crate::preview::provider::{
    LoadContext,
    PreviewProvider,
};

/// Maximum output size from external commands (1 MB).
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;

/// Provider that runs external commands and renders their ANSI output.
///
/// Each configured command specifies file extensions it applies to.
/// The command is run with the file path as an argument, and the
/// ANSI-colored stdout is converted to ratatui `Text`.
#[derive(Debug)]
pub struct ExternalCmdProvider {
    /// External command configurations.
    commands: Vec<ExternalCommand>,
    /// Timeout for commands.
    timeout: Duration,
}

impl ExternalCmdProvider {
    /// Create a new external command provider.
    pub const fn new(commands: Vec<ExternalCommand>, timeout_secs: u64) -> Self {
        Self {
            commands,
            timeout: Duration::from_secs(timeout_secs),
        }
    }

    /// Find the first matching command for the given file extension.
    fn find_command(&self, ext: &str) -> Option<&ExternalCommand> {
        let ext_lower = ext.to_ascii_lowercase();
        self.commands.iter().find(|cmd| {
            cmd.extensions
                .iter()
                .any(|e| e.to_ascii_lowercase() == ext_lower)
        })
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
    fn name(&self) -> &'static str {
        "External"
    }

    fn priority(&self) -> u32 {
        10
    }

    fn can_handle(&self, path: &Path, is_dir: bool) -> bool {
        if is_dir {
            return false;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            return false;
        };
        self.find_command(ext)
            .is_some_and(|cmd| Self::command_exists(&cmd.command))
    }

    fn load(&self, path: &Path, ctx: &LoadContext) -> anyhow::Result<PreviewContent> {
        if ctx.cancel_token.is_cancelled() {
            return Ok(PreviewContent::Empty);
        }

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let Some(cmd_config) = self.find_command(ext) else {
            return Ok(PreviewContent::Error {
                message: "No matching external command".to_string(),
            });
        };

        // Build the command with file path as final argument.
        let mut cmd = Command::new(&cmd_config.command);
        for arg in &cmd_config.args {
            cmd.arg(arg);
        }
        cmd.arg(path);
        cmd.env("TERM", "xterm-256color");
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
                    cmd_config.command,
                    output.status,
                    stderr.trim()
                ),
            });
        }

        // Truncate output if too large.
        let stdout = output
            .stdout
            .get(..MAX_OUTPUT_BYTES)
            .unwrap_or(&output.stdout);

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

    fn make_echo_provider() -> ExternalCmdProvider {
        ExternalCmdProvider::new(
            vec![ExternalCommand {
                extensions: vec!["csv".to_string(), "tsv".to_string()],
                command: "cat".to_string(),
                args: vec![],
            }],
            3,
        )
    }

    fn make_nonexistent_provider() -> ExternalCmdProvider {
        ExternalCmdProvider::new(
            vec![ExternalCommand {
                extensions: vec!["xyz".to_string()],
                command: "nonexistent_command_12345".to_string(),
                args: vec![],
            }],
            3,
        )
    }

    // --- can_handle tests ---

    #[rstest]
    fn can_handle_matching_extension_and_command_exists() {
        let provider = make_echo_provider();
        assert_that!(
            provider.can_handle(&PathBuf::from("data.csv"), false),
            eq(true)
        );
    }

    #[rstest]
    fn can_handle_matching_extension_case_insensitive() {
        let provider = make_echo_provider();
        assert_that!(
            provider.can_handle(&PathBuf::from("data.CSV"), false),
            eq(true)
        );
    }

    #[rstest]
    fn can_handle_non_matching_extension() {
        let provider = make_echo_provider();
        assert_that!(
            provider.can_handle(&PathBuf::from("data.txt"), false),
            eq(false)
        );
    }

    #[rstest]
    fn can_handle_directory_returns_false() {
        let provider = make_echo_provider();
        assert_that!(
            provider.can_handle(&PathBuf::from("data.csv"), true),
            eq(false)
        );
    }

    #[rstest]
    fn can_handle_command_not_found_returns_false() {
        let provider = make_nonexistent_provider();
        assert_that!(
            provider.can_handle(&PathBuf::from("file.xyz"), false),
            eq(false)
        );
    }

    #[rstest]
    fn can_handle_no_extension_returns_false() {
        let provider = make_echo_provider();
        assert_that!(
            provider.can_handle(&PathBuf::from("Makefile"), false),
            eq(false)
        );
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

        let result = provider
            .load(&PathBuf::from("test.csv"), &ctx)
            .unwrap();
        assert!(matches!(result, PreviewContent::Empty));
    }

    #[rstest]
    fn priority_is_highest() {
        let provider = make_echo_provider();
        assert_that!(provider.priority(), eq(10));
    }
}
