#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]

use std::path::{
    Path,
    PathBuf,
};

use criterion::{
    Criterion,
    criterion_group,
    criterion_main,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use trev::app::{
    AppState,
    ScrollState,
};
use trev::file_op::selection::SelectionBuffer;
use trev::file_op::undo::UndoHistory;
use trev::input::AppMode;
use trev::preview::cache::PreviewCache;
use trev::preview::provider::PreviewRegistry;
use trev::preview::state::PreviewState;
use trev::state::tree::{
    ChildrenState,
    TreeNode,
    TreeOptions,
    TreeState,
};

/// Helper: create a file node.
fn file_node(name: &str, parent: &Path) -> TreeNode {
    TreeNode {
        name: name.to_string(),
        path: parent.join(name),
        is_dir: false,
        is_symlink: false,
        symlink_target: None,
        size: 100,
        modified: None,
        recursive_max_mtime: None,
        children: ChildrenState::NotLoaded,
        is_expanded: false,
        is_ignored: false,
    }
}

/// Helper: create a directory node with children.
fn dir_node(name: &str, parent: &Path, children: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        name: name.to_string(),
        path: parent.join(name),
        is_dir: true,
        is_symlink: false,
        symlink_target: None,
        size: 0,
        modified: None,
        recursive_max_mtime: None,
        children: ChildrenState::Loaded(children),
        is_expanded: false,
        is_ignored: false,
    }
}

/// Helper: create a minimal `AppState` from a `TreeState`.
fn app_state_from_tree(tree_state: TreeState) -> AppState {
    AppState {
        tree_state,
        preview_state: PreviewState::new(),
        preview_cache: PreviewCache::new(10),
        preview_registry: PreviewRegistry::new(vec![]).unwrap(),
        mode: AppMode::Normal,
        selection: SelectionBuffer::new(),
        undo_history: UndoHistory::new(100),
        watcher: None,
        should_quit: false,
        show_icons: true,
        show_preview: false,
        show_hidden: false,
        show_ignored: false,
        viewport_height: 50,
        scroll: ScrollState::new(),
        status_message: None,
        processing: false,
        emit_paths: None,
        git_state: std::sync::Arc::new(std::sync::RwLock::new(None)),
        rebuild_generation: 0,
        columns: trev::ui::column::resolve_columns(
            &trev::ui::column::default_columns(),
            &trev::ui::column::ColumnOptionsConfig::default(),
        ),
        layout_split_ratio: 50,
        layout_narrow_split_ratio: 60,
        layout_narrow_width: 80,
        pending_keys: trev::app::PendingKeys::new(std::time::Duration::from_millis(500)),
        needs_redraw: false,
        dirty: true,
        file_style_matcher: trev::ui::file_style::FileStyleMatcher::new(
            &[],
            &trev::config::CategoryStyles::default(),
        )
        .unwrap(),
        preview_debounce: None,
    }
}

/// Helper: create a `TreeState` with root expanded.
fn tree_with_children(children: Vec<TreeNode>) -> TreeState {
    let root = TreeNode {
        name: "root".to_string(),
        path: PathBuf::from("/test/root"),
        is_dir: true,
        is_symlink: false,
        symlink_target: None,
        size: 0,
        modified: None,
        recursive_max_mtime: None,
        children: ChildrenState::Loaded(children),
        is_expanded: true,
        is_ignored: false,
    };
    TreeState::new(root, TreeOptions::default())
}

fn bench_render_tree_100k_flat(c: &mut Criterion) {
    let root_path = Path::new("/test/root");
    let children: Vec<TreeNode> =
        (0..100_000).map(|i| file_node(&format!("file{i:06}.txt"), root_path)).collect();
    let state = app_state_from_tree(tree_with_children(children));
    let mut terminal = Terminal::new(TestBackend::new(120, 50)).unwrap();

    c.bench_function("render_tree_100k_flat", |b| {
        b.iter(|| {
            terminal
                .draw(|frame| {
                    trev::ui::tree_view::render_tree(frame, frame.area(), &state);
                })
                .unwrap();
        });
    });
}

fn bench_render_tree_100k_nested(c: &mut Criterion) {
    let root_path = Path::new("/test/root");
    let children: Vec<TreeNode> = (0..100)
        .map(|d| {
            let dir_path = root_path.join(format!("dir{d:03}"));
            let files: Vec<TreeNode> =
                (0..1000).map(|f| file_node(&format!("file{f:04}.txt"), &dir_path)).collect();
            let mut d = dir_node(&format!("dir{d:03}"), root_path, files);
            d.is_expanded = true;
            d
        })
        .collect();
    let state = app_state_from_tree(tree_with_children(children));
    let mut terminal = Terminal::new(TestBackend::new(120, 50)).unwrap();

    c.bench_function("render_tree_100k_nested", |b| {
        b.iter(|| {
            terminal
                .draw(|frame| {
                    trev::ui::tree_view::render_tree(frame, frame.area(), &state);
                })
                .unwrap();
        });
    });
}

fn bench_render_tree_100k_scrolled(c: &mut Criterion) {
    let root_path = Path::new("/test/root");
    let children: Vec<TreeNode> =
        (0..100_000).map(|i| file_node(&format!("file{i:06}.txt"), root_path)).collect();
    let mut state = app_state_from_tree(tree_with_children(children));
    state.scroll.clamp_to_cursor(50_000, 50);
    state.tree_state.move_cursor_to(50_000);
    let mut terminal = Terminal::new(TestBackend::new(120, 50)).unwrap();

    c.bench_function("render_tree_100k_scrolled", |b| {
        b.iter(|| {
            terminal
                .draw(|frame| {
                    trev::ui::tree_view::render_tree(frame, frame.area(), &state);
                })
                .unwrap();
        });
    });
}

fn bench_full_frame_render_100k(c: &mut Criterion) {
    let root_path = Path::new("/test/root");
    let children: Vec<TreeNode> =
        (0..100_000).map(|i| file_node(&format!("file{i:06}.txt"), root_path)).collect();
    let mut state = app_state_from_tree(tree_with_children(children));
    let mut terminal = Terminal::new(TestBackend::new(120, 50)).unwrap();

    c.bench_function("full_frame_render_100k", |b| {
        b.iter(|| {
            terminal
                .draw(|frame| {
                    trev::ui::render(frame, &mut state);
                })
                .unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_render_tree_100k_flat,
    bench_render_tree_100k_nested,
    bench_render_tree_100k_scrolled,
    bench_full_frame_render_100k,
);
criterion_main!(benches);
