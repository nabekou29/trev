# Data Model: コアツリーデータ構造

## Entities

### TreeNode

ファイルまたはディレクトリを表すノード。

```rust
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// ファイル名（表示用）
    pub name: String,
    /// 絶対パス
    pub path: PathBuf,
    /// ディレクトリかどうか
    pub is_dir: bool,
    /// シンボリックリンクかどうか
    pub is_symlink: bool,
    /// ファイルサイズ（バイト）
    pub size: u64,
    /// 最終更新日時
    pub modified: Option<SystemTime>,
    /// 子ノードの状態（ディレクトリのみ意味を持つ）
    pub children: ChildrenState,
    /// 展開状態（ディレクトリのみ意味を持つ）
    pub is_expanded: bool,
}
```

**Validation**: `path` は絶対パスであること。`is_dir = false` のとき `children` は `ChildrenState::NotLoaded` で固定。

### ChildrenState

ディレクトリの子ノードの読み込み状態。

```rust
#[derive(Debug, Clone)]
pub enum ChildrenState {
    /// まだ読み込んでいない
    NotLoaded,
    /// 読み込み中
    Loading,
    /// 読み込み済み
    Loaded(Vec<TreeNode>),
}
```

**State Transitions**:
```
NotLoaded → Loading → Loaded(children)
                    → NotLoaded  (エラー時)
```

### TreeState

ツリー全体の状態管理。

```rust
pub struct TreeState {
    /// ルートノード
    root: TreeNode,
    /// カーソル位置（visible nodes のインデックス）
    cursor: usize,
    /// ソート設定
    sort_order: SortOrder,
    sort_direction: SortDirection,
    directories_first: bool,
}
```

**Key methods**:
- `visible_nodes(&self) -> Vec<VisibleNode<'_>>` — フラット化
- `move_cursor(&mut self, delta: i32)` — バウンドチェック付き移動
- `toggle_expand(&mut self, index: usize)` — 展開/折り畳みトグル
- `set_children(&mut self, path: &Path, children: Vec<TreeNode>)` — 遅延読み込み結果の反映
- `apply_sort(&mut self)` — 再帰的ソート適用
- `current_node_info(&self) -> Option<NodeInfo>` — シリアライズ可能な公開情報

### VisibleNode

UI 描画用のフラット化されたノード参照。

```rust
pub struct VisibleNode<'a> {
    /// ノードへの参照
    pub node: &'a TreeNode,
    /// インデント深さ
    pub depth: usize,
}
```

### NodeInfo (公開用、Serialize)

IPC/Lua に送信可能なノード情報。

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
}
```

### SortOrder / SortDirection

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum SortOrder {
    #[default]
    Name,
    Size,
    Modified,
    Type,
    Extension,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}
```

### TreeBuilder

ファイルシステムからツリーを構築するビルダー。

```rust
pub struct TreeBuilder {
    show_hidden: bool,
    show_ignored: bool,
}
```

**Key methods**:
- `build(root_path: &Path) -> Result<TreeNode>` — ルート直下 1 階層のみ読み込み
- `load_children(dir_path: &Path) -> Result<Vec<TreeNode>>` — 指定ディレクトリの子を 1 階層読み込み

### SearchIndex

バックグラウンド走査で構築される検索用インデックス。

```rust
pub struct SearchIndex {
    /// 全エントリ (パス, 名前) のリスト
    entries: Vec<SearchEntry>,
    /// 走査完了フラグ
    is_complete: bool,
}

pub struct SearchEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
}
```

**Key methods**:
- `add_entry(&mut self, entry: SearchEntry)` — エントリ追加（走査中に逐次呼び出し）
- `is_complete(&self) -> bool` — 走査完了判定
- `entries(&self) -> &[SearchEntry]` — 全エントリ参照
- `find_children(&self, parent: &Path) -> Vec<&SearchEntry>` — 指定パスの直下エントリを返す（遅延読み込みキャッシュ用）

### TreeAction (Action enum の一部)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TreeAction {
    MoveCursor(i32),
    MoveCursorTo(usize),
    JumpToFirst,
    JumpToLast,
    HalfPageDown(usize),  // viewport_height
    HalfPageUp(usize),
    ToggleExpand,
    Collapse,
    ExpandOrOpen,
    SetChildren { path: PathBuf, children: Vec<TreeNode> },
    SetChildrenError { path: PathBuf, error: String },
    ApplySort(SortOrder, SortDirection, bool),
    RevealPath(PathBuf),
}
```

## Relationships

```
TreeState
  └── root: TreeNode
        ├── children: ChildrenState::Loaded(Vec<TreeNode>)
        │                                     └── (recursive)
        └── → NodeInfo (変換メソッドで生成)

TreeBuilder → TreeNode (build/load_children の出力)

SearchIndex
  └── entries: Vec<SearchEntry>
        └── → TreeNode 構築のキャッシュとして利用可能
```
