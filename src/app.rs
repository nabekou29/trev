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
use std::time::{
    Duration,
    Instant,
};

use anyhow::{
    Context as _,
    Result,
};
use crossterm::event::{
    Event,
    EventStream,
};
use futures_util::StreamExt as _;
use handler::{
    handle_ipc_command,
    handle_key_event,
    handle_mouse_event,
    refresh_directory,
    spawn_load_children,
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
use crate::state::tree::{
    ChildrenState,
    TreeState,
};
use crate::tree::builder::TreeBuilder;
use crate::tree::search_index::{
    self,
    DEFAULT_MAX_ENTRIES,
    SearchIndex,
};
use crate::ui::column::{
    ColumnKind,
    resolve_columns,
};
use crate::watcher::FsWatcher;

/// Async event channel receivers consumed by the main event loop.
///
/// Each channel has a corresponding `pre_*` field that holds the single
/// item consumed by `tokio::select!` when it wakes on that branch.
/// The `drain_*` methods yield the pre-received item (if any) followed
/// by all remaining items via `try_recv`.
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

    // Pre-received items from `tokio::select!` (one per wake).
    /// Pre-received children load result.
    pre_children: Option<ChildrenLoadResult>,
    /// Pre-received preview load result.
    pre_preview: Option<PreviewLoadResult>,
    /// Pre-received git status result.
    pre_git: Option<GitStatusResult>,
    /// Pre-received tree rebuild result.
    pre_rebuild: Option<TreeRebuildResult>,
    /// Pre-received watcher events.
    pre_watcher: Option<Vec<notify_debouncer_mini::DebouncedEvent>>,
    /// Pre-received IPC command.
    pre_ipc: Option<IpcCommand>,
}

/// Generate a drain method that yields `pre_$field.take()` then `$field.try_recv()` items.
macro_rules! impl_drain {
    ($method:ident, $pre:ident, $rx:ident, $T:ty) => {
        /// Drain this channel: pre-received item (if any) + all pending `try_recv` items.
        fn $method(&mut self) -> impl Iterator<Item = $T> + use<'_> {
            self.$pre.take().into_iter().chain(std::iter::from_fn(|| self.$rx.try_recv().ok()))
        }
    };
}

impl EventReceivers {
    /// Clear all pre-received items, preparing for the next `tokio::select!` cycle.
    fn clear_pre(&mut self) {
        self.pre_children = None;
        self.pre_preview = None;
        self.pre_git = None;
        self.pre_rebuild = None;
        self.pre_watcher = None;
        self.pre_ipc = None;
    }

    impl_drain!(drain_children, pre_children, children, ChildrenLoadResult);
    impl_drain!(drain_preview, pre_preview, preview, PreviewLoadResult);
    impl_drain!(drain_git, pre_git, git, GitStatusResult);
    impl_drain!(drain_rebuild, pre_rebuild, rebuild, TreeRebuildResult);
    impl_drain!(drain_watcher, pre_watcher, watcher, Vec<notify_debouncer_mini::DebouncedEvent>);
    impl_drain!(drain_ipc, pre_ipc, ipc, IpcCommand);
}

/// Initialize all application state, context, channels, and terminal.
///
/// Must be called before entering the event loop. Handles config loading,
/// tree construction, session restore, reveal, preview/watcher/IPC setup,
/// and terminal initialization with scroll pre-centering.
#[expect(clippy::too_many_lines, reason = "init_app is a single sequential setup flow")]
fn init_app(
    args: &Args,
) -> Result<(AppState, AppContext, EventReceivers, ratatui::DefaultTerminal)> {
    let _span = tracing::info_span!("init_app").entered();

    let (config, root_path) = load_config(args)?;

    // --- Phase 1: early session load to resolve display/sort overrides ---
    let (session_state, overrides) = {
        let _span = tracing::info_span!("session_overrides").entered();
        load_session_overrides(args, &config, &root_path)
    };

    let show_hidden = overrides.show_hidden;
    let show_ignored = overrides.show_ignored;
    let show_preview = overrides.show_preview;

    let (builder, root) = {
        let _span = tracing::info_span!("tree_build").entered();
        let builder = TreeBuilder::new(show_hidden, show_ignored);
        let root = builder.build(&root_path)?;
        (builder, root)
    };

    // Create tree state with sort/display options (using session overrides).
    let tree_options = crate::state::tree::TreeOptions {
        sort_order: overrides.sort_order,
        sort_direction: overrides.sort_direction,
        directories_first: overrides.directories_first,
        show_root: config.display.show_root_entry,
    };
    let mut tree_state = TreeState::new(root, tree_options);
    tree_state.apply_sort(
        tree_options.sort_order,
        tree_options.sort_direction,
        tree_options.directories_first,
    );

    let (git_state, preview_registry) = detect_terminal_and_build_registry(&config)?;

    // Create watcher channel and instance.
    let (watcher_tx, watcher_rx) =
        tokio::sync::mpsc::unbounded_channel::<Vec<notify_debouncer_mini::DebouncedEvent>>();
    let (fs_watcher, suppressed) = setup_watcher(&config, watcher_tx, &root_path);

    // Start IPC server in daemon mode.
    let (ipc_server, ipc_rx) = start_ipc_if_daemon(args.daemon, &root_path);

    // --- Phase 2: restore remaining session state (expanded dirs, cursor, etc.) ---
    let search_history =
        session_state.as_ref().map_or_else(Vec::new, |s| s.search_history.clone());
    let (selection, undo_history, restored_visual_row, deferred_expansion) = {
        let _span = tracing::info_span!("session_restore").entered();
        restore_session_state(session_state, &config, builder, &mut tree_state)
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

    // Compile file style matcher from display config.
    let file_style_matcher = crate::ui::file_style::FileStyleMatcher::new(
        &config.display.file_styles,
        &config.display.styles,
    )
    .context("Failed to compile file style rules")?;

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
        show_icons: config.display.show_icons,
        show_preview,
        show_hidden,
        show_ignored,
        viewport_height: 0,
        scroll: ScrollState::new(),
        status_message: None,
        processing: false,
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
        dirty: true,
        file_style_matcher,
        preview_debounce: None,
        layout_areas: LayoutAreas::default(),
        deferred_expansion,
        search_history,
        search_match_indices: std::collections::HashMap::new(),
    };

    let (ctx, channels) = build_context_and_channels(
        &config,
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
        // Reveal overrides session scroll: always center on the revealed path.
        let visual_row = if reveal_succeeded { None } else { restored_visual_row };
        setup_terminal(&mut state, visual_row, config.mouse.enabled)
    };

    Ok((state, ctx, channels, terminal))
}

/// Build the immutable runtime context and async event channels.
#[allow(clippy::too_many_arguments)]
fn build_context_and_channels(
    config: &Config,
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
        git_tx,
        git_enabled,
        root_path: root_path.to_path_buf(),
        rebuild_tx,
        menus: config.menus.clone(),
        search_index: Arc::new(RwLock::new(
            SearchIndex::new(),
        )),
        search_max_results: config.search.max_results,
    };

    let channels = EventReceivers {
        children: children_rx,
        preview: preview_rx,
        git: git_rx,
        rebuild: rebuild_rx,
        watcher: watcher_rx,
        ipc: ipc_rx,
        pre_children: None,
        pre_preview: None,
        pre_git: None,
        pre_rebuild: None,
        pre_watcher: None,
        pre_ipc: None,
    };

    (ctx, channels)
}

/// Initialize terminal and pre-compute viewport dimensions.
///
/// Auto-hides preview on narrow terminals and restores scroll position.
/// `restored_visual_row` is `Some(row)` when a session had a saved cursor
/// visual row, or `None` to center the cursor (reveal, old session, fresh start).
fn setup_terminal(
    state: &mut AppState,
    restored_visual_row: Option<usize>,
    mouse: bool,
) -> ratatui::DefaultTerminal {
    let terminal = crate::terminal::init(mouse);

    // Pre-compute viewport height and auto-hide preview on narrow terminals
    // before first draw, so scroll restore can work without a flash.
    if let Ok((width, height)) = crossterm::terminal::size() {
        if width <= state.layout_narrow_width {
            state.show_preview = false;
        }
        state.viewport_height = height.saturating_sub(1) as usize;
    }

    // Restore scroll position before first draw.
    match restored_visual_row {
        Some(visual_row) => {
            let cursor = state.tree_state.cursor();
            state.scroll.set_offset(cursor.saturating_sub(visual_row));
            state.scroll.clamp_to_cursor(cursor, state.viewport_height);
        }
        None => {
            state.scroll.center_on_cursor(state.tree_state.cursor(), state.viewport_height);
        }
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

/// Trigger initial async operations: prefetch, git status, preview, deferred session restore.
fn trigger_initial_loads(state: &mut AppState, ctx: &AppContext, root_path: &Path) {
    trigger_prefetch(&mut state.tree_state, root_path, ctx, state.show_hidden, state.show_ignored);
    schedule_deferred_loads(state, ctx);
    if ctx.git_enabled {
        trigger_git_status(ctx);
    }
    trigger_preview(state, ctx);
}

/// Schedule the next batch of deferred session-restore directory loads.
///
/// Tries to start loading remaining expanded directories whose parents are already
/// loaded, up to the concurrency limit. Cleans up when all loads complete or
/// no further progress can be made (stuck paths with missing parents).
fn schedule_deferred_loads(state: &mut AppState, ctx: &AppContext) {
    let Some(ref mut deferred) = state.deferred_expansion else {
        return;
    };
    let paths = deferred.schedule(&mut state.tree_state);
    for path in paths {
        spawn_load_children(
            &ctx.children_tx,
            path,
            state.show_hidden,
            state.show_ignored,
            LoadKind::DeferredRestore,
        );
    }
    if state.deferred_expansion.as_ref().is_some_and(|d| d.is_done() || d.is_stuck()) {
        if state.deferred_expansion.as_ref().is_some_and(DeferredExpansion::is_stuck) {
            tracing::warn!("deferred expansion stuck — remaining paths have unloaded parents");
        }
        tracing::info!("deferred session restore complete");
        state.deferred_expansion = None;
    }
}

/// Apply children load results to the tree, preserving cursor position for non-user loads.
fn apply_children_load(
    state: &mut AppState,
    path: &Path,
    children: Vec<crate::state::tree::TreeNode>,
    kind: LoadKind,
) {
    let _span =
        tracing::info_span!("set_children", path = %path.display(), child_count = children.len(), ?kind)
            .entered();

    match kind {
        LoadKind::DeferredRestore | LoadKind::Prefetch => {
            // Preserve cursor visual position during deferred restore and
            // background refreshes (e.g. watcher-triggered reloads) so the
            // selected file does not jump when files are added or removed.
            let snapshot = CursorSnapshot::capture(&state.tree_state, &state.scroll);
            let auto_expand = kind != LoadKind::Prefetch;
            state.tree_state.set_children(path, children, auto_expand);
            snapshot.restore(&mut state.tree_state, &mut state.scroll, state.viewport_height);
        }
        LoadKind::UserExpand => {
            state.tree_state.set_children(path, children, true);
        }
    }
}

/// Post-processing after a children load: trigger prefetch or advance deferred loads.
fn post_children_load(state: &mut AppState, ctx: &AppContext, loaded_path: &Path, kind: LoadKind) {
    match kind {
        LoadKind::UserExpand => {
            let _span = tracing::info_span!("trigger_prefetch_in_process").entered();
            trigger_prefetch(
                &mut state.tree_state,
                loaded_path,
                ctx,
                state.show_hidden,
                state.show_ignored,
            );
        }
        LoadKind::DeferredRestore => advance_deferred(state, ctx),
        LoadKind::Prefetch => {}
    }
}

/// Advance deferred session restore: record completion and schedule next batch.
fn advance_deferred(state: &mut AppState, ctx: &AppContext) {
    if let Some(ref mut deferred) = state.deferred_expansion {
        deferred.on_load_complete();
    }
    schedule_deferred_loads(state, ctx);
}

/// Process all async channel results: children loads, watcher, IPC, git, rebuild, preview.
///
/// Returns `(last_cursor, had_events)` -- the updated cursor position and whether
/// any channel produced results (indicating the UI may need a redraw).
fn process_async_events(
    state: &mut AppState,
    ctx: &AppContext,
    channels: &mut EventReceivers,
    mut last_cursor: usize,
) -> (usize, bool) {
    let mut had_events = false;

    // Children load results.
    //
    // DeferredRestore results are batched: all set_children calls happen first,
    // then cursor is resolved once. This avoids O(N×V) per-result cursor walks
    // when many deferred loads complete in the same frame.
    {
        let _span = tracing::info_span!("process_children").entered();

        let mut deferred_snapshot: Option<CursorSnapshot> = None;

        for result in channels.drain_children() {
            had_events = true;
            match result.children {
                Ok(children) => {
                    let loaded_path = result.path.clone();
                    let kind = result.kind;

                    if kind == LoadKind::DeferredRestore {
                        if deferred_snapshot.is_none() {
                            deferred_snapshot =
                                Some(CursorSnapshot::capture(&state.tree_state, &state.scroll));
                        }
                        state
                            .tree_state
                            .set_children(&result.path, children, true);
                    } else {
                        apply_children_load(state, &result.path, children, kind);
                    }
                    post_children_load(state, ctx, &loaded_path, kind);
                }
                Err(err) => {
                    tracing::warn!(?result.path, %err, "failed to load children");
                    state.tree_state.set_children_error(&result.path);
                    if result.kind == LoadKind::DeferredRestore {
                        post_children_load(state, ctx, &result.path, result.kind);
                    }
                }
            }
        }

        if let Some(snapshot) = deferred_snapshot {
            snapshot.restore(&mut state.tree_state, &mut state.scroll, state.viewport_height);
        }
    }

    // Watcher events (file system changes in watched directories).
    {
        let _span = tracing::info_span!("process_watcher").entered();
        if process_watcher_events(channels.drain_watcher(), state, ctx) {
            had_events = true;
        }
    }

    // IPC commands.
    for cmd in channels.drain_ipc() {
        had_events = true;
        handle_ipc_command(cmd, state);
    }

    // Git status results.
    for result in channels.drain_git() {
        had_events = true;
        if let Ok(mut guard) = state.git_state.write() {
            *guard = result.state;
        }
        tracing::debug!("git status updated");
    }

    // Tree rebuild results.
    if process_rebuild_results(channels.drain_rebuild(), state, ctx) {
        had_events = true;
    }

    // Preview load results.
    {
        let _span = tracing::info_span!("process_preview").entered();
        if process_preview_results(channels.drain_preview(), state) {
            had_events = true;
        }
    }

    // Trigger preview when cursor changes.
    let current_cursor = state.tree_state.cursor();
    if current_cursor != last_cursor {
        last_cursor = current_cursor;
        if state.show_preview {
            trigger_preview(state, ctx);
        }
    }

    tracing::debug!(had_events, "async events processed");
    (last_cursor, had_events)
}

/// Compute the adaptive poll/sleep duration based on current state.
///
/// Returns `Duration::ZERO` when dirty (immediate redraw), short timeout when
/// keys are pending, bounded by status message remaining and preview debounce
/// deadline, 500ms when idle.
fn compute_poll_duration(state: &AppState) -> Duration {
    if state.dirty {
        return Duration::ZERO;
    }

    let mut duration = Duration::from_millis(500);

    if state.pending_keys.is_pending() {
        duration = duration.min(state.pending_keys.remaining().min(Duration::from_millis(16)));
    }
    if let Some(remaining) = state.status_message.as_ref().map(StatusMessage::remaining) {
        duration = duration.min(remaining);
    }
    if let Some(deadline) = state.preview_debounce {
        let remaining = deadline.saturating_duration_since(Instant::now());
        duration = duration.min(remaining);
    }

    duration
}

/// Drain remaining buffered terminal key events synchronously.
///
/// Stops early if a key enters pending state (need to render indicator and wait).
fn drain_terminal_events(state: &mut AppState, ctx: &AppContext) -> Result<()> {
    let _span = tracing::info_span!("key_event_drain").entered();
    loop {
        if !crossterm::event::poll(Duration::ZERO)? {
            break;
        }
        match crossterm::event::read()? {
            Event::Key(key) => {
                handle_key_event(key, state, ctx);
                state.dirty = true;
                if state.pending_keys.is_pending() {
                    break;
                }
            }
            Event::Mouse(mouse) => {
                handle_mouse_event(mouse, state, ctx);
                state.dirty = true;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Run the application.
pub async fn run(args: &Args) -> Result<()> {
    let (mut state, ctx, mut channels, mut terminal) = init_app(args)?;
    let root_path = ctx.root_path.clone();
    let mut last_cursor = state.tree_state.cursor();

    let search_cancelled =
        spawn_search_index_build(&ctx.search_index, &root_path, state.show_hidden, state.show_ignored);

    // Create the async terminal event stream.
    let mut event_stream = EventStream::new();

    // Initial draw before entering the event loop.
    terminal.draw(|frame| {
        crate::ui::render(frame, &mut state);
    })?;
    state.dirty = false;

    // Main event loop.
    //
    // Uses `tokio::select!` to wait on terminal events, async channel
    // messages, or timeout — whichever fires first. After waking, drains
    // ALL pending events from all sources before drawing.
    loop {
        // ── Pre-wait housekeeping ──────────────────────────────
        if state.clear_expired_status() {
            state.dirty = true;
        }
        if state.pending_keys.is_pending() && state.pending_keys.is_timed_out() {
            handler::handle_pending_timeout(&mut state, &ctx);
            state.dirty = true;
        }

        // ── Wait phase (tokio::select!) ────────────────────────
        let poll_duration = compute_poll_duration(&state);
        let timeout = tokio::time::sleep(poll_duration);
        tokio::pin!(timeout);

        let mut got_terminal_event = false;
        channels.clear_pre();

        tokio::select! {
            biased;

            maybe_event = event_stream.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) => {
                        got_terminal_event = true;
                        handle_key_event(key, &mut state, &ctx);
                        state.dirty = true;
                    }
                    Some(Ok(Event::Mouse(mouse))) => {
                        handle_mouse_event(mouse, &mut state, &ctx);
                        state.dirty = true;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        tracing::error!(%e, "terminal event stream error");
                    }
                    None => { break; }
                }
            }

            Some(v) = channels.children.recv() => { channels.pre_children = Some(v); }
            Some(v) = channels.preview.recv()  => { channels.pre_preview = Some(v); }
            Some(v) = channels.git.recv()      => { channels.pre_git = Some(v); }
            Some(v) = channels.rebuild.recv()  => { channels.pre_rebuild = Some(v); }
            Some(v) = channels.watcher.recv()  => { channels.pre_watcher = Some(v); }
            Some(v) = channels.ipc.recv()      => { channels.pre_ipc = Some(v); }

            () = &mut timeout => {}
        }

        let _frame_span = tracing::info_span!("frame", dirty = state.dirty).entered();

        // ── Drain remaining terminal events ────────────────────
        if got_terminal_event && !state.pending_keys.is_pending() {
            drain_terminal_events(&mut state, &ctx)?;
        }

        // ── Drain async channels (including pre-received) ──────
        let had_events;
        (last_cursor, had_events) =
            process_async_events(&mut state, &ctx, &mut channels, last_cursor);
        if had_events {
            state.dirty = true;
        }

        // ── Fire pending debounced preview ─────────────────────
        if state.preview_debounce.is_some_and(|d| Instant::now() >= d) {
            handler::preview::fire_pending_preview(&mut state, &ctx);
            state.dirty = true;
        }

        // ── Scroll + Draw ──────────────────────────────────────
        let old_offset = state.scroll.offset();
        state.scroll.clamp_to_cursor(state.tree_state.cursor(), state.viewport_height);
        if state.scroll.offset() != old_offset {
            state.dirty = true;
        }

        if state.should_quit {
            break;
        }

        if state.dirty || state.needs_redraw {
            if state.needs_redraw {
                state.needs_redraw = false;
                terminal.clear()?;
            }
            let _span = tracing::info_span!("draw").entered();
            terminal.draw(|frame| {
                crate::ui::render(frame, &mut state);
            })?;
            state.dirty = false;
        } else {
            tracing::trace!("frame skipped (not dirty)");
        }
    }

    // Cancel background search index build if still running.
    search_cancelled.store(true, std::sync::atomic::Ordering::Relaxed);


    shutdown(&root_path, &state);
    Ok(())
}

/// Spawn a background task to build the search index.
///
/// Returns a cancellation token that should be set to `true` when the app shuts
/// down so the background walk stops early.
fn spawn_search_index_build(
    index: &Arc<RwLock<SearchIndex>>,
    root_path: &Path,
    show_hidden: bool,
    show_ignored: bool,
) -> Arc<AtomicBool> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let idx = Arc::clone(index);
    let root = root_path.to_path_buf();
    let cancel = Arc::clone(&cancelled);
    tokio::task::spawn_blocking(move || {
        search_index::build_search_index(
            &idx,
            &root,
            show_hidden,
            show_ignored,
            &cancel,
            DEFAULT_MAX_ENTRIES,
        );
    });
    cancelled
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

/// Resolved display/sort settings after merging config, session, and CLI overrides.
///
/// Precedence: CLI flags > session > config defaults.
#[expect(clippy::struct_excessive_bools, reason = "mirrors AppState display flags")]
struct SessionOverrides {
    /// Whether to show hidden (dot) files.
    show_hidden: bool,
    /// Whether to show gitignored files.
    show_ignored: bool,
    /// Whether the preview panel is visible.
    show_preview: bool,
    /// Sort order.
    sort_order: crate::state::tree::SortOrder,
    /// Sort direction.
    sort_direction: crate::state::tree::SortDirection,
    /// Whether directories are sorted before files.
    directories_first: bool,
}

/// Phase 1: load session file early and resolve display/sort overrides.
///
/// Returns the loaded session state (if any) and the resolved overrides.
/// CLI flags take precedence over session values, which take precedence
/// over config defaults.
fn load_session_overrides(
    args: &Args,
    config: &Config,
    root_path: &Path,
) -> (Option<session::SessionState>, SessionOverrides) {
    let should_restore = if args.restore {
        true
    } else if args.no_restore {
        false
    } else {
        config.session.restore_by_default
    };

    let session_state = if should_restore {
        match session::restore(root_path) {
            Ok(Some(state)) => Some(state),
            Ok(None) => {
                tracing::debug!("no session to restore");
                None
            }
            Err(e) => {
                tracing::warn!(%e, "failed to restore session");
                None
            }
        }
    } else {
        None
    };

    // Resolve overrides: CLI > session > config.
    // CLI boolean flags (show_hidden, show_ignored, no_preview, no_directories_first)
    // are one-directional: when set, they always win. When not set, use session value.
    let show_hidden = if args.show_hidden {
        true
    } else {
        session_state.as_ref().and_then(|s| s.show_hidden).unwrap_or(config.display.show_hidden)
    };

    let show_ignored = if args.show_ignored {
        true
    } else {
        session_state.as_ref().and_then(|s| s.show_ignored).unwrap_or(config.display.show_ignored)
    };

    let show_preview = if args.no_preview {
        false
    } else {
        session_state.as_ref().and_then(|s| s.show_preview).unwrap_or(config.display.show_preview)
    };

    let sort_order = args.sort_order.map_or_else(
        || {
            session_state
                .as_ref()
                .and_then(|s| s.sort_order)
                .unwrap_or_else(|| config.sort.order.into())
        },
        Into::into,
    );

    let sort_direction = args.sort_direction.map_or_else(
        || {
            session_state
                .as_ref()
                .and_then(|s| s.sort_direction)
                .unwrap_or_else(|| config.sort.direction.into())
        },
        Into::into,
    );

    let directories_first = if args.no_directories_first {
        false
    } else {
        session_state
            .as_ref()
            .and_then(|s| s.directories_first)
            .unwrap_or(config.sort.directories_first)
    };

    let overrides = SessionOverrides {
        show_hidden,
        show_ignored,
        show_preview,
        sort_order,
        sort_direction,
        directories_first,
    };

    (session_state, overrides)
}

/// Phase 2: restore session state with deferred tree expansion.
///
/// Expanded directories are loaded in three phases:
/// 1. **Critical** (sync): cursor ancestors — ensures cursor is immediately visible.
/// 2. **Viewport** (sync): directories visible around cursor — no `[Loading...]` in initial view.
/// 3. **Deferred** (async, returned): everything else — loaded after first render, cursor-proximate first.
///
/// Returns `(selection, undo_history, restored_visual_row, deferred_expansion)`.
fn restore_session_state(
    session_state: Option<session::SessionState>,
    config: &Config,
    builder: TreeBuilder,
    tree_state: &mut TreeState,
) -> (SelectionBuffer, UndoHistory, Option<usize>, Option<DeferredExpansion>) {
    let mut selection = SelectionBuffer::new();
    let mut undo_history = UndoHistory::new(config.file_op.undo_stack_size);
    let mut restored_visual_row = None;
    let mut deferred = None;

    if let Some(session_state) = session_state {
        let cursor_path = session_state.cursor_path.clone();

        // Split expanded paths into critical (cursor ancestors) and the rest.
        let mut paths = session_state.expanded_paths;
        paths.sort_by_key(|p| p.as_os_str().len());

        let (critical, mut remaining) = split_critical_paths(&paths, cursor_path.as_deref());

        // Phase 1: load cursor ancestors synchronously.
        {
            let _span = tracing::info_span!("phase1_critical", count = critical.len()).entered();
            for path in &critical {
                if let Ok(children) = builder.load_children(path) {
                    tree_state.set_children(path, children, true);
                }
            }
        }

        // Restore cursor position (needed before viewport calculation).
        if let Some(ref cp) = cursor_path {
            tree_state.move_cursor_to_path(cp);
        }

        // Phase 2: load viewport-visible expanded directories synchronously.
        {
            let _span = tracing::info_span!("phase2_viewport").entered();
            let viewport_height = estimate_viewport_height();
            load_viewport_paths(tree_state, &mut remaining, builder, viewport_height);
        }

        // Re-establish cursor: Phase 2 set_children calls shift visible indices
        // without updating the cursor field.
        if let Some(ref cp) = cursor_path {
            tree_state.move_cursor_to_path(cp);
        }

        // Phase 3: remaining paths become deferred (async after first render).
        if !remaining.is_empty() {
            let cp = cursor_path.as_deref().unwrap_or_else(|| Path::new(""));
            tracing::info!(count = remaining.len(), "deferring session expansion");
            deferred = Some(DeferredExpansion::new(remaining, cp));
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

        // Old sessions without scroll_visual_row fall back to None
        // (center_on_cursor in setup_terminal).
        restored_visual_row = session_state.scroll_visual_row;
        tracing::info!(?restored_visual_row, "session restored (critical + viewport sync, rest deferred)");
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

    (selection, undo_history, restored_visual_row, deferred)
}

/// Split expanded paths into cursor ancestors (critical) and the rest.
fn split_critical_paths(
    sorted_paths: &[PathBuf],
    cursor_path: Option<&Path>,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let Some(cursor) = cursor_path else {
        // No cursor — nothing is critical, defer everything.
        return (Vec::new(), sorted_paths.to_vec());
    };

    // Collect all ancestor directories of the cursor path.
    let mut ancestors: HashSet<&Path> = HashSet::new();
    let mut current = cursor.parent();
    while let Some(dir) = current {
        ancestors.insert(dir);
        current = dir.parent();
    }

    let mut critical = Vec::new();
    let mut remaining = Vec::new();
    for path in sorted_paths {
        if ancestors.contains(path.as_path()) {
            critical.push(path.clone());
        } else {
            remaining.push(path.clone());
        }
    }
    (critical, remaining)
}

/// Estimate viewport height from terminal size (before raw mode).
fn estimate_viewport_height() -> usize {
    crossterm::terminal::size().map_or(40, |(_, h)| h.saturating_sub(1) as usize)
}

/// Phase 2: synchronously load expanded directories visible within the viewport.
///
/// Iterates until no more `NotLoaded` expanded directories appear in the viewport
/// range around the cursor. Loaded paths are removed from `remaining`.
fn load_viewport_paths(
    tree_state: &mut TreeState,
    remaining: &mut Vec<PathBuf>,
    builder: TreeBuilder,
    viewport_height: usize,
) {
    if remaining.is_empty() {
        return;
    }

    let remaining_set: HashSet<PathBuf> = remaining.iter().cloned().collect();

    loop {
        let visible = tree_state.visible_nodes();
        let cursor = tree_state.cursor();
        let half = viewport_height / 2;
        let start = cursor.saturating_sub(half);
        let end = (cursor + half + 1).min(visible.len());

        // Find expanded directories in the viewport that need loading.
        let mut to_load: Vec<PathBuf> = Vec::new();
        for vnode in visible.get(start..end).unwrap_or_default() {
            if vnode.node.is_dir
                && remaining_set.contains(&vnode.node.path)
                && matches!(vnode.node.children, ChildrenState::NotLoaded)
            {
                to_load.push(vnode.node.path.clone());
            }
        }

        if to_load.is_empty() {
            break;
        }

        for path in &to_load {
            if let Ok(children) = builder.load_children(path) {
                tree_state.set_children(path, children, true);
            }
        }
        remaining.retain(|p| !to_load.contains(p));
    }
}

/// Save session and restore terminal.
fn shutdown(root_path: &Path, state: &AppState) {
    // Save session state.
    let display = session::DisplaySettings {
        show_hidden: state.show_hidden,
        show_ignored: state.show_ignored,
        show_preview: state.show_preview,
        sort_order: state.tree_state.sort_order(),
        sort_direction: state.tree_state.sort_direction(),
        directories_first: state.tree_state.directories_first(),
    };
    let visual_row = state.tree_state.cursor().saturating_sub(state.scroll.offset());
    let session_state = session::build_session_state(
        root_path,
        state.expanded_paths_including_deferred(),
        state.tree_state.cursor_path(),
        &state.selection,
        &state.undo_history,
        visual_row,
        &display,
        state.search_history.clone(),
    );
    if let Err(e) = session::save(&session_state) {
        tracing::warn!(%e, "failed to save session");
    }

    // Restore terminal.
    crate::terminal::restore();
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
    results: impl Iterator<Item = PreviewLoadResult>,
    state: &mut AppState,
) -> bool {
    let mut had_results = false;
    for result in results {
        had_results = true;
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
    had_results
}

/// Process async tree rebuild results: swap tree and re-trigger prefetch/preview.
///
/// Discards stale results from superseded rebuilds using the generation counter.
fn process_rebuild_results(
    results: impl Iterator<Item = TreeRebuildResult>,
    state: &mut AppState,
    ctx: &AppContext,
) -> bool {
    let mut had_results = false;
    for result in results {
        if result.generation != state.rebuild_generation {
            tracing::debug!(
                expected = state.rebuild_generation,
                got = result.generation,
                "discarding stale rebuild result",
            );
            continue;
        }
        had_results = true;
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
    had_results
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
    batches: impl Iterator<Item = Vec<notify_debouncer_mini::DebouncedEvent>>,
    state: &mut AppState,
    ctx: &AppContext,
) -> bool {
    let mut had_events = false;
    for events in batches {
        had_events = true;
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
    had_events
}
