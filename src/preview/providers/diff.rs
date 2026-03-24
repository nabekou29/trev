//! Git diff preview provider.
//!
//! Shows `git diff` output for files with changes. Uses ANSI color output
//! converted to ratatui `Text` via `ansi-to-tui`.

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
#[derive(Debug)]
pub struct DiffPreviewProvider {
    /// Shared git repository state.
    git_state: Arc<RwLock<Option<GitState>>>,
    /// Repository root path (used as `current_dir` for git commands).
    root_path: PathBuf,
    /// Timeout for the git diff command.
    timeout: Duration,
}

impl DiffPreviewProvider {
    /// Create a new diff preview provider.
    pub const fn new(
        git_state: Arc<RwLock<Option<GitState>>>,
        root_path: PathBuf,
        timeout_secs: u64,
    ) -> Self {
        Self { git_state, root_path, timeout: Duration::from_secs(timeout_secs) }
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

        // Use --cached for Added files (staged only), HEAD for others.
        let mut cmd = Command::new("git");
        if matches!(status, Some(GitFileStatus::Added)) {
            cmd.args(["diff", "--cached", "--color=always", "--"]);
        } else {
            cmd.args(["diff", "HEAD", "--color=always", "--"]);
        }
        cmd.arg(path);
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

        // Convert ANSI output to ratatui Text.
        let text = stdout.into_text().map_err(|e| anyhow::anyhow!("{e}"))?;

        Ok(PreviewContent::AnsiText { text })
    }
}

/// Wait for a child process with a timeout.
///
/// If the timeout expires, returns an error.
fn wait_with_timeout(
    child: std::process::Child,
    timeout: Duration,
) -> anyhow::Result<std::process::Output> {
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = std::thread::spawn(move || {
        let child = child;
        let result = child.wait_with_output();
        let _ = tx.send(());
        result
    });

    match rx.recv_timeout(timeout) {
        Ok(()) => handle
            .join()
            .map_err(|_| anyhow::anyhow!("git diff thread panicked"))?
            .map_err(Into::into),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            Err(anyhow::anyhow!("git diff timed out"))
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            Err(anyhow::anyhow!("git diff thread disconnected"))
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
        DiffPreviewProvider::new(git_state, PathBuf::from("/repo"), 3)
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
}
