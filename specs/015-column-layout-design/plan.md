# Implementation Plan: Column Layout Design

**Branch**: `015-column-layout-design` | **Date**: 2026-02-21 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/015-column-layout-design/spec.md`

## Summary

ツリービューにファイルサイズ・更新日時・gitステータスのメタデータカラムを右寄せ固定幅で表示する。カラムは設定ファイルで種類・順序をカスタマイズ可能。プレビュー分割比率・ブレイクポイントも設定可能にする。

## Technical Context

**Language/Version**: Rust 2024 edition, nightly-2026-01-24
**Primary Dependencies**: ratatui 0.30, crossterm 0.29, tokio (full), ignore 0.4
**Storage**: N/A（インメモリ状態のみ）
**Testing**: rstest + googletest, in-source `#[cfg(test)]` modules
**Target Platform**: Terminal (Unix/macOS)
**Project Type**: Single Rust binary (TUI)
**Performance Goals**: 1000ファイルのディレクトリでスクロール性能劣化なし（SC-004）
**Constraints**: strict clippy (all/pedantic/nursery/cargo deny), no unsafe, 各カラムは固定幅
**Scale/Scope**: TreeNode に `size`/`modified` 既存。レンダリングは visible rows のみ

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| 原則 | 状態 | 備考 |
|------|------|------|
| I. 安全な Rust | ✅ PASS | unsafe 不要。`read_link` は `Result` 返却 |
| II. テスト駆動開発 | ✅ PASS | format 関数、config デシリアライズ、カラム幅計算をテスト |
| III. パフォーマンス設計 | ✅ PASS | visible rows のみフォーマット。`read_link` は builder 内で1回のみ |
| IV. シンプルさ & YAGNI | ✅ PASS | 動的レスポンシブ非表示は除外済。カラムは固定幅で常に表示 |
| V. インクリメンタルデリバリー | ✅ PASS | P1(表示)→P2(設定)→P3(レイアウト設定) で段階的 |

## Project Structure

### Documentation (this feature)

```text
specs/015-column-layout-design/
├── plan.md              # This file
├── spec.md              # Feature specification
├── research.md          # Phase 0: technical decisions
├── data-model.md        # Phase 1: entity definitions
├── checklists/
│   └── requirements.md  # Spec quality checklist
└── tasks.md             # Phase 2 output (via /speckit.tasks)
```

### Source Code (repository root)

```text
src/
├── ui/
│   ├── column.rs         # NEW: ColumnKind, ColumnEntry, ResolvedColumn, MtimeFormat, format_size(), format_mtime(), column width calc
│   ├── tree_view.rs      # MODIFY: カラム付きレンダリング、名前切り詰め
│   └── ...               # (他の ui モジュールは変更なし)
├── config.rs             # MODIFY: ColumnConfig, LayoutConfig 追加
├── app.rs                # MODIFY: init_app で column/layout config を適用
├── app/state.rs          # MODIFY: AppState にカラム設定を保持
├── state/tree.rs         # MODIFY: TreeNode に symlink_target 追加
└── tree/
    └── builder.rs        # MODIFY: read_link で symlink_target を取得
```

**Structure Decision**: 既存の `src/ui/` モジュール構造に `column.rs` を追加。フォーマット関数とカラム型定義は純粋関数として独立テスト可能にする。

## Design

### カラムシステム

**ColumnKind** — 表示可能なカラム種別:
- `Size`: ファイルサイズ（固定幅 5文字、右寄せ。例: ` 1.2K`, `  42B`, `   -`）
- `ModifiedAt`: 更新日時（固定幅 10文字、右寄せ。例: `2024-01-15`, `    3d ago`）。`format` オプション: `relative`（デフォルト）/ `absolute`
- `GitStatus`: gitステータス（固定幅 2文字。例: `M `, `? `）

各カラム間に1文字のスペース区切り。全カラム合計幅: 5+1+10+1+2 = 20文字

**ColumnEntry** — 設定ファイルの各エントリ:
- 文字列形式: `- size` → `ColumnEntry::Simple(ColumnKind::Size)`
- オブジェクト形式: `- kind: modified_at\n  format: absolute` → `ColumnEntry::WithOptions { kind: ModifiedAt, format: Some(Absolute) }`
- serde `#[serde(untagged)]` で自動判別

**ResolvedColumn** — 実行時のカラム情報:
- `ColumnEntry` から解決。`kind: ColumnKind` + `mtime_format: MtimeFormat`
- `AppState.columns: Vec<ResolvedColumn>` として保持

**レンダリングフロー**:
1. 右端から: カラム幅合計を計算（設定で有効なカラムのみ）
2. 左端: selection(2) + indent(2*depth) + caret(2) + icon(0 or 2) + name(残り幅)
3. name は残り幅に収まるよう切り詰め（Unicode 対応で `…` 付加）
4. name とカラムの間にパディングスペースを挿入

### 設定構造

```yaml
display:
  # デフォルト: 全て表示（文字列形式）
  columns:
    - size
    - modified_at
    - git_status
  # オプション付き:
  # columns:
  #   - size
  #   - kind: modified_at
  #     format: absolute
  #   - git_status
  # 既存: show_hidden, show_ignored, show_preview, show_root

preview:
  # 既存フィールドに追加:
  split: 50              # wide モードのツリー幅 % (デフォルト: 50)
  narrow_split: 60       # narrow モードのツリー幅 % (デフォルト: 60)
  narrow_threshold: 80   # 縦横切替のブレイクポイント幅 (デフォルト: 80)
```

### TreeNode 変更

```rust
pub struct TreeNode {
    // ... 既存フィールド ...
    pub symlink_target: Option<String>,  // NEW: read_link の結果
}
```

`builder.rs` の `load_children()` 内で `is_symlink` が true の場合に `std::fs::read_link` を呼び、結果を `symlink_target` に格納。失敗時は `None`。

### 名前表示でのシンボリックリンク

名前スパンの後に `" → target"` を薄い色で追加。これは名前領域の一部として切り詰め対象。

## Complexity Tracking

違反なし。全てシンプルな設計。
