# trev Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-02-11

## Active Technologies
- Rust 2024 edition, nightly-2026-01-24 toolchain + ratatui 0.30, crossterm 0.29, tokio (full), devicons 0.6 (011-tui-ratatui)
- N/A (ファイルシステム読み取りのみ) (011-tui-ratatui)
- Rust 2024 edition, nightly-2026-01-24 + ratatui 0.30, syntect 5, two-face 0.4, ratatui-image 5, ansi-to-tui 7, content_inspector 0.4, lru 0.12, terminal-light 1 (005-preview)

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
- 005-preview: Added Rust 2024 edition, nightly-2026-01-24 + ratatui 0.30, syntect 5, two-face 0.4, ratatui-image 5, ansi-to-tui 7, content_inspector 0.4, lru 0.12, terminal-light 1
- 011-tui-ratatui: Added Rust 2024 edition, nightly-2026-01-24 toolchain + ratatui 0.30, crossterm 0.29, tokio (full), devicons 0.6

- 002-core-tree: Added Rust 2024 edition, nightly-2026-01-24 + ignore 0.4 (WalkBuilder), tokio (background tasks), serde + serde_json (シリアライズ)

<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
