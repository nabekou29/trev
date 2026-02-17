//! Application state machine and main event loop.

mod handler;
mod key_parse;
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
    handle_ipc_command,
    handle_key_event,
    refresh_directory,
    trigger_prefetch,
    trigger_preview,
};
pub use keymap::{
    KeyContext,
    KeyMap,
};
use ratatui_image::picker::Picker;
pub use state::*;

use crate::cli::Args;
use crate::config::Config;
use crate::ipc::types::IpcCommand;
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

    // Start IPC server in daemon mode.
    let (ipc_server, mut ipc_rx) = start_ipc_if_daemon(args.daemon, &root_path);

    // Restore session and clean up expired sessions.
    let (selection, undo_history, session_restored) =
        restore_session_if_needed(args, &config, &root_path, builder, &mut tree_state);

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
        emit_paths: if args.emit { Some(Vec::new()) } else { None },
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
        file_op_config: config.file_operations,
        keymap: KeyMap::from_config(&config.keybindings),
        suppressed,
        ipc_server,
        editor_action: args.action.into(),
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

    // Auto-hide preview on narrow terminals before first draw.
    if let Ok((width, _)) = crossterm::terminal::size()
        && width <= crate::ui::NARROW_WIDTH_THRESHOLD
    {
        state.show_preview = false;
    }

    // Track cursor for change detection.
    let mut last_cursor = state.tree_state.cursor();
    let mut needs_center_scroll = session_restored;

    // Main event loop.
    loop {
        // Clear expired status messages.
        state.clear_expired_status();

        // Draw UI.
        terminal.draw(|frame| {
            crate::ui::render(frame, &mut state);
        })?;

        // Center scroll on cursor after first draw (viewport_height is now known).
        if needs_center_scroll {
            needs_center_scroll = false;
            state
                .scroll
                .center_on_cursor(state.tree_state.cursor(), state.viewport_height as usize);
        }

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

        // Receive IPC commands.
        while let Ok(cmd) = ipc_rx.try_recv() {
            handle_ipc_command(cmd, &mut state);
        }

        // Receive async preview load results.
        process_preview_results(&mut preview_rx, &mut state);

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

    // Shutdown: save session, restore terminal, emit paths.
    shutdown(&root_path, &state, args);

    Ok(())
}

/// Restore session state and schedule expired session cleanup.
///
/// Returns `(selection, undo_history, restored)` where `restored` is true
/// when a session was successfully restored (caller should center scroll).
fn restore_session_if_needed(
    args: &Args,
    config: &Config,
    root_path: &Path,
    builder: TreeBuilder,
    tree_state: &mut TreeState,
) -> (SelectionBuffer, UndoHistory, bool) {
    let should_restore = if args.restore {
        true
    } else if args.no_restore {
        false
    } else {
        config.session.restore_by_default
    };

    let mut selection = SelectionBuffer::new();
    let mut undo_history = UndoHistory::new(config.file_operations.undo_stack_size);
    let mut restored = false;

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

                restored = true;
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

    (selection, undo_history, restored)
}

/// Save session, restore terminal, and output emit paths.
fn shutdown(root_path: &Path, state: &AppState, args: &Args) {
    // Save session state.
    let session_state = session::build_session_state(
        root_path,
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

    // Output emit paths after terminal restore (stdout is now safe).
    if let Some(paths) = &state.emit_paths {
        emit_paths(paths, args.emit_format);
    }
}

/// Output accumulated emit paths to stdout.
///
/// Format depends on the `--emit-format` flag:
/// - `Lines`: one path per line
/// - `Nul`: null-separated paths
/// - `Json`: JSON array of strings
#[allow(clippy::print_stdout)]
fn emit_paths(paths: &[PathBuf], format: crate::cli::EmitFormat) {
    use crate::cli::EmitFormat;

    if paths.is_empty() {
        return;
    }
    match format {
        EmitFormat::Lines => {
            for path in paths {
                println!("{}", path.display());
            }
        }
        EmitFormat::Nul => {
            for (i, path) in paths.iter().enumerate() {
                if i > 0 {
                    print!("\0");
                }
                print!("{}", path.display());
            }
        }
        EmitFormat::Json => {
            let json: Vec<&str> = paths
                .iter()
                .filter_map(|p| p.to_str())
                .collect();
            if let Ok(output) = serde_json::to_string(&json) {
                println!("{output}");
            }
        }
    }
}

/// Start IPC server when running in daemon mode.
///
/// Returns the server handle (kept alive for the process lifetime) and
/// the receiver end of the command channel.
fn start_ipc_if_daemon(
    daemon: bool,
    root_path: &Path,
) -> (
    Option<Arc<crate::ipc::server::IpcServer>>,
    tokio::sync::mpsc::UnboundedReceiver<IpcCommand>,
) {
    let (ipc_tx, ipc_rx) = tokio::sync::mpsc::unbounded_channel::<IpcCommand>();
    let server = if daemon {
        // Clean up stale sockets from crashed processes in the background.
        tokio::spawn(async {
            let removed = crate::ipc::paths::cleanup_stale_sockets();
            if removed > 0 {
                tracing::info!(removed, "cleaned up stale sockets");
            }
        });

        let socket_path = crate::ipc::paths::socket_path(root_path);
        match crate::ipc::server::IpcServer::start_on_path(socket_path.clone(), ipc_tx) {
            Ok(s) => {
                if let Err(e) = crate::ipc::paths::write_meta(&socket_path, root_path) {
                    tracing::warn!(%e, "failed to write IPC metadata");
                }
                Some(s)
            }
            Err(e) => {
                tracing::warn!(%e, "failed to start IPC server");
                None
            }
        }
    } else {
        drop(ipc_tx);
        None
    };
    (server, ipc_rx)
}

/// Process async preview load results: cache and display.
fn process_preview_results(
    preview_rx: &mut tokio::sync::mpsc::Receiver<PreviewLoadResult>,
    state: &mut AppState,
) {
    while let Ok(result) = preview_rx.try_recv() {
        let is_prefetch = result.prefetch;
        // Store in cache (skip non-cloneable Image content).
        if let Some(cloned) = result.content.try_clone() {
            let key = crate::preview::cache::CacheKey {
                path: result.path.clone(),
                provider_name: result.provider_name.clone(),
            };
            tracing::debug!(
                path = %result.path.display(),
                provider = %result.provider_name,
                prefetch = is_prefetch,
                "preview result cached",
            );
            state.preview_cache.put(key, cloned);
        }

        // Prefetch results are cache-only — don't update display.
        if is_prefetch {
            continue;
        }

        // Only apply if the path still matches current preview request.
        let is_current =
            state.preview_state.current_path.as_deref().is_some_and(|p| p == result.path);
        if is_current {
            state.preview_state.set_content(result.content);
        }
    }
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
        // Invalidate preview cache for changed files.
        for event in &events {
            tracing::debug!(path = %event.path.display(), "preview cache invalidated (fs change)");
            state.preview_cache.invalidate_path(&event.path);
        }

        let affected_dirs: HashSet<PathBuf> = events
            .iter()
            .filter_map(|event| event.path.parent().map(Path::to_path_buf))
            .collect();

        for dir in &affected_dirs {
            if state.tree_state.handle_fs_change(dir) {
                refresh_directory(state, dir, ctx);
            }
        }
    }
}
