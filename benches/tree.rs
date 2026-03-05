#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]

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

criterion_group!(
    fast_benches,
    bench_visible_nodes_10k_flat,
    bench_visible_nodes_100k_flat,
    bench_visible_nodes_100k_nested,
    bench_visible_node_count_100k,
    bench_expand_subtree_1000_dirs,
    bench_toggle_expand_100k,
    bench_set_children_100k,
);

criterion_group! {
    name = io_benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(3));
    targets = bench_build_100k_files, bench_load_children_100k
}

criterion_main!(fast_benches, io_benches);
