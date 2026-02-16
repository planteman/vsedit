//! Virtual scrolling tree view widget.

/// A node in the tree.
pub struct TreeNode<T> {
    pub data: T,
    pub children: Vec<TreeNode<T>>,
    pub is_expanded: bool,
    pub depth: u32,
}

impl<T> TreeNode<T> {
    /// Create a new tree node with the given data and depth.
    pub fn new(data: T, depth: u32) -> Self {
        Self {
            data,
            children: Vec::new(),
            is_expanded: false,
            depth,
        }
    }

    /// Add a child node, returning a mutable reference to it.
    pub fn add_child(&mut self, data: T) -> &mut TreeNode<T> {
        let child = TreeNode::new(data, self.depth + 1);
        self.children.push(child);
        self.children.last_mut().unwrap()
    }

    fn total_count(&self) -> usize {
        1 + self.children.iter().map(|c| c.total_count()).sum::<usize>()
    }

    fn visible_count(&self) -> usize {
        if self.is_expanded {
            1 + self.children.iter().map(|c| c.visible_count()).sum::<usize>()
        } else {
            1
        }
    }

    fn flatten_into<'a>(&'a self, out: &mut Vec<FlatTreeItem<'a, T>>) {
        let index = out.len();
        out.push(FlatTreeItem {
            node: self,
            depth: self.depth,
            is_expanded: self.is_expanded,
            has_children: !self.children.is_empty(),
            index,
        });
        if self.is_expanded {
            for child in &self.children {
                child.flatten_into(out);
            }
        }
    }

    fn set_expanded_recursive(&mut self, expanded: bool) {
        self.is_expanded = expanded;
        for child in &mut self.children {
            child.set_expanded_recursive(expanded);
        }
    }
}

/// A flattened visible item in the tree, referencing the underlying node.
pub struct FlatTreeItem<'a, T> {
    pub node: &'a TreeNode<T>,
    pub depth: u32,
    pub is_expanded: bool,
    pub has_children: bool,
    pub index: usize,
}

/// The tree data model holding root nodes.
pub struct TreeModel<T> {
    roots: Vec<TreeNode<T>>,
}

impl<T> Default for TreeModel<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> TreeModel<T> {
    pub fn new() -> Self {
        Self { roots: Vec::new() }
    }

    /// Add a root node, returning a mutable reference to it.
    pub fn add_root(&mut self, data: T) -> &mut TreeNode<T> {
        let node = TreeNode::new(data, 0);
        self.roots.push(node);
        self.roots.last_mut().unwrap()
    }

    /// Flatten the tree into a list of visible items.
    pub fn flatten(&self) -> Vec<FlatTreeItem<'_, T>> {
        let mut items = Vec::new();
        for root in &self.roots {
            root.flatten_into(&mut items);
        }
        items
    }

    /// Toggle the expanded state of the node at the given path.
    /// The path is a sequence of child indices from root level.
    pub fn toggle_expanded(&mut self, path: &[usize]) {
        if let Some(node) = Self::node_at_path_mut(&mut self.roots, path) {
            node.is_expanded = !node.is_expanded;
        }
    }

    /// Expand all nodes in the tree.
    pub fn expand_all(&mut self) {
        for root in &mut self.roots {
            root.set_expanded_recursive(true);
        }
    }

    /// Collapse all nodes in the tree.
    pub fn collapse_all(&mut self) {
        for root in &mut self.roots {
            root.set_expanded_recursive(false);
        }
    }

    /// Total number of nodes in the tree.
    pub fn node_count(&self) -> usize {
        self.roots.iter().map(|r| r.total_count()).sum()
    }

    /// Number of currently visible (flattened) nodes.
    pub fn visible_count(&self) -> usize {
        self.roots.iter().map(|r| r.visible_count()).sum()
    }

    fn node_at_path_mut<'a>(nodes: &'a mut [TreeNode<T>], path: &[usize]) -> Option<&'a mut TreeNode<T>> {
        match path {
            [] => None,
            [idx] => nodes.get_mut(*idx),
            [idx, rest @ ..] => nodes
                .get_mut(*idx)
                .and_then(|n| Self::node_at_path_mut(&mut n.children, rest)),
        }
    }

    /// Resolve a flat visible index to a path of child indices, then toggle it.
    fn toggle_at_flat_index(&mut self, flat_index: usize) {
        if let Some(path) = self.path_for_flat_index(flat_index) {
            self.toggle_expanded(&path);
        }
    }

    fn path_for_flat_index(&self, target: usize) -> Option<Vec<usize>> {
        let mut counter = 0;
        for (i, root) in self.roots.iter().enumerate() {
            let mut path = vec![i];
            if Self::find_path(root, target, &mut counter, &mut path) {
                return Some(path);
            }
        }
        None
    }

    fn find_path(
        node: &TreeNode<T>,
        target: usize,
        counter: &mut usize,
        path: &mut Vec<usize>,
    ) -> bool {
        if *counter == target {
            return true;
        }
        *counter += 1;
        if node.is_expanded {
            for (i, child) in node.children.iter().enumerate() {
                path.push(i);
                if Self::find_path(child, target, counter, path) {
                    return true;
                }
                path.pop();
            }
        }
        false
    }
}

/// Tree view with virtual scrolling and selection.
pub struct TreeView<T> {
    model: TreeModel<T>,
    selected_index: Option<usize>,
    scroll_offset: usize,
    viewport_height: usize,
}

impl<T> TreeView<T> {
    pub fn new(model: TreeModel<T>) -> Self {
        Self {
            model,
            selected_index: None,
            scroll_offset: 0,
            viewport_height: 10,
        }
    }

    /// Set the viewport height (number of visible rows).
    pub fn set_viewport_height(&mut self, height: usize) {
        self.viewport_height = height;
    }

    /// Get the visible items within the current viewport.
    pub fn get_visible_items(&self) -> Vec<FlatTreeItem<'_, T>> {
        let all = self.model.flatten();
        let end = (self.scroll_offset + self.viewport_height).min(all.len());
        if self.scroll_offset >= all.len() {
            return Vec::new();
        }
        // Re-flatten and slice to get the viewport window with correct indices.
        all.into_iter()
            .skip(self.scroll_offset)
            .take(end - self.scroll_offset)
            .collect()
    }

    /// Move selection to the next visible item.
    pub fn select_next(&mut self) {
        let count = self.model.visible_count();
        if count == 0 {
            return;
        }
        self.selected_index = Some(match self.selected_index {
            None => 0,
            Some(i) if i + 1 < count => i + 1,
            Some(i) => i,
        });
        self.ensure_selected_visible();
    }

    /// Move selection to the previous visible item.
    pub fn select_previous(&mut self) {
        let count = self.model.visible_count();
        if count == 0 {
            return;
        }
        self.selected_index = Some(match self.selected_index {
            None => 0,
            Some(0) => 0,
            Some(i) => i - 1,
        });
        self.ensure_selected_visible();
    }

    /// Toggle expand/collapse on the currently selected node.
    pub fn toggle_selected(&mut self) {
        if let Some(idx) = self.selected_index {
            self.model.toggle_at_flat_index(idx);
        }
    }

    /// Return the currently selected flat item, if any.
    pub fn selected(&self) -> Option<FlatTreeItem<'_, T>> {
        let idx = self.selected_index?;
        let items = self.model.flatten();
        items.into_iter().find(|item| item.index == idx)
    }

    /// Scroll the viewport up by one row.
    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    /// Scroll the viewport down by one row.
    pub fn scroll_down(&mut self) {
        let count = self.model.visible_count();
        if self.scroll_offset + self.viewport_height < count {
            self.scroll_offset += 1;
        }
    }

    /// Adjust scroll offset so the selected item is within the viewport.
    pub fn ensure_selected_visible(&mut self) {
        if let Some(idx) = self.selected_index {
            if idx < self.scroll_offset {
                self.scroll_offset = idx;
            } else if idx >= self.scroll_offset + self.viewport_height {
                self.scroll_offset = idx - self.viewport_height + 1;
            }
        }
    }
}

/// Result of searching for a node in the tree.
#[derive(Debug, Clone)]
pub struct TreeSearchResult {
    /// Index of this result in the DFS traversal order.
    pub node_index: usize,
    /// Depth of the matched node.
    pub depth: u32,
    /// Path of child indices from the root to this node.
    pub path: Vec<usize>,
}

/// A node paired with its depth, returned by `flatten_tree`.
#[derive(Debug)]
pub struct FlattenedNode<'a, T> {
    pub data: &'a T,
    pub depth: u32,
    pub path: Vec<usize>,
}

/// Aggregate statistics about a tree.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeStatistics {
    pub total_nodes: usize,
    pub leaf_count: usize,
    pub max_depth: u32,
    pub avg_children: f64,
}

/// Search all nodes in the tree that satisfy `predicate`, returning their
/// index in DFS order, depth, and path.
pub fn search_tree<T, F>(model: &TreeModel<T>, predicate: F) -> Vec<TreeSearchResult>
where
    F: Fn(&T) -> bool,
{
    let mut results = Vec::new();
    let mut index = 0;
    for (root_idx, root) in model.roots.iter().enumerate() {
        search_node(root, &predicate, &mut results, &mut index, &mut vec![root_idx]);
    }
    results
}

fn search_node<T, F>(
    node: &TreeNode<T>,
    predicate: &F,
    results: &mut Vec<TreeSearchResult>,
    index: &mut usize,
    path: &mut Vec<usize>,
) where
    F: Fn(&T) -> bool,
{
    if predicate(&node.data) {
        results.push(TreeSearchResult {
            node_index: *index,
            depth: node.depth,
            path: path.clone(),
        });
    }
    *index += 1;
    for (i, child) in node.children.iter().enumerate() {
        path.push(i);
        search_node(child, predicate, results, index, path);
        path.pop();
    }
}

/// Compute the maximum depth of the tree. Returns 0 for an empty tree.
pub fn tree_depth<T>(model: &TreeModel<T>) -> u32 {
    fn node_max_depth<T>(node: &TreeNode<T>) -> u32 {
        if node.children.is_empty() {
            node.depth
        } else {
            node.children.iter().map(|c| node_max_depth(c)).max().unwrap()
        }
    }
    model.roots.iter().map(|r| node_max_depth(r)).max().unwrap_or(0)
}

/// Return all nodes in depth-first order together with their depth and path.
pub fn flatten_tree<T>(model: &TreeModel<T>) -> Vec<FlattenedNode<'_, T>> {
    let mut out = Vec::new();
    for (root_idx, root) in model.roots.iter().enumerate() {
        flatten_node(root, &mut out, &mut vec![root_idx]);
    }
    out
}

fn flatten_node<'a, T>(
    node: &'a TreeNode<T>,
    out: &mut Vec<FlattenedNode<'a, T>>,
    path: &mut Vec<usize>,
) {
    out.push(FlattenedNode {
        data: &node.data,
        depth: node.depth,
        path: path.clone(),
    });
    for (i, child) in node.children.iter().enumerate() {
        path.push(i);
        flatten_node(child, out, path);
        path.pop();
    }
}

/// Compute aggregate statistics for the tree.
pub fn tree_statistics<T>(model: &TreeModel<T>) -> TreeStatistics {
    let mut total_nodes: usize = 0;
    let mut leaf_count: usize = 0;
    let mut max_depth: u32 = 0;
    let mut internal_nodes: usize = 0;
    let mut total_children: usize = 0;

    fn visit<T>(
        node: &TreeNode<T>,
        total: &mut usize,
        leaves: &mut usize,
        max_d: &mut u32,
        internal: &mut usize,
        children_sum: &mut usize,
    ) {
        *total += 1;
        if node.depth > *max_d {
            *max_d = node.depth;
        }
        if node.children.is_empty() {
            *leaves += 1;
        } else {
            *internal += 1;
            *children_sum += node.children.len();
            for child in &node.children {
                visit(child, total, leaves, max_d, internal, children_sum);
            }
        }
    }

    for root in &model.roots {
        visit(root, &mut total_nodes, &mut leaf_count, &mut max_depth, &mut internal_nodes, &mut total_children);
    }

    let avg_children = if internal_nodes > 0 {
        total_children as f64 / internal_nodes as f64
    } else {
        0.0
    };

    TreeStatistics {
        total_nodes,
        leaf_count,
        max_depth,
        avg_children,
    }
}

// ── Tree Serialization ──

/// Serialize a tree model to a nested indented string representation.
/// Each node is printed as `indent + label`, where `label` comes from `to_label`.
pub fn serialize_tree<T, F>(model: &TreeModel<T>, to_label: F) -> String
where
    F: Fn(&T) -> String,
{
    let mut out = String::new();
    for root in &model.roots {
        serialize_node(root, &to_label, &mut out);
    }
    out
}

fn serialize_node<T, F>(node: &TreeNode<T>, to_label: &F, out: &mut String)
where
    F: Fn(&T) -> String,
{
    let indent = "  ".repeat(node.depth as usize);
    out.push_str(&format!("{}{}\n", indent, to_label(&node.data)));
    for child in &node.children {
        serialize_node(child, to_label, out);
    }
}

/// Compute the path from root to a node identified by a predicate.
/// Returns the first match found in DFS order, or `None`.
pub fn find_node_path<T, F>(model: &TreeModel<T>, predicate: F) -> Option<Vec<usize>>
where
    F: Fn(&T) -> bool,
{
    for (i, root) in model.roots.iter().enumerate() {
        let mut path = vec![i];
        if find_path_in_node(root, &predicate, &mut path) {
            return Some(path);
        }
    }
    None
}

fn find_path_in_node<T, F>(node: &TreeNode<T>, predicate: &F, path: &mut Vec<usize>) -> bool
where
    F: Fn(&T) -> bool,
{
    if predicate(&node.data) {
        return true;
    }
    for (i, child) in node.children.iter().enumerate() {
        path.push(i);
        if find_path_in_node(child, predicate, path) {
            return true;
        }
        path.pop();
    }
    false
}

/// Subtree statistics for a single node and all its descendants.
#[derive(Debug, Clone, PartialEq)]
pub struct SubtreeStats {
    pub total_nodes: usize,
    pub leaf_count: usize,
    pub max_depth: u32,
}

/// Compute statistics for the subtree rooted at the node found via `path`.
pub fn subtree_statistics<T>(model: &TreeModel<T>, path: &[usize]) -> Option<SubtreeStats> {
    let node = node_at_path_ref(&model.roots, path)?;
    let mut total: usize = 0;
    let mut leaves: usize = 0;
    let mut max_d: u32 = 0;
    count_subtree(node, &mut total, &mut leaves, &mut max_d);
    Some(SubtreeStats { total_nodes: total, leaf_count: leaves, max_depth: max_d })
}

fn node_at_path_ref<'a, T>(nodes: &'a [TreeNode<T>], path: &[usize]) -> Option<&'a TreeNode<T>> {
    match path {
        [] => None,
        [idx] => nodes.get(*idx),
        [idx, rest @ ..] => nodes.get(*idx).and_then(|n| node_at_path_ref(&n.children, rest)),
    }
}

fn count_subtree<T>(node: &TreeNode<T>, total: &mut usize, leaves: &mut usize, max_d: &mut u32) {
    *total += 1;
    if node.depth > *max_d {
        *max_d = node.depth;
    }
    if node.children.is_empty() {
        *leaves += 1;
    } else {
        for child in &node.children {
            count_subtree(child, total, leaves, max_d);
        }
    }
}

/// Count the number of leaf nodes in the entire tree.
pub fn leaf_count<T>(model: &TreeModel<T>) -> usize {
    fn count_leaves<T>(node: &TreeNode<T>) -> usize {
        if node.children.is_empty() {
            1
        } else {
            node.children.iter().map(count_leaves).sum()
        }
    }
    model.roots.iter().map(count_leaves).sum()
}

// ---------------------------------------------------------------------------
// TreeNodePath, find_by_path, and flatten_visible
// ---------------------------------------------------------------------------

/// A breadcrumb-style path for navigating trees (e.g., "src/main.rs").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TreeNodePath {
    segments: Vec<String>,
}

impl TreeNodePath {
    /// Create a path from a slash-separated string.
    pub fn from_str(path: &str) -> Self {
        Self {
            segments: path
                .split('/')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect(),
        }
    }

    /// Create a path from a vector of segments.
    pub fn from_segments(segments: Vec<String>) -> Self {
        Self { segments }
    }

    /// Create an empty path.
    pub fn empty() -> Self {
        Self { segments: Vec::new() }
    }

    /// Return the path as a slash-separated string.
    pub fn to_string_path(&self) -> String {
        self.segments.join("/")
    }

    /// Return the number of segments.
    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    /// Return the last segment (file/folder name).
    pub fn name(&self) -> Option<&str> {
        self.segments.last().map(|s| s.as_str())
    }

    /// Return the parent path (all segments except the last).
    pub fn parent(&self) -> Option<TreeNodePath> {
        if self.segments.len() <= 1 {
            return None;
        }
        Some(TreeNodePath {
            segments: self.segments[..self.segments.len() - 1].to_vec(),
        })
    }

    /// Append a segment to this path.
    pub fn join(&self, segment: impl Into<String>) -> TreeNodePath {
        let mut segments = self.segments.clone();
        segments.push(segment.into());
        TreeNodePath { segments }
    }

    /// Check if this path starts with another path.
    pub fn starts_with(&self, prefix: &TreeNodePath) -> bool {
        if prefix.segments.len() > self.segments.len() {
            return false;
        }
        self.segments[..prefix.segments.len()] == prefix.segments[..]
    }

    /// Return the segments.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Returns true if the path is empty.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

impl std::fmt::Display for TreeNodePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_path())
    }
}

/// Find a node in the tree by matching segments against a label function.
///
/// Walks the tree depth-first, matching each segment of the path against
/// nodes at corresponding depths. Returns the index path if found.
pub fn find_by_path<T, F>(model: &TreeModel<T>, path: &TreeNodePath, to_label: &F) -> Option<Vec<usize>>
where
    F: Fn(&T) -> String,
{
    if path.is_empty() {
        return None;
    }

    let segments = path.segments();
    // Find matching root
    for (root_idx, root) in model.roots.iter().enumerate() {
        if to_label(&root.data) == segments[0] {
            if segments.len() == 1 {
                return Some(vec![root_idx]);
            }
            let mut result_path = vec![root_idx];
            if find_by_path_recursive(root, &segments[1..], to_label, &mut result_path) {
                return Some(result_path);
            }
        }
    }
    None
}

fn find_by_path_recursive<T, F>(
    node: &TreeNode<T>,
    remaining: &[String],
    to_label: &F,
    path: &mut Vec<usize>,
) -> bool
where
    F: Fn(&T) -> String,
{
    if remaining.is_empty() {
        return true;
    }
    for (i, child) in node.children.iter().enumerate() {
        if to_label(&child.data) == remaining[0] {
            path.push(i);
            if remaining.len() == 1 {
                return true;
            }
            if find_by_path_recursive(child, &remaining[1..], to_label, path) {
                return true;
            }
            path.pop();
        }
    }
    false
}

/// A visible tree item with its rendered indent prefix and label.
#[derive(Debug, Clone)]
pub struct VisibleTreeLine {
    pub indent: String,
    pub label: String,
    pub depth: u32,
    pub is_expanded: bool,
    pub has_children: bool,
}

/// Flatten the visible tree into renderable lines with indent prefixes.
///
/// Only expanded nodes' children are included. The `to_label` closure
/// converts node data to display strings.
pub fn tree_flatten_visible<T, F>(model: &TreeModel<T>, to_label: &F) -> Vec<VisibleTreeLine>
where
    F: Fn(&T) -> String,
{
    let mut lines = Vec::new();
    for root in &model.roots {
        flatten_visible_node(root, to_label, &mut lines);
    }
    lines
}

fn flatten_visible_node<T, F>(
    node: &TreeNode<T>,
    to_label: &F,
    lines: &mut Vec<VisibleTreeLine>,
) where
    F: Fn(&T) -> String,
{
    let indent = "  ".repeat(node.depth as usize);
    let prefix = if !node.children.is_empty() {
        if node.is_expanded { "▼ " } else { "▶ " }
    } else {
        "  "
    };
    lines.push(VisibleTreeLine {
        indent: format!("{indent}{prefix}"),
        label: to_label(&node.data),
        depth: node.depth,
        is_expanded: node.is_expanded,
        has_children: !node.children.is_empty(),
    });
    if node.is_expanded {
        for child in &node.children {
            flatten_visible_node(child, to_label, lines);
        }
    }
}

/// Render a tree to a string suitable for terminal display.
pub fn render_tree<T, F>(model: &TreeModel<T>, to_label: &F) -> String
where
    F: Fn(&T) -> String,
{
    let lines = tree_flatten_visible(model, to_label);
    lines
        .iter()
        .map(|l| format!("{}{}", l.indent, l.label))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_model() -> TreeModel<&'static str> {
        let mut model = TreeModel::new();
        {
            let src = model.add_root("src");
            src.add_child("main.rs");
            let lib = src.add_child("lib");
            lib.add_child("mod.rs");
            lib.add_child("utils.rs");
        }
        model.add_root("Cargo.toml");
        model
    }

    #[test]
    fn build_tree_and_count() {
        let model = sample_model();
        // src (1) + main.rs (2) + lib (3) + mod.rs (4) + utils.rs (5) + Cargo.toml (6)
        assert_eq!(model.node_count(), 6);
    }

    #[test]
    fn flatten_collapsed() {
        let model = sample_model();
        // All collapsed: only roots visible.
        let items = model.flatten();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].node.data, "src");
        assert_eq!(items[0].has_children, true);
        assert_eq!(items[1].node.data, "Cargo.toml");
        assert_eq!(items[1].has_children, false);
    }

    #[test]
    fn flatten_expanded_root() {
        let mut model = sample_model();
        model.toggle_expanded(&[0]); // expand "src"
        let items = model.flatten();
        // src, main.rs, lib, Cargo.toml
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].node.data, "src");
        assert_eq!(items[0].depth, 0);
        assert_eq!(items[1].node.data, "main.rs");
        assert_eq!(items[1].depth, 1);
        assert_eq!(items[2].node.data, "lib");
        assert_eq!(items[2].depth, 1);
        assert_eq!(items[2].has_children, true);
        assert_eq!(items[3].node.data, "Cargo.toml");
    }

    #[test]
    fn flatten_fully_expanded() {
        let mut model = sample_model();
        model.expand_all();
        let items = model.flatten();
        assert_eq!(items.len(), 6);
        assert_eq!(items[3].node.data, "mod.rs");
        assert_eq!(items[3].depth, 2);
        assert_eq!(items[4].node.data, "utils.rs");
        assert_eq!(items[4].depth, 2);
    }

    #[test]
    fn collapse_all() {
        let mut model = sample_model();
        model.expand_all();
        assert_eq!(model.visible_count(), 6);
        model.collapse_all();
        assert_eq!(model.visible_count(), 2);
    }

    #[test]
    fn toggle_expand_nested() {
        let mut model = sample_model();
        model.toggle_expanded(&[0]); // expand src
        model.toggle_expanded(&[0, 1]); // expand lib
        let items = model.flatten();
        assert_eq!(items.len(), 6);
        // collapse src
        model.toggle_expanded(&[0]);
        assert_eq!(model.visible_count(), 2);
    }

    #[test]
    fn visible_count_matches_flatten_len() {
        let mut model = sample_model();
        assert_eq!(model.visible_count(), model.flatten().len());
        model.expand_all();
        assert_eq!(model.visible_count(), model.flatten().len());
    }

    #[test]
    fn virtual_scroll_viewport() {
        let mut model = sample_model();
        model.expand_all();
        let mut view = TreeView::new(model);
        view.set_viewport_height(3);

        let items = view.get_visible_items();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].node.data, "src");
        assert_eq!(items[2].node.data, "lib");

        view.scroll_down();
        let items = view.get_visible_items();
        assert_eq!(items[0].node.data, "main.rs");
        assert_eq!(items[2].node.data, "mod.rs");
    }

    #[test]
    fn scroll_bounds() {
        let mut model = sample_model();
        model.expand_all(); // 6 items
        let mut view = TreeView::new(model);
        view.set_viewport_height(3);

        // Scroll up at top does nothing.
        view.scroll_up();
        assert_eq!(view.scroll_offset, 0);

        // Scroll to the end.
        for _ in 0..10 {
            view.scroll_down();
        }
        // Max offset = 6 - 3 = 3
        assert_eq!(view.scroll_offset, 3);
        let items = view.get_visible_items();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].node.data, "mod.rs");
    }

    #[test]
    fn selection_navigation() {
        let mut model = sample_model();
        model.expand_all();
        let mut view = TreeView::new(model);
        view.set_viewport_height(3);

        assert!(view.selected().is_none());

        view.select_next();
        assert_eq!(view.selected().unwrap().node.data, "src");

        view.select_next();
        assert_eq!(view.selected().unwrap().node.data, "main.rs");

        view.select_previous();
        assert_eq!(view.selected().unwrap().node.data, "src");

        // At the top, select_previous stays at 0.
        view.select_previous();
        assert_eq!(view.selected().unwrap().node.data, "src");
    }

    #[test]
    fn selection_at_end() {
        let mut model = sample_model();
        model.expand_all(); // 6 items
        let mut view = TreeView::new(model);

        for _ in 0..10 {
            view.select_next();
        }
        // Should stop at last item.
        assert_eq!(view.selected().unwrap().node.data, "Cargo.toml");
    }

    #[test]
    fn toggle_selected_expands_and_collapses() {
        let model = sample_model();
        let mut view = TreeView::new(model);
        view.set_viewport_height(10);

        // Select first item (src) and toggle to expand it.
        view.select_next();
        assert_eq!(view.selected().unwrap().node.data, "src");
        assert_eq!(view.selected().unwrap().is_expanded, false);

        view.toggle_selected();
        assert_eq!(view.selected().unwrap().is_expanded, true);

        // Visible count should now include src's children.
        let items = view.get_visible_items();
        assert_eq!(items.len(), 4); // src, main.rs, lib, Cargo.toml

        // Toggle again to collapse.
        view.toggle_selected();
        assert_eq!(view.selected().unwrap().is_expanded, false);
        let items = view.get_visible_items();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn ensure_selected_visible_scrolls_down() {
        let mut model = sample_model();
        model.expand_all(); // 6 items
        let mut view = TreeView::new(model);
        view.set_viewport_height(3);

        // Select item beyond viewport.
        for _ in 0..5 {
            view.select_next();
        }
        // Selected index 4 (utils.rs), viewport should scroll.
        assert!(view.scroll_offset > 0);
        let items = view.get_visible_items();
        assert!(items.iter().any(|i| i.node.data == "utils.rs"));
    }

    #[test]
    fn ensure_selected_visible_scrolls_up() {
        let mut model = sample_model();
        model.expand_all();
        let mut view = TreeView::new(model);
        view.set_viewport_height(3);
        view.scroll_offset = 3; // scroll past beginning

        // Select first item.
        view.selected_index = Some(0);
        view.ensure_selected_visible();
        assert_eq!(view.scroll_offset, 0);
    }

    #[test]
    fn empty_tree() {
        let model: TreeModel<&str> = TreeModel::new();
        assert_eq!(model.node_count(), 0);
        assert_eq!(model.visible_count(), 0);
        assert_eq!(model.flatten().len(), 0);

        let mut view = TreeView::new(model);
        view.select_next();
        assert!(view.selected().is_none());
        view.select_previous();
        assert!(view.selected().is_none());
    }

    #[test]
    fn default_impl() {
        let model: TreeModel<i32> = TreeModel::default();
        assert_eq!(model.node_count(), 0);
    }

    #[test]
    fn search_tree_finds_matching_nodes() {
        let model = sample_model();
        let results = search_tree(&model, |data| data.ends_with(".rs"));
        assert_eq!(results.len(), 3); // main.rs, mod.rs, utils.rs
        assert_eq!(results[0].path, vec![0, 0]); // main.rs
        assert_eq!(results[1].path, vec![0, 1, 0]); // mod.rs
        assert_eq!(results[2].path, vec![0, 1, 1]); // utils.rs
    }

    #[test]
    fn search_tree_no_matches() {
        let model = sample_model();
        let results = search_tree(&model, |data| data.contains("nonexistent"));
        assert!(results.is_empty());
    }

    #[test]
    fn tree_depth_computes_max() {
        let model = sample_model();
        // deepest nodes are mod.rs and utils.rs at depth 2
        assert_eq!(tree_depth(&model), 2);

        let empty: TreeModel<&str> = TreeModel::new();
        assert_eq!(tree_depth(&empty), 0);
    }

    #[test]
    fn flatten_tree_returns_all_nodes_dfs() {
        let model = sample_model();
        let flat = flatten_tree(&model);
        assert_eq!(flat.len(), 6);
        let names: Vec<&&str> = flat.iter().map(|f| f.data).collect();
        assert_eq!(names, vec![&"src", &"main.rs", &"lib", &"mod.rs", &"utils.rs", &"Cargo.toml"]);
        // Verify depths
        let depths: Vec<u32> = flat.iter().map(|f| f.depth).collect();
        assert_eq!(depths, vec![0, 1, 1, 2, 2, 0]);
    }

    #[test]
    fn tree_statistics_correct() {
        let model = sample_model();
        let stats = tree_statistics(&model);
        assert_eq!(stats.total_nodes, 6);
        assert_eq!(stats.leaf_count, 4); // main.rs, mod.rs, utils.rs, Cargo.toml
        assert_eq!(stats.max_depth, 2);
        // Internal nodes: src (2 children) and lib (2 children) => avg = 2.0
        assert!((stats.avg_children - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn tree_statistics_empty() {
        let empty: TreeModel<&str> = TreeModel::new();
        let stats = tree_statistics(&empty);
        assert_eq!(stats.total_nodes, 0);
        assert_eq!(stats.leaf_count, 0);
        assert_eq!(stats.max_depth, 0);
        assert!((stats.avg_children - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn search_result_depth_and_index() {
        let model = sample_model();
        let results = search_tree(&model, |data| *data == "lib");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].depth, 1);
        assert_eq!(results[0].node_index, 2); // src(0), main.rs(1), lib(2)
        assert_eq!(results[0].path, vec![0, 1]);
    }

    #[test]
    fn serialize_tree_produces_indented_output() {
        let model = sample_model();
        let output = serialize_tree(&model, |d| d.to_string());
        assert!(output.contains("src\n"));
        assert!(output.contains("  main.rs\n"));
        assert!(output.contains("  lib\n"));
        assert!(output.contains("    mod.rs\n"));
        assert!(output.contains("Cargo.toml\n"));
    }

    #[test]
    fn find_node_path_locates_nested_node() {
        let model = sample_model();
        let path = find_node_path(&model, |d| *d == "utils.rs");
        assert_eq!(path, Some(vec![0, 1, 1]));
    }

    #[test]
    fn find_node_path_returns_none_for_missing() {
        let model = sample_model();
        assert!(find_node_path(&model, |d| *d == "missing").is_none());
    }

    #[test]
    fn subtree_statistics_for_subtree() {
        let model = sample_model();
        let stats = subtree_statistics(&model, &[0, 1]).unwrap(); // "lib" subtree
        assert_eq!(stats.total_nodes, 3); // lib, mod.rs, utils.rs
        assert_eq!(stats.leaf_count, 2);
        assert_eq!(stats.max_depth, 2);
    }

    #[test]
    fn subtree_statistics_returns_none_for_bad_path() {
        let model = sample_model();
        assert!(subtree_statistics(&model, &[9]).is_none());
    }

    #[test]
    fn leaf_count_counts_leaves() {
        let model = sample_model();
        assert_eq!(leaf_count(&model), 4); // main.rs, mod.rs, utils.rs, Cargo.toml
        let empty: TreeModel<&str> = TreeModel::new();
        assert_eq!(leaf_count(&empty), 0);
    }

    #[test]
    fn tree_node_path_from_str() {
        let p = TreeNodePath::from_str("src/main.rs");
        assert_eq!(p.depth(), 2);
        assert_eq!(p.name(), Some("main.rs"));
        assert_eq!(p.to_string_path(), "src/main.rs");
    }

    #[test]
    fn tree_node_path_parent() {
        let p = TreeNodePath::from_str("src/lib/mod.rs");
        let parent = p.parent().unwrap();
        assert_eq!(parent.to_string_path(), "src/lib");
        let grandparent = parent.parent().unwrap();
        assert_eq!(grandparent.to_string_path(), "src");
        assert!(grandparent.parent().is_none());
    }

    #[test]
    fn tree_node_path_join() {
        let p = TreeNodePath::from_str("src");
        let joined = p.join("main.rs");
        assert_eq!(joined.to_string_path(), "src/main.rs");
    }

    #[test]
    fn tree_node_path_starts_with() {
        let p = TreeNodePath::from_str("src/lib/mod.rs");
        let prefix = TreeNodePath::from_str("src/lib");
        assert!(p.starts_with(&prefix));
        let non_prefix = TreeNodePath::from_str("tests");
        assert!(!p.starts_with(&non_prefix));
    }

    #[test]
    fn tree_node_path_empty() {
        let p = TreeNodePath::empty();
        assert!(p.is_empty());
        assert_eq!(p.depth(), 0);
        assert!(p.name().is_none());
    }

    #[test]
    fn tree_node_path_display() {
        let p = TreeNodePath::from_str("src/main.rs");
        assert_eq!(format!("{p}"), "src/main.rs");
    }

    #[test]
    fn find_by_path_finds_root() {
        let model = sample_model();
        let path = TreeNodePath::from_str("src");
        let result = find_by_path(&model, &path, &|d: &&str| d.to_string());
        assert_eq!(result, Some(vec![0]));
    }

    #[test]
    fn find_by_path_finds_nested() {
        let model = sample_model();
        let path = TreeNodePath::from_str("src/lib/utils.rs");
        let result = find_by_path(&model, &path, &|d: &&str| d.to_string());
        assert_eq!(result, Some(vec![0, 1, 1]));
    }

    #[test]
    fn find_by_path_returns_none_for_missing() {
        let model = sample_model();
        let path = TreeNodePath::from_str("missing/file.rs");
        assert!(find_by_path(&model, &path, &|d: &&str| d.to_string()).is_none());
    }

    #[test]
    fn find_by_path_empty_path() {
        let model = sample_model();
        let path = TreeNodePath::empty();
        assert!(find_by_path(&model, &path, &|d: &&str| d.to_string()).is_none());
    }

    #[test]
    fn tree_flatten_visible_collapsed() {
        let model = sample_model();
        let lines = tree_flatten_visible(&model, &|d: &&str| d.to_string());
        assert_eq!(lines.len(), 2); // only roots visible
        assert!(lines[0].has_children);
        assert!(!lines[0].is_expanded);
        assert!(lines[0].indent.contains("▶"));
    }

    #[test]
    fn tree_flatten_visible_expanded() {
        let mut model = sample_model();
        model.expand_all();
        let lines = tree_flatten_visible(&model, &|d: &&str| d.to_string());
        assert_eq!(lines.len(), 6); // all nodes visible
        assert!(lines[0].indent.contains("▼"));
        assert_eq!(lines[3].label, "mod.rs");
        assert_eq!(lines[3].depth, 2);
    }

    #[test]
    fn render_tree_output() {
        let mut model = sample_model();
        model.toggle_expanded(&[0]); // expand src
        let output = render_tree(&model, &|d: &&str| d.to_string());
        assert!(output.contains("▼ src"));
        assert!(output.contains("  main.rs"));
        assert!(output.contains("  ▶ lib"));
        assert!(output.contains("Cargo.toml"));
    }
}
