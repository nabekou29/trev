//! Lazy stat: schedule and process async metadata fetching for viewport-visible nodes.

use std::collections::HashMap;
use std::path::{
    Path,
    PathBuf,
};

use crate::app::state::{
    AppContext,
    AppState,
    StatLoadResult,
};
use crate::state::tree::SortOrder;

/// Schedule async stat fetches for viewport-visible nodes that lack metadata.
///
/// Collects file paths where `modified == None` within the current viewport,
/// groups them by parent directory, and spawns a blocking task per group.
pub fn schedule_viewport_stats(state: &AppState, ctx: &AppContext) {
    let nodes =
        state.tree_state.visible_nodes_in_range(state.scroll.offset(), state.viewport_height);

    // Group paths needing stat by their parent directory.
    let mut by_dir: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    for vn in &nodes {
        if vn.node.modified.is_none()
            && !vn.node.is_root
            && let Some(dir) = vn.node.path.parent().map(Path::to_path_buf)
        {
            by_dir.entry(dir).or_default().push(vn.node.path.clone());
        }
    }

    for (dir_path, paths) in by_dir {
        spawn_stat_batch(&ctx.stat_tx, dir_path, paths);
    }
}

/// Spawn a blocking task to stat a batch of files in a directory.
fn spawn_stat_batch(
    tx: &tokio::sync::mpsc::Sender<StatLoadResult>,
    dir_path: PathBuf,
    paths: Vec<PathBuf>,
) {
    let tx = tx.clone();
    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            let mut entries = Vec::with_capacity(paths.len());
            for path in paths {
                if let Ok(meta) = std::fs::metadata(&path) {
                    entries.push((path, meta.len(), meta.modified().ok()));
                }
            }
            StatLoadResult { dir_path, entries }
        })
        .await;

        if let Ok(result) = result {
            // Ignore send error (receiver dropped = app is shutting down).
            let _ = tx.send(result).await;
        }
    });
}

/// Apply stat results to tree nodes and trigger re-sort if necessary.
///
/// Returns `true` if any results were processed.
pub fn process_stat_results(
    results: impl Iterator<Item = StatLoadResult>,
    state: &mut AppState,
) -> bool {
    let mut had_results = false;
    let mut dirs_to_resort: Vec<PathBuf> = Vec::new();

    for result in results {
        had_results = true;
        apply_stat_result_to_tree(state, &result);
        dirs_to_resort.push(result.dir_path);
    }

    // Re-sort only when the current sort order depends on stat data.
    if had_results {
        if matches!(state.tree_state.sort_order(), SortOrder::Size | SortOrder::Modified) {
            for dir_path in &dirs_to_resort {
                state.tree_state.resort_children(dir_path);
            }
        }
        state.dirty = true;
    }

    had_results
}

/// Apply a single stat result batch to the tree, updating `size`/`modified` on children.
fn apply_stat_result_to_tree(state: &mut AppState, result: &StatLoadResult) {
    let Some(node) = state.tree_state.find_node_by_path_mut(&result.dir_path) else {
        return;
    };
    let Some(children) = node.children.as_loaded_mut() else {
        return;
    };

    // Build path → index map, then apply updates by index.
    let index: HashMap<&Path, usize> =
        children.iter().enumerate().map(|(i, c)| (c.path.as_path(), i)).collect();

    let updates: Vec<(usize, u64, Option<std::time::SystemTime>)> = result
        .entries
        .iter()
        .filter_map(|(path, size, modified)| {
            index.get(path.as_path()).map(|&idx| (idx, *size, *modified))
        })
        .collect();

    for (idx, size, modified) in updates {
        if let Some(child) = children.get_mut(idx) {
            child.size = size;
            child.modified = modified;
        }
    }

    // Recompute `recursive_max_mtime` for the parent.
    node.recursive_max_mtime = children.iter().filter_map(|c| c.modified).max();
}
