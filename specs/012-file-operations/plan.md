# Implementation Plan: Undo/Redo (Phase 8)

**Branch**: `012-file-operations` | **Date**: 2026-02-16 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification — User Story 4 (FR-021 〜 FR-028)

## Summary

ファイル操作（paste, create, rename, custom trash delete）に undo/redo を追加する。
操作は OpGroup 単位でグループ化し、`u` で undo、`Ctrl+r` で redo。
Permanent delete と System Trash は undo 不可。

## Technical Context

**Language/Version**: Rust 2024 edition, nightly-2026-01-24
**Primary Dependencies**: serde + serde_json (OpGroup 永続化用), 既存の FsOp / executor
**Storage**: N/A (メモリ内スタック。セッション永続化は Phase 10 で対応)
**Testing**: cargo test (googletest + rstest + tempfile)
**Target Platform**: macOS / Linux
**Project Type**: Single Rust binary (TUI)
**Performance Goals**: undo/redo 操作が 1 秒以内に完了 (SC-002)
**Constraints**: undo スタックサイズ設定可能 (default 100)
**Scale/Scope**: 操作履歴 100 エントリ以内

## Constitution Check

| Principle | Status | Notes |
|---|---|---|
| I. Safe Rust | ✅ | unwrap/expect 禁止、Result 返却 |
| II. TDD | ✅ | UndoHistory, OpGroup のユニットテスト先行 |
| III. Performance | ✅ | スタック操作は O(1)、FS 検証は操作数に比例 |
| IV. YAGNI | ✅ | 存在チェックのみで検証、ハッシュ検証は不要 |
| V. Incremental | ✅ | データ型 → ロジック → ハンドラ統合 → UI の順 |

## Data Model

### OpGroup — 操作グループ

```rust
/// A single undoable file system operation with its reverse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoOp {
    /// Forward operation (what was done).
    pub forward: FsOp,
    /// Reverse operation (how to undo it).
    pub reverse: FsOp,
}

/// A group of operations performed as a single user action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpGroup {
    /// Human-readable description for status messages.
    pub description: String,
    /// Individual operations in execution order.
    pub ops: Vec<UndoOp>,
}
```

### UndoHistory — 操作履歴スタック

```rust
/// Bounded undo/redo history.
#[derive(Debug)]
pub struct UndoHistory {
    /// Completed operations (undo target). Last = most recent.
    undo_stack: Vec<OpGroup>,
    /// Undone operations (redo target). Last = most recent undo.
    redo_stack: Vec<OpGroup>,
    /// Maximum undo stack size.
    max_size: usize,
}
```

### 操作ごとの undo マッピング

| 操作 | Forward FsOp | Reverse FsOp | Undo 可否 |
|---|---|---|---|
| Paste (Copy) | `Copy {src, dst}` | `RemoveFile/Dir {dst}` | ✅ |
| Paste (Move) | `Move {src, dst}` | `Move {dst, src}` | ✅ |
| Create file | `CreateFile {path}` | `RemoveFile {path}` | ✅ |
| Create dir | `CreateDir {path}` | `RemoveDir {path}` | ✅ |
| Rename | `Move {old, new}` | `Move {new, old}` | ✅ |
| Delete (CustomTrash) | `Move {orig, trash}` | `Move {trash, orig}` | ✅ |
| Delete (Permanent) | — | — | ❌ |
| System Trash (D) | — | — | ❌ |

### FS 状態検証 (undo/redo 前)

各 reverse/forward op の事前条件を検証:
- `Move {src, dst}`: src が存在、dst が存在しない
- `Copy {src, dst}`: src が存在、dst が存在しない
- `RemoveFile {path}`: path が存在
- `RemoveDir {path}`: path が存在
- `CreateFile {path}`: path が存在しない
- `CreateDir {path}`: path が存在しない

1 つでも失敗 → エラーメッセージ表示、操作中止。

### スタック溢れ時の Trash クリーンアップ

undo スタックが max_size を超えた場合:
1. 最古の OpGroup を pop
2. reverse op に trash パスへの参照があれば `clean_trash_file()` で削除

## Project Structure

```text
src/file_op/
├── undo.rs         # UndoOp, OpGroup, UndoHistory (実装対象)
├── executor.rs     # FsOp, execute() (既存)
├── trash.rs        # trash_path(), clean_trash_file() (既存)
├── selection.rs    # SelectionBuffer (既存)
└── conflict.rs     # resolve_conflict() (既存)

src/app/
├── state.rs        # AppState に undo_history フィールド追加
├── handler/
│   └── file_op.rs  # OpGroup 生成 + undo/redo ハンドラ
└── keymap.rs       # Ctrl+r バインド追加
```

## Implementation Steps

### Step 1: `src/file_op/undo.rs` — データ型 + UndoHistory

- `UndoOp`, `OpGroup` 構造体
- `UndoHistory`: `new(max_size)`, `push(group)`, `undo()`, `redo()`, `can_undo()`, `can_redo()`
- `push` 時: redo スタッククリア、max_size 超過時に最古を evict + trash cleanup
- `validate_preconditions(ops)` — FS 存在チェック
- テスト: push/undo/redo サイクル、スタック溢れ、検証失敗

### Step 2: `src/app/state.rs` — AppState にフィールド追加

- `pub undo_history: UndoHistory`

### Step 3: `src/app.rs` — 初期化

- `undo_history: UndoHistory::new(config.file_op.undo_stack_size)`

### Step 4: `src/app/handler/file_op.rs` — OpGroup 生成

各操作で OpGroup を構築して `undo_history.push()`:
- `execute_paste()` → Copy/Move ops のグループ
- `execute_create()` → CreateFile/CreateDir の単一 op
- `execute_rename()` → Move の単一 op
- `execute_delete()` (CustomTrash のみ) → Move ops のグループ
- Permanent delete / System trash → push しない、"This operation cannot be undone" 表示

### Step 5: `src/app/handler/file_op.rs` — Undo/Redo ハンドラ

- `FileOpAction::Undo` → `undo_history.undo()` → 逆操作を実行 → ステータスメッセージ
- `FileOpAction::Redo` → `undo_history.redo()` → 順操作を再実行 → ステータスメッセージ
- 検証失敗時: "Cannot undo/redo: file state has changed" 表示

### Step 6: `src/app/keymap.rs` — Ctrl+r バインド

- `Ctrl+r` → `FileOpAction::Redo` (u → Undo は既存)

### Step 7: テスト + lint + format

## Verification

```bash
mise run test
mise run lint
mise run format
```
