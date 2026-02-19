# Quickstart: Git Integration

## 前提条件

- Rust nightly-2026-01-24 (mise で管理)
- `git` コマンドが PATH に存在すること

## ビルド & テスト

```bash
mise run build       # ビルド
mise run test        # 全テスト実行
mise run lint        # clippy チェック
```

## 実装順序 (ボトムアップ)

### Step 1: データモデル & パーサー (`src/git.rs`)

```bash
# テスト実行 (git モジュールのみ)
cargo test git::
```

- `GitFileStatus` enum
- `GitState` struct + `from_porcelain()` パーサー
- `file_status()`, `dir_status()` メソッド

### Step 2: 設定 (`src/config.rs`, `src/cli.rs`)

- `GitConfig { enabled: bool }` を `Config` に追加
- `--no-git` CLI フラグ
- `apply_cli_overrides` 更新

### Step 3: アクション & キーマップ

- `TreeAction::Refresh` 追加
- `R` → `tree.refresh` デフォルトバインド
- `FromStr` / `Display` / JSON Schema 更新

### Step 4: 非同期取得 & イベントループ統合

- `AppState.git_state: Option<GitState>` 追加
- `AppContext.git_tx` チャネル追加
- `trigger_git_status()` 関数
- イベントループに `git_rx` 処理追加

### Step 5: UI 表示 (`src/ui/tree_view.rs`)

- セレクションマーカーカラムで git status 表示
- `indicator_for_git_status()` 関数

### Step 6: Watcher 連携 & Refresh ハンドラ

- `process_watcher_events` 内で git status 再取得
- `TreeAction::Refresh` ハンドラ実装

### Step 7: カスタムプレビュー git_status 条件

- `ExternalCommand.git_status: Vec<String>` 追加
- `ExternalCmdProvider.can_handle` 更新

## 動作確認

```bash
# Git リポジトリ内で trev を起動
cargo run -- .

# ファイルを変更してインジケーターが表示されることを確認
echo "test" >> some_file.txt

# R キーでリフレッシュ

# git.enabled: false で起動
# ~/.config/trev/config.yml に git: { enabled: false } を追加
cargo run -- .
```
