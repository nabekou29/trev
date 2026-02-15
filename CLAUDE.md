# trev Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-02-11

## Active Technologies
- Rust 2024 edition, nightly-2026-01-24 toolchain + ratatui 0.30, crossterm 0.29, tokio (full), devicons 0.6 (011-tui-ratatui)
- N/A (ファイルシステム読み取りのみ) (011-tui-ratatui)
- Rust 2024 edition, nightly-2026-01-24 + ratatui 0.30, syntect 5, two-face 0.4, ratatui-image 5, ansi-to-tui 7, content_inspector 0.4, lru 0.12, terminal-light 1 (005-preview)
- Rust 2024 edition, nightly-2026-01-24 + oml (既存), serde (既存), serde_ignored (新規), dirs (新規, `directories` を置換) (001-user-config)
- TOML ファイル (`$XDG_CONFIG_HOME/trev/config.toml` or `~/.config/trev/config.toml`) (001-user-config)
- Rust 2024 edition, nightly-2026-01-24 + ratatui 0.30, crossterm 0.29, tokio (full), notify 8, notify-debouncer-mini 0.5, trash 5, chrono 0.4, sha2 0.10 (012-file-operations)
- JSON ファイル（セッション永続化、undo 履歴）、ローカルファイルシステム (012-file-operations)

- Rust 2024 edition, nightly-2026-01-24 + ignore 0.4 (WalkBuilder), tokio (background tasks), serde + serde_json (シリアライズ) (002-core-tree)

## Project Structure

```text
src/
tests/
```

## Commands

cargo test [ONLY COMMANDS FOR ACTIVE TECHNOLOGIES][ONLY COMMANDS FOR ACTIVE TECHNOLOGIES] cargo clippy

## Code Style

Rust 2024 edition, nightly-2026-01-24: Follow standard conventions

## Recent Changes
- 012-file-operations: Added Rust 2024 edition, nightly-2026-01-24 + ratatui 0.30, crossterm 0.29, tokio (full), notify 8, notify-debouncer-mini 0.5, trash 5, chrono 0.4, sha2 0.10
- 001-user-config: Added Rust 2024 edition, nightly-2026-01-24 + oml (既存), serde (既存), serde_ignored (新規), dirs (新規, `directories` を置換)
- 005-preview: Added Rust 2024 edition, nightly-2026-01-24 + ratatui 0.30, syntect 5, two-face 0.4, ratatui-image 5, ansi-to-tui 7, content_inspector 0.4, lru 0.12, terminal-light 1


<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
