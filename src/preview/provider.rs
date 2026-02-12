//! Preview provider trait and registry.

use std::path::Path;

use tokio_util::sync::CancellationToken;

use super::content::PreviewContent;

/// Context passed to a provider's `load()` method.
#[derive(Debug, Clone)]
pub struct LoadContext {
    /// Maximum number of lines to read.
    pub max_lines: usize,
    /// Maximum number of bytes to read.
    pub max_bytes: u64,
    /// Token to check for cancellation during loading.
    pub cancel_token: CancellationToken,
}

/// A provider that can generate preview content for a file.
///
/// Providers are registered with a `PreviewRegistry` and selected
/// based on `can_handle()` results and priority ordering.
pub trait PreviewProvider: Send + Sync {
    /// Human-readable name of this provider (e.g., "Text", "Image").
    fn name(&self) -> &'static str;

    /// Priority for ordering. Lower values = higher priority.
    fn priority(&self) -> u32;

    /// Whether this provider can handle the given path.
    fn can_handle(&self, path: &Path, is_dir: bool) -> bool;

    /// Whether this provider should be included in the switchable list,
    /// given the names of all providers that `can_handle()` returned true.
    ///
    /// Default: always enabled. Override for providers like Fallback
    /// that should only appear when no other provider is available.
    fn is_enabled(&self, _available: &[&str]) -> bool {
        true
    }

    /// Load preview content for the given path.
    ///
    /// This is called from a blocking context (`spawn_blocking`).
    fn load(&self, path: &Path, ctx: &LoadContext) -> anyhow::Result<PreviewContent>;
}

/// Registry of preview providers, sorted by priority.
///
/// Resolves which providers are available for a given file and
/// filters out those that opt out via `is_enabled()`.
#[derive(Debug)]
pub struct PreviewRegistry {
    /// Providers sorted by priority (ascending — lower number = higher priority).
    providers: Vec<Box<dyn PreviewProvider>>,
}

impl PreviewRegistry {
    /// Create a new registry with the given providers.
    ///
    /// Providers are sorted by priority automatically.
    pub fn new(mut providers: Vec<Box<dyn PreviewProvider>>) -> Self {
        providers.sort_by_key(|p| p.priority());
        Self { providers }
    }

    /// Resolve which providers can handle the given path and are enabled.
    ///
    /// 1. Collect all providers where `can_handle()` returns true (priority order).
    /// 2. Build the names list of candidates.
    /// 3. Filter by `is_enabled(&names)` to produce the final switchable list.
    pub fn resolve(&self, path: &Path, is_dir: bool) -> Vec<&dyn PreviewProvider> {
        // Step 1: collect candidates.
        let candidates: Vec<&dyn PreviewProvider> = self
            .providers
            .iter()
            .filter(|p| p.can_handle(path, is_dir))
            .map(AsRef::as_ref)
            .collect();

        // Step 2: build names list.
        let names: Vec<&str> = candidates.iter().map(|p| p.name()).collect();

        // Step 3: filter by is_enabled.
        candidates
            .into_iter()
            .filter(|p| p.is_enabled(&names))
            .collect()
    }
}

impl std::fmt::Debug for dyn PreviewProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreviewProvider")
            .field("name", &self.name())
            .field("priority", &self.priority())
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::path::PathBuf;

    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    /// A test provider for registry tests.
    struct TestProvider {
        name: &'static str,
        priority: u32,
        handles: bool,
        enabled_fn: Option<Box<dyn Fn(&[&str]) -> bool + Send + Sync>>,
    }

    impl PreviewProvider for TestProvider {
        fn name(&self) -> &'static str {
            &self.name
        }

        fn priority(&self) -> u32 {
            self.priority
        }

        fn can_handle(&self, _path: &Path, _is_dir: bool) -> bool {
            self.handles
        }

        fn is_enabled(&self, available: &[&str]) -> bool {
            self.enabled_fn
                .as_ref()
                .map_or(true, |f| f(available))
        }

        fn load(&self, _path: &Path, _ctx: &LoadContext) -> anyhow::Result<PreviewContent> {
            Ok(PreviewContent::Empty)
        }
    }

    fn make_provider(name: &'static str, priority: u32, handles: bool) -> Box<dyn PreviewProvider> {
        Box::new(TestProvider {
            name,
            priority,
            handles,
            enabled_fn: None,
        })
    }

    fn make_fallback_provider() -> Box<dyn PreviewProvider> {
        Box::new(TestProvider {
            name: "Fallback",
            priority: 100,
            handles: true,
            enabled_fn: Some(Box::new(|available: &[&str]| available.len() <= 1)),
        })
    }

    #[rstest]
    fn resolve_returns_matching_providers_in_priority_order() {
        let registry = PreviewRegistry::new(vec![
            make_provider("Text", 30, true),
            make_provider("Image", 20, true),
        ]);

        let result = registry.resolve(&PathBuf::from("/test.svg"), false);
        assert_that!(result.len(), eq(2));
        assert_that!(result[0].name(), eq("Image"));
        assert_that!(result[1].name(), eq("Text"));
    }

    #[rstest]
    fn resolve_filters_non_matching_providers() {
        let registry = PreviewRegistry::new(vec![
            make_provider("Text", 30, true),
            make_provider("Image", 20, false),
        ]);

        let result = registry.resolve(&PathBuf::from("/test.rs"), false);
        assert_that!(result.len(), eq(1));
        assert_that!(result[0].name(), eq("Text"));
    }

    #[rstest]
    fn resolve_excludes_fallback_when_others_exist() {
        let registry = PreviewRegistry::new(vec![
            make_provider("Text", 30, true),
            make_fallback_provider(),
        ]);

        let result = registry.resolve(&PathBuf::from("/test.rs"), false);
        assert_that!(result.len(), eq(1));
        assert_that!(result[0].name(), eq("Text"));
    }

    #[rstest]
    fn resolve_includes_fallback_when_sole_provider() {
        let registry = PreviewRegistry::new(vec![
            make_provider("Text", 30, false),
            make_fallback_provider(),
        ]);

        let result = registry.resolve(&PathBuf::from("/test.bin"), false);
        assert_that!(result.len(), eq(1));
        assert_that!(result[0].name(), eq("Fallback"));
    }

    #[rstest]
    fn resolve_empty_when_nothing_matches() {
        let registry = PreviewRegistry::new(vec![make_provider("Text", 30, false)]);

        let result = registry.resolve(&PathBuf::from("/test.bin"), false);
        assert_that!(result.is_empty(), eq(true));
    }
}
