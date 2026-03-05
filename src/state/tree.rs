//! Tree state: data structures for file system tree representation and navigation.

use std::collections::HashSet;
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
#[expect(clippy::struct_excessive_bools, reason = "TreeNode uses bools for independent file flags")]
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
    /// Maximum modification time among loaded descendants (directories only).
    ///
    /// Computed when children are loaded via `set_children`.
    /// Uses the max of all children's `modified` (files) and `recursive_max_mtime` (dirs).
    pub recursive_max_mtime: Option<SystemTime>,
    /// Symlink target path (only `Some` when `is_symlink == true` and `read_link` succeeds).
    pub symlink_target: Option<String>,
    /// Children loading state (only meaningful for directories).
    pub children: ChildrenState,
    /// Whether this directory is expanded (only meaningful for directories).
    pub is_expanded: bool,
    /// Whether this file is gitignored (set only when `show_ignored` is true).
    pub is_ignored: bool,
    /// Whether this node is the root of the tree.
    pub is_root: bool,
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
    /// Natural sort with suffix grouping.
    #[default]
    Smart,
    /// Sort by name.
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
            crate::config::SortOrder::Smart => Self::Smart,
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

/// Configuration options for tree display and sorting.
#[derive(Debug, Clone, Copy)]
pub struct TreeOptions {
    /// Current sort order.
    pub sort_order: SortOrder,
    /// Current sort direction.
    pub sort_direction: SortDirection,
    /// Whether directories should appear before files.
    pub directories_first: bool,
    /// Whether the root directory itself is shown as a tree node.
    pub show_root: bool,
}

impl Default for TreeOptions {
    fn default() -> Self {
        Self {
            sort_order: SortOrder::default(),
            sort_direction: SortDirection::default(),
            directories_first: true,
            show_root: false,
        }
    }
}

/// Tree state: manages the root node, cursor, and sort settings.
#[derive(Debug)]
pub struct TreeState {
    /// Root node of the tree.
    root: TreeNode,
    /// Cursor position (index into visible nodes).
    cursor: usize,
    /// Display and sort options.
    options: TreeOptions,
    /// Active search filter: only paths in this set (and their ancestors) are visible.
    ///
    /// When `Some`, the tree view shows a filtered subset.
    /// When `None`, the normal expansion state is used.
    search_filter: Option<HashSet<PathBuf>>,
    /// Whether filtered directories should appear expanded regardless of `is_expanded`.
    ///
    /// `true` during the Typing phase so incremental search results are immediately
    /// visible. `false` during the Filtered phase so the user can collapse/expand
    /// directories freely.
    search_virtual_expand: bool,
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
    /// Create a new tree state from a root node and options.
    pub const fn new(root: TreeNode, options: TreeOptions) -> Self {
        Self { root, cursor: 0, options, search_filter: None, search_virtual_expand: false }
    }

    /// Get the current cursor position.
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Get the current sort order.
    pub const fn sort_order(&self) -> SortOrder {
        self.options.sort_order
    }

    /// Get the current sort direction.
    pub const fn sort_direction(&self) -> SortDirection {
        self.options.sort_direction
    }

    /// Get whether directories come first.
    pub const fn directories_first(&self) -> bool {
        self.options.directories_first
    }

    /// Get whether the root directory is shown as a tree node.
    pub const fn show_root(&self) -> bool {
        self.options.show_root
    }

    /// Get the root directory path.
    pub fn root_path(&self) -> &Path {
        &self.root.path
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

    /// Paths of visible nodes above the cursor, nearest first.
    ///
    /// Used as fallback targets when the cursor's node disappears after
    /// a tree rebuild (e.g. toggling hidden/ignored files).
    pub fn paths_above_cursor(&self) -> Vec<PathBuf> {
        let visible = self.visible_nodes();
        let cursor = self.cursor.min(visible.len().saturating_sub(1));
        (0..cursor).rev().filter_map(|i| visible.get(i).map(|vn| vn.node.path.clone())).collect()
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

    /// Reveal a path in the tree by loading and expanding ancestor directories.
    ///
    /// Walks from the root to the target path, synchronously loading children
    /// for each ancestor that hasn't been loaded yet. After all ancestors are
    /// expanded, moves the cursor to the target.
    ///
    /// Returns `true` if the target was found and the cursor moved.
    pub fn reveal_path(
        &mut self,
        target: &Path,
        builder: crate::tree::builder::TreeBuilder,
    ) -> bool {
        // Canonicalize target to match tree paths (which are canonicalized).
        let Ok(target) = std::fs::canonicalize(target) else {
            return false;
        };

        // Collect directories to expand: all ancestors between root and target.
        let mut dirs_to_expand = Vec::new();
        let mut current = target.parent();
        while let Some(dir) = current {
            if dir == self.root.path {
                break;
            }
            dirs_to_expand.push(dir.to_path_buf());
            current = dir.parent();
        }
        // Expand from root downward (shallowest first).
        dirs_to_expand.reverse();

        // Ensure each directory is loaded and expanded.
        for dir in &dirs_to_expand {
            if !self.ensure_expanded(dir, builder) {
                return false;
            }
        }

        self.move_cursor_to_path(&target)
    }

    /// Load children for all `NotLoaded` directories whose paths are in the
    /// given set.
    ///
    /// Processes paths from shallowest to deepest so that parent directories
    /// are loaded before their children, enabling single-pass traversal.
    pub fn ensure_filter_paths_loaded(
        &mut self,
        paths: &HashSet<PathBuf>,
        builder: crate::tree::builder::TreeBuilder,
    ) {
        let mut dirs: Vec<&PathBuf> = paths.iter().collect();
        dirs.sort_by_key(|p| p.components().count());
        for dir in dirs {
            self.ensure_children_loaded(dir, builder);
        }
    }

    /// Load children of a directory without changing its expansion state.
    ///
    /// Unlike [`ensure_expanded`], this only transitions `NotLoaded` → `Loaded`
    /// and preserves the current `is_expanded` flag. Used during the search
    /// Typing phase where expansion must remain virtual.
    fn ensure_children_loaded(
        &mut self,
        dir: &Path,
        builder: crate::tree::builder::TreeBuilder,
    ) -> bool {
        let Some(node) = self.find_node_mut(dir) else {
            return false;
        };
        if matches!(node.children, ChildrenState::Loaded(_)) {
            return true;
        }
        let was_expanded = node.is_expanded;
        let Ok(children) = builder.load_children(dir) else {
            return false;
        };
        self.set_children(dir, children, was_expanded);
        true
    }

    /// Ensure a directory node is loaded and expanded.
    ///
    /// Returns `false` if the node could not be found or children failed to load.
    fn ensure_expanded(&mut self, dir: &Path, builder: crate::tree::builder::TreeBuilder) -> bool {
        let Some(node) = self.find_node_mut(dir) else {
            return false;
        };
        if matches!(node.children, ChildrenState::Loaded(_)) {
            node.is_expanded = true;
            return true;
        }
        // Need to load children.
        let Ok(children) = builder.load_children(dir) else {
            return false;
        };
        self.set_children(dir, children, true);
        true
    }

    /// Generate the flattened list of visible nodes by DFS walk.
    ///
    /// Only includes nodes that are in expanded + `Loaded` directories.
    /// When `show_root` is true, the root directory itself appears as the first node.
    /// When a search filter is active, only nodes in the filter set are included
    /// and ancestor directories are treated as expanded.
    pub fn visible_nodes(&self) -> Vec<VisibleNode<'_>> {
        let mut result = Vec::new();
        if let Some(ref filter) = self.search_filter {
            collect_visible_filtered(&self.root, 0, &mut result, filter, self.search_virtual_expand, self.options.show_root);
        } else {
            collect_visible(&self.root, 0, &mut result, self.options.show_root);
        }
        result
    }

    /// Generate visible nodes for a viewport range only.
    ///
    /// Walks the tree in DFS order, skipping the first `skip` nodes and
    /// collecting at most `take` nodes. Early-terminates once enough nodes
    /// are collected, avoiding a full tree walk when the viewport is near
    /// the top.
    ///
    /// When a search filter is active, falls back to full `visible_nodes()`
    /// and slices the result (the filtered tree is typically small).
    pub fn visible_nodes_in_range(&self, skip: usize, take: usize) -> Vec<VisibleNode<'_>> {
        // When a search filter is active, fall back to full visible_nodes()
        // since the filtered tree is small (bounded by max_results).
        if self.search_filter.is_some() {
            let all = self.visible_nodes();
            return all.into_iter().skip(skip).take(take).collect();
        }

        let mut result = Vec::with_capacity(take);
        let mut skipped: usize = 0;
        collect_visible_range(&self.root, 0, &mut result, &mut skipped, skip, take, self.options.show_root);
        result
    }

    /// Count of visible nodes (without allocating the full list).
    ///
    /// When a search filter is active, falls back to `visible_nodes().len()`
    /// since the filtered tree is bounded by `max_results`.
    pub fn visible_node_count(&self) -> usize {
        if self.search_filter.is_some() {
            return self.visible_nodes().len();
        }
        count_visible(&self.root, self.options.show_root)
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
        let order = self.options.sort_order;
        let direction = self.options.sort_direction;
        let dirs_first = self.options.directories_first;

        crate::tree::sort::sort_children(&mut children, order, direction, dirs_first);

        if let Some(node) = self.find_node_mut(path) {
            // Preserve expansion state and loaded children from old nodes.
            if let Some(old_children) = node.children.as_loaded_mut() {
                transfer_expansion_state(old_children, &mut children);
            }
            node.recursive_max_mtime = compute_recursive_max_mtime(&children);
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
            return true;
        }
        if is_loaded {
            node.children = ChildrenState::NotLoaded;
        }
        false
    }

    /// Transition a directory from `NotLoaded` to `Loading` state.
    ///
    /// This is the common primitive for all async directory loads: user-initiated
    /// expand, prefetch, and deferred session restore.
    ///
    /// When `auto_expand` is true, the directory is also marked as expanded.
    /// Returns the path if the transition was made, `None` if the node is not
    /// found, not a directory, or not in `NotLoaded` state.
    pub fn prepare_async_load(&mut self, path: &Path, auto_expand: bool) -> Option<PathBuf> {
        let node = self.find_node_mut(path)?;
        if !node.is_dir || !matches!(node.children, ChildrenState::NotLoaded) {
            return None;
        }
        node.children = ChildrenState::Loading;
        if auto_expand {
            node.is_expanded = true;
        }
        Some(node.path.clone())
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

        if is_expanded {
            let node = self.find_node_mut(&path)?;
            node.is_expanded = false;
            None
        } else {
            // Try the common NotLoaded → Loading transition first.
            if let Some(p) = self.prepare_async_load(&path, true) {
                return Some(ExpandResult::NeedsLoad(p));
            }
            // Node was already Loading or Loaded.
            let node = self.find_node_mut(&path)?;
            node.is_expanded = true;
            match &node.children {
                ChildrenState::Loaded(_) => Some(ExpandResult::AlreadyLoaded(path)),
                ChildrenState::Loading | ChildrenState::NotLoaded => None,
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
        // Pre-compute parent index to avoid a second visible_nodes() traversal.
        let parent_idx = vnode
            .node
            .path
            .parent()
            .and_then(|pp| visible.iter().position(|vn| vn.node.path == pp));
        drop(visible);

        if is_dir
            && is_expanded
            && let Some(node) = self.find_node_mut(&path)
        {
            node.is_expanded = false;
            return true;
        }

        // Move to parent directory.
        if let Some(idx) = parent_idx {
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
        let is_dir = vnode.node.is_dir;

        if !is_dir {
            let path = vnode.node.path.clone();
            return Some(ExpandResult::OpenFile(path));
        }

        self.expand_dir()
    }

    /// Expand a directory without opening files.
    ///
    /// Returns:
    /// - `Some(ExpandResult::NeedsLoad(path))` if children need to be loaded
    /// - `Some(ExpandResult::AlreadyLoaded(path))` if already loaded but was collapsed
    /// - `None` if cursor is on a file, already expanded, or nothing to do
    pub fn expand_dir(&mut self) -> Option<ExpandResult> {
        let visible = self.visible_nodes();
        let vnode = visible.get(self.cursor)?;
        let path = vnode.node.path.clone();
        let is_dir = vnode.node.is_dir;

        if !is_dir {
            return None;
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

    /// Set the search filter to restrict visible nodes.
    ///
    /// Only paths in the set (and their ancestors up to root) will be visible.
    /// Enables virtual expansion so results are visible during the Typing phase.
    pub fn set_search_filter(&mut self, mut filter: HashSet<PathBuf>) {
        if self.options.show_root {
            filter.insert(self.root.path.clone());
        }
        self.search_filter = Some(filter);
        self.search_virtual_expand = true;
        // Clamp cursor to new visible range.
        let count = self.visible_node_count();
        if count == 0 {
            self.cursor = 0;
        } else {
            self.cursor = self.cursor.min(count - 1);
        }
    }

    /// Clear the search filter, restoring normal tree visibility.
    ///
    /// Preserves the cursor on the same node by path lookup after clearing.
    pub fn clear_search_filter(&mut self) {
        let cursor_path = self.cursor_path();
        self.search_filter = None;
        self.search_virtual_expand = false;
        if let Some(path) = cursor_path {
            self.move_cursor_to_path(&path);
        }
    }

    /// Disable virtual expansion for the search filter.
    ///
    /// Called after `expand_paths` has truly expanded filter directories,
    /// so that user collapse/expand is respected in the Filtered phase.
    pub const fn pin_search_filter(&mut self) {
        self.search_virtual_expand = false;
    }

    /// Whether a search filter is currently active.
    pub const fn has_search_filter(&self) -> bool {
        self.search_filter.is_some()
    }

    /// Get a reference to the active search filter paths.
    pub const fn search_filter_paths(&self) -> Option<&HashSet<PathBuf>> {
        self.search_filter.as_ref()
    }

    /// Expand all directories whose paths are in the given set.
    ///
    /// Sets `is_expanded = true` on matching directory nodes so the expansion
    /// persists after the search filter is cleared. Only recurses into
    /// children that are already loaded.
    pub fn expand_paths(&mut self, paths: &HashSet<PathBuf>) {
        expand_paths_recursive(&mut self.root, paths);
    }

    /// Apply sort settings and re-sort all loaded children.
    pub fn apply_sort(
        &mut self,
        order: SortOrder,
        direction: SortDirection,
        directories_first: bool,
    ) {
        self.options.sort_order = order;
        self.options.sort_direction = direction;
        self.options.directories_first = directories_first;
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
///
/// When `node.is_root && !show_root`, the node itself is skipped (not added
/// to `result`) but its children are always traversed at the same depth.
fn collect_visible<'a>(
    node: &'a TreeNode,
    depth: usize,
    result: &mut Vec<VisibleNode<'a>>,
    show_root: bool,
) {
    let skip_display = node.is_root && !show_root;
    if !skip_display {
        result.push(VisibleNode { node, depth });
    }
    let child_depth = if skip_display { depth } else { depth + 1 };
    let expanded = skip_display || node.is_expanded;
    if expanded
        && let Some(children) = node.children.as_loaded()
    {
        for child in children {
            collect_visible(child, child_depth, result, show_root);
        }
    }
}

/// Collect visible nodes filtered by a path set.
///
/// A node is included if its path is in `filter`. Ancestor directories are
/// traversed (and included) if they contain any filtered descendant.
/// When `force_expand` is true (Typing phase), directories are traversed
/// regardless of `is_expanded`. When false (Filtered phase), only expanded
/// directories are traversed.
///
/// When `node.is_root && !show_root`, the node is always traversed (skipping
/// both the display and the filter check).
fn collect_visible_filtered<'a>(
    node: &'a TreeNode,
    depth: usize,
    result: &mut Vec<VisibleNode<'a>>,
    filter: &HashSet<PathBuf>,
    force_expand: bool,
    show_root: bool,
) {
    let skip_display = node.is_root && !show_root;
    if !skip_display {
        if !filter.contains(&node.path) {
            return;
        }
        result.push(VisibleNode { node, depth });
    }
    let child_depth = if skip_display { depth } else { depth + 1 };
    let expanded = skip_display || force_expand || node.is_expanded;
    if expanded
        && let Some(children) = node.children.as_loaded()
    {
        for child in children {
            collect_visible_filtered(child, child_depth, result, filter, force_expand, show_root);
        }
    }
}

/// Collect visible nodes within a range, with early termination.
///
/// When `node.is_root && !show_root`, the node is skipped in the count
/// but its children are always traversed.
fn collect_visible_range<'a>(
    node: &'a TreeNode,
    depth: usize,
    result: &mut Vec<VisibleNode<'a>>,
    skipped: &mut usize,
    skip: usize,
    take: usize,
    show_root: bool,
) {
    if result.len() >= take {
        return;
    }

    let skip_display = node.is_root && !show_root;
    if !skip_display {
        if *skipped >= skip {
            result.push(VisibleNode { node, depth });
            if result.len() >= take {
                return;
            }
        } else {
            *skipped += 1;
        }
    }

    let child_depth = if skip_display { depth } else { depth + 1 };
    let expanded = skip_display || node.is_expanded;
    if expanded
        && let Some(children) = node.children.as_loaded()
    {
        for child in children {
            collect_visible_range(child, child_depth, result, skipped, skip, take, show_root);
            if result.len() >= take {
                return;
            }
        }
    }
}

/// Count visible nodes without allocating.
///
/// When `node.is_root && !show_root`, the node itself is not counted
/// but its children are always traversed.
fn count_visible(node: &TreeNode, show_root: bool) -> usize {
    let skip_display = node.is_root && !show_root;
    let self_count = usize::from(!skip_display);
    let expanded = skip_display || node.is_expanded;
    let children_count = node
        .children
        .as_loaded()
        .filter(|_| expanded)
        .map_or(0, |children| {
            children.iter().map(|c| count_visible(c, show_root)).sum()
        });
    self_count + children_count
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

/// Recursively expand directories whose paths are in the given set.
fn expand_paths_recursive(node: &mut TreeNode, paths: &HashSet<PathBuf>) {
    if node.is_dir && paths.contains(&node.path) {
        node.is_expanded = true;
    }
    if let Some(children) = node.children.as_loaded_mut() {
        for child in children.iter_mut() {
            expand_paths_recursive(child, paths);
        }
    }
}

/// Transfer expansion state from old children to new children.
///
/// For each new directory node that also existed in the old list (matched by path),
/// copies `is_expanded` and `children` from the old node to preserve the user's
/// expand/collapse state across directory refreshes (e.g., after file deletion).
fn transfer_expansion_state(old_children: &mut Vec<TreeNode>, new_children: &mut [TreeNode]) {
    for new_child in new_children.iter_mut().filter(|c| c.is_dir) {
        if let Some(old_idx) = old_children.iter().position(|old| old.path == new_child.path) {
            let old_child = old_children.swap_remove(old_idx);
            new_child.is_expanded = old_child.is_expanded;
            new_child.children = old_child.children;
        }
    }
}

/// Compute the maximum modification time among loaded children.
///
/// For file children, uses their `modified` time. For directory children,
/// uses `recursive_max_mtime` if available, falling back to `modified`.
fn compute_recursive_max_mtime(children: &[TreeNode]) -> Option<SystemTime> {
    children
        .iter()
        .filter_map(|child| {
            if child.is_dir { child.recursive_max_mtime.or(child.modified) } else { child.modified }
        })
        .max()
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
            recursive_max_mtime: None,
            symlink_target: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
            is_ignored: false,
            is_root: false,
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
            recursive_max_mtime: None,
            symlink_target: None,
            children: ChildrenState::Loaded(children),
            is_expanded: false,
            is_ignored: false,
            is_root: false,
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
            recursive_max_mtime: None,
            symlink_target: None,
            children: ChildrenState::Loaded(children),
            is_expanded: true,
            is_ignored: false,
            is_root: true,
        }
    }

    /// Helper: create a default `TreeState` from children.
    fn state_with_children(children: Vec<TreeNode>) -> TreeState {
        TreeState::new(root_with_children(children), TreeOptions::default())
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
    // Visible Nodes
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
            symlink_target: None,
            size: 0,
            modified: None,
            recursive_max_mtime: None,
            children: ChildrenState::NotLoaded,
            is_expanded: true, // expanded but NotLoaded
            is_ignored: false,
            is_root: false,
        };
        // Even though expanded, no children to show
        let state = state_with_children(vec![not_loaded_dir]);
        let visible = state.visible_nodes();
        verify_that!(visible.len(), eq(1))?;
        verify_that!(visible[0].node.name.as_str(), eq("empty_dir"))?;
        Ok(())
    }

    // =========================================================================
    // show_root
    // =========================================================================

    #[rstest]
    fn visible_nodes_with_show_root_includes_root_at_depth_zero() -> Result<()> {
        let root = Path::new("/test/root");
        let state = TreeState::new(
            root_with_children(vec![file_node("a.txt", root), file_node("b.txt", root)]),
            TreeOptions { show_root: true, ..TreeOptions::default() },
        );
        let visible = state.visible_nodes();
        // root + 2 children = 3
        verify_that!(visible.len(), eq(3))?;
        verify_that!(visible[0].node.name.as_str(), eq("root"))?;
        verify_that!(visible[0].depth, eq(0))?;
        verify_that!(visible[1].node.name.as_str(), eq("a.txt"))?;
        verify_that!(visible[1].depth, eq(1))?;
        verify_that!(visible[2].depth, eq(1))?;
        Ok(())
    }

    #[rstest]
    fn visible_node_count_with_show_root() -> Result<()> {
        let root = Path::new("/test/root");
        let state = TreeState::new(
            root_with_children(vec![file_node("a.txt", root)]),
            TreeOptions { show_root: true, ..TreeOptions::default() },
        );
        verify_that!(state.visible_node_count(), eq(2))?; // root + a.txt
        Ok(())
    }

    #[rstest]
    fn show_root_collapsed_hides_children() -> Result<()> {
        let root = Path::new("/test/root");
        let mut state = TreeState::new(
            root_with_children(vec![file_node("a.txt", root), file_node("b.txt", root)]),
            TreeOptions { show_root: true, ..TreeOptions::default() },
        );
        // Initially root is expanded: root + 2 children = 3
        verify_that!(state.visible_node_count(), eq(3))?;
        verify_that!(state.visible_nodes().len(), eq(3))?;

        // Collapse root
        state.root.is_expanded = false;

        // Only root is visible, children are hidden
        verify_that!(state.visible_node_count(), eq(1))?;
        let visible = state.visible_nodes();
        verify_that!(visible.len(), eq(1))?;
        verify_that!(visible[0].node.name.as_str(), eq("root"))?;
        Ok(())
    }

    // =========================================================================
    // Lazy Loading (set_children, toggle_expand, set_children_error)
    // =========================================================================

    #[rstest]
    fn test_set_children_loads_and_reflects_in_visible() -> Result<()> {
        let root = Path::new("/test/root");
        let subdir = TreeNode {
            name: "subdir".to_string(),
            path: root.join("subdir"),
            is_dir: true,
            is_symlink: false,
            symlink_target: None,
            size: 0,
            modified: None,
            recursive_max_mtime: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
            is_ignored: false,
            is_root: false,
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
    fn set_children_preserves_expansion_state_of_existing_dirs() -> Result<()> {
        let root = Path::new("/test/root");
        let parent_path = root.join("parent");

        // Build parent with an expanded subdirectory containing a loaded child.
        let inner_path = parent_path.join("inner");
        let mut inner = dir_node("inner", &parent_path, vec![file_node("deep.txt", &inner_path)]);
        inner.is_expanded = true;

        let mut parent =
            dir_node("parent", root, vec![inner, file_node("old_file.txt", &parent_path)]);
        parent.is_expanded = true;

        let mut state = state_with_children(vec![parent]);
        // Before: parent, inner, deep.txt, old_file.txt = 4 visible.
        verify_that!(state.visible_node_count(), eq(4))?;

        // Simulate refresh: new children list (old_file deleted, inner still exists).
        let new_inner = TreeNode {
            name: "inner".to_string(),
            path: parent_path.join("inner"),
            is_dir: true,
            is_symlink: false,
            symlink_target: None,
            size: 0,
            modified: None,
            recursive_max_mtime: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
            is_ignored: false,
            is_root: false,
        };
        state.set_children(&parent_path, vec![new_inner], true);

        // inner should still be expanded with its loaded children preserved.
        let visible = state.visible_nodes();
        // parent, inner, deep.txt = 3 (old_file.txt removed).
        verify_that!(visible.len(), eq(3))?;
        verify_that!(visible[1].node.name.as_str(), eq("inner"))?;
        verify_that!(visible[1].node.is_expanded, eq(true))?;
        verify_that!(visible[2].node.name.as_str(), eq("deep.txt"))?;
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
            symlink_target: None,
            size: 0,
            modified: None,
            recursive_max_mtime: None,
            children: ChildrenState::Loading,
            is_expanded: true,
            is_ignored: false,
            is_root: false,
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
            symlink_target: None,
            size: 0,
            modified: None,
            recursive_max_mtime: None,
            children: ChildrenState::Loading,
            is_expanded: false, // collapsed (prefetch scenario)
            is_ignored: false,
            is_root: false,
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
            symlink_target: None,
            size: 0,
            modified: None,
            recursive_max_mtime: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
            is_ignored: false,
            is_root: false,
        };
        let subdir_b = TreeNode {
            name: "dir_b".to_string(),
            path: root.join("dir_b"),
            is_dir: true,
            is_symlink: false,
            symlink_target: None,
            size: 0,
            modified: None,
            recursive_max_mtime: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
            is_ignored: false,
            is_root: false,
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
            symlink_target: None,
            size: 0,
            modified: None,
            recursive_max_mtime: None,
            children: ChildrenState::Loading,
            is_expanded: false,
            is_ignored: false,
            is_root: false,
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
    // Cursor Management
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
            symlink_target: None,
            size: 0,
            modified: None,
            recursive_max_mtime: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
            is_ignored: false,
            is_root: false,
        };
        let mut state = state_with_children(vec![subdir]);
        let result = state.expand_or_open();
        assert_eq!(result, Some(ExpandResult::NeedsLoad(root.join("subdir"))));
    }

    #[rstest]
    fn expand_dir_on_file_returns_none() {
        let root = Path::new("/test/root");
        let mut state = state_with_children(vec![file_node("test.txt", root)]);
        let result = state.expand_dir();
        assert_eq!(result, None);
    }

    #[rstest]
    fn expand_dir_on_directory_expands() {
        let root = Path::new("/test/root");
        let subdir_path = root.join("subdir");
        let subdir = dir_node("subdir", root, vec![file_node("child.txt", &subdir_path)]);
        let mut state = state_with_children(vec![subdir]);
        let result = state.expand_dir();
        assert_eq!(result, Some(ExpandResult::AlreadyLoaded(root.join("subdir"))));
        assert_eq!(state.visible_node_count(), 2);
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

        let mut state = TreeState::new(root, TreeOptions::default());
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

        let mut state = TreeState::new(root, TreeOptions::default());
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
            symlink_target: None,
            size: 0,
            modified: None,
            recursive_max_mtime: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
            is_ignored: false,
            is_root: false,
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
            recursive_max_mtime: None,
            symlink_target: None,
            children: ChildrenState::NotLoaded,
            is_expanded: false,
            is_ignored: false,
            is_root: false,
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

    // --- visible_nodes_in_range ---

    #[rstest]
    fn visible_nodes_in_range_returns_full_range() -> Result<()> {
        let root = Path::new("/test/root");
        let state = state_with_children(vec![
            file_node("a.txt", root),
            file_node("b.txt", root),
            file_node("c.txt", root),
        ]);
        let range = state.visible_nodes_in_range(0, 10);
        verify_that!(range.len(), eq(3))?;
        verify_that!(range[0].node.name.as_str(), eq("a.txt"))?;
        verify_that!(range[2].node.name.as_str(), eq("c.txt"))?;
        Ok(())
    }

    #[rstest]
    fn visible_nodes_in_range_skips_and_takes() -> Result<()> {
        let root = Path::new("/test/root");
        let state = state_with_children(vec![
            file_node("a.txt", root),
            file_node("b.txt", root),
            file_node("c.txt", root),
            file_node("d.txt", root),
        ]);
        let range = state.visible_nodes_in_range(1, 2);
        verify_that!(range.len(), eq(2))?;
        verify_that!(range[0].node.name.as_str(), eq("b.txt"))?;
        verify_that!(range[1].node.name.as_str(), eq("c.txt"))?;
        Ok(())
    }

    #[rstest]
    fn visible_nodes_in_range_matches_visible_nodes() -> Result<()> {
        let root = Path::new("/test/root");
        let sub_path = root.join("sub");
        let mut sub = dir_node(
            "sub",
            root,
            vec![file_node("x.txt", &sub_path), file_node("y.txt", &sub_path)],
        );
        sub.is_expanded = true;
        let state =
            state_with_children(vec![file_node("a.txt", root), sub, file_node("z.txt", root)]);
        // Full list: a.txt, sub, x.txt, y.txt, z.txt
        let all = state.visible_nodes();
        verify_that!(all.len(), eq(5))?;

        // Range should match the corresponding slice.
        let range = state.visible_nodes_in_range(1, 3);
        verify_that!(range.len(), eq(3))?;
        for (i, vn) in range.iter().enumerate() {
            verify_that!(vn.node.name.as_str(), eq(all[i + 1].node.name.as_str()))?;
            verify_that!(vn.depth, eq(all[i + 1].depth))?;
        }
        Ok(())
    }

    #[rstest]
    fn visible_nodes_in_range_with_show_root() -> Result<()> {
        let root = Path::new("/test/root");
        let state = TreeState::new(
            root_with_children(vec![file_node("a.txt", root), file_node("b.txt", root)]),
            TreeOptions { show_root: true, ..TreeOptions::default() },
        );
        // skip=0, take=2 → root + a.txt
        let range = state.visible_nodes_in_range(0, 2);
        verify_that!(range.len(), eq(2))?;
        verify_that!(range[0].node.name.as_str(), eq("root"))?;
        verify_that!(range[1].node.name.as_str(), eq("a.txt"))?;

        // skip=1, take=2 → a.txt + b.txt
        let range = state.visible_nodes_in_range(1, 2);
        verify_that!(range.len(), eq(2))?;
        verify_that!(range[0].node.name.as_str(), eq("a.txt"))?;
        verify_that!(range[1].node.name.as_str(), eq("b.txt"))?;
        Ok(())
    }

    // =========================================================================
    // paths_above_cursor
    // =========================================================================

    #[rstest]
    fn paths_above_cursor_returns_nearest_first() {
        let root = Path::new("/test/root");
        let mut state = state_with_children(vec![
            file_node("a.txt", root),
            file_node("b.txt", root),
            file_node("c.txt", root),
        ]);
        // show_root is false by default, visible: [a, b, c]
        state.move_cursor_to(2); // cursor on c.txt

        let paths = state.paths_above_cursor();
        assert_eq!(paths, vec![root.join("b.txt"), root.join("a.txt")]);
    }

    #[rstest]
    fn paths_above_cursor_empty_when_cursor_at_top() {
        let root = Path::new("/test/root");
        let state = state_with_children(vec![file_node("a.txt", root), file_node("b.txt", root)]);
        // cursor at 0
        let paths = state.paths_above_cursor();
        assert!(paths.is_empty());
    }

    #[rstest]
    fn paths_above_cursor_single_node_above() {
        let root = Path::new("/test/root");
        let mut state =
            state_with_children(vec![file_node("a.txt", root), file_node("b.txt", root)]);
        state.move_cursor_to(1); // cursor on b.txt

        let paths = state.paths_above_cursor();
        assert_eq!(paths, vec![root.join("a.txt")]);
    }

    // --- Search filter tests ---

    #[rstest]
    fn search_filter_hides_unmatched_nodes() {
        let root = Path::new("/test/root");
        let mut state = state_with_children(vec![
            file_node("a.txt", root),
            file_node("b.txt", root),
            file_node("c.txt", root),
        ]);

        let mut filter = HashSet::new();
        filter.insert(root.join("a.txt"));
        filter.insert(root.join("c.txt"));
        state.set_search_filter(filter);

        let visible = state.visible_nodes();
        assert_that!(visible.len(), eq(2));
        assert_that!(visible[0].node.name.as_str(), eq("a.txt"));
        assert_that!(visible[1].node.name.as_str(), eq("c.txt"));
    }

    #[rstest]
    fn search_filter_includes_ancestors() {
        let root = Path::new("/test/root");
        let sub = dir_node(
            "sub",
            root,
            vec![file_node("target.txt", &root.join("sub"))],
        );
        let mut state = state_with_children(vec![sub, file_node("other.txt", root)]);

        // Filter includes the file AND its parent directory.
        let mut filter = HashSet::new();
        filter.insert(root.join("sub"));
        filter.insert(root.join("sub").join("target.txt"));
        // Simulate confirm_search: expand, set filter, then pin.
        state.expand_paths(&filter);
        state.set_search_filter(filter);
        state.pin_search_filter();

        let visible = state.visible_nodes();
        assert_that!(visible.len(), eq(2));
        assert_that!(visible[0].node.name.as_str(), eq("sub"));
        assert_that!(visible[1].node.name.as_str(), eq("target.txt"));
    }

    #[rstest]
    fn search_filter_clears_correctly() {
        let root = Path::new("/test/root");
        let mut sub = dir_node(
            "sub",
            root,
            vec![file_node("x.txt", &root.join("sub"))],
        );
        sub.is_expanded = true;
        let mut state = state_with_children(vec![sub, file_node("y.txt", root)]);

        // Apply then clear filter.
        let mut filter = HashSet::new();
        filter.insert(root.join("y.txt"));
        state.set_search_filter(filter);
        assert_that!(state.visible_node_count(), eq(1));

        state.clear_search_filter();
        // Should return to normal visibility (sub expanded + x.txt + y.txt = 3).
        assert_that!(state.visible_node_count(), eq(3));
    }

    #[rstest]
    fn search_filter_clamps_cursor() {
        let root = Path::new("/test/root");
        let mut state = state_with_children(vec![
            file_node("a.txt", root),
            file_node("b.txt", root),
            file_node("c.txt", root),
        ]);
        state.move_cursor_to(2); // cursor on c.txt

        // Filter hides c.txt, so cursor should clamp.
        let mut filter = HashSet::new();
        filter.insert(root.join("a.txt"));
        state.set_search_filter(filter);

        assert_that!(state.cursor(), eq(0));
    }

    #[rstest]
    fn search_filter_virtual_expand_shows_unexpanded_children() {
        let root = Path::new("/test/root");
        let sub = dir_node(
            "sub",
            root,
            vec![file_node("target.txt", &root.join("sub"))],
        );
        // sub.is_expanded is false by default.
        let mut state = state_with_children(vec![sub]);

        let mut filter = HashSet::new();
        filter.insert(root.join("sub"));
        filter.insert(root.join("sub").join("target.txt"));
        // set_search_filter enables virtual expand by default (Typing phase).
        state.set_search_filter(filter);

        let visible = state.visible_nodes();
        assert_that!(visible.len(), eq(2));
        assert_that!(visible[0].node.name.as_str(), eq("sub"));
        assert_that!(visible[1].node.name.as_str(), eq("target.txt"));
    }

    #[rstest]
    fn search_filter_pinned_respects_is_expanded() {
        let root = Path::new("/test/root");
        let sub = dir_node(
            "sub",
            root,
            vec![file_node("target.txt", &root.join("sub"))],
        );
        // sub.is_expanded is false by default.
        let mut state = state_with_children(vec![sub]);

        let mut filter = HashSet::new();
        filter.insert(root.join("sub"));
        filter.insert(root.join("sub").join("target.txt"));
        state.set_search_filter(filter);
        state.pin_search_filter();

        // sub is not expanded, so only sub itself is visible.
        let visible = state.visible_nodes();
        assert_that!(visible.len(), eq(1));
        assert_that!(visible[0].node.name.as_str(), eq("sub"));
    }
}
