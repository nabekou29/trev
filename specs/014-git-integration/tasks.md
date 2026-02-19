# Tasks: Git Integration

**Input**: Design documents from `/specs/014-git-integration/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, quickstart.md

**Tests**: TDD required (constitution gate II). Tests are written FIRST and must FAIL before implementation.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: Create new module and basic structure

- [x] T001 Create `src/git.rs` module file with module-level doc comment and add `mod git;` to `src/main.rs`

---

## Phase 2: Foundational (Data Model & Parser)

**Purpose**: Core git data types and porcelain parser that ALL user stories depend on

**CRITICAL**: No user story work can begin until this phase is complete

### Tests for Foundational

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T002 [P] Write tests for `GitFileStatus` enum (display char, color, priority ordering) in `src/git.rs` inline tests
- [x] T003 [P] Write tests for `GitState::from_porcelain()` parser (modified, added, deleted, renamed, untracked, conflicted, staged, staged+modified cases) in `src/git.rs` inline tests
- [x] T004 [P] Write tests for `GitState::file_status()` (exact path lookup, missing path returns None) in `src/git.rs` inline tests
- [x] T005 [P] Write tests for `GitState::dir_status()` (single child, multiple children with priority aggregation, nested directories, empty directory) in `src/git.rs` inline tests
- [x] T006 [P] Write test for `from_porcelain()` rename handling (`R  old -> new` format, status on new path) in `src/git.rs` inline tests

### Implementation for Foundational

- [x] T007 Implement `GitFileStatus` enum with variants (Modified, Staged, StagedModified, Added, Deleted, Renamed, Untracked, Conflicted) and display/color methods in `src/git.rs`
- [x] T008 Implement `GitState` struct with `HashMap<PathBuf, GitFileStatus>` and `file_status()` method in `src/git.rs`
- [x] T009 Implement `GitState::from_porcelain(output: &str, repo_root: &Path) -> Self` parser in `src/git.rs`
- [x] T010 Implement `GitState::dir_status(dir_path: &Path) -> Option<GitFileStatus>` with priority-based aggregation in `src/git.rs`

**Checkpoint**: `cargo test git::` passes. Data model and parser are complete and tested.

---

## Phase 3: User Story 1 - File Git Status Display (Priority: P1) MVP

**Goal**: Display colored 1-character git status indicators on the right side of filenames in the tree view, in a metadata area independent of selection markers

**Independent Test**: Start trev in a git repo with modified/staged/untracked files and verify indicators (M/A/?) appear to the right of filenames with correct colors

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**

- [x] T011 [P] [US1] Write test for `GitConfig` default values (enabled: true) and deserialization in `src/config.rs` inline tests
- [x] T012 [P] [US1] Write test for `git_status_indicator()` function (each status maps to correct char and color, None returns empty Span) in `src/ui/tree_view.rs` inline tests
- [x] T013 [P] [US1] Write test for `GitStatusResult` struct construction in `src/git.rs` inline tests

### Implementation for User Story 1

- [x] T014 [P] [US1] Add `GitConfig { enabled: bool }` to `Config` struct with `#[serde(default)]` in `src/config.rs`
- [x] T015 [P] [US1] Add `GitStatusResult { state: Option<GitState> }` struct in `src/git.rs`
- [x] T016 [US1] Add `git_state: Option<GitState>` field to `AppState` in `src/app/state.rs`
- [x] T017 [US1] Add `git_tx: Sender<GitStatusResult>` to `AppContext` and create `git_rx` channel in `src/app/state.rs`
- [x] T018 [US1] Implement `trigger_git_status()` function (spawn_blocking + `git status --porcelain=v1` + from_porcelain + send via git_tx) in `src/app.rs`
- [x] T019 [US1] Add `git_rx` processing to event loop (update `AppState.git_state` on receive) in `src/app.rs`
- [x] T020 [US1] Call `trigger_git_status()` on app startup when `git.enabled == true` in `src/app.rs`
- [x] T021 [US1] Implement `git_status_indicator()` function returning styled Span (char + Color) in `src/ui/tree_view.rs`
- [x] T022 [US1] Add git status indicator rendering to the right side of filename in tree_view row layout in `src/ui/tree_view.rs`

**Checkpoint**: Files show colored M/A/?/R/D/! indicators to the right of filenames. Indicators are visible simultaneously with selection markers.

---

## Phase 4: User Story 2 - Directory Status Aggregation (Priority: P1)

**Goal**: Directories display the highest-priority git status from their descendants, regardless of expand/collapse state

**Independent Test**: Collapse a directory containing modified files and verify yellow `M` appears on the directory line

**Depends on**: US1 (git status display infrastructure)

### Tests for User Story 2

- [x] T023 [US2] Write test for directory aggregation display (directory shows max-priority child status) in `src/ui/tree_view.rs` inline tests

### Implementation for User Story 2

- [x] T024 [US2] Modify tree_view rendering to call `dir_status()` for directory nodes and display aggregated indicator on right side in `src/ui/tree_view.rs`

**Checkpoint**: Directories show aggregated git status to the right of directory name. Collapsed `src/` with a modified child shows yellow `M`.

---

## Phase 5: User Story 3 - Auto-update on FS Changes (Priority: P1)

**Goal**: Git status automatically refreshes when file system changes are detected (via existing watcher)

**Independent Test**: While trev is running, run `git add file.rs` in another terminal and verify the indicator updates

**Depends on**: US1 (git status fetch infrastructure)

### Implementation for User Story 3

- [x] T025 [US3] Add `trigger_git_status()` call inside `process_watcher_events()` when git is enabled in `src/app.rs`

**Checkpoint**: Editing a file or running `git add` in another terminal triggers automatic git status refresh.

---

## Phase 6: User Story 4 - Manual Refresh with R Key (Priority: P1)

**Goal**: `R` key refreshes both tree structure and git status

**Independent Test**: Press `R` key and verify both tree and git status are reloaded

### Tests for User Story 4

- [x] T026 [P] [US4] Write test for `TreeAction::Refresh` variant (FromStr, Display, JSON Schema) in `src/action.rs` inline tests

### Implementation for User Story 4

- [x] T027 [P] [US4] Add `Refresh` variant to `TreeAction` enum with `FromStr`/`Display` implementations in `src/action.rs`
- [x] T028 [US4] Add `R` -> `tree.refresh` default keybinding in `load_default_display()` in `src/app/keymap.rs`
- [x] T029 [US4] Implement `TreeAction::Refresh` handler: save cursor path, rebuild tree from root, trigger git status re-fetch, restore cursor position in `src/app/handler.rs`

**Checkpoint**: Pressing `R` reloads tree structure and git status. Cursor position is preserved.

---

## Phase 7: User Story 5 - Custom Preview git_status Condition (Priority: P2)

**Goal**: External preview commands can be filtered by git status using `git_status` field in config

**Independent Test**: Configure `git_status: [modified]` on a preview command and verify it only applies to modified files

**Depends on**: US1 (git state available in AppState)

### Tests for User Story 5

- [x] T030 [P] [US5] Write test for `ExternalCommand` deserialization with `git_status` field in `src/config.rs` inline tests
- [x] T031 [P] [US5] Write test for `ExternalCmdProvider::can_handle` with git_status condition (match, no match, no condition) in `src/preview/providers/external.rs` inline tests

### Implementation for User Story 5

- [x] T032 [P] [US5] Add `git_status: Vec<String>` field with `#[serde(default)]` to `ExternalCommand` in `src/config.rs`
- [x] T033 [US5] Add `Arc<RwLock<Option<GitState>>>` to `ExternalCmdProvider` struct in `src/preview/providers/external.rs`
- [x] T034 [US5] Update `ExternalCmdProvider::can_handle` to check `git_status` condition against current git state in `src/preview/providers/external.rs`
- [x] T035 [US5] Wire `Arc<RwLock<Option<GitState>>>` from AppState to ExternalCmdProvider during provider construction in `src/app.rs`

**Checkpoint**: Preview commands with `git_status: [modified]` only trigger for modified files.

---

## Phase 8: User Story 6 - Git Config Setting (Priority: P2)

**Goal**: `git.enabled` config and `--no-git` CLI flag to disable git integration entirely

**Independent Test**: Set `git.enabled: false` in config and verify no git indicators appear

### Tests for User Story 6

- [x] T036 [P] [US6] Write test for `--no-git` CLI flag parsing in `src/cli.rs` inline tests
- [x] T037 [P] [US6] Write test for `apply_cli_overrides` with `--no-git` flag in `src/config.rs` inline tests

### Implementation for User Story 6

- [x] T038 [P] [US6] Add `--no-git` flag to CLI args struct in `src/cli.rs`
- [x] T039 [US6] Update `apply_cli_overrides` to set `git.enabled = false` when `--no-git` is passed in `src/config.rs`
- [x] T040 [US6] Ensure `trigger_git_status()` and watcher git re-fetch are skipped when `git.enabled == false` in `src/app.rs`

**Checkpoint**: `--no-git` flag and `git.enabled: false` both completely disable git integration.

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: Performance validation and final checks

- [x] T041 [P] Write performance test for `from_porcelain()` with 10,000 file entries (must complete in 500ms) in `src/git.rs` inline tests with `#[ignore]`
- [x] T042 [P] Write performance test for `dir_status()` with large HashMap (10,000 entries) in `src/git.rs` inline tests with `#[ignore]`
- [x] T043 Run `mise run lint` and fix any clippy warnings
- [x] T044 Run `mise run test` and verify all tests pass
- [ ] T045 Run quickstart.md validation (manual: `cargo run -- .` in git repo, verify indicators)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 - BLOCKS all user stories
- **US1 (Phase 3)**: Depends on Phase 2 - BLOCKS US2, US3, US4, US5
- **US2 (Phase 4)**: Depends on US1 (display infrastructure)
- **US3 (Phase 5)**: Depends on US1 (async fetch infrastructure)
- **US4 (Phase 6)**: Depends on US1 (git state in AppState)
- **US5 (Phase 7)**: Depends on US1 (git state available)
- **US6 (Phase 8)**: Can start after Phase 2 (config only), but integration touches US1 code
- **Polish (Phase 9)**: Depends on all user stories being complete

### User Story Dependencies

```
Phase 1: Setup
    │
    v
Phase 2: Foundational (Data Model & Parser)
    │
    v
Phase 3: US1 - File Git Status Display ──── MVP
    │
    ├──> Phase 4: US2 - Directory Aggregation
    ├──> Phase 5: US3 - Auto-update
    ├──> Phase 6: US4 - Manual Refresh (R key)
    └──> Phase 7: US5 - Custom Preview Condition
                                              │
Phase 8: US6 - Git Config ───────────────────┘ (can partially parallel)
    │
    v
Phase 9: Polish
```

### Within Each User Story

- Tests MUST be written and FAIL before implementation
- Data model before services/logic
- Core logic before UI integration
- Story complete before moving to next priority

### Parallel Opportunities

- **Phase 2**: T002-T006 (all foundational tests) can run in parallel
- **Phase 3**: T011-T013 (US1 tests) can run in parallel; T014-T015 can run in parallel
- **Phase 4-6**: US2, US3, US4 can start in parallel after US1 completes
- **Phase 7**: T030-T032 can run in parallel
- **Phase 8**: T036-T038 can run in parallel

---

## Parallel Example: Foundational Phase

```bash
# Launch all foundational tests together:
Task: "T002 - Write tests for GitFileStatus enum in src/git.rs"
Task: "T003 - Write tests for from_porcelain() parser in src/git.rs"
Task: "T004 - Write tests for file_status() in src/git.rs"
Task: "T005 - Write tests for dir_status() aggregation in src/git.rs"
Task: "T006 - Write tests for rename handling in src/git.rs"
```

## Parallel Example: After US1 Completes

```bash
# Launch US2, US3, US4 in parallel:
Task: "T023-T024 - US2: Directory aggregation display"
Task: "T025 - US3: Watcher integration for auto-update"
Task: "T026-T029 - US4: TreeAction::Refresh and R keybinding"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001)
2. Complete Phase 2: Foundational (T002-T010)
3. Complete Phase 3: User Story 1 (T011-T022)
4. **STOP and VALIDATE**: Files show git status indicators with correct colors
5. Commit and verify with `cargo run -- .` in a git repo

### Incremental Delivery

1. Setup + Foundational -> Data model and parser tested
2. Add US1 -> File git status display working (MVP!)
3. Add US2 -> Directory aggregation working
4. Add US3 + US4 -> Auto-update and manual refresh working
5. Add US5 + US6 -> Custom preview conditions and config toggle
6. Polish -> Performance validated, all tests green

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- TDD required: all tests must be written and FAIL before implementation
- `tempfile` + `git init` for test environments requiring a git repo
- Performance tests use `#[ignore]` attribute, run with `cargo test -- --ignored`
- Strict clippy (all, pedantic, nursery, cargo at deny level) must pass
- `pub` not `pub(crate)` in private modules (clippy::redundant_pub_crate)
- Simple getters/constructors must be `const fn` (clippy::missing_const_for_fn)
