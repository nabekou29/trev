# Implementation Plan: Git Integration

**Branch**: `014-git-integration` | **Date**: 2026-02-18 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/014-git-integration/spec.md`

## Summary

Git リポジトリ内のファイルステータスをツリービューに色付きインジケーターとして表示し、ディレクトリへの集約表示、FS watcher 連動の自動更新、`R` キーによる手動リフレッシュ、カスタムプレビューの `git_status` 条件フィルタリングを提供する。

## Technical Context

**Language/Version**: Rust 2024 edition, nightly-2026-01-24
**Primary Dependencies**: ratatui 0.30, crossterm 0.29, tokio (full), ignore 0.4
**Storage**: N/A (git status はインメモリで保持、永続化不要)
**Testing**: cargo test, rstest, googletest, tempfile
**Target Platform**: macOS / Linux ターミナル
**Project Type**: Single Rust binary (TUI)
**Performance Goals**: 1万ファイルのリポジトリで git status 取得 500ms 以内、UI ブロックなし
**Constraints**: 非同期バックグラウンド処理必須。`unsafe` コード禁止。strict clippy。
**Scale/Scope**: 一般的な開発リポジトリ (100〜10,000 ファイル)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Gate | Status | Notes |
|------|--------|-------|
| I. 安全な Rust | PASS | `unsafe` 不使用。`git status` は `std::process::Command` で外部プロセス実行。エラーは `anyhow::Result` で伝搬。 |
| II. TDD (必須) | PASS | 全モジュールでインラインテスト。`tempfile` + `git init` でリポジトリ環境をテスト内に構築。 |
| III. パフォーマンス設計 | PASS | Git status は tokio バックグラウンドタスクで非同期取得。UI スレッドをブロックしない。HashMap で O(1) ルックアップ。 |
| IV. シンプルさ & YAGNI | PASS | git2 crate ではなく `git status --porcelain=v1` の CLI パースで最もシンプルに実装。ブランチ表示は明示的にスコープ外。 |
| V. インクリメンタルデリバリー | PASS | ボトムアップ: データモデル → パース → 非同期取得 → UI 表示 → watcher 連携 → プレビュー条件 の順。各ステップでテスト通過。 |

## Project Structure

### Documentation (this feature)

```text
specs/014-git-integration/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
src/
├── git.rs               # NEW: Git status module (GitFileStatus, GitState, porcelain parser)
├── config.rs            # MODIFY: Add GitConfig
├── cli.rs               # MODIFY: Add --no-git flag
├── action.rs            # MODIFY: Add TreeAction::Refresh
├── app.rs               # MODIFY: Add git_tx/git_rx channel, trigger on watcher events
├── app/
│   ├── state.rs         # MODIFY: Add git_state to AppState, git_tx to AppContext
│   ├── handler.rs       # MODIFY: Add Refresh action handler, git status triggers
│   ├── handler/         # (existing handler modules)
│   └── keymap.rs        # MODIFY: Add R → tree.refresh default binding
├── ui/
│   └── tree_view.rs     # MODIFY: Show git status in right-side metadata area
└── preview/
    └── providers/
        └── external.rs  # MODIFY: Add git_status condition to can_handle
```

**Structure Decision**: 既存の単一プロジェクト構造に従い、新規モジュール `src/git.rs` を追加。Git ロジックはこのモジュールに集約し、他モジュールは `GitState` を参照のみ。

## Complexity Tracking

No violations to justify.
