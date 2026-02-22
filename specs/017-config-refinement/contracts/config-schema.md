# Config Schema Contract: 設定体系の整備

**Feature**: 017-config-refinement
**Date**: 2026-02-22

## YAML Config Structure (new)

```yaml
# ~/.config/trev/config.yml

sort:
  order: smart                  # smart | name | size | mtime | type | extension
  direction: asc                # asc | desc
  directories_first: true

display:
  show_hidden: false
  show_ignored: false
  show_preview: true
  show_root_entry: false        # ★ renamed from show_root
  columns:
    - git_status
    - size
    - modified_at
  column_options:
    modified_at:
      format: relative          # relative | absolute
      mode: os_mtime            # os_mtime | recursive_max

preview:
  max_lines: 1000
  max_bytes: 10485760
  cache_size: 50
  command_timeout: 3
  split_ratio: 50               # ★ renamed from split
  narrow_split_ratio: 60        # ★ renamed from narrow_split
  narrow_width: 80              # ★ renamed from narrow_threshold
  word_wrap: false
  commands: []

file_op:                        # ★ renamed from file_operations
  delete_mode: permanent        # permanent | custom_trash
  undo_stack_size: 100

session:
  restore_by_default: true
  expiry_days: 90

watcher:
  enabled: true
  debounce_ms: 250

git:
  enabled: true

# ★ New section: user-defined menus
menus:
  favorites:
    title: "Favorites"
    items:
      - key: "n"
        label: "Sort by name"
        action: "tree.sort.by_name"
      - key: "s"
        label: "Sort by size"
        action: "tree.sort.by_size"
      - key: "o"
        label: "Open in editor"
        run: "code {path}"
      - key: "f"
        label: "Open file in Neovim"
        notify: "open_file"

  my_copy:
    title: "Copy"
    items:
      - key: "p"
        label: "Absolute path"
        action: "file_op.copy.absolute_path"
      - key: "r"
        label: "Relative path"
        action: "file_op.copy.relative_path"
      - key: "n"
        label: "File name"
        action: "file_op.copy.file_name"

keybindings:
  disable_default: false
  universal:
    disable_default: false
    bindings:
      - key: "m"
        menu: "favorites"         # ★ New: open user-defined menu
      - key: "C"
        menu: "my_copy"           # ★ New: custom copy menu
      - key: "<C-1>"
        action: "tree.sort.by_name"   # ★ New: direct sort action
      - key: "<C-2>"
        action: "tree.sort.by_size"
  file: {}
  directory: {}
  daemon:
    universal: {}
    file: {}
    directory: {}
```

## Action Name Registry (complete)

### Tree actions (`tree.*`)

| Action name | Description |
|---|---|
| `tree.move_down` | カーソルを1行下に移動 |
| `tree.move_up` | カーソルを1行上に移動 |
| `tree.expand` | ディレクトリを展開またはファイルを開く |
| `tree.collapse` | ディレクトリを折りたたむまたは親に移動 |
| `tree.toggle_expand` | 展開/折りたたみを切り替え |
| `tree.jump_first` | 最初の項目にジャンプ |
| `tree.jump_last` | 最後の項目にジャンプ |
| `tree.half_page_down` | 半ページ下にスクロール |
| `tree.half_page_up` | 半ページ上にスクロール |
| `tree.expand_all` | カーソル下を再帰的に展開 |
| `tree.collapse_all` | カーソル下をすべて折りたたむ |
| `tree.refresh` | ツリーとgitステータスを更新 |
| `tree.sort.menu` | ★ ソートメニューを開く (旧: `tree.sort_menu`) |
| `tree.sort.toggle_direction` | ★ ソート方向を切り替え (旧: `tree.toggle_sort_direction`) |
| `tree.sort.by_name` | ★ 名前順でソート |
| `tree.sort.by_size` | ★ サイズ順でソート |
| `tree.sort.by_mtime` | ★ 更新日時順でソート |
| `tree.sort.by_type` | ★ 種類順でソート |
| `tree.sort.by_extension` | ★ 拡張子順でソート |
| `tree.sort.by_smart` | ★ スマート順でソート |

### Filter actions (`filter.*`)

| Action name | Description |
|---|---|
| `filter.hidden` | ★ 隠しファイルの表示切替 (旧: `tree.toggle_hidden`) |
| `filter.ignored` | ★ gitignored ファイルの表示切替 (旧: `tree.toggle_ignored`) |

> 将来的に `filter.by_min_lines`, `filter.by_max_size` 等のフィルター条件を追加可能な設計とする。

### Preview actions (`preview.*`)

| Action name | Description |
|---|---|
| `preview.scroll_down` | プレビューを下にスクロール |
| `preview.scroll_up` | プレビューを上にスクロール |
| `preview.scroll_right` | プレビューを右にスクロール |
| `preview.scroll_left` | プレビューを左にスクロール |
| `preview.half_page_down` | プレビュー半ページ下 |
| `preview.half_page_up` | プレビュー半ページ上 |
| `preview.cycle_next_provider` | 次のプレビュープロバイダに切替 |
| `preview.cycle_prev_provider` | 前のプレビュープロバイダに切替 |
| `preview.toggle_preview` | プレビューパネルの表示切替 |
| `preview.toggle_wrap` | ワードラップの切替 |

### File operation actions (`file_op.*`)

| Action name | Description |
|---|---|
| `file_op.yank` | ヤンクバッファにコピー |
| `file_op.cut` | ヤンクバッファにカット |
| `file_op.paste` | 現在のディレクトリに貼り付け |
| `file_op.create_file` | ファイル/ディレクトリ作成の入力を開始 |
| `file_op.rename` | リネーム入力を開始 |
| `file_op.delete` | 選択項目を削除 |
| `file_op.system_trash` | システムゴミ箱に移動 |
| `file_op.undo` | 最後の操作を元に戻す |
| `file_op.redo` | 元に戻した操作をやり直す |
| `file_op.toggle_mark` | カーソル項目のマークを切替 |
| `file_op.clear_selections` | ヤンクバッファとマークをクリア |
| `file_op.copy.menu` | ★ コピーメニューを開く (旧: `file_op.copy_menu`) |
| `file_op.copy.absolute_path` | ★ 絶対パスをコピー |
| `file_op.copy.relative_path` | ★ 相対パスをコピー |
| `file_op.copy.file_name` | ★ ファイル名をコピー |
| `file_op.copy.stem` | ★ 拡張子なしファイル名をコピー |
| `file_op.copy.parent_dir` | ★ 親ディレクトリパスをコピー |

### Special actions

| Action name | Description |
|---|---|
| `quit` | アプリケーションを終了 |
| `noop` | 何もしない（デフォルトキーの無効化用） |
| `shell:<command>` | シェルコマンドを実行 |
| `notify:<method>` | IPC通知を送信 |
| `menu:<name>` | ★ ユーザー定義メニューを開く |
