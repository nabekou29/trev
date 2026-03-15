# Full Keybinding Override Example

This file provides a complete keybinding configuration that can be used as a starting point when you want full control over every binding. By setting `disable_default: true`, all built-in keybindings are removed and only the bindings you explicitly define will be active.

## Top-Level Override

To reset all defaults and define every binding manually, set `disable_default: true` at the top level:

```yaml
keybindings:
  disable_default: true
  universal:
    bindings:
      # --- Navigation ---
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
      - key: "zz"
        action: tree.center_cursor
      - key: "zt"
        action: tree.scroll_cursor_to_top
      - key: "zb"
        action: tree.scroll_cursor_to_bottom

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

      # --- Search ---
      - key: "/"
        action: search.open

      # --- General ---
      - key: q
        action: quit
      - key: e
        action: open_editor
      - key: "?"
        action: help

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

## Per-Section Override

`disable_default: true` can also be set on individual sections. For example, to override only the `file` context while keeping defaults for everything else:

```yaml
keybindings:
  file:
    disable_default: true
    bindings:
      - key: "<CR>"
        action: tree.expand
```
