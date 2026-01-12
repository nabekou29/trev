//! ツリー構築
//!
//! `ignore` クレートを使用してファイルシステムからツリーを構築する。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use ignore::WalkBuilder;

use super::{
    NodeKind,
    SortConfig,
    SortDirection,
    SortOrder,
    TreeNode,
};

/// ツリービルダー
///
/// ファイルシステムを走査してツリー構造を構築する。
#[derive(Debug)]
pub(crate) struct TreeBuilder {
    /// 隠しファイルを表示するか
    show_hidden: bool,
    /// gitignore されたファイルを表示するか
    show_ignored: bool,
    /// 最大深さ（None で無制限）
    max_depth: Option<usize>,
    /// ソート設定
    sort_config: SortConfig,
}

impl Default for TreeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeBuilder {
    /// 新しい `TreeBuilder` を作成する
    pub(crate) fn new() -> Self {
        Self { show_hidden: false, show_ignored: false, max_depth: None, sort_config: SortConfig::new() }
    }

    /// 隠しファイルの表示設定を変更する
    pub(crate) fn show_hidden(mut self, show: bool) -> Self {
        self.show_hidden = show;
        self
    }

    /// gitignore されたファイルの表示設定を変更する
    pub(crate) fn show_ignored(mut self, show: bool) -> Self {
        self.show_ignored = show;
        self
    }

    /// 最大深さを設定する
    #[allow(dead_code)]
    pub(crate) fn max_depth(mut self, depth: Option<usize>) -> Self {
        self.max_depth = depth;
        self
    }

    /// ソート設定を変更する
    pub(crate) fn sort_config(mut self, config: SortConfig) -> Self {
        self.sort_config = config;
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
            return Some(TreeNode::new(root_name, root_path, NodeKind::File, 0));
        }

        // ディレクトリの場合はツリーを構築
        let mut root = TreeNode::new(root_name, root_path.clone(), NodeKind::Directory, 0);
        root.expanded = true;

        // ignore::WalkBuilder で走査
        let walker = WalkBuilder::new(&root_path)
            .hidden(!self.show_hidden)
            .git_ignore(!self.show_ignored)
            .git_global(!self.show_ignored)
            .git_exclude(!self.show_ignored)
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

            let name =
                path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();

            let kind = if path.is_symlink() {
                NodeKind::Symlink
            } else if path.is_dir() {
                NodeKind::Directory
            } else {
                NodeKind::File
            };

            // 深さを計算
            let depth = path.strip_prefix(&root_path).map(|p| p.components().count()).unwrap_or(0);

            // 実行可能フラグを取得（Unix 系のみ）
            let is_executable = is_executable_file(&path);

            // シンボリックリンクのターゲットを取得
            let symlink_target = if kind == NodeKind::Symlink {
                fs::read_link(&path).ok().map(|t| t.to_string_lossy().to_string())
            } else {
                None
            };

            // ファイルメタデータを取得
            let (size, mtime) = fs::metadata(&path)
                .map(|m| (m.len(), m.modified().ok()))
                .unwrap_or((0, None));

            let mut node = TreeNode::new(name, path.to_path_buf(), kind, depth);
            node.is_executable = is_executable;
            node.symlink_target = symlink_target;
            node.size = size;
            node.mtime = mtime;

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
        let sort_config = self.sort_config;
        if let Some(root) = nodes.get_mut(&root_path) {
            Self::sort_children(root, &sort_config);
        }

        nodes.remove(&root_path)
    }

    /// 子ノードを再帰的にソートする
    fn sort_children(node: &mut TreeNode, config: &SortConfig) {
        node.children.sort_by(|a, b| {
            // ディレクトリ優先オプション
            if config.directories_first {
                let a_is_dir = a.kind == NodeKind::Directory;
                let b_is_dir = b.kind == NodeKind::Directory;

                match (a_is_dir, b_is_dir) {
                    (true, false) => return std::cmp::Ordering::Less,
                    (false, true) => return std::cmp::Ordering::Greater,
                    _ => {}
                }
            }

            // ソート順に応じた比較
            let ordering = match config.order {
                SortOrder::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortOrder::Size => {
                    let a_size = fs::metadata(&a.path).map(|m| m.len()).unwrap_or(0);
                    let b_size = fs::metadata(&b.path).map(|m| m.len()).unwrap_or(0);
                    a_size.cmp(&b_size)
                }
                SortOrder::Mtime => {
                    let a_mtime = fs::metadata(&a.path)
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    let b_mtime = fs::metadata(&b.path)
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    a_mtime.cmp(&b_mtime)
                }
                SortOrder::Type => {
                    let a_is_dir = a.kind == NodeKind::Directory;
                    let b_is_dir = b.kind == NodeKind::Directory;
                    a_is_dir.cmp(&b_is_dir).reverse()
                }
                SortOrder::Extension => {
                    let a_ext = a.path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    let b_ext = b.path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    a_ext.to_lowercase().cmp(&b_ext.to_lowercase())
                }
            };

            // 降順の場合は反転
            match config.direction {
                SortDirection::Ascending => ordering,
                SortDirection::Descending => ordering.reverse(),
            }
        });

        for child in &mut node.children {
            Self::sort_children(child, config);
        }
    }
}

/// ファイルが実行可能かどうかを判定する
#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path)
        .map(|m| !m.is_dir() && (m.permissions().mode() & 0o111) != 0)
        .unwrap_or(false)
}

/// ファイルが実行可能かどうかを判定する（Windows では常に false）
#[cfg(not(unix))]
fn is_executable_file(_path: &Path) -> bool {
    false
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

        let tree = TreeBuilder::new().show_hidden(false).build(dir.path()).unwrap();

        assert_that!(tree.children, len(eq(1)));
    }

    #[rstest]
    fn hidden_files_included_when_enabled() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".hidden"), "hidden").unwrap();
        fs::write(dir.path().join("visible"), "visible").unwrap();

        let tree = TreeBuilder::new().show_hidden(true).build(dir.path()).unwrap();

        assert_that!(tree.children, len(eq(2)));
    }
}
