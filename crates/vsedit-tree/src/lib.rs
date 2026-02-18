//! Virtual scrolling tree view widget.

use std::collections::HashMap;

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

// ---------------------------------------------------------------------------
// TreeNode helpers
// ---------------------------------------------------------------------------

impl<T> TreeNode<T> {
    /// Returns true if this node has no children.
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Returns the number of direct children.
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Returns the maximum depth in this subtree.
    pub fn max_depth(&self) -> u32 {
        if self.children.is_empty() {
            self.depth
        } else {
            self.children.iter().map(|c| c.max_depth()).max().unwrap_or(self.depth)
        }
    }

    /// Apply a function to this node and all descendants (depth-first).
    pub fn for_each<F: FnMut(&TreeNode<T>)>(&self, f: &mut F) {
        f(self);
        for child in &self.children {
            child.for_each(f);
        }
    }

    /// Apply a mutable function to this node and all descendants.
    pub fn for_each_mut<F: FnMut(&mut TreeNode<T>)>(&mut self, f: &mut F) {
        f(self);
        for child in &mut self.children {
            child.for_each_mut(f);
        }
    }

    /// Expand this node and all descendants.
    pub fn expand_all(&mut self) {
        self.is_expanded = true;
        for child in &mut self.children {
            child.expand_all();
        }
    }

    /// Collapse this node and all descendants.
    pub fn collapse_all(&mut self) {
        self.is_expanded = false;
        for child in &mut self.children {
            child.collapse_all();
        }
    }
}

// ---------------------------------------------------------------------------
// TreeModel helpers
// ---------------------------------------------------------------------------

impl<T> TreeModel<T> {
    /// Returns total number of root nodes.
    pub fn root_count(&self) -> usize {
        self.roots.len()
    }

    /// Returns true if the model has no nodes.
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    /// Count all leaf nodes in the tree.
    pub fn count_leaves(&self) -> usize {
        fn count_leaves_node<T>(node: &TreeNode<T>) -> usize {
            if node.children.is_empty() {
                1
            } else {
                node.children.iter().map(count_leaves_node).sum()
            }
        }
        self.roots.iter().map(count_leaves_node).sum()
    }

    /// Collect all leaf node data references.
    pub fn collect_leaves(&self) -> Vec<&T> {
        fn collect<'a, T>(node: &'a TreeNode<T>, out: &mut Vec<&'a T>) {
            if node.children.is_empty() {
                out.push(&node.data);
            } else {
                for child in &node.children {
                    collect(child, out);
                }
            }
        }
        let mut result = Vec::new();
        for root in &self.roots {
            collect(root, &mut result);
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Tree depth iterator
// ---------------------------------------------------------------------------

/// Iterator that yields nodes at a specific depth.
pub struct DepthIter<'a, T> {
    stack: Vec<&'a TreeNode<T>>,
    target_depth: u32,
}

impl<'a, T> DepthIter<'a, T> {
    pub fn new(roots: &'a [TreeNode<T>], depth: u32) -> Self {
        Self {
            stack: roots.iter().rev().collect(),
            target_depth: depth,
        }
    }
}

impl<'a, T> Iterator for DepthIter<'a, T> {
    type Item = &'a TreeNode<T>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(node) = self.stack.pop() {
            if node.depth == self.target_depth {
                return Some(node);
            }
            if node.depth < self.target_depth {
                for child in node.children.iter().rev() {
                    self.stack.push(child);
                }
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// TreeFilter<T> – predicate-based visible-node filtering
// ---------------------------------------------------------------------------

/// Filters visible tree nodes using a predicate function.
pub struct TreeFilter<T> {
    predicate: Option<Box<dyn Fn(&T) -> bool>>,
    _marker: std::marker::PhantomData<T>,
}

impl<T> TreeFilter<T> {
    pub fn new() -> Self {
        Self { predicate: None, _marker: std::marker::PhantomData }
    }

    /// Set the filter predicate. Only nodes matching it will be included in `apply`.
    pub fn set_filter(&mut self, pred: Box<dyn Fn(&T) -> bool>) {
        self.predicate = Some(pred);
    }

    /// Clear the filter so all nodes are included.
    pub fn clear_filter(&mut self) {
        self.predicate = None;
    }

    /// Returns true if a filter is currently set.
    pub fn has_filter(&self) -> bool {
        self.predicate.is_some()
    }

    /// Apply the filter to a model, returning references to matching nodes.
    pub fn apply<'a>(&self, model: &'a TreeModel<T>) -> Vec<&'a T> {
        let mut results = Vec::new();
        for root in &model.roots {
            self.collect_matching(root, &mut results);
        }
        results
    }

    fn collect_matching<'a>(&self, node: &'a TreeNode<T>, out: &mut Vec<&'a T>) {
        let matches = match &self.predicate {
            Some(pred) => pred(&node.data),
            None => true,
        };
        if matches {
            out.push(&node.data);
        }
        for child in &node.children {
            self.collect_matching(child, out);
        }
    }
}

// ---------------------------------------------------------------------------
// TreeSearchResultWithPath<T> – search with path tracking
// ---------------------------------------------------------------------------

/// A search result that includes the matched data reference and the path to it.
pub struct TreeSearchResultWithPath<'a, T> {
    pub data: &'a T,
    pub path: Vec<usize>,
    pub depth: u32,
}

/// Search the tree for nodes matching a predicate, returning full paths.
pub fn tree_search_with_paths<'a, T, F>(
    model: &'a TreeModel<T>,
    predicate: F,
) -> Vec<TreeSearchResultWithPath<'a, T>>
where
    F: Fn(&T) -> bool,
{
    let mut results = Vec::new();
    for (i, root) in model.roots.iter().enumerate() {
        let mut path = vec![i];
        search_node_with_path(root, &predicate, &mut path, &mut results);
        path.pop();
    }
    results
}

fn search_node_with_path<'a, T, F>(
    node: &'a TreeNode<T>,
    predicate: &F,
    path: &mut Vec<usize>,
    results: &mut Vec<TreeSearchResultWithPath<'a, T>>,
) where
    F: Fn(&T) -> bool,
{
    if predicate(&node.data) {
        results.push(TreeSearchResultWithPath {
            data: &node.data,
            path: path.clone(),
            depth: node.depth,
        });
    }
    for (i, child) in node.children.iter().enumerate() {
        path.push(i);
        search_node_with_path(child, predicate, path, results);
        path.pop();
    }
}

// ---------------------------------------------------------------------------
// TreeFilterStats – compute filter-aware statistics about a tree model
// ---------------------------------------------------------------------------

/// Statistics about the current state of a tree model (filter-aware).
pub struct TreeFilterStats {
    pub total: usize,
    pub expanded: usize,
    pub collapsed: usize,
    pub leaf: usize,
}

/// Compute statistics about the current state of a tree model.
pub fn compute_tree_stats<T>(model: &TreeModel<T>) -> TreeFilterStats {
    let mut stats = TreeFilterStats {
        total: 0,
        expanded: 0,
        collapsed: 0,
        leaf: 0,
    };
    for root in &model.roots {
        count_stats(root, &mut stats);
    }
    stats
}

fn count_stats<T>(node: &TreeNode<T>, stats: &mut TreeFilterStats) {
    stats.total += 1;
    if node.children.is_empty() {
        stats.leaf += 1;
    } else if node.is_expanded {
        stats.expanded += 1;
    } else {
        stats.collapsed += 1;
    }
    for child in &node.children {
        count_stats(child, stats);
    }
}

impl std::fmt::Display for TreeFilterStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} total, {} expanded, {} collapsed, {} leaf",
            self.total, self.expanded, self.collapsed, self.leaf,
        )
    }
}

/// Collect all leaf nodes from a tree model.
pub fn collect_leaves<'a, T>(model: &'a TreeModel<T>) -> Vec<&'a T> {
    let mut leaves = Vec::new();
    for root in &model.roots {
        collect_leaves_node(root, &mut leaves);
    }
    leaves
}

fn collect_leaves_node<'a, T>(node: &'a TreeNode<T>, out: &mut Vec<&'a T>) {
    if node.children.is_empty() {
        out.push(&node.data);
    }
    for child in &node.children {
        collect_leaves_node(child, out);
    }
}

/// Count the number of expanded nodes in the entire tree.
pub fn expanded_count<T>(model: &TreeModel<T>) -> usize {
    fn count_expanded_node<T>(node: &TreeNode<T>) -> usize {
        let me = if node.is_expanded { 1 } else { 0 };
        me + node.children.iter().map(count_expanded_node).sum::<usize>()
    }
    model.roots.iter().map(count_expanded_node).sum()
}

/// Collect all node data at a specific depth level.
pub fn nodes_at_depth<'a, T>(model: &'a TreeModel<T>, target_depth: u32) -> Vec<&'a T> {
    fn collect_at_depth<'a, T>(node: &'a TreeNode<T>, target: u32, out: &mut Vec<&'a T>) {
        if node.depth == target {
            out.push(&node.data);
        }
        for child in &node.children {
            collect_at_depth(child, target, out);
        }
    }
    let mut result = Vec::new();
    for root in &model.roots {
        collect_at_depth(root, target_depth, &mut result);
    }
    result
}

/// Check if the tree model has any nodes at all.
pub fn is_tree_empty<T>(model: &TreeModel<T>) -> bool {
    model.roots.is_empty()
}

/// Count internal (non-leaf) nodes in the tree.
pub fn internal_node_count<T>(model: &TreeModel<T>) -> usize {
    fn count_internal<T>(node: &TreeNode<T>) -> usize {
        let me = if node.children.is_empty() { 0 } else { 1 };
        me + node.children.iter().map(count_internal).sum::<usize>()
    }
    model.roots.iter().map(count_internal).sum()
}

/// Compute the width of the widest level (most nodes at the same depth).
pub fn max_breadth<T>(model: &TreeModel<T>) -> usize {
    let max_depth = tree_depth(model);
    (0..=max_depth)
        .map(|d| nodes_at_depth(model, d).len())
        .max()
        .unwrap_or(0)
}

/// Collect all root-level data references.
pub fn root_data<'a, T>(model: &'a TreeModel<T>) -> Vec<&'a T> {
    model.roots.iter().map(|r| &r.data).collect()
}

/// Return the depth of the first node matching the predicate, or None.
pub fn find_depth<T, F>(model: &TreeModel<T>, predicate: F) -> Option<u32>
where
    F: Fn(&T) -> bool,
{
    fn search<T, F>(node: &TreeNode<T>, pred: &F) -> Option<u32>
    where
        F: Fn(&T) -> bool,
    {
        if pred(&node.data) {
            return Some(node.depth);
        }
        for child in &node.children {
            if let Some(d) = search(child, pred) {
                return Some(d);
            }
        }
        None
    }
    for root in &model.roots {
        if let Some(d) = search(root, &predicate) {
            return Some(d);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// map_tree – transform all node data producing a new tree
// ---------------------------------------------------------------------------

/// Transform every node's data using the given function, producing a new `TreeModel`.
pub fn map_tree<T, U, F>(model: &TreeModel<T>, f: &F) -> TreeModel<U>
where
    F: Fn(&T) -> U,
{
    TreeModel {
        roots: model.roots.iter().map(|r| map_node(r, f)).collect(),
    }
}

fn map_node<T, U, F>(node: &TreeNode<T>, f: &F) -> TreeNode<U>
where
    F: Fn(&T) -> U,
{
    TreeNode {
        data: f(&node.data),
        children: node.children.iter().map(|c| map_node(c, f)).collect(),
        is_expanded: node.is_expanded,
        depth: node.depth,
    }
}

// ---------------------------------------------------------------------------
// filter_tree – keep only nodes matching a predicate (ancestors preserved)
// ---------------------------------------------------------------------------

/// Create a new tree keeping only nodes where `predicate` returns true.
///
/// A parent node is retained if any of its descendants match, ensuring the
/// tree structure stays connected. The `default` closure produces placeholder
/// data for retained-but-unmatched ancestors.
pub fn filter_tree<T: Clone>(
    model: &TreeModel<T>,
    predicate: &dyn Fn(&T) -> bool,
) -> TreeModel<T> {
    TreeModel {
        roots: model
            .roots
            .iter()
            .filter_map(|r| filter_node(r, predicate))
            .collect(),
    }
}

fn filter_node<T: Clone>(
    node: &TreeNode<T>,
    predicate: &dyn Fn(&T) -> bool,
) -> Option<TreeNode<T>> {
    let filtered_children: Vec<TreeNode<T>> = node
        .children
        .iter()
        .filter_map(|c| filter_node(c, predicate))
        .collect();

    if predicate(&node.data) || !filtered_children.is_empty() {
        Some(TreeNode {
            data: node.data.clone(),
            children: filtered_children,
            is_expanded: node.is_expanded,
            depth: node.depth,
        })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// ancestors – collect ancestor data along a path
// ---------------------------------------------------------------------------

/// Given an index-path, return data references for every ancestor from root
/// down to (but not including) the final node.
pub fn ancestors<'a, T>(model: &'a TreeModel<T>, path: &[usize]) -> Vec<&'a T> {
    let mut result = Vec::new();
    let mut nodes: &[TreeNode<T>] = &model.roots;
    for &idx in path.iter().take(path.len().saturating_sub(1)) {
        if let Some(node) = nodes.get(idx) {
            result.push(&node.data);
            nodes = &node.children;
        } else {
            break;
        }
    }
    result
}

// ---------------------------------------------------------------------------
// breadth_first – BFS traversal collecting data references
// ---------------------------------------------------------------------------

/// Iterate all nodes in breadth-first order, returning data references.
pub fn breadth_first<'a, T>(model: &'a TreeModel<T>) -> Vec<&'a T> {
    let mut result = Vec::new();
    let mut queue = std::collections::VecDeque::new();
    for root in &model.roots {
        queue.push_back(root);
    }
    while let Some(node) = queue.pop_front() {
        result.push(&node.data);
        for child in &node.children {
            queue.push_back(child);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// merge_trees – combine two tree models into one
// ---------------------------------------------------------------------------

/// Merge two tree models by appending the roots of `other` after `base`.
pub fn merge_trees<T>(base: TreeModel<T>, other: TreeModel<T>) -> TreeModel<T> {
    let mut roots = base.roots;
    roots.extend(other.roots);
    TreeModel { roots }
}

// ---------------------------------------------------------------------------
// TreeNode extra methods
// ---------------------------------------------------------------------------

impl<T> TreeNode<T> {
    /// Find the first node (DFS) whose data satisfies the predicate.
    pub fn find<F: Fn(&T) -> bool>(&self, predicate: &F) -> Option<&TreeNode<T>> {
        if predicate(&self.data) {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find(predicate) {
                return Some(found);
            }
        }
        None
    }

    /// Collect data from all nodes in this subtree (DFS order).
    pub fn collect_all_data(&self) -> Vec<&T> {
        let mut out = Vec::new();
        self.collect_data_into(&mut out);
        out
    }

    fn collect_data_into<'a>(&'a self, out: &mut Vec<&'a T>) {
        out.push(&self.data);
        for child in &self.children {
            child.collect_data_into(out);
        }
    }

    /// Count all descendants (not including self).
    pub fn descendant_count(&self) -> usize {
        self.total_count() - 1
    }

    /// Return the depth of the deepest leaf relative to this node.
    pub fn height(&self) -> u32 {
        if self.children.is_empty() {
            0
        } else {
            1 + self
                .children
                .iter()
                .map(|c| c.height())
                .max()
                .unwrap_or(0)
        }
    }
}

// ---------------------------------------------------------------------------
// TreeModel extra methods
// ---------------------------------------------------------------------------

impl<T> TreeModel<T> {
    /// Find the first node (DFS) matching the predicate.
    pub fn find_node<F: Fn(&T) -> bool>(&self, predicate: &F) -> Option<&TreeNode<T>> {
        for root in &self.roots {
            if let Some(found) = root.find(predicate) {
                return Some(found);
            }
        }
        None
    }

    /// Access a node by index-path (immutable).
    pub fn node_at_path(&self, path: &[usize]) -> Option<&TreeNode<T>> {
        node_at_path_ref(&self.roots, path)
    }

    /// Iterate all node data in BFS order.
    pub fn breadth_first_data(&self) -> Vec<&T> {
        breadth_first(self)
    }

    /// Compute the height of the tallest root subtree.
    pub fn tree_height(&self) -> u32 {
        self.roots.iter().map(|r| r.height()).max().unwrap_or(0)
    }

    /// Return true if any node matches the predicate.
    pub fn contains<F: Fn(&T) -> bool>(&self, predicate: &F) -> bool {
        self.find_node(predicate).is_some()
    }

    /// Apply a mutable visitor to every node in DFS order.
    pub fn for_each_mut<F: FnMut(&mut TreeNode<T>)>(&mut self, f: &mut F) {
        for root in &mut self.roots {
            root.for_each_mut(f);
        }
    }

    /// Expand only the nodes along the given index-path.
    pub fn expand_path(&mut self, path: &[usize]) {
        let mut nodes: &mut [TreeNode<T>] = &mut self.roots;
        for &idx in path {
            if let Some(node) = nodes.get_mut(idx) {
                node.is_expanded = true;
                nodes = &mut node.children;
            } else {
                break;
            }
        }
    }

    /// Reveal a node by expanding all its ancestors (path must include the target).
    pub fn reveal(&mut self, path: &[usize]) {
        if path.is_empty() {
            return;
        }
        // Expand all ancestors but not the target itself
        let mut nodes: &mut [TreeNode<T>] = &mut self.roots;
        for &idx in &path[..path.len() - 1] {
            if let Some(node) = nodes.get_mut(idx) {
                node.is_expanded = true;
                nodes = &mut node.children;
            } else {
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// serialize_tree_box_drawing – pretty print with box-drawing characters
// ---------------------------------------------------------------------------

/// Render a tree using box-drawing characters (├── └── │).
pub fn serialize_tree_box<T, F>(model: &TreeModel<T>, to_label: &F) -> String
where
    F: Fn(&T) -> String,
{
    let mut out = String::new();
    let root_count = model.roots.len();
    for (i, root) in model.roots.iter().enumerate() {
        let is_last = i + 1 == root_count;
        serialize_box_node(root, to_label, &mut out, "", is_last);
    }
    out
}

fn serialize_box_node<T, F>(
    node: &TreeNode<T>,
    to_label: &F,
    out: &mut String,
    prefix: &str,
    is_last: bool,
) where
    F: Fn(&T) -> String,
{
    let connector = if node.depth == 0 {
        ""
    } else if is_last {
        "└── "
    } else {
        "├── "
    };
    out.push_str(&format!("{}{}{}\n", prefix, connector, to_label(&node.data)));

    let child_prefix = if node.depth == 0 {
        String::new()
    } else {
        format!("{}{}", prefix, if is_last { "    " } else { "│   " })
    };

    let child_count = node.children.len();
    for (i, child) in node.children.iter().enumerate() {
        serialize_box_node(child, to_label, out, &child_prefix, i + 1 == child_count);
    }
}


// ── Tree Flatten Iterator ──

/// Iterator that yields references to all nodes in depth-first order,
/// regardless of expansion state.
pub struct TreeFlattenIterator<'a, T> {
    stack: Vec<&'a TreeNode<T>>,
}

impl<'a, T> TreeFlattenIterator<'a, T> {
    /// Create a new flatten iterator from a tree model.
    pub fn from_model(model: &'a TreeModel<T>) -> Self {
        let mut stack: Vec<&'a TreeNode<T>> = Vec::new();
        // Push roots in reverse so the first root is visited first.
        for root in model.roots.iter().rev() {
            stack.push(root);
        }
        Self { stack }
    }

    /// Create from a single node.
    pub fn from_node(node: &'a TreeNode<T>) -> Self {
        Self { stack: vec![node] }
    }
}

impl<'a, T> Iterator for TreeFlattenIterator<'a, T> {
    type Item = &'a TreeNode<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;
        // Push children in reverse order so left-most child is visited first.
        for child in node.children.iter().rev() {
            self.stack.push(child);
        }
        Some(node)
    }
}

// ── Tree Path Serializer ──

/// Serializes and deserializes tree paths as slash-separated index strings.
pub struct TreePathSerializer;

impl TreePathSerializer {
    /// Serialize a path of indices to a string like "0/1/2".
    pub fn serialize(path: &[usize]) -> String {
        path.iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("/")
    }

    /// Deserialize a path string back to indices.
    pub fn deserialize(s: &str) -> Option<Vec<usize>> {
        if s.is_empty() {
            return Some(Vec::new());
        }
        let mut result = Vec::new();
        for part in s.split('/') {
            match part.parse::<usize>() {
                Ok(idx) => result.push(idx),
                Err(_) => return None,
            }
        }
        Some(result)
    }

    /// Determine if `child_path` is a descendant of `ancestor_path`.
    pub fn is_descendant(ancestor_path: &str, child_path: &str) -> bool {
        if ancestor_path.is_empty() {
            return true;
        }
        child_path.starts_with(ancestor_path)
            && child_path.len() > ancestor_path.len()
            && child_path.as_bytes()[ancestor_path.len()] == b'/'
    }

    /// Return the parent path (everything before the last `/`).
    pub fn parent_path(path: &str) -> Option<String> {
        if path.is_empty() {
            return None;
        }
        match path.rfind('/') {
            Some(pos) => Some(path[..pos].to_string()),
            None => Some(String::new()),
        }
    }

    /// Depth of a path (number of segments).
    pub fn depth(path: &str) -> usize {
        if path.is_empty() {
            0
        } else {
            path.split('/').count()
        }
    }

    /// Append an index to a path.
    pub fn append(path: &str, index: usize) -> String {
        if path.is_empty() {
            index.to_string()
        } else {
            format!("{path}/{index}")
        }
    }

    /// Return the last segment (leaf index) of a path.
    pub fn leaf_index(path: &str) -> Option<usize> {
        path.rsplit('/').next()?.parse().ok()
    }
}

// ── Tree Depth Calculator ──

/// Utility for computing depth-related metrics on a tree.
pub struct TreeDepthCalculator;

impl TreeDepthCalculator {
    /// Maximum depth in the tree model (0 if empty).
    pub fn max_depth<T>(model: &TreeModel<T>) -> u32 {
        model.roots.iter().map(|r| Self::node_max_depth(r)).max().unwrap_or(0)
    }

    fn node_max_depth<T>(node: &TreeNode<T>) -> u32 {
        if node.children.is_empty() {
            node.depth
        } else {
            node.children.iter().map(|c| Self::node_max_depth(c)).max().unwrap_or(node.depth)
        }
    }

    /// Count of nodes at a given depth.
    pub fn count_at_depth<T>(model: &TreeModel<T>, target_depth: u32) -> usize {
        model.roots.iter()
            .map(|r| Self::count_at_depth_node(r, target_depth))
            .sum()
    }

    fn count_at_depth_node<T>(node: &TreeNode<T>, target_depth: u32) -> usize {
        let mut count = if node.depth == target_depth { 1 } else { 0 };
        for child in &node.children {
            count += Self::count_at_depth_node(child, target_depth);
        }
        count
    }

    /// Return the average depth of all leaf nodes.
    pub fn average_leaf_depth<T>(model: &TreeModel<T>) -> f64 {
        let mut sum: u64 = 0;
        let mut count: u64 = 0;
        for root in &model.roots {
            Self::collect_leaf_depths(root, &mut sum, &mut count);
        }
        if count == 0 { 0.0 } else { sum as f64 / count as f64 }
    }

    fn collect_leaf_depths<T>(node: &TreeNode<T>, sum: &mut u64, count: &mut u64) {
        if node.children.is_empty() {
            *sum += node.depth as u64;
            *count += 1;
        } else {
            for child in &node.children {
                Self::collect_leaf_depths(child, sum, count);
            }
        }
    }

    /// Collect all leaf nodes' data references.
    pub fn leaf_data<'a, T>(model: &'a TreeModel<T>) -> Vec<&'a T> {
        let mut leaves = Vec::new();
        for root in &model.roots {
            Self::collect_leaves(root, &mut leaves);
        }
        leaves
    }

    fn collect_leaves<'a, T>(node: &'a TreeNode<T>, out: &mut Vec<&'a T>) {
        if node.children.is_empty() {
            out.push(&node.data);
        } else {
            for child in &node.children {
                Self::collect_leaves(child, out);
            }
        }
    }
}

// ── Tree Sibling Navigator ──

/// Navigator for moving between siblings in a tree model.
pub struct TreeSiblingNavigator;

impl TreeSiblingNavigator {
    /// Return the number of siblings (including self) at the root level.
    pub fn root_sibling_count<T>(model: &TreeModel<T>) -> usize {
        model.roots.len()
    }

    /// Find the index of a root node by data equality.
    pub fn find_root_index<T: PartialEq>(model: &TreeModel<T>, data: &T) -> Option<usize> {
        model.roots.iter().position(|r| r.data == *data)
    }

    /// Get the data of the next root sibling.
    pub fn next_root_sibling<'a, T: PartialEq>(model: &'a TreeModel<T>, data: &T) -> Option<&'a T> {
        let idx = Self::find_root_index(model, data)?;
        if idx + 1 < model.roots.len() {
            Some(&model.roots[idx + 1].data)
        } else {
            None
        }
    }

    /// Get the data of the previous root sibling.
    pub fn prev_root_sibling<'a, T: PartialEq>(model: &'a TreeModel<T>, data: &T) -> Option<&'a T> {
        let idx = Self::find_root_index(model, data)?;
        if idx > 0 {
            Some(&model.roots[idx - 1].data)
        } else {
            None
        }
    }

    /// Count children of a specific root node by index.
    pub fn child_count_of_root<T>(model: &TreeModel<T>, root_index: usize) -> usize {
        model.roots.get(root_index).map_or(0, |r| r.children.len())
    }

    /// Return the index of a child within a node's children by data equality.
    pub fn find_child_index<T: PartialEq>(node: &TreeNode<T>, data: &T) -> Option<usize> {
        node.children.iter().position(|c| c.data == *data)
    }

    /// Check if a node is the first child among its siblings.
    pub fn is_first_child<T: PartialEq>(parent: &TreeNode<T>, data: &T) -> bool {
        parent.children.first().map_or(false, |c| c.data == *data)
    }

    /// Check if a node is the last child among its siblings.
    pub fn is_last_child<T: PartialEq>(parent: &TreeNode<T>, data: &T) -> bool {
        parent.children.last().map_or(false, |c| c.data == *data)
    }

    /// Get the next sibling data within a parent node.
    pub fn next_sibling<'a, T: PartialEq>(parent: &'a TreeNode<T>, data: &T) -> Option<&'a T> {
        let idx = Self::find_child_index(parent, data)?;
        if idx + 1 < parent.children.len() {
            Some(&parent.children[idx + 1].data)
        } else {
            None
        }
    }

    /// Get the previous sibling data within a parent node.
    pub fn prev_sibling<'a, T: PartialEq>(parent: &'a TreeNode<T>, data: &T) -> Option<&'a T> {
        let idx = Self::find_child_index(parent, data)?;
        if idx > 0 {
            Some(&parent.children[idx - 1].data)
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// TreeIndexPath – path from root to a node
// ---------------------------------------------------------------------------

/// Represents a path from the root of a tree to a specific node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TreeIndexPath {
    segments: Vec<usize>,
}

impl TreeIndexPath {
    pub fn new(segments: Vec<usize>) -> Self {
        Self { segments }
    }

    pub fn root() -> Self {
        Self { segments: Vec::new() }
    }

    pub fn segments(&self) -> &[usize] { &self.segments }

    pub fn depth(&self) -> usize { self.segments.len() }

    /// Return the parent path (all but the last segment).
    pub fn parent_path(&self) -> Option<TreeIndexPath> {
        if self.segments.is_empty() {
            None
        } else {
            let mut p = self.segments.clone();
            p.pop();
            Some(TreeIndexPath::new(p))
        }
    }

    /// Check if this path is an ancestor of another.
    pub fn is_ancestor_of(&self, other: &TreeIndexPath) -> bool {
        if self.segments.len() >= other.segments.len() {
            return false;
        }
        other.segments.starts_with(&self.segments)
    }

    /// Find the common ancestor path between this and another path.
    pub fn common_ancestor_with(&self, other: &TreeIndexPath) -> TreeIndexPath {
        let common: Vec<usize> = self
            .segments
            .iter()
            .zip(other.segments.iter())
            .take_while(|(a, b)| a == b)
            .map(|(a, _)| *a)
            .collect();
        TreeIndexPath::new(common)
    }

    pub fn to_string_repr(&self) -> String {
        self.segments.iter().map(|s| s.to_string()).collect::<Vec<_>>().join("/")
    }

    /// Append a child index to this path.
    pub fn child(&self, index: usize) -> TreeIndexPath {
        let mut segs = self.segments.clone();
        segs.push(index);
        TreeIndexPath::new(segs)
    }
}

// ---------------------------------------------------------------------------
// TreeExpansionState – track expanded/collapsed nodes
// ---------------------------------------------------------------------------

/// Tracks which tree nodes are expanded.
#[derive(Debug, Clone, Default)]
pub struct TreeExpansionState {
    expanded: std::collections::HashSet<Vec<usize>>,
}

impl TreeExpansionState {
    pub fn new() -> Self { Self::default() }

    pub fn expand(&mut self, path: &TreeIndexPath) {
        self.expanded.insert(path.segments().to_vec());
    }

    pub fn collapse(&mut self, path: &TreeIndexPath) {
        self.expanded.remove(path.segments());
    }

    pub fn toggle(&mut self, path: &TreeIndexPath) {
        if self.is_expanded(path) {
            self.collapse(path);
        } else {
            self.expand(path);
        }
    }

    pub fn is_expanded(&self, path: &TreeIndexPath) -> bool {
        self.expanded.contains(path.segments())
    }

    pub fn expand_all(&mut self, paths: &[TreeIndexPath]) {
        for p in paths {
            self.expand(p);
        }
    }

    pub fn collapse_all(&mut self) {
        self.expanded.clear();
    }

    pub fn expanded_count(&self) -> usize {
        self.expanded.len()
    }

    /// Snapshot the current expansion state.
    pub fn expansion_snapshot(&self) -> Vec<Vec<usize>> {
        self.expanded.iter().cloned().collect()
    }

    /// Restore from a snapshot.
    pub fn restore(&mut self, snapshot: Vec<Vec<usize>>) {
        self.expanded = snapshot.into_iter().collect();
    }
}

// ---------------------------------------------------------------------------
// TreeDragDropValidator – validate drag/drop operations
// ---------------------------------------------------------------------------

/// Validates drag and drop operations within a tree.
pub struct TreeDragDropValidator;

impl TreeDragDropValidator {
    /// Check if a node can be dropped on (into) a target node.
    pub fn can_drop_on(source: &TreeIndexPath, target: &TreeIndexPath) -> bool {
        // Cannot drop a node onto itself or one of its descendants
        source != target && !source.is_ancestor_of(target)
    }

    /// Check if a node can be dropped before a target node.
    pub fn can_drop_before(source: &TreeIndexPath, target: &TreeIndexPath) -> bool {
        source != target
    }

    /// Check if a node can be dropped after a target node.
    pub fn can_drop_after(source: &TreeIndexPath, target: &TreeIndexPath) -> bool {
        source != target
    }

    /// Check if reparenting would create a cycle.
    pub fn prevents_cycle(source: &TreeIndexPath, new_parent: &TreeIndexPath) -> bool {
        // A cycle would occur if the source is an ancestor of the new parent
        source.is_ancestor_of(new_parent) || source == new_parent
    }

    /// Validate a reparent operation.
    pub fn validate_reparent(source: &TreeIndexPath, new_parent: &TreeIndexPath) -> bool {
        !Self::prevents_cycle(source, new_parent) && source != new_parent
    }

    /// Return allowed drop positions for a source onto a target.
    pub fn allowed_drop_positions(source: &TreeIndexPath, target: &TreeIndexPath) -> Vec<&'static str> {
        let mut positions = Vec::new();
        if Self::can_drop_before(source, target) { positions.push("before"); }
        if Self::can_drop_on(source, target) { positions.push("on"); }
        if Self::can_drop_after(source, target) { positions.push("after"); }
        positions
    }
}


/// Configuration manager for tree functionality.
pub struct TreeConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl TreeConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &TreeConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for tree operations.
pub struct TreeRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl TreeRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for tree.
pub struct TreeValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl TreeValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &TreeValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 1
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer1 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer1 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_1(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_1<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_1<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_1(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_1(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 185
// ---------------------------------------------------------------------------

/// Generic object pool `Xc185Pool<T>`.
pub struct Xc185Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc185Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc185PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc185Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc185PoolStats {
        Xc185PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc185Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc185Scheduler`.
pub struct Xc185Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc185Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc185Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_185 hash for the given byte slice.
pub fn xc_185_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_185 convention.
pub fn xc_185_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_121 deepening: state machine + event bus ---

/// States for the Xd121 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd121State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd121State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd121Transition {
    pub from: Xd121State,
    pub to: Xd121State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd121StateMachine {
    current: Xd121State,
    history: Vec<Xd121Transition>,
    step_counter: usize,
}

impl Xd121StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd121State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd121State {
        self.current
    }

    pub fn history(&self) -> &[Xd121Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd121State) -> Result<Xd121State, String> {
        let allowed = match (self.current, target) {
            (Xd121State::Idle, Xd121State::Running) => true,
            (Xd121State::Running, Xd121State::Paused) => true,
            (Xd121State::Running, Xd121State::Done) => true,
            (Xd121State::Paused, Xd121State::Running) => true,
            (Xd121State::Paused, Xd121State::Done) => true,
            (Xd121State::Done, Xd121State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_121: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd121Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd121SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd121State> {
        let prefix = "Xd121SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd121State::Idle),
            "Running" => Some(Xd121State::Running),
            "Paused" => Some(Xd121State::Paused),
            "Done" => Some(Xd121State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd121State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd121 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd121Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd121Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd121HandlerFn = Box<dyn Fn(&Xd121Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd121EventBus {
    handlers: Vec<(usize, Option<String>, Xd121HandlerFn)>,
    next_id: usize,
    published: Vec<Xd121Event>,
}

impl Xd121EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd121Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd121Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd121Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd121Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xg_48: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg48Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg48Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg48Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_48: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg48Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg48Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg48Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg48Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 184).
pub struct Xh184SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh184SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 226 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 184).
pub struct Xh184BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh184BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 184).
pub struct Xi184Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi184Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi184Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi184Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 184).
pub struct Xi184IntervalTree {
    xi_intervals: Vec<Xi184Interval>,
}

impl Xi184IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi184Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi184Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi184Interval) -> Vec<&Xi184Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi184Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi184Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi184Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi184Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi184Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi184Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
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

    #[test]
    fn test_tree_node_is_leaf() {
        let leaf = TreeNode::new("leaf", 0);
        assert!(leaf.is_leaf());
        assert_eq!(leaf.child_count(), 0);
    }

    #[test]
    fn test_tree_node_max_depth() {
        let mut root = TreeNode::new("root", 0);
        let child = root.add_child("child");
        child.add_child("grandchild");
        assert_eq!(root.max_depth(), 2);
    }

    #[test]
    fn test_tree_node_for_each() {
        let mut root = TreeNode::new("a", 0);
        root.add_child("b");
        root.add_child("c");
        let mut count = 0;
        root.for_each(&mut |_| count += 1);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_tree_node_expand_collapse_all() {
        let mut root = TreeNode::new("a", 0);
        root.add_child("b");
        root.expand_all();
        assert!(root.is_expanded);
        assert!(root.children[0].is_expanded);
        root.collapse_all();
        assert!(!root.is_expanded);
    }

    #[test]
    fn test_tree_model_helpers() {
        let mut model = TreeModel { roots: Vec::new() };
        assert!(model.is_empty());
        let mut root = TreeNode::new("root", 0);
        root.add_child("leaf1");
        root.add_child("leaf2");
        model.roots.push(root);
        assert_eq!(model.root_count(), 1);
        assert!(!model.is_empty());
        assert_eq!(model.count_leaves(), 2);
    }

    #[test]
    fn test_tree_model_collect_leaves() {
        let mut model = TreeModel { roots: Vec::new() };
        let mut root = TreeNode::new("root", 0);
        root.add_child("leaf1");
        root.add_child("leaf2");
        model.roots.push(root);
        let leaves = model.collect_leaves();
        assert_eq!(leaves, vec![&"leaf1", &"leaf2"]);
    }

    #[test]
    fn test_tree_model_expand_collapse_all() {
        let mut model = TreeModel { roots: Vec::new() };
        let mut root = TreeNode::new("root", 0);
        root.add_child("child");
        model.roots.push(root);
        model.expand_all();
        assert!(model.roots[0].is_expanded);
        model.collapse_all();
        assert!(!model.roots[0].is_expanded);
    }

    #[test]
    fn test_depth_iter() {
        let mut root = TreeNode::new("root", 0);
        root.add_child("a");
        root.add_child("b");
        let roots = vec![root];
        let depth1: Vec<_> = DepthIter::new(&roots, 1).collect();
        assert_eq!(depth1.len(), 2);
        assert_eq!(depth1[0].data, "a");
    }

    #[test]
    fn expanded_count_none_expanded() {
        let model = sample_model();
        assert_eq!(expanded_count(&model), 0);
    }

    #[test]
    fn expanded_count_after_expand_all() {
        let mut model = sample_model();
        model.expand_all();
        let total_nodes = model.node_count();
        assert_eq!(expanded_count(&model), total_nodes);
    }

    #[test]
    fn nodes_at_depth_zero_returns_roots() {
        let model = sample_model();
        let roots = nodes_at_depth(&model, 0);
        assert_eq!(roots.len(), 2); // "src" and "Cargo.toml"
    }

    #[test]
    fn nodes_at_depth_one() {
        let model = sample_model();
        let depth1 = nodes_at_depth(&model, 1);
        assert_eq!(depth1.len(), 2); // "main.rs" and "lib"
    }

    #[test]
    fn nodes_at_depth_too_deep_returns_empty() {
        let model = sample_model();
        assert!(nodes_at_depth(&model, 100).is_empty());
    }

    #[test]
    fn is_tree_empty_false() {
        let model = sample_model();
        assert!(!is_tree_empty(&model));
    }

    #[test]
    fn is_tree_empty_true() {
        let model: TreeModel<&str> = TreeModel::new();
        assert!(is_tree_empty(&model));
    }

    #[test]
    fn internal_node_count_sample() {
        let model = sample_model();
        let internals = internal_node_count(&model);
        let leaves = leaf_count(&model);
        assert_eq!(internals + leaves, model.node_count());
    }

    #[test]
    fn max_breadth_sample() {
        let model = sample_model();
        let mb = max_breadth(&model);
        assert!(mb >= 1);
    }

    #[test]
    fn root_data_returns_all_roots() {
        let mut model: TreeModel<&str> = TreeModel::new();
        model.add_root("a");
        model.add_root("b");
        let data = root_data(&model);
        assert_eq!(data, vec![&"a", &"b"]);
    }

    #[test]
    fn find_depth_found() {
        let model = sample_model();
        let d = find_depth(&model, |s| *s == "src");
        assert_eq!(d, Some(0));
        let d2 = find_depth(&model, |s| *s == "mod.rs");
        assert_eq!(d2, Some(2));
    }

    #[test]
    fn find_depth_not_found() {
        let model = sample_model();
        let d = find_depth(&model, |s| *s == "nonexistent");
        assert!(d.is_none());
    }

    // ── New tests ──

    #[test]
    fn test_map_tree() {
        let model = sample_model();
        let mapped = map_tree(&model, &|s: &&str| s.to_uppercase());
        let flat = flatten_tree(&mapped);
        let names: Vec<&String> = flat.iter().map(|f| f.data).collect();
        assert_eq!(names, vec!["SRC", "MAIN.RS", "LIB", "MOD.RS", "UTILS.RS", "CARGO.TOML"]);
        assert_eq!(mapped.node_count(), 6);
    }

    #[test]
    fn test_filter_tree_keeps_ancestors() {
        let model = sample_model();
        let filtered = filter_tree(&model, &|s: &&str| s.ends_with(".rs"));
        // "src" is kept as ancestor of main.rs; "lib" as ancestor of mod.rs/utils.rs
        assert_eq!(filtered.node_count(), 5); // src, main.rs, lib, mod.rs, utils.rs
        // Cargo.toml is removed (not .rs, no children matching)
        assert_eq!(filtered.root_count(), 1);
    }

    #[test]
    fn test_ancestors() {
        let model = sample_model();
        // path to utils.rs is [0, 1, 1]
        let anc = ancestors(&model, &[0, 1, 1]);
        assert_eq!(anc, vec![&"src", &"lib"]);
        // path to src is [0]
        let anc_root = ancestors(&model, &[0]);
        assert!(anc_root.is_empty());
    }

    #[test]
    fn test_breadth_first_order() {
        let model = sample_model();
        let bfs = breadth_first(&model);
        // BFS: roots first (src, Cargo.toml), then depth-1 (main.rs, lib), then depth-2 (mod.rs, utils.rs)
        assert_eq!(bfs, vec![&"src", &"Cargo.toml", &"main.rs", &"lib", &"mod.rs", &"utils.rs"]);
    }

    #[test]
    fn test_merge_trees() {
        let m1 = sample_model();
        let mut m2 = TreeModel::new();
        m2.add_root("README.md");
        let merged = merge_trees(m1, m2);
        assert_eq!(merged.root_count(), 3);
        assert_eq!(merged.node_count(), 7);
    }

    #[test]
    fn test_find_node_on_model() {
        let model = sample_model();
        let found = model.find_node(&|s: &&str| *s == "mod.rs");
        assert!(found.is_some());
        assert_eq!(found.unwrap().data, "mod.rs");
        assert_eq!(found.unwrap().depth, 2);
        assert!(model.find_node(&|s: &&str| *s == "nope").is_none());
    }

    #[test]
    fn test_node_height() {
        let model = sample_model();
        // src has height 2 (src -> lib -> mod.rs)
        assert_eq!(model.roots[0].height(), 2);
        // Cargo.toml is a leaf, height 0
        assert_eq!(model.roots[1].height(), 0);
        assert_eq!(model.tree_height(), 2);
    }

    #[test]
    fn test_descendant_count() {
        let model = sample_model();
        // src has 4 descendants: main.rs, lib, mod.rs, utils.rs
        assert_eq!(model.roots[0].descendant_count(), 4);
        assert_eq!(model.roots[1].descendant_count(), 0);
    }

    #[test]
    fn test_node_at_path_immutable() {
        let model = sample_model();
        let node = model.node_at_path(&[0, 1, 0]);
        assert!(node.is_some());
        assert_eq!(node.unwrap().data, "mod.rs");
        assert!(model.node_at_path(&[5]).is_none());
    }

    #[test]
    fn test_contains() {
        let model = sample_model();
        assert!(model.contains(&|s: &&str| *s == "lib"));
        assert!(!model.contains(&|s: &&str| *s == "missing"));
    }

    #[test]
    fn test_expand_path() {
        let mut model = sample_model();
        model.expand_path(&[0, 1]); // expand src and lib
        assert!(model.roots[0].is_expanded);
        assert!(model.roots[0].children[1].is_expanded);
        // main.rs and Cargo.toml untouched
        assert!(!model.roots[0].children[0].is_expanded);
        assert!(!model.roots[1].is_expanded);
    }

    #[test]
    fn test_reveal() {
        let mut model = sample_model();
        // reveal mod.rs at [0, 1, 0]: expands src and lib but not mod.rs
        model.reveal(&[0, 1, 0]);
        assert!(model.roots[0].is_expanded);
        assert!(model.roots[0].children[1].is_expanded);
        assert!(!model.roots[0].children[1].children[0].is_expanded);
    }

    #[test]
    fn test_serialize_tree_box_drawing() {
        let model = sample_model();
        let output = serialize_tree_box(&model, &|d: &&str| d.to_string());
        assert!(output.contains("src\n"));
        assert!(output.contains("├── main.rs\n"));
        assert!(output.contains("└── lib\n"));
        assert!(output.contains("├── mod.rs\n"));
        assert!(output.contains("└── utils.rs\n"));
        assert!(output.contains("Cargo.toml\n"));
    }

    #[test]
    fn test_collect_all_data_node() {
        let model = sample_model();
        let all = model.roots[0].collect_all_data();
        assert_eq!(all, vec![&"src", &"main.rs", &"lib", &"mod.rs", &"utils.rs"]);
    }

    #[test]
    fn test_breadth_first_data_method() {
        let model = sample_model();
        let bfs = model.breadth_first_data();
        assert_eq!(bfs[0], &"src");
        assert_eq!(bfs[1], &"Cargo.toml");
        assert_eq!(bfs.len(), 6);
    }

    // ── TreeFlattenIterator tests ──

    #[test]
    fn flatten_iterator_visits_all() {
        let model = sample_model();
        let iter = TreeFlattenIterator::from_model(&model);
        let data: Vec<&&str> = iter.map(|n| &n.data).collect();
        assert_eq!(data.len(), 6);
        assert_eq!(data[0], &"src");
        assert_eq!(data[1], &"main.rs");
    }

    #[test]
    fn flatten_iterator_single_node() {
        let node = TreeNode::new("root", 0);
        let iter = TreeFlattenIterator::from_node(&node);
        let data: Vec<&&str> = iter.map(|n| &n.data).collect();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0], &"root");
    }

    #[test]
    fn flatten_iterator_depth_first_order() {
        let model = sample_model();
        let iter = TreeFlattenIterator::from_model(&model);
        let data: Vec<&&str> = iter.map(|n| &n.data).collect();
        // src -> main.rs -> lib -> mod.rs -> utils.rs -> Cargo.toml
        assert_eq!(data[2], &"lib");
        assert_eq!(data[3], &"mod.rs");
        assert_eq!(data[5], &"Cargo.toml");
    }

    // ── TreePathSerializer tests ──

    #[test]
    fn path_serialize_roundtrip() {
        let path = vec![0, 1, 2];
        let s = TreePathSerializer::serialize(&path);
        assert_eq!(s, "0/1/2");
        assert_eq!(TreePathSerializer::deserialize(&s), Some(vec![0, 1, 2]));
    }

    #[test]
    fn path_deserialize_empty() {
        assert_eq!(TreePathSerializer::deserialize(""), Some(Vec::new()));
    }

    #[test]
    fn path_deserialize_invalid() {
        assert_eq!(TreePathSerializer::deserialize("a/b"), None);
    }

    #[test]
    fn path_is_descendant() {
        assert!(TreePathSerializer::is_descendant("0", "0/1"));
        assert!(TreePathSerializer::is_descendant("0/1", "0/1/2"));
        assert!(!TreePathSerializer::is_descendant("0/1", "0/1"));
        assert!(!TreePathSerializer::is_descendant("0/1", "0/2"));
    }

    #[test]
    fn path_parent() {
        assert_eq!(TreePathSerializer::parent_path("0/1/2"), Some("0/1".to_string()));
        assert_eq!(TreePathSerializer::parent_path("0"), Some(String::new()));
        assert_eq!(TreePathSerializer::parent_path(""), None);
    }

    #[test]
    fn path_depth() {
        assert_eq!(TreePathSerializer::depth("0/1/2"), 3);
        assert_eq!(TreePathSerializer::depth("0"), 1);
        assert_eq!(TreePathSerializer::depth(""), 0);
    }

    #[test]
    fn path_append() {
        assert_eq!(TreePathSerializer::append("0/1", 2), "0/1/2");
        assert_eq!(TreePathSerializer::append("", 0), "0");
    }

    #[test]
    fn path_leaf_index() {
        assert_eq!(TreePathSerializer::leaf_index("0/1/3"), Some(3));
        assert_eq!(TreePathSerializer::leaf_index("5"), Some(5));
    }

    // ── TreeDepthCalculator tests ──

    #[test]
    fn depth_max() {
        let model = sample_model();
        // src(0) -> lib(1) -> mod.rs(2) = max depth 2
        assert_eq!(TreeDepthCalculator::max_depth(&model), 2);
    }

    #[test]
    fn depth_count_at_depth() {
        let model = sample_model();
        assert_eq!(TreeDepthCalculator::count_at_depth(&model, 0), 2); // src, Cargo.toml
        assert_eq!(TreeDepthCalculator::count_at_depth(&model, 1), 2); // main.rs, lib
        assert_eq!(TreeDepthCalculator::count_at_depth(&model, 2), 2); // mod.rs, utils.rs
    }

    #[test]
    fn depth_average_leaf() {
        let model = sample_model();
        // leaves: main.rs(1), mod.rs(2), utils.rs(2), Cargo.toml(0) => avg = 5/4 = 1.25
        let avg = TreeDepthCalculator::average_leaf_depth(&model);
        assert!((avg - 1.25).abs() < 0.001);
    }

    #[test]
    fn depth_leaf_data() {
        let model = sample_model();
        let leaves = TreeDepthCalculator::leaf_data(&model);
        assert_eq!(leaves.len(), 4);
        assert!(leaves.contains(&&"main.rs"));
        assert!(leaves.contains(&&"Cargo.toml"));
    }

    #[test]
    fn depth_empty_model() {
        let model: TreeModel<&str> = TreeModel::new();
        assert_eq!(TreeDepthCalculator::max_depth(&model), 0);
        assert_eq!(TreeDepthCalculator::average_leaf_depth(&model), 0.0);
    }

    // ── TreeSiblingNavigator tests ──

    #[test]
    fn sibling_root_count() {
        let model = sample_model();
        assert_eq!(TreeSiblingNavigator::root_sibling_count(&model), 2);
    }

    #[test]
    fn sibling_find_root() {
        let model = sample_model();
        assert_eq!(TreeSiblingNavigator::find_root_index(&model, &"src"), Some(0));
        assert_eq!(TreeSiblingNavigator::find_root_index(&model, &"Cargo.toml"), Some(1));
        assert_eq!(TreeSiblingNavigator::find_root_index(&model, &"missing"), None);
    }

    #[test]
    fn sibling_next_prev_root() {
        let model = sample_model();
        assert_eq!(TreeSiblingNavigator::next_root_sibling(&model, &"src"), Some(&"Cargo.toml"));
        assert_eq!(TreeSiblingNavigator::next_root_sibling(&model, &"Cargo.toml"), None);
        assert_eq!(TreeSiblingNavigator::prev_root_sibling(&model, &"Cargo.toml"), Some(&"src"));
        assert_eq!(TreeSiblingNavigator::prev_root_sibling(&model, &"src"), None);
    }

    #[test]
    fn sibling_child_count() {
        let model = sample_model();
        assert_eq!(TreeSiblingNavigator::child_count_of_root(&model, 0), 2); // src has main.rs + lib
        assert_eq!(TreeSiblingNavigator::child_count_of_root(&model, 1), 0); // Cargo.toml has none
        assert_eq!(TreeSiblingNavigator::child_count_of_root(&model, 99), 0);
    }

    #[test]
    fn sibling_first_last_child() {
        let model = sample_model();
        let src = &model.roots[0];
        assert!(TreeSiblingNavigator::is_first_child(src, &"main.rs"));
        assert!(TreeSiblingNavigator::is_last_child(src, &"lib"));
        assert!(!TreeSiblingNavigator::is_first_child(src, &"lib"));
    }

    #[test]
    fn sibling_next_prev() {
        let model = sample_model();
        let src = &model.roots[0];
        assert_eq!(TreeSiblingNavigator::next_sibling(src, &"main.rs"), Some(&"lib"));
        assert_eq!(TreeSiblingNavigator::next_sibling(src, &"lib"), None);
        assert_eq!(TreeSiblingNavigator::prev_sibling(src, &"lib"), Some(&"main.rs"));
        assert_eq!(TreeSiblingNavigator::prev_sibling(src, &"main.rs"), None);
    }

    // -- TreeIndexPath -------------------------------------------------------

    #[test]
    fn path_depth_v2() {
        let p = TreeIndexPath::new(vec![0, 1, 2]);
        assert_eq!(p.depth(), 3);
        assert_eq!(TreeIndexPath::root().depth(), 0);
    }

    #[test]
    fn path_parent_v2() {
        let p = TreeIndexPath::new(vec![0, 1, 2]);
        let parent = p.parent_path().unwrap();
        assert_eq!(parent.segments(), &[0, 1]);
        assert!(TreeIndexPath::root().parent_path().is_none());
    }

    #[test]
    fn path_is_ancestor() {
        let ancestor = TreeIndexPath::new(vec![0]);
        let descendant = TreeIndexPath::new(vec![0, 1, 2]);
        assert!(ancestor.is_ancestor_of(&descendant));
        assert!(!descendant.is_ancestor_of(&ancestor));
        assert!(!ancestor.is_ancestor_of(&ancestor));
    }

    #[test]
    fn path_common_ancestor() {
        let a = TreeIndexPath::new(vec![0, 1, 3]);
        let b = TreeIndexPath::new(vec![0, 1, 5]);
        let common = a.common_ancestor_with(&b);
        assert_eq!(common.segments(), &[0, 1]);
    }

    #[test]
    fn path_child() {
        let p = TreeIndexPath::new(vec![0]);
        let c = p.child(3);
        assert_eq!(c.segments(), &[0, 3]);
    }

    #[test]
    fn path_to_string() {
        let p = TreeIndexPath::new(vec![0, 1, 2]);
        assert_eq!(p.to_string_repr(), "0/1/2");
    }

    // -- TreeExpansionState -------------------------------------------------

    #[test]
    fn expansion_expand_collapse() {
        let mut es = TreeExpansionState::new();
        let p = TreeIndexPath::new(vec![0]);
        es.expand(&p);
        assert!(es.is_expanded(&p));
        es.collapse(&p);
        assert!(!es.is_expanded(&p));
    }

    #[test]
    fn expansion_toggle() {
        let mut es = TreeExpansionState::new();
        let p = TreeIndexPath::new(vec![0]);
        es.toggle(&p);
        assert!(es.is_expanded(&p));
        es.toggle(&p);
        assert!(!es.is_expanded(&p));
    }

    #[test]
    fn expansion_collapse_all() {
        let mut es = TreeExpansionState::new();
        es.expand(&TreeIndexPath::new(vec![0]));
        es.expand(&TreeIndexPath::new(vec![1]));
        assert_eq!(es.expanded_count(), 2);
        es.collapse_all();
        assert_eq!(es.expanded_count(), 0);
    }

    #[test]
    fn expansion_snapshot_restore() {
        let mut es = TreeExpansionState::new();
        es.expand(&TreeIndexPath::new(vec![0]));
        let snap = es.expansion_snapshot();
        es.collapse_all();
        assert_eq!(es.expanded_count(), 0);
        es.restore(snap);
        assert_eq!(es.expanded_count(), 1);
    }

    // -- TreeDragDropValidator ----------------------------------------------

    #[test]
    fn dragdrop_can_drop_on() {
        let src = TreeIndexPath::new(vec![0]);
        let tgt = TreeIndexPath::new(vec![1]);
        assert!(TreeDragDropValidator::can_drop_on(&src, &tgt));
        assert!(!TreeDragDropValidator::can_drop_on(&src, &src));
    }

    #[test]
    fn dragdrop_prevents_cycle() {
        let parent = TreeIndexPath::new(vec![0]);
        let child = TreeIndexPath::new(vec![0, 1]);
        assert!(TreeDragDropValidator::prevents_cycle(&parent, &child));
        assert!(!TreeDragDropValidator::prevents_cycle(&child, &parent));
    }

    #[test]
    fn dragdrop_validate_reparent() {
        let src = TreeIndexPath::new(vec![0, 1]);
        let new_parent = TreeIndexPath::new(vec![2]);
        assert!(TreeDragDropValidator::validate_reparent(&src, &new_parent));
    }

    #[test]
    fn dragdrop_allowed_positions() {
        let src = TreeIndexPath::new(vec![0]);
        let tgt = TreeIndexPath::new(vec![1]);
        let pos = TreeDragDropValidator::allowed_drop_positions(&src, &tgt);
        assert!(pos.contains(&"before"));
        assert!(pos.contains(&"on"));
        assert!(pos.contains(&"after"));
    }


    #[test]
    fn tree_config_new() {
        let cfg = TreeConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn tree_config_set_get() {
        let mut cfg = TreeConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn tree_config_remove() {
        let mut cfg = TreeConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn tree_config_keys_sorted() {
        let mut cfg = TreeConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn tree_config_bump_version() {
        let mut cfg = TreeConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn tree_config_clear() {
        let mut cfg = TreeConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn tree_config_merge() {
        let mut cfg1 = TreeConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = TreeConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn tree_config_disable() {
        let mut cfg = TreeConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn tree_rate_tracker_empty() {
        let rt = TreeRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn tree_rate_tracker_record() {
        let mut rt = TreeRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn tree_rate_tracker_prune() {
        let mut rt = TreeRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn tree_validator_valid() {
        let v = TreeValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn tree_validator_errors() {
        let mut v = TreeValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn tree_validator_clear() {
        let mut v = TreeValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn tree_validator_merge() {
        let mut v1 = TreeValidator::new();
        v1.add_error("e1");
        let mut v2 = TreeValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn tree_rate_tracker_clear() {
        let mut rt = TreeRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    #[test]
    fn xb_ring_buffer_1_push_and_len() {
        let mut rb = super::XbRingBuffer1::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_1_overwrite() {
        let mut rb = super::XbRingBuffer1::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_1_get_out_of_bounds() {
        let rb = super::XbRingBuffer1::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_1_drain_all() {
        let mut rb = super::XbRingBuffer1::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_1_peek_front_back() {
        let mut rb = super::XbRingBuffer1::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_1_clear() {
        let mut rb = super::XbRingBuffer1::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_1_capacity() {
        let rb = super::XbRingBuffer1::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_1_basic() {
        let h = super::xb_fnv1a_1(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_1(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_1_different_inputs() {
        let h1 = super::xb_fnv1a_1(b"abc");
        let h2 = super::xb_fnv1a_1(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_1_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_1(&data);
        let dec = super::xb_rle_decode_1(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_1_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_1(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_1(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_1_values() {
        assert!((super::xb_clamp_1(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_1(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_1(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_1_values() {
        assert!((super::xb_lerp_1(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_1(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_1(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_1_wrap_around_twice() {
        let mut rb = super::XbRingBuffer1::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 185 ----

    #[test]
    fn xc_185_pool_new_empty() {
        let pool: super::Xc185Pool<i32> = super::Xc185Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_185_pool_release_acquire() {
        let mut pool = super::Xc185Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_185_pool_acquire_empty() {
        let mut pool: super::Xc185Pool<i32> = super::Xc185Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_185_pool_full() {
        let mut pool = super::Xc185Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_185_pool_drain() {
        let mut pool = super::Xc185Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_185_pool_stats() {
        let mut pool = super::Xc185Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_185_pool_clear() {
        let mut pool = super::Xc185Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_185_pool_shrink() {
        let mut pool = super::Xc185Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_185_pool_default() {
        let pool: super::Xc185Pool<String> = super::Xc185Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_185_pool_extend() {
        let mut pool = super::Xc185Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_185_pool_retain() {
        let mut pool = super::Xc185Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_185_scheduler_round_robin() {
        let mut sched = super::Xc185Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_185_scheduler_empty() {
        let mut sched = super::Xc185Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_185_scheduler_reset() {
        let mut sched = super::Xc185Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_185_scheduler_add_remove() {
        let mut sched = super::Xc185Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_185_scheduler_targets() {
        let sched = super::Xc185Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_185_hash_empty() {
        assert_eq!(super::xc_185_hash(b""), 5381);
    }

    #[test]
    fn xc_185_hash_data() {
        let h = super::xc_185_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_185_hash(b"hello"), h);
    }

    #[test]
    fn xc_185_reverse_str() {
        assert_eq!(super::xc_185_reverse("abc"), "cba");
        assert_eq!(super::xc_185_reverse(""), "");
    }


    // --- xd_121 deepening tests ---

    #[test]
    fn xd_121_sm_initial_state() {
        let sm = Xd121StateMachine::new();
        assert_eq!(sm.current_state(), Xd121State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_121_sm_valid_idle_to_running() {
        let mut sm = Xd121StateMachine::new();
        assert!(sm.transition(Xd121State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd121State::Running);
    }

    #[test]
    fn xd_121_sm_valid_running_to_paused() {
        let mut sm = Xd121StateMachine::new();
        sm.transition(Xd121State::Running).unwrap();
        assert!(sm.transition(Xd121State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd121State::Paused);
    }

    #[test]
    fn xd_121_sm_valid_running_to_done() {
        let mut sm = Xd121StateMachine::new();
        sm.transition(Xd121State::Running).unwrap();
        assert!(sm.transition(Xd121State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd121State::Done);
    }

    #[test]
    fn xd_121_sm_valid_paused_to_running() {
        let mut sm = Xd121StateMachine::new();
        sm.transition(Xd121State::Running).unwrap();
        sm.transition(Xd121State::Paused).unwrap();
        assert!(sm.transition(Xd121State::Running).is_ok());
    }

    #[test]
    fn xd_121_sm_valid_done_to_idle() {
        let mut sm = Xd121StateMachine::new();
        sm.transition(Xd121State::Running).unwrap();
        sm.transition(Xd121State::Done).unwrap();
        assert!(sm.transition(Xd121State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd121State::Idle);
    }

    #[test]
    fn xd_121_sm_invalid_idle_to_done() {
        let mut sm = Xd121StateMachine::new();
        assert!(sm.transition(Xd121State::Done).is_err());
    }

    #[test]
    fn xd_121_sm_invalid_idle_to_paused() {
        let mut sm = Xd121StateMachine::new();
        assert!(sm.transition(Xd121State::Paused).is_err());
    }

    #[test]
    fn xd_121_sm_history_tracking() {
        let mut sm = Xd121StateMachine::new();
        sm.transition(Xd121State::Running).unwrap();
        sm.transition(Xd121State::Paused).unwrap();
        sm.transition(Xd121State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd121State::Idle);
        assert_eq!(sm.history()[0].to, Xd121State::Running);
        assert_eq!(sm.history()[1].from, Xd121State::Running);
        assert_eq!(sm.history()[2].to, Xd121State::Done);
    }

    #[test]
    fn xd_121_sm_serialize_deserialize() {
        let mut sm = Xd121StateMachine::new();
        sm.transition(Xd121State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd121StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd121State::Running));
    }

    #[test]
    fn xd_121_sm_deserialize_invalid() {
        assert_eq!(Xd121StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_121_sm_reset() {
        let mut sm = Xd121StateMachine::new();
        sm.transition(Xd121State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd121State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_121_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd121EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd121Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_121_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd121EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd121Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd121Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_121_bus_unsubscribe() {
        let mut bus = Xd121EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_121_event_kind_and_payload() {
        let e = Xd121Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd121Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_121_bus_clear_history() {
        let mut bus = Xd121EventBus::new();
        bus.publish(Xd121Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_121_sm_step_counter_increments() {
        let mut sm = Xd121StateMachine::new();
        sm.transition(Xd121State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd121State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_48 graph tests ------------------------------------------------

    #[test]
    fn xg_48_graph_empty() {
        let g = super::Xg48Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_48_graph_add_node() {
        let mut g = super::Xg48Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_48_graph_add_edge() {
        let mut g = super::Xg48Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_48_graph_neighbors() {
        let mut g = super::Xg48Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_48_graph_has_path() {
        let mut g = super::Xg48Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_48_graph_self_path() {
        let g = super::Xg48Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_48_graph_topo_sort() {
        let mut g = super::Xg48Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_48_graph_cycle_detect_false() {
        let mut g = super::Xg48Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_48_graph_cycle_detect_true() {
        let mut g = super::Xg48Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_48 heap tests -------------------------------------------------

    #[test]
    fn xg_48_heap_empty() {
        let h: super::Xg48Heap<i32> = super::Xg48Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_48_heap_push_pop() {
        let mut h = super::Xg48Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_48_heap_peek() {
        let mut h = super::Xg48Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_48_heap_drain_sorted() {
        let mut h = super::Xg48Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_48_heap_merge() {
        let mut a = super::Xg48Heap::new();
        let mut b = super::Xg48Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_48_heap_default() {
        let h: super::Xg48Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_48_graph_default() {
        let g: super::Xg48Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh184_skip_insert_contains() {
        let mut sl = super::Xh184SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh184_skip_remove() {
        let mut sl = super::Xh184SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh184_skip_len() {
        let mut sl = super::Xh184SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh184_skip_range_query() {
        let mut sl = super::Xh184SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh184_skip_floor_ceiling() {
        let mut sl = super::Xh184SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh184_skip_rank() {
        let mut sl = super::Xh184SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh184_skip_empty() {
        let sl = super::Xh184SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh184_skip_duplicates() {
        let mut sl = super::Xh184SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh184_bitset_set_test() {
        let mut bs = super::Xh184BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh184_bitset_clear_count() {
        let mut bs = super::Xh184BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh184_bitset_and_or_xor() {
        let mut a = super::Xh184BitSet::xh_new(128);
        let mut b = super::Xh184BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh184_bitset_iter_ones() {
        let mut bs = super::Xh184BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh184_bitset_first_last() {
        let mut bs = super::Xh184BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh184_bitset_empty() {
        let bs = super::Xh184BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi184_deque_push_pop_back() {
        let mut dq = super::Xi184Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi184_deque_push_pop_front() {
        let mut dq = super::Xi184Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi184_deque_mixed_ops() {
        let mut dq = super::Xi184Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi184_deque_get_and_split() {
        let mut dq = super::Xi184Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi184_deque_rotate_left() {
        let mut dq = super::Xi184Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi184_deque_rotate_right() {
        let mut dq = super::Xi184Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi184_deque_grow() {
        let mut dq = super::Xi184Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi184_deque_empty() {
        let dq = super::Xi184Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi184_interval_tree_insert_query() {
        let mut tree = super::Xi184IntervalTree::xi_new();
        tree.xi_insert(super::Xi184Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi184Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi184Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi184_interval_tree_overlap() {
        let mut tree = super::Xi184IntervalTree::xi_new();
        tree.xi_insert(super::Xi184Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi184Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi184Interval::xi_new(12, 20));
        let q = super::Xi184Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi184_interval_tree_remove() {
        let mut tree = super::Xi184IntervalTree::xi_new();
        tree.xi_insert(super::Xi184Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi184Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi184_interval_tree_gaps() {
        let mut tree = super::Xi184IntervalTree::xi_new();
        tree.xi_insert(super::Xi184Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi184Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi184Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi184Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi184Interval::xi_new(8, 10));
    }

    #[test]
    fn xi184_interval_tree_merge() {
        let mut tree = super::Xi184IntervalTree::xi_new();
        tree.xi_insert(super::Xi184Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi184Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi184Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi184Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi184Interval::xi_new(10, 15));
    }

    #[test]
    fn xi184_interval_tree_all() {
        let mut tree = super::Xi184IntervalTree::xi_new();
        tree.xi_insert(super::Xi184Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi184Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi184_interval_tree_empty() {
        let tree = super::Xi184IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi184_interval_tree_contains_point() {
        let iv = super::Xi184Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}
