#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]

use std::collections::HashSet;
use std::path::{
    Path,
    PathBuf,
};

use criterion::{
    Criterion,
    criterion_group,
    criterion_main,
};
use trev::input::SearchMode;
use trev::tree::search_engine::{
    self,
    SearchResult,
};
use trev::tree::search_index::SearchEntry;

/// Helper: create a flat list of `SearchEntry` values under `root`.
fn make_entries(root: &Path, count: usize) -> Vec<SearchEntry> {
    (0..count)
        .map(|i| {
            let name = format!("file{i:08}.txt");
            SearchEntry { path: root.join(&name), name, is_dir: false }
        })
        .collect()
}

/// Helper: create nested entries (dirs × `files_per_dir`) under `root`.
fn make_nested_entries(root: &Path, dirs: usize, files_per_dir: usize) -> Vec<SearchEntry> {
    let mut entries = Vec::with_capacity(dirs * (files_per_dir + 1));
    for d in 0..dirs {
        let dir_name = format!("dir{d:06}");
        let dir_path = root.join(&dir_name);
        entries.push(SearchEntry { path: dir_path.clone(), name: dir_name, is_dir: true });
        for f in 0..files_per_dir {
            let name = format!("file{f:06}.txt");
            entries.push(SearchEntry { path: dir_path.join(&name), name, is_dir: false });
        }
    }
    entries
}

// ---------------------------------------------------------------------------
// search() benchmarks
// ---------------------------------------------------------------------------

fn bench_search_100k(c: &mut Criterion) {
    let root = Path::new("/bench/root");
    let entries = make_entries(root, 100_000);

    c.bench_function("search_100k_entries", |b| {
        b.iter(|| {
            let results = search_engine::search(&entries, "file0005", root, SearchMode::Name);
            std::hint::black_box(results.len());
        });
    });
}

fn bench_search_1m(c: &mut Criterion) {
    let root = Path::new("/bench/root");
    let entries = make_entries(root, 1_000_000);

    c.bench_function("search_1m_entries", |b| {
        b.iter(|| {
            let results = search_engine::search(&entries, "file0005", root, SearchMode::Name);
            std::hint::black_box(results.len());
        });
    });
}

fn bench_search_10m(c: &mut Criterion) {
    let root = Path::new("/bench/root");
    let entries = make_entries(root, 10_000_000);

    c.bench_function("search_10m_entries", |b| {
        b.iter(|| {
            let results = search_engine::search(&entries, "file0005", root, SearchMode::Name);
            std::hint::black_box(results.len());
        });
    });
}

fn bench_search_10m_path_mode(c: &mut Criterion) {
    let root = Path::new("/bench/root");
    let entries = make_nested_entries(root, 10_000, 1_000);

    c.bench_function("search_10m_entries_path_mode", |b| {
        b.iter(|| {
            let results = search_engine::search(&entries, "dir0001/file", root, SearchMode::Path);
            std::hint::black_box(results.len());
        });
    });
}

fn bench_search_10m_few_matches(c: &mut Criterion) {
    let root = Path::new("/bench/root");
    let entries = make_entries(root, 10_000_000);

    // Very specific query that matches very few entries.
    c.bench_function("search_10m_few_matches", |b| {
        b.iter(|| {
            let results = search_engine::search(&entries, "^file00000001", root, SearchMode::Name);
            std::hint::black_box(results.len());
        });
    });
}

fn bench_search_10m_many_matches(c: &mut Criterion) {
    let root = Path::new("/bench/root");
    let entries = make_entries(root, 10_000_000);

    // Broad query that matches most entries.
    c.bench_function("search_10m_many_matches", |b| {
        b.iter(|| {
            let results = search_engine::search(&entries, "file", root, SearchMode::Name);
            std::hint::black_box(results.len());
        });
    });
}

// ---------------------------------------------------------------------------
// NucleoSearchEngine benchmarks (async parallel)
// ---------------------------------------------------------------------------

use std::sync::Arc;

use trev::tree::search_engine::{
    MAX_SEARCH_RESULTS,
    NucleoSearchEngine,
    inject_entry,
};

fn inject_all(engine: &NucleoSearchEngine, entries: &[SearchEntry], root: &Path) {
    let injector = engine.injector();
    for entry in entries {
        inject_entry(&injector, entry.clone(), root);
    }
}

fn tick_until_done(engine: &mut NucleoSearchEngine) {
    for _ in 0..1000 {
        let status = engine.tick(50);
        if !status.running {
            break;
        }
    }
}

fn bench_nucleo_search_100k(c: &mut Criterion) {
    let root = Path::new("/bench/root");
    let entries = make_entries(root, 100_000);

    c.bench_function("nucleo_search_100k", |b| {
        b.iter_batched(
            || {
                let mut engine = NucleoSearchEngine::new(Arc::new(|| {}));
                inject_all(&engine, &entries, root);
                tick_until_done(&mut engine);
                engine
            },
            |mut engine| {
                engine.update_pattern("file0005", SearchMode::Name);
                tick_until_done(&mut engine);
                let results = engine.collect_results(SearchMode::Name, MAX_SEARCH_RESULTS);
                std::hint::black_box(results.len());
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

fn bench_nucleo_search_1m(c: &mut Criterion) {
    let root = Path::new("/bench/root");
    let entries = make_entries(root, 1_000_000);

    c.bench_function("nucleo_search_1m", |b| {
        b.iter_batched(
            || {
                let mut engine = NucleoSearchEngine::new(Arc::new(|| {}));
                inject_all(&engine, &entries, root);
                tick_until_done(&mut engine);
                engine
            },
            |mut engine| {
                engine.update_pattern("file0005", SearchMode::Name);
                tick_until_done(&mut engine);
                let results = engine.collect_results(SearchMode::Name, MAX_SEARCH_RESULTS);
                std::hint::black_box(results.len());
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

fn bench_nucleo_search_10m(c: &mut Criterion) {
    let root = Path::new("/bench/root");
    let entries = make_entries(root, 10_000_000);

    c.bench_function("nucleo_search_10m", |b| {
        b.iter_batched(
            || {
                let mut engine = NucleoSearchEngine::new(Arc::new(|| {}));
                inject_all(&engine, &entries, root);
                tick_until_done(&mut engine);
                engine
            },
            |mut engine| {
                engine.update_pattern("file0005", SearchMode::Name);
                tick_until_done(&mut engine);
                let results = engine.collect_results(SearchMode::Name, MAX_SEARCH_RESULTS);
                std::hint::black_box(results.len());
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

fn bench_nucleo_search_10m_path_mode(c: &mut Criterion) {
    let root = Path::new("/bench/root");
    let entries = make_nested_entries(root, 10_000, 1_000);

    c.bench_function("nucleo_search_10m_path_mode", |b| {
        b.iter_batched(
            || {
                let mut engine = NucleoSearchEngine::new(Arc::new(|| {}));
                inject_all(&engine, &entries, root);
                tick_until_done(&mut engine);
                engine
            },
            |mut engine| {
                engine.update_pattern("dir0001/file", SearchMode::Path);
                tick_until_done(&mut engine);
                let results = engine.collect_results(SearchMode::Path, MAX_SEARCH_RESULTS);
                std::hint::black_box(results.len());
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

fn bench_nucleo_incremental_keystrokes_1m(c: &mut Criterion) {
    let root = Path::new("/bench/root");
    let entries = make_entries(root, 1_000_000);

    c.bench_function("nucleo_incremental_keystrokes_1m", |b| {
        b.iter_batched(
            || {
                let mut engine = NucleoSearchEngine::new(Arc::new(|| {}));
                inject_all(&engine, &entries, root);
                tick_until_done(&mut engine);
                engine
            },
            |mut engine| {
                // Simulate typing "file0005" one character at a time.
                for query in &["f", "fi", "fil", "file", "file0", "file00", "file000", "file0005"] {
                    engine.update_pattern(query, SearchMode::Name);
                    tick_until_done(&mut engine);
                }
                let results = engine.collect_results(SearchMode::Name, MAX_SEARCH_RESULTS);
                std::hint::black_box(results.len());
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

fn bench_nucleo_incremental_keystrokes_10m(c: &mut Criterion) {
    let root = Path::new("/bench/root");
    let entries = make_entries(root, 10_000_000);

    c.bench_function("nucleo_incremental_keystrokes_10m", |b| {
        b.iter_batched(
            || {
                let mut engine = NucleoSearchEngine::new(Arc::new(|| {}));
                inject_all(&engine, &entries, root);
                tick_until_done(&mut engine);
                engine
            },
            |mut engine| {
                // Simulate typing "file0005" one character at a time.
                for query in &["f", "fi", "fil", "file", "file0", "file00", "file000", "file0005"] {
                    engine.update_pattern(query, SearchMode::Name);
                    tick_until_done(&mut engine);
                }
                let results = engine.collect_results(SearchMode::Name, MAX_SEARCH_RESULTS);
                std::hint::black_box(results.len());
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

// ---------------------------------------------------------------------------
// compute_visible_paths() benchmarks
// ---------------------------------------------------------------------------

fn bench_compute_visible_paths_1k_results(c: &mut Criterion) {
    let root = Path::new("/bench/root");
    let results: Vec<SearchResult> = (0..1_000)
        .map(|i| SearchResult {
            path: root.join(format!("dir{:04}", i / 10)).join(format!("file{i:06}.txt")),
            is_dir: false,
            score: 100,
            match_indices: vec![],
        })
        .collect();

    c.bench_function("compute_visible_paths_1k_results", |b| {
        b.iter(|| {
            let visible = search_engine::compute_visible_paths(&results, root);
            std::hint::black_box(visible.len());
        });
    });
}

fn bench_compute_visible_paths_100k_results(c: &mut Criterion) {
    let root = Path::new("/bench/root");
    let results: Vec<SearchResult> = (0..100_000)
        .map(|i| SearchResult {
            path: root.join(format!("dir{:06}", i / 100)).join(format!("file{i:08}.txt")),
            is_dir: false,
            score: 100,
            match_indices: vec![],
        })
        .collect();

    c.bench_function("compute_visible_paths_100k_results", |b| {
        b.iter(|| {
            let visible = search_engine::compute_visible_paths(&results, root);
            std::hint::black_box(visible.len());
        });
    });
}

fn bench_compute_visible_paths_1m_deep(c: &mut Criterion) {
    let root = Path::new("/bench/root");
    // Deep nesting: root/a/b/c/file — more ancestor walks per result.
    let results: Vec<SearchResult> = (0..1_000_000)
        .map(|i| SearchResult {
            path: root
                .join(format!("a{:04}", i / 10_000))
                .join(format!("b{:04}", i / 100))
                .join(format!("file{i:08}.txt")),
            is_dir: false,
            score: 100,
            match_indices: vec![],
        })
        .collect();

    c.bench_function("compute_visible_paths_1m_deep", |b| {
        b.iter(|| {
            let visible = search_engine::compute_visible_paths(&results, root);
            std::hint::black_box(visible.len());
        });
    });
}

// ---------------------------------------------------------------------------
// set_search_filter() benchmarks (tree integration)
// ---------------------------------------------------------------------------

fn bench_set_search_filter_10k(c: &mut Criterion) {
    use trev::state::tree::{
        ChildrenState,
        TreeNode,
        TreeOptions,
        TreeState,
    };

    let root_path = Path::new("/bench/root");
    let children: Vec<TreeNode> = (0..100_000)
        .map(|i| TreeNode {
            name: format!("file{i:06}.txt"),
            path: root_path.join(format!("file{i:06}.txt")),
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
        })
        .collect();

    let root = TreeNode {
        name: "root".to_string(),
        path: root_path.into(),
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

    // Filter: every 10th file.
    let filter: HashSet<PathBuf> = (0..100_000)
        .step_by(10)
        .map(|i| root_path.join(format!("file{i:06}.txt")))
        .chain(std::iter::once(root_path.to_path_buf()))
        .collect();

    c.bench_function("set_search_filter_10k_of_100k", |b| {
        b.iter_batched(
            || TreeState::new(root.clone(), TreeOptions::default()),
            |mut state| {
                state.set_search_filter(filter.clone());
                std::hint::black_box(state.visible_node_count());
            },
            criterion::BatchSize::LargeInput,
        );
    });
}

criterion_group! {
    name = search_benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(5));
    targets =
        bench_search_100k,
        bench_search_1m,
        bench_search_10m,
        bench_search_10m_path_mode,
        bench_search_10m_few_matches,
        bench_search_10m_many_matches
}

criterion_group! {
    name = visible_path_benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(5));
    targets =
        bench_compute_visible_paths_1k_results,
        bench_compute_visible_paths_100k_results,
        bench_compute_visible_paths_1m_deep
}

criterion_group! {
    name = filter_benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(5));
    targets =
        bench_set_search_filter_10k
}

criterion_group! {
    name = nucleo_benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(10));
    targets =
        bench_nucleo_search_100k,
        bench_nucleo_search_1m,
        bench_nucleo_search_10m,
        bench_nucleo_search_10m_path_mode,
        bench_nucleo_incremental_keystrokes_1m,
        bench_nucleo_incremental_keystrokes_10m
}

criterion_main!(search_benches, visible_path_benches, filter_benches, nucleo_benches);
