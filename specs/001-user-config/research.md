# Research: 001-user-config

**Date**: 2026-02-12

## スコープ決定

US1 (TOML 静的設定) と US3 (CLI 上書き) を先行実装する。
US2 (Lua 動的設定) とカスタムソートは将来フェーズとして保留。

---

## R1: 未知キー検出

### Decision: serde_ignored クレート

### Rationale

- 目的に完全一致: 未知フィールドを warn (error ではなく)
- コード量最小 (~10 行)
- ネスト構造・`#[serde(default)]` と自動連携

### Implementation sketch

```rust
let mut unknown_keys = Vec::new();
let deserializer = &mut toml::Deserializer::new(&content);
let config: Config = serde_ignored::deserialize(deserializer, |path| {
    unknown_keys.push(path.to_string());
})?;
for key in &unknown_keys {
    tracing::warn!("Unknown config key '{key}' in {}", path.display());
}
```

### Alternatives Considered

- `deny_unknown_fields`: エラーになる (要件は警告)
- `toml::Value` 手動比較: 150+ 行、メンテ負担大

---

## R2: 設定ファイルパス

### Decision: 明示的に XDG 準拠パスを実装

### Rationale

`directories` クレートの `ProjectDirs` は macOS で `~/Library/Application Support/trev/` を返す。
spec FR-001 は `$XDG_CONFIG_HOME/trev/` → `~/.config/trev/` を要求。

### Implementation

```rust
fn config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("trev")
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config/trev")
    }
}
```

`directories` クレートは不要になるため削除可能。`dirs` クレート (軽量) で `home_dir()` のみ使用。

---

## R3: エラーメッセージ品質

### Decision: toml エラー + anyhow context

`toml::de::Error` は行番号・列番号を含む。`anyhow::Context` でファイルパスを付加。

---

## R4: Config マージ戦略

### Decision: Default → TOML → CLI の 3 段階上書き

1. `Config::default()` — 全フィールドにデフォルト値
2. TOML `config.toml` — `#[serde(default)]` で未設定はデフォルト維持
3. CLI 引数 — `--show-hidden` 等。指定された項目のみ上書き

将来 Lua 層を追加する場合は TOML と CLI の間に挿入する。

---

## R5: Lua ランタイム (将来フェーズ用メモ)

### 選定: mlua (Lua 5.4, vendored)

- `mlua = { version = "0.11", features = ["lua54", "vendored"] }`
- Timeout: `set_hook` + `Instant` で 3 秒制限
- Sandbox: `io`, `os`, `loadfile`, `dofile` を除去
- カスタムソート: sort_key 方式 (Schwartzian transform) で Lua 呼び出しを O(n) に抑える
  - `trev.sort.set_key(function(node) return prefix .. name end)`
  - compare 方式 (`set_compare`) もオプションで提供
