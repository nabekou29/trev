# Implementation Plan: FS Change Detection (Phase 9)

**Branch**: `012-file-operations` | **Date**: 2026-02-16 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification — User Story 5 (FS 変更検出)

## Summary

展開中のディレクトリをファイルシステム監視し、外部からの変更を自動的にツリーに反映する。
`notify` + `notify-debouncer-mini` で 250ms デバウンスの NonRecursive 監視を行い、
trev 自身のファイル操作中はイベントを抑制して重複更新を防ぐ。

## Technical Context

**Language/Version**: Rust 2024 edition, nightly-2026-01-24
**Primary Dependencies**: notify 8, notify-debouncer-mini 0.5, tokio (既存)
**Storage**: N/A
**Testing**: cargo test (googletest + rstest + tempfile)
**Target Platform**: macOS / Linux
**Project Type**: Single Rust binary (TUI)
**Performance Goals**: 外部変更が 250ms デバウンス後にツリーに反映 (AS-1)
**Constraints**: 自己操作の重複更新を回避 (AS-3)
**Scale/Scope**: 展開中ディレクトリ数に比例（通常数十件以下）

## Constitution Check

| Principle | Status | Notes |
|---|---|---|
| I. Safe Rust | ✅ | Arc + AtomicBool で安全な共有状態 |
| II. TDD | ✅ | FsWatcher のユニットテスト先行 |
| III. Performance | ✅ | NonRecursive で展開ディレクトリのみ監視、デバウンスで過剰更新を防止 |
| IV. YAGNI | ✅ | AtomicBool 抑制（muted path set より単純）、Stale 状態は不要（NotLoaded で十分） |
| V. Incremental | ✅ | Watcher 型 → 初期化 → イベント処理 → 動的 watch/unwatch → 抑制 の順 |

## Data Model

### FsWatcher

```rust
/// File system change watcher with debouncing and self-operation suppression.
pub struct FsWatcher {
    /// Debounced file watcher (kept alive for the watcher lifetime).
    debouncer: Debouncer<RecommendedWatcher>,
    /// Set of currently watched directory paths.
    watched_dirs: HashSet<PathBuf>,
    /// Flag to suppress events during self-initiated file operations.
    suppressed: Arc<AtomicBool>,
}
```

### メソッド

| メソッド | 用途 |
|---|---|
| `new(config, tx)` | WatcherConfig からデバウンサーを生成、イベントを `tx` に送信 |
| `watch(path)` | ディレクトリを NonRecursive で監視開始 |
| `unwatch(path)` | ディレクトリの監視を停止 |
| `suppressed()` | `Arc<AtomicBool>` のクローンを返す（ハンドラで共有用） |

### イベント処理フロー

```text
notify → debouncer (250ms) → UnboundedSender → event loop
  ↓
  affected_dirs = events.map(|e| e.path.parent()).dedup()
  ↓
  for each dir:
    if expanded → refresh_directory() (re-read children)
    if collapsed + loaded → invalidate_children() (mark as NotLoaded on next expand)
```

### 自己操作抑制

```text
file op handler:
  suppressed.store(true)
  execute ops...
  refresh_directory() (trev 自身で更新)
  suppressed.store(false)

watcher event handler:
  if suppressed.load() → skip all events
```

### 動的 watch/unwatch タイミング

| イベント | アクション |
|---|---|
| ディレクトリ展開 (expand) | `watcher.watch(dir)` |
| ディレクトリ折り畳み (collapse) | `watcher.unwatch(dir)` |
| 起動時 | root + 初期展開ディレクトリを watch |
| 終了時 | FsWatcher drop で自動停止 |

## Project Structure

```text
src/
├── watcher.rs          # FsWatcher (実装対象)
├── app.rs              # watcher 初期化 + イベントループ統合
├── app/
│   ├── state.rs        # AppContext に suppressed 追加
│   └── handler/
│       ├── tree.rs     # expand/collapse で watch/unwatch
│       └── file_op.rs  # 操作前後で suppress/unsuppress
└── state/
    └── tree.rs         # invalidate_children() (既存)
```

## Implementation Steps

### Step 1: `src/watcher.rs` — FsWatcher 型 + テスト

- `FsWatcher` 構造体: `Debouncer`, `watched_dirs: HashSet`, `suppressed: Arc<AtomicBool>`
- `new(config: &WatcherConfig, tx: UnboundedSender)` — デバウンサー生成
- `watch(path)` / `unwatch(path)` — NonRecursive 監視の追加/削除
- `suppressed()` — Arc clone 取得
- テスト: watch/unwatch の追跡、二重 watch の冪等性

### Step 2: `src/app/state.rs` — AppContext に suppressed 追加

- `pub suppressed: Arc<AtomicBool>` フィールドを AppContext に追加
- ファイル操作ハンドラと watcher イベントハンドラの両方から参照可能に

### Step 3: `src/app.rs` — Watcher 初期化 + イベントループ統合

- `tokio::sync::mpsc::unbounded_channel` で watcher イベントチャネル作成
- `FsWatcher::new(&config.watcher, watcher_tx)` で watcher 生成
- root ディレクトリを初期 watch
- イベントループ内で `watcher_rx.try_recv()` → 影響ディレクトリ特定 → refresh/invalidate
- `config.watcher.enabled == false` の場合は watcher 生成をスキップ

### Step 4: `src/app/handler/tree.rs` — expand/collapse で watch/unwatch

- expand 時: watch 対象に追加
- collapse 時: watch 対象から除外
- FsWatcher への参照は `Option<&mut FsWatcher>` で渡す（enabled=false 時は None）

### Step 5: `src/app/handler/file_op.rs` — 自己操作抑制

- `execute_paste`, `execute_create`, `execute_rename`, `execute_delete` の前後で
  `suppressed.store(true/false, Ordering::SeqCst)`
- `execute_undo`, `execute_redo` も同様

### Step 6: Watcher イベントから refresh/invalidate ロジック

- 受信した `DebouncedEvent` のパス群から親ディレクトリを抽出
- 展開中 (expanded + loaded) なら `refresh_directory()`
- 折り畳み中 (collapsed + loaded) なら `invalidate_children()` で NotLoaded に戻す

### Step 7: テスト + lint + format

## Verification

```bash
mise run test
mise run lint
mise run format
```
