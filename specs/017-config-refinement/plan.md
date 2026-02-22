# Implementation Plan: 設定体系の整備

**Branch**: `017-config-refinement` | **Date**: 2026-02-22 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/017-config-refinement/spec.md`

## Summary

メニュー内のアクション（ソート6種・コピー5種）を個別アクションとして直接キーバインドに割り当て可能にする。ユーザー定義メニューを設定ファイルで自由に作成・キーバインドで呼び出せるようにする。設定項目名とアクション名を一貫性のあるわかりやすい命名に変更する。

技術アプローチ: 既存の Action enum を `SortAction` / `CopyAction` サブenumで階層拡張し、`FilterAction` をトップレベルenumとして新設し、ドット区切りアクション名（`tree.sort.by_name`, `filter.hidden`）を実現する。Config に `menus` セクションと `KeyBindingEntry.menu` フィールドを追加してユーザー定義メニューを実装する。JSON Schema のアクション一覧は `all_action_names()` ヘルパーで自動生成する。

## Technical Context

**Language/Version**: Rust 2024 edition, nightly-2026-01-24
**Primary Dependencies**: ratatui 0.30, crossterm 0.29, tokio (full), serde, schemars, clap 4, arboard (clipboard)
**Storage**: YAML config (`~/.config/trev/config.yml`), JSON Schema (on-demand generation)
**Testing**: `cargo test` with `googletest`, `rstest`, `tempfile`. Inline `#[cfg(test)]` modules.
**Target Platform**: macOS, Linux (terminal)
**Project Type**: Single Rust binary
**Performance Goals**: Config ロードは起動時のみ。アクション名の解析もパフォーマンスクリティカルではない。
**Constraints**: `unsafe` 禁止、`unwrap()`/`expect()`/`panic!()` 禁止。clippy 全グループ deny。
**Scale/Scope**: アクション数 ~40、設定フィールド数 ~30、ユーザー定義メニュー数は通常1〜5個程度

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Pre-Phase 0 Check

| Principle | Status | Notes |
|---|---|---|
| I. 安全な Rust | PASS | `unsafe` 不使用。すべての関数は `Result` を返す。config パース失敗は `anyhow::Result` で伝播 |
| II. テスト駆動開発 | PASS | TDD サイクルで実装。各アクション・設定パースにユニットテスト。メニュー定義のバリデーションテスト |
| III. パフォーマンス設計 | PASS | config ロードは起動時1回のみ。アクション解決は `HashMap` ルックアップ。ホットパスへの影響なし |
| IV. シンプルさ & YAGNI | PASS | 既存の enum 構造を自然に拡張。新しい抽象化は `SortAction`/`CopyAction` サブenum のみ。メニュー定義は `HashMap<String, MenuDefinition>` でシンプル |
| V. インクリメンタルデリバリー | PASS | P3→P1→P2 の順で段階的に実装。各段階でコンパイル・テスト・clippy パス |

### Post-Phase 1 Check

| Principle | Status | Notes |
|---|---|---|
| I. 安全な Rust | PASS | 設計に `unsafe` なし。メニュー定義の解決は `Option` + `Result` で安全にハンドリング |
| II. テスト駆動開発 | PASS | 各サブenumの `FromStr`/`Display`テスト、config パーステスト、メニュー解決テスト、スキーマ整合性テスト |
| III. パフォーマンス設計 | PASS | メニュー定義は config ロード時に解決。ランタイムは `HashMap::get` のみ |
| IV. シンプルさ & YAGNI | PASS | `MenuAction::Custom` は `Vec<MenuItemDef>` を保持するだけ。複雑なメニューエンジンは不要 |
| V. インクリメンタルデリバリー | PASS | data-model.md の構造は段階的に追加可能 |

## Project Structure

### Documentation (this feature)

```text
specs/017-config-refinement/
├── plan.md              # This file
├── spec.md              # Feature specification
├── research.md          # Phase 0: research findings
├── data-model.md        # Phase 1: data model changes
├── quickstart.md        # Phase 1: implementation quickstart
├── contracts/           # Phase 1: config schema contract
│   └── config-schema.md
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 output (/speckit.tasks command)
```

### Source Code (repository root)

```text
src/
├── action.rs            # Action enum 拡張 (SortAction, CopyAction, FilterAction, OpenMenu)
├── config.rs            # Config リネーム、MenuDefinition 追加、KeyBindingEntry.menu 追加
├── input.rs             # MenuAction::Custom 追加
├── app/
│   ├── keymap.rs        # menu フィールド解決処理追加
│   ├── key_parse.rs     # 変更なし
│   └── handler/
│       ├── tree.rs      # SortAction ハンドラ追加
│       ├── file_op.rs   # CopyAction ハンドラ追加
│       └── input.rs     # Custom メニューディスパッチ追加
├── ui/
│   └── modal.rs         # 変更なし（MenuState 構造は同じ）
└── cli.rs               # CLI 引数名の更新
```

**Structure Decision**: 既存のシングルプロジェクト構造を維持。新規ファイル追加なし、既存ファイルの変更のみ。

## Complexity Tracking

> Constitution Check に違反はないため、このセクションは空。
