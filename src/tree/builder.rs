//! ツリー構築
//!
//! `ignore` クレートを使用してファイルシステムからツリーを構築する。

use std::collections::HashMap;
use std::path::Path;

use ignore::WalkBuilder;

use super::{
    NodeKind,
    TreeNode,
};

/// ツリービルダー
///
/// ファイルシステムを走査してツリー構造を構築する。
#[derive(Debug)]
pub(crate) struct TreeBuilder {
    /// 隠しファイルを表示するか
    show_hidden: bool,
    /// 最大深さ（None で無制限）
    max_depth: Option<usize>,
}

impl Default for TreeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeBuilder {
    /// 新しい `TreeBuilder` を作成する
    pub(crate) fn new() -> Self {
        Self {
            show_hidden: false,
            max_depth: None,
        }
    }

    /// 隠しファイルの表示設定を変更する
    pub(crate) fn show_hidden(mut self, show: bool) -> Self {
        self.show_hidden = show;
        self
    }

    /// 最大深さを設定する
    #[allow(dead_code)]
    pub(crate) fn max_depth(mut self, depth: Option<usize>) -> Self {
        self.max_depth = depth;
        self
    }

    /// 指定パスからツリーを構築する
    pub(crate) fn build(self, root_path: &Path) -> Option<TreeNode> {
        let root_path = root_path.canonicalize().ok()?;

        // ルートノードの名前を取得
        let root_name = root_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| root_path.to_string_lossy().to_string());

        // ルートがファイルの場合
        if root_path.is_file() {
            return Some(TreeNode::new(
                root_name,
                root_path,
                NodeKind::File,
                0,
            ));
        }

        // ディレクトリの場合はツリーを構築
        let mut root = TreeNode::new(
            root_name,
            root_path.clone(),
            NodeKind::Directory,
            0,
        );
        root.expanded = true;

        // ignore::WalkBuilder で走査
        let walker = WalkBuilder::new(&root_path)
            .hidden(!self.show_hidden)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .max_depth(self.max_depth)
            .sort_by_file_path(|a, b| {
                // ディレクトリを先に、その後ファイル名でソート
                let a_is_dir = a.is_dir();
                let b_is_dir = b.is_dir();

                match (a_is_dir, b_is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.cmp(b),
                }
            })
            .build();

        // パス -> ノードのマップ（親子関係の構築用）
        let mut nodes: HashMap<std::path::PathBuf, TreeNode> = HashMap::new();
        nodes.insert(root_path.clone(), root);

        // 全エントリを収集
        let mut entries: Vec<_> = walker.flatten().collect();

        // 深さでソート（浅い順）して親から先に処理
        entries.sort_by_key(|e| e.depth());

        for entry in entries {
            // パスを正規化（git status との一致のため）
            let path = match entry.path().canonicalize() {
                Ok(p) => p,
                Err(_) => continue,
            };

            // ルート自身はスキップ
            if path == root_path {
                continue;
            }

            let Some(parent_path) = path.parent() else {
                continue;
            };

            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();

            let kind = if path.is_symlink() {
                NodeKind::Symlink
            } else if path.is_dir() {
                NodeKind::Directory
            } else {
                NodeKind::File
            };

            // 深さを計算
            let depth = path
                .strip_prefix(&root_path)
                .map(|p| p.components().count())
                .unwrap_or(0);

            let node = TreeNode::new(name, path.to_path_buf(), kind, depth);

            // ディレクトリの場合はマップに追加（将来の子の親になる可能性）
            if kind == NodeKind::Directory {
                nodes.insert(path.to_path_buf(), node);
            } else {
                // ファイルの場合は親ノードに直接追加
                if let Some(parent) = nodes.get_mut(parent_path) {
                    parent.children.push(node);
                }
            }
        }

        // ツリー構造を構築（深いノードから親に追加）
        let mut paths: Vec<_> = nodes.keys().cloned().collect();
        paths.sort_by_key(|p| std::cmp::Reverse(p.components().count()));

        for path in paths {
            if path == root_path {
                continue;
            }

            if let Some(node) = nodes.remove(&path)
                && let Some(parent_path) = path.parent()
                && let Some(parent) = nodes.get_mut(parent_path)
            {
                parent.children.push(node);
            }
        }

        // ルートの子をソート
        if let Some(root) = nodes.get_mut(&root_path) {
            Self::sort_children(root);
        }

        nodes.remove(&root_path)
    }

    /// 子ノードを再帰的にソートする（ディレクトリ優先、名前順）
    fn sort_children(node: &mut TreeNode) {
        node.children.sort_by(|a, b| {
            let a_is_dir = a.kind == NodeKind::Directory;
            let b_is_dir = b.kind == NodeKind::Directory;

            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });

        for child in &mut node.children {
            Self::sort_children(child);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::fs;

    use googletest::prelude::*;
    use rstest::*;
    use tempfile::TempDir;

    use super::*;

    /// テスト用のディレクトリ構造を作成するフィクスチャ
    #[fixture]
    fn test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();

        // ファイルとディレクトリを作成
        fs::write(dir.path().join("file1.txt"), "content1").unwrap();
        fs::write(dir.path().join("file2.txt"), "content2").unwrap();

        let subdir = dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("nested.txt"), "nested").unwrap();

        dir
    }

    #[rstest]
    fn build_tree_returns_some(test_dir: TempDir) {
        let builder = TreeBuilder::new();

        let tree = builder.build(test_dir.path());

        assert_that!(tree, some(anything()));
    }

    #[rstest]
    fn build_tree_root_is_expanded_directory(test_dir: TempDir) {
        let builder = TreeBuilder::new();

        let root = builder.build(test_dir.path()).unwrap();

        assert_that!(root.kind, eq(NodeKind::Directory));
        assert_that!(root.expanded, eq(true));
    }

    #[rstest]
    fn build_tree_contains_all_entries(test_dir: TempDir) {
        let builder = TreeBuilder::new();

        let root = builder.build(test_dir.path()).unwrap();

        // ルート直下に3エントリ（file1.txt, file2.txt, subdir）
        assert_that!(root.children, len(eq(3)));
    }

    #[rstest]
    fn hidden_files_excluded_by_default() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".hidden"), "hidden").unwrap();
        fs::write(dir.path().join("visible"), "visible").unwrap();

        let tree = TreeBuilder::new()
            .show_hidden(false)
            .build(dir.path())
            .unwrap();

        assert_that!(tree.children, len(eq(1)));
    }

    #[rstest]
    fn hidden_files_included_when_enabled() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".hidden"), "hidden").unwrap();
        fs::write(dir.path().join("visible"), "visible").unwrap();

        let tree = TreeBuilder::new()
            .show_hidden(true)
            .build(dir.path())
            .unwrap();

        assert_that!(tree.children, len(eq(2)));
    }
}
