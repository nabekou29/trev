//! Application state machine and main event loop.

mod handler;
mod key_parse;
mod key_trie;
mod keymap;
mod pending_keys;
mod state;

use std::collections::HashSet;
use std::path::{
    Path,
    PathBuf,
};
use std::sync::atomic::AtomicBool;
use std::sync::{
    Arc,
    RwLock,
};
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
pub use pending_keys::PendingKeys;
use ratatui_image::picker::Picker;
pub use state::*;

use crate::cli::Args;
use crate::config::Config;
use crate::file_op::selection::SelectionBuffer;
use crate::file_op::undo::UndoHistory;
use crate::git::{
    GitState,
    GitStatusResult,
};
use crate::input::AppMode;
use crate::ipc::types::IpcCommand;
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
use crate::ui::column::{
    ColumnKind,
    resolve_columns,
};
use crate::watcher::FsWatcher;

/// Async event channel receivers consumed by the main event loop.
struct EventReceivers {
    /// Directory children load results.
    children: tokio::sync::mpsc::Receiver<ChildrenLoadResult>,
    /// Preview content load results.
    preview: tokio::sync::mpsc::Receiver<PreviewLoadResult>,
    /// Git status scan results.
    git: tokio::sync::mpsc::Receiver<GitStatusResult>,
    /// Tree rebuild results (from toggle hidden/ignored/refresh).
    rebuild: tokio::sync::mpsc::Receiver<TreeRebuildResult>,
    /// File system change events from the watcher.
    watcher: tokio::sync::mpsc::UnboundedReceiver<Vec<notify_debouncer_mini::DebouncedEvent>>,
    /// IPC commands from connected editors.
    ipc: tokio::sync::mpsc::UnboundedReceiver<IpcCommand>,
}

/// Initialize all application state, context, channels, and terminal.
///
/// Must be called before entering the event loop. Handles config loading,
/// tree construction, session restore, reveal, preview/watcher/IPC setup,
/// and terminal initialization with scroll pre-centering.
fn init_app(
    args: &Args,
) -> Result<(AppState, AppContext, EventReceivers, ratatui::DefaultTerminal)> {
    let _span = tracing::info_span!("init_app").entered();

    let (config, root_path) = load_config(args)?;

    let (show_hidden, show_ignored) = (config.display.show_hidden, config.display.show_ignored);
    let (builder, root) = {
        let _span = tracing::info_span!("tree_build").entered();
        let builder = TreeBuilder::new(show_hidden, show_ignored);
        let root = builder.build(&root_path)?;
        (builder, root)
    };

    // Create tree state with sort/display options.
    let tree_options = crate::state::tree::TreeOptions {
        sort_order: config.sort.order.into(),
        sort_direction: config.sort.direction.into(),
        directories_first: config.sort.directories_first,
        show_root: config.display.show_root_entry,
    };
    let mut tree_state = TreeState::new(root, tree_options);
    tree_state.apply_sort(
        tree_options.sort_order,
        tree_options.sort_direction,
        tree_options.directories_first,
    );

    let (git_state, preview_registry) = detect_terminal_and_build_registry(&config)?;
    let show_preview = config.display.show_preview;

    // Create watcher channel and instance.
    let (watcher_tx, watcher_rx) =
        tokio::sync::mpsc::unbounded_channel::<Vec<notify_debouncer_mini::DebouncedEvent>>();
    let (fs_watcher, suppressed) = setup_watcher(&config, watcher_tx, &root_path);

    // Start IPC server in daemon mode.
    let (ipc_server, ipc_rx) = start_ipc_if_daemon(args.daemon, &root_path);

    // Restore session and clean up expired sessions.
    let (selection, undo_history, session_restored) = {
        let _span = tracing::info_span!("session_restore").entered();
        restore_session_if_needed(args, &config, &root_path, builder, &mut tree_state)
    };

    // Reveal a specific path on startup (--reveal flag).
    let reveal_succeeded = {
        let _span = tracing::info_span!("reveal").entered();
        args.reveal.as_ref().is_some_and(|reveal| {
            let builder = TreeBuilder::new(show_hidden, show_ignored);
            let found = tree_state.reveal_path(reveal, builder);
            if !found {
                tracing::warn!(path = %reveal.display(), "failed to reveal path on startup");
            }
            found
        })
    };

    // Resolve display columns from config, filtering GitStatus when git is disabled.
    let git_enabled = config.git.enabled;
    let mut columns = resolve_columns(&config.display.columns, &config.display.column_options);
    if !git_enabled {
        columns.retain(|c| c.kind != ColumnKind::GitStatus);
    }

    // Create app state.
    let mut state = AppState {
        tree_state,
        preview_state: {
            let mut ps = PreviewState::new();
            ps.word_wrap = config.preview.word_wrap;
            ps
        },
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
        git_state: Arc::clone(&git_state),
        rebuild_generation: 0,
        columns,
        layout_split_ratio: config.preview.split_ratio,
        layout_narrow_split_ratio: config.preview.narrow_split_ratio,
        layout_narrow_width: config.preview.narrow_width,
        pending_keys: PendingKeys::new(Duration::from_millis(
            config.keybindings.key_sequence_timeout_ms,
        )),
        needs_redraw: false,
    };

    let (ctx, channels) = build_context_and_channels(
        &config,
        args,
        suppressed,
        ipc_server,
        git_enabled,
        &root_path,
        watcher_rx,
        ipc_rx,
    );

    trigger_initial_loads(&mut state, &ctx, &root_path);

    let terminal = {
        let _span = tracing::info_span!("terminal_setup").entered();
        setup_terminal(&mut state, session_restored || reveal_succeeded)
    };

    Ok((state, ctx, channels, terminal))
}

/// Build the immutable runtime context and async event channels.
#[allow(clippy::too_many_arguments)]
fn build_context_and_channels(
    config: &Config,
    args: &Args,
    suppressed: Arc<AtomicBool>,
    ipc_server: Option<Arc<crate::ipc::server::IpcServer>>,
    git_enabled: bool,
    root_path: &Path,
    watcher_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<notify_debouncer_mini::DebouncedEvent>>,
    ipc_rx: tokio::sync::mpsc::UnboundedReceiver<IpcCommand>,
) -> (AppContext, EventReceivers) {
    let (children_tx, children_rx) = tokio::sync::mpsc::channel::<ChildrenLoadResult>(64);
    let (preview_tx, preview_rx) = tokio::sync::mpsc::channel::<PreviewLoadResult>(16);
    let (git_tx, git_rx) = tokio::sync::mpsc::channel::<GitStatusResult>(4);
    let (rebuild_tx, rebuild_rx) = tokio::sync::mpsc::channel::<TreeRebuildResult>(2);

    let ctx = AppContext {
        children_tx,
        preview_tx,
        preview_config: config.preview.clone(),
        file_op_config: config.file_op,
        keymap: KeyMap::from_config(&config.keybindings),
        suppressed,
        ipc_server,
        editor_action: args.action.into(),
        git_tx,
        git_enabled,
        root_path: root_path.to_path_buf(),
        rebuild_tx,
        menus: config.menus.clone(),
    };

    let channels = EventReceivers {
        children: children_rx,
        preview: preview_rx,
        git: git_rx,
        rebuild: rebuild_rx,
        watcher: watcher_rx,
        ipc: ipc_rx,
    };

    (ctx, channels)
}

/// Initialize terminal and pre-compute viewport dimensions.
///
/// Auto-hides preview on narrow terminals and centers scroll
/// when restoring a session or revealing a path.
fn setup_terminal(state: &mut AppState, needs_center: bool) -> ratatui::DefaultTerminal {
    let terminal = crate::terminal::init();

    // Pre-compute viewport height and auto-hide preview on narrow terminals
    // before first draw, so session-restore / reveal can center without a flash.
    if let Ok((width, height)) = crossterm::terminal::size() {
        if width <= state.layout_narrow_width {
            state.show_preview = false;
        }
        state.viewport_height = height.saturating_sub(1) as usize;
    }

    // Center scroll before first draw to prevent flash on session restore.
    if needs_center {
        state.scroll.center_on_cursor(state.tree_state.cursor(), state.viewport_height);
    }

    terminal
}

/// Load configuration, apply CLI overrides, and canonicalize the root path.
fn load_config(args: &Args) -> Result<(Config, PathBuf)> {
    let _span = tracing::info_span!("config_load").entered();
    let load_result = Config::load()?;
    for warning in &load_result.warnings {
        tracing::warn!("{warning}");
    }
    let mut config = load_result.config;
    config.apply_cli_overrides(args);
    tracing::info!(?args, "starting trev");
    let root_path = std::fs::canonicalize(&args.path)?;
    Ok((config, root_path))
}

/// Detect terminal graphics protocol and theme, then build preview registry.
///
/// Must be called before entering raw mode. Picker detection must happen before
/// theme detection (OSC 11 query would corrupt Picker's font-size detection).
fn detect_terminal_and_build_registry(
    config: &Config,
) -> Result<(Arc<RwLock<Option<GitState>>>, PreviewRegistry)> {
    let picker =
        Picker::from_query_stdio().unwrap_or_else(|_| ImagePreviewProvider::fallback_picker());
    crate::preview::highlight::init_theme();
    let git_state: Arc<RwLock<Option<GitState>>> = Arc::new(RwLock::new(None));
    let preview_registry = build_preview_registry(picker, config, &git_state)?;
    Ok((git_state, preview_registry))
}

/// Trigger initial async operations: prefetch, git status, preview.
fn trigger_initial_loads(state: &mut AppState, ctx: &AppContext, root_path: &Path) {
    trigger_prefetch(&mut state.tree_state, root_path, ctx, state.show_hidden, state.show_ignored);
    if ctx.git_enabled {
        trigger_git_status(ctx);
    }
    trigger_preview(state, ctx);
}

/// Process all async channel results: children loads, watcher, IPC, git, rebuild, preview.
///
/// Returns the updated `last_cursor` value (may change when preview is triggered).
fn process_async_events(
    state: &mut AppState,
    ctx: &AppContext,
    channels: &mut EventReceivers,
    mut last_cursor: usize,
) -> usize {
    // Receive async children load results.
    {
        let _span = tracing::info_span!("process_children").entered();
        while let Ok(result) = channels.children.try_recv() {
            match result.children {
                Ok(children) => {
                    let loaded_path = result.path.clone();
                    let is_prefetch = result.prefetch;
                    {
                        let _span = tracing::info_span!(
                            "set_children",
                            path = %result.path.display(),
                            child_count = children.len(),
                            is_prefetch,
                        )
                        .entered();
                        state.tree_state.set_children(&result.path, children, !is_prefetch);
                    }

                    // Prefetch child directories one level ahead (user-initiated loads only).
                    if !is_prefetch {
                        let _span = tracing::info_span!("trigger_prefetch_in_process").entered();
                        trigger_prefetch(
                            &mut state.tree_state,
                            &loaded_path,
                            ctx,
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
    }

    // Receive watcher events (file system changes in watched directories).
    {
        let _span = tracing::info_span!("process_watcher").entered();
        process_watcher_events(&mut channels.watcher, state, ctx);
    }

    // Receive IPC commands.
    while let Ok(cmd) = channels.ipc.try_recv() {
        handle_ipc_command(cmd, state);
    }

    // Receive async git status results.
    while let Ok(result) = channels.git.try_recv() {
        if let Ok(mut guard) = state.git_state.write() {
            *guard = result.state;
        }
        tracing::debug!("git status updated");
    }

    // Receive async tree rebuild results.
    process_rebuild_results(&mut channels.rebuild, state, ctx);

    // Receive async preview load results.
    {
        let _span = tracing::info_span!("process_preview").entered();
        process_preview_results(&mut channels.preview, state);
    }

    // Trigger preview when cursor changes.
    let current_cursor = state.tree_state.cursor();
    if current_cursor != last_cursor {
        last_cursor = current_cursor;
        if state.show_preview {
            trigger_preview(state, ctx);
        }
    }

    last_cursor
}

/// Run the application.
#[expect(clippy::unused_async, reason = "Called from tokio async context")]
pub async fn run(args: &Args) -> Result<()> {
    let (mut state, ctx, mut channels, mut terminal) = init_app(args)?;
    let root_path = ctx.root_path.clone();
    let mut last_cursor = state.tree_state.cursor();

    // Initial draw before entering the event loop.
    terminal.draw(|frame| {
        crate::ui::render(frame, &mut state);
    })?;

    // Main event loop.
    //
    // Order: poll → drain key events → process async results → preview → scroll → draw.
    // When keys are pending (multi-key sequence), the poll timeout is shortened
    // to the remaining sequence timeout so fallback actions fire promptly.
    loop {
        let _frame_span = tracing::info_span!("frame").entered();

        // Clear expired status messages.
        state.clear_expired_status();

        // Check pending key timeout before polling.
        if state.pending_keys.is_pending() && state.pending_keys.is_timed_out() {
            handler::handle_pending_timeout(&mut state, &ctx);
        }

        // Poll timeout: shorter when keys are pending so timeout fires quickly.
        let poll_duration = if state.pending_keys.is_pending() {
            state.pending_keys.remaining().min(Duration::from_millis(50))
        } else {
            Duration::from_millis(50)
        };

        // Drain key events. Break early if a key enters pending state
        // (need to render the pending indicator and wait for timeout).
        if crossterm::event::poll(poll_duration)? {
            let _span = tracing::info_span!("key_event").entered();
            loop {
                if let Event::Key(key) = crossterm::event::read()? {
                    handle_key_event(key, &mut state, &ctx);
                    if state.pending_keys.is_pending() {
                        break;
                    }
                }
                if !crossterm::event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }

        // Process all async channel results.
        last_cursor = process_async_events(&mut state, &ctx, &mut channels, last_cursor);

        // Update scroll position.
        state.scroll.clamp_to_cursor(state.tree_state.cursor(), state.viewport_height);

        if state.should_quit {
            break;
        }

        // Force full redraw after shell command execution (alternate screen re-enter).
        if state.needs_redraw {
            state.needs_redraw = false;
            terminal.clear()?;
        }

        // Draw UI (once per frame, after all events are processed).
        {
            let _span = tracing::info_span!("draw").entered();
            terminal.draw(|frame| {
                crate::ui::render(frame, &mut state);
            })?;
        }
    }

    // Shutdown: save session, restore terminal, emit paths.
    shutdown(&root_path, &state, args);

    Ok(())
}

/// Create the file system watcher and suppression flag.
///
/// Returns `(watcher, suppressed)`. If the watcher is disabled or fails
/// to initialise, the channel sender is dropped and a dummy flag is returned.
fn setup_watcher(
    config: &Config,
    watcher_tx: tokio::sync::mpsc::UnboundedSender<Vec<notify_debouncer_mini::DebouncedEvent>>,
    root_path: &Path,
) -> (Option<FsWatcher>, Arc<AtomicBool>) {
    if !config.watcher.enabled {
        drop(watcher_tx);
        return (None, Arc::new(AtomicBool::new(false)));
    }
    match FsWatcher::new(&config.watcher, watcher_tx) {
        Ok(mut w) => {
            let suppressed = w.suppressed();
            if let Err(e) = w.watch_root(root_path) {
                tracing::warn!(%e, "failed to watch root directory");
            }
            (Some(w), suppressed)
        }
        Err(e) => {
            tracing::warn!(%e, "failed to create file system watcher");
            (None, Arc::new(AtomicBool::new(false)))
        }
    }
}

/// Build the preview provider registry from config.
fn build_preview_registry(
    picker: Picker,
    config: &Config,
    git_state: &Arc<RwLock<Option<GitState>>>,
) -> Result<PreviewRegistry> {
    let mut providers: Vec<Arc<dyn PreviewProvider>> = vec![
        Arc::new(ImagePreviewProvider::new(picker)),
        Arc::new(TextPreviewProvider::new()),
        Arc::new(FallbackProvider::new()),
    ];
    for cmd in &config.preview.commands {
        providers.push(Arc::new(ExternalCmdProvider::new(
            cmd.clone(),
            config.preview.command_timeout,
            Arc::clone(git_state),
        )));
    }
    PreviewRegistry::new(providers)
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
    let mut undo_history = UndoHistory::new(config.file_op.undo_stack_size);
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
                    config.file_op.undo_stack_size,
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
#[expect(clippy::print_stdout, reason = "CLI output to stdout is intentional")]
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
            let json: Vec<&str> = paths.iter().filter_map(|p| p.to_str()).collect();
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
) -> (Option<Arc<crate::ipc::server::IpcServer>>, tokio::sync::mpsc::UnboundedReceiver<IpcCommand>)
{
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

/// Process async tree rebuild results: swap tree and re-trigger prefetch/preview.
///
/// Discards stale results from superseded rebuilds using the generation counter.
fn process_rebuild_results(
    rebuild_rx: &mut tokio::sync::mpsc::Receiver<TreeRebuildResult>,
    state: &mut AppState,
    ctx: &AppContext,
) {
    while let Ok(result) = rebuild_rx.try_recv() {
        if result.generation != state.rebuild_generation {
            tracing::debug!(
                expected = state.rebuild_generation,
                got = result.generation,
                "discarding stale rebuild result",
            );
            continue;
        }
        state.tree_state = result.tree_state;
        // Restore scroll so the cursor stays at the same visual row.
        // Clamp offset so we don't scroll past the end of the tree
        // (which would cause blank space at the bottom of the viewport).
        let new_cursor = state.tree_state.cursor();
        let total = state.tree_state.visible_node_count();
        let max_offset = total.saturating_sub(state.viewport_height);
        let desired = new_cursor.saturating_sub(result.visual_row);
        state.scroll.set_offset(desired.min(max_offset));
        trigger_prefetch(
            &mut state.tree_state,
            &result.root_path,
            ctx,
            result.show_hidden,
            result.show_ignored,
        );
        if state.show_preview {
            trigger_preview(state, ctx);
        }
        tracing::info!("tree rebuild applied");
    }
}

/// Trigger an async git status fetch in the background.
///
/// Runs `git status --porcelain=v1` via `spawn_blocking` and sends the
/// result through the `git_tx` channel.
pub fn trigger_git_status(ctx: &AppContext) {
    let tx = ctx.git_tx.clone();
    let root = ctx.root_path.clone();
    tokio::task::spawn_blocking(move || {
        let result = fetch_git_status(&root);
        // Channel may be closed during shutdown — ignore errors.
        let _ = tx.blocking_send(result);
    });
}

/// Synchronously run `git status --porcelain=v1` and parse the output.
fn fetch_git_status(root: &Path) -> GitStatusResult {
    let _span = tracing::info_span!("fetch_git_status", root_path = %root.display()).entered();
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain=v1"])
        .current_dir(root)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let state = GitState::from_porcelain(&stdout, root);
            GitStatusResult { state: Some(state) }
        }
        Ok(out) => {
            tracing::debug!(
                stderr = %String::from_utf8_lossy(&out.stderr).trim(),
                "git status returned non-zero exit code"
            );
            GitStatusResult { state: None }
        }
        Err(e) => {
            tracing::debug!(%e, "git command not available");
            GitStatusResult { state: None }
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

        let affected_dirs: HashSet<PathBuf> =
            events.iter().filter_map(|event| event.path.parent().map(Path::to_path_buf)).collect();

        for dir in &affected_dirs {
            if state.tree_state.handle_fs_change(dir) {
                refresh_directory(state, dir, ctx);
            }
        }

        // Re-fetch git status when files change.
        if ctx.git_enabled {
            trigger_git_status(ctx);
        }
    }
}
