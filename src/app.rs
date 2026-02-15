//! Application state machine and main event loop.

mod handler;
mod keymap;
mod state;

use std::collections::HashSet;
use std::path::{
    Path,
    PathBuf,
};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::Event;
use handler::{
    handle_key_event,
    refresh_directory,
    trigger_prefetch,
    trigger_preview,
};
pub use keymap::KeyMap;
use ratatui_image::picker::Picker;
pub use state::*;

use crate::cli::Args;
use crate::config::Config;
use crate::file_op::selection::SelectionBuffer;
use crate::file_op::undo::UndoHistory;
use crate::input::AppMode;
use crate::preview::cache::PreviewCache;
use crate::preview::provider::{
    PreviewProvider,
    PreviewRegistry,
};
use crate::preview::providers::external::ExternalCmdProvider;
use crate::preview::providers::fallback::FallbackProvider;
use crate::preview::providers::image::ImagePreviewProvider;
use crate::preview::providers::text::TextPreviewProvider;
use crate::preview::state::PreviewState;
use crate::session;
use crate::state::tree::TreeState;
use crate::tree::builder::TreeBuilder;
use crate::watcher::FsWatcher;

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
    let sort_order = config.sort.order.into();
    let sort_direction = config.sort.direction.into();
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
    // IMPORTANT: must run before init_theme() — init_theme sends an OSC 11 query
    // whose response would pollute stdin and corrupt Picker's font-size detection.
    let picker =
        Picker::from_query_stdio().unwrap_or_else(|_| ImagePreviewProvider::fallback_picker());

    // Detect terminal background for theme selection (must be before raw mode).
    // terminal_light::luma() sends OSC 11 query — safe only in cooked mode.
    crate::preview::highlight::init_theme();
    let mut providers: Vec<Arc<dyn PreviewProvider>> = vec![
        Arc::new(ImagePreviewProvider::new(picker)),
        Arc::new(TextPreviewProvider::new()),
        Arc::new(FallbackProvider::new()),
    ];
    for cmd in &config.preview.commands {
        providers
            .push(Arc::new(ExternalCmdProvider::new(cmd.clone(), config.preview.command_timeout)));
    }
    let preview_registry = PreviewRegistry::new(providers)?;

    let show_preview = config.display.show_preview;

    // Create watcher channel and instance.
    let (watcher_tx, mut watcher_rx) =
        tokio::sync::mpsc::unbounded_channel::<Vec<notify_debouncer_mini::DebouncedEvent>>();
    let (fs_watcher, suppressed) = if config.watcher.enabled {
        match FsWatcher::new(&config.watcher, watcher_tx) {
            Ok(mut w) => {
                let suppressed = w.suppressed();
                if let Err(e) = w.watch(&root_path) {
                    tracing::warn!(%e, "failed to watch root directory");
                }
                (Some(w), suppressed)
            }
            Err(e) => {
                tracing::warn!(%e, "failed to create file system watcher");
                (None, Arc::new(AtomicBool::new(false)))
            }
        }
    } else {
        drop(watcher_tx);
        (None, Arc::new(AtomicBool::new(false)))
    };

    // Restore session and clean up expired sessions.
    let (selection, undo_history) =
        restore_session_if_needed(args, &config, &root_path, &builder, &mut tree_state);

    // Create app state.
    let mut state = AppState {
        tree_state,
        preview_state: PreviewState::new(),
        preview_cache: PreviewCache::new(config.preview.cache_size),
        preview_registry,
        mode: AppMode::default(),
        selection,
        undo_history,
        watcher: fs_watcher,
        should_quit: false,
        show_icons: !args.no_icons,
        show_preview,
        show_hidden,
        show_ignored,
        viewport_height: 0,
        scroll: ScrollState::new(),
        status_message: None,
        processing: false,
    };

    // Set up async children load channel.
    let (children_tx, mut children_rx) = tokio::sync::mpsc::channel::<ChildrenLoadResult>(64);

    // Set up async preview load channel.
    let (preview_tx, mut preview_rx) = tokio::sync::mpsc::channel::<PreviewLoadResult>(16);

    // Create immutable runtime context.
    let ctx = AppContext {
        children_tx,
        preview_tx,
        preview_config: config.preview.clone(),
        file_op_config: config.file_operations.clone(),
        keymap: KeyMap::default(),
        suppressed,
    };

    // Prefetch root's child directories for instant first expansion.
    trigger_prefetch(
        &mut state.tree_state,
        &root_path,
        &ctx,
        state.show_hidden,
        state.show_ignored,
    );

    // Trigger initial preview for the currently selected file.
    trigger_preview(&mut state, &ctx);

    // Initialize terminal.
    let mut terminal = crate::terminal::init();

    // Track cursor for change detection.
    let mut last_cursor = state.tree_state.cursor();

    // Main event loop.
    loop {
        // Clear expired status messages.
        state.clear_expired_status();

        // Draw UI.
        terminal.draw(|frame| {
            crate::ui::render(frame, &mut state);
        })?;

        // Poll for events (50ms timeout for responsive async result handling).
        if crossterm::event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = crossterm::event::read()?
        {
            handle_key_event(key, &mut state, &ctx);
        }

        // Receive async children load results.
        while let Ok(result) = children_rx.try_recv() {
            match result.children {
                Ok(children) => {
                    let loaded_path = result.path.clone();
                    let is_prefetch = result.prefetch;
                    state.tree_state.set_children(&result.path, children, !is_prefetch);

                    // Prefetch child directories one level ahead (user-initiated loads only).
                    if !is_prefetch {
                        trigger_prefetch(
                            &mut state.tree_state,
                            &loaded_path,
                            &ctx,
                            state.show_hidden,
                            state.show_ignored,
                        );
                    }
                }
                Err(err) => {
                    tracing::warn!(?result.path, %err, "failed to load children");
                    state.tree_state.set_children_error(&result.path);
                }
            }
        }

        // Receive watcher events (file system changes in watched directories).
        process_watcher_events(&mut watcher_rx, &mut state, &ctx);

        // Receive async preview load results.
        while let Ok(result) = preview_rx.try_recv() {
            // Only apply if the path still matches current preview request.
            let is_current =
                state.preview_state.current_path.as_deref().is_some_and(|p| p == result.path);
            if is_current {
                state.preview_state.set_content(result.content);
            }
        }

        // Trigger preview when cursor changes.
        let current_cursor = state.tree_state.cursor();
        if current_cursor != last_cursor {
            last_cursor = current_cursor;
            if state.show_preview {
                trigger_preview(&mut state, &ctx);
            }
        }

        // Update scroll position.
        state.scroll.clamp_to_cursor(state.tree_state.cursor(), state.viewport_height as usize);

        if state.should_quit {
            break;
        }
    }

    // Save session state before exiting.
    let session_state = session::build_session_state(
        &root_path,
        state.tree_state.expanded_paths(),
        state.tree_state.cursor_path(),
        &state.selection,
        &state.undo_history,
    );
    if let Err(e) = session::save(&session_state) {
        tracing::warn!(%e, "failed to save session");
    }

    // Restore terminal.
    crate::terminal::restore();

    Ok(())
}

/// Restore session state and schedule expired session cleanup.
///
/// Returns the selection buffer and undo history (restored or fresh).
fn restore_session_if_needed(
    args: &Args,
    config: &Config,
    root_path: &Path,
    builder: &TreeBuilder,
    tree_state: &mut TreeState,
) -> (SelectionBuffer, UndoHistory) {
    let should_restore = if args.restore {
        true
    } else if args.no_restore {
        false
    } else {
        config.session.restore_by_default
    };

    let mut selection = SelectionBuffer::new();
    let mut undo_history = UndoHistory::new(config.file_operations.undo_stack_size);

    if should_restore {
        match session::restore(root_path) {
            Ok(Some(session_state)) => {
                // Restore expanded directories (sorted so parents load before children).
                let mut paths = session_state.expanded_paths;
                paths.sort_by_key(|p| p.as_os_str().len());
                for path in &paths {
                    if let Ok(children) = builder.load_children(path) {
                        tree_state.set_children(path, children, true);
                    }
                }

                // Restore cursor position.
                if let Some(ref cursor_path) = session_state.cursor_path {
                    tree_state.move_cursor_to_path(cursor_path);
                }

                // Restore selection buffer.
                selection = SelectionBuffer::from_parts(
                    session_state.selection_paths,
                    session_state.selection_mode,
                );

                // Restore undo history.
                undo_history = UndoHistory::from_stacks(
                    session_state.undo_stack,
                    session_state.redo_stack,
                    config.file_operations.undo_stack_size,
                );

                tracing::info!("session restored");
            }
            Ok(None) => {
                tracing::debug!("no session to restore");
            }
            Err(e) => {
                tracing::warn!(%e, "failed to restore session");
            }
        }
    }

    // Clean up expired sessions in background.
    let expiry_days = config.session.expiry_days;
    tokio::spawn(async move {
        match session::cleanup_expired(expiry_days) {
            Ok(0) => {}
            Ok(n) => tracing::info!(n, "cleaned up expired sessions"),
            Err(e) => tracing::warn!(%e, "failed to clean up expired sessions"),
        }
    });

    (selection, undo_history)
}

/// Process pending file system watcher events and refresh affected directories.
fn process_watcher_events(
    watcher_rx: &mut tokio::sync::mpsc::UnboundedReceiver<
        Vec<notify_debouncer_mini::DebouncedEvent>,
    >,
    state: &mut AppState,
    ctx: &AppContext,
) {
    while let Ok(events) = watcher_rx.try_recv() {
        let mut affected_dirs: HashSet<PathBuf> = HashSet::new();
        for event in &events {
            if let Some(parent) = event.path.parent() {
                affected_dirs.insert(parent.to_path_buf());
            }
        }
        let mut dirs_to_refresh: Vec<PathBuf> = Vec::new();
        for dir in &affected_dirs {
            if state.tree_state.handle_fs_change(dir) {
                dirs_to_refresh.push(dir.clone());
            }
        }
        for dir in &dirs_to_refresh {
            refresh_directory(state, dir, ctx);
        }
    }
}
