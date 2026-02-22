# Tasks: 設定体系の整備

**Input**: Design documents from `/specs/017-config-refinement/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/config-schema.md

**Tests**: TDD required per constitution (Principle II). Tests are written before implementation.

**Organization**: Tasks grouped by user story. Implementation order: Foundational → US3 (naming) → US1 (direct keybinds) → US2 (user-defined menus) → Polish

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: No project setup needed — existing Rust binary project

_(No tasks — project structure already exists)_

---

## Phase 2: Foundational (Action Hierarchy Refactoring)

**Purpose**: Restructure Action enum with sub-enums and add `all_action_names()` helper. This structural change is prerequisite for all user stories.

**⚠️ CRITICAL**: US1/US2/US3 cannot begin until this phase is complete.

### Tests

- [x] T001 [P] Write tests for `SortAction` `FromStr`/`Display` round-trip (`tree.sort.menu`, `tree.sort.toggle_direction`) in `src/action.rs` `#[cfg(test)]` module
- [x] T002 [P] Write tests for `CopyAction` `FromStr`/`Display` round-trip (`file_op.copy.menu`) in `src/action.rs` `#[cfg(test)]` module
- [x] T003 [P] Write tests for `FilterAction` `FromStr`/`Display` round-trip (`filter.hidden`, `filter.ignored`) in `src/action.rs` `#[cfg(test)]` module
- [x] T004 Write test for `Action::all_action_names()` returning complete list including sub-enum variants in `src/action.rs` `#[cfg(test)]` module

### Implementation

- [x] T005 Create `SortAction` sub-enum in `src/action.rs` with variants `Menu`, `ToggleDirection`. Move from `TreeAction::SortMenu`/`ToggleSortDirection`. Implement `FromStr`/`Display` with `tree.sort.*` prefix
- [x] T006 Create `CopyAction` sub-enum in `src/action.rs` with variant `Menu`. Move from `FileOpAction::CopyMenu`. Implement `FromStr`/`Display` with `file_op.copy.*` prefix
- [x] T007 Create `FilterAction` top-level enum in `src/action.rs` with variants `Hidden`, `Ignored`. Add `Action::Filter(FilterAction)` variant. Implement `FromStr`/`Display` with `filter.*` prefix
- [x] T008 Add `TreeAction::Sort(SortAction)` variant to `TreeAction`, remove old `SortMenu`/`ToggleSortDirection` variants in `src/action.rs`
- [x] T009 Add `FileOpAction::Copy(CopyAction)` variant to `FileOpAction`, remove old `CopyMenu` variant in `src/action.rs`
- [x] T010 Remove `ToggleHidden`/`ToggleIgnored` from `TreeAction` in `src/action.rs` (moved to `FilterAction`)
- [x] T011 Implement `all_action_names() -> Vec<&'static str>` on `Action` aggregating all sub-enum variants in `src/action.rs`
- [x] T012 Update `src/app/keymap.rs` default bindings: replace `TreeAction::SortMenu` → `TreeAction::Sort(SortAction::Menu)`, `TreeAction::ToggleSortDirection` → `TreeAction::Sort(SortAction::ToggleDirection)`, `FileOpAction::CopyMenu` → `FileOpAction::Copy(CopyAction::Menu)`, `TreeAction::ToggleHidden` → `Action::Filter(FilterAction::Hidden)`, `TreeAction::ToggleIgnored` → `Action::Filter(FilterAction::Ignored)`
- [x] T013 Update `src/app/handler/tree.rs` to dispatch `TreeAction::Sort(action)` matching on `SortAction` variants
- [x] T014 Update `src/app/handler/file_op.rs` to dispatch `FileOpAction::Copy(action)` matching on `CopyAction` variants
- [x] T015 Add `FilterAction` dispatch in `src/app.rs` (add `Action::Filter(action)` match arm) to toggle hidden/ignored state
- [x] T016 Update `KeyBindingEntry::json_schema` in `src/config.rs` to use `Action::all_action_names()` instead of hardcoded list

**Checkpoint**: All existing functionality works with new action hierarchy. `mise run build && mise run test && mise run lint` passes.

---

## Phase 3: User Story 3 — わかりやすい設定項目名 (Priority: P3, implemented first per plan)

**Goal**: Config field names and action names are renamed to be consistent and intuitive. Breaking change — no backward compatibility needed.

**Independent Test**: Renamed config YAML is loaded correctly and all settings apply.

### Tests

- [x] T017 [P] [US3] Write test for `Config` deserialization with new field names (`file_op`, `split_ratio`, `narrow_split_ratio`, `narrow_width`, `show_root_entry`) in `src/config.rs` `#[cfg(test)]` module
- [x] T018 [P] [US3] Write test that old field names (`file_operations`, `split`, `narrow_split`, `narrow_threshold`, `show_root`) are rejected or ignored in `src/config.rs` `#[cfg(test)]` module

### Implementation

- [x] T019 [US3] Rename `file_operations` → `file_op` in `Config` struct and serde attribute in `src/config.rs`
- [x] T020 [US3] Rename `split` → `split_ratio`, `narrow_split` → `narrow_split_ratio`, `narrow_threshold` → `narrow_width` in `PreviewConfig` struct in `src/config.rs`
- [x] T021 [US3] Rename `show_root` → `show_root_entry` in `DisplayConfig` struct in `src/config.rs`
- [x] T022 [US3] Update all references to renamed fields across codebase: `src/app.rs`, `src/app/state.rs`, `src/ui/preview_view.rs`, `src/ui/tree_view.rs` and other files referencing `config.file_operations`, `config.preview.split`, `config.display.show_root`
- [x] T023 [US3] Update CLI argument names and `apply_cli_overrides` in `src/cli.rs` to match renamed config fields
- [x] T024 [US3] Update existing tests referencing old field names across the codebase

**Checkpoint**: Config with new names loads correctly. All tests pass. Old names are not accepted.

---

## Phase 4: User Story 1 — メニュー操作をショートカットキーで直接実行する (Priority: P1) 🎯 MVP

**Goal**: All 11 menu actions (6 sort + 5 copy) are individually bindable to keys without opening a menu.

**Independent Test**: Configure `tree.sort.by_name` as a keybinding → pressing the key changes sort order directly.

### Tests

- [x] T025 [P] [US1] Write tests for `SortAction::ByName/BySize/ByMtime/ByType/ByExtension/BySmart` `FromStr`/`Display` round-trip in `src/action.rs` `#[cfg(test)]` module
- [x] T026 [P] [US1] Write tests for `CopyAction::AbsolutePath/RelativePath/FileName/Stem/ParentDir` `FromStr`/`Display` round-trip in `src/action.rs` `#[cfg(test)]` module
- [x] T027 [P] [US1] Write test for direct sort action handler applying sort order without opening menu in `src/app/handler/tree.rs` `#[cfg(test)]` module
- [x] T028 [P] [US1] Write test for direct copy action handler copying to clipboard without opening menu in `src/app/handler/file_op.rs` `#[cfg(test)]` module
- [x] T029 [US1] Write test for keybinding config with direct sort/copy actions being correctly parsed and resolved in `src/app/keymap.rs` `#[cfg(test)]` module

### Implementation

- [x] T030 [US1] Add `SortAction::ByName`, `BySize`, `ByMtime`, `ByType`, `ByExtension`, `BySmart` variants with `FromStr`/`Display` in `src/action.rs`
- [x] T031 [US1] Add `CopyAction::AbsolutePath`, `RelativePath`, `FileName`, `Stem`, `ParentDir` variants with `FromStr`/`Display` in `src/action.rs`
- [x] T032 [US1] Update `all_action_names()` to include all new sort and copy action variants in `src/action.rs`
- [x] T033 [US1] Implement direct sort handlers in `src/app/handler/tree.rs`: for each `SortAction::By*`, apply `tree_state.apply_sort(order, direction, dirs_first)` directly
- [x] T034 [US1] Implement direct copy handlers in `src/app/handler/file_op.rs`: for each `CopyAction::*`, compute the target string and copy to clipboard via `arboard`
- [x] T035 [US1] Verify existing sort menu and copy menu continue to work (regression test via existing menu flow)

**Checkpoint**: All 11 individual actions (6 sort + 5 copy) bindable via config. Menu-based flow still works. `mise run test` passes.

---

## Phase 5: User Story 2 — メニューを自由に定義する (Priority: P2)

**Goal**: Users can define custom menus in config YAML and bind them to keys. Menu items support action/run/notify.

**Independent Test**: Define a custom menu in config, bind to key `m`, press `m` → menu appears with custom items.

### Tests

- [x] T036 [P] [US2] Write test for `MenuDefinition` and `MenuItemDef` deserialization from YAML in `src/config.rs` `#[cfg(test)]` module
- [x] T037 [P] [US2] Write test for `KeyBindingEntry` with `menu` field deserialization and validation (exactly one of action/run/notify/menu) in `src/config.rs` `#[cfg(test)]` module
- [x] T038 [P] [US2] Write test for `Action::OpenMenu(name)` `FromStr`/`Display` round-trip (`menu:<name>`) in `src/action.rs` `#[cfg(test)]` module
- [x] T039 [US2] Write test for `OpenMenu` handler building `MenuState` from config `MenuDefinition` in `src/app/handler.rs` `#[cfg(test)]` module. Verify menu items appear in config definition order (FR-006)
- [x] T040 [US2] Write test for `MenuAction::Custom` dispatch: resolve_menu_item_action tests for action/run/notify/invalid/empty in `src/app/handler.rs` `#[cfg(test)]` module

### Implementation

- [x] T041 [P] [US2] Add `MenuDefinition` struct (title, items) and `MenuItemDef` struct (key, label, action/run/notify) with serde + schemars derives in `src/config.rs`
- [x] T042 [P] [US2] Add `menus: HashMap<String, MenuDefinition>` field to `Config` struct in `src/config.rs`
- [x] T043 [US2] Add `menu: Option<String>` field to `KeyBindingEntry` in `src/config.rs`. Update validation to enforce exactly one of action/run/notify/menu
- [x] T044 [US2] Update `KeyBindingEntry::json_schema` to include `menu` field in `src/config.rs`
- [x] T045 [US2] Add `Action::OpenMenu(String)` variant in `src/action.rs`. Implement `FromStr` for `menu:<name>` prefix and `Display`
- [x] T046 [US2] Add `MenuAction::Custom` variant to `MenuAction` enum in `src/input.rs`
- [x] T047 [US2] Implement `menu` field resolution in `resolve_entry_action` in `src/app/keymap.rs`: convert `menu: "name"` to `Action::OpenMenu(name)`
- [x] T048 [US2] Implement `Action::OpenMenu(name)` handler: look up `MenuDefinition` from config, build `MenuState` with `MenuAction::Custom`, set `AppMode::Menu` in `src/app/handler.rs`
- [x] T049 [US2] Implement `MenuAction::Custom` dispatch in `src/app/handler/input.rs`: resolve menu item's action/run/notify to `Action`, execute it
- [x] T050 [US2] Handle edge cases: empty menu (no-op or immediate close), unknown menu name (log warning, no-op), duplicate shortcut keys (first match wins), invalid action name in menu item (log warning, skip item) in `src/app/handler.rs`

**Checkpoint**: Custom menus definable in config, bindable to keys, all 3 action types (action/run/notify) work in menu items. Built-in menus unaffected.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, schema validation, and final regression testing

- [x] T051 [P] Update keybindings table in `README.md` with new action names (`filter.hidden`, `filter.ignored`, `tree.sort.*`, `file_op.copy.*`)
- [x] T052 [P] Update config documentation section in `README.md` with renamed fields (`file_op`, `split_ratio`, `narrow_split_ratio`, `narrow_width`, `show_root_entry`)
- [x] T053 [P] Add user-defined menus documentation and examples to `README.md`
- [x] T054 Regenerate JSON Schema via `trev schema` and verify it reflects all changes (new actions, renamed fields, menus section)
- [x] T055 Run full test suite (`mise run build && mise run test && mise run lint`) and fix any remaining issues

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No tasks needed
- **Phase 2 (Foundational)**: No dependencies — start immediately. BLOCKS all user stories.
- **Phase 3 (US3 - Naming)**: Depends on Phase 2 completion
- **Phase 4 (US1 - Direct Keybinds)**: Depends on Phase 3 completion (names must be finalized)
- **Phase 5 (US2 - User Menus)**: Depends on Phase 4 completion (action system must be complete)
- **Phase 6 (Polish)**: Depends on Phase 5 completion

### User Story Dependencies

- **US3 (P3 - Naming)**: Must be first — renaming after adding features causes conflicts
- **US1 (P1 - Direct Keybinds)**: Depends on US3 — builds on renamed action hierarchy
- **US2 (P2 - User Menus)**: Depends on US1 — uses the same action system for menu items

### Within Each Phase

- Tests MUST be written and FAIL before implementation (TDD)
- Tasks marked [P] can run in parallel (different files)
- Sequential tasks depend on prior tasks in the same phase

### Parallel Opportunities

**Phase 2**: T001, T002, T003 can run in parallel (test different enums)
**Phase 3**: T017, T018 in parallel (test different scenarios); T019, T020, T021 sequential (same file `src/config.rs`)
**Phase 4**: T025, T026 in parallel (sort vs copy tests); T027, T028 in parallel; T030, T031 sequential (same file `src/action.rs`)
**Phase 5**: T036, T037, T038 in parallel (different test targets); T041, T042 in parallel (different structs)
**Phase 6**: T051, T052, T053 in parallel (different README sections)

---

## Parallel Example: Phase 4 (US1)

```text
# Tests (parallel):
T025: SortAction variant FromStr/Display tests  |  T026: CopyAction variant FromStr/Display tests
T027: Direct sort handler tests                  |  T028: Direct copy handler tests

# Implementation (sequential — same file src/action.rs):
T030: Add SortAction variants
T031: Add CopyAction variants

# Sequential:
T032: Update all_action_names() (depends on T030, T031)
T033: Sort handlers (depends on T030)  |  T034: Copy handlers (depends on T031)
T035: Regression verification
```

---

## Implementation Strategy

### MVP First (Phase 2 + Phase 3 + Phase 4)

1. Complete Phase 2: Foundational action hierarchy
2. Complete Phase 3: Config naming (US3)
3. Complete Phase 4: Direct keybinds (US1) ← **MVP: users can bind sort/copy actions directly**
4. **STOP and VALIDATE**: 11 individual actions work via keybindings
5. Ship if ready — user-defined menus can come later

### Incremental Delivery

1. Phase 2 → Action hierarchy works with existing functionality
2. Phase 3 (US3) → Config names are clean and consistent
3. Phase 4 (US1) → Direct keybinds for power users (MVP!)
4. Phase 5 (US2) → Custom menus for advanced customization
5. Phase 6 → Documentation and polish

---

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks
- [US*] label maps task to specific user story
- TDD mandatory: write tests first, verify they fail, then implement
- Commit after each task or logical group
- Run `mise run build && mise run test && mise run lint` at each checkpoint
- All action string names follow `contracts/config-schema.md` action registry
