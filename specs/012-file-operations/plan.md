# Implementation Plan: File Operations + FS Change Detection + Undo/Redo + Session Persistence

**Branch**: `012-file-operations` | **Date**: 2026-02-15 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/012-file-operations/spec.md`

## Summary

trev にファイル操作（コピー、移動、削除、作成、リネーム）、マーク機能、undo/redo、FS 変更検出、セッション永続化を追加する。vim ライクな yank/paste モデルを採用し、vifm 方式の操作ペアテーブルによる undo/redo を永続化する。UI はインライン入力フィールドと確認ダイアログのモーダルシステムで構成する。

## Technical Context

**Language/Version**: Rust 2024 edition, nightly-2026-01-24
**Primary Dependencies**: ratatui 0.30, crossterm 0.29, tokio (full), notify 8, notify-debouncer-mini 0.5, trash 5, chrono 0.4, sha2 0.10
**Storage**: JSON ファイル（セッション永続化、undo 履歴）、ローカルファイルシステム
**Testing**: cargo test, rstest 0.26, googletest 0.14, tempfile 3
**Target Platform**: Linux, macOS, Windows (クロスプラットフォーム TUI)
**Project Type**: Single CLI application
**Performance Goals**: ファイル操作 <1s (100ファイル以下), undo/redo <1s, FS 変更反映 <500ms, セッション復元 <2s
**Constraints**: No unsafe, strict clippy (all/pedantic/nursery/cargo at deny), ブロッキング UI (操作中は入力不可)
**Scale/Scope**: 単一ユーザー、ローカルファイルシステム、undo スタック最大100エントリ

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. 安全な Rust | PASS | unsafe 不使用。全エラーは `anyhow::Result` で処理。`thiserror` でカスタムエラー型 |
| II. テスト駆動開発 | PASS | 全コンポーネントを `tempfile` + `rstest` で TDD。操作ペアの正逆検証がテスト設計の軸 |
| III. パフォーマンス設計 | PASS | ファイル操作はブロッキング（spec 決定）。IO バウンド処理は `spawn_blocking` で分離 |
| IV. シンプルさ & YAGNI | PASS | 非同期プログレスは不要（ブロッキング）。トランザクションなし（spec 決定）。最小限の状態管理 |
| V. インクリメンタルデリバリー | PASS | 5フェーズのボトムアップ構築: UI基盤 → ファイル操作 → undo → FS監視 → セッション |

## Project Structure

### Documentation (this feature)

```text
specs/012-file-operations/
├── plan.md              # This file
├── spec.md              # Feature specification
├── research.md          # Phase 0 research findings
├── data-model.md        # Entity definitions
├── quickstart.md        # Setup guide
├── checklists/
│   └── requirements.md  # Quality checklist
└── tasks.md             # (Phase 2 output — /speckit.tasks)
```

### Source Code (repository root)

```text
src/
├── file_op.rs             # [NEW] Module re-exports
├── file_op/
│   ├── executor.rs        # [NEW] FsOp/IrreversibleOp execution
│   ├── undo.rs            # [NEW] UndoHistory, OpGroup management
│   ├── yank.rs            # [NEW] YankBuffer, YankMode
│   ├── mark.rs            # [NEW] MarkSet (HashSet<PathBuf>)
│   ├── conflict.rs        # [NEW] Auto-rename (file_1.txt pattern)
│   └── trash.rs           # [NEW] Custom trash directory management
├── session.rs             # [NEW] SessionState save/restore
├── watcher.rs             # [NEW] notify watcher integration
├── input.rs               # [MODIFY] InputState, text editing
├── action.rs              # [MODIFY] Add FileOpAction variants
├── app.rs                 # [MODIFY] AppMode state machine, integration
├── config.rs              # [MODIFY] Add FileOpConfig, SessionConfig, WatcherConfig
├── cli.rs                 # [MODIFY] Add --restore/--no-restore
├── state/
│   └── tree.rs            # [MODIFY] ChildrenState::Stale, mark integration
├── ui/
│   ├── modal.rs           # [MODIFY] Confirmation dialog widget
│   ├── inline_input.rs    # [NEW] Inline input widget for tree view
│   ├── tree_view.rs       # [MODIFY] Mark display, inline input rendering
│   └── status_bar.rs      # [MODIFY] Yank/mark/result indicators
└── ui.rs                  # [MODIFY] Modal overlay rendering
```

**Structure Decision**: 既存の `src/` フラットモジュール構造を維持。`file_op/` サブモジュールで操作ロジックをグループ化。UI、FS 監視、セッションは独立モジュールとして追加。

## Complexity Tracking

> No Constitution violations. All design decisions align with core principles.

| Decision | Justification |
|----------|--------------|
| 5つの新規サブモジュール (file_op/) | 責務分離: executor/undo/yank/mark/conflict/trash は独立してテスト可能 |
| AppMode 状態マシン | 既存の "常に Normal" から3状態に拡張。モーダル入力の正しい処理に必須 |
| セッション永続化 | spec P3 要件。UndoHistory の永続化と統合することでシンプルに実現 |
