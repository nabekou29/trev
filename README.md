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

| Option                     | Description                             |
| -------------------------- | --------------------------------------- |
| `-a, --show-hidden`        | Show hidden files                       |
| `--show-ignored`           | Show gitignored files                   |
| `--no-preview`             | Disable preview panel                   |
| `--sort-order <ORDER>`     | smart, name, size, mtime, type, extension |
| `--sort-direction <DIR>`   | asc, desc                               |
| `--no-directories-first`   | Do not sort directories before files    |
| `--no-icons`               | Disable file icons                      |
| `--daemon`                 | Run as daemon (IPC server)              |
| `--emit`                   | Emit selected path(s) to stdout on exit |
| `--emit-format <FMT>`      | lines (default), nul, json              |
| `--action <ACTION>`        | edit, split, vsplit, tabedit            |
| `--restore / --no-restore` | Session state restore control           |
| `--reveal <PATH>`          | Reveal a specific path on startup       |

## Default Keybindings

### Navigation

| Key           | Action         |
| ------------- | -------------- |
| `j` / `Down`  | Move down      |
| `k` / `Up`    | Move up        |
| `l` / `Right` | Expand         |
| `h` / `Left`  | Collapse       |
| `Enter`       | Toggle expand  |
| `g`           | Jump to first  |
| `G`           | Jump to last   |
| `Ctrl+d`      | Half page down |
| `Ctrl+u`      | Half page up   |

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

### Display

| Key | Action                |
| --- | --------------------- |
| `E` | Expand all            |
| `W` | Collapse all          |
| `.` | Toggle hidden files   |
| `I` | Toggle ignored files  |
| `S` | Sort order menu       |
| `s` | Toggle sort direction |

### File Operations

| Key      | Action               |
| -------- | -------------------- |
| `Space`  | Toggle mark          |
| `a`      | Create file          |
| `A`      | Create directory     |
| `r`      | Rename               |
| `y`      | Yank (copy)          |
| `x`      | Cut                  |
| `p`      | Paste                |
| `d`      | Delete               |
| `D`      | Move to system trash |
| `u`      | Undo                 |
| `Ctrl+r` | Redo                 |
| `Esc`    | Clear selections     |
| `c`      | Copy menu            |

### Copy Menu (`c`)

| Key | Action              |
| --- | ------------------- |
| `y` | Copy absolute path  |
| `r` | Copy relative path  |
| `c` | Copy file content   |
| `t` | Copy directory tree |

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
  order: smart          # smart | name | size | mtime | type | extension
  direction: asc        # asc | desc
  directories_first: true

display:
  show_hidden: false
  show_ignored: false
  show_preview: true
  show_root: false
  columns:              # Display order of metadata columns
    - git_status
    - size
    - modified_at
  column_options:
    modified_at:
      format: relative  # relative | absolute
      mode: os_mtime    # os_mtime | recursive_max

preview:
  max_lines: 1000
  max_bytes: 10485760   # 10 MB
  cache_size: 50
  command_timeout: 3    # seconds
  split: 50             # Tree/preview split % (wide layout)
  narrow_split: 60      # Tree/preview split % (narrow layout)
  narrow_threshold: 80  # Width threshold for narrow layout (columns)
  word_wrap: false      # Enable word wrap in preview
  commands: []

file_operations:
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
```

### Preview Commands Example

```yaml
preview:
  commands:
    - name: "Pretty JSON"
      extensions: [json]
      priority: high     # high (0) | mid (100) | low (1000) | <number>
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
        action: tree.toggle_expand
  daemon:
    universal:
      bindings:
        - key: "<C-q>"
          notify: quit_request
```

**Shell command variables**: `{path}`, `{dir}`, `{name}`, `{root}`

**Available actions**: `quit`, `tree.move_down`, `tree.move_up`, `tree.expand`, `tree.collapse`, `tree.toggle_expand`, `tree.jump_first`, `tree.jump_last`, `tree.half_page_down`, `tree.half_page_up`, `tree.expand_all`, `tree.collapse_all`, `tree.toggle_hidden`, `tree.toggle_ignored`, `preview.scroll_down`, `preview.scroll_up`, `preview.scroll_right`, `preview.scroll_left`, `preview.half_page_down`, `preview.half_page_up`, `preview.cycle_next_provider`, `preview.cycle_prev_provider`, `preview.toggle_preview`, `preview.toggle_wrap`, `file_op.yank`, `file_op.cut`, `file_op.paste`, `file_op.create_file`, `file_op.rename`, `file_op.delete`, `file_op.system_trash`, `file_op.undo`, `file_op.redo`, `file_op.toggle_mark`, `file_op.clear_selections`, `file_op.copy_menu`

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
    })
  end,
}
```

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
