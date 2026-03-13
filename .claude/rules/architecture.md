---
paths: "src/**/*.rs, benches/**/*.rs, Cargo.toml"
---

# Architecture

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
