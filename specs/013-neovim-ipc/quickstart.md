# Quickstart: Neovim IPC 連携

## セットアップ

### Rust 側

新しい依存は不要 — すべて既存の依存で実装可能:
- `tokio` (full features) — UDS サーバー/クライアント
- `serde` + `serde_json` — JSON-RPC シリアライゼーション
- `dirs` — ランタイムディレクトリ取得

### Neovim プラグイン

```lua
-- lazy.nvim
{
  "nabekou29/trev",
  config = function()
    require("trev").setup({
      trev_path = "trev",       -- trev バイナリのパス
      width = 40,                -- サイドパネル幅
      auto_reveal = true,        -- BufEnter で自動 reveal
      float = {
        width = 0.8,
        height = 0.8,
        border = "rounded",
      },
      handlers = {
        find_files = function()
          require("telescope.builtin").find_files()
        end,
        grep_project = function()
          require("telescope.builtin").live_grep()
        end,
      },
    })
  end,
}
```

## 使い方

### サイドパネル

```vim
:TrevToggle          " サイドパネルを開閉
:TrevOpen            " サイドパネルを開く
:TrevClose           " サイドパネルを閉じる
:TrevReveal          " 現在のファイルを reveal
:TrevReveal /path    " 指定パスを reveal
```

### フロートピッカー

```vim
:TrevToggle float           " フロートピッカーを開閉
:TrevToggle float vsplit    " フロートピッカー（vsplit で開く）
```

### CLI Daemon 制御

```bash
trev --daemon                    # daemon モードで起動
trev ctl ping                    # 疎通確認
trev ctl reveal /path/to/file    # ファイルを reveal
trev ctl quit                    # daemon を終了
```

### External Commands (config.toml)

```toml
[external_commands]
"C-p" = "find_files"
"C-g" = "grep_project"
```

TUI で `Ctrl+P` を押すと、Neovim 側の `find_files` ハンドラーが呼ばれる。

## 開発フロー

### ビルド & テスト

```bash
mise run build    # ビルド
mise run test     # テスト実行
mise run lint     # clippy
```

### 手動テスト

```bash
# ターミナル 1: daemon 起動
trev --daemon

# ターミナル 2: ctl コマンド（JSON-RPC）
trev ctl ping
trev ctl reveal ./src/main.rs
trev ctl quit
```

### IPC デバッグ

```bash
# ソケットの場所を確認
ls ${XDG_RUNTIME_DIR:-${TMPDIR:-/tmp}}/trev/

# socat で直接 JSON-RPC テスト
echo '{"jsonrpc":"2.0","method":"ping","id":1}' | socat - UNIX-CONNECT:/tmp/trev/trev.sock
```

## 実装順序

1. **JSON-RPC Types & Paths** — JSON-RPC メッセージ型、workspace key、ソケットパス
2. **IPC Server** — UDS サーバー、永続接続管理、メッセージディスパッチ
3. **IPC Client** — `trev ctl` 用クライアント
4. **Reveal** — `TreeState::reveal_path()` 実装
5. **Daemon Integration** — メインループ統合、open_file 通知
6. **Emit Mode** — `--emit` フラグ実装
7. **External Commands** — config.toml 設定、キー検知、通知
8. **Neovim Plugin** — Lua プラグイン（サイドパネル、フロート、reveal、ハンドラー）
