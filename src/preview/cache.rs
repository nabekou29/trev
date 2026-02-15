//! LRU cache for preview content.

use std::path::PathBuf;

use lru::LruCache;

use super::content::PreviewContent;

/// Key for the preview cache — unique per (path, provider) pair.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    /// File or directory path.
    pub path: PathBuf,
    /// Provider name that produced the content.
    pub provider_name: String,
}

/// LRU cache storing loaded preview content.
///
/// Caches `(path, provider_name) → PreviewContent` to avoid re-loading
/// files that have already been previewed.
#[derive(Debug)]
pub struct PreviewCache {
    /// The underlying LRU cache.
    cache: LruCache<CacheKey, PreviewContent>,
}

impl PreviewCache {
    /// Create a new cache with the given capacity.
    pub fn new(capacity: usize) -> Self {
        // 10 is always non-zero, so this is compile-time safe.
        const DEFAULT_CAP: std::num::NonZeroUsize = match std::num::NonZeroUsize::new(10) {
            Some(v) => v,
            None => unreachable!(),
        };
        let cap = std::num::NonZeroUsize::new(capacity).unwrap_or(DEFAULT_CAP);
        Self { cache: LruCache::new(cap) }
    }

    /// Look up a cached preview content by key.
    ///
    /// Promotes the entry to most-recently-used on hit.
    pub fn get(&mut self, key: &CacheKey) -> Option<&PreviewContent> {
        self.cache.get(key)
    }

    /// Insert a preview content into the cache.
    ///
    /// Evicts the least-recently-used entry if at capacity.
    pub fn put(&mut self, key: CacheKey, content: PreviewContent) {
        self.cache.put(key, content);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    fn make_key(path: &str, provider: &str) -> CacheKey {
        CacheKey { path: PathBuf::from(path), provider_name: provider.to_string() }
    }

    #[rstest]
    fn put_and_get_roundtrip() {
        let mut cache = PreviewCache::new(10);
        let key = make_key("/test/file.rs", "text");
        cache.put(key.clone(), PreviewContent::Empty);

        assert_that!(cache.get(&key).is_some(), eq(true));
    }

    #[rstest]
    fn get_missing_returns_none() {
        let mut cache = PreviewCache::new(10);
        let key = make_key("/nonexistent", "text");

        assert_that!(cache.get(&key).is_none(), eq(true));
    }

    #[rstest]
    fn lru_eviction_at_capacity() {
        let mut cache = PreviewCache::new(2);
        let key_a = make_key("/a", "text");
        let key_b = make_key("/b", "text");
        let key_c = make_key("/c", "text");

        cache.put(key_a.clone(), PreviewContent::Empty);
        cache.put(key_b, PreviewContent::Empty);
        // This should evict key_a (least recently used).
        cache.put(key_c, PreviewContent::Empty);

        assert_that!(cache.get(&key_a).is_none(), eq(true));
    }

    #[rstest]
    #[ignore = "Performance test — run with `cargo test -- --ignored`"]
    fn perf_cache_hit_under_1ms() {
        let mut cache = PreviewCache::new(100);

        // Populate cache with entries.
        for i in 0..50 {
            let key = make_key(&format!("/test/file_{i}.rs"), "text");
            cache.put(key, PreviewContent::Empty);
        }

        // Measure cache hit time over many lookups.
        let key = make_key("/test/file_25.rs", "text");
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            let _ = cache.get(&key);
        }
        let elapsed = start.elapsed();
        let per_op = elapsed / 10_000;

        assert!(per_op.as_micros() < 1000, "Cache hit took {per_op:?} per op, expected < 1ms");
    }

    #[rstest]
    fn different_provider_same_path_stores_separately() {
        let mut cache = PreviewCache::new(10);
        let key_text = make_key("/file.svg", "text");
        let key_image = make_key("/file.svg", "image");

        cache.put(
            key_text.clone(),
            PreviewContent::PlainText { lines: vec!["<svg>".to_string()], truncated: false },
        );
        cache.put(key_image.clone(), PreviewContent::Empty);

        // Both should be present.
        assert_that!(cache.get(&key_text).is_some(), eq(true));
        assert_that!(cache.get(&key_image).is_some(), eq(true));
    }
}
