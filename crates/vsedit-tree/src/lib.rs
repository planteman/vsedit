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
}
