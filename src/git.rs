//! Git status integration for the tree view.
//!
//! Parses `git status --porcelain=v1` output into a `GitState` mapping
//! (file path → `GitFileStatus`), providing per-file lookups and
//! directory-level aggregation for display in the TUI.

use std::collections::HashMap;
use std::path::{
    Path,
    PathBuf,
};

use ratatui::style::Color;

/// Git status of a single file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GitFileStatus {
    /// Unstaged modifications in working tree.
    Modified,
    /// Staged changes only (no working tree modifications).
    Staged,
    /// Staged changes with additional working tree modifications.
    StagedModified,
    /// Newly added file, staged.
    Added,
    /// Deleted file.
    Deleted,
    /// Renamed file.
    Renamed,
    /// Untracked file.
    Untracked,
    /// Merge conflict.
    Conflicted,
}

/// Repository-wide git status: maps absolute file paths to their status.
#[derive(Debug, Clone)]
pub struct GitState {
    /// File path (absolute) → status mapping.
    statuses: HashMap<PathBuf, GitFileStatus>,
    /// Pre-computed directory → aggregated status mapping.
    ///
    /// Built once during [`from_porcelain`] by propagating each file's status
    /// to all ancestor directories. Lookups in [`dir_status`] are then O(1).
    dir_statuses: HashMap<PathBuf, GitFileStatus>,
}

/// Result of an async git status fetch, sent through a channel.
#[derive(Debug)]
pub struct GitStatusResult {
    /// Fetched git state, or `None` on error / non-repo.
    pub state: Option<GitState>,
}

impl GitFileStatus {
    /// Display character for the status indicator.
    pub const fn char(self) -> char {
        match self {
            Self::Modified | Self::Staged | Self::StagedModified => 'M',
            Self::Added => 'A',
            Self::Deleted => 'D',
            Self::Renamed => 'R',
            Self::Untracked => '?',
            Self::Conflicted => '!',
        }
    }

    /// Display color for the status indicator.
    pub const fn color(self) -> Color {
        match self {
            Self::Modified | Self::StagedModified => Color::Yellow,
            Self::Staged | Self::Added => Color::Green,
            Self::Deleted | Self::Conflicted => Color::Red,
            Self::Renamed => Color::Blue,
            Self::Untracked => Color::Magenta,
        }
    }

    /// Config string representation for matching `git_status` conditions.
    pub const fn config_name(self) -> &'static str {
        match self {
            Self::Modified => "modified",
            Self::Staged => "staged",
            Self::StagedModified => "staged_modified",
            Self::Added => "added",
            Self::Deleted => "deleted",
            Self::Renamed => "renamed",
            Self::Untracked => "untracked",
            Self::Conflicted => "conflicted",
        }
    }

    /// Priority for directory aggregation (higher = more important).
    const fn priority(self) -> u8 {
        match self {
            Self::Conflicted => 7,
            Self::Modified | Self::StagedModified => 6,
            Self::Added | Self::Staged => 5,
            Self::Deleted => 4,
            Self::Renamed => 3,
            Self::Untracked => 2,
        }
    }
}

impl GitState {
    /// Look up the status of a single file by its absolute path.
    pub fn file_status(&self, path: &Path) -> Option<&GitFileStatus> {
        self.statuses.get(path)
    }

    /// Look up the pre-computed aggregated status for a directory.
    ///
    /// Returns the highest-priority status among all descendants.
    /// O(1) lookup into the cache built by [`from_porcelain`].
    pub fn dir_status(&self, dir_path: &Path) -> Option<GitFileStatus> {
        self.dir_statuses.get(dir_path).copied()
    }

    /// Parse `git status --porcelain=v1` output into a `GitState`.
    ///
    /// `repo_root` is the absolute path to the repository root, used to
    /// convert relative paths in the porcelain output to absolute paths.
    pub fn from_porcelain(output: &str, repo_root: &Path) -> Self {
        let mut statuses = HashMap::new();

        for line in output.lines() {
            // Porcelain v1 format: "XY filename" or "XY old -> new" (rename)
            if line.len() < 4 {
                continue;
            }

            let bytes = line.as_bytes();
            let Some(&x) = bytes.first() else { continue };
            let Some(&y) = bytes.get(1) else { continue };
            // bytes[2] is a space
            let rest = &line[3..];

            // Determine file path (handle renames: "old -> new")
            let file_path = if x == b'R' {
                // Rename: use the new path (after " -> ")
                rest.split(" -> ").last().unwrap_or(rest)
            } else {
                rest
            };

            // Remove surrounding quotes if present (git quotes paths with special chars)
            let file_path = file_path.trim_matches('"');

            let abs_path = repo_root.join(file_path);

            let status = match (x, y) {
                // Untracked
                (b'?', b'?') => GitFileStatus::Untracked,
                // Conflict markers
                (b'U', _) | (_, b'U') | (b'A', b'A') | (b'D', b'D') => GitFileStatus::Conflicted,
                // Staged with working tree modifications
                (x, b'M') if x != b' ' && x != b'?' => GitFileStatus::StagedModified,
                // Working tree modifications (unstaged)
                (_, b'M' | b'D') => GitFileStatus::Modified,
                // Renamed
                (b'R', _) => GitFileStatus::Renamed,
                // Deleted (staged)
                (b'D', _) => GitFileStatus::Deleted,
                // Added (staged)
                (b'A', _) => GitFileStatus::Added,
                // Staged (modified in index, clean in worktree)
                (b'M', b' ') => GitFileStatus::Staged,
                // Other staged states
                _ => continue,
            };

            statuses.insert(abs_path, status);
        }

        // Pre-compute directory statuses by propagating each file's status
        // to all ancestor directories up to (and including) the repo root.
        let mut dir_statuses: HashMap<PathBuf, GitFileStatus> = HashMap::new();
        for (path, &status) in &statuses {
            let mut current = path.parent();
            while let Some(dir) = current {
                match dir_statuses.entry(dir.to_path_buf()) {
                    std::collections::hash_map::Entry::Vacant(e) => {
                        e.insert(status);
                    }
                    std::collections::hash_map::Entry::Occupied(mut e) => {
                        if status.priority() > e.get().priority() {
                            e.insert(status);
                        }
                    }
                }
                // Stop at repo root to avoid climbing into parent directories.
                if dir == repo_root {
                    break;
                }
                current = dir.parent();
            }
        }

        Self { statuses, dir_statuses }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    // --- GitFileStatus display char and color ---

    #[rstest]
    #[case(GitFileStatus::Modified, 'M', Color::Yellow)]
    #[case(GitFileStatus::Staged, 'M', Color::Green)]
    #[case(GitFileStatus::StagedModified, 'M', Color::Yellow)]
    #[case(GitFileStatus::Added, 'A', Color::Green)]
    #[case(GitFileStatus::Deleted, 'D', Color::Red)]
    #[case(GitFileStatus::Renamed, 'R', Color::Blue)]
    #[case(GitFileStatus::Untracked, '?', Color::Magenta)]
    #[case(GitFileStatus::Conflicted, '!', Color::Red)]
    fn file_status_char_and_color(
        #[case] status: GitFileStatus,
        #[case] expected_char: char,
        #[case] expected_color: Color,
    ) {
        assert_that!(status.char(), eq(expected_char));
        assert_that!(status.color(), eq(expected_color));
    }

    // --- GitFileStatus priority ordering ---

    #[rstest]
    fn priority_ordering() {
        // Conflicted has the highest priority
        assert!(GitFileStatus::Conflicted.priority() > GitFileStatus::Modified.priority());
        assert!(GitFileStatus::Modified.priority() > GitFileStatus::Added.priority());
        assert!(GitFileStatus::Added.priority() > GitFileStatus::Deleted.priority());
        assert!(GitFileStatus::Deleted.priority() > GitFileStatus::Renamed.priority());
        assert!(GitFileStatus::Renamed.priority() > GitFileStatus::Untracked.priority());
        // StagedModified has same priority as Modified
        assert_that!(
            GitFileStatus::StagedModified.priority(),
            eq(GitFileStatus::Modified.priority())
        );
        // Staged has same priority as Added
        assert_that!(GitFileStatus::Staged.priority(), eq(GitFileStatus::Added.priority()));
    }

    // --- from_porcelain parser ---

    #[rstest]
    fn from_porcelain_modified_unstaged() {
        let output = " M src/main.rs\n";
        let state = GitState::from_porcelain(output, Path::new("/repo"));
        assert_that!(
            state.file_status(Path::new("/repo/src/main.rs")),
            some(eq(&GitFileStatus::Modified))
        );
    }

    #[rstest]
    fn from_porcelain_modified_staged() {
        let output = "M  src/main.rs\n";
        let state = GitState::from_porcelain(output, Path::new("/repo"));
        assert_that!(
            state.file_status(Path::new("/repo/src/main.rs")),
            some(eq(&GitFileStatus::Staged))
        );
    }

    #[rstest]
    fn from_porcelain_staged_modified() {
        let output = "MM src/main.rs\n";
        let state = GitState::from_porcelain(output, Path::new("/repo"));
        assert_that!(
            state.file_status(Path::new("/repo/src/main.rs")),
            some(eq(&GitFileStatus::StagedModified))
        );
    }

    #[rstest]
    fn from_porcelain_added() {
        let output = "A  src/new.rs\n";
        let state = GitState::from_porcelain(output, Path::new("/repo"));
        assert_that!(
            state.file_status(Path::new("/repo/src/new.rs")),
            some(eq(&GitFileStatus::Added))
        );
    }

    #[rstest]
    fn from_porcelain_deleted() {
        let output = "D  src/old.rs\n";
        let state = GitState::from_porcelain(output, Path::new("/repo"));
        assert_that!(
            state.file_status(Path::new("/repo/src/old.rs")),
            some(eq(&GitFileStatus::Deleted))
        );
    }

    #[rstest]
    fn from_porcelain_untracked() {
        let output = "?? draft.txt\n";
        let state = GitState::from_porcelain(output, Path::new("/repo"));
        assert_that!(
            state.file_status(Path::new("/repo/draft.txt")),
            some(eq(&GitFileStatus::Untracked))
        );
    }

    #[rstest]
    fn from_porcelain_conflicted_both_modified() {
        let output = "UU src/conflict.rs\n";
        let state = GitState::from_porcelain(output, Path::new("/repo"));
        assert_that!(
            state.file_status(Path::new("/repo/src/conflict.rs")),
            some(eq(&GitFileStatus::Conflicted))
        );
    }

    #[rstest]
    fn from_porcelain_conflicted_both_added() {
        let output = "AA src/conflict.rs\n";
        let state = GitState::from_porcelain(output, Path::new("/repo"));
        assert_that!(
            state.file_status(Path::new("/repo/src/conflict.rs")),
            some(eq(&GitFileStatus::Conflicted))
        );
    }

    #[rstest]
    fn from_porcelain_multiple_files() {
        let output = " M src/main.rs\nA  src/new.rs\n?? todo.txt\n";
        let state = GitState::from_porcelain(output, Path::new("/repo"));
        assert_that!(
            state.file_status(Path::new("/repo/src/main.rs")),
            some(eq(&GitFileStatus::Modified))
        );
        assert_that!(
            state.file_status(Path::new("/repo/src/new.rs")),
            some(eq(&GitFileStatus::Added))
        );
        assert_that!(
            state.file_status(Path::new("/repo/todo.txt")),
            some(eq(&GitFileStatus::Untracked))
        );
    }

    #[rstest]
    fn from_porcelain_empty_output() {
        let state = GitState::from_porcelain("", Path::new("/repo"));
        assert_that!(state.file_status(Path::new("/repo/anything")), none());
    }

    // --- file_status lookup ---

    #[rstest]
    fn file_status_exact_match() {
        let output = " M src/main.rs\n";
        let state = GitState::from_porcelain(output, Path::new("/repo"));
        assert_that!(
            state.file_status(Path::new("/repo/src/main.rs")),
            some(eq(&GitFileStatus::Modified))
        );
    }

    #[rstest]
    fn file_status_missing_returns_none() {
        let output = " M src/main.rs\n";
        let state = GitState::from_porcelain(output, Path::new("/repo"));
        assert_that!(state.file_status(Path::new("/repo/src/other.rs")), none());
    }

    // --- dir_status aggregation ---

    #[rstest]
    fn dir_status_single_child() {
        let output = " M src/main.rs\n";
        let state = GitState::from_porcelain(output, Path::new("/repo"));
        assert_that!(state.dir_status(Path::new("/repo/src")), some(eq(GitFileStatus::Modified)));
    }

    #[rstest]
    fn dir_status_multiple_children_picks_highest_priority() {
        let output = " M src/main.rs\n?? src/draft.txt\n";
        let state = GitState::from_porcelain(output, Path::new("/repo"));
        // Modified (priority 6) > Untracked (priority 2)
        assert_that!(state.dir_status(Path::new("/repo/src")), some(eq(GitFileStatus::Modified)));
    }

    #[rstest]
    fn dir_status_nested_directories() {
        let output = "A  src/sub/deep/file.rs\n";
        let state = GitState::from_porcelain(output, Path::new("/repo"));
        // Parent directories should aggregate from nested children
        assert_that!(state.dir_status(Path::new("/repo/src")), some(eq(GitFileStatus::Added)));
        assert_that!(state.dir_status(Path::new("/repo/src/sub")), some(eq(GitFileStatus::Added)));
    }

    #[rstest]
    fn dir_status_empty_directory() {
        let output = " M other/file.rs\n";
        let state = GitState::from_porcelain(output, Path::new("/repo"));
        assert_that!(state.dir_status(Path::new("/repo/src")), none());
    }

    // --- rename handling ---

    #[rstest]
    fn from_porcelain_rename_uses_new_path() {
        let output = "R  old_name.rs -> new_name.rs\n";
        let state = GitState::from_porcelain(output, Path::new("/repo"));
        // Status should be on the new path
        assert_that!(
            state.file_status(Path::new("/repo/new_name.rs")),
            some(eq(&GitFileStatus::Renamed))
        );
        // Old path should not have a status
        assert_that!(state.file_status(Path::new("/repo/old_name.rs")), none());
    }

    #[rstest]
    fn from_porcelain_rename_with_directory() {
        let output = "R  src/old.rs -> src/new.rs\n";
        let state = GitState::from_porcelain(output, Path::new("/repo"));
        assert_that!(
            state.file_status(Path::new("/repo/src/new.rs")),
            some(eq(&GitFileStatus::Renamed))
        );
    }

    // --- Performance test for from_porcelain with 10,000 entries ---

    #[rstest]
    #[ignore = "performance test — run with --ignored"]
    fn from_porcelain_10k_entries_within_500ms() {
        use std::fmt::Write;
        let mut output = String::new();
        for i in 0..10_000 {
            let _ = writeln!(output, " M src/file_{i:05}.rs");
        }
        let start = std::time::Instant::now();
        let state = GitState::from_porcelain(&output, Path::new("/repo"));
        let elapsed = start.elapsed();

        assert_that!(elapsed.as_millis() < 500, eq(true));
        assert!(state.file_status(Path::new("/repo/src/file_00000.rs")).is_some());
    }

    // --- Performance test for dir_status with 10,000 entries ---

    #[rstest]
    #[ignore = "performance test — run with --ignored"]
    fn dir_status_10k_entries_within_500ms() {
        use std::fmt::Write;
        let mut output = String::new();
        for i in 0..10_000 {
            let _ = writeln!(output, " M src/sub/file_{i:05}.rs");
        }
        let state = GitState::from_porcelain(&output, Path::new("/repo"));

        let start = std::time::Instant::now();
        let status = state.dir_status(Path::new("/repo/src"));
        let elapsed = start.elapsed();

        assert_that!(elapsed.as_millis() < 500, eq(true));
        assert_that!(status, some(eq(GitFileStatus::Modified)));
    }
}
