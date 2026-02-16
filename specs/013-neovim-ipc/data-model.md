# Data Model: Neovim IPC 連携

**Date**: 2026-02-16

## Entities

### JSON-RPC Message Types

#### JsonRpcMessage (共通型)

JSON-RPC 2.0 メッセージの union 型。serde の `untagged` enum で request / notification / response を自動判別。

```rust
enum JsonRpcMessage {
    Request { jsonrpc: String, method: String, params: Option<Value>, id: Value },
    Notification { jsonrpc: String, method: String, params: Option<Value> },
    Response { jsonrpc: String, result: Option<Value>, error: Option<JsonRpcError>, id: Value },
}
```

**判別ルール**:
- `id` あり + `method` あり → Request
- `id` なし + `method` あり → Notification
- `id` あり + `result`/`error` あり → Response

#### JsonRpcError

| Field | Type | Description |
|-------|------|-------------|
| code | i64 | エラーコード（JSON-RPC 2.0 標準） |
| message | String | エラーメッセージ |

標準エラーコード:
- `-32700`: Parse error
- `-32600`: Invalid request
- `-32601`: Method not found
- `-32602`: Invalid params
- `-32603`: Internal error

### IpcCommand (内部チャネル用)

IPC サーバーからメインイベントループへ通知する内部コマンド。シリアライズしない。

| Variant | Fields | Description |
|---------|--------|-------------|
| Reveal | `path: PathBuf` | ファイルを reveal する |
| Quit | — | アプリケーション終了 |

### Outgoing Notification Types (TUI → Neovim)

#### open_file

| Field | Type | Description |
|-------|------|-------------|
| action | string | `"edit"` / `"split"` / `"vsplit"` / `"tabedit"` |
| path | string | 開くファイルの絶対パス |

```json
{"jsonrpc": "2.0", "method": "open_file", "params": {"action": "edit", "path": "/abs/path"}}
```

#### external_command

| Field | Type | Description |
|-------|------|-------------|
| name | string | コマンド名（config.toml の external_commands で定義） |

```json
{"jsonrpc": "2.0", "method": "external_command", "params": {"name": "find_files"}}
```

### Incoming Methods (Neovim / ctl → TUI)

#### reveal (request or notification)

| Field | Type | Description |
|-------|------|-------------|
| path | string | reveal するファイルの絶対パス |

```json
{"jsonrpc": "2.0", "method": "reveal", "params": {"path": "/abs/path"}, "id": 1}
```

Response: `{"jsonrpc": "2.0", "result": {"ok": true}, "id": 1}`

#### ping (request)

パラメータなし。

```json
{"jsonrpc": "2.0", "method": "ping", "id": 1}
```

Response: `{"jsonrpc": "2.0", "result": {"ok": true}, "id": 1}`

#### quit (request)

パラメータなし。

```json
{"jsonrpc": "2.0", "method": "quit", "id": 1}
```

Response: `{"jsonrpc": "2.0", "result": {"ok": true}, "id": 1}`

#### get_state (request)

パラメータなし。

```json
{"jsonrpc": "2.0", "method": "get_state", "id": 1}
```

Response:
```json
{
  "jsonrpc": "2.0",
  "result": {
    "cursor_path": "/abs/path/to/file",
    "root_path": "/abs/path/to/root",
    "selected_paths": ["/abs/path1", "/abs/path2"]
  },
  "id": 1
}
```

### EditorAction (enum)

`cli::OpenAction` → `EditorAction` への変換。

| Variant | Serialized | Description |
|---------|-----------|-------------|
| Edit | `"edit"` | 現在のウィンドウで開く |
| Split | `"split"` | 水平分割で開く |
| Vsplit | `"vsplit"` | 垂直分割で開く |
| Tabedit | `"tabedit"` | 新しいタブで開く |

### Workspace Key

| Derivation | Source | Priority |
|------------|--------|----------|
| Git root | `.git` を親方向に探索し、見つかったディレクトリ名 | 1 (highest) |
| Directory name | 作業ディレクトリ名 | 2 (fallback) |

### ExternalCommand (config)

config.toml の `[external_commands]` セクション。

```toml
[external_commands]
"C-p" = "find_files"
"C-g" = "grep_project"
```

Rust 側: `HashMap<String, String>` — キー表現 → コマンド名

## State Transitions

### Daemon Lifecycle

```
Stopped → Starting → Running → Stopping → Stopped
           ↓
       [socket bind]   [accept loop]   [cleanup socket]
```

### Connection Lifecycle

```
Disconnected → Connected → Authenticated → Active → Disconnected
                                            ↑    ↓
                                    [read/write messages]
```

Note: Authentication は不要（ローカル UDS のため）。接続 = アクティブ。

### IPC Command Flow (Neovim/ctl → TUI)

```
Client → [JSON-RPC request] → UDS → Server read task
  → parse JSON-RPC → dispatch method
  → mpsc channel → Main Loop → State Update
  → response via writer → Client
```

### Notification Flow (TUI → Neovim)

```
Main Loop → open_file/external_command イベント
  → IPC server の Neovim writer を取得
  → JSON-RPC notification を書き込み
  → Neovim plugin read loop が受信
  → vim.schedule() でメインスレッドで処理
```

### Reveal Flow

```
IPC reveal / BufEnter auto-reveal
  → JSON-RPC message → Server → IpcCommand::Reveal(path) → Main Loop
  → TreeState::reveal_path(path)
  → For each ancestor: load if needed → expand
  → move_cursor_to_path(target) → center scroll
```

## Relationships

```
Args (cli.rs)
  ├── --daemon → IPC Server 起動 (ipc/server.rs)
  ├── --emit → Emit mode (app.rs)
  ├── --action → EditorAction (open_file notification の action フィールド)
  └── Command::Ctl → IPC Client (ipc/client.rs)

Config (config.rs)
  └── external_commands → HashMap<String, String>

AppContext (app/state.rs)
  └── (new) notifier: Option<IpcNotifier>  // TUI → Neovim 通知用

IpcCommand (ipc/types.rs)
  ├── Reveal(PathBuf) → TreeState::reveal_path()
  └── Quit → state.should_quit = true

IpcNotifier
  └── Arc<Mutex<Option<OwnedWriteHalf>>>  // 永続クライアントの writer
```
