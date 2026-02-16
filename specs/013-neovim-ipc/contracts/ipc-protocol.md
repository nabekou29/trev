# IPC Protocol Contract

**Version**: 1.0
**Transport**: Unix Domain Socket
**Protocol**: JSON-RPC 2.0 (newline-delimited)

## Protocol Overview

- 各メッセージは 1 行の JSON で、改行 (`\n`) で区切る
- 双方向通信: TUI (サーバー) ↔ クライアント (Neovim / ctl)
- 永続接続（Neovim プラグイン）と一時接続（`trev ctl`）の両方をサポート

### JSON-RPC 2.0 Message Types

**Request** (id あり → レスポンス期待):
```json
{"jsonrpc": "2.0", "method": "reveal", "params": {"path": "/abs/path"}, "id": 1}
```

**Notification** (id なし → レスポンス不要):
```json
{"jsonrpc": "2.0", "method": "open_file", "params": {"action": "edit", "path": "/abs/path"}}
```

**Success Response**:
```json
{"jsonrpc": "2.0", "result": {"ok": true}, "id": 1}
```

**Error Response**:
```json
{"jsonrpc": "2.0", "error": {"code": -32601, "message": "Method not found"}, "id": 1}
```

## Socket Path

```
$XDG_RUNTIME_DIR/trev/<workspace_key>-<pid>.sock
```

フォールバック（`$XDG_RUNTIME_DIR` が未設定の場合）:
```
$TMPDIR/trev/<workspace_key>-<pid>.sock     (macOS)
/tmp/trev/<workspace_key>-<pid>.sock        (Linux fallback)
```

`<pid>` は TUI プロセスの PID。同一ワークスペースで複数 daemon が共存可能。
Neovim プラグインは spawn した TUI の PID からソケットパスを構築して接続する。
`trev ctl` は `<workspace_key>-*.sock` を glob で発見する。

## Methods (Client → TUI)

### reveal

指定パスにカーソルを移動しハイライトする。

```json
{"jsonrpc": "2.0", "method": "reveal", "params": {"path": "/absolute/path/to/file"}, "id": 1}
```

**Response**: `{"jsonrpc": "2.0", "result": {"ok": true}, "id": 1}`

**Behavior**:
- パスの祖先ディレクトリを自動展開
- カーソルをターゲットファイルに移動
- パスがツリー外の場合はサイレント失敗（`ok: true`、ログ出力）

**Note**: Neovim の auto-reveal (BufEnter) では notification として送信可能（id なし）。

### ping

daemon の可用性を確認する。

```json
{"jsonrpc": "2.0", "method": "ping", "id": 1}
```

**Response**: `{"jsonrpc": "2.0", "result": {"ok": true}, "id": 1}`

### quit

daemon のグレースフルシャットダウンを要求する。

```json
{"jsonrpc": "2.0", "method": "quit", "id": 1}
```

**Response**: `{"jsonrpc": "2.0", "result": {"ok": true}, "id": 1}`

### get_state

TUI の現在の状態を取得する。

```json
{"jsonrpc": "2.0", "method": "get_state", "id": 1}
```

**Response**:
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

## Notifications (TUI → Client)

### open_file

ファイルを Neovim で開くよう通知する。

```json
{"jsonrpc": "2.0", "method": "open_file", "params": {"action": "edit", "path": "/abs/path"}}
```

| action | Description |
|--------|-------------|
| `edit` | 現在のウィンドウで開く（デフォルト） |
| `split` | 水平分割で開く |
| `vsplit` | 垂直分割で開く |
| `tabedit` | 新しいタブで開く |

### external_command

config.toml で定義されたカスタムコマンドを実行するよう通知する。

```json
{"jsonrpc": "2.0", "method": "external_command", "params": {"name": "find_files"}}
```

## Connection Model

### Neovim Plugin (永続接続)

1. Neovim プラグインが UDS に接続
2. 双方向にメッセージを送受信
3. Neovim 終了時に接続が切断
4. TUI は切断を検知しクライアント参照を解放

### trev ctl (一時接続)

1. `trev ctl <command>` が UDS に接続
2. JSON-RPC request を送信
3. JSON-RPC response を受信
4. 接続を切断

## Socket Discovery

`trev socket-path` サブコマンドでランタイムディレクトリ内のソケットファイルを一覧表示する。

```bash
$ trev socket-path
/tmp/trev/trev-12345.sock
/tmp/trev/trev-67890.sock
/tmp/trev/other-project-11111.sock

$ trev socket-path --workspace trev
/tmp/trev/trev-12345.sock
/tmp/trev/trev-67890.sock
```

`trev ctl` のソケット発見にも使用: workspace key で glob → 1件なら自動選択、複数なら一覧表示でユーザーに選択を促す。

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Socket not found | `trev ctl`: "daemon is not running" エラー |
| Invalid JSON | Server: JSON-RPC parse error レスポンス |
| Unknown method | Server: method not found レスポンス |
| Main loop unreachable | Server: internal error レスポンス |
| Client disconnected | Server: ログ出力、writer 参照を解放 |
| No client connected | TUI: notification を破棄（ログ出力） |

## Emit Mode Protocol (Float Picker)

`--emit` フラグで起動した trev（daemon モードとは独立）:

1. ユーザーがファイルを選択して Enter → 選択パスを stdout に出力 → exit code 0
2. `q` / `Esc` でキャンセル → 何も出力せず → exit code 0

Neovim 側はターミナルバッファの出力を読み取ってファイルを開く。
