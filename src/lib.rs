//! trev library crate — exposes internal modules for benchmarks and integration tests.
//!
//! The application entry point is in `main.rs`.

#![expect(
    unreachable_pub,
    reason = "Sub-modules use `pub` for internal visibility. \
              Conflicts with clippy::redundant_pub_crate from nursery group."
)]
// Internal library exposed only for benchmarks and integration tests.
// These pedantic lints are not meaningful for non-public API.
#![allow(clippy::must_use_candidate, clippy::new_without_default)]

pub mod action;
pub mod app;
pub mod cli;
pub mod config;
pub mod error;
pub mod file_op;
pub mod git;
pub mod input;
pub mod ipc;
pub mod preview;
pub mod session;
pub mod state;
pub mod terminal;
pub mod tree;
pub mod ui;
pub mod watcher;
