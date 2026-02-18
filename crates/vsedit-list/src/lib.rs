//! List / tree widget for sidebar panels.
//!
//! Provides a generic tree-backed list view with keyboard navigation,
//! expand/collapse, and multi-select support.

use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// List filtering / search
// ---------------------------------------------------------------------------

/// Filter a slice of `ListItem`s, returning only those whose label contains
/// `query` (case-insensitive). Children are filtered recursively; a parent is
/// kept if it or any descendant matches.
pub fn filter_items_by_label(items: &[ListItem], query: &str) -> Vec<ListItem> {
    if query.is_empty() {
        return items.to_vec();
    }
    let q = query.to_lowercase();
    items
        .iter()
        .filter_map(|item| {
            let child_matches = filter_items_by_label(&item.children, query);
            let self_matches = item.label.to_lowercase().contains(&q);
            if self_matches || !child_matches.is_empty() {
                let mut clone = item.clone();
                clone.children = child_matches;
                Some(clone)
            } else {
                None
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// List sorting
// ---------------------------------------------------------------------------

/// Sort top-level items alphabetically by label (case-insensitive).
/// Children within each item are also sorted recursively.
pub fn sort_items_by_label(items: &mut [ListItem]) {
    items.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
    for item in items.iter_mut() {
        sort_items_by_label(&mut item.children);
    }
}

// ---------------------------------------------------------------------------
// Bulk selection by predicate
// ---------------------------------------------------------------------------

impl ListView {
    /// Select all items (recursively) whose label satisfies `predicate`.
    pub fn select_by<F>(&mut self, predicate: F)
    where
        F: Fn(&ListItem) -> bool,
    {
        fn select_recursive(items: &mut [ListItem], pred: &dyn Fn(&ListItem) -> bool) {
            for item in items.iter_mut() {
                if pred(item) {
                    item.selected = true;
                }
                select_recursive(&mut item.children, pred);
            }
        }
        if !self.options.multi_select {
            deselect_all_recursive(&mut self.items);
        }
        select_recursive(&mut self.items, &predicate);
    }
}

// ---------------------------------------------------------------------------
// Path (breadcrumb trail) to an item
// ---------------------------------------------------------------------------

/// Return the breadcrumb path of labels from the root to the item with the
/// given `id`, or `None` if the item is not found.
pub fn item_path(items: &[ListItem], id: &str) -> Option<Vec<String>> {
    for item in items {
        if item.id == id {
            return Some(vec![item.label.clone()]);
        }
        if let Some(mut trail) = item_path(&item.children, id) {
            trail.insert(0, item.label.clone());
            return Some(trail);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Flat serialization
// ---------------------------------------------------------------------------

/// A flat representation of a single list item for serialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatListEntry {
    pub id: String,
    pub label: String,
    pub depth: usize,
    pub selected: bool,
    pub expanded: bool,
    pub has_children: bool,
}

/// Serialize a tree of `ListItem`s into a flat `Vec<FlatListEntry>`,
/// including all items regardless of expanded state.
pub fn flatten_all_to_entries(items: &[ListItem]) -> Vec<FlatListEntry> {
    fn collect(items: &[ListItem], depth: usize, out: &mut Vec<FlatListEntry>) {
        for item in items {
            out.push(FlatListEntry {
                id: item.id.clone(),
                label: item.label.clone(),
                depth,
                selected: item.selected,
                expanded: item.expanded,
                has_children: !item.children.is_empty(),
            });
            collect(&item.children, depth + 1, out);
        }
    }
    let mut out = Vec::new();
    collect(items, 0, &mut out);
    out
}

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

/// Paginated view over a list of items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPagination {
    total_items: usize,
    page_size: usize,
    current_page: usize,
}

impl ListPagination {
    /// Create a new pagination state. `page_size` is clamped to at least 1.
    pub fn new(total_items: usize, page_size: usize) -> Self {
        Self {
            total_items,
            page_size: page_size.max(1),
            current_page: 0,
        }
    }

    /// Total number of pages (always >= 1 when page_size > 0).
    pub fn total_pages(&self) -> usize {
        if self.total_items == 0 {
            return 1;
        }
        (self.total_items + self.page_size - 1) / self.page_size
    }

    /// Current zero-based page index.
    pub fn current_page(&self) -> usize {
        self.current_page
    }

    /// Move to the next page if possible. Returns `true` if the page changed.
    pub fn next_page(&mut self) -> bool {
        if self.current_page + 1 < self.total_pages() {
            self.current_page += 1;
            true
        } else {
            false
        }
    }

    /// Move to the previous page if possible. Returns `true` if the page changed.
    pub fn prev_page(&mut self) -> bool {
        if self.current_page > 0 {
            self.current_page -= 1;
            true
        } else {
            false
        }
    }

    /// Jump to a specific page, clamping to valid range.
    pub fn go_to_page(&mut self, page: usize) {
        self.current_page = page.min(self.total_pages().saturating_sub(1));
    }

    /// Return the start..end index range for the current page.
    pub fn page_range(&self) -> std::ops::Range<usize> {
        let start = self.current_page * self.page_size;
        let end = (start + self.page_size).min(self.total_items);
        start..end
    }

    /// Return items from `slice` that belong to the current page.
    pub fn page_items<'a, T>(&self, slice: &'a [T]) -> &'a [T] {
        let range = self.page_range();
        let start = range.start.min(slice.len());
        let end = range.end.min(slice.len());
        &slice[start..end]
    }

    /// Update the total item count, clamping the current page if needed.
    pub fn set_total(&mut self, total: usize) {
        self.total_items = total;
        let max_page = self.total_pages().saturating_sub(1);
        if self.current_page > max_page {
            self.current_page = max_page;
        }
    }

    /// Whether the current page is the first page.
    pub fn is_first_page(&self) -> bool {
        self.current_page == 0
    }

    /// Whether the current page is the last page.
    pub fn is_last_page(&self) -> bool {
        self.current_page + 1 >= self.total_pages()
    }
}

// ---------------------------------------------------------------------------
// Drag reorder
// ---------------------------------------------------------------------------

/// Result of a drag-reorder operation on a `Vec`.
/// Returns `Ok(())` on success, or an error description.
pub fn drag_reorder<T>(items: &mut Vec<T>, from: usize, to: usize) -> Result<(), String> {
    if from >= items.len() {
        return Err(format!("source index {} out of range (len={})", from, items.len()));
    }
    if to >= items.len() {
        return Err(format!("target index {} out of range (len={})", to, items.len()));
    }
    if from == to {
        return Ok(());
    }
    let item = items.remove(from);
    items.insert(to, item);
    Ok(())
}

// ---------------------------------------------------------------------------
// Virtual scrolling calculations
// ---------------------------------------------------------------------------

/// Parameters and calculations for a virtual-scrolling viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualScrollCalc {
    pub total_items: usize,
    pub item_height: usize,
    pub viewport_height: usize,
    pub buffer_items: usize,
}

impl VirtualScrollCalc {
    pub fn new(total_items: usize, item_height: usize, viewport_height: usize) -> Self {
        Self {
            total_items,
            item_height: item_height.max(1),
            viewport_height,
            buffer_items: 5,
        }
    }

    /// Set the number of extra items rendered above and below the viewport.
    pub fn with_buffer(mut self, buffer: usize) -> Self {
        self.buffer_items = buffer;
        self
    }

    /// Number of items that fit inside the viewport (without buffer).
    pub fn visible_count(&self) -> usize {
        if self.item_height == 0 {
            return 0;
        }
        (self.viewport_height + self.item_height - 1) / self.item_height
    }

    /// Total scrollable content height in pixels.
    pub fn total_height(&self) -> usize {
        self.total_items * self.item_height
    }

    /// Given a scroll offset in pixels, return the range of item indices that
    /// should be rendered (including the buffer zone).
    pub fn render_range(&self, scroll_offset: usize) -> std::ops::Range<usize> {
        let first_visible = scroll_offset / self.item_height;
        let start = first_visible.saturating_sub(self.buffer_items);
        let end = (first_visible + self.visible_count() + self.buffer_items).min(self.total_items);
        start..end
    }

    /// Pixel offset for the top of item at `index`.
    pub fn item_offset(&self, index: usize) -> usize {
        index * self.item_height
    }

    /// Scroll offset needed to make `index` the first visible item.
    pub fn scroll_to_item(&self, index: usize) -> usize {
        self.item_offset(index.min(self.total_items.saturating_sub(1)))
    }
}

// ---------------------------------------------------------------------------
// List diff
// ---------------------------------------------------------------------------

/// Describes a single change between two list snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListDiffEntry {
    /// Item was added (id, label).
    Added(String, String),
    /// Item was removed (id, label).
    Removed(String, String),
    /// Item label changed (id, old_label, new_label).
    Changed(String, String, String),
}

/// Compute the diff between two flat slices of `ListItem`s (compared by `id`).
/// Items are matched by id; label changes are detected for matching ids.
pub fn list_diff(old: &[ListItem], new: &[ListItem]) -> Vec<ListDiffEntry> {
    use std::collections::HashMap;
    let old_map: HashMap<&str, &ListItem> = old.iter().map(|i| (i.id.as_str(), i)).collect();
    let new_map: HashMap<&str, &ListItem> = new.iter().map(|i| (i.id.as_str(), i)).collect();

    let mut changes = Vec::new();

    // Removed or changed
    for item in old {
        match new_map.get(item.id.as_str()) {
            None => changes.push(ListDiffEntry::Removed(item.id.clone(), item.label.clone())),
            Some(new_item) if new_item.label != item.label => {
                changes.push(ListDiffEntry::Changed(
                    item.id.clone(),
                    item.label.clone(),
                    new_item.label.clone(),
                ));
            }
            _ => {}
        }
    }

    // Added
    for item in new {
        if !old_map.contains_key(item.id.as_str()) {
            changes.push(ListDiffEntry::Added(item.id.clone(), item.label.clone()));
        }
    }

    changes
}

// ---------------------------------------------------------------------------
// List grouping / sectioning
// ---------------------------------------------------------------------------

/// A group of list items sharing a common section key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListGroup {
    pub key: String,
    pub items: Vec<ListItem>,
}

/// Group a flat slice of `ListItem`s by a key function. Groups preserve the
/// order of the first occurrence of each key.
pub fn group_items_by<F>(items: &[ListItem], key_fn: F) -> Vec<ListGroup>
where
    F: Fn(&ListItem) -> String,
{
    use std::collections::HashMap;
    let mut order: Vec<String> = Vec::new();
    let mut map: HashMap<String, Vec<ListItem>> = HashMap::new();

    for item in items {
        let k = key_fn(item);
        if !map.contains_key(&k) {
            order.push(k.clone());
        }
        map.entry(k).or_default().push(item.clone());
    }

    order
        .into_iter()
        .map(|k| {
            let items = map.remove(&k).unwrap_or_default();
            ListGroup { key: k, items }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// ListAccessibilityProvider – ARIA attributes
// ---------------------------------------------------------------------------

/// ARIA role for list items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaRole {
    TreeItem,
    ListItem,
    Option,
    Row,
}

impl fmt::Display for AriaRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TreeItem => write!(f, "treeitem"),
            Self::ListItem => write!(f, "listitem"),
            Self::Option => write!(f, "option"),
            Self::Row => write!(f, "row"),
        }
    }
}

/// Provides accessibility attributes for a list item.
#[derive(Debug, Clone)]
pub struct ListAccessibilityProvider {
    pub role: AriaRole,
    pub label: String,
    pub level: u32,
    pub set_size: u32,
    pub pos_in_set: u32,
    pub expanded: Option<bool>,
    pub selected: bool,
}

impl ListAccessibilityProvider {
    /// Build accessibility info from a list item and its context.
    pub fn from_item(item: &ListItem, level: u32, pos: u32, siblings: u32) -> Self {
        Self {
            role: if item.is_leaf() { AriaRole::ListItem } else { AriaRole::TreeItem },
            label: item.label.clone(),
            level,
            set_size: siblings,
            pos_in_set: pos,
            expanded: if item.is_leaf() { None } else { Some(item.expanded) },
            selected: item.selected,
        }
    }

    /// Format as HTML aria attribute string.
    pub fn to_aria_attrs(&self) -> String {
        let mut attrs = format!(
            "role=\"{}\" aria-label=\"{}\" aria-level=\"{}\" aria-setsize=\"{}\" aria-posinset=\"{}\"",
            self.role, self.label, self.level, self.set_size, self.pos_in_set
        );
        if let Some(exp) = self.expanded {
            attrs.push_str(&format!(" aria-expanded=\"{}\"", exp));
        }
        if self.selected {
            attrs.push_str(" aria-selected=\"true\"");
        }
        attrs
    }
}

// ---------------------------------------------------------------------------
// ListFilterWidget – inline filtering
// ---------------------------------------------------------------------------

/// Inline filter widget for filtering list items by text.
#[derive(Debug, Clone)]
pub struct ListFilterWidget {
    query: String,
    active: bool,
}

impl ListFilterWidget {
    pub fn new() -> Self {
        Self { query: String::new(), active: false }
    }

    /// Activate the filter.
    pub fn activate(&mut self) {
        self.active = true;
    }

    /// Deactivate and clear the filter.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.query.clear();
    }

    /// Set the filter query.
    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
    }

    /// Filter items by label (case-insensitive substring match).
    pub fn filter<'a>(&self, items: &'a [ListItem]) -> Vec<&'a ListItem> {
        if self.query.is_empty() {
            return items.iter().collect();
        }
        let q = self.query.to_lowercase();
        items.iter().filter(|item| item.label.to_lowercase().contains(&q)).collect()
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn query(&self) -> &str {
        &self.query
    }
}

impl Default for ListFilterWidget {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ListStickyScroll – sticky header for tree items
// ---------------------------------------------------------------------------

/// Tracks which parent items should be "sticky" at the top of the viewport.
#[derive(Debug, Clone)]
pub struct ListStickyScroll {
    sticky_items: Vec<String>,
    max_sticky: usize,
}

impl ListStickyScroll {
    pub fn new(max_sticky: usize) -> Self {
        Self { sticky_items: Vec::new(), max_sticky }
    }

    /// Update sticky items based on the first visible item in the viewport.
    pub fn update(&mut self, ancestors: Vec<String>) {
        self.sticky_items = ancestors;
        self.sticky_items.truncate(self.max_sticky);
    }

    /// Currently sticky item labels.
    pub fn sticky_labels(&self) -> &[String] {
        &self.sticky_items
    }

    /// Number of sticky items.
    pub fn count(&self) -> usize {
        self.sticky_items.len()
    }

    /// Clear sticky state.
    pub fn clear(&mut self) {
        self.sticky_items.clear();
    }
}


// ---------------------------------------------------------------------------
// ListColumnSorter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ListColumnSorter {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl ListColumnSorter {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for ListColumnSorter {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for ListColumnSorter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "ListColumnSorter({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// ListGroupCollapser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ListGroupCollapser {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl ListGroupCollapser {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for ListGroupCollapser {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for ListGroupCollapser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "ListGroupCollapser({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// ListColumnSorterSnapshot — point-in-time snapshot of ListColumnSorter state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ListColumnSorterSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl ListColumnSorterSnapshot {
    pub fn capture(source: &ListColumnSorter, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for ListColumnSorterSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// ListGroupCollapserStats — aggregate statistics for ListGroupCollapser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ListGroupCollapserStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl ListGroupCollapserStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for ListGroupCollapserStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// ListColumnSorterConfig — configuration for ListColumnSorter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ListColumnSorterConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl ListColumnSorterConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for ListColumnSorterConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for ListColumnSorterConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// --- VirtualScrollState ---

pub struct VirtualScrollState {
    total_items: usize,
    visible_count: usize,
    scroll_offset: usize,
}

impl VirtualScrollState {
    pub fn new(total_items: usize, visible_count: usize) -> Self {
        Self { total_items, visible_count, scroll_offset: 0 }
    }

    pub fn scroll_to(&mut self, index: usize) {
        self.scroll_offset = index.min(self.total_items.saturating_sub(self.visible_count));
    }

    pub fn scroll_by(&mut self, delta: isize) {
        if delta >= 0 {
            self.scroll_offset = (self.scroll_offset + delta as usize)
                .min(self.total_items.saturating_sub(self.visible_count));
        } else {
            self.scroll_offset = self.scroll_offset.saturating_sub((-delta) as usize);
        }
    }

    pub fn page_up(&mut self) { self.scroll_by(-(self.visible_count as isize)); }
    pub fn page_down(&mut self) { self.scroll_by(self.visible_count as isize); }

    pub fn visible_range(&self) -> (usize, usize) {
        let end = (self.scroll_offset + self.visible_count).min(self.total_items);
        (self.scroll_offset, end)
    }

    pub fn is_at_top(&self) -> bool { self.scroll_offset == 0 }

    pub fn is_at_bottom(&self) -> bool {
        self.scroll_offset + self.visible_count >= self.total_items
    }

    pub fn ensure_visible(&mut self, index: usize) {
        if index < self.scroll_offset {
            self.scroll_offset = index;
        } else if index >= self.scroll_offset + self.visible_count {
            self.scroll_offset = index.saturating_sub(self.visible_count - 1);
        }
    }

    pub fn total_items(&self) -> usize { self.total_items }
    pub fn set_total_items(&mut self, n: usize) { self.total_items = n; }
}

// --- SelectionModel ---

pub struct SelectionModel {
    selected: Vec<bool>,
    anchor: Option<usize>,
}

impl SelectionModel {
    pub fn new(count: usize) -> Self {
        Self { selected: vec![false; count], anchor: None }
    }

    pub fn select(&mut self, index: usize) {
        if index < self.selected.len() { self.selected[index] = true; self.anchor = Some(index); }
    }

    pub fn deselect(&mut self, index: usize) {
        if index < self.selected.len() { self.selected[index] = false; }
    }

    pub fn toggle(&mut self, index: usize) {
        if index < self.selected.len() { self.selected[index] = !self.selected[index]; }
    }

    pub fn select_range(&mut self, from: usize, to: usize) {
        let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
        for i in lo..=hi.min(self.selected.len() - 1) {
            self.selected[i] = true;
        }
    }

    pub fn select_all(&mut self) { self.selected.iter_mut().for_each(|s| *s = true); }
    pub fn clear(&mut self) { self.selected.iter_mut().for_each(|s| *s = false); }

    pub fn selected_indices(&self) -> Vec<usize> {
        self.selected.iter().enumerate().filter(|&(_, &s)| s).map(|(i, _)| i).collect()
    }

    pub fn is_selected(&self, index: usize) -> bool {
        self.selected.get(index).copied().unwrap_or(false)
    }

    pub fn selection_count(&self) -> usize { self.selected.iter().filter(|&&s| s).count() }
}

// --- ListFilterV2 ---

pub struct ListFilterV2 {
    filter_text: String,
}

impl ListFilterV2 {
    pub fn new() -> Self { Self { filter_text: String::new() } }

    pub fn set_filter(&mut self, text: &str) { self.filter_text = text.to_lowercase(); }
    pub fn clear_filter(&mut self) { self.filter_text.clear(); }
    pub fn is_active(&self) -> bool { !self.filter_text.is_empty() }

    pub fn matches(&self, item_label: &str) -> bool {
        if self.filter_text.is_empty() { return true; }
        item_label.to_lowercase().contains(&self.filter_text)
    }

    pub fn filtered_indices(&self, items: &[&str]) -> Vec<usize> {
        items.iter().enumerate()
            .filter(|(_, label)| self.matches(label))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn match_count(&self, items: &[&str]) -> usize {
        items.iter().filter(|l| self.matches(l)).count()
    }
}


/// List widget configuration manager.
#[derive(Debug, Clone)]
pub struct ListConfig {
    entries: Vec<ListEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single list widget entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ListEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl ListEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl ListConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: ListEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&ListEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut ListEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&ListEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&ListEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&ListEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<ListEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Virtual list widget — extended utilities (qh)
// ---------------------------------------------------------------------------

/// Metric accumulator for list operations.
#[derive(Debug, Clone)]
pub struct QhMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QhMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for list.
#[derive(Debug, Clone)]
pub struct QhRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QhRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for list lookups.
#[derive(Debug, Clone)]
pub struct QhLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QhLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for list
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaListRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaListRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaListCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaListCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaListCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 115
// ---------------------------------------------------------------------------

/// Generic object pool `Xc115Pool<T>`.
pub struct Xc115Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc115Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc115PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc115Pool<T> {
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
    pub fn stats(&self) -> Xc115PoolStats {
        Xc115PoolStats {
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

impl<T> Default for Xc115Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc115Scheduler`.
pub struct Xc115Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc115Scheduler {
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

impl Default for Xc115Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_115 hash for the given byte slice.
pub fn xc_115_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_115 convention.
pub fn xc_115_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe8 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe8Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe8PipelineError {
    pub stage: Xe8Stage,
    pub message: String,
}

impl std::fmt::Display for Xe8PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe8Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe8Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe8PipelineError>>>,
    stage_names: Vec<Xe8Stage>,
}

impl Xe8Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe8PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe8Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe8PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe8Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe8PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe8Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe8PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe8Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe8PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe8Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe8CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe8CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe8Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe8CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe8CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe8Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe8CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_8_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe8CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_8_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe8CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_8_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe8PipelineError> {
    Ok(data)
}

pub fn xe_8_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe8PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_8_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe8PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_8_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe8PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_8_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe8PipelineError> {
    Err(Xe8PipelineError {
        stage: Xe8Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #72
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf72Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf72TrieNode {
    children: std::collections::HashMap<char, Xf72TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf72Trie {
    root: Xf72TrieNode,
    count: usize,
}

impl Xf72Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf72TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf72TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf72TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf72BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf72BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 114).
pub struct Xh114SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh114SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 156 as u64,
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

/// A compact bit set supporting boolean operations (variant 114).
pub struct Xh114BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh114BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 114).
pub struct Xi114Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi114Deque<T> {
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
pub struct Xi114Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi114Interval {
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

/// A simple interval tree (variant 114).
pub struct Xi114IntervalTree {
    xi_intervals: Vec<Xi114Interval>,
}

impl Xi114IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi114Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi114Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi114Interval) -> Vec<&Xi114Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi114Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi114Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi114Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi114Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi114Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi114Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 114) ---

/// Disjoint set / union-find for crate 114.
pub struct Xj114UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj114UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ114_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 114.
pub struct Xj114BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj114BTreeNode<K, V>>>,
    len: usize,
}

struct Xj114BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj114BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj114BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ114_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ114_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj114BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj114BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj114BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj114BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
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

    #[test]
    fn filter_items_by_label_matches_parent() {
        let items = vec![
            ListItem::new("a", "Alpha"),
            ListItem::new("b", "Beta"),
            ListItem::new("c", "Gamma"),
        ];
        let result = filter_items_by_label(&items, "alp");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "a");
    }

    #[test]
    fn filter_items_by_label_keeps_parent_if_child_matches() {
        let parent = ListItem::new("p", "Parent")
            .with_child(ListItem::new("c1", "Target"))
            .with_child(ListItem::new("c2", "Other"));
        let items = vec![parent];
        let result = filter_items_by_label(&items, "target");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].children.len(), 1);
        assert_eq!(result[0].children[0].id, "c1");
    }

    #[test]
    fn sort_items_by_label_alphabetical() {
        let mut items = vec![
            ListItem::new("c", "Gamma"),
            ListItem::new("a", "Alpha"),
            ListItem::new("b", "Beta"),
        ];
        sort_items_by_label(&mut items);
        assert_eq!(items[0].label, "Alpha");
        assert_eq!(items[1].label, "Beta");
        assert_eq!(items[2].label, "Gamma");
    }

    #[test]
    fn select_by_predicate() {
        let mut lv = ListView::new(ListOptions { multi_select: true, ..Default::default() });
        lv.add_item(ListItem::new("a", "Apple"));
        lv.add_item(ListItem::new("b", "Banana"));
        lv.add_item(ListItem::new("c", "Avocado"));
        lv.select_by(|item| item.label.starts_with('A'));
        let sel = lv.selected_ids();
        assert_eq!(sel.len(), 2);
        assert!(sel.contains(&"a"));
        assert!(sel.contains(&"c"));
    }

    #[test]
    fn item_path_finds_breadcrumb() {
        let tree = vec![
            ListItem::new("root", "Root")
                .with_child(
                    ListItem::new("mid", "Middle")
                        .with_child(ListItem::new("leaf", "Leaf"))
                ),
        ];
        let path = item_path(&tree, "leaf");
        assert_eq!(path, Some(vec!["Root".to_string(), "Middle".to_string(), "Leaf".to_string()]));
        assert_eq!(item_path(&tree, "nonexistent"), None);
    }

    #[test]
    fn flatten_all_to_entries_depth() {
        let items = vec![
            ListItem::new("a", "A")
                .with_child(ListItem::new("b", "B")),
            ListItem::new("c", "C"),
        ];
        let flat = flatten_all_to_entries(&items);
        assert_eq!(flat.len(), 3);
        assert_eq!(flat[0].depth, 0);
        assert_eq!(flat[0].has_children, true);
        assert_eq!(flat[1].depth, 1);
        assert_eq!(flat[1].has_children, false);
        assert_eq!(flat[2].depth, 0);
    }

    #[test]
    fn filter_items_empty_query_returns_all() {
        let items = vec![
            ListItem::new("a", "Alpha"),
            ListItem::new("b", "Beta"),
        ];
        let result = filter_items_by_label(&items, "");
        assert_eq!(result.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Pagination tests
    // -----------------------------------------------------------------------

    #[test]
    fn pagination_page_navigation() {
        let mut pg = ListPagination::new(25, 10);
        assert_eq!(pg.total_pages(), 3);
        assert_eq!(pg.current_page(), 0);
        assert!(pg.is_first_page());
        assert!(!pg.is_last_page());

        assert!(pg.next_page());
        assert_eq!(pg.current_page(), 1);
        assert!(pg.next_page());
        assert_eq!(pg.current_page(), 2);
        assert!(pg.is_last_page());
        assert!(!pg.next_page()); // already at last

        assert!(pg.prev_page());
        assert_eq!(pg.current_page(), 1);
        pg.go_to_page(0);
        assert_eq!(pg.current_page(), 0);
        pg.go_to_page(100); // clamped
        assert_eq!(pg.current_page(), 2);
    }

    #[test]
    fn pagination_page_range_and_items() {
        let pg = ListPagination::new(7, 3);
        assert_eq!(pg.page_range(), 0..3);

        let data: Vec<i32> = (0..7).collect();
        assert_eq!(pg.page_items(&data), &[0, 1, 2]);

        let mut pg2 = ListPagination::new(7, 3);
        pg2.go_to_page(2); // last page
        assert_eq!(pg2.page_range(), 6..7);
        assert_eq!(pg2.page_items(&data), &[6]);
    }

    #[test]
    fn pagination_set_total_clamps() {
        let mut pg = ListPagination::new(30, 10);
        pg.go_to_page(2); // page index 2
        pg.set_total(15); // now only 2 pages (0..1)
        assert_eq!(pg.current_page(), 1);
    }

    // -----------------------------------------------------------------------
    // Drag reorder tests
    // -----------------------------------------------------------------------

    #[test]
    fn drag_reorder_moves_item() {
        let mut v = vec!["a", "b", "c", "d"];
        assert!(drag_reorder(&mut v, 0, 2).is_ok());
        assert_eq!(v, vec!["b", "c", "a", "d"]);

        assert!(drag_reorder(&mut v, 3, 0).is_ok());
        assert_eq!(v, vec!["d", "b", "c", "a"]);

        // no-op
        assert!(drag_reorder(&mut v, 1, 1).is_ok());
        assert_eq!(v, vec!["d", "b", "c", "a"]);
    }

    #[test]
    fn drag_reorder_out_of_bounds() {
        let mut v = vec![1, 2, 3];
        assert!(drag_reorder(&mut v, 5, 0).is_err());
        assert!(drag_reorder(&mut v, 0, 10).is_err());
    }

    // -----------------------------------------------------------------------
    // Virtual scroll calc tests
    // -----------------------------------------------------------------------

    #[test]
    fn virtual_scroll_calc_render_range() {
        let vs = VirtualScrollCalc::new(100, 20, 200).with_buffer(3);
        // 200/20 = 10 visible items
        assert_eq!(vs.visible_count(), 10);
        assert_eq!(vs.total_height(), 2000);

        // At scroll offset 0, first visible = 0
        let range = vs.render_range(0);
        assert_eq!(range.start, 0);
        assert_eq!(range.end, 13); // 0 + 10 + 3 buffer

        // At scroll offset 400 (item 20), with buffer 3
        let range2 = vs.render_range(400);
        assert_eq!(range2.start, 17); // 20 - 3
        assert_eq!(range2.end, 33); // 20 + 10 + 3

        assert_eq!(vs.item_offset(5), 100);
        assert_eq!(vs.scroll_to_item(10), 200);
    }

    // -----------------------------------------------------------------------
    // List diff tests
    // -----------------------------------------------------------------------

    #[test]
    fn list_diff_detects_changes() {
        let old = vec![
            ListItem::new("a", "Alpha"),
            ListItem::new("b", "Beta"),
            ListItem::new("c", "Gamma"),
        ];
        let new = vec![
            ListItem::new("a", "Alpha"),
            ListItem::new("b", "Beta Renamed"),
            ListItem::new("d", "Delta"),
        ];
        let diff = list_diff(&old, &new);
        assert_eq!(diff.len(), 3);
        assert!(diff.contains(&ListDiffEntry::Removed("c".into(), "Gamma".into())));
        assert!(diff.contains(&ListDiffEntry::Changed(
            "b".into(),
            "Beta".into(),
            "Beta Renamed".into()
        )));
        assert!(diff.contains(&ListDiffEntry::Added("d".into(), "Delta".into())));
    }

    #[test]
    fn list_diff_identical_is_empty() {
        let items = vec![ListItem::new("x", "X")];
        assert!(list_diff(&items, &items).is_empty());
    }

    // -----------------------------------------------------------------------
    // Group items tests
    // -----------------------------------------------------------------------

    #[test]
    fn group_items_by_first_char() {
        let items = vec![
            ListItem::new("a1", "Apple"),
            ListItem::new("a2", "Avocado"),
            ListItem::new("b1", "Banana"),
            ListItem::new("c1", "Cherry"),
            ListItem::new("b2", "Blueberry"),
        ];
        let groups = group_items_by(&items, |i| {
            i.label.chars().next().unwrap_or('?').to_string()
        });
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].key, "A");
        assert_eq!(groups[0].items.len(), 2);
        assert_eq!(groups[1].key, "B");
        assert_eq!(groups[1].items.len(), 2);
        assert_eq!(groups[2].key, "C");
        assert_eq!(groups[2].items.len(), 1);
    }

    // -- ListAccessibilityProvider tests --

    #[test]
    fn a11y_leaf_item() {
        let item = ListItem::new("f1", "file.rs");
        let a11y = ListAccessibilityProvider::from_item(&item, 1, 1, 5);
        assert_eq!(a11y.role, AriaRole::ListItem);
        assert!(a11y.expanded.is_none());
        let attrs = a11y.to_aria_attrs();
        assert!(attrs.contains("listitem"));
        assert!(attrs.contains("file.rs"));
    }

    #[test]
    fn a11y_tree_item_expanded() {
        let item = ListItem::new("d1", "src").with_child(ListItem::new("f1", "main.rs")).with_expanded(true);
        let a11y = ListAccessibilityProvider::from_item(&item, 0, 1, 3);
        assert_eq!(a11y.role, AriaRole::TreeItem);
        assert_eq!(a11y.expanded, Some(true));
        let attrs = a11y.to_aria_attrs();
        assert!(attrs.contains("aria-expanded=\"true\""));
    }

    #[test]
    fn aria_role_display() {
        assert_eq!(format!("{}", AriaRole::Option), "option");
        assert_eq!(format!("{}", AriaRole::Row), "row");
    }

    // -- ListMultiSelect tests (using existing ListMultiSelect) --

    #[test]
    fn multi_select_click() {
        let mut ms = ListMultiSelect::new(10);
        ms.click(3);
        assert!(ms.is_selected(3));
        assert!(!ms.is_selected(0));
        assert_eq!(ms.selected_count(), 1);
    }

    #[test]
    fn multi_select_shift_click() {
        let mut ms = ListMultiSelect::new(10);
        ms.click(2);
        ms.shift_click(5);
        assert_eq!(ms.selected_indices(), vec![2, 3, 4, 5]);
        assert_eq!(ms.selected_count(), 4);
    }

    #[test]
    fn multi_select_ctrl_click_toggle() {
        let mut ms = ListMultiSelect::new(10);
        ms.click(1);
        ms.ctrl_click(3);
        assert_eq!(ms.selected_count(), 2);
        ms.ctrl_click(1); // deselect
        assert_eq!(ms.selected_count(), 1);
        assert!(!ms.is_selected(1));
    }

    #[test]
    fn multi_select_deselect_all() {
        let mut ms = ListMultiSelect::new(10);
        ms.click(0);
        ms.shift_click(5);
        ms.deselect_all();
        assert_eq!(ms.selected_count(), 0);
    }

    // -- ListFilterWidget tests --

    #[test]
    fn filter_basic() {
        let mut fw = ListFilterWidget::new();
        let items = vec![
            ListItem::new("1", "Apple"),
            ListItem::new("2", "Banana"),
            ListItem::new("3", "Avocado"),
        ];
        fw.activate();
        fw.set_query("a");
        let filtered = fw.filter(&items);
        // "Apple", "Banana", "Avocado" all contain 'a'
        assert_eq!(filtered.len(), 3);
        fw.set_query("av");
        let filtered = fw.filter(&items);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].label, "Avocado");
    }

    #[test]
    fn filter_empty_query_returns_all() {
        let fw = ListFilterWidget::default();
        let items = vec![ListItem::new("1", "X"), ListItem::new("2", "Y")];
        assert_eq!(fw.filter(&items).len(), 2);
    }

    #[test]
    fn filter_deactivate_clears() {
        let mut fw = ListFilterWidget::new();
        fw.activate();
        fw.set_query("test");
        fw.deactivate();
        assert!(!fw.is_active());
        assert!(fw.query().is_empty());
    }

    // -- ListStickyScroll tests --

    #[test]
    fn sticky_scroll_basic() {
        let mut ss = ListStickyScroll::new(2);
        ss.update(vec!["root".into(), "src".into(), "lib".into()]);
        assert_eq!(ss.count(), 2); // truncated to max
        assert_eq!(ss.sticky_labels(), &["root", "src"]);
    }

    #[test]
    fn sticky_scroll_clear() {
        let mut ss = ListStickyScroll::new(3);
        ss.update(vec!["a".into()]);
        ss.clear();
        assert_eq!(ss.count(), 0);
    }

    #[test] fn listColumnSorter_new() { let s = ListColumnSorter::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn listColumnSorter_add() { let mut s = ListColumnSorter::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn listColumnSorter_remove() { let mut s = ListColumnSorter::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn listColumnSorter_config() { let mut s = ListColumnSorter::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn listColumnSorter_nav() { let mut s = ListColumnSorter::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn listColumnSorter_filter() { let mut s = ListColumnSorter::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn listColumnSorter_display() { assert!(format!("{}", ListColumnSorter::new()).contains("ListColumnSorter")); }
    #[test] fn listGroupCollapser_new() { let s = ListGroupCollapser::new(); assert!(s.is_empty()); }
    #[test] fn listGroupCollapser_add() { let mut s = ListGroupCollapser::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn listGroupCollapser_active() { let mut s = ListGroupCollapser::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn listGroupCollapser_error() { let mut s = ListGroupCollapser::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn listGroupCollapser_rm_group() { let mut s = ListGroupCollapser::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn listGroupCollapser_display() { assert!(format!("{}", ListGroupCollapser::new()).contains("ListGroupCollapser")); }


    #[test] fn listColumnSorter_snap_capture() {
        let s = ListColumnSorter::new();
        let snap = ListColumnSorterSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn listColumnSorter_snap_stale() {
        let s = ListColumnSorter::new();
        let snap = ListColumnSorterSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn listColumnSorter_snap_diff() {
        let s = ListColumnSorter::new();
        let s1v = ListColumnSorterSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn listColumnSorter_snap_display() {
        let s = ListColumnSorter::new();
        let snap = ListColumnSorterSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn listGroupCollapser_stats_record() {
        let mut st = ListGroupCollapserStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn listGroupCollapser_stats_hit_ratio() {
        let mut st = ListGroupCollapserStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn listGroupCollapser_stats_merge() {
        let mut a = ListGroupCollapserStats::new();
        a.total_adds = 5;
        let mut b = ListGroupCollapserStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn listGroupCollapser_stats_display() {
        let st = ListGroupCollapserStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn listColumnSorter_config_default() {
        let c = ListColumnSorterConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn listColumnSorter_config_builder() {
        let c = ListColumnSorterConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn listColumnSorter_config_labels() {
        let mut c = ListColumnSorterConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn listColumnSorter_config_cleanup_threshold() {
        let c = ListColumnSorterConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn listColumnSorter_config_display() {
        assert!(format!("{}", ListColumnSorterConfig::new()).contains("Config"));
    }
    #[test] fn listGroupCollapser_stats_peaks() {
        let mut st = ListGroupCollapserStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    #[test]
    fn virtual_scroll_visible_range() {
        let s = VirtualScrollState::new(100, 10);
        assert_eq!(s.visible_range(), (0, 10));
    }

    #[test]
    fn virtual_scroll_scroll_to() {
        let mut s = VirtualScrollState::new(100, 10);
        s.scroll_to(50);
        assert_eq!(s.visible_range(), (50, 60));
    }

    #[test]
    fn virtual_scroll_scroll_to_clamps() {
        let mut s = VirtualScrollState::new(20, 10);
        s.scroll_to(999);
        assert_eq!(s.visible_range(), (10, 20));
    }

    #[test]
    fn virtual_scroll_page_up_down() {
        let mut s = VirtualScrollState::new(100, 10);
        s.page_down();
        assert_eq!(s.visible_range(), (10, 20));
        s.page_up();
        assert!(s.is_at_top());
    }

    #[test]
    fn virtual_scroll_is_at_bottom() {
        let mut s = VirtualScrollState::new(20, 10);
        s.scroll_to(10);
        assert!(s.is_at_bottom());
    }

    #[test]
    fn virtual_scroll_ensure_visible() {
        let mut s = VirtualScrollState::new(100, 10);
        s.ensure_visible(50);
        assert!(s.visible_range().0 <= 50 && s.visible_range().1 > 50);
    }

    #[test]
    fn selection_model_select_deselect() {
        let mut m = SelectionModel::new(5);
        m.select(2);
        assert!(m.is_selected(2));
        m.deselect(2);
        assert!(!m.is_selected(2));
    }

    #[test]
    fn selection_model_toggle() {
        let mut m = SelectionModel::new(3);
        m.toggle(1);
        assert!(m.is_selected(1));
        m.toggle(1);
        assert!(!m.is_selected(1));
    }

    #[test]
    fn selection_model_select_range() {
        let mut m = SelectionModel::new(10);
        m.select_range(3, 6);
        assert_eq!(m.selected_indices(), vec![3, 4, 5, 6]);
        assert_eq!(m.selection_count(), 4);
    }

    #[test]
    fn selection_model_select_all_clear() {
        let mut m = SelectionModel::new(5);
        m.select_all();
        assert_eq!(m.selection_count(), 5);
        m.clear();
        assert_eq!(m.selection_count(), 0);
    }

    #[test]
    fn list_filter_v2_matches() {
        let mut f = ListFilterV2::new();
        f.set_filter("hel");
        assert!(f.matches("Hello World"));
        assert!(!f.matches("Goodbye"));
    }

    #[test]
    fn list_filter_v2_filtered_indices() {
        let mut f = ListFilterV2::new();
        f.set_filter("a");
        let items = vec!["apple", "banana", "cherry", "avocado"];
        assert_eq!(f.filtered_indices(&items), vec![0, 1, 3]);
        assert_eq!(f.match_count(&items), 3);
    }


    #[test]
    fn list_entry_creation() {
        let e = ListEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn list_entry_with_priority() {
        let e = ListEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn list_entry_metadata() {
        let e = ListEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn list_entry_remove_meta() {
        let mut e = ListEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn list_entry_activate_deactivate() {
        let mut e = ListEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn list_config_add_sorted() {
        let mut c = ListConfig::new(10);
        c.add(ListEntry::new("lo", "Lo").with_priority(1));
        c.add(ListEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn list_config_capacity() {
        let mut c = ListConfig::new(1);
        assert!(c.add(ListEntry::new("a", "A")));
        assert!(!c.add(ListEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn list_config_remove() {
        let mut c = ListConfig::new(10);
        c.add(ListEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn list_config_get() {
        let mut c = ListConfig::new(10);
        c.add(ListEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn list_config_active_entries() {
        let mut c = ListConfig::new(10);
        c.add(ListEntry::new("a", "A"));
        c.add(ListEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn list_config_enable_disable() {
        let mut c = ListConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn list_config_clear() {
        let mut c = ListConfig::new(10);
        c.add(ListEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn list_config_find_by_label() {
        let mut c = ListConfig::new(10);
        c.add(ListEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn list_config_top_n() {
        let mut c = ListConfig::new(10);
        c.add(ListEntry::new("a", "A").with_priority(1));
        c.add(ListEntry::new("b", "B").with_priority(2));
        c.add(ListEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn list_config_deactivate_activate_all() {
        let mut c = ListConfig::new(10);
        c.add(ListEntry::new("a", "A"));
        c.add(ListEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn list_config_highest_priority() {
        let mut c = ListConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(ListEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn list_config_contains() {
        let mut c = ListConfig::new(10);
        c.add(ListEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn list_config_labels() {
        let mut c = ListConfig::new(10);
        c.add(ListEntry::new("a", "Alpha"));
        c.add(ListEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn list_config_drain_inactive() {
        let mut c = ListConfig::new(10);
        c.add(ListEntry::new("a", "A"));
        c.add(ListEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn qh_metrics_empty() {
        let m = QhMetrics::new("list");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qh_metrics_record_and_mean() {
        let mut m = QhMetrics::new("list");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qh_metrics_min_max() {
        let mut m = QhMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qh_metrics_variance_and_std() {
        let mut m = QhMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn qh_metrics_percentile() {
        let mut m = QhMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qh_metrics_merge() {
        let mut a = QhMetrics::new("a");
        a.record(1.0);
        let mut b = QhMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qh_metrics_reset() {
        let mut m = QhMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qh_rate_window_empty() {
        let rw = QhRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qh_rate_window_tick_and_rate() {
        let mut rw = QhRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qh_lru_cache_basic() {
        let mut c = QhLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qh_lru_cache_contains_and_keys() {
        let mut c = QhLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qh_lru_cache_remove() {
        let mut c = QhLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qh_metrics_sum() {
        let mut m = QhMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qh_metrics_label() {
        let m = QhMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qh_lru_cache_clear() {
        let mut c = QhLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for list
    #[test]
    fn xa_list_ring_new() {
        let rb = super::XaListRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_list_ring_push_len() {
        let mut rb = super::XaListRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_list_ring_wrap() {
        let mut rb = super::XaListRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_list_ring_mean_empty() {
        let rb = super::XaListRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_list_ring_mean_values() {
        let mut rb = super::XaListRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_list_ring_min_max() {
        let mut rb = super::XaListRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_list_ring_iter() {
        let mut rb = super::XaListRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_list_counter_new() {
        let c = super::XaListCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_list_counter_inc() {
        let mut c = super::XaListCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_list_counter_inc_by() {
        let mut c = super::XaListCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_list_counter_reset() {
        let mut c = super::XaListCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_list_counter_clear() {
        let mut c = super::XaListCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_list_counter_default() {
        let c = super::XaListCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 115 ----

    #[test]
    fn xc_115_pool_new_empty() {
        let pool: super::Xc115Pool<i32> = super::Xc115Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_115_pool_release_acquire() {
        let mut pool = super::Xc115Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_115_pool_acquire_empty() {
        let mut pool: super::Xc115Pool<i32> = super::Xc115Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_115_pool_full() {
        let mut pool = super::Xc115Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_115_pool_drain() {
        let mut pool = super::Xc115Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_115_pool_stats() {
        let mut pool = super::Xc115Pool::new(8);
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
    fn xc_115_pool_clear() {
        let mut pool = super::Xc115Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_115_pool_shrink() {
        let mut pool = super::Xc115Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_115_pool_default() {
        let pool: super::Xc115Pool<String> = super::Xc115Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_115_pool_extend() {
        let mut pool = super::Xc115Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_115_pool_retain() {
        let mut pool = super::Xc115Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_115_scheduler_round_robin() {
        let mut sched = super::Xc115Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_115_scheduler_empty() {
        let mut sched = super::Xc115Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_115_scheduler_reset() {
        let mut sched = super::Xc115Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_115_scheduler_add_remove() {
        let mut sched = super::Xc115Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_115_scheduler_targets() {
        let sched = super::Xc115Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_115_hash_empty() {
        assert_eq!(super::xc_115_hash(b""), 5381);
    }

    #[test]
    fn xc_115_hash_data() {
        let h = super::xc_115_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_115_hash(b"hello"), h);
    }

    #[test]
    fn xc_115_reverse_str() {
        assert_eq!(super::xc_115_reverse("abc"), "cba");
        assert_eq!(super::xc_115_reverse(""), "");
    }


    #[test]
    fn xe_8_pipeline_empty() {
        let p = super::Xe8Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_8_pipeline_parse_stage() {
        let p = super::Xe8Pipeline::new()
            .add_parse(super::xe_8_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_8_pipeline_transform_double() {
        let p = super::Xe8Pipeline::new()
            .add_transform(super::xe_8_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_8_pipeline_validate_reverse() {
        let p = super::Xe8Pipeline::new()
            .add_validate(super::xe_8_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_8_pipeline_emit_filter() {
        let p = super::Xe8Pipeline::new()
            .add_emit(super::xe_8_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_8_pipeline_multi_stage() {
        let p = super::Xe8Pipeline::new()
            .add_parse(super::xe_8_pipeline_identity)
            .add_transform(super::xe_8_pipeline_double)
            .add_validate(super::xe_8_pipeline_reverse)
            .add_emit(super::xe_8_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_8_pipeline_error_propagation() {
        let p = super::Xe8Pipeline::new()
            .add_parse(super::xe_8_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe8Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_8_pipeline_compose() {
        let p1 = super::Xe8Pipeline::new()
            .add_parse(super::xe_8_pipeline_identity);
        let p2 = super::Xe8Pipeline::new()
            .add_transform(super::xe_8_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_8_pipeline_error_display() {
        let e = super::Xe8PipelineError {
            stage: super::Xe8Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_8_cache_put_get() {
        let mut c = super::Xe8Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_8_cache_miss() {
        let mut c: super::Xe8Cache<&str, i32> = super::Xe8Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_8_cache_ttl_expiry() {
        let mut c = super::Xe8Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_8_cache_evict() {
        let mut c = super::Xe8Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_8_cache_capacity() {
        let mut c = super::Xe8Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_8_cache_stats() {
        let mut c = super::Xe8Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_8_cache_clear() {
        let mut c = super::Xe8Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #72 --

    #[test]
    fn xf72_trie_insert_search() {
        let mut t = Xf72Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf72_trie_starts_with() {
        let mut t = Xf72Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf72_trie_remove() {
        let mut t = Xf72Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf72_trie_word_count() {
        let mut t = Xf72Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf72_trie_longest_prefix() {
        let mut t = Xf72Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf72_trie_all_words() {
        let mut t = Xf72Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf72_trie_autocomplete() {
        let mut t = Xf72Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf72_trie_empty_search() {
        let t = Xf72Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf72_bloom_add_contains() {
        let mut bf = Xf72BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf72_bloom_probably_absent() {
        let bf = Xf72BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf72_bloom_false_positive_rate() {
        let mut bf = Xf72BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf72_bloom_clear() {
        let mut bf = Xf72BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf72_bloom_union() {
        let mut a = Xf72BloomFilter::xf_new(512, 2);
        let mut b = Xf72BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf72_bloom_intersection_estimate() {
        let mut a = Xf72BloomFilter::xf_new(512, 2);
        let mut b = Xf72BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf72_bloom_union_size_mismatch() {
        let a = Xf72BloomFilter::xf_new(256, 2);
        let b = Xf72BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh114_skip_insert_contains() {
        let mut sl = super::Xh114SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh114_skip_remove() {
        let mut sl = super::Xh114SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh114_skip_len() {
        let mut sl = super::Xh114SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh114_skip_range_query() {
        let mut sl = super::Xh114SkipList::xh_new(4);
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
    fn xh114_skip_floor_ceiling() {
        let mut sl = super::Xh114SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh114_skip_rank() {
        let mut sl = super::Xh114SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh114_skip_empty() {
        let sl = super::Xh114SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh114_skip_duplicates() {
        let mut sl = super::Xh114SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh114_bitset_set_test() {
        let mut bs = super::Xh114BitSet::xh_new(256);
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
    fn xh114_bitset_clear_count() {
        let mut bs = super::Xh114BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh114_bitset_and_or_xor() {
        let mut a = super::Xh114BitSet::xh_new(128);
        let mut b = super::Xh114BitSet::xh_new(128);
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
    fn xh114_bitset_iter_ones() {
        let mut bs = super::Xh114BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh114_bitset_first_last() {
        let mut bs = super::Xh114BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh114_bitset_empty() {
        let bs = super::Xh114BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi114_deque_push_pop_back() {
        let mut dq = super::Xi114Deque::xi_new(4);
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
    fn xi114_deque_push_pop_front() {
        let mut dq = super::Xi114Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi114_deque_mixed_ops() {
        let mut dq = super::Xi114Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi114_deque_get_and_split() {
        let mut dq = super::Xi114Deque::xi_new(8);
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
    fn xi114_deque_rotate_left() {
        let mut dq = super::Xi114Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi114_deque_rotate_right() {
        let mut dq = super::Xi114Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi114_deque_grow() {
        let mut dq = super::Xi114Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi114_deque_empty() {
        let dq = super::Xi114Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi114_interval_tree_insert_query() {
        let mut tree = super::Xi114IntervalTree::xi_new();
        tree.xi_insert(super::Xi114Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi114Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi114Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi114_interval_tree_overlap() {
        let mut tree = super::Xi114IntervalTree::xi_new();
        tree.xi_insert(super::Xi114Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi114Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi114Interval::xi_new(12, 20));
        let q = super::Xi114Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi114_interval_tree_remove() {
        let mut tree = super::Xi114IntervalTree::xi_new();
        tree.xi_insert(super::Xi114Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi114Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi114_interval_tree_gaps() {
        let mut tree = super::Xi114IntervalTree::xi_new();
        tree.xi_insert(super::Xi114Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi114Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi114Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi114Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi114Interval::xi_new(8, 10));
    }

    #[test]
    fn xi114_interval_tree_merge() {
        let mut tree = super::Xi114IntervalTree::xi_new();
        tree.xi_insert(super::Xi114Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi114Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi114Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi114Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi114Interval::xi_new(10, 15));
    }

    #[test]
    fn xi114_interval_tree_all() {
        let mut tree = super::Xi114IntervalTree::xi_new();
        tree.xi_insert(super::Xi114Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi114Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi114_interval_tree_empty() {
        let tree = super::Xi114IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi114_interval_tree_contains_point() {
        let iv = super::Xi114Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 114) ---

    #[test]
    fn xj_114_uf_make_and_find() {
        let mut uf = super::Xj114UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_114_uf_union_connected() {
        let mut uf = super::Xj114UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_114_uf_component_count() {
        let mut uf = super::Xj114UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_114_uf_component_size() {
        let mut uf = super::Xj114UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_114_uf_largest_component() {
        let mut uf = super::Xj114UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_114_uf_many_elements() {
        let mut uf = super::Xj114UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_114_uf_separate_components() {
        let mut uf = super::Xj114UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_114_uf_path_compression() {
        let mut uf = super::Xj114UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_114_bt_insert_get() {
        let mut bt = super::Xj114BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_114_bt_contains_len() {
        let mut bt = super::Xj114BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_114_bt_replace() {
        let mut bt = super::Xj114BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_114_bt_remove() {
        let mut bt = super::Xj114BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_114_bt_keys_values() {
        let mut bt = super::Xj114BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_114_bt_range() {
        let mut bt = super::Xj114BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_114_bt_min_max() {
        let mut bt = super::Xj114BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_114_bt_many_inserts() {
        let mut bt = super::Xj114BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }

}
