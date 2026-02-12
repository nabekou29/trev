//! Application state machine and main event loop.

use std::path::{
    Path,
    PathBuf,
};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{
    Event,
    KeyCode,
    KeyModifiers,
};

use crate::cli::Args;
use crate::config::Config;
use crate::state::tree::{
    TreeNode,
    TreeState,
};
use crate::tree::builder::TreeBuilder;

/// Application-wide state wrapping tree state and UI settings.
#[derive(Debug)]
pub struct AppState {
    /// Tree state (cursor, sort, nodes).
    pub tree_state: TreeState,
    /// Whether the application should quit.
    pub should_quit: bool,
    /// Whether to show file icons (Nerd Fonts).
    pub show_icons: bool,
    /// Current viewport height (tree area rows).
    pub viewport_height: u16,
    /// Scroll state for the tree view.
    pub scroll: ScrollState,
}

/// Scroll position management for the tree view.
#[derive(Debug)]
pub struct ScrollState {
    /// Scroll offset (first visible row index).
    offset: usize,
}

impl ScrollState {
    /// Create a new scroll state starting at the top.
    pub const fn new() -> Self {
        Self { offset: 0 }
    }

    /// Get the current scroll offset.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Clamp the scroll offset so the cursor stays visible.
    ///
    /// Ensures `cursor - viewport_height + 1 <= offset <= cursor`.
    pub const fn clamp_to_cursor(&mut self, cursor: usize, viewport_height: usize) {
        if viewport_height == 0 {
            self.offset = 0;
            return;
        }
        // If cursor is above the current viewport, scroll up.
        if cursor < self.offset {
            self.offset = cursor;
        }
        // If cursor is below the current viewport, scroll down.
        if cursor >= self.offset + viewport_height {
            self.offset = cursor.saturating_sub(viewport_height - 1);
        }
    }
}

/// Result of an async directory children load operation.
///
/// Sent through an mpsc channel from the blocking task to the event loop.
#[derive(Debug)]
pub struct ChildrenLoadResult {
    /// Path of the directory whose children were loaded.
    pub path: PathBuf,
    /// Loaded children, or an error message.
    pub children: Result<Vec<TreeNode>, String>,
    /// Whether this was a background prefetch (one-level-ahead load).
    pub prefetch: bool,
}

/// Convert config sort order to tree state sort order.
const fn map_sort_order(order: crate::config::SortOrder) -> crate::state::tree::SortOrder {
    match order {
        crate::config::SortOrder::Name => crate::state::tree::SortOrder::Name,
        crate::config::SortOrder::Size => crate::state::tree::SortOrder::Size,
        crate::config::SortOrder::Mtime => crate::state::tree::SortOrder::Modified,
        crate::config::SortOrder::Type => crate::state::tree::SortOrder::Type,
        crate::config::SortOrder::Extension => crate::state::tree::SortOrder::Extension,
    }
}

/// Convert config sort direction to tree state sort direction.
const fn map_sort_direction(direction: crate::config::SortDirection) -> crate::state::tree::SortDirection {
    match direction {
        crate::config::SortDirection::Asc => crate::state::tree::SortDirection::Asc,
        crate::config::SortDirection::Desc => crate::state::tree::SortDirection::Desc,
    }
}

/// Run the application.
#[allow(clippy::unused_async)]
pub async fn run(args: &Args) -> Result<()> {
    let config = Config::load()?;
    tracing::info!(?args, "starting trev");

    // Resolve the root path.
    let root_path = std::fs::canonicalize(&args.path)?;

    // Map config sort settings to tree state types.
    let sort_order = map_sort_order(config.sort.order);
    let sort_direction = map_sort_direction(config.sort.direction);
    let directories_first = config.sort.directories_first;

    // Build the initial tree (depth 1).
    let show_hidden = args.show_hidden || config.display.show_hidden;
    let show_ignored = false;
    let builder = TreeBuilder::new(show_hidden, show_ignored);
    let root = builder.build(&root_path)?;

    // Create tree state with sort settings.
    let mut tree_state = TreeState::new(root, sort_order, sort_direction, directories_first);
    tree_state.apply_sort(sort_order, sort_direction, directories_first);

    // Create app state.
    let mut state = AppState {
        tree_state,
        should_quit: false,
        show_icons: !args.no_icons,
        viewport_height: 0,
        scroll: ScrollState::new(),
    };

    // Set up async children load channel.
    let (children_tx, mut children_rx) =
        tokio::sync::mpsc::channel::<ChildrenLoadResult>(64);

    // Prefetch root's child directories for instant first expansion.
    trigger_prefetch(
        &mut state.tree_state,
        &root_path,
        &children_tx,
        show_hidden,
        show_ignored,
    );

    // Initialize terminal.
    let mut terminal = crate::terminal::init();

    // Main event loop.
    loop {
        // Draw UI.
        terminal.draw(|frame| {
            crate::ui::render(frame, &mut state);
        })?;

        // Poll for events (50ms timeout for responsive async result handling).
        if crossterm::event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = crossterm::event::read()?
        {
            handle_key_event(key, &mut state, &children_tx, show_hidden, show_ignored);
        }

        // Receive async children load results.
        while let Ok(result) = children_rx.try_recv() {
            match result.children {
                Ok(children) => {
                    let loaded_path = result.path.clone();
                    let is_prefetch = result.prefetch;
                    state
                        .tree_state
                        .set_children(&result.path, children, !is_prefetch);

                    // Prefetch child directories one level ahead (user-initiated loads only).
                    if !is_prefetch {
                        trigger_prefetch(
                            &mut state.tree_state,
                            &loaded_path,
                            &children_tx,
                            show_hidden,
                            show_ignored,
                        );
                    }
                }
                Err(err) => {
                    tracing::warn!(?result.path, %err, "failed to load children");
                    state.tree_state.set_children_error(&result.path);
                }
            }
        }

        // Update scroll position.
        state.scroll.clamp_to_cursor(
            state.tree_state.cursor(),
            state.viewport_height as usize,
        );

        if state.should_quit {
            break;
        }
    }

    // Restore terminal.
    crate::terminal::restore();

    Ok(())
}

/// Handle a key event and update application state.
fn handle_key_event(
    key: crossterm::event::KeyEvent,
    state: &mut AppState,
    children_tx: &tokio::sync::mpsc::Sender<ChildrenLoadResult>,
    show_hidden: bool,
    show_ignored: bool,
) {
    let Some(action) = map_key_event(key) else {
        return;
    };

    match action {
        crate::action::Action::Quit => {
            state.should_quit = true;
        }
        crate::action::Action::Tree(ref tree_action) => {
            handle_tree_action(tree_action, state, children_tx, show_hidden, show_ignored);
        }
    }
}

/// Map a key event to an action.
const fn map_key_event(key: crossterm::event::KeyEvent) -> Option<crate::action::Action> {
    use crate::action::{
        Action,
        TreeAction,
    };

    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), KeyModifiers::NONE) => Some(Action::Quit),
        (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => {
            Some(Action::Tree(TreeAction::MoveDown))
        }
        (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => {
            Some(Action::Tree(TreeAction::MoveUp))
        }
        (KeyCode::Char('l'), KeyModifiers::NONE) | (KeyCode::Right, _) => {
            Some(Action::Tree(TreeAction::Expand))
        }
        (KeyCode::Char('h'), KeyModifiers::NONE) | (KeyCode::Left, _) => {
            Some(Action::Tree(TreeAction::Collapse))
        }
        (KeyCode::Enter, KeyModifiers::NONE) => Some(Action::Tree(TreeAction::ToggleExpand)),
        (KeyCode::Char('g'), KeyModifiers::NONE) => Some(Action::Tree(TreeAction::JumpFirst)),
        (KeyCode::Char('G'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
            Some(Action::Tree(TreeAction::JumpLast))
        }
        (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
            Some(Action::Tree(TreeAction::HalfPageDown))
        }
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            Some(Action::Tree(TreeAction::HalfPageUp))
        }
        _ => None,
    }
}

/// Handle a tree action.
fn handle_tree_action(
    action: &crate::action::TreeAction,
    state: &mut AppState,
    children_tx: &tokio::sync::mpsc::Sender<ChildrenLoadResult>,
    show_hidden: bool,
    show_ignored: bool,
) {
    use crate::action::TreeAction;

    match *action {
        TreeAction::MoveDown => {
            state.tree_state.move_cursor(1);
        }
        TreeAction::MoveUp => {
            state.tree_state.move_cursor(-1);
        }
        TreeAction::Expand => {
            if let Some(result) = state.tree_state.expand_or_open() {
                handle_expand_result(
                    &result,
                    &mut state.tree_state,
                    children_tx,
                    show_hidden,
                    show_ignored,
                );
            }
        }
        TreeAction::Collapse => {
            state.tree_state.collapse();
        }
        TreeAction::ToggleExpand => {
            if let Some(result) = state.tree_state.toggle_expand(state.tree_state.cursor()) {
                handle_expand_result(
                    &result,
                    &mut state.tree_state,
                    children_tx,
                    show_hidden,
                    show_ignored,
                );
            }
        }
        TreeAction::JumpFirst => {
            state.tree_state.jump_to_first();
        }
        TreeAction::JumpLast => {
            state.tree_state.jump_to_last();
        }
        TreeAction::HalfPageDown => {
            state.tree_state.half_page_down(state.viewport_height as usize);
        }
        TreeAction::HalfPageUp => {
            state.tree_state.half_page_up(state.viewport_height as usize);
        }
    }
}

/// Handle an expand result: spawn loads or prefetch as appropriate.
fn handle_expand_result(
    result: &crate::state::tree::ExpandResult,
    tree_state: &mut TreeState,
    children_tx: &tokio::sync::mpsc::Sender<ChildrenLoadResult>,
    show_hidden: bool,
    show_ignored: bool,
) {
    use crate::state::tree::ExpandResult;

    match *result {
        ExpandResult::NeedsLoad(ref path) => {
            spawn_load_children(children_tx.clone(), path.clone(), show_hidden, show_ignored, false);
        }
        ExpandResult::AlreadyLoaded(ref path) => {
            // Directory was prefetched — trigger prefetch for its children.
            trigger_prefetch(tree_state, path, children_tx, show_hidden, show_ignored);
        }
        ExpandResult::OpenFile(_) => {
            // File opening not implemented yet.
        }
    }
}

/// Prefetch child directories one level ahead.
///
/// Transitions `NotLoaded` child directories to `Loading` and spawns
/// background tasks to load their children.
fn trigger_prefetch(
    tree_state: &mut TreeState,
    parent_path: &Path,
    children_tx: &tokio::sync::mpsc::Sender<ChildrenLoadResult>,
    show_hidden: bool,
    show_ignored: bool,
) {
    let prefetch_paths = tree_state.start_prefetch(parent_path);
    for path in prefetch_paths {
        spawn_load_children(children_tx.clone(), path, show_hidden, show_ignored, true);
    }
}

/// Spawn a blocking task to load directory children asynchronously.
fn spawn_load_children(
    tx: tokio::sync::mpsc::Sender<ChildrenLoadResult>,
    path: PathBuf,
    show_hidden: bool,
    show_ignored: bool,
    prefetch: bool,
) {
    tokio::spawn(async move {
        let load_path = path.clone();
        let result = tokio::task::spawn_blocking(move || {
            let builder = TreeBuilder::new(show_hidden, show_ignored);
            builder.load_children(&load_path)
        })
        .await;

        let children = match result {
            Ok(Ok(children)) => Ok(children),
            Ok(Err(err)) => Err(err.to_string()),
            Err(err) => Err(err.to_string()),
        };

        // Ignore send error (receiver dropped = app is shutting down).
        let _ = tx
            .send(ChildrenLoadResult {
                path,
                children,
                prefetch,
            })
            .await;
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    #[rstest]
    fn scroll_state_clamp_cursor_below_viewport() {
        let mut scroll = ScrollState::new();
        // Cursor at 15, viewport 10 → offset should move to 6.
        scroll.clamp_to_cursor(15, 10);
        assert_that!(scroll.offset(), eq(6));
    }

    #[rstest]
    fn scroll_state_clamp_cursor_above_viewport() {
        let mut scroll = ScrollState { offset: 10 };
        // Cursor at 5, viewport 10 → offset should move to 5.
        scroll.clamp_to_cursor(5, 10);
        assert_that!(scroll.offset(), eq(5));
    }

    #[rstest]
    fn scroll_state_clamp_cursor_within_viewport() {
        let mut scroll = ScrollState { offset: 5 };
        // Cursor at 8, viewport 10 → offset stays at 5.
        scroll.clamp_to_cursor(8, 10);
        assert_that!(scroll.offset(), eq(5));
    }

    #[rstest]
    fn scroll_state_clamp_zero_viewport() {
        let mut scroll = ScrollState { offset: 5 };
        scroll.clamp_to_cursor(3, 0);
        assert_that!(scroll.offset(), eq(0));
    }

    #[rstest]
    fn map_key_q_to_quit() {
        use crate::action::Action;
        let key = crossterm::event::KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(map_key_event(key), Some(Action::Quit));
    }

    #[rstest]
    fn map_key_j_to_move_down() {
        use crate::action::{Action, TreeAction};
        let key = crossterm::event::KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(map_key_event(key), Some(Action::Tree(TreeAction::MoveDown)));
    }

    #[rstest]
    fn map_key_k_to_move_up() {
        use crate::action::{Action, TreeAction};
        let key = crossterm::event::KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        assert_eq!(map_key_event(key), Some(Action::Tree(TreeAction::MoveUp)));
    }

    #[rstest]
    fn map_key_l_to_expand() {
        use crate::action::{Action, TreeAction};
        let key = crossterm::event::KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE);
        assert_eq!(map_key_event(key), Some(Action::Tree(TreeAction::Expand)));
    }

    #[rstest]
    fn map_key_h_to_collapse() {
        use crate::action::{Action, TreeAction};
        let key = crossterm::event::KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
        assert_eq!(map_key_event(key), Some(Action::Tree(TreeAction::Collapse)));
    }

    #[rstest]
    fn map_key_enter_to_toggle() {
        use crate::action::{Action, TreeAction};
        let key = crossterm::event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(map_key_event(key), Some(Action::Tree(TreeAction::ToggleExpand)));
    }

    #[rstest]
    fn map_key_g_to_jump_first() {
        use crate::action::{Action, TreeAction};
        let key = crossterm::event::KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        assert_eq!(map_key_event(key), Some(Action::Tree(TreeAction::JumpFirst)));
    }

    #[rstest]
    fn map_key_shift_g_to_jump_last() {
        use crate::action::{Action, TreeAction};
        let key = crossterm::event::KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);
        assert_eq!(map_key_event(key), Some(Action::Tree(TreeAction::JumpLast)));
    }

    #[rstest]
    fn map_key_ctrl_d_to_half_page_down() {
        use crate::action::{Action, TreeAction};
        let key = crossterm::event::KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert_eq!(map_key_event(key), Some(Action::Tree(TreeAction::HalfPageDown)));
    }

    #[rstest]
    fn map_key_ctrl_u_to_half_page_up() {
        use crate::action::{Action, TreeAction};
        let key = crossterm::event::KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!(map_key_event(key), Some(Action::Tree(TreeAction::HalfPageUp)));
    }

    #[rstest]
    fn map_unknown_key_to_none() {
        let key = crossterm::event::KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE);
        assert_eq!(map_key_event(key), None);
    }
}
