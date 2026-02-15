# Tasks: File Operations + FS Change Detection + Undo/Redo + Session Persistence

**Input**: Design documents from `/specs/012-file-operations/`
**Prerequisites**: plan.md, spec.md, data-model.md, research.md, quickstart.md
**Methodology**: TDD (Red-Green-Refactor) — 各実装タスクでテストを先に書く

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: Dependencies and module skeleton

- [x] T001 Add new dependencies (notify 8, notify-debouncer-mini 0.5, trash 5, chrono 0.4 with serde feature, sha2 0.10) to Cargo.toml
- [x] T002 Create module skeleton files with doc comments: src/file_op.rs (pub mod re-exports), src/file_op/executor.rs, src/file_op/undo.rs, src/file_op/yank.rs, src/file_op/mark.rs, src/file_op/conflict.rs, src/file_op/trash.rs, src/session.rs, src/watcher.rs, src/ui/inline_input.rs; register modules in src/main.rs and src/ui.rs

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared types, config, action variants, and core FS execution that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T003 [P] Add FileOpConfig (delete_mode: DeleteMode, undo_stack_size: usize), SessionConfig (restore_by_default: bool, expiry_days: u64), WatcherConfig (enabled: bool, debounce_ms: u64) with defaults and serde to src/config.rs. Add DeleteMode enum (Permanent, CustomTrash) with serde
- [x] T004 [P] Add --restore and --no-restore mutually exclusive CLI options (both optional bool flags) to src/cli.rs
- [x] T005 [P] Add FileOpAction enum variants (Yank, Cut, Paste, CreateFile, Rename, Delete, SystemTrash, Undo, Redo, ToggleMark) and integrate as Action::FileOp(FileOpAction) in src/action.rs
- [x] T006 [P] Implement MarkSet struct (HashSet<PathBuf>) with toggle, clear, is_marked, count, targets_or_cursor methods in src/file_op/mark.rs. All simple methods must be const fn where possible
- [x] T007 [P] Implement FsOp enum (Copy, Move, CreateFile, CreateDir, RemoveFile, RemoveDir with PathBuf fields) and IrreversibleOp enum (SystemTrash, PermanentDelete) with serde derive in src/file_op/executor.rs
- [x] T008 [P] Implement auto-rename conflict resolution (resolve_conflict: given dst path, if exists return path with _N suffix before extension, scanning for first available N) in src/file_op/conflict.rs
- [x] T009 [P] Implement custom trash directory management: trash_dir() returns platform data dir + "trev/trash/", trash_path() generates timestamped filename ({nanos}_{name}), ensure_trash_dir() creates dir, clean_trash_file() removes specific file in src/file_op/trash.rs
- [x] T010 Implement FsOp executor: execute(op: &FsOp) → Result<()> dispatching to copy_file (with symlink-aware copy), copy_dir_recursive (using walkdir pattern), move_path (rename with cross-fs fallback), create_file, create_dir (with create_dir_all for nested), remove_file, remove_dir_all in src/file_op/executor.rs
- [x] T011 [P] Implement self-reference detection: is_ancestor(parent: &Path, child: &Path) → bool checking if a directory would be copied/moved into itself, in src/file_op/executor.rs
- [x] T012 Implement AppMode enum (Normal, Input(InputState), Confirm(ConfirmState)) in src/app.rs. Define InputState (prompt, value, cursor_pos, on_confirm: InputAction) in src/input.rs and ConfirmState (message, paths, on_confirm: ConfirmAction) — InputAction variants: Create { parent_dir }, Rename { target }; ConfirmAction variants: PermanentDelete, CustomTrash, SystemTrash

**Checkpoint**: Foundation ready — user story implementation can begin

---

## Phase 3: User Story 7 — マーク（複数選択） (Priority: P1) 🎯 MVP

**Goal**: Space でファイル/ディレクトリをマーク/解除し、視覚的に区別する。マーク済みファイルがファイル操作の対象となる

**Independent Test**: Space でマーク → 視覚表示確認 → yank でマーク済みが対象になる

**Dependencies**: Foundational (Phase 2)

**FR**: FR-016, FR-017, FR-018, FR-019, FR-020

- [x] T013 [US7] Integrate MarkSet into AppState (add mark_set: MarkSet field) and expose toggle/clear/is_marked/targets_or_cursor delegating to MarkSet in src/app.rs
- [x] T014 [US7] Add Space keybinding: toggle mark on cursor path, then move cursor down one position. Handle in AppMode::Normal only, in src/app.rs
- [x] T015 [US7] Add mark visual indicator: render marker symbol (e.g. "●" or "*") before filename for marked nodes in src/ui/tree_view.rs
- [x] T016 [US7] Implement mark auto-clear: after yank/delete operations, call mark_set.clear() in src/app.rs (prepare hook points for US1/US3 integration)

**Checkpoint**: Mark system functional — Space toggles marks, visual feedback shown, targets_or_cursor resolves correctly

---

## Phase 4: User Story 2 — 新規作成・リネーム (Priority: P1)

**Goal**: `a` でファイル/ディレクトリ新規作成、`r` でリネーム。インライン入力フィールドで操作

**Independent Test**: `a` → ファイル名入力 → Enter → ファイル作成確認。`r` → 名前変更 → Enter → リネーム確認

**Dependencies**: Foundational (Phase 2)

**FR**: FR-007, FR-008, FR-009, FR-010, FR-011, FR-038

- [x] T017 [US2] Implement InputState text editing methods: insert_char, delete_char_backward (backspace), delete_char_forward (delete), move_cursor_left, move_cursor_right, move_cursor_home, move_cursor_end, clear_to_start (ctrl+u), handle_key_event dispatcher in src/input.rs
- [x] T018 [P] [US2] Implement inline input widget: render single-line input field with prompt text, current value, and visible cursor at cursor_pos position in src/ui/inline_input.rs
- [x] T019 [US2] Implement file creation flow: determine parent_dir (if cursor on dir use it, if on file use parent), detect trailing "/" for dir creation, support nested paths with create_dir_all for intermediate dirs, execute FsOp::CreateFile or FsOp::CreateDir in src/app.rs
- [x] T020 [US2] Implement rename flow: populate InputState.value with existing filename, set cursor_pos before extension (find last "."), on confirm execute fs::rename (as FsOp::Move with same parent) in src/app.rs
- [x] T021 [US2] Add `a` (create) and `r` (rename) keybindings: transition AppMode::Normal → AppMode::Input with appropriate InputState. Add Input mode key handling: Enter → confirm and execute, Esc → cancel and return to Normal, other keys → delegate to InputState.handle_key_event, in src/app.rs
- [x] T022 [US2] Render inline input in tree view: when AppMode::Input, show input field at cursor position row (replacing or overlaying the node line) using inline_input widget, in src/ui/tree_view.rs
- [x] T023 [US2] After create/rename, refresh affected directory in tree state (trigger re-read of parent directory) in src/app.rs

**Checkpoint**: Create and rename functional — inline input, file/dir creation with nested paths, rename with extension-aware cursor

---

## Phase 5: User Story 1 — コピー・移動 (Priority: P1)

**Goal**: y/x/p で yank/paste によるファイルコピーと移動。マーク済みファイルまたはカーソル位置を対象

**Independent Test**: y → 別ディレクトリ移動 → p → コピー確認。x → p → 移動確認（元ファイル消失）

**Dependencies**: US7 (Phase 3) — targets_or_cursor for multi-file selection

**FR**: FR-001, FR-002, FR-003, FR-004, FR-005, FR-006

- [ ] T024 [P] [US1] Implement YankBuffer struct (paths: Vec<PathBuf>, mode: YankMode) with YankMode enum (Copy, Cut), set/clear/is_empty/display_info methods in src/file_op/yank.rs. Derive serde for session persistence
- [ ] T025 [US1] Implement paste execution: for each path in YankBuffer, resolve destination (cursor dir + filename), apply conflict resolution if dst exists, execute FsOp::Copy (Copy mode) or FsOp::Move (Cut mode). After Cut paste, clear buffer; after Copy paste, retain buffer, in src/app.rs
- [ ] T026 [US1] Add `y` (yank/copy) and `x` (cut) keybindings: collect paths via targets_or_cursor, create YankBuffer, clear marks after yank. Add `p` (paste) keybinding: execute paste into cursor directory, in src/app.rs
- [ ] T027 [US1] Add self-reference check before directory paste: call is_ancestor for each src-dst pair, show error message if detected in src/app.rs
- [ ] T028 [US1] Refresh tree state after paste operations: trigger re-read of destination directory (and source directory for Cut mode) in src/app.rs

**Checkpoint**: Copy/move functional — yank, paste, conflict resolution, buffer lifecycle (copy retains, cut clears)

---

## Phase 6: User Story 3 — 削除 (Priority: P1)

**Goal**: d/D でファイル削除。確認ダイアログ付き。d は設定依存 (permanent/custom_trash)、D は常にOS標準ゴミ箱

**Independent Test**: d → 確認ダイアログ → y → ファイル削除確認。設定切り替えで動作変更確認

**Dependencies**: US7 (Phase 3) — targets_or_cursor for multi-file selection

**FR**: FR-012, FR-013, FR-014, FR-015, FR-019, FR-025, FR-026

- [ ] T029 [P] [US3] Implement confirmation dialog widget: centered modal overlay with message, scrollable file list, and "y/Enter to confirm, n/Esc to cancel" footer in src/ui/modal.rs
- [ ] T030 [US3] Implement permanent delete flow: on confirm, iterate paths and execute remove_file/remove_dir_all for each. This is an IrreversibleOp (not added to undo stack), in src/app.rs
- [ ] T031 [US3] Implement custom trash delete flow: on confirm, iterate paths, move each to trash dir (using file_op/trash::trash_path + FsOp::Move), in src/app.rs
- [ ] T032 [US3] Implement system trash delete flow: on confirm, call trash::delete_all via tokio::task::spawn_blocking for all target paths. This is an IrreversibleOp, in src/app.rs
- [ ] T033 [US3] Add `d` keybinding: check config delete_mode, build ConfirmState with appropriate message and ConfirmAction (PermanentDelete or CustomTrash), transition to AppMode::Confirm. Add `D` keybinding: build ConfirmState with SystemTrash action, in src/app.rs
- [ ] T034 [US3] Add Confirm mode key handling: y/Enter → execute on_confirm action and return to Normal, n/Esc → cancel and return to Normal, in src/app.rs
- [ ] T035 [US3] Render confirmation dialog: when AppMode::Confirm, render modal overlay on top of main UI using modal widget in src/ui.rs
- [ ] T036 [US3] Clear marks and refresh tree state after delete operations in src/app.rs

**Checkpoint**: Delete functional — permanent, custom trash, system trash, all with confirmation dialog

---

## Phase 7: User Story 8 — UI フィードバック (Priority: P2)

**Goal**: ステータスバーに yank/mark/操作結果を表示。操作中は "Processing..." 表示

**Independent Test**: y でファイルを yank → ステータスバーに [Y:1] 表示確認

**Dependencies**: US1 (Phase 5), US3 (Phase 6), US7 (Phase 3) — needs yank/mark/op state

**FR**: FR-040, FR-041, FR-043, FR-026

- [ ] T037 [US8] Add yank indicator ([Y:N] for copy, [X:N] for cut) and mark indicator ([M:N]) to status bar rendering, reading from AppState yank_buffer and mark_set in src/ui/status_bar.rs
- [ ] T038 [US8] Implement temporary message system: add status_message: Option<(String, Instant)> to AppState, set on operation completion (e.g. "Copied 3 files", "Renamed file.txt"), auto-clear after 3 seconds in render loop, in src/app.rs and src/ui/status_bar.rs
- [ ] T039 [US8] Add "Processing..." indicator: set a processing flag before blocking file operations, display in status bar, clear after operation completes, in src/app.rs and src/ui/status_bar.rs
- [ ] T040 [US8] Add "This operation cannot be undone" temporary message after irreversible operations (permanent delete, system trash) in src/app.rs

**Checkpoint**: UI feedback functional — yank/mark indicators, temporary messages, processing state

---

## Phase 8: User Story 4 — Undo/Redo (Priority: P2)

**Goal**: u/Ctrl+r で操作の取り消しとやり直し。操作グループ単位。事前検証。永続化可能

**Independent Test**: コピー → u → コピー先削除確認 → Ctrl+r → 再コピー確認

**Dependencies**: US1 (Phase 5), US2 (Phase 4), US3 (Phase 6) — OpGroup integration into each operation

**FR**: FR-021, FR-022, FR-023, FR-024, FR-025, FR-027, FR-028

- [ ] T041 [P] [US4] Implement OpGroup struct (description: String, ops: Vec<FsOp>, undo_ops: Vec<FsOp>, expect_exists: Vec<PathBuf>, expect_not_exists: Vec<PathBuf>) with serde derive in src/file_op/undo.rs
- [ ] T042 [US4] Implement UndoHistory struct (groups: Vec<OpGroup>, cursor: usize, max_size: usize) with push (truncates redo stack), can_undo, can_redo methods and serde derive in src/file_op/undo.rs
- [ ] T043 [US4] Implement undo method: validate expect_not_exists paths don't exist, execute undo_ops in reverse order, decrement cursor. Implement redo method: validate expect_exists paths exist, execute ops in order, increment cursor. Return error on validation failure, in src/file_op/undo.rs
- [ ] T044 [US4] Implement overflow handling: when groups.len() > max_size after push, remove groups[0], decrement cursor. If removed group references custom trash files, call trash::clean_trash_file for each, in src/file_op/undo.rs
- [ ] T045 [US4] Generate OpGroup for copy operations: ops=Copy per file, undo_ops=RemoveFile/RemoveDir per copied file, expect_exists=dst paths for undo, in src/app.rs
- [ ] T046 [US4] Generate OpGroup for move/cut operations: ops=Move per file, undo_ops=Move reverse per file, expect_exists/expect_not_exists accordingly, in src/app.rs
- [ ] T047 [US4] Generate OpGroup for create operations: ops=CreateFile/CreateDir, undo_ops=RemoveFile/RemoveDir, in src/app.rs
- [ ] T048 [US4] Generate OpGroup for rename operations: ops=Move(old→new), undo_ops=Move(new→old), in src/app.rs
- [ ] T049 [US4] Generate OpGroup for custom trash delete: ops=Move(path→trash), undo_ops=Move(trash→path), in src/app.rs
- [ ] T050 [US4] Add `u` (undo) and `Ctrl+r` (redo) keybindings: call UndoHistory.undo/redo, display result/error message, refresh tree state, in src/app.rs

**Checkpoint**: Undo/redo functional — operation tracking, group undo/redo, pre-validation, overflow with trash cleanup

---

## Phase 9: User Story 5 — FS 変更検出 (Priority: P2)

**Goal**: 展開中ディレクトリの外部変更を自動検出しツリーを更新。折り畳み中は Stale マーキング

**Independent Test**: 展開中ディレクトリに外部からファイル追加 → ツリー自動更新確認

**Dependencies**: Foundational (Phase 2) — independent of file operation user stories

**FR**: FR-029, FR-030, FR-031, FR-032

- [ ] T051 [P] [US5] Add ChildrenState::Stale(Vec<TreeNode>) variant to src/state/tree.rs. Update pattern matches in existing code (expand, collapse, prefetch, visible_nodes) to handle Stale: treat as NotLoaded on expand (trigger re-read), display stale children when collapsed
- [ ] T052 [US5] Implement FsWatcher struct: setup RecommendedWatcher with RecursiveMode::NonRecursive and notify-debouncer-mini (250ms timeout), expose watch(path)/unwatch(path) methods, in src/watcher.rs
- [ ] T053 [US5] Implement tokio bridge: spawn std::thread to receive debounced events from notify, forward via tokio::sync::mpsc::Sender<Vec<PathBuf>> to async event loop, in src/watcher.rs
- [ ] T054 [US5] Implement dynamic watch/unwatch: call watcher.watch(dir) when directory expands (ChildrenState → Loaded), call watcher.unwatch(dir) when directory collapses, in src/app.rs
- [ ] T055 [US5] Implement AtomicBool self-operation filter: add suppressed: Arc<AtomicBool> to FsWatcher, set true before file ops, set false after, check in event handler to skip events while suppressed, in src/watcher.rs
- [ ] T056 [US5] Implement FS event handler in app event loop: on watcher events, for each affected directory: if expanded (Loaded) → re-read children and update in place; if collapsed (Loaded but not expanded) → mark as Stale, in src/app.rs
- [ ] T057 [US5] Integrate watcher lifecycle: create FsWatcher on app init, watch initially expanded dirs, stop watcher on app quit, in src/app.rs

**Checkpoint**: FS change detection functional — auto-refresh expanded dirs, stale marking for collapsed, self-op filtering

---

## Phase 10: User Story 6 — セッション永続化 (Priority: P3)

**Goal**: セッション状態の保存/復元。起動時に前回の展開状態・カーソル・undo 履歴を復元

**Independent Test**: ディレクトリ展開 → 終了 → 再起動 → 展開状態・カーソル復元確認

**Dependencies**: US4 (Phase 8) — UndoHistory persistence

**FR**: FR-033, FR-034, FR-035, FR-036, FR-037

- [ ] T058 [P] [US6] Implement SessionState struct (root_path, last_accessed, expanded_dirs, cursor_path, scroll_offset, marked_paths, yank, undo_history) with serde Serialize/Deserialize in src/session.rs
- [ ] T059 [US6] Implement session file path computation: sha256 first 16 chars of canonical root_path → {data_dir}/trev/sessions/{hash}.json, in src/session.rs
- [ ] T060 [US6] Implement atomic write: write JSON to {hash}.json.tmp → fsync → rename to {hash}.json, using tempfile for cross-platform safety, in src/session.rs
- [ ] T061 [US6] Implement session save: extract state from AppState (expanded dirs, cursor path, scroll offset, marks, yank, undo_history), update last_accessed, write atomically. Trigger with 1s debounce on state changes (expand/collapse, file ops, undo/redo, quit), in src/session.rs and src/app.rs
- [ ] T062 [US6] Implement session restore: read JSON, validate all paths exist (filter out missing), rebuild TreeState with saved expanded dirs and cursor position, restore marks/yank/undo_history, in src/session.rs and src/app.rs
- [ ] T063 [US6] Integrate CLI restore options: --restore forces restore, --no-restore skips, neither defers to config session.restore_by_default, in src/app.rs
- [ ] T064 [US6] Implement expired session cleanup: on startup, scan sessions dir, delete files with last_accessed older than expiry_days, in src/session.rs

**Checkpoint**: Session persistence functional — save, restore, CLI control, path validation, expiry cleanup

---

## Phase 11: Polish & Cross-Cutting Concerns

**Purpose**: Edge cases, symlink handling, and final validation

- [ ] T065 Implement symlink-aware operations: copy symlink recreates the link (not following target), move/delete operates on link only, in src/file_op/executor.rs
- [ ] T066 Handle permission errors and read-only directories: catch PermissionDenied in executor, display user-friendly error message via status bar, in src/app.rs
- [ ] T067 Handle cut-yanked file externally deleted before paste: check source paths exist before paste execution, show error for missing files, in src/app.rs
- [ ] T068 Run full validation: mise run test && mise run lint && mise run format. Fix any failures

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Setup — BLOCKS all user stories
- **US7 Mark (Phase 3)**: Depends on Foundational
- **US2 Create/Rename (Phase 4)**: Depends on Foundational — can run in parallel with US7
- **US1 Copy/Move (Phase 5)**: Depends on US7 (mark targets)
- **US3 Delete (Phase 6)**: Depends on US7 (mark targets) — can run in parallel with US1
- **US8 UI Feedback (Phase 7)**: Depends on US1, US3, US7 (needs yank/mark/op state)
- **US4 Undo/Redo (Phase 8)**: Depends on US1, US2, US3 (OpGroup generation)
- **US5 FS Watch (Phase 9)**: Depends on Foundational only — can run in parallel with any post-Foundational phase
- **US6 Session (Phase 10)**: Depends on US4 (UndoHistory persistence)
- **Polish (Phase 11)**: Depends on all user stories being complete

### User Story Dependencies

```
Phase 2 (Foundational)
├── Phase 3 (US7: Mark) ──┬── Phase 5 (US1: Copy/Move) ──┐
│                          └── Phase 6 (US3: Delete) ──────┤
├── Phase 4 (US2: Create/Rename) ─────────────────────────┤
│                                                          ├── Phase 7 (US8: UI Feedback)
│                                                          ├── Phase 8 (US4: Undo/Redo) ── Phase 10 (US6: Session)
└── Phase 9 (US5: FS Watch) ← independent                 │
                                                           └── Phase 11 (Polish)
```

### Parallel Opportunities

**Phase 2 (Foundational)**:
- T003, T004, T005, T006, T007, T008, T009, T011 can run in parallel (different files)
- T010 depends on T007/T008/T009 (executor uses FsOp types, conflict, trash)

**After Foundational**:
- US7 (Phase 3) and US2 (Phase 4) can run in parallel
- US1 (Phase 5) and US3 (Phase 6) can run in parallel (after US7)
- US5 (Phase 9) can run in parallel with any story after Foundational
- US4 (Phase 8) and US5 (Phase 9) can run in parallel

---

## Parallel Example: Foundational Phase

```bash
# Independent type definitions (all different files):
Task: "Implement MarkSet in src/file_op/mark.rs"        # T006
Task: "Implement FsOp enum in src/file_op/executor.rs"   # T007
Task: "Implement conflict resolution in src/file_op/conflict.rs"  # T008
Task: "Implement custom trash in src/file_op/trash.rs"   # T009
```

## Parallel Example: After Foundational

```bash
# US7 and US2 can proceed in parallel:
Task: "Integrate MarkSet into AppState in src/app.rs"    # T013 (US7)
Task: "Implement InputState editing in src/input.rs"     # T017 (US2)
```

## Parallel Example: After US7

```bash
# US1 and US3 can proceed in parallel:
Task: "Implement YankBuffer in src/file_op/yank.rs"      # T024 (US1)
Task: "Implement confirmation dialog in src/ui/modal.rs" # T029 (US3)
```

---

## Implementation Strategy

### MVP First (P1 Stories Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL — blocks all stories)
3. Complete Phase 3: US7 — Mark
4. Complete Phase 4: US2 — Create/Rename
5. Complete Phase 5: US1 — Copy/Move
6. Complete Phase 6: US3 — Delete
7. **STOP and VALIDATE**: All P1 stories functional — basic file manager operations complete

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. US7 (Mark) → Multi-select operational
3. US2 (Create/Rename) → File creation/rename operational
4. US1 (Copy/Move) + US3 (Delete) → Core file ops complete (MVP!)
5. US8 (UI Feedback) → Visual indicators and status messages
6. US4 (Undo/Redo) → Safety net with operation history
7. US5 (FS Watch) → External change detection
8. US6 (Session) → Full state persistence
9. Polish → Edge cases and validation

---

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- TDD: Write failing tests first, then implement, then refactor
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Strict clippy (all/pedantic/nursery/cargo at deny): ensure const fn for simple methods, no unwrap/expect, no indexing
