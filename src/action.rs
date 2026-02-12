//! Application action definitions.

/// Top-level application actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Tree-related actions.
    Tree(TreeAction),
    /// Preview-related actions.
    Preview(PreviewAction),
    /// Quit the application.
    Quit,
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
    HalfPageDown,
    /// Scroll preview half a page up.
    HalfPageUp,
    /// Cycle to the next available preview provider.
    CycleProvider,
}
