# Tasks: TOML ユーザー設定

**Input**: Design documents from `/specs/001-user-config/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md
**Scope**: US1 (TOML 静的設定), US3 (CLI 引数上書き). US2 (Lua) は将来フェーズ.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: 依存関係の更新

- [x] T001 Add `serde_ignored = "0.1"` and `dirs = "6"` to Cargo.toml, remove `directories` dependency

---

## Phase 2: User Story 1 - TOML による静的設定 (Priority: P1) 🎯 MVP

**Goal**: TOML 設定ファイルの堅牢な読み込み。XDG パス、未知キー警告、エラーメッセージ改善。

**Independent Test**: `~/.config/trev/config.toml` にソート設定を書き、trev 起動で反映を確認。構文エラー時はエラー表示+デフォルト起動。

### Tests for US1

- [x] T002 [P] [US1] Test: `config_dir()` が `$XDG_CONFIG_HOME/trev/` を返すことを検証 in src/config.rs
- [x] T003 [P] [US1] Test: `config_dir()` が XDG 未設定時に `~/.config/trev/` にフォールバックすることを検証 in src/config.rs
- [x] T004 [P] [US1] Test: 設定ファイルが存在しない場合 `Config::default()` が返ることを検証 in src/config.rs
- [x] T005 [P] [US1] Test: 空の設定ファイルで全デフォルト値が適用されることを検証 in src/config.rs
- [x] T006 [P] [US1] Test: 有効な TOML で各フィールドが正しくデシリアライズされることを検証 in src/config.rs (既存 test_config_deserialize を拡張)
- [x] T007 [P] [US1] Test: 未知キーがある場合 warnings に収集され config は正常に返ることを検証 in src/config.rs
- [x] T008 [P] [US1] Test: 構文エラーのある TOML でエラーメッセージにファイルパスと行番号が含まれることを検証 in src/config.rs
- [x] T009 [P] [US1] Test: 読み取り不可ファイルでエラーが返ることを検証 in src/config.rs

### Implementation for US1

- [x] T010 [US1] Implement `config_dir()` with explicit XDG path resolution (`$XDG_CONFIG_HOME` → `~/.config/`) in src/config.rs, replacing `directories` usage with `dirs::home_dir()`
- [x] T011 [US1] Implement `ConfigLoadResult` struct with `config: Config` and `warnings: Vec<String>` in src/config.rs
- [x] T012 [US1] Refactor `Config::load()` to return `ConfigLoadResult`, using `serde_ignored::deserialize` for unknown key detection in src/config.rs
- [x] T013 [US1] Add `anyhow::Context` to TOML parse errors to include file path in error messages in src/config.rs
- [x] T014 [US1] Update `Config::load_from()` to use `serde_ignored` and collect warnings in src/config.rs
- [x] T015 [US1] Update app.rs to handle `ConfigLoadResult`: display warnings via `tracing::warn`, apply CLI overrides after config load

**Checkpoint**: TOML 設定ファイルの読み込みが堅牢に動作。未知キー警告、エラーメッセージ改善が完了。

---

## Phase 3: User Story 3 - CLI 引数による設定の上書き (Priority: P3)

**Goal**: CLI 引数で TOML 設定を一時的に上書き。優先順位: CLI > TOML > Default。

**Independent Test**: TOML で `show_hidden = false` の状態で `trev -a` を実行し、隠しファイルが表示されることを確認。

### Tests for US3

- [x] T016 [P] [US3] Test: `apply_cli_overrides` で `--sort-order size` が SortOrder::Size に上書きされることを検証 in src/config.rs
- [x] T017 [P] [US3] Test: `apply_cli_overrides` で `--no-preview` が show_preview=false に上書きされることを検証 in src/config.rs
- [x] T018 [P] [US3] Test: CLI 引数未指定時は TOML の値が保持されることを検証 in src/config.rs
- [x] T019 [P] [US3] Test: `--show-ignored` が show_ignored=true に上書きされることを検証 in src/config.rs

### Implementation for US3

- [x] T020 [US3] Add CLI flags to Args struct: `--sort-order`, `--sort-direction`, `--no-directories-first`, `--show-ignored`, `--no-preview` in src/cli.rs
- [x] T021 [US3] Implement `Config::apply_cli_overrides(&mut self, args: &Args)` in src/config.rs
- [x] T022 [US3] Integrate CLI overrides into app startup: call `apply_cli_overrides` after config load in src/app.rs
- [x] T023 [US3] Refactor existing `-a` / `--show-hidden` handling in src/app.rs to go through `apply_cli_overrides` instead of direct field access

**Checkpoint**: CLI 引数が TOML 設定を正しく上書き。`-a` の既存動作も `apply_cli_overrides` 経由に統一。

---

## Phase 4: Polish & Cross-Cutting Concerns

**Purpose**: ビルド検証、lint 修正、統合確認

- [x] T024 Verify all tests pass with `mise run test`
- [x] T025 Fix any clippy warnings with `mise run lint`
- [x] T026 Verify release build with `cargo build --release`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies
- **US1 (Phase 2)**: Depends on Phase 1. Tests T002-T009 can run in parallel, then implementation T010-T015 sequentially.
- **US3 (Phase 3)**: Depends on Phase 2 (needs ConfigLoadResult). Tests T016-T019 in parallel, then implementation T020-T023 sequentially.
- **Polish (Phase 4)**: Depends on Phase 3 completion.

### Parallel Opportunities

**Within US1 tests**: T002-T009 are all independent (different test cases, same file)
**Within US3 tests**: T016-T019 are all independent
**Between US1 and US3**: Sequential — US3 depends on ConfigLoadResult from US1

---

## Implementation Strategy

### MVP First (US1 Only)

1. Phase 1: Setup (T001)
2. Phase 2: US1 tests → implementation (T002-T015)
3. **STOP and VALIDATE**: config.toml を作成し、設定反映・未知キー警告・エラー表示を確認

### Incremental Delivery

1. Setup → US1 → Validate (MVP)
2. US3 → Validate (CLI 上書き)
3. Polish → Final validation
