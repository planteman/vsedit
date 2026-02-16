//! List / tree widget for sidebar panels.
//!
//! Provides a generic tree-backed list view with keyboard navigation,
//! expand/collapse, and multi-select support.

use std::fmt;
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

    /// Builder: set the description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Builder: set the icon.
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Builder: add a child item.
    pub fn with_child(mut self, child: ListItem) -> Self {
        self.children.push(child);
        self
    }

    /// Builder: set the expanded state.
    pub fn with_expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Returns the depth of the deepest nested child (0 if leaf).
    pub fn depth(&self) -> usize {
        if self.children.is_empty() {
            0
        } else {
            1 + self.children.iter().map(|c| c.depth()).max().unwrap_or(0)
        }
    }

    /// Returns total number of descendants (not counting self).
    pub fn descendant_count(&self) -> usize {
        self.children
            .iter()
            .map(|c| 1 + c.descendant_count())
            .sum()
    }

    /// Returns true if this item is a leaf (has no children).
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Find a descendant by id (depth-first).
    pub fn find_descendant(&self, id: &str) -> Option<&ListItem> {
        for child in &self.children {
            if child.id == id {
                return Some(child);
            }
            if let Some(found) = child.find_descendant(id) {
                return Some(found);
            }
        }
        None
    }
}

impl std::fmt::Display for ListItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref desc) = self.description {
            write!(f, "{} ({})", self.label, desc)
        } else {
            write!(f, "{}", self.label)
        }
    }
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during list-view operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListError {
    /// The requested item id was not found.
    ItemNotFound(String),
    /// An item with this id already exists.
    DuplicateId(String),
    /// A validation constraint was violated.
    ValidationError(String),
}

impl std::fmt::Display for ListError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ItemNotFound(id) => write!(f, "item not found: {id}"),
            Self::DuplicateId(id) => write!(f, "duplicate item id: {id}"),
            Self::ValidationError(msg) => write!(f, "validation error: {msg}"),
        }
    }
}

impl std::error::Error for ListError {}

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
#[derive(Debug, Clone, PartialEq)]
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

    /// Try to select an item, returning an error if the id is not found.
    pub fn try_select(&mut self, id: &str) -> Result<(), ListError> {
        if find_item_mut(&mut self.items, id).is_none() {
            return Err(ListError::ItemNotFound(id.to_string()));
        }
        self.select(id);
        Ok(())
    }

    /// Add an item, rejecting duplicates.
    pub fn try_add_item(&mut self, item: ListItem) -> Result<(), ListError> {
        if self.find_item(&item.id).is_some() {
            return Err(ListError::DuplicateId(item.id.clone()));
        }
        if item.id.is_empty() {
            return Err(ListError::ValidationError("id must not be empty".into()));
        }
        self.items.push(item);
        Ok(())
    }

    /// Find an item by id (immutable).
    pub fn find_item(&self, id: &str) -> Option<&ListItem> {
        fn find_recursive<'a>(items: &'a [ListItem], id: &str) -> Option<&'a ListItem> {
            for item in items {
                if item.id == id {
                    return Some(item);
                }
                if let Some(found) = find_recursive(&item.children, id) {
                    return Some(found);
                }
            }
            None
        }
        find_recursive(&self.items, id)
    }

    /// Returns the currently focused item, if any.
    pub fn focused_item(&self) -> Option<&ListItem> {
        let flat = flatten(&self.items);
        self.focused_index.and_then(|i| flat.get(i).copied())
    }

    /// Returns the maximum nesting depth across all items.
    pub fn max_depth(&self) -> usize {
        self.items.iter().map(|i| i.depth()).max().unwrap_or(0)
    }

    /// Returns a count of only the visible (flattened) items.
    pub fn visible_count(&self) -> usize {
        flatten(&self.items).len()
    }

    /// Expand all items recursively.
    pub fn expand_all(&mut self) {
        fn expand_recursive(items: &mut [ListItem]) {
            for item in items.iter_mut() {
                if !item.children.is_empty() {
                    item.expanded = true;
                }
                expand_recursive(&mut item.children);
            }
        }
        expand_recursive(&mut self.items);
    }

    /// Collapse all items recursively.
    pub fn collapse_all(&mut self) {
        fn collapse_recursive(items: &mut [ListItem]) {
            for item in items.iter_mut() {
                item.expanded = false;
                collapse_recursive(&mut item.children);
            }
        }
        collapse_recursive(&mut self.items);
    }

    /// Select the currently focused item (if any).
    pub fn select_focused(&mut self) {
        if let Some(item) = self.focused_item() {
            let id = item.id.clone();
            self.select(&id);
        }
    }

    /// Returns ids of all selected items.
    pub fn selected_ids(&self) -> Vec<&str> {
        self.get_selected().iter().map(|i| i.id.as_str()).collect()
    }
}

impl Default for ListView {
    fn default() -> Self {
        Self::new(ListOptions::default())
    }
}

impl std::fmt::Display for ListView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total = self.item_count();
        let visible = self.visible_count();
        let selected = self.get_selected().len();
        write!(f, "ListView({total} items, {visible} visible, {selected} selected)")
    }
}

impl std::fmt::Display for ListOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ListOptions(multi_select={}, keyboard_nav={}, smooth_scroll={})",
            self.multi_select, self.keyboard_navigation, self.smooth_scrolling
        )
    }
}

// ---------------------------------------------------------------------------
// Virtual list
// ---------------------------------------------------------------------------

/// A virtual list that only renders a viewport window over a large collection.
#[derive(Debug, Clone)]
pub struct VirtualList<T> {
    pub items: Vec<T>,
    pub viewport_start: usize,
    pub viewport_size: usize,
}

impl<T> VirtualList<T> {
    pub fn new(items: Vec<T>, viewport_size: usize) -> Self {
        Self {
            items,
            viewport_start: 0,
            viewport_size,
        }
    }

    pub fn total_count(&self) -> usize {
        self.items.len()
    }

    pub fn visible_items(&self) -> &[T] {
        let end = (self.viewport_start + self.viewport_size).min(self.items.len());
        &self.items[self.viewport_start..end]
    }

    pub fn scroll_to(&mut self, index: usize) {
        let max_start = self.items.len().saturating_sub(self.viewport_size);
        self.viewport_start = index.min(max_start);
    }

    pub fn scroll_down(&mut self, count: usize) {
        let max_start = self.items.len().saturating_sub(self.viewport_size);
        self.viewport_start = (self.viewport_start + count).min(max_start);
    }

    pub fn scroll_up(&mut self, count: usize) {
        self.viewport_start = self.viewport_start.saturating_sub(count);
    }

    pub fn is_visible(&self, index: usize) -> bool {
        let end = (self.viewport_start + self.viewport_size).min(self.items.len());
        index >= self.viewport_start && index < end
    }

    pub fn ensure_visible(&mut self, index: usize) {
        if index < self.viewport_start {
            self.viewport_start = index;
        } else if index >= self.viewport_start + self.viewport_size {
            self.viewport_start = index + 1 - self.viewport_size;
        }
    }
}

// ---------------------------------------------------------------------------
// Keyboard navigation
// ---------------------------------------------------------------------------

/// Keyboard navigation state for a list.
#[derive(Debug, Clone)]
pub struct ListKeyNav {
    focused: usize,
    total: usize,
}

impl ListKeyNav {
    pub fn new(total: usize) -> Self {
        Self { focused: 0, total }
    }

    pub fn move_up(&mut self) {
        if self.total > 0 && self.focused > 0 {
            self.focused -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.total > 0 && self.focused + 1 < self.total {
            self.focused += 1;
        }
    }

    pub fn home(&mut self) {
        self.focused = 0;
    }

    pub fn end(&mut self) {
        if self.total > 0 {
            self.focused = self.total - 1;
        }
    }

    pub fn page_up(&mut self, page_size: usize) {
        self.focused = self.focused.saturating_sub(page_size);
    }

    pub fn page_down(&mut self, page_size: usize) {
        if self.total > 0 {
            self.focused = (self.focused + page_size).min(self.total - 1);
        }
    }

    pub fn focused(&self) -> usize {
        self.focused
    }

    pub fn set_total(&mut self, total: usize) {
        self.total = total;
        if total == 0 {
            self.focused = 0;
        } else if self.focused >= total {
            self.focused = total - 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Multi-select
// ---------------------------------------------------------------------------

/// Selection state supporting click, ctrl-click, and shift-click patterns.
#[derive(Debug, Clone)]
pub struct ListMultiSelect {
    selected: std::collections::BTreeSet<usize>,
    anchor: Option<usize>,
    total: usize,
}

impl ListMultiSelect {
    pub fn new(total: usize) -> Self {
        Self {
            selected: std::collections::BTreeSet::new(),
            anchor: None,
            total,
        }
    }

    pub fn click(&mut self, index: usize) {
        self.selected.clear();
        if index < self.total {
            self.selected.insert(index);
            self.anchor = Some(index);
        }
    }

    pub fn ctrl_click(&mut self, index: usize) {
        if index >= self.total {
            return;
        }
        if self.selected.contains(&index) {
            self.selected.remove(&index);
        } else {
            self.selected.insert(index);
        }
        self.anchor = Some(index);
    }

    pub fn shift_click(&mut self, index: usize) {
        if index >= self.total {
            return;
        }
        let anchor = self.anchor.unwrap_or(0);
        let (lo, hi) = if anchor <= index {
            (anchor, index)
        } else {
            (index, anchor)
        };
        self.selected.clear();
        for i in lo..=hi {
            self.selected.insert(i);
        }
    }

    pub fn select_all(&mut self) {
        for i in 0..self.total {
            self.selected.insert(i);
        }
    }

    pub fn deselect_all(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }

    pub fn is_selected(&self, index: usize) -> bool {
        self.selected.contains(&index)
    }

    pub fn selected_count(&self) -> usize {
        self.selected.len()
    }

    pub fn selected_indices(&self) -> Vec<usize> {
        self.selected.iter().copied().collect()
    }

    pub fn set_total(&mut self, total: usize) {
        self.total = total;
        self.selected.retain(|&i| i < total);
        if let Some(a) = self.anchor {
            if a >= total {
                self.anchor = None;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for list operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ListStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ListStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &ListStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for ListStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ListStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ListStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for list.
#[derive(Debug, Clone)]
pub struct ListValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ListValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for ListValidator {
    fn default() -> Self {
        Self::new()
    }
}

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

    #[test]
    fn focus_prev_wraps_around() {
        let mut lv = ListView::new(ListOptions::default());
        lv.add_item(ListItem::new("a", "A"));
        lv.add_item(ListItem::new("b", "B"));
        lv.focus_prev(); // wraps to 1
        assert_eq!(lv.focused_index, Some(1));
        lv.focus_prev(); // 0
        assert_eq!(lv.focused_index, Some(0));
    }

    #[test]
    fn builder_pattern() {
        let item = ListItem::new("x", "X")
            .with_description("desc")
            .with_icon("file")
            .with_expanded(true)
            .with_child(ListItem::new("y", "Y"));
        assert_eq!(item.description.as_deref(), Some("desc"));
        assert_eq!(item.icon.as_deref(), Some("file"));
        assert!(item.expanded);
        assert_eq!(item.children.len(), 1);
    }

    #[test]
    fn list_item_depth() {
        let leaf = ListItem::new("l", "Leaf");
        assert_eq!(leaf.depth(), 0);
        assert!(leaf.is_leaf());

        let nested = ListItem::new("a", "A")
            .with_child(ListItem::new("b", "B").with_child(ListItem::new("c", "C")));
        assert_eq!(nested.depth(), 2);
        assert!(!nested.is_leaf());
    }

    #[test]
    fn descendant_count() {
        let item = ListItem::new("a", "A")
            .with_child(ListItem::new("b", "B"))
            .with_child(
                ListItem::new("c", "C").with_child(ListItem::new("d", "D")),
            );
        assert_eq!(item.descendant_count(), 3);
    }

    #[test]
    fn find_descendant() {
        let item = ListItem::new("root", "Root")
            .with_child(
                ListItem::new("mid", "Mid").with_child(ListItem::new("deep", "Deep")),
            );
        assert!(item.find_descendant("deep").is_some());
        assert!(item.find_descendant("nonexistent").is_none());
    }

    #[test]
    fn try_add_rejects_duplicate() {
        let mut lv = ListView::default();
        lv.add_item(ListItem::new("a", "A"));
        let result = lv.try_add_item(ListItem::new("a", "Dup"));
        assert_eq!(result, Err(ListError::DuplicateId("a".into())));
    }

    #[test]
    fn try_add_rejects_empty_id() {
        let mut lv = ListView::default();
        let result = lv.try_add_item(ListItem::new("", "Empty"));
        assert_eq!(result, Err(ListError::ValidationError("id must not be empty".into())));
    }

    #[test]
    fn try_select_returns_error() {
        let mut lv = ListView::default();
        lv.add_item(ListItem::new("a", "A"));
        assert!(lv.try_select("a").is_ok());
        assert_eq!(
            lv.try_select("missing"),
            Err(ListError::ItemNotFound("missing".into()))
        );
    }

    #[test]
    fn expand_and_collapse_all() {
        let mut lv = sample_tree();
        lv.expand_all();
        assert_eq!(lv.visible_count(), 4); // all 4 items visible
        lv.collapse_all();
        assert_eq!(lv.visible_count(), 2); // only top-level
    }

    #[test]
    fn select_focused_item() {
        let mut lv = ListView::default();
        lv.add_item(ListItem::new("a", "A"));
        lv.add_item(ListItem::new("b", "B"));
        lv.focus_next(); // focus on "a"
        lv.select_focused();
        assert_eq!(lv.selected_ids(), vec!["a"]);
    }

    #[test]
    fn multi_select_mode() {
        let mut lv = ListView::new(ListOptions {
            multi_select: true,
            ..Default::default()
        });
        lv.add_item(ListItem::new("a", "A"));
        lv.add_item(ListItem::new("b", "B"));
        lv.select("a");
        lv.select("b");
        assert_eq!(lv.get_selected().len(), 2);
    }

    #[test]
    fn display_impls() {
        let item = ListItem::new("x", "Hello").with_description("world");
        assert_eq!(format!("{item}"), "Hello (world)");

        let item2 = ListItem::new("y", "Bare");
        assert_eq!(format!("{item2}"), "Bare");

        let lv = ListView::default();
        assert!(format!("{lv}").contains("ListView("));

        let opts = ListOptions::default();
        assert!(format!("{opts}").contains("ListOptions("));
    }

    #[test]
    fn remove_item_resets_focus() {
        let mut lv = ListView::default();
        lv.add_item(ListItem::new("a", "A"));
        lv.add_item(ListItem::new("b", "B"));
        lv.focus_next(); // 0
        lv.focus_next(); // 1
        lv.remove_item("b");
        assert_eq!(lv.focused_index, Some(0));
    }

    #[test]
    fn find_item_in_list_view() {
        let mut lv = sample_tree();
        assert!(lv.find_item("c1").is_some());
        assert!(lv.find_item("nonexistent").is_none());
        assert_eq!(lv.max_depth(), 1);

        lv.toggle_expand("p1");
        assert_eq!(lv.focused_item(), None);
    }

    #[test]
    fn list_error_display() {
        let e1 = ListError::ItemNotFound("abc".into());
        assert_eq!(format!("{e1}"), "item not found: abc");
        let e2 = ListError::DuplicateId("xyz".into());
        assert_eq!(format!("{e2}"), "duplicate item id: xyz");
        let e3 = ListError::ValidationError("bad".into());
        assert_eq!(format!("{e3}"), "validation error: bad");
        // Verify it implements std::error::Error
        let _: &dyn std::error::Error = &e1;
    }

    #[test]
    fn test_virtual_list_viewport() {
        let vl = VirtualList::new(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9], 3);
        assert_eq!(vl.total_count(), 10);
        assert_eq!(vl.visible_items(), &[0, 1, 2]);
    }

    #[test]
    fn test_virtual_list_scroll_down_up() {
        let mut vl = VirtualList::new(vec![0, 1, 2, 3, 4], 3);
        vl.scroll_down(2);
        assert_eq!(vl.visible_items(), &[2, 3, 4]);
        vl.scroll_up(1);
        assert_eq!(vl.visible_items(), &[1, 2, 3]);
    }

    #[test]
    fn test_virtual_list_scroll_to() {
        let mut vl = VirtualList::new(vec![0, 1, 2, 3, 4], 3);
        vl.scroll_to(100);
        assert_eq!(vl.viewport_start, 2); // clamped to max
        vl.scroll_to(1);
        assert_eq!(vl.viewport_start, 1);
    }

    #[test]
    fn test_virtual_list_ensure_visible() {
        let mut vl = VirtualList::new(vec![0, 1, 2, 3, 4, 5], 3);
        vl.ensure_visible(4);
        assert_eq!(vl.viewport_start, 2);
        assert!(vl.is_visible(4));
        vl.ensure_visible(0);
        assert_eq!(vl.viewport_start, 0);
    }

    #[test]
    fn test_virtual_list_is_visible() {
        let vl = VirtualList::new(vec![0, 1, 2, 3, 4], 3);
        assert!(vl.is_visible(0));
        assert!(vl.is_visible(2));
        assert!(!vl.is_visible(3));
    }

    #[test]
    fn test_key_nav_up_down() {
        let mut nav = ListKeyNav::new(5);
        assert_eq!(nav.focused(), 0);
        nav.move_down();
        assert_eq!(nav.focused(), 1);
        nav.move_up();
        assert_eq!(nav.focused(), 0);
        nav.move_up(); // stays at 0
        assert_eq!(nav.focused(), 0);
    }

    #[test]
    fn test_key_nav_home_end() {
        let mut nav = ListKeyNav::new(10);
        nav.move_down();
        nav.move_down();
        nav.home();
        assert_eq!(nav.focused(), 0);
        nav.end();
        assert_eq!(nav.focused(), 9);
    }

    #[test]
    fn test_key_nav_page_up_down() {
        let mut nav = ListKeyNav::new(20);
        nav.page_down(5);
        assert_eq!(nav.focused(), 5);
        nav.page_down(100);
        assert_eq!(nav.focused(), 19);
        nav.page_up(10);
        assert_eq!(nav.focused(), 9);
        nav.page_up(100);
        assert_eq!(nav.focused(), 0);
    }

    #[test]
    fn test_key_nav_set_total() {
        let mut nav = ListKeyNav::new(10);
        nav.end(); // focused = 9
        nav.set_total(5);
        assert_eq!(nav.focused(), 4); // clamped
        nav.set_total(0);
        assert_eq!(nav.focused(), 0);
    }

    #[test]
    fn test_multi_select_click() {
        let mut ms = ListMultiSelect::new(5);
        ms.click(2);
        assert!(ms.is_selected(2));
        assert_eq!(ms.selected_count(), 1);
        ms.click(3);
        assert!(!ms.is_selected(2));
        assert!(ms.is_selected(3));
    }

    #[test]
    fn test_multi_select_ctrl_click() {
        let mut ms = ListMultiSelect::new(5);
        ms.ctrl_click(1);
        ms.ctrl_click(3);
        assert_eq!(ms.selected_count(), 2);
        assert!(ms.is_selected(1));
        assert!(ms.is_selected(3));
        ms.ctrl_click(1); // toggle off
        assert!(!ms.is_selected(1));
        assert_eq!(ms.selected_count(), 1);
    }

    #[test]
    fn test_multi_select_shift_click() {
        let mut ms = ListMultiSelect::new(10);
        ms.click(2); // anchor at 2
        ms.shift_click(5);
        assert_eq!(ms.selected_indices(), vec![2, 3, 4, 5]);
    }

    #[test]
    fn test_multi_select_select_all() {
        let mut ms = ListMultiSelect::new(4);
        ms.select_all();
        assert_eq!(ms.selected_count(), 4);
        ms.deselect_all();
        assert_eq!(ms.selected_count(), 0);
    }

    #[test]
    fn test_multi_select_set_total_prunes() {
        let mut ms = ListMultiSelect::new(10);
        ms.click(8);
        ms.ctrl_click(9);
        assert_eq!(ms.selected_count(), 2);
        ms.set_total(5);
        assert_eq!(ms.selected_count(), 0); // both pruned
    }

    #[test]
    fn list_stats_new_defaults() {
        let stats = ListStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn list_stats_record_success() {
        let mut stats = ListStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn list_stats_record_failure() {
        let mut stats = ListStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn list_stats_reset() {
        let mut stats = ListStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn list_stats_merge() {
        let mut a = ListStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ListStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn list_stats_display() {
        let mut stats = ListStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn list_stats_default() {
        let stats = ListStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn list_validator_accepts_valid_name() {
        let v = ListValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn list_validator_rejects_empty() {
        let v = ListValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn list_validator_rejects_too_long() {
        let v = ListValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn list_validator_forbidden_prefix() {
        let v = ListValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn list_validator_allowed_chars() {
        let v = ListValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn list_validator_range() {
        let v = ListValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn list_sanitize_removes_control() {
        let result = ListValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn list_truncate_short_string() {
        assert_eq!(ListValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn list_truncate_long_string() {
        let result = ListValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn list_is_ascii_printable() {
        assert!(ListValidator::is_ascii_printable("Hello World 123"));
        assert!(!ListValidator::is_ascii_printable("Hello\x00World"));
    }
}
