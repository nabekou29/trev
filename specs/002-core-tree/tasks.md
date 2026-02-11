# Tasks: コアツリーデータ構造

**Input**: Design documents from `/specs/002-core-tree/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, quickstart.md
**Tests**: TDD 必須 (Constitution 原則 II)

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup (Module Structure)

**Purpose**: plan.md に基づくモジュール構造の準備

- [x] T001 Create `src/state.rs` with `pub mod tree;` declaration
- [x] T002 Create empty `src/state/tree.rs` with module doc comment
- [x] T003 [P] Create `src/tree/sort.rs` with module doc comment
- [x] T004 [P] Create `src/tree/search_index.rs` with module doc comment
- [x] T005 [P] Create `src/action.rs` with placeholder `Action` enum
- [x] T006 Add `mod state;` and `mod action;` to `src/main.rs`, add `pub mod sort;` and `pub mod search_index;` to `src/tree.rs`
- [x] T007 Verify build passes with `mise run build && mise run lint`

---

## Phase 2: Foundational (Core Data Types)

**Purpose**: 全ユーザーストーリーが依存するデータ型の定義

- [x] T008 Implement `TreeNode` struct in `src/state/tree.rs` (name, path, is_dir, is_symlink, size, modified, children, is_expanded)
- [x] T009 Implement `ChildrenState` enum in `src/state/tree.rs` (NotLoaded, Loading, Loaded)
- [x] T010 [P] Implement `SortOrder` and `SortDirection` enums in `src/state/tree.rs` with Serialize/Deserialize
- [x] T011 [P] Implement `VisibleNode<'a>` struct in `src/state/tree.rs` (node reference + depth)
- [x] T012 [P] Implement `NodeInfo` struct in `src/state/tree.rs` with Serialize/Deserialize
- [x] T013 Implement `TreeState` struct in `src/state/tree.rs` (root, cursor, sort_order, sort_direction, directories_first)
- [x] T014 [P] Implement `TreeAction` enum in `src/action.rs` per data-model.md
- [x] T015 Move `SortOrder` and `SortDirection` from `src/config.rs` to `src/state/tree.rs` and re-export or update `config.rs` to use state types
- [x] T016 Verify build and lint pass with `mise run build && mise run lint`

**Checkpoint**: Core types defined — user story implementation can begin

---

## Phase 3: User Story 1 — ファイルシステムからのツリー構築 (Priority: P1) 🎯 MVP

**Goal**: `TreeBuilder` で指定パスからツリーを構築。ルート直下 1 階層のみ読み込み、`.gitignore` 尊重、隠しファイル制御。

**Independent Test**: tempdir にファイル構造を作成 → `TreeBuilder::build()` → 正しい TreeNode ツリーを検証

### Tests for User Story 1 (TDD: Red phase)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T017 [P] [US1] Test: `build()` returns root with immediate children, subdirectories have `NotLoaded` in `src/tree/builder.rs`
- [x] T018 [P] [US1] Test: `.gitignore` entries are excluded from tree in `src/tree/builder.rs`
- [x] T019 [P] [US1] Test: `show_hidden = false` excludes dotfiles, `show_hidden = true` includes them in `src/tree/builder.rs`
- [x] T020 [P] [US1] Test: non-existent path returns error in `src/tree/builder.rs`
- [x] T021 [P] [US1] Test: symlinks are included with `is_symlink = true` in `src/tree/builder.rs`

### Implementation for User Story 1 (TDD: Green phase)

- [x] T022 [US1] Implement `TreeBuilder::new(show_hidden, show_ignored)` in `src/tree/builder.rs`
- [x] T023 [US1] Implement `TreeBuilder::build(&self, root_path: &Path) -> Result<TreeNode>` using `ignore::WalkBuilder` with `max_depth(Some(1))` in `src/tree/builder.rs`
- [x] T024 [US1] Implement `TreeBuilder::load_children(&self, dir_path: &Path) -> Result<Vec<TreeNode>>` for single-level child loading in `src/tree/builder.rs`
- [x] T025 [US1] Verify all US1 tests pass with `mise run test`

**Checkpoint**: TreeBuilder works — can build a tree from any directory

---

## Phase 4: User Story 3 — ツリーのソート (Priority: P1)

**Goal**: children の再帰的ソート。Name/Size/Modified/Type/Extension、Asc/Desc、directories_first 対応。

**Independent Test**: ソート適用後の children 順序を直接検証

### Tests for User Story 3 (TDD: Red phase)

- [x] T026 [P] [US3] Test: `SortOrder::Name` + `Asc` sorts case-insensitively in `src/tree/sort.rs`
- [x] T027 [P] [US3] Test: `SortOrder::Size` + `Desc` sorts by size descending in `src/tree/sort.rs`
- [x] T028 [P] [US3] Test: `directories_first = true` groups directories at top in `src/tree/sort.rs`
- [x] T029 [P] [US3] Test: sort applies recursively to nested `Loaded` children in `src/tree/sort.rs`
- [x] T030 [P] [US3] Test: `modified = None` entries sort to end in `src/tree/sort.rs`

### Implementation for User Story 3 (TDD: Green phase)

- [x] T031 [US3] Implement `sort_children(children: &mut [TreeNode], order, direction, dirs_first)` in `src/tree/sort.rs`
- [x] T032 [US3] Implement `apply_sort_recursive(node: &mut TreeNode, order, direction, dirs_first)` for recursive sort in `src/tree/sort.rs`
- [x] T033 [US3] Verify all US3 tests pass with `mise run test`

**Checkpoint**: Sort works — trees can be sorted by any criteria

---

## Phase 5: User Story 4 — Visible Nodes の生成 (Priority: P1)

**Goal**: 展開状態に基づくフラットリスト生成。参照ベース、clone なし。

**Independent Test**: ツリーの展開/折り畳みを操作 → `visible_nodes()` の結果を検証

### Tests for User Story 4 (TDD: Red phase)

- [x] T034 [P] [US4] Test: root-only expanded tree returns depth=0 entries in `src/state/tree.rs`
- [x] T035 [P] [US4] Test: expanded subdirectory includes children with correct depth in `src/state/tree.rs`
- [x] T036 [P] [US4] Test: collapsed directory excludes children from visible nodes in `src/state/tree.rs`
- [x] T037 [P] [US4] Test: `NotLoaded` directory is visible but has no children in output in `src/state/tree.rs`

### Implementation for User Story 4 (TDD: Green phase)

- [x] T038 [US4] Implement `TreeState::visible_nodes(&self) -> Vec<VisibleNode<'_>>` with DFS walk in `src/state/tree.rs`
- [x] T039 [US4] Implement `TreeState::visible_node_count(&self) -> usize` helper in `src/state/tree.rs`
- [x] T040 [US4] Verify all US4 tests pass with `mise run test`

**Checkpoint**: Visible nodes generation works — UI can render a flat list from tree state

---

## Phase 6: User Story 2 — 遅延読み込みとディレクトリ展開 (Priority: P1)

**Goal**: `set_children()` で子ノードを反映。`toggle_expand` / `collapse` / `expand_or_open` ロジック。

**Independent Test**: TreeState に children を設定 → visible_nodes で反映を検証

### Tests for User Story 2 (TDD: Red phase)

- [x] T041 [P] [US2] Test: `set_children()` changes `NotLoaded` → `Loaded` and visible_nodes reflects children in `src/state/tree.rs`
- [x] T042 [P] [US2] Test: `toggle_expand()` on expanded dir collapses it (children hidden in visible_nodes) in `src/state/tree.rs`
- [x] T043 [P] [US2] Test: `toggle_expand()` on collapsed `Loaded` dir re-expands without reload in `src/state/tree.rs`
- [x] T044 [P] [US2] Test: `set_children_error()` reverts `Loading` → `NotLoaded` in `src/state/tree.rs`

### Implementation for User Story 2 (TDD: Green phase)

- [x] T045 [US2] Implement `TreeState::set_children(&mut self, path: &Path, children: Vec<TreeNode>)` — find node by path, set `Loaded`, apply sort in `src/state/tree.rs`
- [x] T046 [US2] Implement `TreeState::set_children_error(&mut self, path: &Path)` — revert to `NotLoaded` in `src/state/tree.rs`
- [x] T047 [US2] Implement `TreeState::toggle_expand(&mut self, index: usize)` in `src/state/tree.rs`
- [x] T048 [US2] Verify all US2 tests pass with `mise run test`

**Checkpoint**: Lazy loading works — directories can be expanded/collapsed with async child loading

---

## Phase 7: User Story 5 — カーソル管理 (Priority: P2)

**Goal**: visible nodes インデックスベースのカーソル。バウンドチェック、ジャンプ、半ページ移動。

**Independent Test**: カーソル操作 → 正しい位置を検証

### Tests for User Story 5 (TDD: Red phase)

- [x] T049 [P] [US5] Test: cursor at 0, move up stays at 0 (bound check) in `src/state/tree.rs`
- [x] T050 [P] [US5] Test: cursor at last, move down stays at last in `src/state/tree.rs`
- [x] T051 [P] [US5] Test: `half_page_down(viewport_height=20)` moves cursor by 10 in `src/state/tree.rs`
- [x] T052 [P] [US5] Test: `jump_to_first` sets cursor to 0, `jump_to_last` sets cursor to last in `src/state/tree.rs`
- [x] T053 [P] [US5] Test: collapse hides cursor target → cursor moves to collapsed directory in `src/state/tree.rs`
- [x] T054 [P] [US5] Test: collapse on file → cursor moves to parent directory in `src/state/tree.rs`

### Implementation for User Story 5 (TDD: Green phase)

- [x] T055 [US5] Implement `TreeState::move_cursor(&mut self, delta: i32)` with bound check in `src/state/tree.rs`
- [x] T056 [US5] Implement `TreeState::move_cursor_to(&mut self, index: usize)` in `src/state/tree.rs`
- [x] T057 [US5] Implement `TreeState::jump_to_first(&mut self)` and `jump_to_last(&mut self)` in `src/state/tree.rs`
- [x] T058 [US5] Implement `TreeState::half_page_down(&mut self, viewport_height: usize)` and `half_page_up` in `src/state/tree.rs`
- [x] T059 [US5] Implement `TreeState::collapse(&mut self)` — collapse current or move to parent in `src/state/tree.rs`
- [x] T060 [US5] Implement `TreeState::expand_or_open(&mut self)` — expand dir or signal file open in `src/state/tree.rs`
- [x] T061 [US5] Implement `TreeState::current_node_info(&self) -> Option<NodeInfo>` for IPC/serialization in `src/state/tree.rs`
- [x] T062 [US5] Verify all US5 tests pass with `mise run test`

**Checkpoint**: Cursor management works — all navigation within tree is functional

---

## Phase 8: User Story 6 — バックグラウンド全ツリー走査 (Priority: P2)

**Goal**: SearchIndex 構築。バックグラウンドで全ファイル走査、部分結果アクセス可能。

**Independent Test**: tempdir で走査 → 全エントリが SearchIndex に含まれることを検証

### Tests for User Story 6 (TDD: Red phase)

- [x] T063 [P] [US6] Test: `SearchIndex::add_entry()` and `entries()` return added entries in `src/tree/search_index.rs`
- [x] T064 [P] [US6] Test: `is_complete()` returns false initially, true after marking complete in `src/tree/search_index.rs`
- [x] T065 [P] [US6] Test: `find_children(parent_path)` returns direct children only in `src/tree/search_index.rs`
- [x] T066 [P] [US6] Test: `.gitignore` entries are excluded from index (show_ignored=false) in `src/tree/search_index.rs`

### Implementation for User Story 6 (TDD: Green phase)

- [x] T067 [US6] Implement `SearchEntry` struct in `src/tree/search_index.rs`
- [x] T068 [US6] Implement `SearchIndex` struct with `add_entry`, `entries`, `is_complete`, `mark_complete`, `find_children` in `src/tree/search_index.rs`
- [x] T069 [US6] Implement `build_search_index(root_path, show_hidden, show_ignored) -> SearchIndex` using `ignore::WalkBuilder` full scan in `src/tree/search_index.rs`
- [x] T070 [US6] Verify all US6 tests pass with `mise run test`

**Checkpoint**: SearchIndex works — background scan builds a flat index of all files

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: 統合確認とパフォーマンス検証

- [x] T071 Integration test: TreeBuilder → sort → visible_nodes end-to-end in `src/state/tree.rs`
- [x] T072 Integration test: TreeBuilder + set_children + cursor navigation end-to-end in `src/state/tree.rs`
- [x] T073 [P] Performance test (ignored by default): visible_nodes < 16ms @ 10k entries in `src/state/tree.rs`
- [x] T074 [P] Performance test (ignored by default): TreeBuilder::build < 100ms @ 1k files in `src/tree/builder.rs`
- [x] T075 Run full validation: `mise run build && mise run test && mise run lint`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 — defines types all stories need
- **US1 (Phase 3)**: Depends on Phase 2 — TreeBuilder needs TreeNode, ChildrenState
- **US3 (Phase 4)**: Depends on Phase 2 — Sort needs TreeNode
- **US4 (Phase 5)**: Depends on Phase 2 — visible_nodes needs TreeState, TreeNode
- **US2 (Phase 6)**: Depends on Phase 5 (US4) — set_children verified via visible_nodes
- **US5 (Phase 7)**: Depends on Phase 5 (US4) — cursor operates on visible_nodes index
- **US6 (Phase 8)**: Depends on Phase 2 — SearchIndex is independent data structure
- **Polish (Phase 9)**: Depends on all user stories

### User Story Dependencies

```
Phase 2 (Foundational)
  ├── US1 (TreeBuilder)     ← independent
  ├── US3 (Sort)            ← independent
  ├── US4 (Visible Nodes)   ← independent
  │     ├── US2 (Lazy Load) ← depends on US4
  │     └── US5 (Cursor)    ← depends on US4
  └── US6 (SearchIndex)     ← independent
```

### Parallel Opportunities

- **Phase 2**: T010, T011, T012, T014 can run in parallel (different types, different files)
- **Phase 3**: All US1 tests (T017-T021) can run in parallel
- **Phase 4**: All US3 tests (T026-T030) can run in parallel
- **Phase 5**: All US4 tests (T034-T037) can run in parallel
- **After Phase 2**: US1, US3, US4, US6 can all be worked on in parallel
- **After Phase 5**: US2 and US5 can be worked on in parallel

---

## Implementation Strategy

### MVP First (US1 + US3 + US4)

1. Phase 1: Setup module structure
2. Phase 2: Define core data types
3. Phase 3: TreeBuilder (US1) — can build a tree from filesystem
4. Phase 4: Sort (US3) — trees are sorted correctly
5. Phase 5: Visible Nodes (US4) — flat list for UI rendering
6. **STOP and VALIDATE**: Build + test + lint all pass, tree can be built/sorted/flattened

### Incremental Delivery

1. Setup + Foundational → Types ready
2. US1 → TreeBuilder works
3. US3 → Sort works
4. US4 → visible_nodes works → **MVP complete**
5. US2 → Lazy loading (expand/collapse with async children)
6. US5 → Cursor navigation
7. US6 → SearchIndex for background scan
8. Polish → Integration tests, performance verification

---

## Summary

| Phase | Story | Tasks | Tests | Implementation |
|-------|-------|-------|-------|----------------|
| 1 | Setup | 7 | 0 | 7 |
| 2 | Foundational | 9 | 0 | 9 |
| 3 | US1 — ツリー構築 | 9 | 5 | 4 |
| 4 | US3 — ソート | 8 | 5 | 3 |
| 5 | US4 — Visible Nodes | 7 | 4 | 3 |
| 6 | US2 — 遅延読み込み | 8 | 4 | 4 |
| 7 | US5 — カーソル | 14 | 6 | 8 |
| 8 | US6 — 検索インデックス | 8 | 4 | 4 |
| 9 | Polish | 5 | 2 | 3 |
| **Total** | | **75** | **30** | **45** |

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- TDD is mandatory per Constitution Principle II
- Tests use `googletest` for assertions, `rstest` for parameterization, `tempfile` for filesystem tests
- All tests are inline `#[cfg(test)]` modules with `#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]`
- Verify tests fail before implementing (Red phase)
- Commit after each phase or logical group
