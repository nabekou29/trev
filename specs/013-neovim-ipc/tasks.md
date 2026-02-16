# Tasks: Neovim IPC 連携

**Input**: Design documents from `/specs/013-neovim-ipc/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ipc-protocol.md

**Tests**: TDD アプローチ — テスト可能なコンポーネント (JSON-RPC types, paths, reveal) はテストファーストで実装。

**Organization**: User story ごとに独立実装・独立テスト可能なフェーズに分割。

## Format: `[ID] [P?] [Story] Description`

- **[P]**: 並列実行可能（異なるファイル、依存なし）
- **[Story]**: 対応する User Story（US1, US2, US3, US4）

---

## Phase 1: Setup

**Purpose**: IPC モジュール構造の初期化

- [ ] T001 Convert src/ipc.rs stub to module directory: create src/ipc.rs (module root with submodule declarations) and empty submodules src/ipc/types.rs, src/ipc/paths.rs, src/ipc/server.rs, src/ipc/client.rs

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: すべての User Story が依存する IPC 基盤。JSON-RPC プロトコル実装、ソケットパス、サーバー、メインループ統合。

**CRITICAL**: このフェーズが完了するまで User Story の実装は不可。

- [ ] T002 [P] Implement JSON-RPC message types (JsonRpcMessage enum with untagged serde, JsonRpcError, standard error codes), IpcCommand enum (Reveal, Quit with oneshot response channels), and EditorAction enum (Edit/Split/Vsplit/Tabedit with serde serialization) in src/ipc/types.rs
- [ ] T003 [P] Implement workspace_key() (Git root directory name, fallback to cwd name) and socket_path() (XDG_RUNTIME_DIR/trev/<workspace_key>-<pid>.sock with TMPDIR and /tmp fallback) with unit tests in src/ipc/paths.rs
- [ ] T004 Implement IPC Server: tokio UnixListener, per-connection read task (BufReader line-by-line), JSON-RPC message parsing and method dispatch (ping handled directly, reveal/quit dispatched via mpsc channel), persistent client writer management (Arc<Mutex<Option<OwnedWriteHalf>>>) for outgoing notifications in src/ipc/server.rs
- [ ] T005 Integrate IPC into application lifecycle: start IPC server task when --daemon flag is set, add ipc_rx channel to main event loop (try_recv pattern matching existing children_rx/watcher_rx/preview_rx), wire daemon startup in src/main.rs and src/app.rs
- [ ] T006 Implement IPC command handlers in main loop: Quit sets should_quit=true and sends response, Reveal dispatches to handler (placeholder until US2) in src/app/handler.rs

**Checkpoint**: IPC サーバーが起動し、ping レスポンスを返し、quit でシャットダウンできる状態。socat でテスト可能。

---

## Phase 3: User Story 1 - サイドパネルファイルブラウザ (Priority: P1) MVP

**Goal**: Neovim の横に trev を常駐サイドパネルとして開き、ファイル選択で Neovim のバッファにファイルが開かれる。

**Independent Test**: Neovim で `:TrevToggle` → サイドパネルが開く → ファイル選択 → Neovim でファイルが開く → サイドパネルは開いたまま。

### Implementation for User Story 1

- [ ] T007 [US1] Add notification sending API to IPC server: send_open_file(action, path) and send_notification(method, params) that write JSON-RPC notification to persistent client writer in src/ipc/server.rs
- [ ] T008 [US1] Integrate EditorAction from --action CLI flag into AppContext/AppState, make it accessible to handlers in src/app/state.rs
- [ ] T009 [US1] Handle ExpandResult::OpenFile in daemon mode: when --daemon is active, send open_file notification with configured EditorAction instead of no-op in src/app/handler/tree.rs
- [ ] T010 [US1] Create Neovim plugin core: setup(opts) function with config (trev_path, width, auto_reveal, float, handlers), user commands (:TrevToggle, :TrevOpen, :TrevClose), plugin state management in nvim-plugin/lua/trev/init.lua
- [ ] T011 [US1] Implement side panel terminal: detect toggleterm.nvim or snacks.nvim, create vertical split terminal running trev --daemon, manage panel open/close/toggle lifecycle in nvim-plugin/lua/trev/init.lua
- [ ] T012 [US1] Implement JSON-RPC client in Neovim plugin: vim.uv.new_pipe() connection to UDS, read_start() for incoming messages, line-buffered JSON parsing, write() for outgoing messages in nvim-plugin/lua/trev/init.lua
- [ ] T013 [US1] Handle open_file notification in Neovim plugin: parse action and path, execute vim.cmd(action .. " " .. path) on vim.schedule() in nvim-plugin/lua/trev/init.lua
- [ ] T014 [P] [US1] Create plugin loader that calls require("trev") in nvim-plugin/plugin/trev.lua

**Checkpoint**: `:TrevToggle` でサイドパネルが開き、ファイル選択で Neovim にファイルが開かれる。

---

## Phase 4: User Story 2 - 現在のファイルを Reveal (Priority: P1)

**Goal**: Neovim で作業中のファイルを trev ツリーで自動ハイライトする。手動 reveal と BufEnter による自動 reveal。

**Independent Test**: Neovim でファイルを開き `:TrevReveal` → サイドパネルがそのファイルまでスクロールしてハイライト。バッファ切り替えで自動 reveal。

### Implementation for User Story 2

- [ ] T015 [US2] Implement TreeState::reveal_path(target): resolve relative path from root, iterate ancestors, sync load_children for unloaded dirs, expand collapsed dirs, call move_cursor_to_path(target) with unit tests in src/state/tree.rs
- [ ] T016 [US2] Wire Reveal IPC command: handler calls reveal_path() and sends JSON-RPC success response. Also implement --reveal CLI flag using reveal_path() on startup in src/app/handler.rs and src/app.rs
- [ ] T017 [US2] Add :TrevReveal command to Neovim plugin: send JSON-RPC reveal request (notification for auto-reveal, request for manual reveal) with current buffer path or specified path in nvim-plugin/lua/trev/init.lua
- [ ] T018 [US2] Implement auto-reveal on BufEnter: create autocmd that sends reveal notification on buffer switch, skip special buffers (terminal, quickfix, help) and trev's own terminal buffer in nvim-plugin/lua/trev/init.lua

**Checkpoint**: `:TrevReveal` でファイルが reveal され、BufEnter で自動 reveal が動作する。

---

## Phase 5: User Story 3 - フロートピッカー (Priority: P2)

**Goal**: フローティングウィンドウで素早くファイル選択し、Neovim で開く。ウィンドウレイアウトを変更しない。

**Independent Test**: `:TrevToggle float` → フローティングウィンドウ表示 → ファイル選択 → ウィンドウが閉じてファイルが開く。

### Implementation for User Story 3

- [ ] T019 [US3] Implement --emit mode: accumulate selected paths on ExpandResult::OpenFile, set should_quit on first selection, output accumulated paths to stdout after terminal restore (format per --emit-format: lines/nul/json) in src/app/handler/tree.rs and src/main.rs
- [ ] T020 [US3] Implement float picker in Neovim plugin: create centered floating terminal window running trev --emit, read terminal output on process exit, open selected file with configured action (default or argument), handle cancel (no output) in nvim-plugin/lua/trev/init.lua

**Checkpoint**: `:TrevToggle float` でフロートピッカーが開き、ファイル選択で Neovim にファイルが開く。キャンセルで閉じる。

---

## Phase 6: User Story 4 - CLI Daemon 制御 (Priority: P3)

**Goal**: `trev ctl` コマンドで外部から daemon を制御する。スクリプティングや外部ツール連携。

**Independent Test**: ターミナル 1 で `trev --daemon`、ターミナル 2 で `trev ctl ping` → 成功レスポンス。

### Implementation for User Story 4

- [ ] T021 [US4] Implement IPC Client: connect to UDS, send JSON-RPC request, read response with timeout, handle connection errors in src/ipc/client.rs
- [ ] T022 [US4] Route trev ctl subcommands in main.rs: match Command::Ctl, use workspace flag to find socket via glob pattern (<workspace_key>-*.sock), call IPC client for ping/reveal/quit, print results in src/main.rs
- [ ] T023 [US4] Add Command::SocketPath subcommand to CLI (src/cli.rs), implement socket discovery: list all .sock files in runtime dir, filter by optional --workspace flag, print paths in src/main.rs

**Checkpoint**: `trev ctl ping/reveal/quit` が動作し、`trev socket-path` がソケット一覧を表示する。

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: external_commands、get_state、エッジケース対応。

- [ ] T024 Add [external_commands] section to config: parse HashMap<String, String> (key representation → command name) in src/config.rs
- [ ] T025 Implement external command key handling: detect external_command key press in handler, send external_command JSON-RPC notification with command name to connected Neovim client in src/app/handler.rs and src/ipc/server.rs
- [ ] T026 Handle external_command notification in Neovim plugin: dispatch to user-defined handler function from setup(opts).handlers table in nvim-plugin/lua/trev/init.lua
- [ ] T027 Implement get_state request handling: return cursor_path, root_path, selected_paths in JSON-RPC response. Add IpcCommand::GetState variant with oneshot response channel in src/ipc/server.rs, src/ipc/types.rs, src/app/handler.rs
- [ ] T028 Implement stale socket cleanup: on daemon startup, remove socket file if it exists (previous crash). Handle edge cases: client disconnect detection (log + release writer), no client connected (discard notifications with log), invalid JSON (JSON-RPC parse error response) in src/ipc/server.rs
- [ ] T029 Run quickstart.md validation: socat による JSON-RPC テスト、trev ctl 全コマンド動作確認、Neovim プラグイン手動テスト

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: 依存なし — 即時開始可能
- **Foundational (Phase 2)**: Phase 1 完了後 — すべての User Story をブロック
- **US1 Side Panel (Phase 3)**: Phase 2 完了後
- **US2 Reveal (Phase 4)**: Phase 2 完了後（US1 と並列可能だが、Neovim プラグインが US1 で作成されるため US1 後が自然）
- **US3 Float Picker (Phase 5)**: Phase 2 完了後（US1/US2 と独立 — --emit は daemon モードと排他）
- **US4 CLI Control (Phase 6)**: Phase 2 完了後（US1/US2 と独立）
- **Polish (Phase 7)**: 全 User Story 完了後

### User Story Dependencies

- **US1 (P1)**: Phase 2 完了で開始可能。他の Story に依存なし。
- **US2 (P1)**: Phase 2 完了で開始可能。Neovim プラグインが US1 で作成されるため、US1 後の実装が効率的。
- **US3 (P2)**: Phase 2 完了で開始可能。daemon モードと独立（--emit は排他フラグ）。Neovim プラグインの float 部分は US1 後。
- **US4 (P3)**: Phase 2 完了で開始可能。IPC Client は新規ファイルで他に依存なし。

### Within Each User Story

- Rust 側の実装 → Neovim プラグイン側の実装（Rust 側が先にないとテスト不可）
- サーバー API → ハンドラー統合 → プラグイン
- reveal_path() → Reveal コマンドハンドラー → プラグイン :TrevReveal

### Parallel Opportunities

- **Phase 2**: T002 + T003 は並列実行可能（異なるファイル）
- **Phase 3**: T008 + T014 は並列実行可能（異なるファイル）
- **Phase 6**: T021 は新規ファイルで他と独立
- **Cross-story**: US3 (--emit) と US4 (ctl) は US1/US2 と並列開発可能（異なるコードパス）

---

## Parallel Example: Phase 2 (Foundational)

```
# JSON-RPC types と socket path を同時に実装:
Task T002: "JSON-RPC types in src/ipc/types.rs"
Task T003: "Socket path utilities in src/ipc/paths.rs"

# 完了後、IPC Server を実装:
Task T004: "IPC Server in src/ipc/server.rs" (depends on T002, T003)
```

## Parallel Example: Cross-Story

```
# US1 完了後、US2 と US4 を同時に進行:
Developer A: T015-T018 (US2: Reveal)
Developer B: T021-T023 (US4: CLI Control)
```

---

## Implementation Strategy

### MVP First (US1: Side Panel のみ)

1. Phase 1: Setup 完了
2. Phase 2: Foundational 完了（IPC 基盤）
3. Phase 3: US1 完了（サイドパネル + ファイルオープン）
4. **STOP and VALIDATE**: `:TrevToggle` でサイドパネル表示、ファイル選択で Neovim にファイルが開くことを確認
5. MVP として利用可能

### Incremental Delivery

1. Setup + Foundational → IPC 基盤完了
2. US1 (Side Panel) → テスト → **MVP リリース**
3. US2 (Reveal) → テスト → サイドパネル + reveal 連携
4. US3 (Float Picker) → テスト → 軽量ファイル選択手段追加
5. US4 (CLI Control) → テスト → スクリプティング対応
6. Polish → external_commands, get_state, エッジケース

### 推奨実行順序 (Single Developer)

```
Phase 1 → Phase 2 → Phase 3 (US1) → Phase 4 (US2)
→ Phase 5 (US3) → Phase 6 (US4) → Phase 7 (Polish)
```

---

## Notes

- [P] タスク = 異なるファイル、依存なし
- [Story] ラベル = User Story へのトレーサビリティ
- Neovim プラグイン (nvim-plugin/lua/trev/init.lua) は US1 で作成し、US2/US3/US4 で機能を追加する積み上げ方式
- `--emit` モードは `--daemon` と排他（spec 記載）
- `print_stdout` は `--emit` と `socket-path` でのみ許可（#[allow] 必要）
- Rust 2024 edition、nightly-2026-01-24、strict clippy ルールに従うこと
- Commit after each task or logical group
