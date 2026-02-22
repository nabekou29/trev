# trev

Fast TUI file viewer with tree view, syntax-highlighted preview, and Neovim integration.

Built in Rust for speed and safety.

## Features

- **Tree view** — Browse directory structure with expand/collapse
- **Syntax-highlighted preview** — Built-in highlighting via syntect, no external tools needed
- **Image preview** — Sixel/Kitty protocol support for inline images
- **External command preview** — Pipe files through custom commands (e.g. `jq`, `glow`, `dust`)
- **Fuzzy search** — Filter files interactively with nucleo
- **File operations** — Copy, cut, paste, rename, delete, undo/redo
- **File system watcher** — Auto-refresh on external changes
- **Session persistence** — Restore tree state across restarts
- **Neovim integration** — Daemon mode with IPC for side panel and floating picker
- **Configurable** — YAML config with keybinding customization, JSON Schema support

## Installation

### From source

```sh
cargo install --git https://github.com/nabekou29/trev
```

### Requirements

- A terminal with [Nerd Fonts](https://www.nerdfonts.com/) for file icons
- Sixel or Kitty graphics protocol for image preview (optional)

## Usage

```sh
trev              # Open current directory
trev ~/projects   # Open specific directory
trev -a           # Show hidden files
```

### Subcommands

```sh
trev ctl reveal <PATH>    # Reveal file in a running daemon
trev ctl ping             # Check if daemon is alive
trev ctl quit             # Stop a running daemon
trev socket-path          # List running daemon sockets
trev schema               # Print config JSON Schema
```

### CLI Options

| Option                     | Description                               |
| -------------------------- | ----------------------------------------- |
| `-a, --show-hidden`        | Show hidden files                         |
| `--show-ignored`           | Show gitignored files                     |
| `--no-preview`             | Disable preview panel                     |
| `--sort-order <ORDER>`     | smart, name, size, mtime, type, extension |
| `--sort-direction <DIR>`   | asc, desc                                 |
| `--no-directories-first`   | Do not sort directories before files      |
| `--no-icons`               | Disable file icons                        |
| `--daemon`                 | Run as daemon (IPC server)                |
| `--emit`                   | Emit selected path(s) to stdout on exit   |
| `--emit-format <FMT>`      | lines (default), nul, json                |
| `--action <ACTION>`        | edit, split, vsplit, tabedit              |
| `--restore / --no-restore` | Session state restore control             |
| `--reveal <PATH>`          | Reveal a specific path on startup         |

## Default Keybindings

### Navigation

| Key           | Action                            |
| ------------- | --------------------------------- |
| `j` / `Down`  | Move down                         |
| `k` / `Up`    | Move up                           |
| `l` / `Right` | Expand                            |
| `h` / `Left`  | Collapse                          |
| `Enter`       | Change root (on directory)        |
| `Backspace`   | Go to parent directory            |
| `g`           | Jump to first                     |
| `G`           | Jump to last                      |
| `Ctrl+d`      | Half page down                    |
| `Ctrl+u`      | Half page up                      |
| `z z`         | Center cursor in viewport         |

### Preview

| Key         | Action                    |
| ----------- | ------------------------- |
| `J`         | Scroll preview down       |
| `K`         | Scroll preview up         |
| `L`         | Scroll preview right      |
| `H`         | Scroll preview left       |
| `U`         | Half page up in preview   |
| `Tab`       | Next preview provider     |
| `Shift+Tab` | Previous preview provider |
| `P`         | Toggle preview            |
| `w`         | Toggle word wrap          |

### Display & Filter

| Key | Action                                               |
| --- | ---------------------------------------------------- |
| `E` | Expand all                                           |
| `W` | Collapse all                                         |
| `.` | Toggle hidden files (`filter.hidden`)                |
| `I` | Toggle ignored files (`filter.ignored`)              |
| `S` | Sort order menu (`tree.sort.menu`)                   |
| `s` | Toggle sort direction (`tree.sort.toggle_direction`) |

### File Operations

| Key      | Action                          |
| -------- | ------------------------------- |
| `Space`  | Toggle mark                     |
| `a`      | Create file                     |
| `A`      | Create directory                |
| `r`      | Rename                          |
| `y`      | Yank (copy)                     |
| `x`      | Cut                             |
| `p`      | Paste                           |
| `d`      | Delete                          |
| `D`      | Move to system trash            |
| `u`      | Undo                            |
| `Ctrl+r` | Redo                            |
| `Esc`    | Clear selections                |
| `c`      | Copy menu (`file_op.copy.menu`) |

### Copy Menu (`c`)

| Key | Action                                            |
| --- | ------------------------------------------------- |
| `p` | Copy absolute path (`file_op.copy.absolute_path`) |
| `r` | Copy relative path (`file_op.copy.relative_path`) |
| `n` | Copy file name (`file_op.copy.file_name`)         |
| `s` | Copy file stem (`file_op.copy.stem`)              |
| `d` | Copy parent directory (`file_op.copy.parent_dir`) |

### Search

| Key   | Action        |
| ----- | ------------- |
| `/`   | Fuzzy search  |
| `Esc` | Cancel search |

## Configuration

Config file: `~/.config/trev/config.yml`

Generate JSON Schema for editor autocompletion:

```sh
trev schema > ~/.config/trev/config.schema.json
```

### Default Configuration

```yaml
sort:
  order: smart # smart | name | size | mtime | type | extension
  direction: asc # asc | desc
  directories_first: true

display:
  show_hidden: false
  show_ignored: false
  show_preview: true
  show_root_entry: false
  columns: # Display order of metadata columns
    - git_status
    - size
    - modified_at
  column_options:
    modified_at:
      format: relative # relative | absolute
      mode: os_mtime # os_mtime | recursive_max

preview:
  max_lines: 1000
  max_bytes: 10485760 # 10 MB
  cache_size: 50
  command_timeout: 3 # seconds
  split_ratio: 50 # Tree/preview split % (wide layout)
  narrow_split_ratio: 60 # Tree/preview split % (narrow layout)
  narrow_width: 80 # Width threshold for narrow layout (columns)
  word_wrap: false # Enable word wrap in preview
  commands: []

file_op:
  delete_mode: permanent # permanent | custom_trash
  undo_stack_size: 100

session:
  restore_by_default: true
  expiry_days: 90

watcher:
  enabled: true
  debounce_ms: 250

git:
  enabled: true

keybindings:
  key_sequence_timeout_ms: 500 # Timeout for multi-key sequences (ms)
```

### Preview Commands Example

```yaml
preview:
  commands:
    - name: "Pretty JSON"
      extensions: [json]
      priority: high # high (0) | mid (100) | low (1000) | <number>
      command: jq
      args: ["."]
    - name: "Markdown"
      extensions: [md]
      command: glow
    - name: "Disk Usage"
      directories: true
      priority: low
      command: dust
      args: ["-r"]
```

### Keybinding Customization

Keybindings use Vim-style notation: `j`, `G`, `<C-a>`, `<A-j>`, `<S-CR>`, `<Space>`, `<Tab>`, `<Esc>`.

Modifiers: `C-` (Ctrl), `A-`/`M-` (Alt), `S-` (Shift).

Multi-key sequences are written with spaces: `"z z"`, `"g g"`. When a prefix key is pressed, trev waits for the next key (timeout: `key_sequence_timeout_ms`, default 500ms). If only a single-key binding exists for the prefix, it fires on timeout.

Bindings are organized by context: `universal`, `file` (cursor on file), `directory` (cursor on directory), and `daemon.*` variants.

```yaml
keybindings:
  universal:
    bindings:
      - key: "q"
        action: quit
      - key: "o"
        run: "open {path}" # Shell command with variables
  file:
    bindings:
      - key: "<CR>"
        notify: open_file # IPC notification (daemon mode)
  directory:
    bindings:
      - key: "<CR>"
        action: tree.change_root
  daemon:
    universal:
      bindings:
        - key: "<C-q>"
          notify: quit_request
```

**Shell command variables**: `{path}`, `{dir}`, `{name}`, `{root}`

**Available actions**:

- **General**: `quit`, `noop`
- **Tree**: `tree.move_down`, `tree.move_up`, `tree.expand`, `tree.collapse`, `tree.toggle_expand`, `tree.change_root`, `tree.change_root_up`, `tree.jump_first`, `tree.jump_last`, `tree.half_page_down`, `tree.half_page_up`, `tree.center_cursor`, `tree.expand_all`, `tree.collapse_all`, `tree.refresh`
- **Sort**: `tree.sort.menu`, `tree.sort.toggle_direction`, `tree.sort.by_name`, `tree.sort.by_size`, `tree.sort.by_mtime`, `tree.sort.by_type`, `tree.sort.by_extension`, `tree.sort.by_smart`
- **Filter**: `filter.hidden`, `filter.ignored`
- **Preview**: `preview.scroll_down`, `preview.scroll_up`, `preview.scroll_right`, `preview.scroll_left`, `preview.half_page_down`, `preview.half_page_up`, `preview.cycle_next_provider`, `preview.cycle_prev_provider`, `preview.toggle_preview`, `preview.toggle_wrap`
- **File operations**: `file_op.yank`, `file_op.cut`, `file_op.paste`, `file_op.create_file`, `file_op.create_directory`, `file_op.rename`, `file_op.delete`, `file_op.system_trash`, `file_op.undo`, `file_op.redo`, `file_op.toggle_mark`, `file_op.clear_selections`
- **Copy**: `file_op.copy.menu`, `file_op.copy.absolute_path`, `file_op.copy.relative_path`, `file_op.copy.file_name`, `file_op.copy.stem`, `file_op.copy.parent_dir`
- **Menu**: `menu:<name>` (open a user-defined menu)

### Full Keybinding Override Example

To reset all defaults and define every binding manually, set `disable_default: true` at the top level:

```yaml
keybindings:
  disable_default: true
  universal:
    bindings:
      # --- Navigation ---
      - key: q
        action: quit
      - key: j
        action: tree.move_down
      - key: "<Down>"
        action: tree.move_down
      - key: k
        action: tree.move_up
      - key: "<Up>"
        action: tree.move_up
      - key: l
        action: tree.expand
      - key: "<Right>"
        action: tree.expand
      - key: h
        action: tree.collapse
      - key: "<Left>"
        action: tree.collapse
      - key: g
        action: tree.jump_first
      - key: G
        action: tree.jump_last
      - key: "<C-d>"
        action: tree.half_page_down
      - key: "<C-u>"
        action: tree.half_page_up
      - key: "z z"
        action: tree.center_cursor

      # --- Display & Filter ---
      - key: E
        action: tree.expand_all
      - key: W
        action: tree.collapse_all
      - key: "."
        action: filter.hidden
      - key: I
        action: filter.ignored
      - key: R
        action: tree.refresh
      - key: S
        action: tree.sort.menu
      - key: s
        action: tree.sort.toggle_direction

      # --- Preview ---
      - key: J
        action: preview.scroll_down
      - key: K
        action: preview.scroll_up
      - key: L
        action: preview.scroll_right
      - key: H
        action: preview.scroll_left
      - key: U
        action: preview.half_page_up
      - key: "<Tab>"
        action: preview.cycle_next_provider
      - key: "<S-Tab>"
        action: preview.cycle_prev_provider
      - key: P
        action: preview.toggle_preview
      - key: w
        action: preview.toggle_wrap

      # --- File Operations ---
      - key: "<Space>"
        action: file_op.toggle_mark
      - key: a
        action: file_op.create_file
      - key: A
        action: file_op.create_directory
      - key: "<BS>"
        action: tree.change_root_up
      - key: r
        action: file_op.rename
      - key: "y"
        action: file_op.yank
      - key: x
        action: file_op.cut
      - key: p
        action: file_op.paste
      - key: d
        action: file_op.delete
      - key: D
        action: file_op.system_trash
      - key: u
        action: file_op.undo
      - key: "<C-r>"
        action: file_op.redo
      - key: "<Esc>"
        action: file_op.clear_selections
      - key: c
        action: file_op.copy.menu

  directory:
    bindings:
      - key: "<CR>"
        action: tree.change_root

  daemon:
    file:
      bindings:
        - key: "<CR>"
          notify: open_file
```

`disable_default: true` はセクション単位でも指定できます。例えば `file:` セクションのデフォルトだけ無効にする場合:

```yaml
keybindings:
  file:
    disable_default: true
    bindings:
      - key: "<CR>"
        action: tree.expand
```

### User-Defined Menus

Define custom menus in config and bind them to keys with `menu:<name>`:

```yaml
menus:
  tools:
    title: Tools
    items:
      - key: g
        label: Git status
        run: "git status"
      - key: l
        label: Git log
        run: "git log --oneline -20"
      - key: o
        label: Open in editor
        action: quit

keybindings:
  universal:
    bindings:
      - key: m
        menu: tools
```

Each menu item supports one of:

- `action` — a built-in action name (e.g. `tree.sort.by_name`, `quit`)
- `run` — a shell command (supports `{path}`, `{dir}`, `{name}`, `{root}` variables)
- `notify` — an IPC notification method name (daemon mode)

## Neovim Integration

trev provides a Lua plugin for Neovim with side panel and floating picker modes.

### Setup

Add the plugin directory to your Neovim config (e.g. with lazy.nvim):

```lua
{
  dir = "/path/to/trev/nvim-plugin",
  config = function()
    require("trev").setup({
      trev_path = "trev",       -- Path to trev binary
      width = 30,               -- Side panel width
      auto_reveal = true,       -- Sync tree with current buffer
      action = "edit",          -- edit | split | vsplit | tabedit
      adapter = "auto",         -- auto | native | snacks | toggleterm | tmux
    })
  end,
}
```

#### Adapter Options

| Adapter      | Description                                                    |
| ------------ | -------------------------------------------------------------- |
| `auto`       | Auto-detect: snacks > toggleterm > native                      |
| `native`     | Built-in Neovim terminal (no plugin dependencies)              |
| `snacks`     | Uses snacks.nvim terminal                                      |
| `toggleterm` | Uses toggleterm.nvim                                           |
| `tmux`       | Uses tmux split-window (panel) and display-popup (float)       |
| `zellij`     | Uses Zellij directional pane (panel) and floating pane (float) |

The `tmux` and `zellij` adapters require Neovim to be running inside the respective multiplexer. They provide native terminal rendering without Neovim terminal emulation overhead. Falls back to `native` if not inside the multiplexer.

### Commands

| Command              | Description          |
| -------------------- | -------------------- |
| `:TrevToggle`        | Toggle side panel    |
| `:TrevToggle float`  | Open floating picker |
| `:TrevOpen`          | Open side panel      |
| `:TrevClose`         | Close side panel     |
| `:TrevReveal [path]` | Reveal file in tree  |

### Daemon Mode

Side panel mode uses `--daemon` to keep trev running with IPC:

- **Auto-reveal**: On `BufEnter`, the tree syncs to the current file
- **File opening**: Selecting a file sends an IPC notification to open it in Neovim
- **Socket management**: Sockets at `$XDG_RUNTIME_DIR/trev/<key>-<pid>.sock`

## License

MIT
