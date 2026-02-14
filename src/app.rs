//! Application state machine and main event loop.

use std::path::{
    Path,
    PathBuf,
};
use std::sync::{
    Arc,
    Mutex,
};
use std::time::Duration;

use anyhow::Result;
use ratatui_image::picker::Picker;
use crossterm::event::{
    Event,
    KeyCode,
    KeyModifiers,
};

use crate::cli::Args;
use crate::config::Config;
use crate::preview::cache::PreviewCache;
use crate::preview::content::PreviewContent;
use crate::preview::provider::{
    LoadContext,
    PreviewProvider,
    PreviewRegistry,
};
use crate::preview::providers::external::ExternalCmdProvider;
use crate::preview::providers::fallback::FallbackProvider;
use crate::preview::providers::image::ImagePreviewProvider;
use crate::preview::providers::text::TextPreviewProvider;
use crate::preview::state::PreviewState;
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
    /// Preview display state.
    pub preview_state: PreviewState,
    /// Preview content cache (LRU).
    #[expect(dead_code, reason = "Cache integration pending")]
    pub preview_cache: PreviewCache,
    /// Preview provider registry.
    pub preview_registry: PreviewRegistry,
    /// Whether the application should quit.
    pub should_quit: bool,
    /// Whether to show file icons (Nerd Fonts).
    pub show_icons: bool,
    /// Whether the preview panel is visible.
    pub show_preview: bool,
    /// Current viewport height (tree area rows).
    pub viewport_height: u16,
    /// Scroll state for the tree view.
    pub scroll: ScrollState,
    /// Shared image picker (terminal graphics protocol).
    pub image_picker: Arc<Mutex<Picker>>,
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

/// Result of an async preview load operation.
pub struct PreviewLoadResult {
    /// Path of the file that was previewed.
    pub path: PathBuf,
    /// Provider name that produced this content.
    #[expect(dead_code, reason = "Used for cache key integration")]
    pub provider_name: String,
    /// Loaded preview content.
    pub content: PreviewContent,
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
#[allow(clippy::unused_async, clippy::too_many_lines)]
pub async fn run(args: &Args) -> Result<()> {
    let load_result = Config::load()?;
    for warning in &load_result.warnings {
        tracing::warn!("{warning}");
    }
    let mut config = load_result.config;
    config.apply_cli_overrides(args);
    tracing::info!(?args, "starting trev");

    // Resolve the root path.
    let root_path = std::fs::canonicalize(&args.path)?;

    // Map config sort settings to tree state types.
    let sort_order = map_sort_order(config.sort.order);
    let sort_direction = map_sort_direction(config.sort.direction);
    let directories_first = config.sort.directories_first;

    // Build the initial tree (depth 1).
    let show_hidden = config.display.show_hidden;
    let show_ignored = config.display.show_ignored;
    let builder = TreeBuilder::new(show_hidden, show_ignored);
    let root = builder.build(&root_path)?;

    // Create tree state with sort settings.
    let mut tree_state = TreeState::new(root, sort_order, sort_direction, directories_first);
    tree_state.apply_sort(sort_order, sort_direction, directories_first);

    // Detect terminal graphics protocol (must be before terminal init/raw mode).
    // Falls back to halfblocks if detection fails.
    let picker = Picker::from_query_stdio()
        .unwrap_or_else(|_| ImagePreviewProvider::fallback_picker());
    let image_picker = Arc::new(Mutex::new(picker.clone()));
    let mut providers: Vec<Box<dyn PreviewProvider>> = vec![
        Box::new(ImagePreviewProvider::new(picker)),
        Box::new(TextPreviewProvider::new()),
        Box::new(FallbackProvider::new()),
    ];
    if !config.preview.commands.is_empty() {
        providers.push(Box::new(ExternalCmdProvider::new(
            config.preview.commands.clone(),
            config.preview.command_timeout,
        )));
    }
    let preview_registry = PreviewRegistry::new(providers);

    let show_preview = config.display.show_preview;

    // Create app state.
    let mut state = AppState {
        tree_state,
        preview_state: PreviewState::new(),
        preview_cache: PreviewCache::new(config.preview.cache_size),
        preview_registry,
        should_quit: false,
        show_icons: !args.no_icons,
        show_preview,
        viewport_height: 0,
        scroll: ScrollState::new(),
        image_picker,
    };

    // Set up async children load channel.
    let (children_tx, mut children_rx) =
        tokio::sync::mpsc::channel::<ChildrenLoadResult>(64);

    // Set up async preview load channel.
    let (preview_tx, mut preview_rx) =
        tokio::sync::mpsc::channel::<PreviewLoadResult>(16);

    // Prefetch root's child directories for instant first expansion.
    trigger_prefetch(
        &mut state.tree_state,
        &root_path,
        &children_tx,
        show_hidden,
        show_ignored,
    );

    // Trigger initial preview for the currently selected file.
    trigger_preview(&mut state, &preview_tx, &config.preview);

    // Initialize terminal.
    let mut terminal = crate::terminal::init();

    // Track cursor for change detection.
    let mut last_cursor = state.tree_state.cursor();

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
            handle_key_event(key, &mut state, &children_tx, &preview_tx, &config.preview, show_hidden, show_ignored);
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

        // Receive async preview load results.
        while let Ok(result) = preview_rx.try_recv() {
            // Only apply if the path still matches current preview request.
            let is_current = state
                .preview_state
                .current_path
                .as_deref()
                .is_some_and(|p| p == result.path);
            if is_current {
                state.preview_state.set_content(result.content);
            }
        }

        // Trigger preview when cursor changes.
        let current_cursor = state.tree_state.cursor();
        if current_cursor != last_cursor {
            last_cursor = current_cursor;
            if state.show_preview {
                trigger_preview(&mut state, &preview_tx, &config.preview);
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
    preview_tx: &tokio::sync::mpsc::Sender<PreviewLoadResult>,
    preview_config: &crate::config::PreviewConfig,
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
        crate::action::Action::Preview(ref preview_action) => {
            handle_preview_action(preview_action, state, preview_tx, preview_config);
        }
    }
}

/// Map a key event to an action.
const fn map_key_event(key: crossterm::event::KeyEvent) -> Option<crate::action::Action> {
    use crate::action::{
        Action,
        TreeAction,
    };

    use crate::action::PreviewAction;

    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), KeyModifiers::NONE) => Some(Action::Quit),
        // Tree navigation (vim-style).
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
        // Preview scroll (Shift + vim-style).
        (KeyCode::Char('J'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
            Some(Action::Preview(PreviewAction::ScrollDown))
        }
        (KeyCode::Char('K'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
            Some(Action::Preview(PreviewAction::ScrollUp))
        }
        (KeyCode::Char('L'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
            Some(Action::Preview(PreviewAction::ScrollRight))
        }
        (KeyCode::Char('H'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
            Some(Action::Preview(PreviewAction::ScrollLeft))
        }
        (KeyCode::Char('D'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
            Some(Action::Preview(PreviewAction::HalfPageDown))
        }
        (KeyCode::Char('U'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
            Some(Action::Preview(PreviewAction::HalfPageUp))
        }
        // Provider cycling (Tab).
        (KeyCode::Tab, KeyModifiers::NONE) => {
            Some(Action::Preview(PreviewAction::CycleProvider))
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

/// Handle a preview action.
fn handle_preview_action(
    action: &crate::action::PreviewAction,
    state: &mut AppState,
    preview_tx: &tokio::sync::mpsc::Sender<PreviewLoadResult>,
    preview_config: &crate::config::PreviewConfig,
) {
    use crate::action::PreviewAction;

    let viewport_height = state.viewport_height as usize;

    match *action {
        PreviewAction::ScrollDown => {
            state.preview_state.scroll_down(1, viewport_height);
        }
        PreviewAction::ScrollUp => {
            state.preview_state.scroll_up(1);
        }
        PreviewAction::ScrollRight => {
            state.preview_state.scroll_right(1);
        }
        PreviewAction::ScrollLeft => {
            state.preview_state.scroll_left(1);
        }
        PreviewAction::HalfPageDown => {
            state
                .preview_state
                .scroll_down(viewport_height / 2, viewport_height);
        }
        PreviewAction::HalfPageUp => {
            state.preview_state.scroll_up(viewport_height / 2);
        }
        PreviewAction::CycleProvider => {
            if state.preview_state.cycle_provider() {
                reload_preview(state, preview_tx, preview_config);
            }
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

/// Trigger preview for a new file (cursor change).
///
/// Resets provider index, resolves providers, and spawns async load.
fn trigger_preview(
    state: &mut AppState,
    preview_tx: &tokio::sync::mpsc::Sender<PreviewLoadResult>,
    preview_config: &crate::config::PreviewConfig,
) {
    let Some((path, providers)) = resolve_preview_providers(state) else {
        return;
    };
    state.preview_state.request_preview(path.clone());
    spawn_preview_for(state, &path, &providers, preview_tx, preview_config);
}

/// Reload the current file with the (already-cycled) provider.
///
/// Preserves the provider index set by `cycle_provider()`.
fn reload_preview(
    state: &mut AppState,
    preview_tx: &tokio::sync::mpsc::Sender<PreviewLoadResult>,
    preview_config: &crate::config::PreviewConfig,
) {
    let Some((path, providers)) = resolve_preview_providers(state) else {
        return;
    };
    state.preview_state.reload_preview(path.clone());
    spawn_preview_for(state, &path, &providers, preview_tx, preview_config);
}

/// Resolve available providers for the current node.
///
/// Returns the path and provider name list, or `None` if no providers match.
fn resolve_preview_providers(state: &mut AppState) -> Option<(PathBuf, Vec<String>)> {
    let node_info = state.tree_state.current_node_info()?;
    let path = node_info.path.clone();
    let is_dir = node_info.is_dir;

    let providers = state.preview_registry.resolve(&path, is_dir);
    if providers.is_empty() {
        state.preview_state.set_content(PreviewContent::Empty);
        return None;
    }

    let provider_names: Vec<String> = providers.iter().map(|p| p.name().to_string()).collect();
    state.preview_state.set_available_providers(provider_names.clone());

    Some((path, provider_names))
}

/// Pick the active provider and spawn the async load task.
fn spawn_preview_for(
    state: &AppState,
    path: &Path,
    providers: &[String],
    preview_tx: &tokio::sync::mpsc::Sender<PreviewLoadResult>,
    preview_config: &crate::config::PreviewConfig,
) {
    let active_index = state.preview_state.active_provider_index;
    let Some(provider_name) = providers.get(active_index) else {
        return;
    };
    let provider_name = provider_name.clone();

    spawn_preview_load(PreviewLoadParams {
        tx: preview_tx.clone(),
        path: path.to_path_buf(),
        provider_name,
        max_lines: preview_config.max_lines,
        max_bytes: preview_config.max_bytes,
        cancel_token: state.preview_state.cancel_token.clone(),
        commands: preview_config.commands.clone(),
        command_timeout: preview_config.command_timeout,
        image_picker: Arc::clone(&state.image_picker),
    });
}

/// Parameters for spawning a preview load task.
struct PreviewLoadParams {
    tx: tokio::sync::mpsc::Sender<PreviewLoadResult>,
    path: PathBuf,
    provider_name: String,
    max_lines: usize,
    max_bytes: u64,
    cancel_token: tokio_util::sync::CancellationToken,
    commands: Vec<crate::config::ExternalCommand>,
    command_timeout: u64,
    image_picker: Arc<Mutex<Picker>>,
}

/// Spawn a blocking task to load preview content.
fn spawn_preview_load(params: PreviewLoadParams) {
    let PreviewLoadParams {
        tx,
        path,
        provider_name,
        max_lines,
        max_bytes,
        cancel_token,
        commands,
        command_timeout,
        image_picker,
    } = params;
    tokio::spawn(async move {
        let load_path = path.clone();
        let pname = provider_name.clone();
        let token = cancel_token.clone();

        let result = tokio::task::spawn_blocking(move || {
            let ctx = LoadContext {
                max_lines,
                max_bytes,
                cancel_token: token,
            };

            // Create the appropriate provider and load.
            let content = match pname.as_str() {
                "External" => {
                    ExternalCmdProvider::new(commands, command_timeout).load(&load_path, &ctx)
                }
                "Image" => {
                    ImagePreviewProvider::load_with_picker(&load_path, &ctx, &image_picker)
                }
                "Text" => TextPreviewProvider::new().load(&load_path, &ctx),
                "Fallback" => FallbackProvider::new().load(&load_path, &ctx),
                _ => Ok(PreviewContent::Error {
                    message: format!("Unknown provider: {pname}"),
                }),
            };

            match content {
                Ok(c) => c,
                Err(e) => PreviewContent::Error {
                    message: e.to_string(),
                },
            }
        })
        .await;

        let content = match result {
            Ok(content) => content,
            Err(e) => PreviewContent::Error {
                message: e.to_string(),
            },
        };

        // Ignore send error (receiver dropped = app shutting down).
        let _ = tx
            .send(PreviewLoadResult {
                path,
                provider_name,
                content,
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
