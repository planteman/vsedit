//! List / tree widget for sidebar panels.
//!
//! Provides a generic tree-backed list view with keyboard navigation,
//! expand/collapse, and multi-select support.

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single item in the list tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub children: Vec<ListItem>,
    pub expanded: bool,
    pub selected: bool,
}

impl ListItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
            icon: None,
            children: Vec::new(),
            expanded: false,
            selected: false,
        }
    }
}

/// Options controlling list-view behaviour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListOptions {
    pub multi_select: bool,
    pub keyboard_navigation: bool,
    pub smooth_scrolling: bool,
}

impl Default for ListOptions {
    fn default() -> Self {
        Self {
            multi_select: false,
            keyboard_navigation: true,
            smooth_scrolling: false,
        }
    }
}

/// The list-view widget state.
#[derive(Debug, Clone)]
pub struct ListView {
    pub items: Vec<ListItem>,
    pub options: ListOptions,
    pub focused_index: Option<usize>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Flatten the tree into a depth-first ordered sequence of references,
/// respecting the `expanded` flag on parent nodes.
pub fn flatten(items: &[ListItem]) -> Vec<&ListItem> {
    let mut out = Vec::new();
    for item in items {
        out.push(item);
        if item.expanded {
            out.extend(flatten(&item.children));
        }
    }
    out
}

fn find_item_mut<'a>(items: &'a mut [ListItem], id: &str) -> Option<&'a mut ListItem> {
    for item in items.iter_mut() {
        if item.id == id {
            return Some(item);
        }
        if let Some(found) = find_item_mut(&mut item.children, id) {
            return Some(found);
        }
    }
    None
}

fn deselect_all_recursive(items: &mut [ListItem]) {
    for item in items.iter_mut() {
        item.selected = false;
        deselect_all_recursive(&mut item.children);
    }
}

fn collect_selected<'a>(items: &'a [ListItem], out: &mut Vec<&'a ListItem>) {
    for item in items {
        if item.selected {
            out.push(item);
        }
        collect_selected(&item.children, out);
    }
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl ListView {
    pub fn new(options: ListOptions) -> Self {
        Self {
            items: Vec::new(),
            options,
            focused_index: None,
        }
    }

    pub fn add_item(&mut self, item: ListItem) {
        self.items.push(item);
    }

    pub fn remove_item(&mut self, id: &str) -> bool {
        fn remove_recursive(items: &mut Vec<ListItem>, id: &str) -> bool {
            if let Some(pos) = items.iter().position(|i| i.id == id) {
                items.remove(pos);
                return true;
            }
            items.iter_mut().any(|i| remove_recursive(&mut i.children, id))
        }
        let removed = remove_recursive(&mut self.items, id);
        if removed {
            // Reset focus if the flat length shrinks below the index.
            let flat_len = flatten(&self.items).len();
            if let Some(idx) = self.focused_index {
                if idx >= flat_len {
                    self.focused_index = if flat_len == 0 { None } else { Some(flat_len - 1) };
                }
            }
        }
        removed
    }

    pub fn toggle_expand(&mut self, id: &str) {
        if let Some(item) = find_item_mut(&mut self.items, id) {
            item.expanded = !item.expanded;
        }
    }

    pub fn select(&mut self, id: &str) {
        if !self.options.multi_select {
            deselect_all_recursive(&mut self.items);
        }
        if let Some(item) = find_item_mut(&mut self.items, id) {
            item.selected = true;
        }
    }

    pub fn deselect_all(&mut self) {
        deselect_all_recursive(&mut self.items);
    }

    pub fn get_selected(&self) -> Vec<&ListItem> {
        let mut out = Vec::new();
        collect_selected(&self.items, &mut out);
        out
    }

    pub fn focus_next(&mut self) {
        let flat_len = flatten(&self.items).len();
        if flat_len == 0 {
            return;
        }
        self.focused_index = Some(match self.focused_index {
            Some(i) if i + 1 < flat_len => i + 1,
            Some(_) => 0,
            None => 0,
        });
    }

    pub fn focus_prev(&mut self) {
        let flat_len = flatten(&self.items).len();
        if flat_len == 0 {
            return;
        }
        self.focused_index = Some(match self.focused_index {
            Some(0) | None => flat_len - 1,
            Some(i) => i - 1,
        });
    }

    pub fn item_count(&self) -> usize {
        fn count(items: &[ListItem]) -> usize {
            items.iter().map(|i| 1 + count(&i.children)).sum()
        }
        count(&self.items)
    }
}

impl Default for ListView {
    fn default() -> Self {
        Self::new(ListOptions::default())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tree() -> ListView {
        let mut lv = ListView::new(ListOptions::default());
        let mut parent = ListItem::new("p1", "Parent");
        parent.children.push(ListItem::new("c1", "Child 1"));
        parent.children.push(ListItem::new("c2", "Child 2"));
        lv.add_item(parent);
        lv.add_item(ListItem::new("p2", "Sibling"));
        lv
    }

    #[test]
    fn item_count_includes_children() {
        let lv = sample_tree();
        assert_eq!(lv.item_count(), 4);
    }

    #[test]
    fn flatten_respects_expanded() {
        let mut lv = sample_tree();
        // collapsed – only top-level visible
        assert_eq!(flatten(&lv.items).len(), 2);
        // expand parent
        lv.toggle_expand("p1");
        assert_eq!(flatten(&lv.items).len(), 4);
    }

    #[test]
    fn select_and_deselect() {
        let mut lv = sample_tree();
        lv.select("p2");
        assert_eq!(lv.get_selected().len(), 1);
        lv.deselect_all();
        assert!(lv.get_selected().is_empty());
    }

    #[test]
    fn focus_wraps_around() {
        let mut lv = ListView::new(ListOptions::default());
        lv.add_item(ListItem::new("a", "A"));
        lv.add_item(ListItem::new("b", "B"));
        lv.focus_next(); // 0
        lv.focus_next(); // 1
        lv.focus_next(); // wraps to 0
        assert_eq!(lv.focused_index, Some(0));
    }
}
