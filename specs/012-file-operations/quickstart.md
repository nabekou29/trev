# Quickstart: 012-file-operations

**Date**: 2026-02-15

## 新規依存クレート

```toml
# Cargo.toml に追加
[dependencies]
notify = "8"
notify-debouncer-mini = "0.5"
trash = "5"
chrono = { version = "0.4", features = ["serde"] }
sha2 = "0.10"
```

## 設定ファイル追加 (config.toml)

```toml
[session]
restore_by_default = true
expiry_days = 90

[file_operations]
delete_mode = "permanent"   # "permanent" | "custom_trash"
undo_stack_size = 100

[watcher]
enabled = true
debounce_ms = 250
```

## CLI オプション追加

```
trev [--restore | --no-restore] [path]
```

## 新規モジュール構成

```text
src/
├── file_op.rs           # pub mod (re-export)
├── file_op/
│   ├── executor.rs      # FsOp/IrreversibleOp の実行ロジック
│   ├── undo.rs          # UndoHistory, OpGroup 管理
│   ├── yank.rs          # YankBuffer, YankMode
│   ├── mark.rs          # MarkSet
│   ├── conflict.rs      # 自動リネーム (file_1.txt)
│   └── trash.rs         # 独自 trash ディレクトリ管理
├── session.rs           # SessionState の保存/復元
├── watcher.rs           # notify watcher 統合
├── input.rs             # InputState, テキスト編集 (既存ファイルを利用)
├── ui/
│   ├── modal.rs         # 確認ダイアログ (既存ファイルを利用)
│   └── inline_input.rs  # インライン入力 widget
└── (既存ファイルの変更)
    ├── action.rs         # FileOpAction 追加
    ├── app.rs            # AppMode 導入, file_op/watcher/session 統合
    ├── config.rs         # FileOpConfig, SessionConfig, WatcherConfig 追加
    ├── cli.rs            # --restore/--no-restore 追加
    ├── state/tree.rs     # ChildrenState::Stale 追加, MarkSet 統合
    ├── ui/tree_view.rs   # マーク表示, インライン入力表示
    └── ui/status_bar.rs  # yank/mark/操作結果表示
```

## 実装順序 (ボトムアップ)

### Phase 1: UI 基盤 + マーク
1. `InputState` — テキスト入力バッファ + 編集操作
2. `ConfirmState` — 確認ダイアログ状態
3. `AppMode` — Normal/Input/Confirm 状態マシン
4. `MarkSet` — マーク管理
5. UI: インライン入力 widget, 確認ダイアログ widget, マーク表示
6. `Action`: `FileOpAction` 追加, キーマッピング

### Phase 2: ファイル操作コア
1. `FsOp` / `IrreversibleOp` — 操作定義
2. `executor` — 各 FsOp の実行ロジック
3. `conflict` — 自動リネーム
4. `trash` — 独自 trash ディレクトリ管理
5. `yank` — YankBuffer
6. 各操作の統合: create, rename, delete, yank/paste

### Phase 3: Undo/Redo
1. `OpGroup` — 操作グループ構造
2. `UndoHistory` — スタック管理 + 事前検証
3. 各操作への OpGroup 生成統合
4. undo/redo キーバインディング

### Phase 4: FS 変更検出
1. `watcher` — notify watcher セットアップ
2. デバウンス + イベントハンドリング
3. `ChildrenState::Stale` 対応
4. 自己操作中の一時停止 (AtomicBool フラグ)

### Phase 5: セッション永続化
1. `SessionState` — データ構造 + シリアライズ
2. 保存ロジック (atomic write)
3. 復元ロジック (CLI オプション + 検証)
4. 古いセッションの自動削除

## 検証コマンド

```bash
mise run build     # ビルド
mise run test      # テスト
mise run lint      # clippy
mise run format    # rustfmt
```
