# Research: 設定体系の整備

**Feature**: 017-config-refinement
**Date**: 2026-02-22

## R-001: アクション名の階層化パターン

**Decision**: ドット区切りの階層的アクション名（`tree.sort.by_name`）を採用し、Rust 側では既存のネストした enum 構造を拡張する。

**Rationale**:
- 現在の `Action::Tree(TreeAction::SortMenu)` のような2段 enum は自然に3段に拡張可能
- `TreeAction` に `Sort(SortAction)` のようなサブ enum を追加し、`SortAction::ByName` 等を定義
- `FromStr` / `Display` のプレフィックスベースのディスパッチ機構はそのまま拡張できる（`"tree.sort."` → `SortAction::from_str`）
- 文字列からの解析はパフォーマンスクリティカルではない（config ロード時のみ）

**Alternatives considered**:
- フラットな enum に全バリアントを追加（`TreeAction::SortByName` 等）→ enum が肥大化し、階層構造が失われる
- マクロで自動生成 → 複雑すぎる。手動でも十分管理可能な規模

## R-002: JSON Schema のアクション一覧の自動生成

**Decision**: `Action` 型に `all_action_names() -> Vec<&'static str>` のようなヘルパー関数を追加し、`KeyBindingEntry` の `JsonSchema` 実装からこれを呼び出す。

**Rationale**:
- 現在は `config.rs` の `KeyBindingEntry::json_schema` 内にアクション文字列が手動でハードコードされている
- `action.rs` に変更を加えるたびに `config.rs` も更新が必要という二重管理が発生している
- 各サブ enum（`TreeAction`, `PreviewAction`, `FileOpAction` および新設の `SortAction`, `CopyAction`）に `variants() -> Vec<&str>` を追加し、`Action::all_action_names()` で集約する
- `schemars` の `JsonSchema` trait は手動実装を許可しているため、この関数の結果を enum リストに埋め込める

**Alternatives considered**:
- `strum` クレートを導入して `EnumIter` + `Display` で自動列挙 → 依存追加に見合わない、現在の `FromStr`/`Display` 手動実装で十分
- ビルドスクリプトで生成 → 過剰な複雑さ

## R-003: ユーザー定義メニューの設定構造

**Decision**: `Config` に `menus` セクションを追加。`KeyBindingEntry` に `menu` フィールドを追加してメニューを呼び出す。

**Rationale**:
- メニュー定義は `HashMap<String, MenuDefinition>` で名前付き管理
- `MenuDefinition` は `title: String`, `items: Vec<MenuItemDef>` を持つ
- `MenuItemDef` は `KeyBindingEntry` と同様に `key: char`, `label: String`, `action/run/notify` を持つ
- キーバインドからは `menu: "menu_name"` で参照
- `Action` enum に `OpenMenu(String)` バリアントを追加し、`KeyMap` 解決時にメニュー名を保持
- ハンドラでは `Action::OpenMenu(name)` を受け取り、config のメニュー定義を参照して `MenuState` を構築
- `MenuAction` に `Custom` バリアントを追加（メニュー項目の action/run/notify を直接ディスパッチ）

**Alternatives considered**:
- メニュー定義をキーバインド設定内にインライン化 → 設定が深くネストして読みにくい
- メニュー定義を別ファイルに分離 → YAML の利便性が損なわれる

## R-004: 設定項目名のリネーム対象

**Decision**: 以下の設定項目名を変更する。

### Config フィールドのリネーム

| 現在の名前 | 新しい名前 | 理由 |
|---|---|---|
| `file_operations` | `file_op` | アクションプレフィックス `file_op.*` との一貫性 |
| `preview.split` | `preview.split_ratio` | 何の split かを明確化（プレビューパネル幅の比率） |
| `preview.narrow_split` | `preview.narrow_split_ratio` | 同上、narrow モード時の比率 |
| `preview.narrow_threshold` | `preview.narrow_width` | 何のしきい値かを明確化（ターミナル幅の列数） |
| `display.show_root` | `display.show_root_entry` | 「root」が何を指すか明確化（ツリーのルートエントリ） |

### アクション名のリネーム

| 現在の名前 | 新しい名前 | 理由 |
|---|---|---|
| `tree.sort_menu` | `tree.sort.menu` | ドット階層形式への統一 |
| `tree.toggle_sort_direction` | `tree.sort.toggle_direction` | ドット階層形式への統一 |
| `file_op.copy_menu` | `file_op.copy.menu` | ドット階層形式への統一 |
| `tree.toggle_hidden` | `filter.hidden` | フィルター操作を独立カテゴリに分離。将来的なフィルター拡張（行数、サイズ等）に備える |
| `tree.toggle_ignored` | `filter.ignored` | 同上 |

### 新規アクション

| アクション名 | 説明 |
|---|---|
| `tree.sort.by_name` | ソート順を名前順に設定 |
| `tree.sort.by_size` | ソート順をサイズ順に設定 |
| `tree.sort.by_mtime` | ソート順を更新日時順に設定 |
| `tree.sort.by_type` | ソート順を種類順に設定 |
| `tree.sort.by_extension` | ソート順を拡張子順に設定 |
| `tree.sort.by_smart` | ソート順をスマート順に設定 |
| `file_op.copy.absolute_path` | 絶対パスをクリップボードにコピー |
| `file_op.copy.relative_path` | 相対パスをクリップボードにコピー |
| `file_op.copy.file_name` | ファイル名をクリップボードにコピー |
| `file_op.copy.stem` | 拡張子なしファイル名をクリップボードにコピー |
| `file_op.copy.parent_dir` | 親ディレクトリパスをクリップボードにコピー |
| `menu:<name>` | ユーザー定義メニューを開く |

**Alternatives considered**:
- より大規模なリネーム（`display` → `view` 等）→ 現在の名前は十分明確。不必要な変更は混乱を招く
- `file_op.create_file` → `file_op.create` のような短縮 → 既存ユーザー設定との互換性が失われ、メリットが少ない。ただしリリース前なので破壊的変更可。planning phase で最終決定

## R-005: `Shell`/`Notify` アクションの `FromStr` 対応

**Decision**: `Action::from_str` で `"shell:{cmd}"` と `"notify:{method}"` を解析可能にする。

**Rationale**:
- 現在これらは `KeyBindingEntry` の `run` / `notify` フィールド経由でのみ構築可能
- メニュー項目でも同じ3種類のアクションを使えるようにするため、統一的な文字列表現が必要
- `from_str` で `"shell:"` プレフィックスと `"notify:"` プレフィックスをパースすれば、メニュー項目の `action` フィールドから直接構築可能になる
- ただし、メニュー項目の設定構造は `KeyBindingEntry` と同様に `action/run/notify` の3フィールド方式も併用可能

**Alternatives considered**:
- メニュー項目専用のパース処理を別途作成 → コードの重複
