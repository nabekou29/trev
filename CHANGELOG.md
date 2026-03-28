
## [0.1.8] - 2026-03-28

### Features

- Add configurable diff pager with built-in fallback (#1)


## [0.1.7] - 2026-03-24

### Bug Fixes

- Prevent cancelled preview loads from being cached as empty


## [0.1.6] - 2026-03-23

### Features

- Handle terminal resize events for automatic redraw


## [0.1.5] - 2026-03-21

### Documentation

- Mention OS clipboard integration in README features
- Replace remaining "daemon" references with "IPC" in Japanese docs

### Features

- Allow users to override and disable built-in preview providers
- Add OS clipboard paste support for files and images
- Bind C to copy relative path to clipboard
- Add clipboard_sync option for yank/cut
- Add copy_to_clipboard action and copy menu Files entry
- Add env option to external preview providers
- Add show_preview and hide_preview actions
- Add action method to execute arbitrary actions via IPC


## [0.1.4] - 2026-03-20

### Bug Fixes

- Prevent intermediate search matches from permanently expanding directories
- Remove remaining "Daemon mode" references in README feature lists

### Performance

- Reduce unnecessary dependencies by trimming feature flags
- Filter out .git changes to break feedback loop

### Refactor

- Rename --daemon to --ipc and remove IPC-specific keybinding sections


## [0.1.3] - 2026-03-18

### Bug Fixes

- Use inner height for scroll clamp to allow scrolling to end

### Documentation

- Update Neovim integration example to match current trev.nvim API


## [0.1.2] - 2026-03-17

### Features

- Add modal_avoid_preview option to constrain modals to tree area


## [0.1.1] - 2026-03-17

### Documentation

- Add GitHub backend option for mise installation
- Add Japanese documentation
- Add preview patterns, config override rules, and IPC sections

### Features

- Add preview notifications and get_state request
- Add partial config override and glob patterns for preview commands

### Refactor

- Make logging opt-in, reduce span noise

### Styling

- Fix code formatting

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).
