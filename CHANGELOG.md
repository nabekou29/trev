
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
