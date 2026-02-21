# Implementation Plan: Smart Sort

**Branch**: `016-smart-sort` | **Date**: 2026-02-22 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/016-smart-sort/spec.md`

## Summary

Smart sort を実装し、natural sort（数値順）+ 接尾辞グルーピング（テストファイル等を対応ファイルの直後に配置）をデフォルトのソート方式とする。`S` キーでソート選択メニューを表示し、既存メニュー機構を `MenuAction` enum で汎用化する。

## Technical Context

**Language/Version**: Rust 2024 edition, nightly-2026-01-24
**Primary Dependencies**: ratatui 0.30, crossterm 0.29, tokio (full), clap 4, serde, schemars
**Storage**: N/A (インメモリ)
**Testing**: rstest + googletest, tempfile
**Target Platform**: macOS / Linux (TUI)
**Project Type**: Single Rust crate
**Performance Goals**: 100,000 ファイルのフラットディレクトリで smart sort が 1 秒以内
**Constraints**: visible nodes 計算 16ms 以内、起動 500ms 以内
**Scale/Scope**: ファイルツリーの並び替え、既存ソート機構への追加

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Gate | Status | Notes |
|------|--------|-------|
| I. 安全な Rust | PASS | unsafe 不使用、Result ベースのエラー処理 |
| II. テスト駆動開発 | PASS | sort ロジックは unit test で完全にカバー可能 |
| III. パフォーマンス設計 | PASS | sort は既存パス内で実行、追加アロケーション最小 |
| IV. シンプルさ & YAGNI | PASS | 外部 crate なし、自前実装 ~50 行 |
| V. インクリメンタルデリバリー | PASS | US1→US2→US3 の順で独立デリバリー可能 |

## Project Structure

### Documentation (this feature)

```text
specs/016-smart-sort/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
└── tasks.md             # Phase 2 output (by /speckit.tasks)
```

### Source Code (modified files)

```text
src/
├── tree/sort.rs         # Smart sort 実装 (natural sort + decompose_name)
├── state/tree.rs        # SortOrder に Smart variant 追加
├── config.rs            # config::SortOrder に Smart 追加、デフォルト変更
├── action.rs            # TreeAction に SortMenu, ToggleSortDirection 追加
├── input.rs             # MenuState に on_select: MenuAction 追加
├── app/handler/input.rs # handle_menu_mode_key を MenuAction 対応に汎用化
├── app/handler/tree.rs  # SortMenu, ToggleSortDirection ハンドラ追加
├── app/keymap.rs        # S → SortMenu, s → ToggleSortDirection バインド
├── cli.rs               # --sort-order に smart 追加 (clap::ValueEnum 自動)
└── ui/modal.rs          # メニュー表示で現在選択中のハイライト対応
```

**Structure Decision**: 既存ファイルへの追加・変更のみ。新規ファイルなし。

## Key Design Decisions

### 1. Smart Sort の比較キー順序

```
compare_smart(a, b):
  1. decompose_name(a) → (base_a, suffix_a)
     decompose_name(b) → (base_b, suffix_b)
  2. compare_natural(base_a, base_b)  // ベース名で natural sort
  3. if equal: suffix_a.is_none() < suffix_b.is_none()  // 接尾辞なしが先
  4. if both have suffix: compare_natural(suffix_a, suffix_b)  // 接尾辞同士
  5. tie-break: compare_natural(a.full_name, b.full_name)  // 完全一致回避
```

### 2. Natural Sort 実装

チャンクベース比較。数値は文字列のまま（桁数比較 → 辞書順比較）で任意精度をサポート。

```rust
enum Chunk<'a> {
    Text(&'a str),
    Num(&'a str),  // 数字列を文字列のまま保持
}
```

- Text vs Text: case-insensitive 比較、同値なら case-sensitive で tie-break
- Num vs Num: 桁数比較 → 辞書順比較（u128 パースなし）
- Text vs Num: 数字が先（ファイル名の慣例）

### 3. 接尾辞分解 (decompose_name)

research.md のアルゴリズムに従う:
1. 拡張子なし → `(name, None)`
2. stem が `_test` or `_spec` で終わる → underscore 分解
3. stem にドットがある → 最後のドットで分解
4. 上記いずれも該当しない → `(name, None)`

### 4. メニュー汎用化

```rust
// input.rs に追加
#[derive(Debug, Clone, Copy)]
pub enum MenuAction {
    CopyToClipboard,
    SelectSortOrder,
}

// MenuState に追加
pub struct MenuState {
    pub title: String,
    pub items: Vec<MenuItem>,
    pub cursor: usize,
    pub on_select: MenuAction,  // NEW
}
```

`handle_menu_mode_key` の Enter/shortcut 処理を `match menu.on_select` で分岐:
- `CopyToClipboard` → 既存の `copy_to_clipboard` 呼び出し
- `SelectSortOrder` → `MenuItem.value` を `SortOrder` にパースして `apply_sort`

### 5. デフォルト変更の影響

- `config::SortOrder::default()` を `Name` → `Smart` に変更
- `state::tree::SortOrder` にも `Smart` を追加し `From<config::SortOrder>` を更新
- 既存テストで `SortOrder::Name` を明示的に指定しているものは影響なし
- 設定ファイルに `sort.order: name` と書いていた既存ユーザーは影響なし

## Complexity Tracking

> No violations. All changes fit within existing patterns.
