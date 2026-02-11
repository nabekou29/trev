# Quickstart: コアツリーデータ構造

## Prerequisites

- Rust nightly-2026-01-24 (`rustup toolchain install nightly-2026-01-24`)
- mise (`mise run build` 等のタスクランナー)

## Dependencies (already in Cargo.toml)

```toml
# File system (gitignore-aware walk)
ignore = "0.4"

# Async runtime (spawn_blocking for sync IO)
tokio = { version = "1", features = ["full"] }

# Serialization (NodeInfo, Action enum)
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## Dev Dependencies (already in Cargo.toml)

```toml
googletest = "0.14"
rstest = "0.26"
tempfile = "3"
```

## Module Structure

```text
src/
├── state.rs             # pub mod tree;
├── state/
│   └── tree.rs          # TreeState, TreeNode, ChildrenState, VisibleNode, SortOrder, SortDirection
├── tree.rs              # pub mod builder; pub mod sort; pub mod search_index;
├── tree/
│   ├── builder.rs       # TreeBuilder (ignore::WalkBuilder wrapper)
│   ├── sort.rs          # sort_children() — 再帰的ソート
│   └── search_index.rs  # SearchIndex (バックグラウンド走査)
├── action.rs            # Action enum (TreeAction を含む)
```

## Key Patterns

### TreeBuilder — ignore::WalkBuilder でビルド

```rust
use ignore::WalkBuilder;

pub struct TreeBuilder {
    show_hidden: bool,
    show_ignored: bool,
}

impl TreeBuilder {
    pub fn build(&self, root_path: &Path) -> Result<TreeNode> {
        // WalkBuilder::new(root_path).max_depth(Some(1)) でルート直下のみ
        // .hidden(!self.show_hidden) で隠しファイル制御
        // .git_ignore(!self.show_ignored) で gitignore 制御
    }

    pub fn load_children(&self, dir_path: &Path) -> Result<Vec<TreeNode>> {
        // 同じ WalkBuilder パターンで 1 階層読み込み
        // skip_entry(|e| e.path() == dir_path) でルート自身を除外
    }
}
```

### TreeState — 都度計算の visible_nodes

```rust
pub struct TreeState {
    root: TreeNode,
    cursor: usize,
    sort_order: SortOrder,
    sort_direction: SortDirection,
    directories_first: bool,
}

impl TreeState {
    pub fn visible_nodes(&self) -> Vec<VisibleNode<'_>> {
        // DFS walk: 展開済み + Loaded のノードのみ走査
        // Vec<VisibleNode { node: &TreeNode, depth: usize }>
    }

    pub fn set_children(&mut self, path: &Path, children: Vec<TreeNode>) {
        // パスを辿って対象ノードを見つけ、children を Loaded(children) に設定
    }
}
```

### ChildrenState — 状態遷移

```rust
pub enum ChildrenState {
    NotLoaded,
    Loading,
    Loaded(Vec<TreeNode>),
}
// NotLoaded → Loading → Loaded(children)
//                     → NotLoaded (エラー時)
```

### ソート — children の再帰的ソート

```rust
fn sort_children(
    children: &mut [TreeNode],
    order: SortOrder,
    direction: SortDirection,
    dirs_first: bool,
) {
    children.sort_by(|a, b| {
        // 1. dirs_first: ディレクトリを先頭
        // 2. order + direction: キーに応じた比較
        // 3. Name は case-insensitive (to_lowercase)
    });
    // 再帰: Loaded な子ディレクトリにも適用
}
```

### SearchIndex — バックグラウンド走査

```rust
pub struct SearchIndex {
    entries: Vec<SearchEntry>,
    is_complete: bool,
}

// tokio::task::spawn_blocking で走査
// ignore::WalkBuilder::new(root).build() のフル走査
// 走査中も entries() で部分結果にアクセス可能
```

## Test Patterns

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use googletest::prelude::*;
    use tempfile::TempDir;

    fn create_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("file.txt"), "").unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();
        dir
    }

    #[test]
    fn test_build_tree() -> Result<()> {
        let dir = create_test_dir();
        let builder = TreeBuilder::new(false, false);
        let root = builder.build(dir.path()).unwrap();
        verify_that!(root.is_dir, eq(true))?;
        Ok(())
    }
}
```

## Build & Test

```bash
mise run build    # コンパイル確認
mise run test     # テスト実行
mise run lint     # clippy チェック
```
