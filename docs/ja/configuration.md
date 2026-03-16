# 設定

設定ファイル: `~/.config/trev/config.yml`

## JSON Schema

[yaml-language-server](https://github.com/redhat-developer/yaml-language-server) 対応のエディタで自動補完とバリデーションを有効にするには、`config.yml` の先頭に以下を追加してください:

```yaml
# yaml-language-server: $schema=https://raw.githubusercontent.com/nabekou29/trev/main/config.schema.json
```

ローカルに Schema を生成することもできます:

```sh
trev schema > ~/.config/trev/config.schema.json
```

## デフォルト設定

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
  columns: # メタデータカラムの表示順
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
  command_timeout: 3 # 秒
  split_ratio: 50 # ツリー/プレビューの分割比率 (%) （ワイドレイアウト）
  narrow_split_ratio: 60 # ツリー/プレビューの分割比率 (%) （ナローレイアウト）
  narrow_width: 80 # ナローレイアウトの幅閾値（カラム数）
  word_wrap: false # プレビューの折り返しを有効にする
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
  enabled: true # マウスサポートを有効にする（クリック、スクロール）

keybindings:
  key_sequence_timeout_ms: 500 # マルチキーシーケンスのタイムアウト (ms)
```

## プレビューコマンドの例

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

## キーバインドのカスタマイズ

キーバインドは Vim スタイルの記法を使用します: `j`、`G`、`<C-a>`、`<A-j>`、`<S-CR>`、`<Space>`、`<Tab>`、`<Esc>`

修飾キー: `C-`（Ctrl）、`A-`/`M-`（Alt）、`S-`（Shift）

マルチキーシーケンスはスペースなしで記述します: `"zz"`、`"zt"`、`"zb"`。角括弧キーも混在可能です: `"g<CR>"`。プレフィックスキーが押されると、trev は次のキーを待ちます（タイムアウト: `key_sequence_timeout_ms`、デフォルト 500ms）。プレフィックスに単一のバインドのみ存在する場合、タイムアウト時に発火します。

バインドはコンテキストごとに整理されます: `universal`、`file`（カーソルがファイル上）、`directory`（カーソルがディレクトリ上）、および `daemon.*` バリアント。

```yaml
keybindings:
  universal:
    bindings:
      - key: "q"
        action: quit
      - key: "o"
        run: "open {path}" # 変数付きシェルコマンド
  file:
    bindings:
      - key: "<CR>"
        notify: open_file # IPC 通知（デーモンモード）
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

## シェルコマンド変数

キーバインドやメニューのシェルコマンドでは以下の変数が使用できます:

| 変数     | 説明                       |
| -------- | -------------------------- |
| `{path}` | 絶対ファイルパス           |
| `{dir}`  | 親ディレクトリ             |
| `{name}` | ファイル名                 |
| `{root}` | ツリーのルートディレクトリ |

## 利用可能なアクション

**一般:**
`quit`, `open_editor`, `help`, `noop`

**ツリーナビゲーション:**
`tree.move_down`, `tree.move_up`, `tree.expand`, `tree.collapse`, `tree.toggle_expand`,
`tree.change_root`, `tree.change_root_up`,
`tree.jump_first`, `tree.jump_last`,
`tree.half_page_down`, `tree.half_page_up`,
`tree.center_cursor`, `tree.scroll_cursor_to_top`, `tree.scroll_cursor_to_bottom`,
`tree.expand_all`, `tree.collapse_all`, `tree.refresh`

**ソート:**
`tree.sort.menu`, `tree.sort.toggle_direction`, `tree.sort.toggle_directories_first`,
`tree.sort.by_name`, `tree.sort.by_size`, `tree.sort.by_mtime`,
`tree.sort.by_type`, `tree.sort.by_extension`, `tree.sort.by_smart`

**フィルター:**
`filter.hidden`, `filter.ignored`

**プレビュー:**
`preview.scroll_down`, `preview.scroll_up`, `preview.scroll_right`, `preview.scroll_left`,
`preview.half_page_down`, `preview.half_page_up`,
`preview.cycle_next_provider`, `preview.cycle_prev_provider`,
`preview.toggle_preview`, `preview.toggle_wrap`

**ファイル操作:**
`file_op.yank`, `file_op.cut`, `file_op.paste`,
`file_op.create_file`, `file_op.create_directory`, `file_op.rename`,
`file_op.delete`, `file_op.system_trash`,
`file_op.undo`, `file_op.redo`,
`file_op.toggle_mark`, `file_op.clear_selections`

**コピー:**
`file_op.copy.menu`,
`file_op.copy.absolute_path`, `file_op.copy.relative_path`,
`file_op.copy.file_name`, `file_op.copy.stem`, `file_op.copy.parent_dir`

**検索:**
`search.open`

**メニュー:**
`menu:<name>`（ユーザー定義メニューを開く）

## ユーザー定義メニュー

設定でカスタムメニューを定義し、`menu:<name>` でキーにバインドできます:

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

各メニュー項目は以下のいずれかをサポートします:

- `action` — 組み込みアクション名（例: `tree.sort.by_name`、`quit`）
- `run` — シェルコマンド（`{path}`、`{dir}`、`{name}`、`{root}` 変数をサポート）
- `notify` — IPC 通知メソッド名（デーモンモード）

## エディタプラグイン向け設定オーバーライド

`--config-override` CLI オプションにより、エディタプラグインがユーザーの設定ファイルを変更せずに設定を注入できます。デーモンモードのキーバインド設定や、ホストエディタと競合する機能の無効化に便利です。

```sh
trev --daemon --config-override /path/to/override.yml
```

オーバーライドファイルはメイン設定と同じスキーマを使用します。オーバーライドの値がユーザーの設定に上書きマージされます。

典型的な使用例 — Neovim プラグインが IPC 通信用のキーバインドを提供する場合:

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
```

これにより、プラグイン固有のバインドをユーザーの個人設定と分離できます。
