//! Application action definitions.

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
}

/// Actions for file operations (copy, move, delete, create, rename, undo/redo, mark).
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(dead_code, reason = "Variants used incrementally as file ops are implemented")]
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
}

/// Actions that modify the tree state.
#[derive(Debug, Clone, PartialEq, Eq)]
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
}

/// Actions for the preview panel.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    #[expect(dead_code, reason = "Keybinding will be re-mapped in future")]
    HalfPageDown,
    /// Scroll preview half a page up.
    HalfPageUp,
    /// Cycle to the next available preview provider.
    CycleProvider,
}
