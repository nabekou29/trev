//! IPC server (Unix Domain Socket).
//!
//! JSON-RPC 2.0 over Unix Domain Socket for bidirectional communication
//! between TUI (server) and clients (Neovim plugin, `trev ctl`).

pub mod client;
pub mod paths;
pub mod server;
pub mod types;
