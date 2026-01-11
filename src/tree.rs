//! ツリーデータ構造
//!
//! ファイルシステムのツリー表示に必要なデータ構造を定義する。

pub(crate) mod builder;

use std::path::PathBuf;

use crate::git::GitStatus;

/// ノードの種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NodeKind {
    /// ファイル
    File,
    /// ディレクトリ
    Directory,
    /// シンボリックリンク
    Symlink,
}

/// ツリーノード
///
/// ファイルシステムの各エントリを表現する。
#[derive(Debug, Clone)]
pub(crate) struct TreeNode {
    /// ファイル名（パスの最後の要素）
    pub(crate) name: String,
    /// フルパス
    pub(crate) path: PathBuf,
    /// ノード種別
    pub(crate) kind: NodeKind,
    /// 子ノード（ディレクトリの場合）
    pub(crate) children: Vec<TreeNode>,
    /// 展開状態（ディレクトリの場合）
    pub(crate) expanded: bool,
    /// ツリー内での深さ
    pub(crate) depth: usize,
}

impl TreeNode {
    /// 新しい `TreeNode` を作成する
    pub(crate) fn new(
        name: String,
        path: PathBuf,
        kind: NodeKind,
        depth: usize,
    ) -> Self {
        Self {
            name,
            path,
            kind,
            children: Vec::new(),
            expanded: false,
            depth,
        }
    }

    /// 子ノードを持つかどうかを返す
    pub(crate) fn has_children(&self) -> bool {
        !self.children.is_empty()
    }
}

/// 表示用のフラット化ノード
///
/// ツリーの現在の表示状態をフラットなリストとして表現する。
#[derive(Debug, Clone)]
pub(crate) struct VisibleNode {
    /// ファイル名
    pub(crate) name: String,
    /// フルパス
    pub(crate) path: PathBuf,
    /// ツリー内での深さ
    pub(crate) depth: usize,
    /// ノード種別
    pub(crate) kind: NodeKind,
    /// 展開状態
    pub(crate) expanded: bool,
    /// Git ステータス
    pub(crate) git_status: Option<GitStatus>,
    /// 子ノードを持つか
    pub(crate) has_children: bool,
}

/// ツリー全体の状態
#[derive(Debug)]
pub(crate) struct TreeState {
    /// ルートノード
    root: TreeNode,
    /// フラット化されたノードリスト（表示用）
    visible_nodes: Vec<VisibleNode>,
    /// 選択中のインデックス
    selected: usize,
    /// スクロールオフセット
    scroll_offset: usize,
}

impl TreeState {
    /// 新しい `TreeState` を作成する
    pub(crate) fn new(root: TreeNode) -> Self {
        let mut state = Self {
            root,
            visible_nodes: Vec::new(),
            selected: 0,
            scroll_offset: 0,
        };
        state.rebuild_visible_nodes();
        state
    }

    /// 表示用ノードリストを取得する
    pub(crate) fn visible_nodes(&self) -> &[VisibleNode] {
        &self.visible_nodes
    }

    /// 選択中のインデックスを取得する
    pub(crate) fn selected(&self) -> usize {
        self.selected
    }

    /// スクロールオフセットを取得する
    pub(crate) fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// 選択中のノードを取得する
    pub(crate) fn selected_node(&self) -> Option<&VisibleNode> {
        self.visible_nodes.get(self.selected)
    }

    /// 次のノードを選択する
    pub(crate) fn select_next(&mut self) {
        if self.selected + 1 < self.visible_nodes.len() {
            self.selected += 1;
        }
    }

    /// 前のノードを選択する
    pub(crate) fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// 先頭のノードを選択する
    pub(crate) fn select_first(&mut self) {
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// 末尾のノードを選択する
    pub(crate) fn select_last(&mut self) {
        if !self.visible_nodes.is_empty() {
            self.selected = self.visible_nodes.len() - 1;
        }
    }

    /// 選択中のノードを展開する
    ///
    /// ディレクトリの場合は展開し、ファイルの場合は何もしない。
    /// 戻り値: 展開が行われた場合は `true`
    pub(crate) fn expand_selected(&mut self) -> bool {
        let Some(node) = self.visible_nodes.get(self.selected) else {
            return false;
        };

        if node.kind != NodeKind::Directory || node.expanded {
            return false;
        }

        let path = node.path.clone();
        if self.set_expanded(&path, true) {
            self.rebuild_visible_nodes();
            true
        } else {
            false
        }
    }

    /// 選択中のノードを折り畳む
    ///
    /// 展開されたディレクトリの場合は折り畳み、
    /// そうでない場合は親ディレクトリに移動する。
    pub(crate) fn collapse_selected(&mut self) {
        let Some(node) = self.visible_nodes.get(self.selected) else {
            return;
        };

        if node.kind == NodeKind::Directory && node.expanded {
            // 展開されたディレクトリを折り畳む
            let path = node.path.clone();
            if self.set_expanded(&path, false) {
                self.rebuild_visible_nodes();
            }
        } else {
            // 親ディレクトリに移動
            self.select_parent();
        }
    }

    /// 親ディレクトリを選択する
    pub(crate) fn select_parent(&mut self) {
        let Some(current) = self.visible_nodes.get(self.selected) else {
            return;
        };

        let current_depth = current.depth;
        if current_depth == 0 {
            return;
        }

        // 現在位置より前で、深さが1つ小さいノードを探す
        for i in (0..self.selected).rev() {
            if let Some(node) = self.visible_nodes.get(i)
                && node.depth < current_depth
            {
                self.selected = i;
                break;
            }
        }
    }

    /// 指定パスのノードの展開状態を設定する
    fn set_expanded(
        &mut self,
        path: &PathBuf,
        expanded: bool,
    ) -> bool {
        Self::set_expanded_recursive(&mut self.root, path, expanded)
    }

    /// 再帰的に展開状態を設定する
    fn set_expanded_recursive(
        node: &mut TreeNode,
        path: &PathBuf,
        expanded: bool,
    ) -> bool {
        if &node.path == path {
            node.expanded = expanded;
            return true;
        }

        for child in &mut node.children {
            if Self::set_expanded_recursive(child, path, expanded) {
                return true;
            }
        }

        false
    }

    /// 表示用ノードリストを再構築する
    fn rebuild_visible_nodes(&mut self) {
        self.visible_nodes.clear();
        self.collect_visible_nodes(&self.root.clone(), None);

        // 選択インデックスが範囲外にならないように調整
        if !self.visible_nodes.is_empty() && self.selected >= self.visible_nodes.len() {
            self.selected = self.visible_nodes.len() - 1;
        }
    }

    /// 可視ノードを再帰的に収集する
    fn collect_visible_nodes(
        &mut self,
        node: &TreeNode,
        git_status: Option<GitStatus>,
    ) {
        self.visible_nodes.push(VisibleNode {
            name: node.name.clone(),
            path: node.path.clone(),
            depth: node.depth,
            kind: node.kind,
            expanded: node.expanded,
            git_status,
            has_children: node.has_children(),
        });

        if node.expanded {
            for child in &node.children {
                self.collect_visible_nodes(child, None);
            }
        }
    }

    /// Git ステータスを更新する
    pub(crate) fn update_git_status<F>(&mut self, get_status: F)
    where
        F: Fn(&PathBuf) -> Option<GitStatus>,
    {
        for node in &mut self.visible_nodes {
            node.git_status = get_status(&node.path);
        }
    }

    /// 表示領域に合わせてスクロールオフセットを調整する
    pub(crate) fn adjust_scroll(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }

        // 選択が表示領域より上にある場合
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }

        // 選択が表示領域より下にある場合
        if self.selected >= self.scroll_offset + visible_height {
            self.scroll_offset = self.selected - visible_height + 1;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    /// テスト用のツリーを作成するフィクスチャ
    #[fixture]
    fn test_tree() -> TreeNode {
        let mut root = TreeNode::new(
            "root".to_string(),
            PathBuf::from("/root"),
            NodeKind::Directory,
            0,
        );
        root.expanded = true;

        let file1 = TreeNode::new(
            "file1.txt".to_string(),
            PathBuf::from("/root/file1.txt"),
            NodeKind::File,
            1,
        );

        let mut dir1 = TreeNode::new(
            "dir1".to_string(),
            PathBuf::from("/root/dir1"),
            NodeKind::Directory,
            1,
        );

        let file2 = TreeNode::new(
            "file2.txt".to_string(),
            PathBuf::from("/root/dir1/file2.txt"),
            NodeKind::File,
            2,
        );

        dir1.children.push(file2);
        root.children.push(file1);
        root.children.push(dir1);

        root
    }

    #[rstest]
    fn tree_state_creation_with_expanded_root(test_tree: TreeNode) {
        let state = TreeState::new(test_tree);

        // ルートが展開されているので、ルート + 直下の2つ = 3ノード
        assert_that!(state.visible_nodes(), len(eq(3)));
        assert_that!(state.selected(), eq(0));
    }

    #[rstest]
    fn select_next_increments_selection(test_tree: TreeNode) {
        let mut state = TreeState::new(test_tree);

        assert_that!(state.selected(), eq(0));

        state.select_next();
        assert_that!(state.selected(), eq(1));

        state.select_next();
        assert_that!(state.selected(), eq(2));
    }

    #[rstest]
    fn select_next_does_not_exceed_end(test_tree: TreeNode) {
        let mut state = TreeState::new(test_tree);

        state.select_next();
        state.select_next();
        state.select_next(); // 末尾を超えようとする

        assert_that!(state.selected(), eq(2));
    }

    #[rstest]
    fn select_prev_decrements_selection(test_tree: TreeNode) {
        let mut state = TreeState::new(test_tree);

        state.select_next();
        state.select_next();
        state.select_prev();

        assert_that!(state.selected(), eq(1));
    }

    #[rstest]
    fn select_first_jumps_to_beginning(test_tree: TreeNode) {
        let mut state = TreeState::new(test_tree);

        state.select_next();
        state.select_next();
        state.select_first();

        assert_that!(state.selected(), eq(0));
    }

    #[rstest]
    fn select_last_jumps_to_end(test_tree: TreeNode) {
        let mut state = TreeState::new(test_tree);

        state.select_last();

        assert_that!(state.selected(), eq(2));
    }

    #[rstest]
    fn expand_directory_increases_visible_nodes(test_tree: TreeNode) {
        let mut state = TreeState::new(test_tree);

        // dir1 を選択
        state.select_next();
        state.select_next();

        // 展開前は3ノード
        assert_that!(state.visible_nodes(), len(eq(3)));

        // 展開
        assert_that!(state.expand_selected(), eq(true));

        // 展開後は4ノード（file2.txt が追加）
        assert_that!(state.visible_nodes(), len(eq(4)));
    }

    #[rstest]
    fn collapse_directory_decreases_visible_nodes(test_tree: TreeNode) {
        let mut state = TreeState::new(test_tree);

        // dir1 を選択して展開
        state.select_next();
        state.select_next();
        state.expand_selected();

        // 折り畳み
        state.collapse_selected();

        assert_that!(state.visible_nodes(), len(eq(3)));
    }
}
