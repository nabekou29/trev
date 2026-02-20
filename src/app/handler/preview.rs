//! Preview action handlers.
//!
//! Handles preview scroll, provider cycling, and async preview loading.

use std::path::PathBuf;
use std::sync::Arc;

use crate::app::state::{
    AppContext,
    AppState,
    PreviewLoadResult,
};
use crate::preview::cache::CacheKey;
use crate::preview::content::PreviewContent;
use crate::preview::provider::{
    LoadContext,
    PreviewProvider,
};

/// Handle a preview action.
pub fn handle_preview_action(
    action: crate::action::PreviewAction,
    state: &mut AppState,
    ctx: &AppContext,
) {
    use crate::action::PreviewAction;

    let viewport_height = state.viewport_height;

    match action {
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
            state.preview_state.scroll_down(viewport_height / 2, viewport_height);
        }
        PreviewAction::HalfPageUp => {
            state.preview_state.scroll_up(viewport_height / 2);
        }
        PreviewAction::CycleNextProvider => {
            if state.preview_state.cycle_next_provider() {
                reload_preview(state, ctx);
            }
        }
        PreviewAction::CyclePrevProvider => {
            if state.preview_state.cycle_prev_provider() {
                reload_preview(state, ctx);
            }
        }
        PreviewAction::TogglePreview => {
            state.show_preview = !state.show_preview;
            if state.show_preview {
                trigger_preview(state, ctx);
            }
        }
    }
}

/// Trigger preview for a new file (cursor change).
///
/// Checks the cache first; on hit, displays immediately without async load.
/// On miss, spawns an async load task. Also prefetches adjacent nodes.
pub fn trigger_preview(state: &mut AppState, ctx: &AppContext) {
    let Some((path, providers)) = resolve_preview_providers(state) else {
        return;
    };
    state.preview_state.request_preview(path.clone());

    if !try_load_from_cache(state, &path, &providers) {
        spawn_preview_for(state, &path, &providers, ctx, false);
    }
    prefetch_adjacent(state, ctx);
}

/// Reload the current file with the (already-cycled) provider.
///
/// Preserves the provider index set by `cycle_next_provider()`/`cycle_prev_provider()`.
fn reload_preview(state: &mut AppState, ctx: &AppContext) {
    let Some((path, providers)) = resolve_preview_providers(state) else {
        return;
    };
    state.preview_state.reload_preview(path.clone());

    if try_load_from_cache(state, &path, &providers) {
        return;
    }
    spawn_preview_for(state, &path, &providers, ctx, false);
}

/// Try to load preview from cache. Returns `true` if cache hit.
fn try_load_from_cache(
    state: &mut AppState,
    path: &std::path::Path,
    providers: &[Arc<dyn PreviewProvider>],
) -> bool {
    let active_index = state.preview_state.active_provider_index;
    let Some(provider) = providers.get(active_index) else {
        return false;
    };
    let provider_name = provider.name().to_string();
    let key = CacheKey { path: path.to_path_buf(), provider_name };
    let Some(cached) = state.preview_cache.get(&key) else {
        tracing::debug!(path = %path.display(), "preview cache miss");
        return false;
    };
    let Some(content) = cached.try_clone() else {
        tracing::debug!(path = %path.display(), "preview cache hit but not cloneable");
        return false;
    };
    tracing::debug!(path = %path.display(), "preview cache hit");
    state.preview_state.set_content(content);
    true
}

/// Resolve available providers for the current node.
///
/// Returns the path and resolved provider list, or `None` if no providers match.
fn resolve_preview_providers(
    state: &mut AppState,
) -> Option<(PathBuf, Vec<Arc<dyn PreviewProvider>>)> {
    let node_info = state.tree_state.current_node_info()?;
    let path = node_info.path.clone();
    let is_dir = node_info.is_dir;

    let providers = state.preview_registry.resolve(&path, is_dir);
    if providers.is_empty() {
        state.preview_state.set_content(PreviewContent::Empty);
        return None;
    }

    let provider_names: Vec<String> = providers.iter().map(|p| p.name().to_string()).collect();
    state.preview_state.set_available_providers(provider_names);

    Some((path, providers))
}

/// Pick the active provider and spawn the async load task.
fn spawn_preview_for(
    state: &AppState,
    path: &std::path::Path,
    providers: &[Arc<dyn PreviewProvider>],
    ctx: &AppContext,
    prefetch: bool,
) {
    let active_index = state.preview_state.active_provider_index;
    let Some(provider) = providers.get(active_index) else {
        return;
    };
    let provider = Arc::clone(provider);
    let provider_name = provider.name().to_string();

    spawn_preview_load(PreviewLoadParams {
        tx: ctx.preview_tx.clone(),
        path: path.to_path_buf(),
        provider,
        provider_name,
        max_lines: ctx.preview_config.max_lines,
        max_bytes: ctx.preview_config.max_bytes,
        cancel_token: state.preview_state.cancel_token.clone(),
        prefetch,
    });
}

/// Prefetch preview content for adjacent nodes (cursor ± 1).
///
/// Only prefetches file nodes that are not already cached.
fn prefetch_adjacent(state: &mut AppState, ctx: &AppContext) {
    let cursor = state.tree_state.cursor();
    let visible = state.tree_state.visible_nodes();

    let indices = [cursor.wrapping_sub(1), cursor + 1];
    for &idx in &indices {
        let Some(vn) = visible.get(idx) else { continue };
        let path = &vn.node.path;
        let providers = state.preview_registry.resolve(path, vn.node.is_dir);
        let Some(provider) = providers.first() else { continue };
        let key =
            CacheKey { path: path.clone(), provider_name: provider.name().to_string() };
        if state.preview_cache.get(&key).is_some() {
            tracing::debug!(path = %path.display(), "prefetch skipped (already cached)");
            continue;
        }
        tracing::debug!(path = %path.display(), "prefetch spawning");
        let provider = Arc::clone(provider);
        let provider_name = provider.name().to_string();
        spawn_preview_load(PreviewLoadParams {
            tx: ctx.preview_tx.clone(),
            path: path.clone(),
            provider,
            provider_name,
            max_lines: ctx.preview_config.max_lines,
            max_bytes: ctx.preview_config.max_bytes,
            cancel_token: state.preview_state.cancel_token.clone(),
            prefetch: true,
        });
    }
}

/// Parameters for spawning a preview load task.
struct PreviewLoadParams {
    /// Sender for preview load results.
    tx: tokio::sync::mpsc::Sender<PreviewLoadResult>,
    /// Path of the file to preview.
    path: PathBuf,
    /// Provider to use for loading.
    provider: Arc<dyn PreviewProvider>,
    /// Provider name (for result tracking).
    provider_name: String,
    /// Maximum lines to load.
    max_lines: usize,
    /// Maximum bytes to load.
    max_bytes: u64,
    /// Cancellation token for aborting the load.
    cancel_token: tokio_util::sync::CancellationToken,
    /// Whether this is a background prefetch (not for immediate display).
    prefetch: bool,
}

/// Spawn a blocking task to load preview content.
fn spawn_preview_load(params: PreviewLoadParams) {
    let PreviewLoadParams {
        tx,
        path,
        provider,
        provider_name,
        max_lines,
        max_bytes,
        cancel_token,
        prefetch,
    } = params;
    tokio::spawn(async move {
        let load_path = path.clone();
        let token = cancel_token.clone();

        let result = tokio::task::spawn_blocking(move || {
            let ctx = LoadContext { max_lines, max_bytes, cancel_token: token };
            match provider.load(&load_path, &ctx) {
                Ok(c) => c,
                Err(e) => PreviewContent::Error { message: e.to_string() },
            }
        })
        .await;

        let content = match result {
            Ok(content) => content,
            Err(e) => PreviewContent::Error { message: e.to_string() },
        };

        // Ignore send error (receiver dropped = app shutting down).
        let _ = tx
            .send(PreviewLoadResult { path, provider_name, content, prefetch })
            .await;
    });
}
