//! Tree state: data structures for file system tree representation and navigation.

use std::path::{
    Path,
    PathBuf,
};
use std::time::SystemTime;

use serde::{
    Deserialize,
    Serialize,
};

/// A file or directory node in the tree.
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// File name (for display).
    pub name: String,
    /// Absolute path.
    pub path: PathBuf,
    /// Whether this node is a directory.
    pub is_dir: bool,
    /// Whether this node is a symbolic link.
    pub is_symlink: bool,
    /// File size in bytes.
    pub size: u64,
    /// Last modification time.
    pub modified: Option<SystemTime>,
    /// Children loading state (only meaningful for directories).
    pub children: ChildrenState,
    /// Whether this directory is expanded (only meaningful for directories).
    pub is_expanded: bool,
}

/// Loading state for a directory's children.
#[derive(Debug, Clone)]
pub enum ChildrenState {
    /// Children have not been loaded yet.
    NotLoaded,
    /// Children are currently being loaded.
    Loading,
    /// Children have been loaded successfully.
    Loaded(Vec<TreeNode>),
}

/// Sort order field.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    /// Sort by name.
    #[default]
    Name,
    /// Sort by file size.
    Size,
    /// Sort by modification time.
    Modified,
    /// Sort by file type (directory vs file).
    Type,
    /// Sort by file extension.
    Extension,
}

impl From<crate::config::SortOrder> for SortOrder {
    fn from(order: crate::config::SortOrder) -> Self {
        match order {
            crate::config::SortOrder::Name => Self::Name,
            crate::config::SortOrder::Size => Self::Size,
            crate::config::SortOrder::Mtime => Self::Modified,
            crate::config::SortOrder::Type => Self::Type,
            crate::config::SortOrder::Extension => Self::Extension,
        }
    }
}

/// Sort direction.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    /// Ascending order.
    #[default]
    Asc,
    /// Descending order.
    Desc,
}

impl From<crate::config::SortDirection> for SortDirection {
    fn from(direction: crate::config::SortDirection) -> Self {
        match direction {
            crate::config::SortDirection::Asc => Self::Asc,
            crate::config::SortDirection::Desc => Self::Desc,
        }
    }
}

/// A visible node for UI rendering — a flattened reference into the tree.
#[derive(Debug)]
pub struct VisibleNode<'a> {
    /// Reference to the tree node.
    pub node: &'a TreeNode,
    /// Indentation depth (0 = root's children).
    pub depth: usize,
}

/// Serializable node information for IPC / Lua communication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Absolute path.
    pub path: PathBuf,
    /// File name.
    pub name: String,
    /// Whether this is a directory.
    pub is_dir: bool,
    /// Whether this is a symbolic link.
    pub is_symlink: bool,
    /// File size in bytes.
    pub size: u64,
    /// Last modification time.
    pub modified: Option<SystemTime>,
}

/// Tree state: manages the root node, cursor, and sort settings.
#[derive(Debug)]
pub struct TreeState {
    /// Root node of the tree.
    root: TreeNode,
    /// Cursor position (index into visible nodes).
    cursor: usize,
    /// Current sort order.
    sort_order: SortOrder,
    /// Current sort direction.
    sort_direction: SortDirection,
    /// Whether directories should appear before files.
    directories_first: bool,
}

impl TreeNode {
    /// Convert this node into a serializable `NodeInfo`.
    pub fn to_node_info(&self) -> NodeInfo {
        NodeInfo {
            path: self.path.clone(),
            name: self.name.clone(),
            is_dir: self.is_dir,
            is_symlink: self.is_symlink,
            size: self.size,
            modified: self.modified,
        }
    }
}

impl ChildrenState {
    /// Returns the loaded children, if available.
    pub fn as_loaded(&self) -> Option<&[TreeNode]> {
        match self {
            Self::Loaded(children) => Some(children),
            Self::NotLoaded | Self::Loading => None,
        }
    }

    /// Returns a mutable reference to loaded children, if available.
    pub const fn as_loaded_mut(&mut self) -> Option<&mut Vec<TreeNode>> {
        match self {
            Self::Loaded(children) => Some(children),
            Self::NotLoaded | Self::Loading => None,
        }
    }
}

impl TreeState {
    /// Create a new tree state from a root node and sort settings.
    pub const fn new(
        root: TreeNode,
        sort_order: SortOrder,
        sort_direction: SortDirection,
        directories_first: bool,
    ) -> Self {
        Self { root, cursor: 0, sort_order, sort_direction, directories_first }
    }

    /// Get a reference to the root node.
    #[expect(dead_code, reason = "Public API for future use")]
    pub const fn root(&self) -> &TreeNode {
        &self.root
    }

    /// Get a mutable reference to the root node.
    #[expect(dead_code, reason = "Public API for future use")]
    pub const fn root_mut(&mut self) -> &mut TreeNode {
        &mut self.root
    }

    /// Get the current cursor position.
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Get the current sort order.
    #[expect(dead_code, reason = "Public API for future use")]
    pub const fn sort_order(&self) -> SortOrder {
        self.sort_order
    }

    /// Get the current sort direction.
    #[expect(dead_code, reason = "Public API for future use")]
    pub const fn sort_direction(&self) -> SortDirection {
        self.sort_direction
    }

    /// Get whether directories come first.
    #[expect(dead_code, reason = "Public API for future use")]
    pub const fn directories_first(&self) -> bool {
        self.directories_first
    }

    /// Collect paths of all expanded directories in the tree.
    pub fn expanded_paths(&self) -> Vec<PathBuf> {
        let mut result = Vec::new();
        collect_expanded(&self.root, &mut result);
        result
    }

    /// Get the path of the node at the current cursor position.
    pub fn cursor_path(&self) -> Option<PathBuf> {
        self.current_node_info().map(|info| info.path)
    }

    /// Move the cursor to the node matching the given path.
    ///
    /// Searches visible nodes for a path match. If found, moves the cursor
    /// there. Returns `true` if the path was found and cursor moved.
    pub fn move_cursor_to_path(&mut self, path: &Path) -> bool {
        let visible = self.visible_nodes();
        if let Some(idx) = visible.iter().position(|vn| vn.node.path == path) {
            self.cursor = idx;
            return true;
        }
        false
    }

    /// Generate the flattened list of visible nodes by DFS walk.
    ///
    /// Only includes nodes that are in expanded + `Loaded` directories.
    pub fn visible_nodes(&self) -> Vec<VisibleNode<'_>> {
        let mut result = Vec::new();
        if let Some(children) = self.root.children.as_loaded() {
            for child in children {
                collect_visible(child, 0, &mut result);
            }
        }
        result
    }

    /// Count of visible nodes (without allocating the full list).
    pub fn visible_node_count(&self) -> usize {
        fn count_visible(node: &TreeNode) -> usize {
            1 + node
                .children
                .as_loaded()
                .filter(|_| node.is_expanded)
                .map_or(0, |children| children.iter().map(count_visible).sum())
        }

        self.root
            .children
            .as_loaded()
            .map_or(0, |children| children.iter().map(count_visible).sum())
    }

    /// Find the node at the given path in the tree (mutable).
    ///
    /// Walks from root, matching path components.
    fn find_node_mut(&mut self, path: &Path) -> Option<&mut TreeNode> {
        if self.root.path == path {
            return Some(&mut self.root);
        }
        find_node_recursive(&mut self.root, path)
    }

    /// Set loaded children for a directory at the given path.
    ///
    /// Applies current sort settings to the children.
    /// When `auto_expand` is true, the directory is automatically expanded (user-initiated loads).
    /// When false, the current `is_expanded` state is preserved (prefetch loads).
    pub fn set_children(&mut self, path: &Path, mut children: Vec<TreeNode>, auto_expand: bool) {
        let order = self.sort_order;
        let direction = self.sort_direction;
        let dirs_first = self.directories_first;

        crate::tree::sort::sort_children(&mut children, order, direction, dirs_first);

        if let Some(node) = self.find_node_mut(path) {
            node.children = ChildrenState::Loaded(children);
            if auto_expand {
                node.is_expanded = true;
            }
        }
    }

    /// Handle a filesystem change event for a directory.
    ///
    /// Returns `true` if the directory is expanded and loaded (caller should refresh).
    /// For collapsed + loaded directories, marks children as `NotLoaded` so they
    /// reload on next expand.
    pub fn handle_fs_change(&mut self, path: &Path) -> bool {
        let Some(node) = self.find_node_mut(path) else {
            return false;
        };
        let is_expanded = node.is_expanded;
        let is_loaded = matches!(node.children, ChildrenState::Loaded(_));

        if is_expanded && is_loaded {
            true
        } else if !is_expanded && is_loaded {
            node.children = ChildrenState::NotLoaded;
            false
        } else {
            false
        }
    }

    /// Prepare prefetching for child directories at the given path.
    ///
    /// For each child directory with `NotLoaded` children, transitions them
    /// to `Loading` state and returns their paths for background loading.
    pub fn start_prefetch(&mut self, path: &Path) -> Vec<PathBuf> {
        let Some(node) = self.find_node_mut(path) else {
            return Vec::new();
        };
        let Some(children) = node.children.as_loaded_mut() else {
            return Vec::new();
        };

        children
            .iter_mut()
            .filter(|c| c.is_dir && matches!(c.children, ChildrenState::NotLoaded))
            .map(|c| {
                c.children = ChildrenState::Loading;
                c.path.clone()
            })
            .collect()
    }

    /// Revert a directory to `NotLoaded` state (e.g., on load error).
    pub fn set_children_error(&mut self, path: &Path) {
        if let Some(node) = self.find_node_mut(path) {
            node.children = ChildrenState::NotLoaded;
            node.is_expanded = false;
        }
    }

    /// Toggle the expand/collapse state of the node at the given visible index.
    ///
    /// Returns an [`ExpandResult`] indicating what happened:
    /// - `NeedsLoad` if children need to be loaded from disk
    /// - `AlreadyLoaded` if the directory was expanded with pre-loaded children
    /// - `None` if collapsed, not a directory, or already loading
    pub fn toggle_expand(&mut self, index: usize) -> Option<ExpandResult> {
        let visible = self.visible_nodes();
        let vnode = visible.get(index)?;
        let path = vnode.node.path.clone();
        let is_dir = vnode.node.is_dir;
        let is_expanded = vnode.node.is_expanded;

        if !is_dir {
            return None;
        }

        let node = self.find_node_mut(&path)?;

        if is_expanded {
            node.is_expanded = false;
            None
        } else {
            node.is_expanded = true;
            match &node.children {
                ChildrenState::NotLoaded => {
                    node.children = ChildrenState::Loading;
                    Some(ExpandResult::NeedsLoad(path))
                }
                ChildrenState::Loaded(_) => Some(ExpandResult::AlreadyLoaded(path)),
                ChildrenState::Loading => None,
            }
        }
    }

    /// Move cursor by a signed delta with bounds checking.
    pub fn move_cursor(&mut self, delta: i32) {
        let count = self.visible_node_count();
        if count == 0 {
            self.cursor = 0;
            return;
        }
        if delta >= 0 {
            self.cursor = self.cursor.saturating_add(delta.unsigned_abs() as usize).min(count - 1);
        } else {
            self.cursor = self.cursor.saturating_sub(delta.unsigned_abs() as usize);
        }
    }

    /// Move cursor to a specific index with bounds checking.
    #[cfg_attr(not(test), expect(dead_code, reason = "Public API for future use"))]
    pub fn move_cursor_to(&mut self, index: usize) {
        let count = self.visible_node_count();
        if count == 0 {
            self.cursor = 0;
        } else {
            self.cursor = index.min(count - 1);
        }
    }

    /// Jump cursor to the first visible node.
    pub const fn jump_to_first(&mut self) {
        self.cursor = 0;
    }

    /// Jump cursor to the last visible node.
    pub fn jump_to_last(&mut self) {
        let count = self.visible_node_count();
        self.cursor = count.saturating_sub(1);
    }

    /// Move cursor half a page down.
    pub fn half_page_down(&mut self, viewport_height: usize) {
        let delta = viewport_height / 2;
        let count = self.visible_node_count();
        if count == 0 {
            self.cursor = 0;
            return;
        }
        self.cursor = (self.cursor + delta).min(count - 1);
    }

    /// Move cursor half a page up.
    pub const fn half_page_up(&mut self, viewport_height: usize) {
        let delta = viewport_height / 2;
        self.cursor = self.cursor.saturating_sub(delta);
    }

    /// Collapse: if on expanded directory, collapse it.
    /// If on file or collapsed directory, move cursor to parent.
    ///
    /// Returns `true` if state changed.
    pub fn collapse(&mut self) -> bool {
        let visible = self.visible_nodes();
        let Some(vnode) = visible.get(self.cursor) else {
            return false;
        };
        let path = vnode.node.path.clone();
        let is_dir = vnode.node.is_dir;
        let is_expanded = vnode.node.is_expanded;

        if is_dir
            && is_expanded
            && let Some(node) = self.find_node_mut(&path)
        {
            node.is_expanded = false;
            return true;
        }

        // Move to parent directory
        if let Some(parent_path) = path.parent()
            && let Some(idx) =
                self.visible_nodes().iter().position(|vn| vn.node.path == parent_path)
        {
            self.cursor = idx;
            return true;
        }

        false
    }

    /// Expand a directory or signal that a file should be opened.
    ///
    /// Returns:
    /// - `Some(ExpandResult::NeedsLoad(path))` if children need to be loaded
    /// - `Some(ExpandResult::OpenFile(path))` if the node is a file
    /// - `None` if already expanded or nothing to do
    pub fn expand_or_open(&mut self) -> Option<ExpandResult> {
        let visible = self.visible_nodes();
        let vnode = visible.get(self.cursor)?;
        let path = vnode.node.path.clone();
        let is_dir = vnode.node.is_dir;

        if !is_dir {
            return Some(ExpandResult::OpenFile(path));
        }

        let node = self.find_node_mut(&path)?;
        if node.is_expanded {
            return None;
        }

        node.is_expanded = true;
        match &node.children {
            ChildrenState::NotLoaded => {
                node.children = ChildrenState::Loading;
                Some(ExpandResult::NeedsLoad(path))
            }
            ChildrenState::Loaded(_) => Some(ExpandResult::AlreadyLoaded(path)),
            ChildrenState::Loading => None,
        }
    }

    /// Get serializable info for the node at the current cursor position.
    pub fn current_node_info(&self) -> Option<NodeInfo> {
        let visible = self.visible_nodes();
        visible.get(self.cursor).map(|vn| vn.node.to_node_info())
    }

    /// Get the directory path at the current cursor position.
    ///
    /// If the cursor is on a directory, returns its path.
    /// If the cursor is on a file, returns the parent directory path.
    pub fn cursor_dir_path(&self) -> Option<PathBuf> {
        let visible = self.visible_nodes();
        let vnode = visible.get(self.cursor)?;
        if vnode.node.is_dir {
            Some(vnode.node.path.clone())
        } else {
            vnode.node.path.parent().map(Path::to_path_buf)
        }
    }

    /// Expand all directories under the given path recursively, up to `limit`.
    ///
    /// Sets `is_expanded = true` for each directory and transitions `NotLoaded`
    /// children to `Loading`. Returns an [`ExpandAllResult`] with the count of
    /// expanded directories, paths needing async load, and whether the limit
    /// was reached.
    pub fn expand_subtree(&mut self, path: &Path, limit: usize) -> ExpandAllResult {
        let Some(node) = self.find_node_mut(path) else {
            return ExpandAllResult { expanded: 0, needs_load: Vec::new(), hit_limit: false };
        };
        let mut needs_load = Vec::new();
        let expanded = expand_subtree_recursive(node, limit, &mut needs_load);
        let hit_limit = expanded >= limit;
        ExpandAllResult { expanded, needs_load, hit_limit }
    }

    /// Collapse all directories under the given path (including itself).
    ///
    /// Returns paths of directories that were expanded (for unwatching).
    pub fn collapse_subtree(&mut self, path: &Path) -> Vec<PathBuf> {
        let Some(node) = self.find_node_mut(path) else {
            return Vec::new();
        };
        let mut collapsed = Vec::new();
        collapse_subtree_recursive(node, &mut collapsed);
        // Clamp cursor to valid range after collapse.
        let count = self.visible_node_count();
        if count == 0 {
            self.cursor = 0;
        } else {
            self.cursor = self.cursor.min(count - 1);
        }
        collapsed
    }

    /// Apply sort settings and re-sort all loaded children.
    pub fn apply_sort(
        &mut self,
        order: SortOrder,
        direction: SortDirection,
        directories_first: bool,
    ) {
        self.sort_order = order;
        self.sort_direction = direction;
        self.directories_first = directories_first;
        crate::tree::sort::apply_sort_recursive(
            &mut self.root,
            order,
            direction,
            directories_first,
        );
    }
}

/// Result of an expand-all operation on a subtree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandAllResult {
    /// Number of directories expanded.
    pub expanded: usize,
    /// Paths of directories that need loading (transitioned `NotLoaded` → `Loading`).
    pub needs_load: Vec<PathBuf>,
    /// Whether the expansion limit was reached.
    pub hit_limit: bool,
}

/// Result of an expand-or-open operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpandResult {
    /// The directory needs its children loaded from the filesystem.
    NeedsLoad(PathBuf),
    /// The node is a file and should be opened.
    OpenFile(PathBuf),
    /// The directory was expanded and its children are already loaded (e.g., via prefetch).
    AlreadyLoaded(PathBuf),
}

/// Recursively collect visible nodes via DFS.
fn collect_visible<'a>(node: &'a TreeNode, depth: usize, result: &mut Vec<VisibleNode<'a>>) {
    result.push(VisibleNode { node, depth });
    if node.is_expanded
        && let Some(children) = node.children.as_loaded()
    {
        for child in children {
            collect_visible(child, depth + 1, result);
        }
    }
}

/// Recursively collect paths of all expanded directories via DFS.
fn collect_expanded(node: &TreeNode, result: &mut Vec<PathBuf>) {
    if node.is_dir && node.is_expanded {
        result.push(node.path.clone());
    }
    if let Some(children) = node.children.as_loaded() {
        for child in children {
            collect_expanded(child, result);
        }
    }
}

/// Recursively expand directories in a subtree, up to `remaining` limit.
///
/// Returns the number of directories expanded.
fn expand_subtree_recursive(
    node: &mut TreeNode,
    remaining: usize,
    needs_load: &mut Vec<PathBuf>,
) -> usize {
    if !node.is_dir || remaining == 0 {
        return 0;
    }

    let mut count = 0;

    // Expand this directory if not already expanded.
    if !node.is_expanded {
        node.is_expanded = true;
        count += 1;

        // Transition NotLoaded → Loading.
        if matches!(node.children, ChildrenState::NotLoaded) {
            node.children = ChildrenState::Loading;
            needs_load.push(node.path.clone());
        }
    }

    // Recurse into loaded children.
    if let Some(children) = node.children.as_loaded_mut() {
        for child in children.iter_mut() {
            if count >= remaining {
                break;
            }
            count += expand_subtree_recursive(child, remaining - count, needs_load);
        }
    }

    count
}

/// Recursively collapse directories in a subtree.
fn collapse_subtree_recursive(node: &mut TreeNode, collapsed: &mut Vec<PathBuf>) {
    if node.is_dir && node.is_expanded {
        collapsed.push(node.path.clone());
        node.is_expanded = false;
    }
    if let Some(children) = node.children.as_loaded_mut() {
        for child in children.iter_mut() {
            collapse_subtree_recursive(child, collapsed);
        }
    }
}

/// Recursively search for a node by path.
fn find_node_recursive<'a>(node: &'a mut TreeNode, path: &Path) -> Option<&'a mut TreeNode> {
    node.children.as_loaded_mut()?.iter_mut().find_map(|child| {
        if child.path == path {
            Some(child)
        } else if path.starts_with(&child.path) {
            find_node_recursive(child, path)
        } else {
            None
        }
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use googletest::prelude::*;
    use rstest::*;

    use super::*;

    /// Helper: create a file node.
    fn file_node(name: &str, parent: &Path) -> TreeNode {
        TreeNode {
            name: name.to_string(),
            path: parent.join(name),
            is_dir: false,
            is_symlink: false,
            size: 100,
            modified: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
        }
    }

    /// Helper: create a directory node with `Loaded` children.
    fn dir_node(name: &str, parent: &Path, children: Vec<TreeNode>) -> TreeNode {
        TreeNode {
            name: name.to_string(),
            path: parent.join(name),
            is_dir: true,
            is_symlink: false,
            size: 0,
            modified: None,
            children: ChildrenState::Loaded(children),
            is_expanded: false,
        }
    }

    /// Helper: create a root node with given children.
    fn root_with_children(children: Vec<TreeNode>) -> TreeNode {
        TreeNode {
            name: "root".to_string(),
            path: PathBuf::from("/test/root"),
            is_dir: true,
            is_symlink: false,
            size: 0,
            modified: None,
            children: ChildrenState::Loaded(children),
            is_expanded: true,
        }
    }

    /// Helper: create a default `TreeState` from children.
    fn state_with_children(children: Vec<TreeNode>) -> TreeState {
        TreeState::new(root_with_children(children), SortOrder::Name, SortDirection::Asc, true)
    }

    // --- NodeInfo ---

    #[rstest]
    fn test_to_node_info() -> Result<()> {
        let node = file_node("test.txt", Path::new("/root"));
        let info = node.to_node_info();
        verify_that!(info.name, eq("test.txt"))?;
        verify_that!(info.is_dir, eq(false))?;
        Ok(())
    }

    // --- ChildrenState ---

    #[rstest]
    fn test_children_state_as_loaded() -> Result<()> {
        let not_loaded = ChildrenState::NotLoaded;
        verify_that!(not_loaded.as_loaded().is_some(), eq(false))?;

        let loading = ChildrenState::Loading;
        verify_that!(loading.as_loaded().is_some(), eq(false))?;

        let loaded = ChildrenState::Loaded(vec![]);
        verify_that!(loaded.as_loaded().is_some(), eq(true))?;
        Ok(())
    }

    // =========================================================================
    // US4: Visible Nodes
    // =========================================================================

    #[rstest]
    fn test_visible_nodes_root_only_expanded() -> Result<()> {
        let root = Path::new("/test/root");
        let state = state_with_children(vec![file_node("a.txt", root), file_node("b.txt", root)]);
        let visible = state.visible_nodes();
        verify_that!(visible.len(), eq(2))?;
        verify_that!(visible[0].depth, eq(0))?;
        verify_that!(visible[1].depth, eq(0))?;
        Ok(())
    }

    #[rstest]
    fn test_visible_nodes_expanded_subdir_includes_children() -> Result<()> {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let mut subdir = dir_node("subdir", root, vec![file_node("child.txt", &subdir_path)]);
        subdir.is_expanded = true;

        let state = state_with_children(vec![file_node("a.txt", root), subdir]);
        let visible = state.visible_nodes();
        // a.txt, subdir, child.txt
        verify_that!(visible.len(), eq(3))?;
        verify_that!(visible[0].node.name.as_str(), eq("a.txt"))?;
        verify_that!(visible[0].depth, eq(0))?;
        verify_that!(visible[1].node.name.as_str(), eq("subdir"))?;
        verify_that!(visible[1].depth, eq(0))?;
        verify_that!(visible[2].node.name.as_str(), eq("child.txt"))?;
        verify_that!(visible[2].depth, eq(1))?;
        Ok(())
    }

    #[rstest]
    fn test_visible_nodes_collapsed_dir_excludes_children() -> Result<()> {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let subdir = dir_node("subdir", root, vec![file_node("child.txt", &subdir_path)]);
        // subdir.is_expanded = false (default from dir_node)

        let state = state_with_children(vec![subdir]);
        let visible = state.visible_nodes();
        // Only subdir itself, no children
        verify_that!(visible.len(), eq(1))?;
        verify_that!(visible[0].node.name.as_str(), eq("subdir"))?;
        Ok(())
    }

    #[rstest]
    fn test_visible_nodes_not_loaded_dir_shows_self_only() -> Result<()> {
        let root = Path::new("/test/root");
        let not_loaded_dir = TreeNode {
            name: "empty_dir".to_string(),
            path: root.join("empty_dir"),
            is_dir: true,
            is_symlink: false,
            size: 0,
            modified: None,
            children: ChildrenState::NotLoaded,
            is_expanded: true, // expanded but NotLoaded
        };
        // Even though expanded, no children to show
        let state = state_with_children(vec![not_loaded_dir]);
        let visible = state.visible_nodes();
        verify_that!(visible.len(), eq(1))?;
        verify_that!(visible[0].node.name.as_str(), eq("empty_dir"))?;
        Ok(())
    }

    // =========================================================================
    // US2: Lazy Loading (set_children, toggle_expand, set_children_error)
    // =========================================================================

    #[rstest]
    fn test_set_children_loads_and_reflects_in_visible() -> Result<()> {
        let root = Path::new("/test/root");
        let subdir = TreeNode {
            name: "subdir".to_string(),
            path: root.join("subdir"),
            is_dir: true,
            is_symlink: false,
            size: 0,
            modified: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
        };
        let mut state = state_with_children(vec![subdir]);

        // Before: only subdir visible
        verify_that!(state.visible_node_count(), eq(1))?;

        // set_children loads children and expands
        let subdir_path = root.join("subdir");
        state.set_children(
            &subdir_path,
            vec![file_node("child1.txt", &subdir_path), file_node("child2.txt", &subdir_path)],
            true,
        );

        let visible = state.visible_nodes();
        // subdir + 2 children
        verify_that!(visible.len(), eq(3))?;
        verify_that!(visible[0].node.name.as_str(), eq("subdir"))?;
        Ok(())
    }

    #[rstest]
    fn test_toggle_expand_collapses_expanded_dir() -> Result<()> {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let mut subdir = dir_node("subdir", root, vec![file_node("child.txt", &subdir_path)]);
        subdir.is_expanded = true;

        let mut state = state_with_children(vec![subdir]);
        verify_that!(state.visible_node_count(), eq(2))?; // subdir + child

        // Toggle at index 0 (subdir) should collapse
        let result = state.toggle_expand(0);
        verify_that!(result.is_none(), eq(true))?; // No load needed

        verify_that!(state.visible_node_count(), eq(1))?; // only subdir
        Ok(())
    }

    #[rstest]
    fn test_toggle_expand_reexpands_loaded_dir_without_reload() -> Result<()> {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let subdir = dir_node("subdir", root, vec![file_node("child.txt", &subdir_path)]);
        // Collapsed but Loaded

        let mut state = state_with_children(vec![subdir]);
        verify_that!(state.visible_node_count(), eq(1))?;

        // Toggle should expand, return AlreadyLoaded (no disk load needed)
        let result = state.toggle_expand(0);
        assert_eq!(result, Some(ExpandResult::AlreadyLoaded(root.join("subdir"))));

        verify_that!(state.visible_node_count(), eq(2))?; // subdir + child
        Ok(())
    }

    #[rstest]
    fn test_set_children_error_reverts_to_not_loaded() -> Result<()> {
        let root = Path::new("/test/root");
        let subdir = TreeNode {
            name: "subdir".to_string(),
            path: root.join("subdir"),
            is_dir: true,
            is_symlink: false,
            size: 0,
            modified: None,
            children: ChildrenState::Loading,
            is_expanded: true,
        };
        let mut state = state_with_children(vec![subdir]);

        state.set_children_error(&root.join("subdir"));

        let visible = state.visible_nodes();
        verify_that!(visible.len(), eq(1))?;
        let node = visible[0].node;
        verify_that!(node.children.as_loaded().is_some(), eq(false))?;
        verify_that!(node.is_expanded, eq(false))?;
        Ok(())
    }

    // =========================================================================
    // Prefetch
    // =========================================================================

    #[rstest]
    fn test_set_children_no_auto_expand_preserves_collapsed() -> Result<()> {
        let root = Path::new("/test/root");
        let subdir = TreeNode {
            name: "subdir".to_string(),
            path: root.join("subdir"),
            is_dir: true,
            is_symlink: false,
            size: 0,
            modified: None,
            children: ChildrenState::Loading,
            is_expanded: false, // collapsed (prefetch scenario)
        };
        let mut state = state_with_children(vec![subdir]);

        let subdir_path = root.join("subdir");
        state.set_children(&subdir_path, vec![file_node("child.txt", &subdir_path)], false);

        // Children loaded but directory stays collapsed
        let visible = state.visible_nodes();
        verify_that!(visible.len(), eq(1))?; // only subdir visible
        let node = visible[0].node;
        verify_that!(node.children.as_loaded().is_some(), eq(true))?;
        verify_that!(node.is_expanded, eq(false))?;
        Ok(())
    }

    #[rstest]
    fn test_start_prefetch_transitions_not_loaded_dirs() -> Result<()> {
        let root = Path::new("/test/root");
        let subdir_a = TreeNode {
            name: "dir_a".to_string(),
            path: root.join("dir_a"),
            is_dir: true,
            is_symlink: false,
            size: 0,
            modified: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
        };
        let subdir_b = TreeNode {
            name: "dir_b".to_string(),
            path: root.join("dir_b"),
            is_dir: true,
            is_symlink: false,
            size: 0,
            modified: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
        };
        let mut state = state_with_children(vec![subdir_a, subdir_b, file_node("file.txt", root)]);

        let paths = state.start_prefetch(&PathBuf::from("/test/root"));
        // Only directories with NotLoaded, not files
        verify_that!(paths.len(), eq(2))?;
        verify_that!(paths.contains(&root.join("dir_a")), eq(true))?;
        verify_that!(paths.contains(&root.join("dir_b")), eq(true))?;

        // Verify they are now Loading
        let visible = state.visible_nodes();
        assert!(matches!(visible[0].node.children, ChildrenState::Loading));
        assert!(matches!(visible[1].node.children, ChildrenState::Loading));
        Ok(())
    }

    #[rstest]
    fn test_start_prefetch_skips_already_loaded_and_loading() -> Result<()> {
        let root = Path::new("/test/root");
        let loaded_dir = dir_node("loaded", root, vec![]);
        let loading_dir = TreeNode {
            name: "loading".to_string(),
            path: root.join("loading"),
            is_dir: true,
            is_symlink: false,
            size: 0,
            modified: None,
            children: ChildrenState::Loading,
            is_expanded: false,
        };
        let mut state = state_with_children(vec![loaded_dir, loading_dir]);

        let paths = state.start_prefetch(&PathBuf::from("/test/root"));
        verify_that!(paths.len(), eq(0))?;
        Ok(())
    }

    #[rstest]
    fn test_expand_or_open_already_loaded_returns_already_loaded() {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let subdir = dir_node("subdir", root, vec![file_node("child.txt", &subdir_path)]);
        // Collapsed but Loaded (prefetched)
        let mut state = state_with_children(vec![subdir]);
        let result = state.expand_or_open();
        assert_eq!(result, Some(ExpandResult::AlreadyLoaded(root.join("subdir"))));
        // Now expanded with children visible
        assert_eq!(state.visible_node_count(), 2);
    }

    // =========================================================================
    // US5: Cursor Management
    // =========================================================================

    #[rstest]
    fn test_move_cursor_at_top_stays_at_top() -> Result<()> {
        let root = Path::new("/test/root");
        let mut state =
            state_with_children(vec![file_node("a.txt", root), file_node("b.txt", root)]);
        state.move_cursor(-1);
        verify_that!(state.cursor(), eq(0))?;
        Ok(())
    }

    #[rstest]
    fn test_move_cursor_at_bottom_stays_at_bottom() -> Result<()> {
        let root = Path::new("/test/root");
        let mut state =
            state_with_children(vec![file_node("a.txt", root), file_node("b.txt", root)]);
        state.move_cursor_to(1); // last
        state.move_cursor(1);
        verify_that!(state.cursor(), eq(1))?;
        Ok(())
    }

    #[rstest]
    fn test_half_page_down() -> Result<()> {
        let root = Path::new("/test/root");
        let children: Vec<TreeNode> =
            (0..50).map(|i| file_node(&format!("file{i:03}.txt"), root)).collect();
        let mut state = state_with_children(children);

        state.half_page_down(20); // move 10
        verify_that!(state.cursor(), eq(10))?;
        Ok(())
    }

    #[rstest]
    fn test_jump_to_first_and_last() -> Result<()> {
        let root = Path::new("/test/root");
        let children: Vec<TreeNode> =
            (0..10).map(|i| file_node(&format!("file{i}.txt"), root)).collect();
        let mut state = state_with_children(children);

        state.jump_to_last();
        verify_that!(state.cursor(), eq(9))?;

        state.jump_to_first();
        verify_that!(state.cursor(), eq(0))?;
        Ok(())
    }

    #[rstest]
    fn test_collapse_on_expanded_dir_collapses() -> Result<()> {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let mut subdir = dir_node("subdir", root, vec![file_node("child.txt", &subdir_path)]);
        subdir.is_expanded = true;

        let mut state = state_with_children(vec![subdir]);
        // cursor is at 0 (subdir)
        let changed = state.collapse();
        verify_that!(changed, eq(true))?;
        verify_that!(state.visible_node_count(), eq(1))?;
        Ok(())
    }

    #[rstest]
    fn test_collapse_on_file_moves_to_parent() -> Result<()> {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let mut subdir = dir_node("subdir", root, vec![file_node("child.txt", &subdir_path)]);
        subdir.is_expanded = true;

        let mut state = state_with_children(vec![subdir]);
        // Move cursor to child.txt (index 1)
        state.move_cursor_to(1);
        verify_that!(state.cursor(), eq(1))?;

        let changed = state.collapse();
        verify_that!(changed, eq(true))?;
        // Cursor should have moved to parent (subdir at index 0)
        verify_that!(state.cursor(), eq(0))?;
        Ok(())
    }

    #[rstest]
    fn test_expand_or_open_on_file_returns_open() {
        let root = Path::new("/test/root");
        let mut state = state_with_children(vec![file_node("test.txt", root)]);
        let result = state.expand_or_open();
        assert_eq!(result, Some(ExpandResult::OpenFile(root.join("test.txt"))));
    }

    #[rstest]
    fn test_expand_or_open_on_not_loaded_dir_returns_needs_load() {
        let root = Path::new("/test/root");
        let subdir = TreeNode {
            name: "subdir".to_string(),
            path: root.join("subdir"),
            is_dir: true,
            is_symlink: false,
            size: 0,
            modified: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
        };
        let mut state = state_with_children(vec![subdir]);
        let result = state.expand_or_open();
        assert_eq!(result, Some(ExpandResult::NeedsLoad(root.join("subdir"))));
    }

    #[rstest]
    fn test_current_node_info() -> Result<()> {
        let root = Path::new("/test/root");
        let state = state_with_children(vec![file_node("test.txt", root)]);
        let info = state.current_node_info().unwrap();
        verify_that!(info.name.as_str(), eq("test.txt"))?;
        verify_that!(info.is_dir, eq(false))?;
        Ok(())
    }

    // =========================================================================
    // Integration: TreeBuilder → sort → visible_nodes end-to-end
    // =========================================================================

    #[rstest]
    fn test_integration_builder_sort_visible_nodes() {
        use std::fs;

        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("charlie.txt"), "c").unwrap();
        fs::write(dir.path().join("alpha.txt"), "a").unwrap();
        fs::create_dir(dir.path().join("bravo_dir")).unwrap();
        fs::write(dir.path().join("bravo_dir").join("nested.txt"), "n").unwrap();

        let builder = crate::tree::builder::TreeBuilder::new(false, false);
        let root = builder.build(dir.path()).unwrap();

        let mut state = TreeState::new(root, SortOrder::Name, SortDirection::Asc, true);
        state.apply_sort(SortOrder::Name, SortDirection::Asc, true);

        let visible = state.visible_nodes();
        // directories_first: bravo_dir first, then alpha.txt, charlie.txt
        assert_eq!(visible.len(), 3);
        assert_eq!(visible[0].node.name, "bravo_dir");
        assert!(visible[0].node.is_dir);
        assert_eq!(visible[1].node.name, "alpha.txt");
        assert_eq!(visible[2].node.name, "charlie.txt");
        assert_eq!(visible[0].depth, 0);
    }

    // =========================================================================
    // Integration: TreeBuilder + set_children + cursor navigation end-to-end
    // =========================================================================

    #[rstest]
    fn test_integration_builder_set_children_cursor() {
        use std::fs;

        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file1.txt"), "f1").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        fs::write(dir.path().join("subdir").join("child.txt"), "c").unwrap();

        let builder = crate::tree::builder::TreeBuilder::new(false, false);
        let root = builder.build(dir.path()).unwrap();

        let mut state = TreeState::new(root, SortOrder::Name, SortDirection::Asc, true);
        state.apply_sort(SortOrder::Name, SortDirection::Asc, true);

        // Initially: subdir (dirs first), file1.txt
        assert_eq!(state.visible_node_count(), 2);

        // Expand subdir (index 0) — should need load
        let result = state.toggle_expand(0);
        assert!(matches!(result, Some(ExpandResult::NeedsLoad(_))));

        // Simulate loading children
        let subdir_path = state.visible_nodes()[0].node.path.clone();
        let loaded_children = builder.load_children(&subdir_path).unwrap();
        state.set_children(&subdir_path, loaded_children, true);

        // Now visible: subdir, child.txt, file1.txt
        assert_eq!(state.visible_node_count(), 3);

        // Navigate cursor down to child.txt
        state.move_cursor(1);
        assert_eq!(state.cursor(), 1);
        let info = state.current_node_info().unwrap();
        assert_eq!(info.name, "child.txt");

        // Collapse on file — cursor should move to parent subdir
        let changed = state.collapse();
        assert!(changed);
        assert_eq!(state.cursor(), 0);
        // Subdir is still expanded (collapse on file only moves cursor to parent)
        assert_eq!(state.visible_node_count(), 3);

        // Collapse on expanded dir — actually collapses it
        let changed = state.collapse();
        assert!(changed);
        assert_eq!(state.visible_node_count(), 2);
    }

    // =========================================================================
    // FS Change Detection: handle_fs_change
    // =========================================================================

    #[rstest]
    fn handle_fs_change_expanded_loaded_returns_true() {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let mut subdir = dir_node("subdir", root, vec![file_node("child.txt", &subdir_path)]);
        subdir.is_expanded = true;

        let mut state = state_with_children(vec![subdir]);
        // Expanded + Loaded → returns true (caller should refresh)
        assert_that!(state.handle_fs_change(&subdir_path), eq(true));
    }

    #[rstest]
    fn handle_fs_change_collapsed_loaded_invalidates() {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let subdir = dir_node("subdir", root, vec![file_node("child.txt", &subdir_path)]);
        // Collapsed but Loaded (prefetched)

        let mut state = state_with_children(vec![subdir]);
        // Collapsed + Loaded → returns false, sets to NotLoaded
        assert_that!(state.handle_fs_change(&subdir_path), eq(false));

        // Verify children are now NotLoaded
        let visible = state.visible_nodes();
        assert!(matches!(visible[0].node.children, ChildrenState::NotLoaded));
    }

    #[rstest]
    fn handle_fs_change_not_loaded_is_noop() {
        let root = Path::new("/test/root");
        let subdir = TreeNode {
            name: "subdir".to_string(),
            path: root.join("subdir"),
            is_dir: true,
            is_symlink: false,
            size: 0,
            modified: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
        };
        let mut state = state_with_children(vec![subdir]);
        assert_that!(state.handle_fs_change(&root.join("subdir")), eq(false));
    }

    #[rstest]
    fn handle_fs_change_unknown_path_returns_false() {
        let root = Path::new("/test/root");
        let mut state = state_with_children(vec![file_node("a.txt", root)]);
        assert_that!(state.handle_fs_change(&root.join("nonexistent")), eq(false));
    }

    // =========================================================================
    // Performance: visible_nodes < 16ms @ 10k entries
    // =========================================================================

    #[rstest]
    #[ignore = "Performance test — run with `cargo test -- --ignored`"]
    fn test_perf_visible_nodes_10k_entries() {
        let root_path = Path::new("/perf/root");

        // Build a flat tree with 10k entries
        let children: Vec<TreeNode> =
            (0..10_000).map(|i| file_node(&format!("file{i:05}.txt"), root_path)).collect();
        let state = state_with_children(children);

        let start = std::time::Instant::now();
        let _visible = state.visible_nodes();
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() < 16,
            "visible_nodes took {}ms (limit: 16ms)",
            elapsed.as_millis()
        );
    }

    #[rstest]
    #[ignore = "Performance test — run with `cargo test -- --ignored`"]
    fn test_perf_visible_nodes_100k_flat() {
        let root_path = Path::new("/perf/root");

        let children: Vec<TreeNode> =
            (0..100_000).map(|i| file_node(&format!("file{i:06}.txt"), root_path)).collect();
        let state = state_with_children(children);

        // Warmup.
        let _ = state.visible_nodes();

        let start = std::time::Instant::now();
        let visible = state.visible_nodes();
        let elapsed = start.elapsed();

        eprintln!(
            "visible_nodes 100k flat: {:?} ({} nodes)",
            elapsed,
            visible.len()
        );
    }

    #[rstest]
    #[ignore = "Performance test — run with `cargo test -- --ignored`"]
    fn test_perf_visible_nodes_100k_nested() {
        let root_path = Path::new("/perf/root");

        // 100 dirs × 1000 files = 100,100 visible nodes.
        let children: Vec<TreeNode> = (0..100)
            .map(|d| {
                let dir_path = root_path.join(format!("dir{d:03}"));
                let files: Vec<TreeNode> = (0..1000)
                    .map(|f| file_node(&format!("file{f:04}.txt"), &dir_path))
                    .collect();
                let mut d = dir_node(&format!("dir{d:03}"), root_path, files);
                d.is_expanded = true;
                d
            })
            .collect();
        let state = state_with_children(children);

        // Warmup.
        let _ = state.visible_nodes();

        let start = std::time::Instant::now();
        let visible = state.visible_nodes();
        let elapsed = start.elapsed();

        eprintln!(
            "visible_nodes 100k nested: {:?} ({} nodes)",
            elapsed,
            visible.len()
        );
    }

    #[rstest]
    #[ignore = "Performance test — run with `cargo test -- --ignored`"]
    fn test_perf_visible_node_count_100k() {
        let root_path = Path::new("/perf/root");

        let children: Vec<TreeNode> =
            (0..100_000).map(|i| file_node(&format!("file{i:06}.txt"), root_path)).collect();
        let state = state_with_children(children);

        // Warmup.
        let _ = state.visible_node_count();

        let start = std::time::Instant::now();
        let count = state.visible_node_count();
        let elapsed = start.elapsed();

        eprintln!("visible_node_count 100k: {:?} ({count} nodes)", elapsed);
    }

    #[rstest]
    #[ignore = "Performance test — run with `cargo test -- --ignored`"]
    fn test_perf_expand_subtree_1000_dirs() {
        let root_path = Path::new("/perf/root");

        // 1000 dirs, each with 10 files + 2 subdirs (loaded).
        let children: Vec<TreeNode> = (0..1000)
            .map(|d| {
                let dir_path = root_path.join(format!("dir{d:04}"));
                let mut sub: Vec<TreeNode> = (0..10)
                    .map(|f| file_node(&format!("file{f:02}.txt"), &dir_path))
                    .collect();
                sub.push(dir_node("sub_a", &dir_path, vec![]));
                sub.push(dir_node("sub_b", &dir_path, vec![]));
                dir_node(&format!("dir{d:04}"), root_path, sub)
            })
            .collect();
        let mut state = state_with_children(children);

        let start = std::time::Instant::now();
        let result = state.expand_subtree(root_path, 5000);
        let elapsed = start.elapsed();

        eprintln!(
            "expand_subtree 1000 dirs: {:?} (expanded: {}, needs_load: {})",
            elapsed,
            result.expanded,
            result.needs_load.len()
        );
    }

    // =========================================================================
    // Session Persistence helpers
    // =========================================================================

    #[rstest]
    fn expanded_paths_collects_expanded_dirs() {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let mut subdir = dir_node("subdir", root, vec![file_node("child.txt", &subdir_path)]);
        subdir.is_expanded = true;
        let state = state_with_children(vec![subdir, file_node("a.txt", root)]);

        let mut paths = state.expanded_paths();
        paths.sort();
        // Root is expanded + subdir is expanded.
        assert_eq!(paths, vec![PathBuf::from("/test/root"), subdir_path]);
    }

    #[rstest]
    fn expanded_paths_skips_collapsed_dirs() {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let subdir = dir_node("subdir", root, vec![file_node("child.txt", &subdir_path)]);
        // subdir is collapsed (default from dir_node helper).
        let state = state_with_children(vec![subdir]);

        let paths = state.expanded_paths();
        // Only root is expanded.
        assert_eq!(paths, vec![PathBuf::from("/test/root")]);
    }

    #[rstest]
    fn cursor_path_returns_current_node_path() {
        let root = Path::new("/test/root");
        let mut state =
            state_with_children(vec![file_node("a.txt", root), file_node("b.txt", root)]);
        state.move_cursor_to(1);
        assert_eq!(state.cursor_path(), Some(root.join("b.txt")));
    }

    #[rstest]
    fn move_cursor_to_path_finds_node() {
        let root = Path::new("/test/root");
        let mut state = state_with_children(vec![
            file_node("a.txt", root),
            file_node("b.txt", root),
            file_node("c.txt", root),
        ]);
        assert_that!(state.move_cursor_to_path(&root.join("c.txt")), eq(true));
        assert_that!(state.cursor(), eq(2));
    }

    #[rstest]
    fn move_cursor_to_path_returns_false_for_missing() {
        let root = Path::new("/test/root");
        let mut state = state_with_children(vec![file_node("a.txt", root)]);
        assert_that!(state.move_cursor_to_path(&root.join("nonexistent.txt")), eq(false));
        assert_that!(state.cursor(), eq(0));
    }

    // =========================================================================
    // ExpandAll / CollapseAll
    // =========================================================================

    /// Helper: create a directory node with `NotLoaded` children.
    fn not_loaded_dir(name: &str, parent: &Path) -> TreeNode {
        TreeNode {
            name: name.to_string(),
            path: parent.join(name),
            is_dir: true,
            is_symlink: false,
            size: 0,
            modified: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
        }
    }

    #[rstest]
    fn cursor_dir_path_on_dir() {
        let root = Path::new("/test/root");
        let subdir = dir_node("subdir", root, vec![]);
        let state = state_with_children(vec![subdir, file_node("a.txt", root)]);
        // Cursor at index 0 = subdir.
        assert_eq!(state.cursor_dir_path(), Some(root.join("subdir")));
    }

    #[rstest]
    fn cursor_dir_path_on_file() {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let mut subdir = dir_node("subdir", root, vec![file_node("child.txt", &subdir_path)]);
        subdir.is_expanded = true;

        let mut state = state_with_children(vec![subdir]);
        state.move_cursor_to(1); // child.txt
        // File → returns parent dir path.
        assert_eq!(state.cursor_dir_path(), Some(subdir_path));
    }

    #[rstest]
    fn expand_subtree_expands_loaded_dirs() {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let inner_path = subdir_path.join("inner");
        let inner = dir_node("inner", &subdir_path, vec![file_node("deep.txt", &inner_path)]);
        let subdir = dir_node("subdir", root, vec![inner, file_node("a.txt", &subdir_path)]);

        let mut state = state_with_children(vec![subdir]);
        let result = state.expand_subtree(&root.join("subdir"), 300);

        // subdir + inner = 2 expanded.
        assert_eq!(result.expanded, 2);
        assert!(result.needs_load.is_empty());
        assert!(!result.hit_limit);

        // All should be visible now: subdir, inner, deep.txt, a.txt.
        assert_eq!(state.visible_node_count(), 4);
    }

    #[rstest]
    fn expand_subtree_returns_not_loaded_paths() {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let subdir = dir_node(
            "subdir",
            root,
            vec![not_loaded_dir("child_dir", &subdir_path), file_node("a.txt", &subdir_path)],
        );

        let mut state = state_with_children(vec![subdir]);
        let result = state.expand_subtree(&root.join("subdir"), 300);

        // subdir + child_dir = 2 expanded.
        assert_eq!(result.expanded, 2);
        // child_dir had NotLoaded children → now Loading, path returned.
        assert_eq!(result.needs_load, vec![subdir_path.join("child_dir")]);
    }

    #[rstest]
    fn expand_subtree_respects_limit() {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let inner = dir_node("inner", &subdir_path, vec![]);
        let subdir = dir_node("subdir", root, vec![inner]);

        let mut state = state_with_children(vec![subdir]);
        let result = state.expand_subtree(&root.join("subdir"), 1);

        // Only 1 expanded (subdir), inner not expanded.
        assert_eq!(result.expanded, 1);
        assert!(result.hit_limit);
    }

    #[rstest]
    fn expand_subtree_skips_already_expanded() {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let mut subdir = dir_node("subdir", root, vec![file_node("a.txt", &subdir_path)]);
        subdir.is_expanded = true;

        let mut state = state_with_children(vec![subdir]);
        let result = state.expand_subtree(&root.join("subdir"), 300);

        // Already expanded, count = 0.
        assert_eq!(result.expanded, 0);
    }

    #[rstest]
    fn collapse_subtree_collapses_all() {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let inner_path = subdir_path.join("inner");
        let mut inner = dir_node("inner", &subdir_path, vec![file_node("deep.txt", &inner_path)]);
        inner.is_expanded = true;
        let mut subdir = dir_node("subdir", root, vec![inner, file_node("a.txt", &subdir_path)]);
        subdir.is_expanded = true;

        let mut state = state_with_children(vec![subdir]);
        // Before: subdir, inner, deep.txt, a.txt = 4 visible.
        assert_eq!(state.visible_node_count(), 4);

        let collapsed = state.collapse_subtree(&root.join("subdir"));

        // After: only subdir visible (collapsed).
        assert_eq!(state.visible_node_count(), 1);
        // Both subdir and inner were collapsed.
        assert_eq!(collapsed.len(), 2);
        assert!(collapsed.contains(&root.join("subdir")));
        assert!(collapsed.contains(&subdir_path.join("inner")));
    }

    #[rstest]
    fn collapse_subtree_clamps_cursor() {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let mut subdir = dir_node(
            "subdir",
            root,
            vec![file_node("a.txt", &subdir_path), file_node("b.txt", &subdir_path)],
        );
        subdir.is_expanded = true;

        let mut state = state_with_children(vec![subdir]);
        state.move_cursor_to(2); // b.txt (index 2)
        assert_eq!(state.cursor(), 2);

        state.collapse_subtree(&root.join("subdir"));

        // After collapse only 1 node visible, cursor clamped to 0.
        assert_eq!(state.cursor(), 0);
    }
}
