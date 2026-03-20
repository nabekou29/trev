---
paths: "nvim-plugin/**/*.lua"
---

# Neovim Plugin Rules

Lua plugin for trev integration with Neovim.

## Architecture

- `lua/trev/init.lua` - Main module with setup(), commands, and state management
- `plugin/trev.lua` - Plugin loader (minimal, just sets `vim.g.loaded_trev`)

## Key Patterns

- Use `vim.api` for Neovim API calls
- Use `vim.fn` for Vimscript functions
- Use `vim.schedule()` for async callbacks to main thread
- Use `vim.defer_fn()` for delayed execution
- Use `vim.uv` (libuv) for file watching

## Dependencies

- `toggleterm.nvim` - Required for side panel mode
- trev IPC mode - `--ipc` flag starts IPC server

## IPC Protocol

- Socket: `$XDG_RUNTIME_DIR/trev/<workspace_key>.sock`
- Command file: `$XDG_RUNTIME_DIR/trev/<workspace_key>.cmd`
- Workspace key: Git root directory name or cwd directory name
