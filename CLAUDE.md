# trev Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-02-17

## Tech Stack

- Rust 2024 edition, nightly-2026-01-24
- TUI: ratatui 0.30, crossterm 0.29
- Async: tokio (full)
- Config: YAML (`~/.config/trev/config.yml`), schemars for JSON Schema
- Session: JSON (`{data_dir}/trev/sessions/`)
- IPC: Unix Domain Socket (JSON-RPC 2.0)

## Architecture

```
src/
├── app.rs           # Event loop, state machine
├── app/             # handler/, keymap, key_parse, state
├── config.rs        # YAML config loading
├── session.rs       # Session persistence (save/restore)
├── state/tree.rs    # TreeState (cursor, expand, sort)
├── tree/            # builder, sort, search_index
├── preview/         # provider registry, content, cache, highlight
├── file_op/         # executor, selection, undo, trash, conflict
├── ui/              # tree_view, preview_view, status_bar, modal
├── input.rs         # AppMode, key events
├── action.rs        # Action enum
├── ipc/             # server, client, types, paths
├── watcher.rs       # FS change detection (notify)
└── cli.rs           # CLI args (clap)
```

<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->

## Active Technologies
- Rust 2024 edition, nightly-2026-01-24 + ratatui 0.30, crossterm 0.29, tokio (full), ignore 0.4 (014-git-integration)
- N/A (git status はインメモリで保持、永続化不要) (014-git-integration)
- N/A（インメモリ状態のみ） (015-column-layout-design)
- Rust 2024 edition, nightly-2026-01-24 + ratatui 0.30, crossterm 0.29, serde, schemars, clap 4 (016-smart-sort)
- N/A (ソートはインメモリ操作) (016-smart-sort)
- Rust 2024 edition, nightly-2026-01-24 + ratatui 0.30, crossterm 0.29, tokio (full), clap 4, serde, schemars (016-smart-sort)
- N/A (インメモリ) (016-smart-sort)

## Recent Changes
- 014-git-integration: Added Rust 2024 edition, nightly-2026-01-24 + ratatui 0.30, crossterm 0.29, tokio (full), ignore 0.4
