//! ツリーデータ構造
//!
//! ファイルシステムのツリー表示に必要なデータ構造を定義する。

pub(crate) mod builder;

use std::path::PathBuf;
use std::time::SystemTime;

use nucleo::pattern::{
    AtomKind,
    CaseMatching,
    Normalization,
    Pattern,
};

use crate::git::{
    GitStatus,
    GitStatusSummary,
};

/// ソート順
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SortOrder {
    /// 名前順
    #[default]
    Name,
    /// サイズ順
    Size,
    /// 更新日時順
    Mtime,
    /// 種別順（ディレクトリ/ファイル）
    Type,
    /// 拡張子順
    Extension,
}

impl SortOrder {
    /// 次のソート順を取得する
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Name => Self::Size,
            Self::Size => Self::Mtime,
            Self::Mtime => Self::Type,
            Self::Type => Self::Extension,
            Self::Extension => Self::Name,
        }
    }

    /// 表示名を取得する
    pub(crate) fn display_name(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Size => "size",
            Self::Mtime => "mtime",
            Self::Type => "type",
            Self::Extension => "ext",
        }
    }
}

/// ソート方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SortDirection {
    /// 昇順
    #[default]
    Ascending,
    /// 降順
    Descending,
}

impl SortDirection {
    /// ソート方向を反転する
    pub(crate) fn toggle(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }

    /// 表示用シンボルを取得する
    pub(crate) fn symbol(self) -> &'static str {
        match self {
            Self::Ascending => "↑",
            Self::Descending => "↓",
        }
    }
}

/// ソート設定
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SortConfig {
    /// ソート順
    pub(crate) order: SortOrder,
    /// ソート方向
    pub(crate) direction: SortDirection,
    /// ディレクトリを先に表示するか
    pub(crate) directories_first: bool,
}

impl SortConfig {
    /// 新しい `SortConfig` を作成する
    pub(crate) fn new() -> Self {
        Self { order: SortOrder::Name, direction: SortDirection::Ascending, directories_first: true }
    }
}

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
    /// 実行可能ファイルか
    pub(crate) is_executable: bool,
    /// シンボリックリンクの場合のターゲットパス
    pub(crate) symlink_target: Option<String>,
    /// ファイルサイズ（バイト）
    pub(crate) size: u64,
    /// 更新日時
    pub(crate) mtime: Option<SystemTime>,
}

impl TreeNode {
    /// 新しい `TreeNode` を作成する
    pub(crate) fn new(name: String, path: PathBuf, kind: NodeKind, depth: usize) -> Self {
        Self {
            name,
            path,
            kind,
            children: Vec::new(),
            expanded: false,
            depth,
            is_executable: false,
            symlink_target: None,
            size: 0,
            mtime: None,
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
    /// Git ステータス集計（ディレクトリの場合）
    pub(crate) git_summary: Option<GitStatusSummary>,
    /// 子ノードを持つか
    pub(crate) has_children: bool,
    /// 実行可能ファイルか
    pub(crate) is_executable: bool,
    /// シンボリックリンクの場合のターゲットパス
    pub(crate) symlink_target: Option<String>,
    /// ファイルサイズ（バイト）
    pub(crate) size: u64,
    /// 更新日時
    pub(crate) mtime: Option<SystemTime>,
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
    /// フィルタクエリ
    filter_query: Option<String>,
}

impl TreeState {
    /// 新しい `TreeState` を作成する
    pub(crate) fn new(root: TreeNode) -> Self {
        let mut state =
            Self { root, visible_nodes: Vec::new(), selected: 0, scroll_offset: 0, filter_query: None };
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

    /// 半ページ下にスクロール
    pub(crate) fn page_down(&mut self, page_size: usize) {
        let half = page_size / 2;
        let max = self.visible_nodes.len().saturating_sub(1);
        self.selected = (self.selected + half).min(max);
    }

    /// 半ページ上にスクロール
    pub(crate) fn page_up(&mut self, page_size: usize) {
        let half = page_size / 2;
        self.selected = self.selected.saturating_sub(half);
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

    /// 指定パスのノードを選択する
    pub(crate) fn select_path(&mut self, path: &PathBuf) {
        for (i, node) in self.visible_nodes.iter().enumerate() {
            if &node.path == path {
                self.selected = i;
                return;
            }
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
    fn set_expanded(&mut self, path: &PathBuf, expanded: bool) -> bool {
        Self::set_expanded_recursive(&mut self.root, path, expanded)
    }

    /// 再帰的に展開状態を設定する
    fn set_expanded_recursive(node: &mut TreeNode, path: &PathBuf, expanded: bool) -> bool {
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
    fn collect_visible_nodes(&mut self, node: &TreeNode, git_status: Option<GitStatus>) {
        let filter_active = self.filter_query.is_some();

        // フィルタがある場合は、このノードまたは子孫がマッチするかチェック
        if filter_active && !self.node_or_descendants_match(node) {
            return;
        }

        // フィルタ中は、マッチする子孫がいるディレクトリを自動展開
        // 通常時は node.expanded に従う
        let should_show_children = if filter_active && node.kind == NodeKind::Directory {
            // 子または子孫にマッチがあれば展開表示
            node.children.iter().any(|child| self.node_or_descendants_match(child))
        } else {
            node.expanded
        };

        self.visible_nodes.push(VisibleNode {
            name: node.name.clone(),
            path: node.path.clone(),
            depth: node.depth,
            kind: node.kind,
            expanded: should_show_children,
            git_status,
            git_summary: None, // 後で update_git_status で設定
            has_children: node.has_children(),
            is_executable: node.is_executable,
            symlink_target: node.symlink_target.clone(),
            size: node.size,
            mtime: node.mtime,
        });

        if should_show_children {
            for child in &node.children {
                self.collect_visible_nodes(child, None);
            }
        }
    }

    /// Git ステータスを更新する
    pub(crate) fn update_git_status<F, S>(&mut self, get_status: F, get_summary: S)
    where
        F: Fn(&PathBuf) -> Option<GitStatus>,
        S: Fn(&PathBuf) -> GitStatusSummary,
    {
        for node in &mut self.visible_nodes {
            node.git_status = get_status(&node.path);
            // ディレクトリの場合はサマリを設定
            if node.kind == NodeKind::Directory {
                let summary = get_summary(&node.path);
                node.git_summary = if summary.is_empty() { None } else { Some(summary) };
            }
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

    /// フィルタを設定する（fuzzy マッチ対応）
    pub(crate) fn set_filter(&mut self, query: &str) {
        self.filter_query = Some(query.to_string());
        self.rebuild_visible_nodes();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// フィルタをクリアする
    pub(crate) fn clear_filter(&mut self) {
        self.filter_query = None;
        self.rebuild_visible_nodes();
    }

    /// フィルタがアクティブかどうかを返す
    pub(crate) fn has_filter(&self) -> bool {
        self.filter_query.is_some()
    }

    /// 展開されているパスのリストを取得する
    pub(crate) fn get_expanded_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        Self::collect_expanded_paths(&self.root, &mut paths);
        paths
    }

    /// 再帰的に展開されているパスを収集する
    fn collect_expanded_paths(node: &TreeNode, paths: &mut Vec<PathBuf>) {
        if node.expanded && node.kind == NodeKind::Directory {
            paths.push(node.path.clone());
            for child in &node.children {
                Self::collect_expanded_paths(child, paths);
            }
        }
    }

    /// 指定されたパスを展開する
    pub(crate) fn expand_paths(&mut self, paths: &[PathBuf]) {
        for path in paths {
            self.set_expanded(path, true);
        }
        self.rebuild_visible_nodes();
    }

    /// 指定パスをツリー上で表示・選択する（IPC reveal 用）
    ///
    /// 対象パスまでの親ディレクトリをすべて展開し、対象を選択する。
    /// 成功した場合は `true`、パスが見つからない場合は `false` を返す。
    pub(crate) fn reveal_path(&mut self, target: &std::path::Path) -> bool {
        // ツリー内でパスを探し、親ディレクトリを展開する
        let ancestors = Self::collect_ancestors(&self.root, target);
        if ancestors.is_empty() {
            return false;
        }

        // 親ディレクトリをすべて展開
        for path in &ancestors {
            self.set_expanded(path, true);
        }

        // 表示ノードを再構築
        self.rebuild_visible_nodes();

        // 対象を選択
        let target_buf = target.to_path_buf();
        for (i, node) in self.visible_nodes.iter().enumerate() {
            if node.path == target_buf {
                self.selected = i;
                return true;
            }
        }

        false
    }

    /// 指定パスまでの親ディレクトリパスを収集する（ルートから対象まで）
    fn collect_ancestors(node: &TreeNode, target: &std::path::Path) -> Vec<PathBuf> {
        if node.path == target {
            return vec![node.path.clone()];
        }

        // 対象パスがこのノード配下にあるかチェック
        if target.starts_with(&node.path) || node.path.as_os_str().is_empty() {
            for child in &node.children {
                let mut result = Self::collect_ancestors(child, target);
                if !result.is_empty() {
                    // このノードがディレクトリなら先頭に追加
                    if node.kind == NodeKind::Directory {
                        result.insert(0, node.path.clone());
                    }
                    return result;
                }
            }
        }

        Vec::new()
    }

    /// ノードがフィルタにマッチするかを判定する（fuzzy マッチ）
    fn matches_filter(&self, node: &TreeNode) -> bool {
        match &self.filter_query {
            Some(query) => {
                let pattern = Pattern::new(
                    query,
                    CaseMatching::Ignore,
                    Normalization::Smart,
                    AtomKind::Fuzzy,
                );
                let mut matcher = nucleo::Matcher::new(nucleo::Config::DEFAULT);
                let mut buf = Vec::new();
                let haystack = nucleo::Utf32Str::new(&node.name, &mut buf);
                pattern.score(haystack, &mut matcher).is_some()
            }
            None => true,
        }
    }

    /// ノードまたはその子孫がフィルタにマッチするかを判定する
    fn node_or_descendants_match(&self, node: &TreeNode) -> bool {
        if self.matches_filter(node) {
            return true;
        }

        for child in &node.children {
            if self.node_or_descendants_match(child) {
                return true;
            }
        }

        false
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
        let mut root =
            TreeNode::new("root".to_string(), PathBuf::from("/root"), NodeKind::Directory, 0);
        root.expanded = true;

        let file1 = TreeNode::new(
            "file1.txt".to_string(),
            PathBuf::from("/root/file1.txt"),
            NodeKind::File,
            1,
        );

        let mut dir1 =
            TreeNode::new("dir1".to_string(), PathBuf::from("/root/dir1"), NodeKind::Directory, 1);

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
