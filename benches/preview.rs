#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]

use std::fmt::Write as _;
use std::hint::black_box;
use std::path::PathBuf;

use criterion::{
    Criterion,
    criterion_group,
    criterion_main,
};
use trev::preview::cache::{
    CacheKey,
    PreviewCache,
};
use trev::preview::content::PreviewContent;
use trev::preview::highlight::highlight_lines;

fn bench_highlight_1000_lines(c: &mut Criterion) {
    let mut code = String::new();
    for i in 0..1000 {
        let _ = writeln!(code, "fn func_{i}() {{ let x = {i}; }}");
    }

    c.bench_function("highlight_1000_lines", |b| {
        b.iter(|| highlight_lines(&code, "rs"));
    });
}

fn bench_cache_hit(c: &mut Criterion) {
    let mut cache = PreviewCache::new(100);
    for i in 0..50 {
        let key = CacheKey {
            path: PathBuf::from(format!("/test/file_{i}.rs")),
            provider_name: "text".to_string(),
        };
        cache.put(key, PreviewContent::Empty);
    }
    let key =
        CacheKey { path: PathBuf::from("/test/file_25.rs"), provider_name: "text".to_string() };

    c.bench_function("cache_hit", |b| {
        b.iter(|| {
            let _ = black_box(cache.get(&key).is_some());
        });
    });
}

criterion_group!(benches, bench_highlight_1000_lines, bench_cache_hit,);
criterion_main!(benches);
