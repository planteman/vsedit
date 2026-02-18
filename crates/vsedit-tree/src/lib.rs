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

}
