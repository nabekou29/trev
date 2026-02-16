# Research: Neovim IPC 連携

**Date**: 2026-02-16

## R1: IPC プロトコル設計

### Decision
JSON-RPC 2.0 over Unix Domain Socket（改行区切り、双方向永続接続）

### Rationale
- 標準プロトコル — request/response と notification の両方をサポート
- 双方向 — TUI→Neovim（通知）と Neovim→TUI（リクエスト/通知）を同一接続で処理
- デバッグしやすい — `socat` 等で直接テスト可能
- serde_json で手動実装が容易（外部 JSON-RPC ライブラリ不要）
- `.cmd` ファイルハックを排除 — 直接通信でシンプルかつ信頼性が高い

### Alternatives Considered
- **カスタム NDJSON（プロトタイプ方式）**: 一方向のみ。双方向にするには `.cmd` ファイルが必要でハック的。
- **MessagePack-RPC**: Neovim 内部が使用するが、Rust 側のシリアライゼーションが JSON より複雑。
- **gRPC / tonic**: 大幅なオーバーエンジニアリング。

### JSON-RPC 2.0 Implementation
外部ライブラリは使わず手動実装:

```rust
// Request (id あり → レスポンス期待)
{"jsonrpc": "2.0", "method": "reveal", "params": {"path": "/..."}, "id": 1}

// Notification (id なし → レスポンス不要)
{"jsonrpc": "2.0", "method": "open_file", "params": {"action": "edit", "path": "/..."}}

// Response (成功)
{"jsonrpc": "2.0", "result": {"ok": true}, "id": 1}

// Response (エラー)
{"jsonrpc": "2.0", "error": {"code": -32601, "message": "Method not found"}, "id": 1}
```

## R2: 接続モデル

### Decision
TUI = UDS サーバー、Neovim プラグイン = 永続クライアント。`trev ctl` = 一時クライアント。

### Rationale
- TUI（daemon）が先に起動し、Neovim プラグインが後から接続するフローが自然
- 永続接続により Neovim→TUI（reveal）と TUI→Neovim（open_file, external_command）の双方向通信が可能
- `trev ctl` は一時的な接続でリクエスト/レスポンスして切断
- サーバーは接続を区別する必要あり（永続クライアント vs 一時クライアント）

### Connection Lifecycle
```
1. TUI daemon 起動 → UDS サーバー開始
2. Neovim プラグイン → UDS に接続（永続）
3. TUI → Neovim: open_file/external_command notification
4. Neovim → TUI: reveal notification / get_state request
5. trev ctl → UDS に接続 → request → response → 切断
6. Neovim 終了 → 接続切断
7. TUI daemon 終了 → ソケット削除
```

### Server Design
- `tokio::net::UnixListener` で接続を受け付け
- 各接続に対して read タスクを spawn
- 永続クライアント（Neovim）の writer を `Arc<Mutex<OwnedWriteHalf>>` で保持
- TUI→Neovim の通知は保持した writer 経由で送信
- 複数クライアント対応（`trev ctl` と Neovim が同時接続可能）

## R3: ソケットパスの規約

### Decision
`$XDG_RUNTIME_DIR/trev/<workspace_key>-<pid>.sock`（フォールバック: `$TMPDIR` → `/tmp`）

プロセスごとに一意のソケットを作成し、同一ワークスペースで複数 daemon を許容する。

### Rationale
- PID で一意化 — 同一ワークスペースで複数の Neovim + daemon を並行運用可能
- Neovim プラグインは spawn した TUI プロセスの PID を取得してソケットパスを構築
- `trev ctl` は `trev/<workspace_key>-*.sock` を glob で発見（複数ある場合は全件表示 or `--workspace` で絞り込み）
- XDG Base Directory 仕様に準拠（フォールバック: `$TMPDIR` → `/tmp`）

### Implementation
- `dirs::runtime_dir().unwrap_or_else(std::env::temp_dir)` で取得
- サブディレクトリ `trev/` を作成
- ファイル名: `<workspace_key>-<pid>.sock`（`pid` は `std::process::id()`）
- `trev ctl` のソケット発見: glob パターン `trev/<workspace_key>-*.sock` で検索、1件なら自動選択、複数ならリスト表示

## R4: Workspace Key の導出

### Decision
Git リポジトリのルートディレクトリ名、フォールバックとして cwd のディレクトリ名。

### Rationale
- Git root で同一リポジトリ内の異なるサブディレクトリから同じ daemon を `trev ctl` で発見可能
- `.git` ディレクトリの存在チェックで実装（外部コマンド不要）
- ディレクトリ名の衝突があっても PID で区別されるため実害なし

## R5: メインイベントループへの IPC 統合

### Decision
既存の `mpsc::UnboundedChannel<IpcCommand>` パターンで IPC コマンドをメインループに統合。

### Rationale
- 既存の `children_rx`, `preview_rx`, `watcher_rx` と同じパターン
- `try_recv()` で非ブロッキング処理
- IPC サーバーは `tokio::spawn` で別タスク

### Implementation
```
メインループ:
1. draw()
2. poll(50ms) → key event
3. try_recv(children_rx) → 子ノードロード結果
4. try_recv(watcher_rx) → FS 変更通知
5. try_recv(preview_rx) → プレビュー結果
6. try_recv(ipc_rx) → IPC コマンド (NEW)
7. cursor change → preview trigger
```

IpcCommand のバリアント:
- `Reveal(PathBuf)` → `TreeState::reveal_path()` 呼び出し
- `Quit` → `state.should_quit = true`
- `GetState { response_tx }` → 状態をシリアライズして送り返す

## R6: Reveal 実装

### Decision
`TreeState::reveal_path()` メソッドを追加。パスの各祖先を展開し、カーソルを移動する。

### Rationale
- 現在の `move_cursor_to_path()` は可視ノードのみ検索
- reveal には祖先ディレクトリの再帰的展開が必要
- 同期ロード（`TreeBuilder::load_children()` 直接呼び出し）で即座に完了

### Algorithm
```
reveal_path(target):
  // root から target までのパスコンポーネントを取得
  components = relative_path(root, target).components()
  current = root
  for each component:
    current = current / component
    if current is dir && not loaded:
      sync load_children(current)
    if current is dir && not expanded:
      expand(current)
  move_cursor_to_path(target)
```

## R7: TUI → Neovim 通知（open_file, external_command）

### Decision
永続接続クライアントの writer に JSON-RPC notification を書き込む。

### Rationale
- daemon モードの TUI がファイルを「開く」 → `open_file` notification を Neovim に送信
- Neovim プラグインが notification を受信し `vim.cmd("edit ...")` 実行
- `external_command` も同じ仕組み

### Flow
```
User presses Enter on file → ExpandResult::OpenFile(path)
  → IPC server の writer に open_file notification を送信
  → Neovim plugin が受信して vim.cmd() 実行
```

```
User presses external_command key → config.external_commands で定義された名前を取得
  → IPC server の writer に external_command notification を送信
  → Neovim plugin のハンドラーが受信して処理
```

## R8: External Commands

### Decision
`config.toml` の `[external_commands]` セクションでキーバインド→コマンド名のマッピングを定義。

### Rationale
- TUI はキー入力を検知して通知するだけ、処理ロジックは Neovim 側
- ユーザーが自由にハンドラーを追加可能（Neovim 側で定義）
- TUI が Neovim プラグインの詳細を知る必要がない

### Config Format
```toml
[external_commands]
"C-p" = "find_files"
"C-g" = "grep_project"
"C-f" = "live_grep"
```

### Neovim Side
```lua
-- setup() で handlers を登録
require("trev").setup({
  handlers = {
    find_files = function() require("telescope.builtin").find_files() end,
    grep_project = function() require("telescope.builtin").live_grep() end,
  },
})
```

## R9: Neovim プラグイン: toggleterm vs snacks

### Decision
プラグインは `toggleterm.nvim` と `snacks.nvim` の両方をサポートし、利用可能な方を使用する。

### Rationale
- spec の要件: 「利用可能な方を使う」
- toggleterm はターミナルバッファ管理に特化（プロトタイプで実績あり）
- snacks.nvim は最近人気の汎用ユーティリティ（terminal モジュール含む）

### Implementation
```lua
local function get_terminal_backend()
  local ok, toggleterm = pcall(require, "toggleterm.terminal")
  if ok then return "toggleterm", toggleterm end
  local ok2, snacks = pcall(require, "snacks")
  if ok2 and snacks.terminal then return "snacks", snacks end
  return nil, nil
end
```

## R10: `--emit` モード（フロートピッカー）

### Decision
`--emit` フラグ付きの trev はファイル選択時に選択パスを stdout に出力して終了。

### Rationale
- 既存の `cli.rs` に `--emit`, `--emit-format` が定義済み
- フロートピッカーは Neovim の `vim.fn.termopen()` で起動し、終了時に出力を読み取る
- daemon モードとは独立（`--emit` は `--daemon` と排他的）

### Implementation
- `ExpandResult::OpenFile(path)` 発生時に emit パスを蓄積
- `should_quit` 時に蓄積したパスを stdout に出力（`terminal::restore()` 後）
- `--emit` + Enter で quit
