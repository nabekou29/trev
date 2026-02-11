# Data Model: TUI 基盤

## 既存エンティティ (002-core-tree で定義済み)

### TreeNode
ファイル/ディレクトリノード。`src/state/tree.rs` に定義済み。

### TreeState
ツリー状態管理。root, cursor, sort 設定。`src/state/tree.rs` に定義済み。

### VisibleNode
UI 描画用の参照ベースフラットリスト。`src/state/tree.rs` に定義済み。

## 新規エンティティ

### AppState

アプリケーション全体の状態。TreeState を保持し、UI に必要な追加情報を管理。

- `tree_state: TreeState` — ツリー状態
- `should_quit: bool` — 終了フラグ
- `show_icons: bool` — アイコン表示フラグ
- `viewport_height: u16` — 現在のビューポート高さ

### ScrollState

スクロール位置管理。ツリーの描画オフセットを管理。

- `offset: usize` — スクロールオフセット (表示開始行)
- `viewport_height: usize` — 表示可能行数

**ルール**: offset は `cursor - viewport_height + 1 <= offset <= cursor` を常に満たす。

### ChildrenLoadResult

非同期読み込みの結果。チャネル経由で UI スレッドに送信。

- `path: PathBuf` — 読み込み対象ディレクトリ
- `result: Result<Vec<TreeNode>>` — 読み込み結果 (成功: children, 失敗: error)

## 状態遷移

### アプリケーションライフサイクル

```
Init → Running → Quit
```

### ディレクトリ展開フロー

```
NotLoaded → [user expands] → Loading → [load complete] → Loaded
                                      → [load error]   → NotLoaded
```

(ChildrenState は 002-core-tree で既に定義済み)
