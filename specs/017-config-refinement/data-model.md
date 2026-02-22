# Data Model: 設定体系の整備

**Feature**: 017-config-refinement
**Date**: 2026-02-22

## エンティティ

### Action (拡張)

既存の Action enum を階層的に拡張する。

```
Action
├── Tree(TreeAction)
│   ├── MoveDown, MoveUp, Expand, Collapse, ...  (既存、変更なし)
│   ├── Sort(SortAction)                          ★新設サブenum
│   │   ├── Menu          (旧: TreeAction::SortMenu)
│   │   ├── ToggleDirection (旧: TreeAction::ToggleSortDirection)
│   │   ├── ByName        ★新規
│   │   ├── BySize        ★新規
│   │   ├── ByMtime       ★新規
│   │   ├── ByType        ★新規
│   │   ├── ByExtension   ★新規
│   │   └── BySmart       ★新規
│   └── Refresh, ...                              (既存、変更なし)
├── Filter(FilterAction)                           ★新設トップレベルenum
│   ├── Hidden      (旧: TreeAction::Hidden)
│   └── Ignored     (旧: TreeAction::Ignored)
│   # 将来拡張でフィルター条件を追加可能
├── Preview(PreviewAction)                         (既存、変更なし)
├── FileOp(FileOpAction)
│   ├── Yank, Cut, Paste, ...                      (既存、変更なし)
│   └── Copy(CopyAction)                          ★新設サブenum
│       ├── Menu          (旧: FileOpAction::CopyMenu)
│       ├── AbsolutePath  ★新規
│       ├── RelativePath  ★新規
│       ├── FileName      ★新規
│       ├── Stem          ★新規
│       └── ParentDir     ★新規
├── OpenMenu(String)                               ★新規: ユーザー定義メニュー
├── Quit, Shell(String), Notify(String), Noop      (既存、変更なし)
```

**文字列表現**:

| Variant | 文字列 |
|---|---|
| `Tree(Sort(ByName))` | `"tree.sort.by_name"` |
| `Tree(Sort(Menu))` | `"tree.sort.menu"` |
| `Tree(Sort(ToggleDirection))` | `"tree.sort.toggle_direction"` |
| `Filter(Hidden)` | `"filter.hidden"` |
| `Filter(Ignored)` | `"filter.ignored"` |
| `FileOp(Copy(AbsolutePath))` | `"file_op.copy.absolute_path"` |
| `FileOp(Copy(Menu))` | `"file_op.copy.menu"` |
| `OpenMenu(name)` | `"menu:<name>"` |

### MenuDefinition (新規)

ユーザー定義メニューの設定エンティティ。

```
MenuDefinition
├── title: String           # メニューのタイトル（表示用）
└── items: Vec<MenuItemDef> # メニュー項目のリスト（表示順）

MenuItemDef
├── key: char               # ショートカットキー（1文字）
├── label: String           # 表示ラベル
├── action: Option<String>  # 組み込みアクション名（排他）
├── run: Option<String>     # シェルコマンドテンプレート（排他）
└── notify: Option<String>  # IPC通知メソッド名（排他）
```

**制約**:
- `action`, `run`, `notify` のうち正確に1つが指定されなければならない
- `key` はメニュー内で一意であるべき（重複時は最初の項目が優先）
- `items` が空のメニューは有効（開いても即閉じる、または何も表示しない）

### MenuAction (拡張)

```
MenuAction
├── CopyToClipboard         # 既存: 値をクリップボードにコピー
├── SelectSortOrder          # 既存: ソート順を変更
└── Custom(Vec<MenuItemDef>) # ★新規: ユーザー定義メニューの項目を保持
```

### Config (リネーム)

```
Config
├── sort: SortConfig                  # 変更なし
├── display: DisplayConfig            # フィールド名変更あり
│   ├── show_hidden: bool
│   ├── show_ignored: bool
│   ├── show_preview: bool
│   ├── show_root_entry: bool         ★ 旧: show_root
│   ├── columns: Vec<ColumnKind>
│   └── column_options: ColumnOptionsConfig
├── preview: PreviewConfig            # フィールド名変更あり
│   ├── max_lines: usize
│   ├── max_bytes: u64
│   ├── cache_size: usize
│   ├── commands: Vec<ExternalCommand>
│   ├── command_timeout: u64
│   ├── split_ratio: u16              ★ 旧: split
│   ├── narrow_split_ratio: u16       ★ 旧: narrow_split
│   ├── narrow_width: u16             ★ 旧: narrow_threshold
│   └── word_wrap: bool
├── file_op: FileOpConfig             ★ 旧: file_operations
│   ├── delete_mode: DeleteMode
│   └── undo_stack_size: usize
├── session: SessionConfig            # 変更なし
├── watcher: WatcherConfig            # 変更なし
├── keybindings: KeybindingConfig     # KeyBindingEntry に menu フィールド追加
├── git: GitConfig                    # 変更なし
└── menus: HashMap<String, MenuDefinition>  ★ 新規セクション
```

### KeyBindingEntry (拡張)

```
KeyBindingEntry
├── key: String             # キー表記
├── action: Option<String>  # アクション名（排他）
├── run: Option<String>     # シェルコマンド（排他）
├── notify: Option<String>  # IPC通知（排他）
└── menu: Option<String>    # ★ 新規: ユーザー定義メニュー名（排他）
```

**制約**: `action`, `run`, `notify`, `menu` のうち正確に1つが指定されなければならない。

## 状態遷移

### メニュー表示フロー

```
Normal → (キー押下)
  ├── action = "tree.sort.menu" → 組み込みソートメニュー表示
  ├── action = "file_op.copy.menu" → 組み込みコピーメニュー表示
  ├── menu = "my_menu" → ユーザー定義メニュー表示
  └── action = "tree.sort.by_name" → 直接ソート実行（メニューなし）

Menu → (項目選択)
  ├── CopyToClipboard → クリップボードにコピー → Normal
  ├── SelectSortOrder → ソート順変更 → Normal
  └── Custom → MenuItemDef から Action を解決して実行 → Normal

Menu → (Esc/q) → Normal
```
