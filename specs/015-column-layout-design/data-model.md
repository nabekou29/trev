# Data Model: Column Layout Design

## Entity Changes

### TreeNode (既存 — 変更)

**File**: `src/state/tree.rs`

```rust
pub struct TreeNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub children: ChildrenState,
    pub is_expanded: bool,
    pub symlink_target: Option<String>,  // NEW
}
```

| Field | Type | 変更 | 説明 |
|-------|------|------|------|
| `symlink_target` | `Option<String>` | NEW | `std::fs::read_link` の結果。`is_symlink == true` かつ read_link 成功時のみ `Some`。表示用に `to_string_lossy()` で変換 |

**Validation**: `symlink_target` は `is_symlink == true` の場合のみ `Some` を持ちうる。`is_symlink == false` の場合は常に `None`。

## New Entities

### ColumnKind (新規)

**File**: `src/ui/column.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ColumnKind {
    Size,
    ModifiedAt,
    GitStatus,
}
```

| Variant | 表示幅 | フォーマット | ディレクトリ |
|---------|--------|-------------|-------------|
| `Size` | 5文字 | 人間可読 (B/K/M/G/T) 右寄せ | `   -` |
| `ModifiedAt` | 10文字 | ティア形式 右寄せ（relative）/ ISO 形式（absolute） | 通常表示 |
| `GitStatus` | 2文字 | ステータス文字 + スペース | 集約ステータス |

### MtimeFormat (新規)

**File**: `src/ui/column.rs`

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MtimeFormat {
    #[default]
    Relative,
    Absolute,
}
```

| Variant | 説明 |
|---------|------|
| `Relative` | デフォルト。ティア形式（`Xm ago`, `Xh ago`, `Xd ago`, `Xw ago`, ISO） |
| `Absolute` | 常に ISO 形式（`YYYY-MM-DD`） |

### ColumnEntry (新規)

**File**: `src/ui/column.rs`

YAML の各エントリを表す。文字列形式（`- size`）とオブジェクト形式（`- kind: modified_at\n  format: absolute`）の両方に対応。

```rust
/// YAML 上の1カラムエントリ。serde untagged で文字列/オブジェクト両対応。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ColumnEntry {
    Simple(ColumnKind),
    WithOptions {
        kind: ColumnKind,
        #[serde(default)]
        format: Option<MtimeFormat>,
    },
}
```

`ColumnEntry` は `kind()` メソッドで `ColumnKind` を、`mtime_format()` メソッドで `MtimeFormat` を返す（`ModifiedAt` 以外は `format` を無視）。

### ResolvedColumn (新規)

**File**: `src/ui/column.rs`

実行時に使用する解決済みカラム情報。config の `ColumnEntry` リストから生成。

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedColumn {
    pub kind: ColumnKind,
    pub mtime_format: MtimeFormat,
}
```

## Config Entities

### ColumnConfig (DisplayConfig 内)

**File**: `src/config.rs`

`DisplayConfig` に追加:

```rust
pub struct DisplayConfig {
    pub show_hidden: bool,
    pub show_ignored: bool,
    pub show_preview: bool,
    pub show_root: bool,
    pub columns: Vec<ColumnEntry>,  // NEW
}
```

| Field | Type | Default | 説明 |
|-------|------|---------|------|
| `columns` | `Vec<ColumnEntry>` | `[Size, ModifiedAt, GitStatus]`（全て Simple 形式） | 表示するカラムとその順序。文字列/オブジェクト混在可 |

### LayoutConfig (PreviewConfig 内)

**File**: `src/config.rs`

`PreviewConfig` に追加:

```rust
pub struct PreviewConfig {
    // ... 既存フィールド ...
    pub split: u16,              // NEW
    pub narrow_split: u16,       // NEW
    pub narrow_threshold: u16,   // NEW
}
```

| Field | Type | Default | 説明 |
|-------|------|---------|------|
| `split` | `u16` | `50` | Wide モードのツリー幅パーセンテージ |
| `narrow_split` | `u16` | `60` | Narrow モードのツリー幅パーセンテージ |
| `narrow_threshold` | `u16` | `80` | 横/縦切替のブレイクポイント（列数） |

## State Entities

### AppState 変更

**File**: `src/app/state.rs`

```rust
pub struct AppState {
    // ... 既存フィールド ...
    pub columns: Vec<ResolvedColumn>,  // NEW: 実行時のカラムリスト（git_enabled で filter 済み、オプション解決済み）
}
```

### AppContext 変更

**File**: `src/app/state.rs`

```rust
pub struct AppContext {
    // ... 既存フィールド ...
    pub layout_split: u16,           // NEW
    pub layout_narrow_split: u16,    // NEW
    pub layout_narrow_threshold: u16, // NEW
}
```

レイアウト設定は実行中に変化しないため `AppContext`（不変）に配置。

## Relationships

```
Config
├── DisplayConfig
│   └── columns: Vec<ColumnEntry>  →  AppState.columns: Vec<ResolvedColumn> (resolve + git_enabled フィルタ)
└── PreviewConfig
    ├── split              →  AppContext.layout_split
    ├── narrow_split       →  AppContext.layout_narrow_split
    └── narrow_threshold   →  AppContext.layout_narrow_threshold

ColumnEntry (config 形式)
├── Simple(ColumnKind)              →  ResolvedColumn { kind, mtime_format: default }
└── WithOptions { kind, format }    →  ResolvedColumn { kind, mtime_format }

TreeNode
├── size: u64           →  ColumnKind::Size でフォーマット
├── modified: Option<SystemTime>  →  ColumnKind::ModifiedAt でフォーマット (MtimeFormat に応じて)
├── is_symlink: bool    →  名前表示に "→ target" 追加
└── symlink_target: Option<String>  (NEW)

GitState
└── file_status() / dir_status()  →  ColumnKind::GitStatus でフォーマット
```
