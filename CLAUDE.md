# trev Development Guidelines

## Tech Stack

- Rust 2024 edition, nightly-2026-01-24
- TUI: ratatui 0.30, crossterm 0.29
- Async: tokio (full)
- Config: YAML (`~/.config/trev/config.yml`), schemars for JSON Schema
- Session: JSON (`{data_dir}/trev/sessions/`)
- IPC: Unix Domain Socket (JSON-RPC 2.0)

<!-- MANUAL ADDITIONS START -->

## Performance

- Always measure before optimizing — never optimize based on guesses
- See `perf` skill for detailed measurement workflow

<!-- MANUAL ADDITIONS END -->
