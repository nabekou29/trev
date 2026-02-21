# Tasks: Column Layout Design

**Input**: Design documents from `/specs/015-column-layout-design/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md

**Tests**: Required (constitution II: TDD 必須)

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: 依存クレート追加とモジュール構造の準備

- [x] T001 Add `time = { version = "0.3", features = ["local-offset"] }` and `unicode-width = "0.2"` to Cargo.toml
- [x] T002 Create `src/ui/column.rs` module file and register it as `pub mod column;` in `src/ui.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: 全ユーザーストーリーが依存するコアデータ型とフォーマット関数

**⚠️ CRITICAL**: US1/US2/US3 はこのフェーズ完了後に開始可能

- [x] T003 [P] Define `ColumnKind` enum (`Size`, `ModifiedAt`, `GitStatus`) with `width()` method returning fixed column widths (5, 10, 2) and `total_columns_width()` helper in `src/ui/column.rs`. Define `MtimeFormat` enum (`Relative`, `Absolute`) with `#[default] Relative`. Define `ColumnEntry` untagged enum (`Simple(ColumnKind)`, `WithOptions { kind, format }`) and `ResolvedColumn` struct (`kind: ColumnKind`, `mtime_format: MtimeFormat`). Include serde/JsonSchema derives with `rename_all = "snake_case"`. Add `ColumnEntry::resolve() -> ResolvedColumn` and `resolve_columns(entries: &[ColumnEntry]) -> Vec<ResolvedColumn>` helper
- [x] T004 [P] Add `symlink_target: Option<String>` field to `TreeNode` in `src/state/tree.rs`. Update all TreeNode construction sites: `src/tree/builder.rs` (build, load_children), test fixtures in `src/state/tree.rs`, `src/tree/sort.rs`, `src/tree/search_index.rs`
- [x] T005 Populate `symlink_target` in `src/tree/builder.rs`: call `std::fs::read_link(path).ok().map(|t| t.to_string_lossy().into_owned())` when `is_symlink == true` in `load_children()`. Root node gets `None`
- [x] T006 [P] TDD: Implement `format_size(size: u64, is_dir: bool) -> String` in `src/ui/column.rs`. Tests first: 0→`"  0B"`, 1023→`"1023B"`, 1024→`" 1.0K"`, 1536→`" 1.5K"`, 1048576→`" 1.0M"`, dir→`"   -"`. Fixed 4-char right-aligned output
- [x] T007 [P] TDD: Implement `format_mtime(modified: Option<SystemTime>, fmt: MtimeFormat) -> String` in `src/ui/column.rs`. Tests first: None→`"-"`, Relative mode: <1h→`"Xm ago"`, <24h→`"Xh ago"`, <7d→`"Xd ago"`, <30d→`"Xw ago"`, >=30d→`"YYYY-MM-DD"`. Absolute mode: always `"YYYY-MM-DD"`. Use `time::OffsetDateTime` for ISO format. Fixed 10-char right-aligned output
- [x] T008 [P] TDD: Implement `truncate_to_width(text: &str, max_width: usize) -> String` in `src/ui/column.rs`. Tests first: ASCII truncation, CJK (2-width chars) truncation, "…" suffix, no-op when fits. Use `unicode-width::UnicodeWidthStr`

**Checkpoint**: フォーマット関数が全てテスト済みで使用可能

---

## Phase 3: User Story 1 - ファイルメタデータの表示 (Priority: P1) 🎯 MVP

**Goal**: デフォルト設定でサイズ・更新日時・gitステータスが右寄せで表示される

**Independent Test**: `cargo run` で起動し、各ファイル行の右側にサイズ・mtime・git status カラムが表示されることを確認

### Implementation for User Story 1

- [x] T009 [US1] Add `columns: Vec<ResolvedColumn>` field to `AppState` in `src/app/state.rs`. Default: `vec![ResolvedColumn::new(Size, default), ResolvedColumn::new(ModifiedAt, default), ResolvedColumn::new(GitStatus, default)]`
- [x] T010 [US1] Initialize `AppState.columns` in `init_app()` in `src/app.rs` with the default column list using `resolve_columns()`. Filter out `GitStatus` if `!git_enabled`
- [x] T011 [US1] Rewrite `render_tree()` in `src/ui/tree_view.rs` to use column system: (1) calculate `columns_width` from `state.columns`, (2) calculate `name_width = area_width - left_content_width - columns_width`, (3) truncate name+symlink indicator to `name_width`, (4) render column spans right-aligned using `format_size`/`format_mtime(modified, col.mtime_format)`/`git_status_indicator`, (5) remove `METADATA_WIDTH` constant and old git-status padding logic
- [x] T012 [US1] Show symlink indicator in name area: append `" → {target}"` span (DarkGray color) after name for nodes with `symlink_target.is_some()`, before truncation
- [x] T013 [US1] Update `AppState` initialization in `benches/render.rs` and `src/app/handler/ipc.rs` with `columns` field
- [x] T014 [US1] Run `mise run build && mise run lint && mise run test` — fix any compilation or lint issues

**Checkpoint**: US1 完了 — デフォルトカラムが表示され、git status は column system に統合済み

---

## Phase 4: User Story 2 - カラム表示の設定 (Priority: P2)

**Goal**: config.yml の `display.columns` で表示カラムの種類と順序をカスタマイズ可能

**Independent Test**: config.yml で `columns: [size]` に設定し、サイズのみ表示されることを確認

### Tests for User Story 2

- [x] T015 [P] [US2] TDD: Add config tests in `src/config.rs`: (1) default columns = [size, modified_at, git_status], (2) YAML `columns: [size]` → Size only (Simple form), (3) YAML `columns: []` → empty, (4) YAML `columns: [git_status, size, modified_at]` → reordered, (5) YAML `columns: [{kind: modified_at, format: absolute}]` → WithOptions form, (6) mixed simple/object entries, (7) unknown column name → error

### Implementation for User Story 2

- [x] T016 [US2] Add `columns: Vec<ColumnEntry>` to `DisplayConfig` in `src/config.rs` with default `[Simple(Size), Simple(ModifiedAt), Simple(GitStatus)]`. Import `ColumnEntry`, `ColumnKind` from `crate::ui::column`
- [x] T017 [US2] Wire `config.display.columns` to `AppState.columns` in `src/app.rs` `init_app()` using `resolve_columns()`, replacing the hardcoded default. Apply `git_enabled` filter on resolved columns
- [x] T018 [US2] Update JSON schema test `schema_contains_all_top_level_sections` and add `schema_display_has_columns_field` test in `src/config.rs`
- [x] T019 [US2] Run `mise run build && mise run lint && mise run test` — fix any issues

**Checkpoint**: US2 完了 — config.yml でカラム設定が反映される

---

## Phase 5: User Story 3 - プレビュー分割比率の設定 (Priority: P3)

**Goal**: config.yml でプレビュー分割比率とブレイクポイントをカスタマイズ可能

**Independent Test**: config.yml で `preview.split: 40` に設定し、ツリー40%/プレビュー60%になることを確認

### Tests for User Story 3

- [x] T020 [P] [US3] TDD: Add config tests in `src/config.rs`: (1) default split=50, narrow_split=60, narrow_threshold=80, (2) YAML overrides for each field, (3) existing preview config fields unaffected

### Implementation for User Story 3

- [x] T021 [US3] Add `split: u16`, `narrow_split: u16`, `narrow_threshold: u16` to `PreviewConfig` in `src/config.rs` with defaults (50, 60, 80)
- [x] T022 [US3] Add `layout_split: u16`, `layout_narrow_split: u16`, `layout_narrow_threshold: u16` to `AppContext` in `src/app/state.rs`. Wire from `config.preview` in `src/app.rs` `init_app()`
- [x] T023 [US3] Replace hardcoded `NARROW_WIDTH_THRESHOLD` constant in `src/ui.rs` with `ctx.layout_narrow_threshold` (passed via AppState or render parameter). Replace `Percentage(50)`/`Percentage(60)` with `ctx.layout_split`/`ctx.layout_narrow_split`. Also update the auto-hide check in `src/app.rs` that references `NARROW_WIDTH_THRESHOLD`
- [x] T024 [US3] Run `mise run build && mise run lint && mise run test` — fix any issues

**Checkpoint**: US3 完了 — プレビューレイアウトが設定可能

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: 全ストーリー横断の品質改善

- [x] T025 Run quickstart.md manual validation: test all 3 user stories with sample config
- [x] T026 Verify `cargo run -- schema` output includes `display.columns`, `preview.split`, `preview.narrow_split`, `preview.narrow_threshold`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 依存なし — 即開始可能
- **Foundational (Phase 2)**: Setup 完了後に開始。全 US をブロック
- **US1 (Phase 3)**: Foundational 完了後に開始
- **US2 (Phase 4)**: US1 完了後に開始（columns field が AppState に必要）
- **US3 (Phase 5)**: Foundational 完了後に開始（US1/US2 と独立）
- **Polish (Phase 6)**: 全 US 完了後

### User Story Dependencies

- **US1 (P1)**: Foundational 完了が前提。他 US への依存なし
- **US2 (P2)**: US1 完了が前提（AppState.columns のデフォルト初期化を config に移行）
- **US3 (P3)**: Foundational 完了が前提。US1/US2 と独立して実装可能

### Within Each Phase

- TDD: テストを先に書き、FAIL を確認してから実装
- [P] タスクは並行実行可能
- 各フェーズ末尾の build/lint/test で品質ゲート

### Parallel Opportunities

Phase 2 内:
- T003 (ColumnKind), T004 (symlink_target), T006 (format_size), T007 (format_mtime), T008 (truncate) は全て並列可能

Phase 4 + Phase 5:
- US3 は US1 完了後に US2 と並列実行可能

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1: Setup (T001-T002)
2. Phase 2: Foundational (T003-T008)
3. Phase 3: US1 (T009-T014)
4. **STOP and VALIDATE**: `cargo run` でカラム表示を確認
5. デモ可能な状態

### Incremental Delivery

1. Setup + Foundational → フォーマット関数テスト済み
2. US1 → カラム表示 MVP → 手動確認
3. US2 → config.yml でカスタマイズ可能 → 手動確認
4. US3 → レイアウト設定可能 → 手動確認
5. Polish → 全体品質確認

---

## Notes

- [P] tasks = 異なるファイル、依存なし
- [Story] label = ユーザーストーリーへのマッピング
- Constitution II: TDD 必須 — テスト → FAIL → 実装 → PASS → リファクタ
- `unicode-width` は ratatui の transitive dependency だが direct dependency として追加
- `time` クレートは ISO 日付フォーマットに必要（research.md Decision 3）
- 各チェックポイントで `mise run build && mise run lint && mise run test` を実行
