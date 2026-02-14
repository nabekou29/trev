//! Preview state management and content loading.

#[cfg_attr(not(test), expect(dead_code, reason = "Cache integration pending"))]
pub mod cache;
pub mod content;
pub mod highlight;
pub mod provider;
pub mod providers;
pub mod state;
