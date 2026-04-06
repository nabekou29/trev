#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]

use std::collections::HashSet;
use std::path::Path;

use criterion::{
    Criterion,
    criterion_group,
    criterion_main,
};
use trev::state::tree::{
    ChildrenState,
    TreeNode,
    TreeOptions,
    TreeState,
};
use trev::tree::builder::TreeBuilder;

/// Helper: create a file node.
fn file_node(name: &str, parent: &Path) -> TreeNode {
    TreeNode {
        name: name.to_string(),
        path: parent.join(name),
        is_dir: false,
        is_symlink: false,
        symlink_target: None,
        is_orphan: false,
        size: 100,
        modified: None,
        recursive_max_mtime: None,
        children: ChildrenState::NotLoaded,
        is_expanded: false,
        is_ignored: false,
        is_root: false,
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
        is_orphan: false,
        size: 0,
        modified: None,
        recursive_max_mtime: None,
        children: ChildrenState::Loaded(children),
        is_expanded: false,
        is_ignored: false,
        is_root: false,
    }
}

/// Helper: create a `TreeState` with root expanded.
fn state_with_children(children: Vec<TreeNode>) -> TreeState {
    let root = TreeNode {
        name: "root".to_string(),
        path: "/perf/root".into(),
        is_dir: true,
        is_symlink: false,
        symlink_target: None,
        is_orphan: false,
        size: 0,
        modified: None,
        recursive_max_mtime: None,
        children: ChildrenState::Loaded(children),
        is_expanded: true,
        is_ignored: false,
        is_root: true,
    };
    TreeState::new(root, TreeOptions::default())
}

fn bench_visible_nodes_10k_flat(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");
    let children: Vec<TreeNode> =
        (0..10_000).map(|i| file_node(&format!("file{i:05}.txt"), root_path)).collect();
    let state = state_with_children(children);

    c.bench_function("visible_nodes_10k_flat", |b| {
        b.iter(|| state.visible_nodes());
    });
}

fn bench_visible_nodes_100k_flat(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");
    let children: Vec<TreeNode> =
        (0..100_000).map(|i| file_node(&format!("file{i:06}.txt"), root_path)).collect();
    let state = state_with_children(children);

    c.bench_function("visible_nodes_100k_flat", |b| {
        b.iter(|| state.visible_nodes());
    });
}

fn bench_visible_nodes_100k_nested(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");
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
    let state = state_with_children(children);

    c.bench_function("visible_nodes_100k_nested", |b| {
        b.iter(|| state.visible_nodes());
    });
}

fn bench_visible_node_count_100k(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");
    let children: Vec<TreeNode> =
        (0..100_000).map(|i| file_node(&format!("file{i:06}.txt"), root_path)).collect();
    let state = state_with_children(children);

    c.bench_function("visible_node_count_100k", |b| {
        b.iter(|| state.visible_node_count());
    });
}

fn bench_expand_subtree_1000_dirs(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");

    c.bench_function("expand_subtree_1000_dirs", |b| {
        b.iter_batched(
            || {
                let children: Vec<TreeNode> = (0..1000)
                    .map(|d| {
                        let dir_path = root_path.join(format!("dir{d:04}"));
                        let mut sub: Vec<TreeNode> = (0..10)
                            .map(|f| file_node(&format!("file{f:02}.txt"), &dir_path))
                            .collect();
                        sub.push(dir_node("sub_a", &dir_path, vec![]));
                        sub.push(dir_node("sub_b", &dir_path, vec![]));
                        dir_node(&format!("dir{d:04}"), root_path, sub)
                    })
                    .collect();
                state_with_children(children)
            },
            |mut state| {
                state.expand_subtree(root_path, 5000);
            },
            criterion::BatchSize::SmallInput,
        );
    });
}

fn bench_build_100k_files(c: &mut Criterion) {
    let dir = tempfile::TempDir::new().unwrap();
    for i in 0..100_000 {
        std::fs::write(dir.path().join(format!("file{i:06}.txt")), "content").unwrap();
    }
    let builder = TreeBuilder::new(false, false);

    c.bench_function("build_100k_files", |b| {
        b.iter(|| {
            builder.build(dir.path()).unwrap();
        });
    });
}

fn bench_toggle_expand_100k(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");

    c.bench_function("toggle_expand_100k_children", |b| {
        b.iter_batched(
            || {
                // Directory at cursor with 100k loaded children (collapsed).
                let dir_children: Vec<TreeNode> = (0..100_000)
                    .map(|i| file_node(&format!("file{i:06}.txt"), &root_path.join("target")))
                    .collect();
                let target = dir_node("target", root_path, dir_children);
                state_with_children(vec![target])
            },
            |mut state| {
                // Expand (cursor=0 → the directory).
                state.toggle_expand(0);
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

fn bench_set_children_100k(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");

    c.bench_function("set_children_100k", |b| {
        b.iter_batched(
            || {
                // Create a state with an expanded, loading directory.
                let target = TreeNode {
                    name: "target".to_string(),
                    path: root_path.join("target"),
                    is_dir: true,
                    is_symlink: false,
                    symlink_target: None,
                    is_orphan: false,
                    size: 0,
                    modified: None,
                    recursive_max_mtime: None,
                    children: ChildrenState::Loading,
                    is_expanded: true,
                    is_ignored: false,
                    is_root: false,
                };
                let state = state_with_children(vec![target]);
                // Children to be inserted (unsorted).
                let children: Vec<TreeNode> = (0..100_000)
                    .rev()
                    .map(|i| file_node(&format!("file{i:06}.txt"), &root_path.join("target")))
                    .collect();
                (state, children)
            },
            |(mut state, children)| {
                state.set_children(&root_path.join("target"), children, false);
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

fn bench_visible_node_count_filtered_1k(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");
    let children: Vec<TreeNode> =
        (0..10_000).map(|i| file_node(&format!("file{i:05}.txt"), root_path)).collect();
    let mut state = state_with_children(children);

    // Build filter containing ~1k paths (every 10th file + root ancestors).
    let mut filter: HashSet<std::path::PathBuf> =
        (0..10_000).step_by(10).map(|i| root_path.join(format!("file{i:05}.txt"))).collect();
    filter.insert(root_path.to_path_buf());
    state.set_search_filter(filter);

    c.bench_function("visible_node_count_filtered_1k", |b| {
        b.iter(|| state.visible_node_count());
    });
}

fn bench_visible_nodes_in_range_filtered_10k(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");
    let children: Vec<TreeNode> =
        (0..100_000).map(|i| file_node(&format!("file{i:06}.txt"), root_path)).collect();
    let mut state = state_with_children(children);

    // Broad filter: ~10k paths.
    let mut filter: HashSet<std::path::PathBuf> =
        (0..100_000).step_by(10).map(|i| root_path.join(format!("file{i:06}.txt"))).collect();
    filter.insert(root_path.to_path_buf());
    state.set_search_filter(filter);

    c.bench_function("visible_nodes_in_range_filtered_10k", |b| {
        b.iter(|| state.visible_nodes_in_range(0, 50));
    });
}

fn bench_load_children_100k(c: &mut Criterion) {
    let dir = tempfile::TempDir::new().unwrap();
    for i in 0..100_000 {
        std::fs::write(dir.path().join(format!("file{i:06}.txt")), "content").unwrap();
    }
    let builder = TreeBuilder::new(false, false);

    c.bench_function("load_children_100k", |b| {
        b.iter(|| {
            builder.load_children(dir.path()).unwrap();
        });
    });
}

fn bench_visible_node_count_1m(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");
    let children: Vec<TreeNode> =
        (0..1_000_000).map(|i| file_node(&format!("file{i:07}.txt"), root_path)).collect();
    let state = state_with_children(children);

    c.bench_function("visible_node_count_1m", |b| {
        b.iter(|| state.visible_node_count());
    });
}

fn bench_set_children_1m_fresh(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");

    c.bench_function("set_children_1m_fresh", |b| {
        b.iter_batched(
            || {
                let target = TreeNode {
                    name: "target".to_string(),
                    path: root_path.join("target"),
                    is_dir: true,
                    is_symlink: false,
                    symlink_target: None,
                    is_orphan: false,
                    size: 0,
                    modified: None,
                    recursive_max_mtime: None,
                    children: ChildrenState::Loading,
                    is_expanded: true,
                    is_ignored: false,
                    is_root: false,
                };
                let state = state_with_children(vec![target]);
                let children: Vec<TreeNode> = (0..1_000_000)
                    .rev()
                    .map(|i| file_node(&format!("file{i:07}.txt"), &root_path.join("target")))
                    .collect();
                (state, children)
            },
            |(mut state, children)| {
                state.set_children(&root_path.join("target"), children, false);
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

fn bench_set_children_1m_refresh(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");
    let target_path = root_path.join("target");

    c.bench_function("set_children_1m_refresh", |b| {
        b.iter_batched(
            || {
                // Old children already loaded (simulates watcher reload).
                let old_children: Vec<TreeNode> = (0..1_000_000)
                    .map(|i| file_node(&format!("file{i:07}.txt"), &target_path))
                    .collect();
                let target = TreeNode {
                    name: "target".to_string(),
                    path: target_path.clone(),
                    is_dir: true,
                    is_symlink: false,
                    symlink_target: None,
                    is_orphan: false,
                    size: 0,
                    modified: None,
                    recursive_max_mtime: None,
                    children: ChildrenState::Loaded(old_children),
                    is_expanded: true,
                    is_ignored: false,
                    is_root: false,
                };
                let state = state_with_children(vec![target]);
                // New children (same set, slightly different).
                let new_children: Vec<TreeNode> = (0..1_000_000)
                    .rev()
                    .map(|i| file_node(&format!("file{i:07}.txt"), &target_path))
                    .collect();
                (state, new_children)
            },
            |(mut state, children)| {
                state.set_children(&target_path, children, false);
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

fn bench_set_children_1m_dirs_refresh(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");
    let target_path = root_path.join("target");

    c.bench_function("set_children_1m_dirs_refresh", |b| {
        b.iter_batched(
            || {
                // Old: 1M directories (triggers O(N²) transfer_expansion_state).
                let old_children: Vec<TreeNode> = (0..1_000_000)
                    .map(|i| dir_node(&format!("dir{i:07}"), &target_path, vec![]))
                    .collect();
                let target = TreeNode {
                    name: "target".to_string(),
                    path: target_path.clone(),
                    is_dir: true,
                    is_symlink: false,
                    symlink_target: None,
                    is_orphan: false,
                    size: 0,
                    modified: None,
                    recursive_max_mtime: None,
                    children: ChildrenState::Loaded(old_children),
                    is_expanded: true,
                    is_ignored: false,
                    is_root: false,
                };
                let state = state_with_children(vec![target]);
                let new_children: Vec<TreeNode> = (0..1_000_000)
                    .rev()
                    .map(|i| dir_node(&format!("dir{i:07}"), &target_path, vec![]))
                    .collect();
                (state, new_children)
            },
            |(mut state, children)| {
                state.set_children(&target_path, children, false);
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

fn bench_sort_1m(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");
    let opts = TreeOptions::default();

    c.bench_function("sort_1m", |b| {
        b.iter_batched(
            || {
                (0..1_000_000)
                    .rev()
                    .map(|i| file_node(&format!("file{i:07}.txt"), root_path))
                    .collect::<Vec<TreeNode>>()
            },
            |mut children| {
                trev::tree::sort::sort_children(
                    &mut children,
                    opts.sort_order,
                    opts.sort_direction,
                    opts.directories_first,
                );
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

fn bench_sort_1m_unstable(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");

    c.bench_function("sort_1m_unstable", |b| {
        b.iter_batched(
            || {
                (0..1_000_000)
                    .rev()
                    .map(|i| file_node(&format!("file{i:07}.txt"), root_path))
                    .collect::<Vec<TreeNode>>()
            },
            |mut children| {
                children
                    .sort_unstable_by(|a, b| trev::tree::sort::compare_smart_pub(&a.name, &b.name));
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

fn bench_sort_1m_cached_key(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");

    c.bench_function("sort_1m_cached_key", |b| {
        b.iter_batched(
            || {
                (0..1_000_000)
                    .rev()
                    .map(|i| file_node(&format!("file{i:07}.txt"), root_path))
                    .collect::<Vec<TreeNode>>()
            },
            |mut children| {
                children.sort_by_cached_key(|n| n.name.to_lowercase());
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

fn bench_sort_1m_plain_cmp(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");

    c.bench_function("sort_1m_plain_cmp", |b| {
        b.iter_batched(
            || {
                (0..1_000_000)
                    .rev()
                    .map(|i| file_node(&format!("file{i:07}.txt"), root_path))
                    .collect::<Vec<TreeNode>>()
            },
            |mut children| {
                children.sort_unstable_by(|a, b| a.name.cmp(&b.name));
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

fn bench_move_cursor_to_path_1m(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");
    let children: Vec<TreeNode> =
        (0..1_000_000).map(|i| file_node(&format!("file{i:07}.txt"), root_path)).collect();
    let mut state = state_with_children(children);
    let target = root_path.join("file0500000.txt");

    c.bench_function("move_cursor_to_path_1m", |b| {
        b.iter(|| {
            state.move_cursor_to_path(&target);
        });
    });
}

criterion_group!(
    fast_benches,
    bench_visible_nodes_10k_flat,
    bench_visible_nodes_100k_flat,
    bench_visible_nodes_100k_nested,
    bench_visible_node_count_100k,
    bench_visible_node_count_filtered_1k,
    bench_visible_nodes_in_range_filtered_10k,
    bench_expand_subtree_1000_dirs,
    bench_toggle_expand_100k,
    bench_set_children_100k,
);

fn bench_visible_node_count_10m(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");
    let children: Vec<TreeNode> =
        (0..10_000_000).map(|i| file_node(&format!("file{i:08}.txt"), root_path)).collect();
    let state = state_with_children(children);

    c.bench_function("visible_node_count_10m", |b| {
        b.iter(|| state.visible_node_count());
    });
}

fn bench_set_children_10m_fresh(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");

    c.bench_function("set_children_10m_fresh", |b| {
        b.iter_batched(
            || {
                let target = TreeNode {
                    name: "target".to_string(),
                    path: root_path.join("target"),
                    is_dir: true,
                    is_symlink: false,
                    symlink_target: None,
                    is_orphan: false,
                    size: 0,
                    modified: None,
                    recursive_max_mtime: None,
                    children: ChildrenState::Loading,
                    is_expanded: true,
                    is_ignored: false,
                    is_root: false,
                };
                let state = state_with_children(vec![target]);
                let children: Vec<TreeNode> = (0..10_000_000)
                    .rev()
                    .map(|i| file_node(&format!("file{i:08}.txt"), &root_path.join("target")))
                    .collect();
                (state, children)
            },
            |(mut state, children)| {
                state.set_children(&root_path.join("target"), children, false);
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

fn bench_move_cursor_to_path_10m(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");
    let children: Vec<TreeNode> =
        (0..10_000_000).map(|i| file_node(&format!("file{i:08}.txt"), root_path)).collect();
    let mut state = state_with_children(children);
    let target = root_path.join("file05000000.txt");

    c.bench_function("move_cursor_to_path_10m", |b| {
        b.iter(|| {
            state.move_cursor_to_path(&target);
        });
    });
}

fn bench_visible_nodes_in_range_10m(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");
    let children: Vec<TreeNode> =
        (0..10_000_000).map(|i| file_node(&format!("file{i:08}.txt"), root_path)).collect();
    let state = state_with_children(children);

    c.bench_function("visible_nodes_in_range_10m", |b| {
        b.iter(|| state.visible_nodes_in_range(5_000_000, 50));
    });
}

fn bench_visible_node_count_filtered_10m(c: &mut Criterion) {
    let root_path = Path::new("/perf/root");
    let children: Vec<TreeNode> =
        (0..10_000_000).map(|i| file_node(&format!("file{i:08}.txt"), root_path)).collect();
    let mut state = state_with_children(children);

    // Filter: every 1000th file = ~10k visible paths.
    let mut filter: HashSet<std::path::PathBuf> =
        (0..10_000_000).step_by(1000).map(|i| root_path.join(format!("file{i:08}.txt"))).collect();
    filter.insert(root_path.to_path_buf());
    state.set_search_filter(filter);

    c.bench_function("visible_node_count_filtered_10m", |b| {
        b.iter(|| state.visible_node_count());
    });
}

criterion_group! {
    name = large_benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(5));
    targets =
        bench_visible_node_count_1m,
        bench_sort_1m,
        bench_sort_1m_unstable,
        bench_sort_1m_cached_key,
        bench_sort_1m_plain_cmp,
        bench_set_children_1m_fresh,
        bench_set_children_1m_refresh,
        bench_set_children_1m_dirs_refresh,
        bench_move_cursor_to_path_1m
}

criterion_group! {
    name = huge_benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(10));
    targets =
        bench_visible_node_count_10m,
        bench_set_children_10m_fresh,
        bench_move_cursor_to_path_10m,
        bench_visible_nodes_in_range_10m,
        bench_visible_node_count_filtered_10m
}

criterion_group! {
    name = io_benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(3));
    targets = bench_build_100k_files, bench_load_children_100k
}

criterion_main!(fast_benches, large_benches, huge_benches, io_benches);
