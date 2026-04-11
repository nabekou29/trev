# trev

Rust TUI file tree viewer.

## Tasks

Run via `just <task>` — see `justfile` for full list.

Key: `build`, `test`, `lint`, `lint-fix`, `format`, `bench`

## Gotchas

- After changing `config.rs` or `src/ui/column.rs`, run `just schema` to regenerate `config.schema.json`
