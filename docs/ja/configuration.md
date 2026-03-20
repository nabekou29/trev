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
  modal_avoid_preview: false # フローティングモーダルをツリー領域内に制限
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

### Glob パターン

`pattern` フィールドで glob 構文による柔軟なファイルマッチングが可能です。`pattern` を指定すると `extensions` より優先されます。

```yaml
preview:
  commands:
    # 全ファイルに適用
    - name: "Universal Viewer"
      pattern: "*"
      command: bat
      priority: low

    # ブレース展開で複数拡張子にマッチ
    - name: "Code Preview"
      pattern: "*.{rs,go,ts}"
      command: bat

    # 複数パターン（配列）
    - name: "Config Files"
      pattern: ["*.toml", "*.yaml", "*.json", "Makefile"]
      command: bat
```

| フィールド    | 説明                                                               |
| ------------- | ------------------------------------------------------------------ |
| `pattern`     | ファイル名に対する glob パターン（`extensions` より優先）          |
| `extensions`  | 拡張子リスト（大文字小文字不問、`pattern` 未指定時に使用）         |
| `directories` | ディレクトリを対象にするかどうか                                   |
| `git_status`  | 特定の git ステータスのファイルのみ適用（`modified`、`staged` 等） |

## キーバインドのカスタマイズ

キーバインドは Vim スタイルの記法を使用します: `j`、`G`、`<C-a>`、`<A-j>`、`<S-CR>`、`<Space>`、`<Tab>`、`<Esc>`

修飾キー: `C-`（Ctrl）、`A-`/`M-`（Alt）、`S-`（Shift）

マルチキーシーケンスはスペースなしで記述します: `"zz"`、`"zt"`、`"zb"`。角括弧キーも混在可能です: `"g<CR>"`。プレフィックスキーが押されると、trev は次のキーを待ちます（タイムアウト: `key_sequence_timeout_ms`、デフォルト 500ms）。プレフィックスに単一のバインドのみ存在する場合、タイムアウト時に発火します。

バインドはコンテキストごとに整理されます: `universal`、`file`（カーソルがファイル上）、`directory`（カーソルがディレクトリ上）。

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
        notify: open_file # IPC 通知（IPC モード）
  directory:
    bindings:
      - key: "<CR>"
        action: tree.change_root
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

`--config-override` CLI オプションにより、エディタプラグインがユーザーの設定ファイルを変更せずに設定を注入できます。デーモンモードのキーバインド設定やプレビューコマンドの追加、ホストエディタと競合する機能の無効化に便利です。

```sh
trev --ipc --config-override /path/to/override.yml
```

オーバーライドファイルはメイン設定と同じ YAML 形式を使用します。明示的に指定したフィールドのみ適用され、省略したフィールドはユーザーの値が維持されます。

マージルール:

- **スカラー値**（bool、数値、文字列）: 指定されていれば置換
- **リスト**（`keybindings`、`preview.commands`、`file_styles`）: 既存に追加
- **マップ**（`custom_actions`、`menus`）: マージ（オーバーライド側のキーが優先）

例 — Neovim プラグインがキーバインドとプレビューコマンドを提供する場合:

```yaml
keybindings:
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

これにより、プラグイン固有の設定をユーザーの個人設定と分離できます。

## IPC 通知（デーモンモード）

デーモンモードでは、trev は接続中の IPC クライアントに JSON-RPC 2.0 通知を送信します。

### `preview` 通知

プレビューの状態が変化した際に自動送信されます（カーソル移動、プレビューの表示/非表示切替、スクロール、リサイズ）。

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

プレビュー非表示時:

```json
{
  "jsonrpc": "2.0",
  "method": "preview",
  "params": { "path": null }
}
```

### `get_state` リクエスト

接続時に完全なアプリケーション状態を取得できます（初回の `preview` 通知に間に合わない場合に使用）:

```json
{ "jsonrpc": "2.0", "method": "get_state", "id": 1 }
```

レスポンス:

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
