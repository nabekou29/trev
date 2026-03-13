//! Application action definitions.

use std::fmt;
use std::str::FromStr;

/// Execution mode for shell commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShellMode {
    /// Suspend TUI, run command, show "Press ENTER to continue...", resume TUI.
    #[default]
    Foreground,
    /// Run command in the background without suspending the TUI.
    Background,
    /// Suspend TUI, run command, resume TUI immediately (for full-screen TUI apps).
    Interactive,
}

/// Top-level application actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Tree-related actions.
    Tree(TreeAction),
    /// Filter-related actions.
    Filter(FilterAction),
    /// Preview-related actions.
    Preview(PreviewAction),
    /// File operation actions.
    FileOp(FileOpAction),
    /// Search-related actions.
    Search(SearchAction),
    /// Quit the application.
    Quit,
    /// Execute a shell command (template string).
    Shell {
        /// The command template to execute.
        cmd: String,
        /// How the command should be executed.
        run_mode: ShellMode,
    },
    /// Send an IPC notification with the given method name.
    Notify(String),
    /// Open a user-defined menu by name.
    OpenMenu(String),
    /// Open the current file in `$VISUAL` / `$EDITOR` (falls back to `vi`).
    OpenEditor,
    /// Show the keybinding help overlay.
    ShowHelp,
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
    /// Create a new file (opens inline input).
    CreateFile,
    /// Create a new directory (opens inline input).
    CreateDirectory,
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
    /// Copy-to-clipboard actions.
    Copy(CopyAction),
}

/// Actions for copy-to-clipboard operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyAction {
    /// Open copy-to-clipboard menu.
    Menu,
    /// Copy absolute path to clipboard.
    AbsolutePath,
    /// Copy relative path (from root) to clipboard.
    RelativePath,
    /// Copy file name to clipboard.
    FileName,
    /// Copy file stem (name without extension) to clipboard.
    Stem,
    /// Copy parent directory path to clipboard.
    ParentDir,
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
    /// Refresh tree structure and git status.
    Refresh,
    /// Change root to the selected directory.
    ChangeRoot,
    /// Change root to the parent of the current root.
    ChangeRootUp,
    /// Center cursor in viewport.
    CenterCursor,
    /// Scroll so the cursor is at the top of the viewport.
    ScrollCursorToTop,
    /// Scroll so the cursor is at the bottom of the viewport.
    ScrollCursorToBottom,
    /// Sort-related actions.
    Sort(SortAction),
}

/// Actions for sort operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortAction {
    /// Open sort selection menu.
    Menu,
    /// Toggle sort direction (asc/desc).
    ToggleDirection,
    /// Sort by name directly.
    ByName,
    /// Sort by file size directly.
    BySize,
    /// Sort by modification time directly.
    ByMtime,
    /// Sort by file type directly.
    ByType,
    /// Sort by file extension directly.
    ByExtension,
    /// Sort by smart (natural) order directly.
    BySmart,
    /// Toggle directories-first sorting.
    ToggleDirectoriesFirst,
}

/// Actions for filter operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterAction {
    /// Toggle hidden (dot) file visibility.
    Hidden,
    /// Toggle gitignored file visibility.
    Ignored,
}

/// Actions for search operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchAction {
    /// Open the search input.
    Open,
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
    HalfPageDown,
    /// Scroll preview half a page up.
    HalfPageUp,
    /// Cycle to the next available preview provider.
    CycleNextProvider,
    /// Cycle to the previous available preview provider.
    CyclePrevProvider,
    /// Toggle preview panel visibility.
    TogglePreview,
    /// Toggle word wrap in preview.
    ToggleWrap,
}

// ---------------------------------------------------------------------------
// Display implementations (action name → string)
// ---------------------------------------------------------------------------

impl fmt::Display for SortAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Menu => "tree.sort.menu",
            Self::ToggleDirection => "tree.sort.toggle_direction",
            Self::ByName => "tree.sort.by_name",
            Self::BySize => "tree.sort.by_size",
            Self::ByMtime => "tree.sort.by_mtime",
            Self::ByType => "tree.sort.by_type",
            Self::ByExtension => "tree.sort.by_extension",
            Self::BySmart => "tree.sort.by_smart",
            Self::ToggleDirectoriesFirst => "tree.sort.toggle_directories_first",
        };
        f.write_str(s)
    }
}

impl fmt::Display for CopyAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Menu => "file_op.copy.menu",
            Self::AbsolutePath => "file_op.copy.absolute_path",
            Self::RelativePath => "file_op.copy.relative_path",
            Self::FileName => "file_op.copy.file_name",
            Self::Stem => "file_op.copy.stem",
            Self::ParentDir => "file_op.copy.parent_dir",
        };
        f.write_str(s)
    }
}

impl fmt::Display for FilterAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Hidden => "filter.hidden",
            Self::Ignored => "filter.ignored",
        };
        f.write_str(s)
    }
}

impl fmt::Display for TreeAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MoveDown => f.write_str("tree.move_down"),
            Self::MoveUp => f.write_str("tree.move_up"),
            Self::Expand => f.write_str("tree.expand"),
            Self::Collapse => f.write_str("tree.collapse"),
            Self::ToggleExpand => f.write_str("tree.toggle_expand"),
            Self::JumpFirst => f.write_str("tree.jump_first"),
            Self::JumpLast => f.write_str("tree.jump_last"),
            Self::HalfPageDown => f.write_str("tree.half_page_down"),
            Self::HalfPageUp => f.write_str("tree.half_page_up"),
            Self::ExpandAll => f.write_str("tree.expand_all"),
            Self::CollapseAll => f.write_str("tree.collapse_all"),
            Self::Refresh => f.write_str("tree.refresh"),
            Self::ChangeRoot => f.write_str("tree.change_root"),
            Self::ChangeRootUp => f.write_str("tree.change_root_up"),
            Self::CenterCursor => f.write_str("tree.center_cursor"),
            Self::ScrollCursorToTop => f.write_str("tree.scroll_cursor_to_top"),
            Self::ScrollCursorToBottom => f.write_str("tree.scroll_cursor_to_bottom"),
            Self::Sort(action) => action.fmt(f),
        }
    }
}

impl fmt::Display for SearchAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Open => "search.open",
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
            Self::CycleNextProvider => "preview.cycle_next_provider",
            Self::CyclePrevProvider => "preview.cycle_prev_provider",
            Self::TogglePreview => "preview.toggle_preview",
            Self::ToggleWrap => "preview.toggle_wrap",
        };
        f.write_str(s)
    }
}

impl fmt::Display for FileOpAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yank => f.write_str("file_op.yank"),
            Self::Cut => f.write_str("file_op.cut"),
            Self::Paste => f.write_str("file_op.paste"),
            Self::CreateFile => f.write_str("file_op.create_file"),
            Self::CreateDirectory => f.write_str("file_op.create_directory"),
            Self::Rename => f.write_str("file_op.rename"),
            Self::Delete => f.write_str("file_op.delete"),
            Self::SystemTrash => f.write_str("file_op.system_trash"),
            Self::Undo => f.write_str("file_op.undo"),
            Self::Redo => f.write_str("file_op.redo"),
            Self::ToggleMark => f.write_str("file_op.toggle_mark"),
            Self::ClearSelections => f.write_str("file_op.clear_selections"),
            Self::Copy(action) => action.fmt(f),
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tree(a) => a.fmt(f),
            Self::Filter(a) => a.fmt(f),
            Self::Preview(a) => a.fmt(f),
            Self::FileOp(a) => a.fmt(f),
            Self::Search(a) => a.fmt(f),
            Self::Quit => f.write_str("quit"),
            Self::ShowHelp => f.write_str("help"),
            Self::OpenEditor => f.write_str("open_editor"),
            Self::Shell { cmd, .. } => write!(f, "shell:{cmd}"),
            Self::Notify(method) => write!(f, "notify:{method}"),
            Self::OpenMenu(name) => write!(f, "menu:{name}"),
            Self::Noop => f.write_str("noop"),
        }
    }
}

// ---------------------------------------------------------------------------
// FromStr implementations (string → action name)
// ---------------------------------------------------------------------------

impl FromStr for SortAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tree.sort.menu" => Ok(Self::Menu),
            "tree.sort.toggle_direction" => Ok(Self::ToggleDirection),
            "tree.sort.by_name" => Ok(Self::ByName),
            "tree.sort.by_size" => Ok(Self::BySize),
            "tree.sort.by_mtime" => Ok(Self::ByMtime),
            "tree.sort.by_type" => Ok(Self::ByType),
            "tree.sort.by_extension" => Ok(Self::ByExtension),
            "tree.sort.by_smart" => Ok(Self::BySmart),
            "tree.sort.toggle_directories_first" => Ok(Self::ToggleDirectoriesFirst),
            _ => Err(format!("unknown sort action: {s}")),
        }
    }
}

impl FromStr for CopyAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "file_op.copy.menu" => Ok(Self::Menu),
            "file_op.copy.absolute_path" => Ok(Self::AbsolutePath),
            "file_op.copy.relative_path" => Ok(Self::RelativePath),
            "file_op.copy.file_name" => Ok(Self::FileName),
            "file_op.copy.stem" => Ok(Self::Stem),
            "file_op.copy.parent_dir" => Ok(Self::ParentDir),
            _ => Err(format!("unknown copy action: {s}")),
        }
    }
}

impl FromStr for FilterAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "filter.hidden" => Ok(Self::Hidden),
            "filter.ignored" => Ok(Self::Ignored),
            _ => Err(format!("unknown filter action: {s}")),
        }
    }
}

impl FromStr for TreeAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("tree.sort.") {
            return s.parse::<SortAction>().map(Self::Sort);
        }
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
            "tree.refresh" => Ok(Self::Refresh),
            "tree.change_root" => Ok(Self::ChangeRoot),
            "tree.change_root_up" => Ok(Self::ChangeRootUp),
            "tree.center_cursor" => Ok(Self::CenterCursor),
            "tree.scroll_cursor_to_top" => Ok(Self::ScrollCursorToTop),
            "tree.scroll_cursor_to_bottom" => Ok(Self::ScrollCursorToBottom),
            _ => Err(format!("unknown tree action: {s}")),
        }
    }
}

impl FromStr for SearchAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "search.open" => Ok(Self::Open),
            _ => Err(format!("unknown search action: {s}")),
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
            "preview.cycle_next_provider" => Ok(Self::CycleNextProvider),
            "preview.cycle_prev_provider" => Ok(Self::CyclePrevProvider),
            "preview.toggle_preview" => Ok(Self::TogglePreview),
            "preview.toggle_wrap" => Ok(Self::ToggleWrap),
            _ => Err(format!("unknown preview action: {s}")),
        }
    }
}

impl FromStr for FileOpAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("file_op.copy.") {
            return s.parse::<CopyAction>().map(Self::Copy);
        }
        match s {
            "file_op.yank" => Ok(Self::Yank),
            "file_op.cut" => Ok(Self::Cut),
            "file_op.paste" => Ok(Self::Paste),
            "file_op.create_file" => Ok(Self::CreateFile),
            "file_op.create_directory" => Ok(Self::CreateDirectory),
            "file_op.rename" => Ok(Self::Rename),
            "file_op.delete" => Ok(Self::Delete),
            "file_op.system_trash" => Ok(Self::SystemTrash),
            "file_op.undo" => Ok(Self::Undo),
            "file_op.redo" => Ok(Self::Redo),
            "file_op.toggle_mark" => Ok(Self::ToggleMark),
            "file_op.clear_selections" => Ok(Self::ClearSelections),
            _ => Err(format!("unknown file_op action: {s}")),
        }
    }
}

impl FromStr for Action {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "quit" => Ok(Self::Quit),
            "open_editor" => Ok(Self::OpenEditor),
            "help" => Ok(Self::ShowHelp),
            "noop" => Ok(Self::Noop),
            _ if s.starts_with("tree.") => s.parse::<TreeAction>().map(Self::Tree),
            _ if s.starts_with("filter.") => s.parse::<FilterAction>().map(Self::Filter),
            _ if s.starts_with("preview.") => s.parse::<PreviewAction>().map(Self::Preview),
            _ if s.starts_with("file_op.") => s.parse::<FileOpAction>().map(Self::FileOp),
            _ if s.starts_with("search.") => s.parse::<SearchAction>().map(Self::Search),
            _ if s.starts_with("menu:") => {
                let name = &s["menu:".len()..];
                if name.is_empty() {
                    Err("menu action requires a name".to_string())
                } else {
                    Ok(Self::OpenMenu(name.to_string()))
                }
            }
            _ => Err(format!("unknown action: {s}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Action name helpers (for JSON Schema generation)
// ---------------------------------------------------------------------------

impl SortAction {
    /// Returns all valid sort action name strings.
    pub(crate) fn action_names() -> Vec<&'static str> {
        vec![
            "tree.sort.menu",
            "tree.sort.toggle_direction",
            "tree.sort.by_name",
            "tree.sort.by_size",
            "tree.sort.by_mtime",
            "tree.sort.by_type",
            "tree.sort.by_extension",
            "tree.sort.by_smart",
            "tree.sort.toggle_directories_first",
        ]
    }
}

impl CopyAction {
    /// Returns all valid copy action name strings.
    pub(crate) fn action_names() -> Vec<&'static str> {
        vec![
            "file_op.copy.menu",
            "file_op.copy.absolute_path",
            "file_op.copy.relative_path",
            "file_op.copy.file_name",
            "file_op.copy.stem",
            "file_op.copy.parent_dir",
        ]
    }
}

impl SearchAction {
    /// Returns all valid search action name strings.
    pub(crate) fn action_names() -> Vec<&'static str> {
        vec!["search.open"]
    }
}

impl FilterAction {
    /// Returns all valid filter action name strings.
    pub(crate) fn action_names() -> Vec<&'static str> {
        vec!["filter.hidden", "filter.ignored"]
    }
}

impl TreeAction {
    /// Returns all valid tree action name strings (including sort sub-actions).
    pub(crate) fn action_names() -> Vec<&'static str> {
        let mut names = vec![
            "tree.move_down",
            "tree.move_up",
            "tree.expand",
            "tree.collapse",
            "tree.toggle_expand",
            "tree.jump_first",
            "tree.jump_last",
            "tree.half_page_down",
            "tree.half_page_up",
            "tree.expand_all",
            "tree.collapse_all",
            "tree.refresh",
            "tree.change_root",
            "tree.change_root_up",
            "tree.center_cursor",
            "tree.scroll_cursor_to_top",
            "tree.scroll_cursor_to_bottom",
        ];
        names.extend(SortAction::action_names());
        names
    }
}

impl PreviewAction {
    /// Returns all valid preview action name strings.
    pub(crate) fn action_names() -> Vec<&'static str> {
        vec![
            "preview.scroll_down",
            "preview.scroll_up",
            "preview.scroll_right",
            "preview.scroll_left",
            "preview.half_page_down",
            "preview.half_page_up",
            "preview.cycle_next_provider",
            "preview.cycle_prev_provider",
            "preview.toggle_preview",
            "preview.toggle_wrap",
        ]
    }
}

impl FileOpAction {
    /// Returns all valid file operation action name strings (including copy sub-actions).
    pub(crate) fn action_names() -> Vec<&'static str> {
        let mut names = vec![
            "file_op.yank",
            "file_op.cut",
            "file_op.paste",
            "file_op.create_file",
            "file_op.create_directory",
            "file_op.rename",
            "file_op.delete",
            "file_op.system_trash",
            "file_op.undo",
            "file_op.redo",
            "file_op.toggle_mark",
            "file_op.clear_selections",
        ];
        names.extend(CopyAction::action_names());
        names
    }
}

impl Action {
    /// Returns all valid action name strings for JSON Schema generation.
    pub fn all_action_names() -> Vec<&'static str> {
        let mut names = vec!["quit", "open_editor", "help", "noop"];
        names.extend(TreeAction::action_names());
        names.extend(FilterAction::action_names());
        names.extend(PreviewAction::action_names());
        names.extend(FileOpAction::action_names());
        names.extend(SearchAction::action_names());
        names
    }

    /// Human-readable description of this action for the help overlay.
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Quit => "Quit",
            Self::OpenEditor => "Open in editor",
            Self::ShowHelp => "Show help",
            Self::Noop => "No operation",
            Self::Shell { .. } => "Run shell command",
            Self::Notify(_) => "Send IPC notification",
            Self::OpenMenu(_) => "Open menu",
            Self::Tree(a) => a.description(),
            Self::Filter(a) => a.description(),
            Self::Preview(a) => a.description(),
            Self::FileOp(a) => a.description(),
            Self::Search(a) => a.description(),
        }
    }
}

impl TreeAction {
    /// Human-readable description of this action for the help overlay.
    pub const fn description(&self) -> &'static str {
        match self {
            Self::MoveDown => "Move down",
            Self::MoveUp => "Move up",
            Self::Expand => "Expand / Open",
            Self::Collapse => "Collapse / Parent",
            Self::ToggleExpand => "Toggle expand",
            Self::JumpFirst => "Jump to first",
            Self::JumpLast => "Jump to last",
            Self::HalfPageDown => "Half page down",
            Self::HalfPageUp => "Half page up",
            Self::ExpandAll => "Expand all",
            Self::CollapseAll => "Collapse all",
            Self::Refresh => "Refresh tree",
            Self::ChangeRoot => "Change root to selected",
            Self::ChangeRootUp => "Change root to parent",
            Self::CenterCursor => "Center cursor in view",
            Self::ScrollCursorToTop => "Scroll cursor to top",
            Self::ScrollCursorToBottom => "Scroll cursor to bottom",
            Self::Sort(a) => a.description(),
        }
    }
}

impl SortAction {
    /// Human-readable description of this action for the help overlay.
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Menu => "Open sort menu",
            Self::ToggleDirection => "Toggle sort direction",
            Self::ByName => "Sort by name",
            Self::BySize => "Sort by size",
            Self::ByMtime => "Sort by modified time",
            Self::ByType => "Sort by type",
            Self::ByExtension => "Sort by extension",
            Self::BySmart => "Sort by natural order",
            Self::ToggleDirectoriesFirst => "Toggle directories first",
        }
    }
}

impl FilterAction {
    /// Human-readable description of this action for the help overlay.
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Hidden => "Toggle hidden files",
            Self::Ignored => "Toggle ignored files",
        }
    }
}

impl SearchAction {
    /// Human-readable description of this action for the help overlay.
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Open => "Open search",
        }
    }
}

impl PreviewAction {
    /// Human-readable description of this action for the help overlay.
    pub const fn description(&self) -> &'static str {
        match self {
            Self::ScrollDown => "Scroll down",
            Self::ScrollUp => "Scroll up",
            Self::ScrollRight => "Scroll right",
            Self::ScrollLeft => "Scroll left",
            Self::HalfPageDown => "Half page down",
            Self::HalfPageUp => "Half page up",
            Self::CycleNextProvider => "Next provider",
            Self::CyclePrevProvider => "Previous provider",
            Self::TogglePreview => "Toggle preview",
            Self::ToggleWrap => "Toggle word wrap",
        }
    }
}

impl FileOpAction {
    /// Human-readable description of this action for the help overlay.
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Yank => "Copy (yank) files",
            Self::Cut => "Cut files",
            Self::Paste => "Paste files",
            Self::CreateFile => "Create file",
            Self::CreateDirectory => "Create directory",
            Self::Rename => "Rename",
            Self::Delete => "Delete",
            Self::SystemTrash => "Move to trash",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::ToggleMark => "Toggle mark",
            Self::ClearSelections => "Clear selections",
            Self::Copy(a) => a.description(),
        }
    }
}

impl CopyAction {
    /// Human-readable description of this action for the help overlay.
    pub const fn description(&self) -> &'static str {
        match self {
            Self::Menu => "Open copy menu",
            Self::AbsolutePath => "Copy absolute path",
            Self::RelativePath => "Copy relative path",
            Self::FileName => "Copy file name",
            Self::Stem => "Copy name without extension",
            Self::ParentDir => "Copy parent directory",
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::literal_string_with_formatting_args)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    #[rstest]
    #[case("unknown")]
    #[case("tree.nonexistent")]
    #[case("preview.nope")]
    #[case("file_op.bad")]
    #[case("filter.nope")]
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
            TreeAction::Refresh,
            TreeAction::CenterCursor,
            TreeAction::ScrollCursorToTop,
            TreeAction::ScrollCursorToBottom,
            TreeAction::Sort(SortAction::Menu),
            TreeAction::Sort(SortAction::ToggleDirection),
            TreeAction::Sort(SortAction::ByName),
            TreeAction::Sort(SortAction::BySize),
            TreeAction::Sort(SortAction::ByMtime),
            TreeAction::Sort(SortAction::ByType),
            TreeAction::Sort(SortAction::ByExtension),
            TreeAction::Sort(SortAction::BySmart),
            TreeAction::Sort(SortAction::ToggleDirectoriesFirst),
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
            PreviewAction::CycleNextProvider,
            PreviewAction::CyclePrevProvider,
            PreviewAction::TogglePreview,
            PreviewAction::ToggleWrap,
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
            FileOpAction::Copy(CopyAction::Menu),
            FileOpAction::Copy(CopyAction::AbsolutePath),
            FileOpAction::Copy(CopyAction::RelativePath),
            FileOpAction::Copy(CopyAction::FileName),
            FileOpAction::Copy(CopyAction::Stem),
            FileOpAction::Copy(CopyAction::ParentDir),
        ];
        for action in actions {
            let s = action.to_string();
            let parsed: FileOpAction = s.parse().unwrap();
            assert_eq!(parsed, action);
        }
    }

    #[rstest]
    fn all_filter_actions_roundtrip() {
        let actions = [FilterAction::Hidden, FilterAction::Ignored];
        for action in actions {
            let s = action.to_string();
            let parsed: FilterAction = s.parse().unwrap();
            assert_eq!(parsed, action);
        }
    }

    #[rstest]
    fn shell_action_display() {
        let action = Action::Shell { cmd: "open {path}".to_string(), run_mode: ShellMode::Foreground };
        assert_that!(action.to_string().as_str(), eq("shell:open {path}"));
    }

    #[rstest]
    fn notify_action_display() {
        let action = Action::Notify("open_file".to_string());
        assert_that!(action.to_string().as_str(), eq("notify:open_file"));
    }

    #[rstest]
    fn open_menu_action_roundtrip() {
        let action = Action::OpenMenu("my_menu".to_string());
        assert_that!(action.to_string().as_str(), eq("menu:my_menu"));

        let parsed: Action = "menu:my_menu".parse().unwrap();
        assert_eq!(parsed, Action::OpenMenu("my_menu".to_string()));
    }

    #[rstest]
    fn open_menu_action_empty_name_is_error() {
        let result = "menu:".parse::<Action>();
        assert!(result.is_err());
    }

    #[rstest]
    fn all_action_names_roundtrip_all() {
        let names = Action::all_action_names();
        for name in &names {
            let parsed: Action = name.parse().unwrap();
            assert_that!(parsed.to_string().as_str(), eq(*name));
        }
    }
}
