//! Application action definitions.

use std::fmt;
use std::str::FromStr;

/// Top-level application actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Tree-related actions.
    Tree(TreeAction),
    /// Preview-related actions.
    Preview(PreviewAction),
    /// File operation actions.
    FileOp(FileOpAction),
    /// Quit the application.
    Quit,
    /// Execute a shell command (template string).
    Shell(String),
    /// Send an IPC notification with the given method name.
    Notify(String),
    /// No operation (used to unbind a default key).
    Noop,
}

/// Actions for file operations (copy, move, delete, create, rename, undo/redo, mark).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOpAction {
    /// Yank (copy) selected files to yank buffer.
    Yank,
    /// Cut selected files to yank buffer.
    Cut,
    /// Paste yank buffer contents to current directory.
    Paste,
    /// Create a new file or directory (opens inline input).
    CreateFile,
    /// Rename the file at cursor (opens inline input).
    Rename,
    /// Delete selected files (mode depends on config).
    Delete,
    /// Send selected files to system trash.
    SystemTrash,
    /// Undo the last operation.
    Undo,
    /// Redo the last undone operation.
    Redo,
    /// Toggle mark on cursor file.
    ToggleMark,
    /// Clear yank buffer and mark set.
    ClearSelections,
    /// Open copy-to-clipboard menu.
    CopyMenu,
}

/// Actions that modify the tree state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeAction {
    /// Move cursor one line down.
    MoveDown,
    /// Move cursor one line up.
    MoveUp,
    /// Expand directory or open file.
    Expand,
    /// Collapse directory or move to parent.
    Collapse,
    /// Toggle expand/collapse state.
    ToggleExpand,
    /// Jump to first visible node.
    JumpFirst,
    /// Jump to last visible node.
    JumpLast,
    /// Move cursor half a page down.
    HalfPageDown,
    /// Move cursor half a page up.
    HalfPageUp,
    /// Expand all directories under cursor recursively.
    ExpandAll,
    /// Collapse all directories under cursor.
    CollapseAll,
    /// Toggle hidden (dot) file visibility.
    ToggleHidden,
    /// Toggle gitignored file visibility.
    ToggleIgnored,
}

/// Actions for the preview panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewAction {
    /// Scroll preview content down.
    ScrollDown,
    /// Scroll preview content up.
    ScrollUp,
    /// Scroll preview content right.
    ScrollRight,
    /// Scroll preview content left.
    ScrollLeft,
    /// Scroll preview half a page down.
    #[allow(dead_code)]
    HalfPageDown,
    /// Scroll preview half a page up.
    HalfPageUp,
    /// Cycle to the next available preview provider.
    CycleProvider,
    /// Toggle preview panel visibility.
    TogglePreview,
}

// ---------------------------------------------------------------------------
// Display implementations (action name → string)
// ---------------------------------------------------------------------------

impl fmt::Display for TreeAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::MoveDown => "tree.move_down",
            Self::MoveUp => "tree.move_up",
            Self::Expand => "tree.expand",
            Self::Collapse => "tree.collapse",
            Self::ToggleExpand => "tree.toggle_expand",
            Self::JumpFirst => "tree.jump_first",
            Self::JumpLast => "tree.jump_last",
            Self::HalfPageDown => "tree.half_page_down",
            Self::HalfPageUp => "tree.half_page_up",
            Self::ExpandAll => "tree.expand_all",
            Self::CollapseAll => "tree.collapse_all",
            Self::ToggleHidden => "tree.toggle_hidden",
            Self::ToggleIgnored => "tree.toggle_ignored",
        };
        f.write_str(s)
    }
}

impl fmt::Display for PreviewAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::ScrollDown => "preview.scroll_down",
            Self::ScrollUp => "preview.scroll_up",
            Self::ScrollRight => "preview.scroll_right",
            Self::ScrollLeft => "preview.scroll_left",
            Self::HalfPageDown => "preview.half_page_down",
            Self::HalfPageUp => "preview.half_page_up",
            Self::CycleProvider => "preview.cycle_provider",
            Self::TogglePreview => "preview.toggle_preview",
        };
        f.write_str(s)
    }
}

impl fmt::Display for FileOpAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Yank => "file_op.yank",
            Self::Cut => "file_op.cut",
            Self::Paste => "file_op.paste",
            Self::CreateFile => "file_op.create_file",
            Self::Rename => "file_op.rename",
            Self::Delete => "file_op.delete",
            Self::SystemTrash => "file_op.system_trash",
            Self::Undo => "file_op.undo",
            Self::Redo => "file_op.redo",
            Self::ToggleMark => "file_op.toggle_mark",
            Self::ClearSelections => "file_op.clear_selections",
            Self::CopyMenu => "file_op.copy_menu",
        };
        f.write_str(s)
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tree(a) => a.fmt(f),
            Self::Preview(a) => a.fmt(f),
            Self::FileOp(a) => a.fmt(f),
            Self::Quit => f.write_str("quit"),
            Self::Shell(cmd) => write!(f, "shell:{cmd}"),
            Self::Notify(method) => write!(f, "notify:{method}"),
            Self::Noop => f.write_str("noop"),
        }
    }
}

// ---------------------------------------------------------------------------
// FromStr implementations (string → action name)
// ---------------------------------------------------------------------------

impl FromStr for TreeAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tree.move_down" => Ok(Self::MoveDown),
            "tree.move_up" => Ok(Self::MoveUp),
            "tree.expand" => Ok(Self::Expand),
            "tree.collapse" => Ok(Self::Collapse),
            "tree.toggle_expand" => Ok(Self::ToggleExpand),
            "tree.jump_first" => Ok(Self::JumpFirst),
            "tree.jump_last" => Ok(Self::JumpLast),
            "tree.half_page_down" => Ok(Self::HalfPageDown),
            "tree.half_page_up" => Ok(Self::HalfPageUp),
            "tree.expand_all" => Ok(Self::ExpandAll),
            "tree.collapse_all" => Ok(Self::CollapseAll),
            "tree.toggle_hidden" => Ok(Self::ToggleHidden),
            "tree.toggle_ignored" => Ok(Self::ToggleIgnored),
            _ => Err(format!("unknown tree action: {s}")),
        }
    }
}

impl FromStr for PreviewAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "preview.scroll_down" => Ok(Self::ScrollDown),
            "preview.scroll_up" => Ok(Self::ScrollUp),
            "preview.scroll_right" => Ok(Self::ScrollRight),
            "preview.scroll_left" => Ok(Self::ScrollLeft),
            "preview.half_page_down" => Ok(Self::HalfPageDown),
            "preview.half_page_up" => Ok(Self::HalfPageUp),
            "preview.cycle_provider" => Ok(Self::CycleProvider),
            "preview.toggle_preview" => Ok(Self::TogglePreview),
            _ => Err(format!("unknown preview action: {s}")),
        }
    }
}

impl FromStr for FileOpAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "file_op.yank" => Ok(Self::Yank),
            "file_op.cut" => Ok(Self::Cut),
            "file_op.paste" => Ok(Self::Paste),
            "file_op.create_file" => Ok(Self::CreateFile),
            "file_op.rename" => Ok(Self::Rename),
            "file_op.delete" => Ok(Self::Delete),
            "file_op.system_trash" => Ok(Self::SystemTrash),
            "file_op.undo" => Ok(Self::Undo),
            "file_op.redo" => Ok(Self::Redo),
            "file_op.toggle_mark" => Ok(Self::ToggleMark),
            "file_op.clear_selections" => Ok(Self::ClearSelections),
            "file_op.copy_menu" => Ok(Self::CopyMenu),
            _ => Err(format!("unknown file_op action: {s}")),
        }
    }
}

impl FromStr for Action {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "quit" => Ok(Self::Quit),
            "noop" => Ok(Self::Noop),
            _ if s.starts_with("tree.") => s.parse::<TreeAction>().map(Self::Tree),
            _ if s.starts_with("preview.") => s.parse::<PreviewAction>().map(Self::Preview),
            _ if s.starts_with("file_op.") => s.parse::<FileOpAction>().map(Self::FileOp),
            _ => Err(format!("unknown action: {s}")),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::literal_string_with_formatting_args
)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    #[rstest]
    #[case("quit", Action::Quit)]
    #[case("noop", Action::Noop)]
    #[case("tree.move_down", Action::Tree(TreeAction::MoveDown))]
    #[case("tree.jump_last", Action::Tree(TreeAction::JumpLast))]
    #[case("preview.scroll_down", Action::Preview(PreviewAction::ScrollDown))]
    #[case("preview.toggle_preview", Action::Preview(PreviewAction::TogglePreview))]
    #[case("file_op.yank", Action::FileOp(FileOpAction::Yank))]
    #[case("file_op.copy_menu", Action::FileOp(FileOpAction::CopyMenu))]
    fn action_roundtrip(#[case] s: &str, #[case] expected: Action) {
        let parsed: Action = s.parse().unwrap();
        assert_eq!(parsed, expected);
        assert_that!(parsed.to_string().as_str(), eq(s));
    }

    #[rstest]
    #[case("unknown")]
    #[case("tree.nonexistent")]
    #[case("preview.nope")]
    #[case("file_op.bad")]
    #[case("")]
    fn action_parse_error(#[case] s: &str) {
        let result = s.parse::<Action>();
        assert!(result.is_err());
    }

    #[rstest]
    fn all_tree_actions_roundtrip() {
        let actions = [
            TreeAction::MoveDown,
            TreeAction::MoveUp,
            TreeAction::Expand,
            TreeAction::Collapse,
            TreeAction::ToggleExpand,
            TreeAction::JumpFirst,
            TreeAction::JumpLast,
            TreeAction::HalfPageDown,
            TreeAction::HalfPageUp,
            TreeAction::ExpandAll,
            TreeAction::CollapseAll,
            TreeAction::ToggleHidden,
            TreeAction::ToggleIgnored,
        ];
        for action in actions {
            let s = action.to_string();
            let parsed: TreeAction = s.parse().unwrap();
            assert_eq!(parsed, action);
        }
    }

    #[rstest]
    fn all_preview_actions_roundtrip() {
        let actions = [
            PreviewAction::ScrollDown,
            PreviewAction::ScrollUp,
            PreviewAction::ScrollRight,
            PreviewAction::ScrollLeft,
            PreviewAction::HalfPageDown,
            PreviewAction::HalfPageUp,
            PreviewAction::CycleProvider,
            PreviewAction::TogglePreview,
        ];
        for action in actions {
            let s = action.to_string();
            let parsed: PreviewAction = s.parse().unwrap();
            assert_eq!(parsed, action);
        }
    }

    #[rstest]
    fn all_file_op_actions_roundtrip() {
        let actions = [
            FileOpAction::Yank,
            FileOpAction::Cut,
            FileOpAction::Paste,
            FileOpAction::CreateFile,
            FileOpAction::Rename,
            FileOpAction::Delete,
            FileOpAction::SystemTrash,
            FileOpAction::Undo,
            FileOpAction::Redo,
            FileOpAction::ToggleMark,
            FileOpAction::ClearSelections,
            FileOpAction::CopyMenu,
        ];
        for action in actions {
            let s = action.to_string();
            let parsed: FileOpAction = s.parse().unwrap();
            assert_eq!(parsed, action);
        }
    }

    #[rstest]
    fn shell_action_display() {
        let action = Action::Shell("open {path}".to_string());
        assert_that!(action.to_string().as_str(), eq("shell:open {path}"));
    }

    #[rstest]
    fn notify_action_display() {
        let action = Action::Notify("open_file".to_string());
        assert_that!(action.to_string().as_str(), eq("notify:open_file"));
    }
}
