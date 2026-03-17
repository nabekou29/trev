# Configuration

Config file: `~/.config/trev/config.yml`

## JSON Schema

Add the following line at the top of your `config.yml` to enable autocompletion and validation in editors that support [yaml-language-server](https://github.com/redhat-developer/yaml-language-server):

```yaml
# yaml-language-server: $schema=https://raw.githubusercontent.com/nabekou29/trev/main/config.schema.json
```

Alternatively, generate the schema locally:

```sh
trev schema > ~/.config/trev/config.schema.json
```

## Default Configuration

```yaml
sort:
  order: smart # smart | name | size | mtime | type | extension
  direction: asc # asc | desc
  directories_first: true

display:
  show_hidden: false
  show_ignored: false
  show_preview: true
  show_icons: true
  show_root_entry: false
  modal_avoid_preview: false # Constrain floating modals to the tree area
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

mouse:
  enabled: true # Enable mouse support (click, scroll wheel)

keybindings:
  key_sequence_timeout_ms: 500 # Timeout for multi-key sequences (ms)
```

## Preview Commands Example

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

### Glob Patterns

Use the `pattern` field for flexible file matching with glob syntax. When `pattern` is set, it takes precedence over `extensions`.

```yaml
preview:
  commands:
    # Match all files
    - name: "Universal Viewer"
      pattern: "*"
      command: bat
      priority: low

    # Match multiple extensions with brace expansion
    - name: "Code Preview"
      pattern: "*.{rs,go,ts}"
      command: bat

    # Multiple patterns (array)
    - name: "Config Files"
      pattern: ["*.toml", "*.yaml", "*.json", "Makefile"]
      command: bat
```

| Field         | Description                                                                        |
| ------------- | ---------------------------------------------------------------------------------- |
| `pattern`     | Glob pattern(s) matched against the file name (takes precedence over `extensions`) |
| `extensions`  | Simple extension list (case-insensitive, used when `pattern` is not set)           |
| `directories` | Whether this command handles directories                                           |
| `git_status`  | Only apply when file has specific git status (`modified`, `staged`, etc.)          |

## Keybinding Customization

Keybindings use Vim-style notation: `j`, `G`, `<C-a>`, `<A-j>`, `<S-CR>`, `<Space>`, `<Tab>`, `<Esc>`.

Modifiers: `C-` (Ctrl), `A-`/`M-` (Alt), `S-` (Shift).

Multi-key sequences are written without spaces: `"zz"`, `"zt"`, `"zb"`. Angle-bracket keys can be mixed in: `"g<CR>"`. When a prefix key is pressed, trev waits for the next key (timeout: `key_sequence_timeout_ms`, default 500ms). If only a single-key binding exists for the prefix, it fires on timeout.

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

## Shell Command Variables

Shell commands in keybindings and menus support these variables:

| Variable | Description         |
| -------- | ------------------- |
| `{path}` | Absolute file path  |
| `{dir}`  | Parent directory    |
| `{name}` | File name           |
| `{root}` | Tree root directory |

## Available Actions

**General:**
`quit`, `open_editor`, `help`, `noop`

**Tree navigation:**
`tree.move_down`, `tree.move_up`, `tree.expand`, `tree.collapse`, `tree.toggle_expand`,
`tree.change_root`, `tree.change_root_up`,
`tree.jump_first`, `tree.jump_last`,
`tree.half_page_down`, `tree.half_page_up`,
`tree.center_cursor`, `tree.scroll_cursor_to_top`, `tree.scroll_cursor_to_bottom`,
`tree.expand_all`, `tree.collapse_all`, `tree.refresh`

**Sort:**
`tree.sort.menu`, `tree.sort.toggle_direction`, `tree.sort.toggle_directories_first`,
`tree.sort.by_name`, `tree.sort.by_size`, `tree.sort.by_mtime`,
`tree.sort.by_type`, `tree.sort.by_extension`, `tree.sort.by_smart`

**Filter:**
`filter.hidden`, `filter.ignored`

**Preview:**
`preview.scroll_down`, `preview.scroll_up`, `preview.scroll_right`, `preview.scroll_left`,
`preview.half_page_down`, `preview.half_page_up`,
`preview.cycle_next_provider`, `preview.cycle_prev_provider`,
`preview.toggle_preview`, `preview.toggle_wrap`

**File operations:**
`file_op.yank`, `file_op.cut`, `file_op.paste`,
`file_op.create_file`, `file_op.create_directory`, `file_op.rename`,
`file_op.delete`, `file_op.system_trash`,
`file_op.undo`, `file_op.redo`,
`file_op.toggle_mark`, `file_op.clear_selections`

**Copy:**
`file_op.copy.menu`,
`file_op.copy.absolute_path`, `file_op.copy.relative_path`,
`file_op.copy.file_name`, `file_op.copy.stem`, `file_op.copy.parent_dir`

**Search:**
`search.open`

**Menu:**
`menu:<name>` (open a user-defined menu)

## User-Defined Menus

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
        action: open_editor

keybindings:
  universal:
    bindings:
      - key: m
        menu: tools
```

Each menu item supports one of:

- `action` -- a built-in action name (e.g. `tree.sort.by_name`, `quit`)
- `run` -- a shell command (supports `{path}`, `{dir}`, `{name}`, `{root}` variables)
- `notify` -- an IPC notification method name (daemon mode)

## Config Override for Editor Plugins

The `--config-override` CLI option lets editor plugins inject configuration without modifying the user's config file. This is useful for setting daemon-mode keybindings, adding preview commands, or disabling features that conflict with the host editor.

```sh
trev --daemon --config-override /path/to/override.yml
```

The override file uses the same YAML format as the main config. Only explicitly specified fields are applied -- omitted fields keep the user's values.

Merge rules:

- **Scalar values** (bool, number, string): replaced if present
- **Lists** (`keybindings`, `preview.commands`, `file_styles`): appended to existing
- **Maps** (`custom_actions`, `menus`): merged (override keys win on conflict)

Example -- a Neovim plugin providing keybindings and a preview command:

```yaml
keybindings:
  daemon:
    file:
      bindings:
        - key: "<CR>"
          notify: open_file
    universal:
      bindings:
        - key: "<C-q>"
          notify: quit_request

preview:
  commands:
    - name: Neovim
      pattern: "*"
      command: echo
      args: ["Neovim preview"]
      priority: high

custom_actions:
  nvim.open_split:
    description: "Open in split"
    notify: nvim.open_split
```

This keeps plugin-specific settings separate from the user's personal configuration.

## IPC Notifications (Daemon Mode)

In daemon mode, trev sends JSON-RPC 2.0 notifications to connected IPC clients.

### `preview` Notification

Sent automatically when the preview state changes (cursor moves to a different file, preview is toggled, scrolled, or resized).

```json
{
  "jsonrpc": "2.0",
  "method": "preview",
  "params": {
    "path": "/absolute/path/to/file.rs",
    "provider": "Text",
    "x": 30,
    "y": 1,
    "width": 80,
    "height": 40,
    "scroll": 0
  }
}
```

When preview is hidden:

```json
{
  "jsonrpc": "2.0",
  "method": "preview",
  "params": { "path": null }
}
```

### `get_state` Request

Clients can request the full application state on connect (since the initial `preview` notification may be missed):

```json
{ "jsonrpc": "2.0", "method": "get_state", "id": 1 }
```

Response:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "preview": {
      "path": "/path/to/file.rs",
      "provider": "Text",
      "x": 30,
      "y": 1,
      "width": 80,
      "height": 40,
      "scroll": 0
    },
    "cursor": { "path": "/path/to/file.rs", "name": "file.rs", "dir": "/path/to", "is_dir": false },
    "root": "/project/root",
    "show_preview": true,
    "show_hidden": false,
    "show_ignored": false,
    "mode": "normal"
  }
}
```
