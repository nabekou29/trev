//! Tree navigation action handlers.
//!
//! Handles expand, collapse, cursor movement, prefetch, and directory reload.

use std::path::{
    Path,
    PathBuf,
};

use rayon::prelude::*;

use crate::app::state::{
    AppContext,
    AppState,
    ChildrenLoadResult,
    CursorSnapshot,
    LoadKind,
    TreeRebuildResult,
};
use crate::state::tree::{
    ExpandResult,
    TreeState,
};
use crate::tree::builder::TreeBuilder;

/// Handle a tree action.
pub fn handle_tree_action(
    action: crate::action::TreeAction,
    state: &mut AppState,
    ctx: &AppContext,
) {
    use crate::action::TreeAction;

    match action {
        TreeAction::MoveDown => {
            state.tree_state.move_cursor(1);
        }
        TreeAction::MoveUp => {
            state.tree_state.move_cursor(-1);
        }
        TreeAction::Expand => {
            let show_hidden = state.show_hidden;
            let show_ignored = state.show_ignored;
            if let Some(result) = state.tree_state.expand_dir() {
                handle_expand_result(
                    &result,
                    &mut state.tree_state,
                    ctx,
                    show_hidden,
                    show_ignored,
                );
            }
        }
        TreeAction::Collapse => {
            state.tree_state.collapse();
        }
        TreeAction::ToggleExpand => {
            let show_hidden = state.show_hidden;
            let show_ignored = state.show_ignored;
            let cursor = state.tree_state.cursor();

            if let Some(result) = state.tree_state.toggle_expand(cursor) {
                handle_expand_result(
                    &result,
                    &mut state.tree_state,
                    ctx,
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
            state.tree_state.half_page_down(state.viewport_height);
        }
        TreeAction::HalfPageUp => {
            state.tree_state.half_page_up(state.viewport_height);
        }
        TreeAction::ExpandAll => {
            handle_expand_all(state, ctx);
        }
        TreeAction::CollapseAll => {
            handle_collapse_all(state);
        }
        TreeAction::Refresh => {
            rebuild_tree(state, ctx);
            // Re-fetch git status.
            if ctx.git_enabled {
                crate::app::trigger_git_status(ctx);
            }
            state.set_status("Refreshed");
        }
        TreeAction::ChangeRoot => {
            if let Some(info) = state.tree_state.current_node_info()
                && info.is_dir
            {
                change_root(state, ctx, &info.path);
            }
        }
        TreeAction::ChangeRootUp => {
            let root = state.tree_state.root_path().to_path_buf();
            if let Some(parent) = root.parent()
                && parent != root
            {
                change_root(state, ctx, parent);
            }
        }
        TreeAction::CenterCursor => {
            state.scroll.center_on_cursor(state.tree_state.cursor(), state.viewport_height);
        }
        TreeAction::ScrollCursorToTop => {
            state.scroll.scroll_cursor_to_top(state.tree_state.cursor());
        }
        TreeAction::ScrollCursorToBottom => {
            state.scroll.scroll_cursor_to_bottom(state.tree_state.cursor(), state.viewport_height);
        }
        TreeAction::Sort(sort_action) => handle_sort_action(sort_action, state),
    }
}

/// Maximum number of directories to expand in a single expand-all operation.
const EXPAND_ALL_LIMIT: usize = 300;

/// Handle expand-all: recursively expand the subtree under cursor.
fn handle_expand_all(state: &mut AppState, ctx: &AppContext) {
    let Some(dir_path) = state.tree_state.cursor_dir_path() else {
        return;
    };
    let show_hidden = state.show_hidden;
    let show_ignored = state.show_ignored;
    let result = state.tree_state.expand_subtree(&dir_path, EXPAND_ALL_LIMIT);

    // Spawn loads for directories that need loading.
    // Use Prefetch because expand_subtree already set is_expanded=true.
    // This avoids redundant trigger_prefetch calls in process_children,
    // which would otherwise do an expensive find_node_mut for each result.
    for path in &result.needs_load {
        spawn_load_children(
            &ctx.children_tx,
            path.clone(),
            show_hidden,
            show_ignored,
            LoadKind::Prefetch,
        );
    }

    // Status message.
    if result.hit_limit {
        state.set_status(format!(
            "Expanded {} directories (limit: {EXPAND_ALL_LIMIT})",
            result.expanded
        ));
    } else if result.expanded > 0 {
        state.set_status(format!("Expanded {} directories", result.expanded));
    }
}

/// Handle collapse-all: collapse the subtree under cursor.
///
/// If the cursor is on a file or a collapsed directory, collapses the parent
/// directory instead. Moves the cursor to the collapsed directory afterwards.
fn handle_collapse_all(state: &mut AppState) {
    let Some(info) = state.tree_state.current_node_info() else {
        return;
    };

    // Determine which directory to collapse:
    // - Expanded directory → collapse it directly
    // - File or collapsed directory → collapse the parent
    let is_expanded_dir = info.is_dir
        && state
            .tree_state
            .visible_nodes()
            .get(state.tree_state.cursor())
            .is_some_and(|vn| vn.node.is_expanded);

    let dir_path = if is_expanded_dir {
        info.path
    } else {
        let Some(parent) = info.path.parent() else {
            return;
        };
        parent.to_path_buf()
    };

    let collapsed = state.tree_state.collapse_subtree(&dir_path);

    // Move cursor to the collapsed directory.
    state.tree_state.move_cursor_to_path(&dir_path);

    if !collapsed.is_empty() {
        state.set_status(format!("Collapsed {} directories", collapsed.len()));
    }
}

/// Handle a sort sub-action.
fn handle_sort_action(sort_action: crate::action::SortAction, state: &mut AppState) {
    use crate::action::SortAction;

    match sort_action {
        SortAction::Menu => open_sort_menu(state),
        SortAction::ToggleDirection => toggle_sort_direction(state),
        SortAction::ByName => apply_sort_order(state, crate::state::tree::SortOrder::Name),
        SortAction::BySize => apply_sort_order(state, crate::state::tree::SortOrder::Size),
        SortAction::ByMtime => apply_sort_order(state, crate::state::tree::SortOrder::Modified),
        SortAction::ByType => apply_sort_order(state, crate::state::tree::SortOrder::Type),
        SortAction::ByExtension => {
            apply_sort_order(state, crate::state::tree::SortOrder::Extension);
        }
        SortAction::BySmart => apply_sort_order(state, crate::state::tree::SortOrder::Smart),
        SortAction::ToggleDirectoriesFirst => toggle_directories_first(state),
    }
}

/// Open the sort order selection menu.
///
/// Lists all sort order variants with the current one marked with `●`.
fn open_sort_menu(state: &mut AppState) {
    use clap::ValueEnum;

    use crate::input::{
        AppMode,
        MenuAction,
        MenuItem,
        MenuState,
    };

    let current = state.tree_state.sort_order();
    let variants = crate::config::SortOrder::value_variants();

    let mut items = Vec::with_capacity(variants.len());
    let mut current_idx = 0;

    for (i, variant) in variants.iter().enumerate() {
        let Some(pv) = variant.to_possible_value() else {
            continue;
        };
        let name = pv.get_name();
        let state_order: crate::state::tree::SortOrder = (*variant).into();
        let is_current = state_order == current;
        if is_current {
            current_idx = i;
        }
        let label = if is_current { format!("● {name}") } else { format!("  {name}") };
        // Use first character as shortcut key.
        let key = name.chars().next().unwrap_or(' ');
        items.push(MenuItem { key, label, value: name.to_string() });
    }

    state.mode = AppMode::Menu(MenuState {
        title: "Sort order".to_string(),
        items,
        cursor: current_idx,
        on_select: MenuAction::SelectSortOrder,
        item_actions: Vec::new(),
    });
}

/// Apply a specific sort order directly (without opening the menu).
fn apply_sort_order(state: &mut AppState, order: crate::state::tree::SortOrder) {
    let direction = state.tree_state.sort_direction();
    let dirs_first = state.tree_state.directories_first();
    state.tree_state.apply_sort(order, direction, dirs_first);
    state.set_status(format!("Sort: {order:?}"));
}

/// Toggle sort direction between ascending and descending.
fn toggle_sort_direction(state: &mut AppState) {
    use crate::state::tree::SortDirection;

    let new_direction = match state.tree_state.sort_direction() {
        SortDirection::Asc => SortDirection::Desc,
        SortDirection::Desc => SortDirection::Asc,
    };
    let order = state.tree_state.sort_order();
    let dirs_first = state.tree_state.directories_first();
    state.tree_state.apply_sort(order, new_direction, dirs_first);
    let label = match new_direction {
        SortDirection::Asc => "ascending",
        SortDirection::Desc => "descending",
    };
    state.set_status(format!("Sort direction: {label}"));
}

/// Toggle directories-first sorting on or off.
fn toggle_directories_first(state: &mut AppState) {
    let new_dirs_first = !state.tree_state.directories_first();
    let order = state.tree_state.sort_order();
    let direction = state.tree_state.sort_direction();
    state.tree_state.apply_sort(order, direction, new_dirs_first);
    let label = if new_dirs_first { "on" } else { "off" };
    state.set_status(format!("Directories first: {label}"));
}

/// Spawn an async tree rebuild with the current display settings.
///
/// Captures sort/display state, builds the tree on a background thread,
/// and sends the result through the rebuild channel. The event loop
/// applies the result when it arrives.
pub(super) fn rebuild_tree(state: &mut AppState, ctx: &AppContext) {
    // Increment generation so any in-flight rebuild is discarded.
    state.rebuild_generation = state.rebuild_generation.wrapping_add(1);
    let generation = state.rebuild_generation;

    let expanded = state.tree_state.expanded_paths();
    let cursor_path = state.tree_state.cursor_path();
    let fallback_paths = state.tree_state.paths_above_cursor();
    let visual_row = CursorSnapshot::capture(&state.tree_state, &state.scroll).visual_row;
    let order = state.tree_state.sort_order();
    let direction = state.tree_state.sort_direction();
    let dirs_first = state.tree_state.directories_first();
    let show_root = state.tree_state.show_root();
    let root_path = state.tree_state.root_path().to_path_buf();
    let show_hidden = state.show_hidden;
    let show_ignored = state.show_ignored;

    let tx = ctx.rebuild_tx.clone();

    tokio::task::spawn_blocking(move || {
        let _span =
            tracing::info_span!("rebuild_tree", expanded_count = expanded.len(), generation,)
                .entered();

        let builder = TreeBuilder::new(show_hidden, show_ignored);
        let Ok(root) = builder.build(&root_path) else {
            tracing::warn!("rebuild_tree: failed to build root");
            return;
        };

        let options = crate::state::tree::TreeOptions {
            sort_order: order,
            sort_direction: direction,
            directories_first: dirs_first,
            show_root,
        };
        let mut new_tree = TreeState::new(root, options);

        // Sort root's children (builder.build doesn't sort them).
        new_tree.apply_sort(order, direction, dirs_first);

        // Re-expand directories (shortest paths first so parents load before children).
        // Skip root — already loaded by builder.build().
        let mut sorted_expanded = expanded;
        sorted_expanded.retain(|p| p != &root_path);
        sorted_expanded.sort_by_key(|p| p.as_os_str().len());

        // Load children in parallel (each load_children is independent FS I/O).
        let loaded: Vec<_> = sorted_expanded
            .par_iter()
            .filter_map(|path| builder.load_children(path).ok().map(|children| (path, children)))
            .collect();

        // Apply results sequentially (parent→child order preserved by par_iter).
        for (path, children) in loaded {
            new_tree.set_children(path, children, true);
        }

        // Restore cursor position.
        // If the cursor's node was filtered out, fall back to the nearest
        // node that was above the cursor in the previous tree.
        if let Some(ref cp) = cursor_path
            && !new_tree.move_cursor_to_path(cp)
        {
            for path in &fallback_paths {
                if new_tree.move_cursor_to_path(path) {
                    break;
                }
            }
        }

        // Send result (ignore error if receiver dropped during shutdown).
        let _ = tx.blocking_send(TreeRebuildResult {
            tree_state: new_tree,
            root_path,
            show_hidden,
            show_ignored,
            generation,
            visual_row,
            cursor_path,
        });
    });
}

/// Change the tree root to a new directory.
///
/// Builds a new tree from `new_root` asynchronously and sends the result
/// through the rebuild channel. Updates the file system watcher to monitor
/// the new root.
fn change_root(state: &mut AppState, ctx: &AppContext, new_root: &Path) {
    // Increment generation so any in-flight rebuild is discarded.
    state.rebuild_generation = state.rebuild_generation.wrapping_add(1);
    let generation = state.rebuild_generation;

    let show_hidden = state.show_hidden;
    let show_ignored = state.show_ignored;
    let order = state.tree_state.sort_order();
    let direction = state.tree_state.sort_direction();
    let dirs_first = state.tree_state.directories_first();
    let show_root = state.tree_state.show_root();

    let tx = ctx.rebuild_tx.clone();
    let root_path = new_root.to_path_buf();

    tokio::task::spawn_blocking(move || {
        let builder = TreeBuilder::new(show_hidden, show_ignored);
        let Ok(root) = builder.build(&root_path) else {
            tracing::warn!("change_root: failed to build root");
            return;
        };

        let options = crate::state::tree::TreeOptions {
            sort_order: order,
            sort_direction: direction,
            directories_first: dirs_first,
            show_root,
        };
        let mut new_tree = TreeState::new(root, options);
        new_tree.apply_sort(order, direction, dirs_first);

        let _ = tx.blocking_send(TreeRebuildResult {
            tree_state: new_tree,
            root_path,
            show_hidden,
            show_ignored,
            generation,
            visual_row: 0,
            cursor_path: None,
        });
    });

    // Update watcher to monitor the new root.
    if let Some(ref mut watcher) = state.watcher
        && let Err(e) = watcher.watch_root(new_root)
    {
        tracing::warn!(%e, "failed to watch new root directory");
    }

    let name = new_root.file_name().map_or_else(
        || new_root.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    state.set_status(format!("Root: {name}"));
}

/// Handle an expand result: spawn loads or prefetch as appropriate.
///
/// Accepts `show_hidden`/`show_ignored` as separate parameters because this
/// function operates on a partial borrow of `TreeState` (not full `AppState`).
/// Callers should extract these values from `state` before the partial borrow.
fn handle_expand_result(
    result: &ExpandResult,
    tree_state: &mut TreeState,
    ctx: &AppContext,
    show_hidden: bool,
    show_ignored: bool,
) {
    match *result {
        ExpandResult::NeedsLoad(ref path) => {
            spawn_load_children(
                &ctx.children_tx,
                path.clone(),
                show_hidden,
                show_ignored,
                LoadKind::UserExpand,
            );
        }
        ExpandResult::AlreadyLoaded(ref path) => {
            // Directory was prefetched — trigger prefetch for its children.
            trigger_prefetch(tree_state, path, ctx, show_hidden, show_ignored);
        }
        ExpandResult::OpenFile(ref path) => {
            send_open_file_notification(ctx, path);
        }
    }
}

/// Send an `open_file` notification to the connected Neovim client.
///
/// Only sends if the IPC server is running (daemon mode). The notification
/// includes the file path. The editor action is determined by the Neovim plugin.
fn send_open_file_notification(ctx: &AppContext, path: &Path) {
    if let Some(server) = &ctx.ipc_server {
        let server = server.clone();
        let path = path.to_path_buf();
        tokio::spawn(async move {
            server
                .send_notification("open_file", serde_json::json!({"path": path.to_string_lossy()}))
                .await;
        });
    }
}

/// Prefetch child directories one level ahead.
///
/// Transitions `NotLoaded` child directories to `Loading` and spawns
/// background tasks to load their children.
///
/// Accepts `show_hidden`/`show_ignored` as separate parameters because this
/// function operates on a partial borrow of `TreeState` (not full `AppState`).
pub fn trigger_prefetch(
    tree_state: &mut TreeState,
    parent_path: &Path,
    ctx: &AppContext,
    show_hidden: bool,
    show_ignored: bool,
) {
    let prefetch_paths = tree_state.start_prefetch(parent_path);
    for path in prefetch_paths {
        spawn_load_children(&ctx.children_tx, path, show_hidden, show_ignored, LoadKind::Prefetch);
    }
}

/// Spawn a blocking task to load directory children asynchronously.
pub fn spawn_load_children(
    tx: &tokio::sync::mpsc::Sender<ChildrenLoadResult>,
    path: PathBuf,
    show_hidden: bool,
    show_ignored: bool,
    kind: LoadKind,
) {
    let tx = tx.clone();
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
        let _ = tx.send(ChildrenLoadResult { path, children, kind }).await;
    });
}

/// Refresh a directory in the tree by triggering a re-read of its children.
///
/// Keeps existing children visible during reload to avoid a visual flash
/// where the directory briefly appears collapsed.
pub fn refresh_directory(state: &AppState, dir: &Path, ctx: &AppContext) {
    // Spawn reload without invalidating: old children remain visible until
    // set_children() replaces them with the fresh listing.
    // Use Prefetch to preserve the current expansion state (don't force re-expand).
    spawn_load_children(
        &ctx.children_tx,
        dir.to_path_buf(),
        state.show_hidden,
        state.show_ignored,
        LoadKind::Prefetch,
    );
}
