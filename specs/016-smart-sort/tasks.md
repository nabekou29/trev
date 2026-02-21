# Tasks: Smart Sort

**Input**: Design documents from `/specs/016-smart-sort/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, quickstart.md

**Tests**: TDD required (Constitution II). Tests written first, verified to fail, then implementation.

**Organization**: Tasks grouped by user story (US1=P1, US2=P2, US3=P3).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Foundational (SortOrder Enum Extension)

**Purpose**: Add `Smart` variant to both `SortOrder` enums — shared prerequisite for US1 and US3

- [x] T001 [P] Add `Smart` variant to `config::SortOrder` in `src/config.rs` (before `Name`, with `#[default]` attribute, serde value `"smart"`)
- [x] T002 [P] Add `Smart` variant to `state::tree::SortOrder` in `src/state/tree.rs` and update `From<config::SortOrder>` mapping
- [x] T003 Add temporary `SortOrder::Smart => SortOrder::Name` fallback in `sort_children` match in `src/tree/sort.rs` (prevents non-exhaustive match error; replaced in US1)

**Checkpoint**: Project compiles with `Smart` variant available. Existing tests pass unchanged.

---

## Phase 2: User Story 1 — Smart Sort as Default (Priority: P1) 🎯 MVP

**Goal**: Implement natural sort + suffix grouping algorithm. Files with numeric parts sort by value (`file2` < `file10`), test/story files group after their base file.

**Independent Test**: `cargo test sort` — all smart sort unit tests pass.

### Tests for User Story 1 ⚠️

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T004 [P] [US1] Write tests for `decompose_name` in `src/tree/sort.rs`: dot patterns (`user.test.ts` → `("user", Some(".test.ts"))`), underscore patterns (`handler_test.go` → `("handler", Some("_test.go"))`), no suffix (`main.rs` → `("main", None)`), 3-dot (`foo.bar.test.ts` → `("foo.bar", Some(".test.ts"))`), no extension (`Makefile` → `("Makefile", None)`), dotfiles (`.gitignore`), false positive (`test_utils.ts` should NOT decompose)
- [x] T005 [P] [US1] Write tests for `compare_natural` in `src/tree/sort.rs`: numeric ordering (`file2` < `file10`), case-insensitive (`api.ts` ≈ `Api.ts`), tie-break case-sensitive, pure numbers (`1` < `2` < `10`), long numbers (`file99999999999999999.txt`), text vs num ordering, empty/unicode strings
- [x] T006 [US1] Write tests for `SortOrder::Smart` in `sort_children` in `src/tree/sort.rs`: full integration — suffix grouping with natural sort, Go-style, mixed with dirs_first, acceptance scenarios from spec.md

### Implementation for User Story 1

- [x] T007 [US1] Implement `decompose_name(name: &str) -> (&str, Option<&str>)` in `src/tree/sort.rs` following research.md algorithm (rfind dot for ext, check `_test`/`_spec` in stem, check dot in stem, fallback no suffix)
- [x] T008 [US1] Implement `compare_natural(a: &str, b: &str) -> Ordering` in `src/tree/sort.rs` using chunk-based comparison (`Chunk::Text` / `Chunk::Num`); case-insensitive with case-sensitive tie-break; digit comparison by length then lexicographic (no integer parse)
- [x] T009 [US1] Replace `SortOrder::Smart` fallback in `sort_children` in `src/tree/sort.rs` with real implementation: decompose both names, compare bases with `compare_natural`, suffix-none first, suffix alphabetical, full-name tie-break

**Checkpoint**: `cargo test sort` — all smart sort tests pass. `SortOrder::Smart` produces correct ordering for all acceptance scenarios.

---

## Phase 3: User Story 2 — Sort Selection Menu (Priority: P2)

**Goal**: `S` key opens sort selection menu. Menu shows all sort orders, current one highlighted. Selection applies sort immediately. `s` key toggles sort direction.

**Independent Test**: Build and run trev, press `S` → menu appears, select sort → tree re-sorts.

### Tests for User Story 2 ⚠️

- [x] T010 [P] [US2] Write tests for `TreeAction::SortMenu` and `TreeAction::ToggleSortDirection` roundtrip (Display + FromStr) in `src/action.rs`
- [ ] T011 [P] [US2] Write tests for `MenuAction` dispatch in `handle_menu_mode_key` in `src/app/handler/input.rs`: `SelectSortOrder` path parses value and calls sort, `CopyToClipboard` path still calls clipboard

### Implementation for User Story 2

- [x] T012 [P] [US2] Add `TreeAction::SortMenu` and `TreeAction::ToggleSortDirection` to `src/action.rs` (enum variants, Display `"tree.sort_menu"` / `"tree.toggle_sort_direction"`, FromStr, add to `all_tree_actions_roundtrip` test, add to schema enum in `src/config.rs`)
- [x] T013 [US2] Add `MenuAction` enum (`CopyToClipboard`, `SelectSortOrder`) and `on_select: MenuAction` field to `MenuState` in `src/input.rs`
- [x] T014 [US2] Update existing `CopyMenu` construction site to include `on_select: MenuAction::CopyToClipboard` (find in `src/app/handler/tree.rs` or `src/app/handler/file_op.rs` where `AppMode::Menu(MenuState { ... })` is built)
- [x] T015 [US2] Refactor `handle_menu_mode_key` in `src/app/handler/input.rs`: on Enter/shortcut, match `menu.on_select` — `CopyToClipboard` calls existing `copy_to_clipboard`, `SelectSortOrder` calls new `apply_sort_from_menu` helper
- [x] T016 [US2] Implement `apply_sort_from_menu` in `src/app/handler/input.rs`: parse `MenuItem.value` as `SortOrder`, set `state.tree.options.sort_order`, call `state.tree.apply_sort()`, set status message
- [x] T017 [US2] Implement `SortMenu` handler in `src/app/handler/tree.rs`: build `Vec<MenuItem>` for each SortOrder variant (key=first letter, label=display name, value=config string), prepend `"● "` to label of current sort order (FR-005), set cursor to current sort index, open `AppMode::Menu` with `MenuAction::SelectSortOrder`
- [x] T018 [US2] Implement `ToggleSortDirection` handler in `src/app/handler/tree.rs`: flip `state.tree.options.sort_direction` between Asc/Desc, call `state.tree.apply_sort()`, set status message
- [x] T019 [US2] Add default keybindings in `src/app/keymap.rs`: `S` (Shift) → `TreeAction::SortMenu`, `s` → `TreeAction::ToggleSortDirection` (in `load_default_display` method)

**Checkpoint**: `S` key opens menu with 6 sort options, current sort marked with `●`. Selecting a different sort re-sorts the tree. `s` toggles direction. `Esc` cancels. Existing CopyMenu (`c` key) still works.

---

## Phase 4: User Story 3 — Default Sort Order Change (Priority: P3)

**Goal**: Default sort is `Smart` instead of `Name`. Config/CLI support `smart` value.

**Independent Test**: Start trev without config → files in smart sort order. Set `sort.order: name` in config → name order.

### Tests for User Story 3 ⚠️

- [x] T020 [US3] Write test in `src/config.rs`: default `SortOrder` is `Smart`, YAML `sort.order: smart` parses correctly, schema enum includes `"smart"`

### Implementation for User Story 3

- [x] T021 [US3] Move `#[default]` attribute from `SortOrder::Name` to `SortOrder::Smart` in `src/config.rs` (already added Smart variant in T001)
- [x] T022 [US3] Update existing tests in `src/config.rs` that assert `SortOrder::Name` as default (e.g., `empty_config_applies_defaults`, `unset_cli_args_preserve_config`)
- [x] T023 [US3] Verify `schema_sort_order_has_enum_values` test includes `"smart"` in `src/config.rs`

**Checkpoint**: `cargo test config` — all config tests pass with Smart as default. `trev` starts with smart sort. `--sort-order name` overrides to name sort.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Performance validation and final verification

- [x] T024 [P] Write performance test (marked `#[ignore]`) for smart sort with 100,000 file nodes in `src/tree/sort.rs` — must complete within 1 second (SC-001)
- [x] T025 Run `mise run build && mise run lint && mise run test` — all pass
- [ ] T026 Run quickstart.md validation scenarios (natural sort, suffix grouping, Go style, 3-dot, menu, default change, edge cases)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Foundational (Phase 1)**: No dependencies — start immediately
- **US1 (Phase 2)**: Depends on Phase 1 (Smart variant must exist)
- **US2 (Phase 3)**: Depends on Phase 1 (Smart variant in sort menu items). Can start in parallel with US1 for non-sort tasks (T012-T014)
- **US3 (Phase 4)**: Depends on Phase 1 (Smart variant) and Phase 2 (sort logic must work)
- **Polish (Phase 5)**: Depends on all user stories being complete

### User Story Dependencies

- **US1 (P1)**: Can start after Phase 1 — No dependencies on other stories
- **US2 (P2)**: Can start after Phase 1 — Independent of US1 (menu works with any sort order)
- **US3 (P3)**: Depends on US1 completion (Smart sort logic must exist before setting it as default)

### Within Each User Story

- Tests MUST be written and FAIL before implementation (Constitution II: TDD)
- Implementation tasks are sequential within each story (later tasks build on earlier ones)
- Each story ends with a checkpoint validation

### Parallel Opportunities

- T001 + T002 can run in parallel (different files)
- T004 + T005 can run in parallel (different test groups in same file, but truly independent)
- T010 + T011 can run in parallel (different test files)
- T012 can run in parallel with T010/T011 (different files)
- US1 and US2 can largely proceed in parallel after Phase 1

---

## Parallel Example: User Story 1

```bash
# Write tests in parallel (independent test groups):
Task: T004 "Tests for decompose_name in src/tree/sort.rs"
Task: T005 "Tests for compare_natural in src/tree/sort.rs"

# Then sequential implementation:
Task: T007 "Implement decompose_name"
Task: T008 "Implement compare_natural"
Task: T009 "Replace Smart fallback with real implementation"
```

## Parallel Example: User Story 2

```bash
# Write tests in parallel (different files):
Task: T010 "Tests for TreeAction roundtrip in src/action.rs"
Task: T011 "Tests for MenuAction dispatch in src/app/handler/input.rs"

# Implementation T012 can also start in parallel:
Task: T012 "Add TreeAction variants to src/action.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Foundational (SortOrder enum)
2. Complete Phase 2: US1 — Smart Sort Algorithm
3. **STOP and VALIDATE**: `cargo test sort` passes, smart sort ordering is correct
4. Smart sort is usable (though not yet selectable via menu or set as default)

### Incremental Delivery

1. Phase 1: SortOrder enum extension → Foundation ready
2. US1: Smart sort algorithm → Core functionality works (MVP!)
3. US2: Sort selection menu → Users can switch sort orders interactively
4. US3: Default change → Smart sort is the out-of-box experience
5. Polish: Performance validation + full test suite

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- TDD is mandatory (Constitution II) — all test tasks must be completed and verified RED before implementation
- Each user story is independently testable at its checkpoint
- `src/tree/sort.rs` is the primary file for US1 (contains both implementation and tests)
- `src/app/handler/input.rs` is the key file for US2 menu generalization
- Existing CopyMenu must continue to work after US2 changes (backward compatibility)
