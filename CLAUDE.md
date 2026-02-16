# trev

Fast TUI file viewer with tree view, syntax-highlighted preview, and Neovim integration.

## Tech Stack

- Rust 2024 edition, nightly-2026-01-24
- TUI: ratatui 0.30, crossterm 0.29
- Async: tokio (full)
- Config: TOML (`~/.config/trev/config.toml`)
- Session: JSON (`{data_dir}/trev/sessions/`)

## Architecture

```
src/
├── app.rs          # Event loop, state machine
├── app/            # handler/, keymap, state
├── config.rs       # TOML config loading
├── session.rs      # Session persistence (save/restore)
├── state/tree.rs   # TreeState (cursor, expand, sort)
├── tree/           # builder, sort, search_index
├── preview/        # provider registry, content, cache, highlight
├── file_op/        # executor, selection, undo, trash, conflict
├── ui/             # tree_view, preview_view, status_bar, modal
├── input.rs        # AppMode, key events
├── action.rs       # Action enum
└── watcher.rs      # FS change detection (notify)
```

<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->

## Active Technologies
- Rust 2024 edition, nightly-2026-01-24 + okio (full, 既存), serde + serde_json (既存), dirs (既存), clap 4 (既存) (013-neovim-ipc)
- Unix Domain Socket (JSON-RPC 2.0) (013-neovim-ipc)

## Recent Changes
- 013-neovim-ipc: Added Rust 2024 edition, nightly-2026-01-24 + okio (full, 既存), serde + serde_json (既存), dirs (既存), clap 4 (既存)
