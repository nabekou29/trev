# Implementation Plan: ファイルプレビュー

**Branch**: `005-preview` | **Date**: 2026-02-12 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/005-preview/spec.md`

## Summary

ファイル選択時にプレビューペインにシンタックスハイライト付きの内容を非同期で表示する。プロバイダーパターンで拡張可能な設計とし、テキスト (syntect)、画像 (ratatui-image)、外部コマンド (ansi-to-tui)、バイナリ/ディレクトリのフォールバックを統一インターフェースで切り替え可能にする。

## Technical Context

**Language/Version**: Rust 2024 edition, nightly-2026-01-24
**Primary Dependencies**: ratatui 0.30, syntect 5, two-face 0.4, ratatui-image 5, ansi-to-tui 7, content_inspector 0.4, lru 0.12, terminal-light 1
**Storage**: N/A (ファイルシステム読み取りのみ)
**Testing**: rstest + googletest (in-source tests), tempfile
**Target Platform**: macOS / Linux 端末 (Kitty, iTerm2, Sixel 対応端末, Halfblocks フォールバック)
**Project Type**: Single Rust binary
**Performance Goals**: プレビュー切替 16ms 以内 (Loading 表示), ハイライト 1000 行 50ms 以内, キャッシュヒット 1ms 以内
**Constraints**: メモリ上限 LRU 10 エントリ, 大ファイル 1000 行 + 10MB デュアルリミット
**Scale/Scope**: 一般的な開発プロジェクト (数千〜数万ファイル)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. 安全な Rust | PASS | unsafe なし、anyhow/thiserror でエラー処理 |
| II. テスト駆動開発 | PASS | TDD で実装。各プロバイダーにユニットテスト |
| III. パフォーマンス設計 | PASS | 非同期読み込み、キャッシュ、spawn_blocking |
| IV. シンプルさ & YAGNI | PASS | Provider トレイトは最小限。最初から全 US を実装するが、各プロバイダーはシンプル |
| V. インクリメンタルデリバリー | PASS | データ構造 → ハイライター → 非同期 → UI → プロバイダー |

## Project Structure

### Documentation (this feature)

```text
specs/005-preview/
├── plan.md              # This file
├── research.md          # Phase 0 output (complete)
├── data-model.md        # Phase 1 output
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
src/
├── preview.rs           # pub mod — サブモジュール宣言
├── preview/
│   ├── content.rs       # PreviewContent enum
│   ├── state.rs         # PreviewState (scroll, cancel, active provider)
│   ├── cache.rs         # PreviewCache (LRU)
│   ├── provider.rs      # PreviewProvider trait + PreviewRegistry
│   ├── providers/
│   │   ├── text.rs      # TextPreviewProvider (syntect)
│   │   ├── image.rs     # ImagePreviewProvider (ratatui-image)
│   │   ├── external.rs  # ExternalCmdProvider (ansi-to-tui)
│   │   └── fallback.rs  # FallbackProvider (binary/dir/empty/error)
│   └── highlight.rs     # syntect → ratatui 変換ユーティリティ
├── highlight.rs         # → 削除 (preview/highlight.rs に移動)
├── config.rs            # PreviewConfig 追加 (max_lines, cache_size, commands)
├── state.rs             # pub mod preview 追加
├── state/
│   ├── tree.rs          # 既存
│   └── preview.rs       # → preview/state.rs からの re-export or 直接配置
├── action.rs            # PreviewAction 追加
├── app.rs               # プレビュー非同期読み込み統合
├── ui.rs                # レイアウト分割 (tree | preview)
└── ui/
    ├── preview_view.rs  # プレビュー描画ウィジェット
    └── ...
```

**Structure Decision**: 既存の Rust モジュール構造を拡張。`preview/` サブモジュールに provider パターンを配置。`preview.rs` がモジュールルートで `pub mod` 宣言。

## Design Notes

### Provider パターン

```rust
pub trait PreviewProvider: Send + Sync {
    fn name(&self) -> &str;
    fn priority(&self) -> u32;  // 低い数値 = 高い優先度
    fn can_handle(&self, path: &Path, is_dir: bool) -> bool;
    /// 有効なプロバイダー名一覧を見て、自身を切替候補に含めるか判定。
    /// デフォルトは true (常に有効)。Fallback 等がオーバーライド。
    fn is_enabled(&self, available: &[&str]) -> bool { true }
    fn load(&self, path: &Path, ctx: &LoadContext) -> anyhow::Result<PreviewContent>;
}
```

`PreviewRegistry` は `Vec<Box<dyn PreviewProvider>>` を優先度順に保持。

**プロバイダー選択フロー**:
1. 優先度順に全プロバイダーの `can_handle()` を評価 → 候補リスト
2. 候補の名前一覧を構築
3. 各候補の `is_enabled(&names)` でフィルタ → 最終切替リスト
4. 最終リストの先頭 (最高優先度) がデフォルトプロバイダー
5. `Tab` キーでリスト内の次のプロバイダーへ切替 (wrap-around)

### 各プロバイダーの判定ロジック

| Provider | Priority | can_handle() | is_enabled() |
|----------|----------|-------------|-------------|
| External | 10 | `!is_dir` かつ拡張子が設定の `extensions` に一致、かつコマンドが `$PATH` に存在 | デフォルト `true` |
| Image | 20 | `!is_dir` かつ拡張子が画像形式 (png, jpg, jpeg, gif, bmp, webp, svg, ico, tiff) | デフォルト `true` |
| Text | 30 | `!is_dir` かつ先頭 1024 バイトが `content_inspector` でテキスト判定 | デフォルト `true` |
| Fallback | 100 | 常に `true` | `available.len() <= 1` (自分しかいなければ有効) |

**結果例**:
| ファイル | can_handle 候補 | is_enabled 後 |
|---------|---------------|--------------|
| `.rs` | [Text, Fallback] | [Text] |
| `.svg` | [Image, Text, Fallback] | [Image, Text] |
| `.csv` (external設定あり) | [External, Text, Fallback] | [External, Text] |
| `.png` | [Image, Fallback] | [Image] |
| `.zip` (バイナリ) | [Fallback] | [Fallback] |
| ディレクトリ | [Fallback] | [Fallback] |

**Fallback の表示内容**: ディレクトリは子要素数・合計サイズ・最終更新日 (US5)、バイナリはファイルサイズ・MIME タイプ表示。

**テキスト判定**: `content_inspector::inspect()` で先頭 1024 バイトを検査。syntect は `find_syntax_by_extension()` → `find_syntax_by_token()` → `find_syntax_plain_text()` のチェーンで未知拡張子もプレーンテキストとしてハイライトなし表示。

### 非同期フロー

1. カーソル移動 → `PreviewState::request_preview(path)`
2. 前の `CancellationToken` をキャンセル
3. `PreviewContent::Loading` を即座に設定
4. キャッシュチェック → ヒットなら即座に設定して終了
5. `tokio::spawn` + `spawn_blocking` でプロバイダーの `load()` を実行
6. 結果を mpsc チャネルで event loop に送信
7. event loop で `PreviewState::set_content()` + キャッシュに格納

### UI レイアウト

```
┌─────────────────┬─────────────────┐
│  Tree View      │  Preview View   │
│  (50%)          │  (50%)          │
│                 │                 │
├─────────────────┴─────────────────┤
│ Status Bar                        │
└───────────────────────────────────┘
```

`show_preview` 設定が false の場合はツリーのみ全幅表示。

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Provider trait (抽象化) | テキスト/画像/外部コマンド/フォールバックの4種を統一的に切り替えるため | if-else チェーンはプロバイダー追加のたびに肥大化する |
