# Data Model: 001-user-config

## Entities

### Config (既存、拡張)

アプリケーション全体の設定。Default → TOML → CLI の順で上書きマージされる。

| Field | Type | Default | Source |
|-------|------|---------|--------|
| sort | SortConfig | (below) | TOML, CLI |
| display | DisplayConfig | (below) | TOML, CLI |
| preview | PreviewConfig | (below) | TOML |

### SortConfig (既存)

| Field | Type | Default | CLI flag |
|-------|------|---------|----------|
| order | SortOrder | Name | `--sort-order` |
| direction | SortDirection | Asc | `--sort-direction` |
| directories_first | bool | true | `--no-directories-first` |

### DisplayConfig (既存)

| Field | Type | Default | CLI flag |
|-------|------|---------|----------|
| show_hidden | bool | false | `-a` / `--show-hidden` (既存) |
| show_ignored | bool | false | `--show-ignored` |
| show_preview | bool | true | `--no-preview` |

### PreviewConfig (既存)

| Field | Type | Default | CLI flag |
|-------|------|---------|----------|
| max_lines | usize | 1000 | — |
| max_bytes | u64 | 10MB | — |
| cache_size | usize | 10 | — |
| commands | Vec\<ExternalCommand\> | [] | — |
| command_timeout | u64 | 3 | — |

### ConfigLoadResult (新規)

設定読み込みの結果。警告がある場合も正常に config を返す。

| Field | Type | Description |
|-------|------|-------------|
| config | Config | マージ済み設定 |
| warnings | Vec\<String\> | 未知キー、軽微なエラー等 |

## Relationships

```
Config::load()
  → config_dir() で XDG パスを解決
  → TOML ファイルを serde_ignored でデシリアライズ
  → 未知キーを warnings に収集
  → ConfigLoadResult を返す

Config::apply_cli_overrides(&mut self, args)
  → CLI 引数で指定された値のみ上書き
```
