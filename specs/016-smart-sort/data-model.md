# Data Model: Smart Sort

## Entities

### SortOrder (既存 enum の拡張)

| Variant   | Description                              | Config value  |
|-----------|------------------------------------------|---------------|
| Smart     | Natural sort + 接尾辞グルーピング (NEW, default) | `smart`       |
| Name      | 単純な名前順（case-insensitive）          | `name`        |
| Size      | ファイルサイズ順                          | `size`        |
| Modified  | 更新日時順                                | `mtime`       |
| Type      | ファイル種別（dir/file）→ 名前順          | `type`        |
| Extension | 拡張子順 → 名前順                         | `extension`   |

2箇所に存在:
- `config::SortOrder` — serde, JsonSchema, clap::ValueEnum derive
- `state::tree::SortOrder` — serde derive、`From<config::SortOrder>` 変換

### MenuAction (新規 enum)

| Variant          | Description                         |
|------------------|-------------------------------------|
| CopyToClipboard  | クリップボードにコピー（既存動作）  |
| SelectSortOrder  | ソート方式を選択して適用            |

`MenuState.on_select` フィールドとして保持。

### DecomposedName (sort 内部の概念)

| Field  | Type            | Description                           |
|--------|-----------------|---------------------------------------|
| base   | &str            | ベース名（グルーピングキー）          |
| suffix | Option<&str>    | 接尾辞（`.test.ts`, `_test.go` 等）   |

ライフタイムは sort コンパレータ内のローカル。構造体にする必要はなくタプルで十分。

### TreeAction (既存 enum の拡張)

| Variant              | Description                        | Action string            |
|----------------------|------------------------------------|--------------------------|
| SortMenu             | ソート選択メニューを開く (NEW)     | `tree.sort_menu`         |
| ToggleSortDirection  | ソート方向を切り替え (NEW)         | `tree.toggle_sort_direction` |

## Relationships

```
Config          →  TreeState.options.sort_order
                →  TreeState.options.sort_direction

S key           →  TreeAction::SortMenu
                →  AppMode::Menu(sort items, MenuAction::SelectSortOrder)
                →  handle_menu: apply_sort → re-render

s key           →  TreeAction::ToggleSortDirection
                →  apply_sort with reversed direction → re-render

sort_children   ←  SortOrder::Smart
                ←  decompose_name → natural_compare
```

## State Transitions

```
Normal --[S]--> Menu(SortMenu)
  Menu --[Enter/shortcut]--> Normal (sort applied)
  Menu --[Esc/q]--> Normal (unchanged)

Normal --[s]--> Normal (direction toggled, sort applied)
```
