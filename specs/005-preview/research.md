# Research: 005-preview

## R1: Syntax Highlighting (syntect + two-face → ratatui)

### Decision: syntect 5 + two-face 0.4 で行単位ハイライト

**Rationale**: syntect は Rust のシンタックスハイライトのデファクトスタンダード。two-face はビルトインの拡張構文セット (200+ 言語) とテーマセットを組み込みで提供。

**API フロー**:
1. `two_face::syntax::extra_newlines()` → `SyntaxSet` (`LazyLock` で一度だけ初期化)
2. `two_face::theme::extra()` → `EmbeddedLazyThemeSet` (`LazyLock`)
3. 拡張子 → `find_syntax_by_extension()` → `find_syntax_by_token()` → `find_syntax_plain_text()`
4. `HighlightLines::new(syntax, theme)` → `highlight_line()` → `Vec<(Style, &str)>`
5. syntect Style → ratatui Style 変換 (foreground のみ、background は端末に委ねる)

**スタイル変換**: `Color::Rgb(fg.r, fg.g, fg.b)` + `FontStyle::BOLD/ITALIC/UNDERLINE` → `Modifier`

---

## R2: テーマ自動選択

### Decision: `terminal-light` で端末背景検出 → Base16OceanDark / Light 自動切替

`terminal_light::luma() > 0.6` → light, else dark。検出失敗時は dark にフォールバック。

**Cargo.toml**: `terminal-light = "1"`

---

## R3: バイナリファイル検出

### Decision: `content_inspector` crate (v0.4)

`bat`, `helix`, `typos` で使用。先頭 1024 バイトで NUL + BOM + マジックナンバー検出。

**Cargo.toml**: `content_inspector = "0.4"`

---

## R4: LRU キャッシュ

### Decision: `lru` crate (v0.12)

キー: `CacheKey { path: PathBuf, provider_name: String }`, 容量: 10。

**Cargo.toml**: `lru = "0.12"`

---

## R5: CancellationToken

### Decision: `tokio_util::sync::CancellationToken` (既に依存済み)

Cancel-and-Replace: `old_token.cancel()` → 新 token → `tokio::select!` で `cancelled()` リッスン。

---

## R6: 大きなファイル

### Decision: 同期 `BufReader::lines()` + `spawn_blocking` + デュアルリミット (1000行 + 10MB)

syntect が同期 API のため `spawn_blocking` が自然。

---

## R7: 画像プレビュー (ratatui-image)

### Decision: `ratatui-image` crate

**初期化**: `Picker::from_query_stdio()` (TUI 開始前に呼ぶ必要あり) → `Picker::halfblocks()` フォールバック

**プロトコル自動検出**: Kitty, Sixel, iTerm2, Halfblocks

**描画**: `StatefulImage` + `StatefulProtocol` (render_stateful_widget)

**非同期**: `ThreadProtocol` で resize+encode をバックグラウンドスレッドにオフロード。ファイル I/O は自前で非同期化する必要あり。

**Cargo.toml**: `ratatui-image = "5"`, `image = "0.25"`

---

## R8: ANSI テキスト変換 (ansi-to-tui)

### Decision: `ansi-to-tui` crate

`IntoText` トレイト: `bytes.into_text()` → `Result<Text<'static>>`. 8/16/256/TrueColor 対応。

**Cargo.toml**: `ansi-to-tui = "7"`

---

## R9: 外部コマンドプレビュー

### Decision: `tokio::process::Command` + タイムアウト + ansi-to-tui

**実行パターン**:
```rust
let output = tokio::time::timeout(
    Duration::from_secs(timeout),
    tokio::process::Command::new(&cmd)
        .args(&args)
        .env("TERM", "xterm-256color")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
).await;
```

**設定フォーマット** (config.toml):
```toml
[[preview.commands]]
extensions = ["csv"]
command = "column"
args = ["-s", ",", "-t"]

[[preview.commands]]
extensions = ["md"]
command = "glow"
args = ["-s", "dark"]
```

**can_handle()**: 拡張子ベースで判定。コマンド存在チェックは `which` crate または `std::process::Command` で確認。

**出力上限**: stdout を最大 1MB に制限 (無限出力対策)

---

## R10: SVG ラスタライズ (resvg)

### Decision: `resvg` crate を常時依存として追加

**Rationale**: SVG を画像としてプレビューするため。`image` crate は SVG 非対応のため resvg が必要。

**パフォーマンス**: アイコン 100ms 未満、複雑な SVG 1-3 秒。TUI プレビューに十分高速。

**依存コスト**: +35 クレート、クリーンビルド +9 秒。常時依存として受け入れ。

**フロー**: SVG → `resvg::render()` → `tiny_skia::Pixmap` → `image::DynamicImage` → `ratatui-image`

**Cargo.toml**: `resvg = "0.46"`

---

## R11: Cargo.toml 追加依存まとめ

```toml
# Binary detection
content_inspector = "0.4"

# LRU cache
lru = "0.12"

# Terminal background detection
terminal-light = "1"

# Image preview
ratatui-image = "5"
image = "0.25"

# SVG rasterization
resvg = "0.46"

# ANSI text conversion (external command output)
ansi-to-tui = "7"
```
