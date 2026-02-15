# Implementation Plan: Session Persistence (Phase 10)

**Branch**: `012-file-operations` | **Date**: 2026-02-16 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification — User Story 6 (FR-033 〜 FR-037)

## Summary

セッション状態（展開状態、カーソル位置、選択バッファ、undo 履歴）をルートパスごとに
JSON ファイルに保存・復元する。起動時に `--restore`/`--no-restore` または設定で制御。
保存はアプリ終了時に実行し、期限切れセッション（デフォルト 90 日）は起動時に自動削除する。

## Technical Context

**Language/Version**: Rust 2024 edition, nightly-2026-01-24
**Primary Dependencies**: serde + serde_json (既存), sha2 (既存), dirs (既存)
**Storage**: JSON ファイル (`{data_dir}/trev/sessions/{hash}.json`)
**Testing**: cargo test (googletest + rstest + tempfile)
**Target Platform**: macOS / Linux
**Project Type**: Single Rust binary (TUI)
**Performance Goals**: セッション復元が 2 秒以内 (SC-004)
**Constraints**: クラッシュ時は最後の保存時点に戻る (Clarification)
**Scale/Scope**: 通常 1 セッションファイル（数 KB）

## Constitution Check

| Principle | Status | Notes |
|---|---|---|
| I. Safe Rust | ✅ | Result 返却、unwrap 禁止 |
| II. TDD | ✅ | SessionState のシリアライズ/デシリアライズテスト先行 |
| III. Performance | ✅ | JSON ファイルは数 KB、save/restore は数 ms |
| IV. YAGNI | ✅ | save は終了時のみ（debounce 不要）、展開パスリスト方式でツリー全体を保存しない |
| V. Incremental | ✅ | データ型 → save → restore → CLI 制御 → cleanup の順 |

## Data Model

### SessionState

```rust
/// Serializable session state for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Canonical root path this session belongs to.
    pub root_path: PathBuf,
    /// Last access timestamp (for expiry cleanup).
    pub last_accessed: SystemTime,
    /// Paths of expanded directories.
    pub expanded_paths: Vec<PathBuf>,
    /// Path the cursor was on (more robust than index).
    pub cursor_path: Option<PathBuf>,
    /// Scroll offset.
    pub scroll_offset: usize,
    /// Selection buffer paths.
    pub selection_paths: Vec<PathBuf>,
    /// Selection buffer mode.
    pub selection_mode: Option<SelectionMode>,
    /// Undo stack.
    pub undo_stack: Vec<OpGroup>,
    /// Redo stack.
    pub redo_stack: Vec<OpGroup>,
}
```

### セッションファイルパス

```text
{data_dir}/trev/sessions/{sha256_hex_16}.json
```

- `data_dir`: `dirs::data_dir()` (macOS: `~/Library/Application Support`, Linux: `$XDG_DATA_HOME`)
- `sha256_hex_16`: ルートパスの SHA-256 の先頭 16 文字

### Save/Restore フロー

```text
Save (on quit):
  AppState → SessionState → serde_json::to_string_pretty
  → write to {hash}.json.tmp → rename to {hash}.json (atomic)

Restore (on startup):
  Read {hash}.json → SessionState → validate paths
  → filter out non-existent expanded_paths
  → rebuild tree with expand + cursor restore
```

### CLI 制御

```text
--restore     → 強制復元
--no-restore  → 強制スキップ
未指定         → config.session.restore_by_default に従う
```

### 期限切れクリーンアップ

```text
起動時: sessions/ 内の全 .json を走査
  → last_accessed + expiry_days < now → 削除
```

## Project Structure

```text
src/
├── session.rs          # SessionState, save, restore, cleanup (実装対象)
├── app.rs              # save on quit, restore on startup
├── file_op/
│   ├── undo.rs         # UndoHistory に export_stacks/from_stacks 追加
│   └── selection.rs    # SelectionBuffer に export/from 追加
└── state/
    └── tree.rs         # expanded_paths() 抽出メソッド追加
```

## Implementation Steps

### Step 1: `src/file_op/undo.rs` — UndoHistory のシリアライズサポート

- `export_stacks(&self) -> (&[OpGroup], &[OpGroup])` — undo/redo スタックの参照を返す
- `from_stacks(undo: Vec<OpGroup>, redo: Vec<OpGroup>, max_size: usize) -> Self` — スタックから復元
- テスト: export → from_stacks のラウンドトリップ

### Step 2: `src/file_op/selection.rs` — SelectionBuffer のシリアライズサポート

- `export(&self) -> (Vec<PathBuf>, Option<SelectionMode>)` — パスとモードを返す
- `from_parts(paths: Vec<PathBuf>, mode: Option<SelectionMode>) -> Self` — パーツから復元
- テスト: export → from_parts のラウンドトリップ

### Step 3: `src/state/tree.rs` — expanded_paths 抽出

- `expanded_paths(&self) -> Vec<PathBuf>` — 展開中ディレクトリのパスリストを返す
- `cursor_path(&self) -> Option<PathBuf>` — カーソル位置のパスを返す
- テスト: 展開済みツリーから正しいパスが返ること

### Step 4: `src/session.rs` — SessionState 型 + ファイルパス計算

- `SessionState` 構造体 (Serialize, Deserialize)
- `session_dir() -> PathBuf` — セッションディレクトリパス
- `session_path(root: &Path) -> PathBuf` — SHA-256 ベースのファイルパス
- テスト: パス計算の一貫性、ハッシュの安定性

### Step 5: `src/session.rs` — Save 実装

- `save(state: &SessionState) -> Result<()>` — atomic write (tmp + rename)
- `SessionState::from_app(state: &AppState, root_path: &Path) -> Self` — AppState から抽出
- テスト: save → ファイル存在確認、JSON 内容の検証

### Step 6: `src/session.rs` — Restore 実装

- `restore(root_path: &Path) -> Result<Option<SessionState>>` — ファイルが無ければ None
- パス検証: expanded_paths の存在しないパスを除外
- テスト: restore → 正しい状態、存在しないパス除外、ファイル無し → None

### Step 7: `src/session.rs` — 期限切れクリーンアップ

- `cleanup_expired(expiry_days: u64) -> Result<usize>` — 削除件数を返す
- テスト: 期限切れファイル削除、期限内ファイル保持

### Step 8: `src/app.rs` — Save/Restore 統合

- 起動時: CLI / config に基づき `restore()` 呼び出し
  - 復元成功: expanded_paths を順に expand、cursor_path に移動
- 終了時: `SessionState::from_app()` → `save()`
- テスト + lint + format

## Complexity Tracking

該当なし — 全原則に適合。
