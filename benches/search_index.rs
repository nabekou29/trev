#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]

use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;

use criterion::{
    Criterion,
    criterion_group,
    criterion_main,
};
use trev::tree::search_index::{
    DEFAULT_MAX_ENTRIES,
    SearchIndex,
    build_search_index,
};

fn bench_build_search_index_self(c: &mut Criterion) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    c.bench_function("build_search_index_self_repo", |b| {
        b.iter(|| {
            let index = Arc::new(RwLock::new(SearchIndex::new()));
            let cancelled = Arc::new(AtomicBool::new(false));
            build_search_index(&index, root, false, false, &cancelled, DEFAULT_MAX_ENTRIES);
            std::hint::black_box(index.read().unwrap().entries().len());
        });
    });
}

fn bench_build_search_index_self_with_hidden(c: &mut Criterion) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    c.bench_function("build_search_index_self_repo_with_hidden", |b| {
        b.iter(|| {
            let index = Arc::new(RwLock::new(SearchIndex::new()));
            let cancelled = Arc::new(AtomicBool::new(false));
            build_search_index(&index, root, true, false, &cancelled, DEFAULT_MAX_ENTRIES);
            std::hint::black_box(index.read().unwrap().entries().len());
        });
    });
}

fn bench_build_search_index_100k(c: &mut Criterion) {
    let dir = tempfile::TempDir::new().unwrap();
    // Create 100 directories with 1000 files each = 100K files.
    for d in 0..100 {
        let sub = dir.path().join(format!("dir{d:03}"));
        std::fs::create_dir(&sub).unwrap();
        for f in 0..1000 {
            std::fs::write(sub.join(format!("file{f:04}.txt")), "").unwrap();
        }
    }

    c.bench_function("build_search_index_100k_files", |b| {
        b.iter(|| {
            let index = Arc::new(RwLock::new(SearchIndex::new()));
            let cancelled = Arc::new(AtomicBool::new(false));
            build_search_index(&index, dir.path(), false, false, &cancelled, DEFAULT_MAX_ENTRIES);
            std::hint::black_box(index.read().unwrap().entries().len());
        });
    });
}

fn bench_build_search_index_1m(c: &mut Criterion) {
    let dir = tempfile::TempDir::new().unwrap();
    // Create 1000 directories with 1000 files each = 1M files.
    for d in 0..1000 {
        let sub = dir.path().join(format!("dir{d:04}"));
        std::fs::create_dir(&sub).unwrap();
        for f in 0..1000 {
            std::fs::write(sub.join(format!("file{f:04}.txt")), "").unwrap();
        }
    }

    c.bench_function("build_search_index_1m_files", |b| {
        b.iter(|| {
            let index = Arc::new(RwLock::new(SearchIndex::new()));
            let cancelled = Arc::new(AtomicBool::new(false));
            build_search_index(&index, dir.path(), false, false, &cancelled, DEFAULT_MAX_ENTRIES);
            std::hint::black_box(index.read().unwrap().entries().len());
        });
    });
}

criterion_group! {
    name = search_index_benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(5));
    targets = bench_build_search_index_self,
              bench_build_search_index_self_with_hidden,
              bench_build_search_index_100k
}

criterion_group! {
    name = search_index_large_benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(10));
    targets = bench_build_search_index_1m
}

criterion_main!(search_index_benches, search_index_large_benches);
