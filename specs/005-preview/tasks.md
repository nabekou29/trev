# Tasks: ファイルプレビュー

**Input**: Design documents from `/specs/005-preview/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md

**Tests**: TDD アプローチ (constitution II)。各プロバイダーにユニットテスト。

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: 依存追加、モジュール構造作成、共通データ型定義

- [x] T001 Add new dependencies to Cargo.toml: content_inspector 0.2, lru 0.12, terminal-light 1, ratatui-image 5, image 0.25, resvg 0.46, ansi-to-tui 7
- [x] T002 Create module structure: src/preview.rs (pub mod declarations), src/preview/ directory with content.rs, state.rs, cache.rs, provider.rs, highlight.rs, and src/preview/providers/ with text.rs, image.rs, external.rs, fallback.rs
- [x] T003 [P] Implement PreviewContent enum in src/preview/content.rs (HighlightedText, PlainText, AnsiText, Image, Binary, Directory, Loading, Error, Empty variants)
- [x] T004 [P] Implement PreviewAction enum (ScrollDown, ScrollUp, ScrollRight, ScrollLeft, HalfPageDown, HalfPageUp, CycleProvider) and add Action::Preview variant in src/action.rs
- [x] T005 [P] Add PreviewConfig (max_lines, max_bytes, cache_size, commands, command_timeout) and ExternalCommand struct to src/config.rs with serde derive and defaults

**Checkpoint**: モジュール構造と共通型が揃い、`mise run build` が通ること

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Provider trait、Registry、Cache、Highlight ユーティリティ — 全 US の前提

**CRITICAL**: この Phase が完了しないと US 実装に入れない

- [x] T006 Implement PreviewProvider trait and LoadContext struct in src/preview/provider.rs — trait: name(), priority(), can_handle(), is_enabled() (default true), load(); LoadContext: max_lines, max_bytes, cancel_token
- [x] T007 Implement PreviewRegistry in src/preview/provider.rs — providers: Vec<Box<dyn PreviewProvider>>, resolve(path, is_dir) method with can_handle → is_enabled filtering, new() with builder pattern to register providers
- [x] T008 Implement PreviewCache (LRU) in src/preview/cache.rs — CacheKey { path, provider_name }, LruCache with configurable capacity, get/put methods
- [x] T009 Implement syntect → ratatui highlight utility in src/preview/highlight.rs — LazyLock<SyntaxSet> via two_face::syntax::extra_newlines(), LazyLock<EmbeddedLazyThemeSet> via two_face::theme::extra(), syntect_to_ratatui_style() (foreground RGB + font modifiers, skip background), highlight_lines(code, extension) -> Vec<Line<'static>>, auto_theme() via terminal-light luma detection
- [x] T010 Implement PreviewState in src/preview/state.rs — content, scroll_row, scroll_col, current_path, cancel_token, active_provider_index, available_providers; methods: request_preview(), set_content(), scroll_down/up/right/left with clamping, cycle_provider()

**Checkpoint**: Provider trait, Registry, Cache, Highlight, State が全てビルド・テストが通ること

---

## Phase 3: User Story 1 — テキストファイルのシンタックスハイライトプレビュー (Priority: P1) MVP

**Goal**: ファイル選択でプレビューペインにシンタックスハイライト付きテキストを表示

**Independent Test**: テストファイルを読み込み、ハイライト結果が正しく生成されることをユニットテストで検証

### Tests for User Story 1

- [x] T011 [P] [US1] Write tests for TextPreviewProvider in src/preview/providers/text.rs — can_handle: text file true, binary false, directory false; load: .rs file produces HighlightedText with language "Rust", unknown extension produces PlainText, empty file produces Empty, large file truncation

### Implementation for User Story 1

- [x] T012 [US1] Implement TextPreviewProvider in src/preview/providers/text.rs — can_handle: !is_dir && content_inspector text check (read first 1024 bytes), load: BufReader::lines() with max_lines/max_bytes dual limit, syntax detection via find_syntax_by_extension → find_syntax_by_token → find_syntax_plain_text, highlight_lines() for HighlightedText or PlainText for plain text, Empty for empty files, truncated flag
- [x] T013 [US1] Implement FallbackProvider in src/preview/providers/fallback.rs — can_handle: always true, is_enabled: available.len() <= 1, load: is_dir → Directory { entry_count, total_size }, binary → Binary { size }, else → Empty
- [x] T014 [US1] Implement preview_view widget in src/ui/preview_view.rs — render PreviewContent variants: HighlightedText (Line rendering with scroll offset), PlainText (string rendering), Loading ("Loading..." centered), Error (error message styled), Empty ("(empty)" centered), Binary ("Binary file (size)" centered), Directory (entry count + size info)
- [x] T015 [US1] Update UI layout in src/ui.rs — split main area into tree (50%) | preview (50%) when show_preview=true, full width tree when show_preview=false
- [x] T016 [US1] Integrate preview into AppState in src/app.rs — add preview_state, preview_cache, preview_registry fields; on cursor move, trigger preview load for current file; wire PreviewAction handling in event loop

**Checkpoint**: テキストファイル選択でシンタックスハイライト付きプレビューが表示されること

---

## Phase 4: User Story 2 — 非同期プレビュー読み込みとキャンセル (Priority: P1)

**Goal**: プレビュー読み込みを非同期化し、UI をブロックしない。キャンセル&キャッシュ対応

**Independent Test**: 大きなファイルの読み込み中にカーソル移動が可能なことを確認

### Tests for User Story 2

- [x] T017 [P] [US2] Write tests for PreviewCache in src/preview/cache.rs — put/get roundtrip, LRU eviction at capacity, different provider_name for same path stores separately

### Implementation for User Story 2

- [x] T018 [US2] Implement async preview loading in src/app.rs — on cursor move: cancel previous CancellationToken, set Loading state immediately, check cache (hit → set content and return), spawn tokio::spawn + spawn_blocking for provider.load(), send result via mpsc channel, event loop receives result → set_content + cache put
- [x] T019 [US2] Handle cancellation in preview load flow in src/app.rs — tokio::select! on cancel_token.cancelled() vs load completion, discard stale results (path mismatch check on receive)

**Checkpoint**: 大きなファイルでも UI がフリーズせず、ファイル切替で前の読み込みがキャンセルされること

---

## Phase 5: User Story 3 — プレビューのスクロール (Priority: P1)

**Goal**: プレビュー内容を縦横にスクロール可能にする

**Independent Test**: プレビュー表示状態でスクロールキーを押し、表示範囲が正しく変わることを確認

### Tests for User Story 3

- [x] T020 [P] [US3] Write tests for PreviewState scroll methods in src/preview/state.rs — scroll_down/up clamping to content bounds, scroll_right/left clamping, request_preview resets scroll to (0,0)

### Implementation for User Story 3

- [x] T021 [US3] Wire PreviewAction scroll events to key bindings in src/app.rs — handle_key_event: Shift+j/k for vertical scroll, Shift+h/l for horizontal scroll, Shift+D/U for half page scroll, Tab for provider cycling
- [x] T022 [US3] Update preview_view rendering to apply scroll offsets in src/ui/preview_view.rs — skip scroll_row lines from top, skip scroll_col characters from left for each line, show scroll indicators if content extends beyond viewport

**Checkpoint**: プレビューが縦横にスクロール可能で、ファイル切替でリセットされること

---

## Phase 6: User Story 4 — バイナリファイルの検出と表示 (Priority: P2)

**Goal**: バイナリファイルを自動検出し、テキスト表示ではなく情報メッセージを表示

**Independent Test**: バイナリファイル (.png on non-image terminal, .exe) を選択し、"Binary file (size)" が表示されること

### Tests for User Story 4

- [x] T023 [P] [US4] Write tests for binary detection in src/preview/providers/text.rs — can_handle returns false for binary content (NUL bytes), can_handle returns true for valid UTF-8 text

### Implementation for User Story 4

- [x] T024 [US4] Verify FallbackProvider handles binary display in src/preview/providers/fallback.rs — ensure Binary variant includes human-readable file size, verify preview_view renders Binary content correctly with centered message

**Checkpoint**: バイナリファイルが "Binary file (1.2 MB)" と表示され、文字化けしないこと

---

## Phase 7: User Story 5 — ディレクトリプレビュー (Priority: P2)

**Goal**: ディレクトリ選択時に概要情報を表示

**Independent Test**: ディレクトリを選択し、エントリ数が表示されること

### Tests for User Story 5

- [x] T025 [P] [US5] Write tests for FallbackProvider directory handling in src/preview/providers/fallback.rs — directory produces Directory content with correct entry_count, empty directory shows entry_count=0

### Implementation for User Story 5

- [x] T026 [US5] Implement directory info collection in FallbackProvider.load() in src/preview/providers/fallback.rs — read_dir to count entries and sum sizes, handle permission errors gracefully
- [x] T027 [US5] Update preview_view to render Directory content in src/ui/preview_view.rs — display entry count, total size, empty directory message

**Checkpoint**: ディレクトリ選択で子要素数・サイズが表示されること

---

## Phase 8: User Story 6 — プレビューモード切替 (Priority: P2)

**Goal**: 複数プロバイダーが利用可能な場合に Tab キーで切り替え

**Independent Test**: 複数プロバイダーがマッチするファイルで Tab を押して切り替えられること

### Tests for User Story 6

- [x] T028 [P] [US6] Write tests for PreviewRegistry.resolve() in src/preview/provider.rs — multiple providers matching, is_enabled filtering (Fallback excluded when others exist), priority ordering
- [x] T029 [P] [US6] Write tests for PreviewState.cycle_provider() in src/preview/state.rs — index wraps around, single provider does nothing

### Implementation for User Story 6

- [x] T030 [US6] Wire CycleProvider action (Tab key) in src/app.rs — on Tab: cycle_provider(), resolve new provider, load with new provider (check cache first), update available_providers list on file change
- [x] T031 [US6] Show current provider name in preview panel title in src/ui/preview_view.rs — display active provider name and index from PreviewState.available_providers

**Checkpoint**: SVG 等で Tab 切替が動作し、ステータスバーにプロバイダー名が表示されること

---

## Phase 9: User Story 7 — 画像プレビュー (Priority: P3)

**Goal**: 画像ファイルを端末のグラフィックスプロトコルで表示

**Independent Test**: 画像ファイルを選択し、対応端末でインラインプレビューが表示されること

### Implementation for User Story 7

- [x] T032 [US7] Initialize ratatui-image Picker in src/app.rs or src/main.rs — call Picker::from_query_stdio() before TUI init, fallback to Picker::halfblocks() on error, store Picker in AppState or global
- [x] T033 [US7] Implement ImagePreviewProvider in src/preview/providers/image.rs — can_handle: !is_dir && extension in [png, jpg, jpeg, gif, bmp, webp, svg, ico, tiff]; load: for non-SVG use image::open() → DynamicImage, for SVG use resvg::render() → tiny_skia::Pixmap → DynamicImage; then Picker.new_resize_protocol() → StatefulProtocol, return PreviewContent::Image; handle decode errors gracefully
- [x] T034 [US7] Update preview_view to render Image content in src/ui/preview_view.rs — use StatefulImage widget with render_stateful_widget, handle resize via ThreadProtocol if needed

**Checkpoint**: PNG/JPEG ファイルが Kitty/iTerm2/Sixel/Halfblocks で表示されること

---

## Phase 10: User Story 8 — 外部コマンドによるプレビュー (Priority: P3)

**Goal**: 設定で定義した外部コマンドの ANSI 出力をプレビュー表示

**Independent Test**: 外部コマンド設定後、対応ファイルで外部コマンド出力が表示されること

### Tests for User Story 8

- [x] T035 [P] [US8] Write tests for ExternalCmdProvider in src/preview/providers/external.rs — can_handle: extension match + command exists true, extension mismatch false, command not found false; load: successful command returns AnsiText

### Implementation for User Story 8

- [x] T036 [US8] Implement ExternalCmdProvider in src/preview/providers/external.rs — can_handle: !is_dir && extension in config.commands[].extensions && command exists in PATH (std::process::Command status check); load: tokio::process::Command with args, env TERM=xterm-256color, stdout piped, timeout (config.command_timeout), output limited to 1MB, ansi-to-tui IntoText conversion → AnsiText; handle timeout → Error, command failure → Error
- [x] T037 [US8] Update preview_view to render AnsiText content in src/ui/preview_view.rs — render Text<'static> from ansi-to-tui with scroll offsets applied

**Checkpoint**: 外部コマンド (例: column -t for CSV) の出力がカラー付きで表示されること

---

## Phase 11: Polish & Cross-Cutting Concerns

**Purpose**: 全 US にまたがる改善

- [x] T038 Delete empty src/highlight.rs (moved to src/preview/highlight.rs)
- [x] T039 [P] Performance test: verify highlight of 1000-line file completes within 50ms in src/preview/highlight.rs (use #[ignore] test)
- [x] T040 [P] Performance test: verify cache hit returns within 1ms in src/preview/cache.rs (use #[ignore] test)
- [x] T041 Edge case handling: permission denied, file deleted during load, FIFO/socket special files → Error variant in relevant providers
- [x] T042 Run `mise run lint` and fix all clippy warnings
- [x] T043 Run `mise run test` and verify all tests pass

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS all user stories
- **Phase 3-5 (US1-3, P1)**: Depend on Phase 2 — sequential (US1 provides base for US2/3)
- **Phase 6-8 (US4-6, P2)**: Depend on Phase 5 — can be parallel
- **Phase 9-10 (US7-8, P3)**: Depend on Phase 8 — can be parallel
- **Phase 11 (Polish)**: Depends on all user stories complete

### User Story Dependencies

- **US1 (Text Preview)**: Phase 2 → US1 (MVP, provides preview_view and app integration)
- **US2 (Async/Cancel)**: US1 → US2 (converts US1's sync load to async)
- **US3 (Scroll)**: US1 → US3 (adds scroll to existing preview)
- **US4 (Binary)**: US1 → US4 (binary detection is part of text can_handle, FallbackProvider already in US1)
- **US5 (Directory)**: US1 → US5 (extends FallbackProvider from US1)
- **US6 (Provider Switch)**: US1 → US6 (requires Registry.resolve() and multiple providers)
- **US7 (Image)**: US6 → US7 (new provider, benefits from provider switching)
- **US8 (External Cmd)**: US6 → US8 (new provider, benefits from provider switching)

### Within Each User Story

- Tests FIRST (TDD Red phase)
- Implementation (TDD Green phase)
- Verify at checkpoint

### Parallel Opportunities

**Phase 1**:
- T003, T004, T005 can run in parallel (different files)

**Phase 2**:
- T008, T009, T010 can run in parallel after T006/T007 (different files, but depend on trait definition)

**Phase 6-8 (P2 stories)**:
- US4, US5, US6 can run in parallel (different providers/features)

**Phase 9-10 (P3 stories)**:
- US7, US8 can run in parallel (different providers)

---

## Implementation Strategy

### MVP First (US1 + US2 + US3)

1. Phase 1: Setup (dependencies, module structure, types)
2. Phase 2: Foundational (trait, registry, cache, highlight, state)
3. Phase 3: US1 — Text preview with syntax highlighting
4. Phase 4: US2 — Async loading with cancel
5. Phase 5: US3 — Scroll support
6. **STOP and VALIDATE**: テキストファイルが非同期でハイライト・スクロール表示できること

### Incremental Delivery

1. MVP (US1+2+3) → テキストプレビューが動作
2. US4+5 → バイナリ/ディレクトリ対応
3. US6 → プロバイダー切替 UI
4. US7+8 → 画像/外部コマンド拡張

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story
- Constitution II: TDD — tests before implementation
- Strict clippy: no unwrap, no indexing, no print, docs on everything
- `content_inspector` for binary detection, not manual NUL check
- syntect background color is skipped (foreground + modifiers only)
- Fallback.is_enabled() = available.len() <= 1
