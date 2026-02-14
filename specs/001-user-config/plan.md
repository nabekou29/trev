# Implementation Plan: TOML ユーザー設定

**Branch**: `001-user-config` | **Date**: 2026-02-12 | **Spec**: `specs/001-user-config/spec.md`

## Summary

TOML ファイルによるユーザー設定と CLI 引数による上書き機能を実装する。
設定ファイルの自動検出 (XDG)、未知キーの警告、構文エラーの丁寧な報告を含む。
Lua による動的設定 (US2) は将来フェーズとして保留。

## Scope

- **IN**: US1 (TOML 静的設定), US3 (CLI 引数上書き)
- **OUT**: US2 (Lua 動的設定), カスタムソート, テーマ, キーマップ (将来フェーズ)

## Technical Context

**Language/Version**: Rust 2024 edition, nightly-2026-01-24
**Primary Dependencies**: toml (既存), serde (既存), serde_ignored (新規), dirs (新規, `directories` を置換)
**Storage**: TOML ファイル (`$XDG_CONFIG_HOME/trev/config.toml` or `~/.config/trev/config.toml`)
**Testing**: cargo test, rstest, googletest, tempfile
**Target Platform**: macOS (darwin), Linux
**Performance Goals**: 設定読み込み < 50ms (SC-003)
**Constraints**: 未知キーでクラッシュしない (SC-005), エラーにファイル名+行番号含む (SC-002)

## Constitution Check

| Gate | Status | Notes |
|------|--------|-------|
| I. 安全な Rust | PASS | unsafe 不使用, anyhow でエラー処理 |
| II. TDD | PASS | テスト先行で実装 |
| III. パフォーマンス | PASS | 設定読み込みは起動時 1 回のみ |
| IV. シンプルさ & YAGNI | PASS | Lua は将来フェーズに保留 |
| V. インクリメンタル | PASS | TOML → CLI 上書きの順で段階実装 |

## Project Structure

### Source Code

```text
src/
├── config.rs              # Config struct, load/merge logic (既存を拡張)
├── cli.rs                 # CLI args (既存を拡張, config override)
├── app.rs                 # Config 適用 (既存を修正)
└── main.rs                # エラー表示 (既存を修正)
```

### Config File

```toml
# ~/.config/trev/config.toml

[sort]
order = "name"           # name | size | mtime | type | extension
direction = "asc"        # asc | desc
directories_first = true

[display]
show_hidden = false
show_ignored = false
show_preview = true

[preview]
max_lines = 1000
max_bytes = 10485760     # 10 MB
cache_size = 10
command_timeout = 3

[[preview.commands]]
extensions = ["json"]
command = "jq"
args = ["."]

[[preview.commands]]
extensions = ["md"]
command = "bat"
args = ["--color=always", "--style=plain"]
```

## Implementation Strategy

### Phase 1: Config 読み込みの堅牢化

1. `directories` → `dirs` クレートへ移行、XDG パス解決を明示実装
2. `serde_ignored` で未知キー警告
3. TOML パースエラーにファイルパス付加
4. 設定ファイル不在時のデフォルト動作を維持

### Phase 2: CLI 引数による上書き

1. `Args` に設定上書き用フィールドを追加 (`--sort-order`, `--sort-direction` 等)
2. `Config::apply_cli_overrides(&mut self, args: &Args)` で CLI 値を適用
3. 優先順位: CLI > TOML > Default

### Phase 3: main.rs のエラー表示改善

1. 設定エラー時: エラーメッセージ表示 + デフォルト設定で起動
2. 致命的エラー時: メッセージ表示 + 終了

## Dependencies

### New

- `serde_ignored = "0.1"` — 未知キー検出
- `dirs = "6"` — ホームディレクトリ取得 (軽量)

### Remove

- `directories` — `dirs` に置換 (より軽量、必要な機能は `home_dir()` のみ)

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| 未知キー検出 | serde_ignored | warn (not error), 最小コード |
| XDG パス | 明示実装 | macOS で `~/Library/...` ではなく `~/.config/` を使用 |
| Lua | 将来フェーズ | YAGNI — TOML で十分な範囲を先に固める |
| Config マージ | 段階上書き | Default → TOML → CLI の 3 段階 |
