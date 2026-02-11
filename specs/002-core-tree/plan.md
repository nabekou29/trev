# Implementation Plan: コアツリーデータ構造

**Branch**: `prototype` | **Date**: 2026-02-11 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/002-core-tree/spec.md`

## Summary

ファイルシステムのツリー構造を表現するデータ構造 (TreeNode, Tree, TreeBuilder) を実装する。遅延読み込み (ChildrenState)、バックグラウンド全ツリー走査 (SearchIndex)、ソート、visible nodes 生成、カーソル管理を含む。アーキテクチャ決定に基づき、TreeState は純粋データ構造として設計し、Action Dispatch パターンとの統合を前提とする。

## Technical Context

**Language/Version**: Rust 2024 edition, nightly-2026-01-24
**Primary Dependencies**: ignore 0.4 (WalkBuilder), tokio (background tasks), serde + serde_json (シリアライズ)
**Storage**: N/A (ファイルシステム直接読み取り)
**Testing**: googletest 0.14, rstest 0.26, tempfile 3 — インライン `#[cfg(test)]` モジュール
**Target Platform**: macOS / Linux (crossplatform, Unix 系)
**Project Type**: Single Rust binary
**Performance Goals**: visible_nodes() < 16ms @10k entries, 初期構築 < 100ms @1k files
**Constraints**: unsafe 禁止, unwrap/expect/panic 禁止, すべて Result ベース
**Scale/Scope**: 100k+ ファイルのディレクトリに対応（遅延読み込みで）

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. 安全な Rust | PASS | unsafe 禁止、Result ベース、clippy deny |
| II. テスト駆動開発 | PASS | spec に Acceptance Scenarios 定義済み、TDD で実装 |
| III. パフォーマンス設計 | PASS | NFR で目標値明示、参照ベース visible_nodes、遅延読み込み |
| IV. シンプルさ & YAGNI | PASS | Arena ベースツリー・ファイル監視を明示的にスコープ外 |
| V. インクリメンタルデリバリー | PASS | P1 → P2 の段階的実装、各ストーリーが独立テスト可能 |

## Project Structure

### Documentation (this feature)

```text
specs/002-core-tree/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

モダンモジュール命名規則 (Rust 2018+) を使用。`mod.rs` は使わない。

```text
src/
├── state.rs             # pub mod tree; (state サブモジュールの親)
├── state/
│   └── tree.rs          # TreeState, TreeNode, ChildrenState, VisibleNode, SortOrder, etc.
├── tree.rs              # pub mod builder; pub mod sort; pub mod search_index;
├── tree/
│   ├── builder.rs       # TreeBuilder (ignore::WalkBuilder wrapper)
│   ├── sort.rs          # ソートロジック
│   └── search_index.rs  # SearchIndex (バックグラウンド走査)
├── action.rs            # Action enum (TreeAction 含む) — 将来全 Action を集約
├── config.rs            # SortOrder, SortDirection は state/tree.rs へ移動
```

テストはソースファイル内に `#[cfg(test)]` モジュールとしてインラインで配置。

**Structure Decision**: 既存の `src/tree.rs` + `src/tree/builder.rs` を拡張し、新たに `src/state/tree.rs` を追加。state モジュールはアーキテクチャ決定に基づく Domain Module パターンの一部。

## Complexity Tracking

> No constitution violations. No complexity justification needed.
