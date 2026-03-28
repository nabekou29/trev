//! Git diff preview provider.
//!
//! Shows `git diff` output for files with changes. Supports an optional external
//! pager (e.g. `delta`) for rich rendering, with a built-in semantic diff
//! styler as fallback.

use std::path::{
    Path,
    PathBuf,
};
use std::process::Command;
use std::sync::{
    Arc,
    RwLock,
};
use std::time::Duration;

use ansi_to_tui::IntoText;
use ratatui::style::{
    Color,
    Modifier,
    Style,
};
use ratatui::text::Line;

use crate::git::{
    GitFileStatus,
    GitState,
};
use crate::preview::content::PreviewContent;
use crate::preview::provider::{
    LoadContext,
    NodeInfo,
    PreviewProvider,
};

/// Maximum output size from git diff (1 MB).
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;

/// Built-in provider that shows `git diff` output for files with changes.
///
/// Active only when git is enabled and the file has a diff-producing status
/// (`Modified`, `Staged`, `StagedModified`, `Added`, `Deleted`, `Renamed`, `Conflicted`).
/// Untracked files are skipped since they have no diff.
///
/// When a pager command is configured and available, git diff output is piped
/// through it for rich rendering. Otherwise, the built-in semantic diff styler
/// is used.
#[derive(Debug)]
pub struct DiffPreviewProvider {
    /// Shared git repository state.
    git_state: Arc<RwLock<Option<GitState>>>,
    /// Repository root path (used as `current_dir` for git commands).
    root_path: PathBuf,
    /// Timeout for the git diff command.
    timeout: Duration,
    /// Resolved pager command name, if configured and available.
    pager_command: Option<String>,
    /// Resolved pager arguments.
    pager_args: Vec<String>,
}

impl DiffPreviewProvider {
    /// Create a new diff preview provider.
    ///
    /// If `pager` is provided and the command exists in `$PATH`, it will be
    /// used to render diff output. When `pager` is `None`, the provider
    /// attempts to detect a suitable pager from git config (`pager.diff` or
    /// `core.pager`). If no usable pager is found, the built-in styler is used.
    pub fn new(
        git_state: Arc<RwLock<Option<GitState>>>,
        root_path: PathBuf,
        timeout_secs: u64,
        pager: Option<&str>,
    ) -> Self {
        let detected = pager.is_none().then(|| detect_pager_from_git(&root_path)).flatten();
        let pager_str = pager.or(detected.as_deref());

        let (pager_command, pager_args): (Option<String>, Vec<String>) = pager_str
            .and_then(parse_pager_command)
            .filter(|(cmd, _)| command_exists(cmd))
            .map(|(cmd, args)| {
                (Some(cmd.to_string()), args.into_iter().map(String::from).collect())
            })
            .unwrap_or_default();

        Self {
            git_state,
            root_path,
            timeout: Duration::from_secs(timeout_secs),
            pager_command,
            pager_args,
        }
    }

    /// Get the git file status for a path, if available.
    fn file_status(&self, path: &Path) -> Option<GitFileStatus> {
        self.git_state
            .read()
            .ok()
            .as_ref()
            .and_then(|guard| guard.as_ref())
            .and_then(|gs| gs.file_status(path).copied())
    }
}

impl PreviewProvider for DiffPreviewProvider {
    fn name(&self) -> &'static str {
        "Diff"
    }

    fn priority(&self) -> u32 {
        crate::config::Priority::MID.value()
    }

    fn can_handle(&self, path: &Path, node: &NodeInfo) -> bool {
        if node.is_dir() {
            return false;
        }

        self.file_status(path).is_some_and(|status| {
            matches!(
                status,
                GitFileStatus::Modified
                    | GitFileStatus::Staged
                    | GitFileStatus::StagedModified
                    | GitFileStatus::Added
                    | GitFileStatus::Deleted
                    | GitFileStatus::Renamed
                    | GitFileStatus::Conflicted
            )
        })
    }

    fn load(&self, path: &Path, ctx: &LoadContext) -> anyhow::Result<PreviewContent> {
        let _span = tracing::info_span!("diff_load", path = %path.display()).entered();

        if ctx.cancel_token.is_cancelled() {
            return Ok(PreviewContent::Cancelled);
        }

        let status = self.file_status(path);
        let use_color = self.pager_command.is_some();

        // Use --cached for Added files (staged only), HEAD for others.
        let mut cmd = Command::new("git");
        if matches!(status, Some(GitFileStatus::Added)) {
            cmd.args(["diff", "--cached"]);
        } else {
            cmd.args(["diff", "HEAD"]);
        }
        if use_color {
            cmd.arg("--color=always");
        }
        cmd.arg("--").arg(path);
        cmd.current_dir(&self.root_path);
        cmd.env("TERM", "xterm-256color");
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let child = cmd.spawn()?;
        let output = wait_with_timeout(child, self.timeout)?;

        if ctx.cancel_token.is_cancelled() {
            return Ok(PreviewContent::Cancelled);
        }

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let trimmed = stderr.trim();
            if trimmed.is_empty() {
                return Ok(PreviewContent::Empty);
            }
            return Ok(PreviewContent::Error {
                message: format!("git diff exited with {}: {trimmed}", output.status),
            });
        }

        if output.stdout.is_empty() {
            return Ok(PreviewContent::Empty);
        }

        // Truncate output if too large.
        let stdout = output.stdout.get(..MAX_OUTPUT_BYTES).unwrap_or(&output.stdout);

        if let Some(ref pager_cmd) = self.pager_command {
            // Path A: pipe through external pager.
            let pager_args: Vec<&str> = self.pager_args.iter().map(String::as_str).collect();
            let pager_output = pipe_through_pager(stdout, pager_cmd, &pager_args, self.timeout)?;
            let pager_stdout = pager_output.get(..MAX_OUTPUT_BYTES).unwrap_or(&pager_output);
            let text = pager_stdout.into_text().map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(PreviewContent::AnsiText { text })
        } else {
            // Path B: built-in semantic styling.
            let raw = String::from_utf8_lossy(stdout);
            let lines = style_diff_lines(&raw);
            let truncated = stdout.len() >= MAX_OUTPUT_BYTES;
            Ok(PreviewContent::HighlightedText { lines, language: "Diff".to_string(), truncated })
        }
    }
}

/// Parse unified diff output and apply semantic styling.
///
/// Line types are identified by their prefix:
/// - `diff --git`, `--- `, `+++ `, `index ` → file headers (bold)
/// - `@@ ... @@` → hunk headers (cyan foreground)
/// - `+` → additions (green foreground, subtle green background)
/// - `-` → deletions (red foreground, subtle red background)
/// - everything else → context (default style)
fn style_diff_lines(raw: &str) -> Vec<Line<'static>> {
    raw.lines()
        .map(|line| {
            let style = if line.starts_with("diff --git ")
                || line.starts_with("--- ")
                || line.starts_with("+++ ")
                || line.starts_with("index ")
            {
                Style::default().add_modifier(Modifier::BOLD)
            } else if line.starts_with("@@") {
                Style::default().fg(Color::Cyan)
            } else if line.starts_with('+') {
                Style::default().fg(Color::Green).bg(Color::Rgb(0, 40, 0))
            } else if line.starts_with('-') {
                Style::default().fg(Color::Red).bg(Color::Rgb(40, 0, 0))
            } else {
                Style::default()
            };
            Line::styled(line.to_string(), style)
        })
        .collect()
}

/// Parse a pager command string into (command, args).
///
/// Splits on whitespace. The first token is the command name,
/// remaining tokens are arguments.
fn parse_pager_command(pager: &str) -> Option<(&str, Vec<&str>)> {
    let mut parts = pager.split_whitespace();
    let cmd = parts.next()?;
    let args: Vec<&str> = parts.collect();
    Some((cmd, args))
}

/// Detect a suitable diff pager from git configuration.
///
/// Checks `pager.diff` first, then falls back to `core.pager`. Only returns
/// a value for known diff-highlighting tools (`delta`, `diff-so-fancy`).
/// Interactive pagers like `less` or `more` are skipped.
///
/// For `delta`, `--paging=never` is appended automatically if not already present.
fn detect_pager_from_git(root_path: &Path) -> Option<String> {
    let raw = git_config_value(root_path, "pager.diff")
        .or_else(|| git_config_value(root_path, "core.pager"))?;

    let (cmd, _) = parse_pager_command(&raw)?;

    // Extract the base command name (handles paths like /usr/bin/delta).
    let base = cmd.rsplit('/').next().unwrap_or(cmd);

    match base {
        "delta" => {
            let mut result = raw.clone();
            if !result.contains("--paging") {
                result.push_str(" --paging=never");
            }
            Some(result)
        }
        "diff-so-fancy" => Some(raw),
        // Interactive pagers (less, more, most, cat, bat) are not suitable.
        _ => None,
    }
}

/// Read a single git config value.
fn git_config_value(root_path: &Path, key: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["config", "--get", key])
        .current_dir(root_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

/// Check if a command exists in `$PATH`.
fn command_exists(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Pipe raw bytes through an external pager command.
///
/// Writes `input` to the pager's stdin in a background thread (to avoid
/// deadlocks) and captures stdout.
fn pipe_through_pager(
    input: &[u8],
    pager_cmd: &str,
    pager_args: &[&str],
    timeout: Duration,
) -> anyhow::Result<Vec<u8>> {
    let mut child = Command::new(pager_cmd)
        .args(pager_args)
        .env("TERM", "xterm-256color")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    // Write input to pager's stdin in a separate thread to avoid deadlocks.
    let stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("failed to open pager stdin"))?;
    let input_owned = input.to_vec();
    let write_handle = std::thread::spawn(move || {
        use std::io::Write;
        let mut stdin = stdin;
        let _ = stdin.write_all(&input_owned);
        // stdin is dropped here, signaling EOF.
    });

    let output = wait_with_timeout(child, timeout)?;
    let _ = write_handle.join();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("pager exited with {}: {}", output.status, stderr.trim()));
    }

    Ok(output.stdout)
}

/// Wait for a child process with a timeout, draining stdout/stderr concurrently.
///
/// This approach prevents deadlocks that can occur when a child process
/// fills its stdout or stderr pipe buffer while we are not reading from it.
fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
) -> anyhow::Result<std::process::Output> {
    // Take stdout/stderr handles for concurrent draining.
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let stdout_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = stdout_pipe {
            let _ = std::io::Read::read_to_end(&mut pipe, &mut buf);
        }
        buf
    });

    let stderr_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut pipe) = stderr_pipe {
            let _ = std::io::Read::read_to_end(&mut pipe, &mut buf);
        }
        buf
    });

    // Poll try_wait with timeout.
    let start = std::time::Instant::now();
    let poll_interval = Duration::from_millis(50);

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_handle.join();
                let _ = stderr_handle.join();
                return Err(anyhow::anyhow!("git diff timed out"));
            }
            Ok(None) => std::thread::sleep(poll_interval),
            Err(e) => return Err(e.into()),
        }
    };

    let stdout = stdout_handle.join().map_err(|_| anyhow::anyhow!("stdout reader panicked"))?;
    let stderr = stderr_handle.join().map_err(|_| anyhow::anyhow!("stderr reader panicked"))?;

    Ok(std::process::Output { status, stdout, stderr })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::path::PathBuf;

    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    /// Shared empty git state for tests with no git status.
    fn no_git() -> Arc<RwLock<Option<GitState>>> {
        Arc::new(RwLock::new(None))
    }

    /// Git state with a modified file.
    fn git_with_modified() -> Arc<RwLock<Option<GitState>>> {
        Arc::new(RwLock::new(Some(GitState::from_porcelain(
            " M src/main.rs\n",
            Path::new("/repo"),
        ))))
    }

    /// Git state with a staged (added) file.
    fn git_with_added() -> Arc<RwLock<Option<GitState>>> {
        Arc::new(RwLock::new(Some(GitState::from_porcelain("A  src/new.rs\n", Path::new("/repo")))))
    }

    /// Git state with an untracked file.
    fn git_with_untracked() -> Arc<RwLock<Option<GitState>>> {
        Arc::new(RwLock::new(Some(GitState::from_porcelain(
            "?? src/scratch.rs\n",
            Path::new("/repo"),
        ))))
    }

    fn make_provider(git_state: Arc<RwLock<Option<GitState>>>) -> DiffPreviewProvider {
        DiffPreviewProvider::new(git_state, PathBuf::from("/repo"), 3, None)
    }

    /// Helper to create a `NodeInfo` for a file.
    const fn file_node() -> NodeInfo {
        NodeInfo { file_type: crate::preview::provider::FileType::File }
    }

    /// Helper to create a `NodeInfo` for a directory.
    const fn dir_node() -> NodeInfo {
        NodeInfo { file_type: crate::preview::provider::FileType::Directory }
    }

    // --- name / priority ---

    #[rstest]
    fn name_is_diff() {
        let provider = make_provider(no_git());
        assert_that!(provider.name(), eq("Diff"));
    }

    #[rstest]
    fn priority_is_mid() {
        let provider = make_provider(no_git());
        assert_that!(provider.priority(), eq(crate::config::Priority::MID.value()));
    }

    // --- can_handle ---

    #[rstest]
    fn can_handle_modified_file() {
        let provider = make_provider(git_with_modified());
        assert_that!(
            provider.can_handle(&PathBuf::from("/repo/src/main.rs"), &file_node()),
            eq(true)
        );
    }

    #[rstest]
    fn can_handle_added_file() {
        let provider = make_provider(git_with_added());
        assert_that!(
            provider.can_handle(&PathBuf::from("/repo/src/new.rs"), &file_node()),
            eq(true)
        );
    }

    #[rstest]
    fn can_handle_untracked_returns_false() {
        let provider = make_provider(git_with_untracked());
        assert_that!(
            provider.can_handle(&PathBuf::from("/repo/src/scratch.rs"), &file_node()),
            eq(false)
        );
    }

    #[rstest]
    fn can_handle_no_git_state_returns_false() {
        let provider = make_provider(no_git());
        assert_that!(
            provider.can_handle(&PathBuf::from("/repo/src/main.rs"), &file_node()),
            eq(false)
        );
    }

    #[rstest]
    fn can_handle_directory_returns_false() {
        let provider = make_provider(git_with_modified());
        assert_that!(provider.can_handle(&PathBuf::from("/repo/src"), &dir_node()), eq(false));
    }

    #[rstest]
    fn can_handle_clean_file_returns_false() {
        let provider = make_provider(git_with_modified());
        assert_that!(
            provider.can_handle(&PathBuf::from("/repo/src/other.rs"), &file_node()),
            eq(false)
        );
    }

    // --- parse_pager_command ---

    #[rstest]
    fn parse_pager_command_with_args() {
        let (cmd, args) = parse_pager_command("delta --color-only --paging=never").unwrap();
        assert_eq!(cmd, "delta");
        assert_eq!(args, vec!["--color-only", "--paging=never"]);
    }

    #[rstest]
    fn parse_pager_command_no_args() {
        let (cmd, args) = parse_pager_command("delta").unwrap();
        assert_eq!(cmd, "delta");
        assert_that!(args.len(), eq(0));
    }

    #[rstest]
    fn parse_pager_command_empty_string() {
        assert_that!(parse_pager_command(""), none());
    }

    #[rstest]
    fn parse_pager_command_whitespace_only() {
        assert_that!(parse_pager_command("   "), none());
    }

    // --- style_diff_lines ---

    #[rstest]
    fn style_diff_lines_file_header_is_bold() {
        let lines = style_diff_lines("diff --git a/foo b/foo");
        assert_that!(lines.len(), eq(1));
        assert_that!(lines[0].style.add_modifier, eq(Modifier::BOLD));
    }

    #[rstest]
    fn style_diff_lines_hunk_header_is_cyan() {
        let lines = style_diff_lines("@@ -1,3 +1,4 @@ fn main()");
        assert_that!(lines.len(), eq(1));
        assert_that!(lines[0].style.fg, some(eq(Color::Cyan)));
    }

    #[rstest]
    fn style_diff_lines_addition_is_green() {
        let lines = style_diff_lines("+    let x = 1;");
        assert_that!(lines.len(), eq(1));
        assert_that!(lines[0].style.fg, some(eq(Color::Green)));
        assert_that!(lines[0].style.bg, some(eq(Color::Rgb(0, 40, 0))));
    }

    #[rstest]
    fn style_diff_lines_deletion_is_red() {
        let lines = style_diff_lines("-    let x = 1;");
        assert_that!(lines.len(), eq(1));
        assert_that!(lines[0].style.fg, some(eq(Color::Red)));
        assert_that!(lines[0].style.bg, some(eq(Color::Rgb(40, 0, 0))));
    }

    #[rstest]
    fn style_diff_lines_context_is_default() {
        let lines = style_diff_lines("     let y = 2;");
        assert_that!(lines.len(), eq(1));
        assert_that!(lines[0].style, eq(Style::default()));
    }

    #[rstest]
    fn style_diff_lines_minus_header_is_bold() {
        let lines = style_diff_lines("--- a/foo.rs");
        assert_that!(lines.len(), eq(1));
        assert_that!(lines[0].style.add_modifier, eq(Modifier::BOLD));
    }

    #[rstest]
    fn style_diff_lines_plus_header_is_bold() {
        let lines = style_diff_lines("+++ b/foo.rs");
        assert_that!(lines.len(), eq(1));
        assert_that!(lines[0].style.add_modifier, eq(Modifier::BOLD));
    }

    // --- constructor with pager ---

    #[rstest]
    fn new_without_pager_has_no_pager() {
        let provider = DiffPreviewProvider::new(no_git(), PathBuf::from("/repo"), 3, None);
        assert_that!(provider.pager_command, none());
        assert_that!(provider.pager_args.len(), eq(0));
    }

    #[rstest]
    fn new_with_nonexistent_pager_falls_back() {
        let provider = DiffPreviewProvider::new(
            no_git(),
            PathBuf::from("/repo"),
            3,
            Some("nonexistent_pager_12345 --flag"),
        );
        assert_that!(provider.pager_command, none());
    }
}
