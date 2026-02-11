# Tasks: TUI 基盤 — イベントループ・ツリー描画・キー入力

**Input**: Design documents from `/specs/011-tui-ratatui/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, quickstart.md

**Tests**: テスト専用フェーズなし。実装タスク内でロジック部分のユニットテストを含む（TDD ベースでイベントハンドリングのテストを実装）。

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: tokio runtime への移行、CLI 拡張、ターミナル管理モジュールの追加

- [x] T001 Add `--no-icons` flag to CLI args in `src/cli.rs`
- [x] T002 Convert `fn main()` to `#[tokio::main] async fn main()` and `app::run()` to async in `src/main.rs` and `src/app.rs`
- [x] T003 Create `src/terminal.rs` — ターミナル初期化 (`ratatui::init()`) / 復帰 (`ratatui::restore()`) ヘルパー関数

**Checkpoint**: tokio ランタイム稼働、`--no-icons` フラグが使える、ターミナルの初期化/復帰が動作する

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: AppState、Action enum 拡張、スクロール管理 — すべての User Story が依存する基盤

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [x] T004 Implement `AppState` struct (wrapping `TreeState`, `should_quit`, `show_icons`, `viewport_height`) in `src/app.rs`
- [x] T005 [P] Implement `ScrollState` struct (offset, viewport_height) with scroll offset clamping logic in `src/app.rs`
- [x] T006 [P] Extend `Action` / `TreeAction` enum with UI action variants (`Quit`, `MoveUp`, `MoveDown`, `Expand`, `Collapse`, `ToggleExpand`, `JumpFirst`, `JumpLast`, `HalfPageDown`, `HalfPageUp`) in `src/action.rs`
- [x] T007 Implement `ChildrenLoadResult` struct (`path: PathBuf`, `result: Result<Vec<TreeNode>>`) in `src/app.rs`

**Checkpoint**: Foundation ready — AppState, ScrollState, Action variants, ChildrenLoadResult all defined

---

## Phase 3: User Story 1 — ツリー表示でディレクトリを閲覧する (Priority: P1) 🎯 MVP

**Goal**: ターミナルで trev を起動するとファイルツリーが表示され、カーソル行がハイライトされ、`q` で終了できる

**Independent Test**: `cargo run` で起動 → ツリーが画面に表示される → カーソル行がハイライト → `q` で正常終了を目視確認

### Implementation for User Story 1

- [x] T008 [US1] Implement tree_view widget — `render_tree()` function rendering visible nodes with indentation and cursor highlight in `src/ui/tree_view.rs`
- [x] T009 [US1] Implement devicons integration — file icon + color lookup using `devicons::FileIcon` in `src/ui/tree_view.rs`
- [x] T010 [US1] Implement top-level `render()` layout function (tree_view area + status_bar area split) in `src/ui.rs`
- [x] T011 [US1] Implement basic event loop — `terminal.draw()` + `crossterm::event::poll()` + `q` to quit in `src/app.rs`
- [x] T012 [US1] Wire up initial TreeBuilder::build() + TreeState + AppState + render in `src/app.rs` (startup flow)

**Checkpoint**: `cargo run` でツリー表示、カーソルハイライト、`q` で終了が動作する

---

## Phase 4: User Story 2 — キーボードでツリーをナビゲーションする (Priority: P1)

**Goal**: Vim ライクなキーバインド (j/k/h/l/g/G/Ctrl+d/Ctrl+u/Enter) でツリー内を移動・展開・折り畳みできる

**Independent Test**: `cargo run` で起動 → 各キーバインドでカーソル移動・ディレクトリ展開・折り畳みを目視確認

### Implementation for User Story 2

- [x] T013 [US2] Create `map_key_event()` function mapping `crossterm::event::KeyEvent` to `Action` enum in `src/app.rs`
- [x] T014 [US2] Implement `handle_tree_action()` function dispatching `Action` to `AppState` / `TreeState` methods in `src/app.rs`
- [x] T015 [US2] Unit tests for `map_key_event()` — verify all key bindings map to correct Action variants in `src/app.rs`
- [x] T016 [US2] Integrate event handling into event loop in `src/app.rs`
- [x] T017 [US2] Implement scroll offset tracking — `ScrollState.clamp_to_cursor()` on cursor movement in `src/app.rs`

**Checkpoint**: すべてのキーバインドが動作し、カーソル移動・展開・折り畳み・ジャンプ・半ページ移動ができる

---

## Phase 5: User Story 3 — ディレクトリの遅延読み込み (Priority: P1)

**Goal**: ディレクトリ展開時に `tokio::spawn_blocking` で非同期に子ノードを読み込み、UI がブロックされない

**Independent Test**: 大きなディレクトリで `l` を押して展開 → UI がフリーズしない → 読み込み完了後にツリー更新を目視確認

### Implementation for User Story 3

- [x] T018 [US3] Set up `tokio::sync::mpsc` channel (`children_tx` / `children_rx`) in `src/app.rs` event loop
- [x] T019 [US3] Implement async load dispatch — on `ExpandResult::NeedsLoad`, spawn `tokio::spawn_blocking` with `TreeBuilder::load_children()` in `src/app.rs`
- [x] T020 [US3] Implement `try_recv()` result handling — receive `ChildrenLoadResult` and call `TreeState::set_children()` / `set_children_error()` in `src/app.rs`
- [x] T021 [US3] Render loading indicator — show `[Loading...]` for directories with `ChildrenState::Loading` in `src/ui/tree_view.rs`

**Checkpoint**: ディレクトリ展開が非同期で動作し、Loading インジケーター表示後にツリーが自動更新される

---

## Phase 6: User Story 4 — 画面リサイズ対応 (Priority: P2)

**Goal**: ターミナルウィンドウのリサイズに自動追従し、表示が崩れない

**Independent Test**: trev 実行中にターミナルをリサイズ → 表示が追従し崩れないことを目視確認

### Implementation for User Story 4

- [x] T022 [US4] Handle `Event::Resize` in event loop — viewport_height updated on next draw via `ui::render()` in `src/app.rs`
- [x] T023 [US4] Ensure scroll offset re-clamping on viewport height change — `clamp_to_cursor()` runs every loop iteration in `src/app.rs`

**Checkpoint**: リサイズ追従が動作し、カーソルが画面外に消えない

---

## Phase 7: User Story 5 — ステータスバー表示 (Priority: P2)

**Goal**: 画面下部にステータスバーを表示し、選択ファイルのパスとカーソル位置 (N/Total) を確認できる

**Independent Test**: `cargo run` で起動 → カーソル移動に応じてステータスバーの情報が更新されることを目視確認

### Implementation for User Story 5

- [x] T024 [US5] Implement status_bar widget — `render_status()` function showing file path and cursor position (N/Total) in `src/ui/status_bar.rs`
- [x] T025 [US5] Integrate status_bar rendering into top-level `render()` layout in `src/ui.rs`

**Checkpoint**: ステータスバーにパスとカーソル位置が表示され、カーソル移動で更新される

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: エッジケース対応、パフォーマンス確認、コード品質

- [x] T026 Handle empty directory display — show `(empty)` placeholder in `src/ui/tree_view.rs`
- [x] T027 Handle permission errors gracefully — `set_children_error()` reverts to NotLoaded state
- [x] T028 Verify long file name truncation at terminal width boundary — ratatui Paragraph auto-truncates
- [x] T029 Run `mise run lint` and fix all clippy warnings
- [x] T030 Run `mise run test` and verify all existing + new tests pass (54 passed, 2 ignored)
- [x] T031 Run `mise run build` and verify release build succeeds
- [ ] T032 Manual testing — run quickstart.md scenarios and verify all key patterns work

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 (T001-T003) — BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Phase 2 — delivers MVP (ツリー表示 + quit)
- **User Story 2 (Phase 4)**: Depends on Phase 3 (event loop must exist)
- **User Story 3 (Phase 5)**: Depends on Phase 4 (event handling must exist for expand triggers)
- **User Story 4 (Phase 6)**: Depends on Phase 3 (event loop must exist)
- **User Story 5 (Phase 7)**: Depends on Phase 3 (render layout must exist)
- **Polish (Phase 8)**: Depends on all user stories being complete

### User Story Dependencies

- **US1 (P1)**: Can start after Foundational (Phase 2)
- **US2 (P1)**: Depends on US1 (event loop from US1 is the base)
- **US3 (P1)**: Depends on US2 (expand action triggers async load)
- **US4 (P2)**: Can start after US1 (only needs event loop)
- **US5 (P2)**: Can start after US1 (only needs render layout)

### Within Each User Story

- Widget implementation before event loop integration
- Core logic before visual polish
- Unit tests alongside implementation (TDD)

### Parallel Opportunities

- **Phase 2**: T005 (ScrollState) and T006 (Action enum) can run in parallel [P]
- **Phase 6 + Phase 7**: US4 and US5 can proceed in parallel after US1 (different files, no dependencies)
- **Phase 3**: T008 (tree_view widget) and T009 (devicons) can be part of same file but logical separation exists

---

## Parallel Example: Phase 2

```bash
# Launch parallelizable foundational tasks:
Task: "Implement ScrollState in src/app.rs"
Task: "Extend Action enum in src/action.rs"
```

## Parallel Example: Phase 6 + Phase 7

```bash
# After US1 complete, launch US4 and US5 in parallel:
Task: "Handle Event::Resize in src/app.rs"
Task: "Implement status_bar widget in src/ui/status_bar.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T003)
2. Complete Phase 2: Foundational (T004-T007)
3. Complete Phase 3: User Story 1 (T008-T012)
4. **STOP and VALIDATE**: `cargo run` でツリーが表示され `q` で終了できることを確認
5. Proceed to User Story 2 for navigation

### Incremental Delivery

1. Setup + Foundational → Infrastructure ready
2. US1 → ツリー表示 + quit (MVP!)
3. US2 → キーボードナビゲーション (fully interactive)
4. US3 → 非同期読み込み (production ready)
5. US4 + US5 → リサイズ対応 + ステータスバー (polish)
6. Polish → エッジケース・最終検証

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Commit after each phase or logical group
- Stop at any checkpoint to validate story independently
- Strict clippy rules apply: no unsafe, no unwrap/expect, no indexing
- `ratatui::init()` / `ratatui::restore()` for terminal management (research.md R-002)
- `crossterm::event::poll()` based event loop (research.md R-001)
- `tokio::spawn_blocking` for async directory loading (research.md R-003)
