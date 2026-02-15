# Data Model: 012-file-operations

**Date**: 2026-02-15

## Entities

### 1. AppMode (状態マシン)

アプリケーションのモード管理。現在は Normal のみだが、インライン入力と確認ダイアログを追加。

```
Variants:
  - Normal              — 通常のツリー操作
  - Input(InputState)   — インライン入力中 (create/rename)
  - Confirm(ConfirmState) — 確認ダイアログ表示中 (delete)

State Transitions:
  Normal → Input:    `a` (create) or `r` (rename) キー押下
  Input → Normal:    Enter (確定) or Esc (キャンセル)
  Normal → Confirm:  `d` or `D` キー押下
  Confirm → Normal:  `y`/Enter (実行) or `n`/Esc (キャンセル)
```

### 2. InputState (インライン入力)

```
Fields:
  - prompt: String           — 表示用プロンプト ("Create: " / "Rename: ")
  - value: String            — 入力中のテキスト
  - cursor_pos: usize        — テキストカーソル位置
  - on_confirm: InputAction  — 確定時の操作

InputAction Variants:
  - Create { parent_dir: PathBuf }
  - Rename { target: PathBuf }
```

### 3. ConfirmState (確認ダイアログ)

```
Fields:
  - message: String          — ダイアログメッセージ
  - paths: Vec<PathBuf>      — 対象ファイル一覧
  - on_confirm: ConfirmAction — 確定時の操作

ConfirmAction Variants:
  - PermanentDelete
  - CustomTrash
  - SystemTrash
```

### 4. YankBuffer (yank バッファ)

```
Fields:
  - paths: Vec<PathBuf>      — 対象ファイルパス
  - mode: YankMode           — Copy or Cut

YankMode Variants:
  - Copy
  - Cut

Lifecycle:
  - `y`/`x` で上書き設定
  - Copy 後の `p`: バッファ保持（複数回ペースト可能）
  - Cut 後の `p`: バッファクリア（1回のみ）
```

### 5. MarkSet (マーク集合)

```
Fields:
  - marked: HashSet<PathBuf>  — マーク済みパスの集合

Operations:
  - toggle(path)              — マーク追加/解除
  - clear()                   — 全マーク解除
  - targets_or_cursor(cursor_path) — マークがあればマーク一覧、なければカーソル位置

Invariants:
  - yank/delete 実行後に自動クリア
```

### 6. FsOp (ファイルシステム操作 — undo 可能)

```
Variants:
  - Copy { src: PathBuf, dst: PathBuf }
  - Move { src: PathBuf, dst: PathBuf }
  - CreateFile { path: PathBuf }
  - CreateDir { path: PathBuf }
  - RemoveFile { path: PathBuf }
  - RemoveDir { path: PathBuf }

Serializable: Yes (serde JSON)
```

### 7. IrreversibleOp (undo 不可な操作)

```
Variants:
  - SystemTrash { path: PathBuf }
  - PermanentDelete { path: PathBuf }

Serializable: No (スタックに記録しない)
```

### 8. OpGroup (操作グループ)

```
Fields:
  - description: String              — 表示用 ("Copy 3 files to /dst")
  - ops: Vec<FsOp>                   — 正方向の操作リスト
  - undo_ops: Vec<FsOp>              — 逆操作リスト (逆順で適用)
  - expect_exists: Vec<PathBuf>      — undo 事前条件: 存在すべきパス
  - expect_not_exists: Vec<PathBuf>  — redo 事前条件: 存在しないべきパス

Serializable: Yes (serde JSON)

Invariants:
  - ops.len() == undo_ops.len() (1:1 対応)
  - undo_ops は ops の逆順
```

### 9. UndoHistory (undo/redo スタック)

```
Fields:
  - groups: Vec<OpGroup>   — 操作履歴
  - cursor: usize          — 現在位置 (0..=groups.len())
  - max_size: usize        — 最大サイズ (default: 100)

State:
  - cursor == groups.len()  → すべて実行済み (redo なし)
  - cursor == 0             → すべて undo 済み
  - 0 < cursor < len        → undo/redo 両方可能

Operations:
  - undo(): cursor > 0 なら groups[cursor-1].undo_ops を逆順実行、cursor--
  - redo(): cursor < len なら groups[cursor].ops を順に実行、cursor++
  - push(group): groups[cursor..] を切り捨て、group を追加、cursor = new_len
  - overflow: groups.len() > max_size なら groups[0] を削除、cursor--

Serializable: Yes (serde JSON)
```

### 10. SessionState (セッション永続化)

```
Fields:
  - root_path: PathBuf                — セッション識別用ルートパス
  - last_accessed: String             — ISO 8601 日時 (古いセッション削除用)
  - expanded_dirs: Vec<PathBuf>       — 展開中ディレクトリ
  - cursor_path: Option<PathBuf>      — カーソル位置 (パスで保存)
  - scroll_offset: usize              — スクロールオフセット
  - marked_paths: Vec<PathBuf>        — マーク済みパス
  - yank: Option<YankBuffer>          — yank バッファ
  - undo_history: UndoHistory         — undo/redo 履歴

Storage:
  - Path: {data_dir}/trev/sessions/{sha256_16}.json
  - Format: JSON
  - Write: atomic (write-rename pattern)

Lifecycle:
  - 保存: 操作時、undo/redo 時、終了時、展開/折り畳み時 (1s デバウンス)
  - 復元: 起動時 (--restore / session.restore_by_default)
  - 削除: last_accessed が expiry_days 超過時に起動時削除
```

### 11. ChildrenState (拡張)

既存の ChildrenState に `Stale` バリアントを追加。

```
Variants:
  - NotLoaded                    — 未読み込み
  - Loading                      — 読み込み中
  - Loaded(Vec<TreeNode>)        — 読み込み済み
  - Stale(Vec<TreeNode>)         — 要再読み込み (折り畳み中に変更検出)

Transitions:
  NotLoaded → Loading:  展開操作 or prefetch
  Loading → Loaded:     children 読み込み完了
  Loaded → Stale:       折り畳み中に FS 変更検出
  Stale → Loading:      再展開時
  Loaded → Loaded:      展開中に FS 変更検出 (children 差し替え)
```

### 12. FileOpConfig (設定)

```
Fields:
  - delete_mode: DeleteMode       — "permanent" | "custom_trash"
  - undo_stack_size: usize        — default: 100

DeleteMode Variants:
  - Permanent                     — 完全削除 (undo 不可)
  - CustomTrash                   — 独自 trash (undo 可能)
```

### 13. SessionConfig (設定)

```
Fields:
  - restore_by_default: bool      — default: true
  - expiry_days: u64              — default: 90
```

### 14. WatcherConfig (設定)

```
Fields:
  - enabled: bool                 — default: true
  - debounce_ms: u64              — default: 250
```

## Relationships

```
AppState
├── AppMode (Normal | Input | Confirm)
├── TreeState
│   ├── TreeNode (recursive, with ChildrenState)
│   └── MarkSet
├── YankBuffer (Option)
├── UndoHistory
├── FsWatcher (notify watcher instance)
└── SessionState (persistence layer)

Config
├── FileOpConfig
├── SessionConfig
└── WatcherConfig
```
