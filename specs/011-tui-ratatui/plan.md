# Implementation Plan: TUI 基盤

**Branch**: `011-tui-ratatui` | **Date**: 2026-02-10 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/011-tui-ratatui/spec.md`

## Summary

ratatui 0.30 + crossterm 0.29 + tokio でイベントループ・ツリー描画・キー入力を実装する。既存の TreeState/TreeBuilder/Sort を TUI に接続し、ターミナルでファイルツリーをナビゲーション可能にする。

## Technical Context

**Language/Version**: Rust 2024 edition, nightly-2026-01-24 toolchain
**Primary Dependencies**: ratatui 0.30, crossterm 0.29, tokio (full), devicons 0.6
**Storage**: N/A (ファイルシステム読み取りのみ)
**Testing**: cargo test (googletest, rstest, tempfile) + 目視確認
**Target Platform**: macOS / Linux ターミナル
**Project Type**: Single binary (TUI アプリケーション)
**Performance Goals**: 起動 < 1s, キー応答 < 16ms (60fps), 1k ファイルでスムーズナビゲーション
**Constraints**: unsafe 禁止, unwrap/expect 禁止, strict clippy
**Scale/Scope**: 10k エントリのツリーでスムーズ動作

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. 安全な Rust | PASS | unsafe 不使用。ratatui/crossterm は safe API |
| II. テスト駆動開発 | PASS | ロジック部分は TDD。UI 描画は目視 + スナップショット検討 |
| III. パフォーマンス設計 | PASS | 遅延読み込み済み (TreeState)。描画は visible_nodes 参照ベース |
| IV. シンプルさ & YAGNI | PASS | 最小限: イベントループ + ツリー描画 + キー入力のみ |
| V. インクリメンタルデリバリー | PASS | US1 (表示) → US2 (ナビ) → US3 (遅延) → US4 (リサイズ) → US5 (ステータス) |

## Project Structure

### Documentation (this feature)

```text
specs/011-tui-ratatui/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
└── tasks.md             # Phase 2 output (by /speckit.tasks)
```

### Source Code (repository root)

```text
src/
├── main.rs              # エントリーポイント (変更: tokio::main)
├── app.rs               # イベントループ・アプリケーションロジック (大幅書き換え)
├── cli.rs               # CLI args (変更: --no-icons 追加)
├── config.rs            # 設定 (既存)
├── action.rs            # Action enum (拡張)
├── state.rs             # mod state
│   └── tree.rs          # TreeState (既存)
├── tree.rs              # mod tree
│   ├── builder.rs       # TreeBuilder (既存)
│   ├── sort.rs          # sort (既存)
│   └── search_index.rs  # SearchIndex (既存)
├── terminal.rs          # ターミナル初期化・復帰・パニックフック (新規)
├── event.rs             # イベントハンドリング (新規)
└── ui/
    ├── tree_view.rs     # ツリー描画ウィジェット (新規実装)
    └── status_bar.rs    # ステータスバーウィジェット (新規実装)
```

**Structure Decision**: 既存の module 構成を維持し、`terminal.rs` (ターミナル管理) と `event.rs` (イベント処理) を追加。UI ウィジェットは `ui/` に配置。

## Architecture

### イベントループ

```
main() → app::run()
  ├── terminal::init()          # raw mode, alt screen, panic hook
  ├── TreeBuilder::build()      # 初回ツリー構築 (depth=1)
  ├── loop {
  │     ├── terminal.draw(ui)   # 描画
  │     ├── crossterm::event::poll()  # イベント待機
  │     ├── match event {
  │     │     Key('q') → break
  │     │     Key('j') → state.move_cursor(1)
  │     │     Key('k') → state.move_cursor(-1)
  │     │     Key('l') → expand_or_open()
  │     │     Key('h') → state.collapse()
  │     │     Enter → toggle_expand()
  │     │     ...
  │     │     Resize → continue (次フレームで再描画)
  │     │   }
  │     └── try_recv(children_rx)  # 非同期ロード結果受信
  │   }
  └── terminal::restore()       # ターミナル復帰
```

### 非同期ディレクトリ読み込み

```
expand_or_load() / toggle_expand():
  TreeState → Some(path)  // NeedsLoad
    → tokio::spawn_blocking(load_children)
    → children_tx.send(path, children)

メインループ:
  try_recv(children_rx)
    → state.set_children(path, children)
    → 次フレームで自動描画
```

### 描画レイアウト

```
┌─────────────────────────────────────────┐
│  tree_view (残り全域)                      │
│  ▸  src/                                │
│     main.rs                             │
│  ▸  tests/                              │
│     README.md                           │
├─────────────────────────────────────────┤
│  status_bar (1行)                        │
│  /Users/foo/project/src/main.rs  3/42   │
└─────────────────────────────────────────┘
```

### キーバインドマッピング

| Key | Action | TreeState method |
|-----|--------|-----------------|
| `j` / `↓` | カーソル下 | `move_cursor(1)` |
| `k` / `↑` | カーソル上 | `move_cursor(-1)` |
| `l` / `→` | 展開 / ファイルオープン | `expand_or_open()` |
| `h` / `←` | 折り畳み / 親移動 | `collapse()` |
| `Enter` | トグル（展開/折り畳み切替） | `toggle_expand()` |
| `g` | 先頭ジャンプ | `jump_to_first()` |
| `G` | 末尾ジャンプ | `jump_to_last()` |
| `Ctrl+d` | 半ページ下 | `half_page_down(height)` |
| `Ctrl+u` | 半ページ上 | `half_page_up(height)` |
| `q` | 終了 | — |

## Key Design Decisions

### D-001: イベントループは crossterm polling + tokio channel

**Decision**: `crossterm::event::poll()` ベースのメインループ + `tokio::sync::mpsc` で非同期タスク結果を受信

**Rationale**: polling は最もシンプルで debuggable。EventStream は複雑さが増すが、現段階では不要。非同期はチャネル経由で結合する。

**Alternatives rejected**:
- `EventStream` + `tokio::select!`: 将来的に IPC を追加する際に検討。現段階では YAGNI。

### D-002: ratatui::init() / ratatui::restore() でターミナル管理

**Decision**: ratatui 0.30 の `init()` / `restore()` ヘルパーを使用

**Rationale**: パニックフック、raw mode、alternate screen を自動設定。手動管理より安全・簡潔。

### D-003: tokio::spawn_blocking でディレクトリ読み込み

**Decision**: `TreeBuilder::load_children()` は同期関数のため、`tokio::spawn_blocking` でオフロード

**Rationale**: ignore::WalkBuilder は同期 API。tokio の blocking pool に逃がすのが自然。

### D-004: ファイルアイコンは devicons crate で Nerd Fonts 表示

**Decision**: devicons 0.6 でファイルタイプに応じたアイコンをデフォルト表示。`--no-icons` で無効化。

**Rationale**: Nerd Fonts は開発者の多くが導入済み。フォールバックはフラグで対応。

### D-005: main は #[tokio::main] に変更

**Decision**: `fn main()` → `#[tokio::main] async fn main()` に変更

**Rationale**: `tokio::spawn_blocking` に tokio runtime が必要。将来の IPC (010-ipc-neovim) でも必要。

## Complexity Tracking

> No violations — all decisions align with Constitution principles.
