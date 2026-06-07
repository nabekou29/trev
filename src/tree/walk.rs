//! Shared `ignore::WalkBuilder` configuration for tree and search walks.

use std::path::Path;

use ignore::WalkBuilder;

/// Build a `WalkBuilder` with the project's standard hidden/gitignore filters.
///
/// Centralizes the `hidden` / `git_ignore` / `git_global` / `git_exclude`
/// configuration so the tree builder and the search index share identical
/// visibility rules. Callers add their own `max_depth`, `build()`, or
/// `build_parallel()` as needed.
pub fn configured_walk_builder(path: &Path, show_hidden: bool, show_ignored: bool) -> WalkBuilder {
    let mut builder = WalkBuilder::new(path);
    builder
        .hidden(!show_hidden)
        .git_ignore(!show_ignored)
        .git_global(!show_ignored)
        .git_exclude(!show_ignored);
    if !show_hidden {
        // A gitignore whitelist (`!pattern`) produces a `Match::Whitelist`
        // that overrides the `ignore` crate's implicit `hidden` filter, so an
        // explicitly un-ignored dotfile would otherwise leak through. Drop any
        // dotfile entry to keep the hidden filter authoritative. Returning
        // `false` for a hidden directory also prunes its subtree, matching the
        // `hidden(true)` traversal semantics. The root (depth 0) is never filtered.
        builder.filter_entry(|entry| {
            entry.depth() == 0 || !entry.file_name().to_string_lossy().starts_with('.')
        });
    }
    builder
}
