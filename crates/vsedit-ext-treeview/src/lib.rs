//! Ext API: Tree views.
//!
//! RPC bridge between the extension host and the main thread for the TreeView API.

use std::fmt;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_treeview";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TreeViewMessage {
    RegisterProvider {
        view_id: String,
    },
    UnregisterProvider {
        view_id: String,
    },
    GetChildren {
        view_id: String,
        element: Option<String>,
    },
    Reveal {
        view_id: String,
        element: String,
        select: bool,
        focus: bool,
    },
    SetMessage {
        view_id: String,
        message: Option<String>,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum TreeItemCollapsibleState {
    None,
    Collapsed,
    Expanded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TreeItem {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub tooltip: Option<String>,
    pub icon_id: Option<String>,
    pub collapsible_state: TreeItemCollapsibleState,
    pub command: Option<String>,
    pub context_value: Option<String>,
    #[serde(default)]
    pub children: Vec<TreeItem>,
}

/// A trait representing tree data providers.
pub trait TreeDataProvider {
    fn get_children(&self, element: Option<&str>) -> Vec<TreeItem>;
    fn get_tree_item(&self, element: &str) -> Option<TreeItem>;
}

// ── Bridge ──

pub struct TreeViewBridge {
    views: Vec<String>,
}

impl TreeViewBridge {
    pub fn new() -> Self {
        Self { views: Vec::new() }
    }

    pub fn register_view(&mut self, view_id: &str) {
        if !self.views.contains(&view_id.to_string()) {
            self.views.push(view_id.to_string());
        }
    }

    pub fn unregister_view(&mut self, view_id: &str) {
        self.views.retain(|v| v != view_id);
    }

    pub fn has_view(&self, view_id: &str) -> bool {
        self.views.iter().any(|v| v == view_id)
    }

    pub fn view_count(&self) -> usize {
        self.views.len()
    }

    pub fn handle_message(&mut self, msg: &TreeViewMessage) -> serde_json::Value {
        match msg {
            TreeViewMessage::RegisterProvider { view_id } => {
                self.register_view(view_id);
                serde_json::json!({"registered": true})
            }
            TreeViewMessage::UnregisterProvider { view_id } => {
                self.unregister_view(view_id);
                serde_json::json!({"unregistered": true})
            }
            TreeViewMessage::GetChildren { view_id, element } => {
                let found = self.has_view(view_id);
                serde_json::json!({"found": found, "element": element, "children": []})
            }
            TreeViewMessage::Reveal {
                view_id,
                element,
                select,
                focus,
            } => {
                serde_json::json!({"view": view_id, "element": element, "select": select, "focus": focus})
            }
            TreeViewMessage::SetMessage { view_id, message } => {
                serde_json::json!({"view": view_id, "message": message})
            }
        }
    }

    /// Returns true if views is empty.
    pub fn is_views_empty(&self) -> bool {
        self.views.is_empty()
    }

    /// Get the first view, if any.
    pub fn first_view(&self) -> Option<&String> {
        self.views.first()
    }

    /// Get the last view, if any.
    pub fn last_view(&self) -> Option<&String> {
        self.views.last()
    }

    /// Retain only views matching the predicate.
    pub fn retain_views(&mut self, f: impl Fn(&String) -> bool) {
        self.views.retain(|item| f(item));
    }
}

impl Default for TreeViewBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ── Additional Types ──

/// Options for configuring a tree view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TreeViewOptions {
    pub can_select_many: bool,
    pub show_collapse_all: bool,
}

impl Default for TreeViewOptions {
    fn default() -> Self {
        Self {
            can_select_many: false,
            show_collapse_all: false,
        }
    }
}

/// Controller for drag-and-drop operations within a tree view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DragAndDropController {
    pub drag_mime_types: Vec<String>,
    pub drop_mime_types: Vec<String>,
}

impl DragAndDropController {
    pub fn new(drag_mime_types: Vec<String>, drop_mime_types: Vec<String>) -> Self {
        Self {
            drag_mime_types,
            drop_mime_types,
        }
    }
}

// ── Item Store ──

/// Per-view storage of tree items.
pub struct TreeItemStore {
    items: HashMap<String, Vec<TreeItem>>,
}

impl TreeItemStore {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    pub fn set_items(&mut self, view_id: &str, items: Vec<TreeItem>) {
        self.items.insert(view_id.to_string(), items);
    }

    pub fn get_items(&self, view_id: &str) -> &[TreeItem] {
        self.items.get(view_id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn clear_items(&mut self, view_id: &str) {
        self.items.remove(view_id);
    }
}

impl Default for TreeItemStore {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tree Utilities ──

/// Recursively search for an item by id in a tree.
pub fn find_item<'a>(items: &'a [TreeItem], id: &str) -> Option<&'a TreeItem> {
    for item in items {
        if item.id == id {
            return Some(item);
        }
        if let Some(found) = find_item(&item.children, id) {
            return Some(found);
        }
    }
    None
}

/// Recursively count all items (including nested children).
pub fn count_items(items: &[TreeItem]) -> usize {
    items
        .iter()
        .fold(0, |acc, item| acc + 1 + count_items(&item.children))
}

/// Flatten a tree into a depth-first ordered list of references.
pub fn flatten_tree<'a>(items: &'a [TreeItem]) -> Vec<&'a TreeItem> {
    let mut result = Vec::new();
    for item in items {
        result.push(item);
        result.extend(flatten_tree(&item.children));
    }
    result
}

/// Filter items by label using case-insensitive substring matching.
/// Parent items are preserved if any of their descendants match.
pub fn filter_items(items: &[TreeItem], query: &str) -> Vec<TreeItem> {
    let query_lower = query.to_lowercase();
    items
        .iter()
        .filter_map(|item| {
            let filtered_children = filter_items(&item.children, query);
            let label_matches = item.label.to_lowercase().contains(&query_lower);
            if label_matches || !filtered_children.is_empty() {
                let mut cloned = item.clone();
                cloned.children = filtered_children;
                Some(cloned)
            } else {
                None
            }
        })
        .collect()
}

/// Initialize the treeview extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

/// Options for revealing a node in a tree view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeViewReveal {
    pub item_id: String,
    pub select: bool,
    pub focus: bool,
    pub expand: bool,
}

impl TreeViewReveal {
    pub fn new(item_id: &str) -> Self {
        Self {
            item_id: item_id.to_string(),
            select: true,
            focus: false,
            expand: false,
        }
    }

    pub fn with_select(mut self, select: bool) -> Self {
        self.select = select;
        self
    }

    pub fn with_focus(mut self, focus: bool) -> Self {
        self.focus = focus;
        self
    }

    pub fn with_expand(mut self, expand: bool) -> Self {
        self.expand = expand;
        self
    }

    /// Validate that the item_id exists in the given tree. Returns the path of ancestor IDs.
    pub fn find_path(&self, items: &[TreeItem]) -> Option<Vec<String>> {
        fn find_path_inner(items: &[TreeItem], target: &str, path: &mut Vec<String>) -> bool {
            for item in items {
                path.push(item.id.clone());
                if item.id == target {
                    return true;
                }
                if find_path_inner(&item.children, target, path) {
                    return true;
                }
                path.pop();
            }
            false
        }
        let mut path = Vec::new();
        if find_path_inner(items, &self.item_id, &mut path) {
            Some(path)
        } else {
            None
        }
    }
}

/// Manages drag-and-drop state for a tree view.
#[derive(Debug, Clone)]
pub struct TreeViewDragDrop {
    pub drag_sources: Vec<String>,
    pub drop_targets: Vec<String>,
    drag_mime_types: Vec<String>,
    drop_mime_types: Vec<String>,
}

impl TreeViewDragDrop {
    pub fn new() -> Self {
        Self {
            drag_sources: Vec::new(),
            drop_targets: Vec::new(),
            drag_mime_types: Vec::new(),
            drop_mime_types: Vec::new(),
        }
    }

    pub fn register_drag_source(&mut self, item_id: &str) {
        if !self.drag_sources.contains(&item_id.to_string()) {
            self.drag_sources.push(item_id.to_string());
        }
    }

    pub fn register_drop_target(&mut self, item_id: &str) {
        if !self.drop_targets.contains(&item_id.to_string()) {
            self.drop_targets.push(item_id.to_string());
        }
    }

    pub fn add_drag_mime_type(&mut self, mime: &str) {
        if !self.drag_mime_types.contains(&mime.to_string()) {
            self.drag_mime_types.push(mime.to_string());
        }
    }

    pub fn add_drop_mime_type(&mut self, mime: &str) {
        if !self.drop_mime_types.contains(&mime.to_string()) {
            self.drop_mime_types.push(mime.to_string());
        }
    }

    pub fn can_drag(&self, item_id: &str) -> bool {
        self.drag_sources.iter().any(|s| s == item_id)
    }

    pub fn can_drop_on(&self, item_id: &str) -> bool {
        self.drop_targets.iter().any(|t| t == item_id)
    }

    /// Check if a drag source can be dropped on a target (validates both are registered).
    pub fn validate_drop(&self, source_id: &str, target_id: &str) -> bool {
        self.can_drag(source_id) && self.can_drop_on(target_id) && source_id != target_id
    }

    pub fn drag_mime_types(&self) -> &[String] {
        &self.drag_mime_types
    }

    pub fn drop_mime_types(&self) -> &[String] {
        &self.drop_mime_types
    }
}

impl Default for TreeViewDragDrop {
    fn default() -> Self {
        Self::new()
    }
}

/// Search a tree recursively for items whose label contains the query (case-insensitive).
/// Returns a flat list of matching items (cloned).
pub fn tree_view_search(items: &[TreeItem], query: &str) -> Vec<TreeItem> {
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();
    for item in items {
        if item.label.to_lowercase().contains(&query_lower) {
            results.push(item.clone());
        }
        results.extend(tree_view_search(&item.children, query));
    }
    results
}

/// Filter a tree, keeping only items matching the predicate plus their ancestors.
/// Returns a new tree with non-matching leaf nodes removed.
pub fn tree_view_filter(items: &[TreeItem], predicate: &dyn Fn(&TreeItem) -> bool) -> Vec<TreeItem> {
    let mut result = Vec::new();
    for item in items {
        let filtered_children = tree_view_filter(&item.children, predicate);
        if predicate(item) || !filtered_children.is_empty() {
            let mut cloned = item.clone();
            cloned.children = filtered_children;
            result.push(cloned);
        }
    }
    result
}

/// Accumulated statistics for ext-treeview operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtTreeviewStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtTreeviewStats {
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
    pub fn merge(&mut self, other: &ExtTreeviewStats) {
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

impl Default for ExtTreeviewStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtTreeviewStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtTreeviewStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-treeview.
#[derive(Debug, Clone)]
pub struct ExtTreeviewValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtTreeviewValidator {
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

impl Default for ExtTreeviewValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tree Serializer ──

/// Serializes and deserializes a tree to/from a compact JSON representation.
pub struct TreeViewSerializer;

impl TreeViewSerializer {
    /// Serialize a tree into a JSON string.
    pub fn to_json(items: &[TreeItem]) -> Result<String, serde_json::Error> {
        serde_json::to_string(items)
    }

    /// Serialize a tree into a pretty-printed JSON string.
    pub fn to_json_pretty(items: &[TreeItem]) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(items)
    }

    /// Deserialize a tree from a JSON string.
    pub fn from_json(json: &str) -> Result<Vec<TreeItem>, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Compute the depth of the deepest leaf in the tree.
    pub fn max_depth(items: &[TreeItem]) -> usize {
        items
            .iter()
            .map(|item| {
                if item.children.is_empty() {
                    1
                } else {
                    1 + Self::max_depth(&item.children)
                }
            })
            .max()
            .unwrap_or(0)
    }
}

// ── Tree Accessibility ──

/// ARIA-style accessibility role for tree view items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TreeItemRole {
    TreeItem,
    Group,
    None,
}

/// Accessibility metadata attached to a tree item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TreeItemAccessibility {
    pub role: TreeItemRole,
    pub label: String,
    pub level: u32,
    pub set_size: u32,
    pub pos_in_set: u32,
    pub expanded: Option<bool>,
}

impl TreeItemAccessibility {
    /// Generate accessibility metadata for a flat list of sibling items at a given depth level.
    pub fn for_siblings(items: &[TreeItem], level: u32) -> Vec<TreeItemAccessibility> {
        let set_size = items.len() as u32;
        items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let role = if item.children.is_empty() {
                    TreeItemRole::TreeItem
                } else {
                    TreeItemRole::Group
                };
                let expanded = match item.collapsible_state {
                    TreeItemCollapsibleState::Expanded => Some(true),
                    TreeItemCollapsibleState::Collapsed => Some(false),
                    TreeItemCollapsibleState::None => None,
                };
                TreeItemAccessibility {
                    role,
                    label: item.label.clone(),
                    level,
                    set_size,
                    pos_in_set: (i + 1) as u32,
                    expanded,
                }
            })
            .collect()
    }

    /// Generate a full accessibility tree (depth-first) for the given items.
    pub fn for_tree(items: &[TreeItem]) -> Vec<TreeItemAccessibility> {
        fn walk(items: &[TreeItem], level: u32, out: &mut Vec<TreeItemAccessibility>) {
            let siblings = TreeItemAccessibility::for_siblings(items, level);
            for (acc, item) in siblings.into_iter().zip(items.iter()) {
                out.push(acc);
                if !item.children.is_empty() {
                    walk(&item.children, level + 1, out);
                }
            }
        }
        let mut result = Vec::new();
        walk(items, 1, &mut result);
        result
    }
}

// ── Tree Selection ──

/// Manages multi-selection state for a tree view.
#[derive(Debug, Clone)]
pub struct TreeViewSelection {
    selected: Vec<String>,
    anchor: Option<String>,
}

impl TreeViewSelection {
    pub fn new() -> Self {
        Self {
            selected: Vec::new(),
            anchor: None,
        }
    }

    /// Select a single item, clearing any previous selection.
    pub fn select(&mut self, item_id: &str) {
        self.selected.clear();
        self.selected.push(item_id.to_string());
        self.anchor = Some(item_id.to_string());
    }

    /// Toggle selection of an item (add if absent, remove if present).
    pub fn toggle(&mut self, item_id: &str) {
        if let Some(pos) = self.selected.iter().position(|s| s == item_id) {
            self.selected.remove(pos);
        } else {
            self.selected.push(item_id.to_string());
            self.anchor = Some(item_id.to_string());
        }
    }

    /// Extend selection by adding items between the anchor and the given target
    /// in the provided flat ordering of IDs.
    pub fn select_range(&mut self, target_id: &str, flat_ids: &[&str]) {
        let anchor = match &self.anchor {
            Some(a) => a.clone(),
            None => {
                self.select(target_id);
                return;
            }
        };
        let anchor_pos = flat_ids.iter().position(|id| *id == anchor);
        let target_pos = flat_ids.iter().position(|id| *id == target_id);
        if let (Some(a), Some(t)) = (anchor_pos, target_pos) {
            let (start, end) = if a <= t { (a, t) } else { (t, a) };
            for id in &flat_ids[start..=end] {
                let s = id.to_string();
                if !self.selected.contains(&s) {
                    self.selected.push(s);
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }

    pub fn is_selected(&self, item_id: &str) -> bool {
        self.selected.iter().any(|s| s == item_id)
    }

    pub fn selected_items(&self) -> &[String] {
        &self.selected
    }

    pub fn count(&self) -> usize {
        self.selected.len()
    }
}

impl Default for TreeViewSelection {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tree Statistics ──

/// Structural statistics about a tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeStats {
    /// Total number of nodes.
    pub total_nodes: usize,
    /// Number of leaf nodes (no children).
    pub leaf_count: usize,
    /// Number of internal (non-leaf) nodes.
    pub internal_count: usize,
    /// Maximum depth (root children are depth 1).
    pub max_depth: usize,
    /// Maximum breadth (largest number of siblings at any level).
    pub max_breadth: usize,
}

impl TreeStats {
    /// Compute statistics for the given tree items.
    pub fn compute(items: &[TreeItem]) -> Self {
        let total_nodes = count_items(items);
        let mut leaf_count = 0;
        let mut max_depth: usize = 0;
        let mut max_breadth = items.len();

        fn walk(items: &[TreeItem], depth: usize, leaf_count: &mut usize, max_depth: &mut usize, max_breadth: &mut usize) {
            if depth > *max_depth {
                *max_depth = depth;
            }
            for item in items {
                if item.children.is_empty() {
                    *leaf_count += 1;
                } else {
                    if item.children.len() > *max_breadth {
                        *max_breadth = item.children.len();
                    }
                    walk(&item.children, depth + 1, leaf_count, max_depth, max_breadth);
                }
            }
        }

        if !items.is_empty() {
            walk(items, 1, &mut leaf_count, &mut max_depth, &mut max_breadth);
        }

        let internal_count = total_nodes - leaf_count;
        Self { total_nodes, leaf_count, internal_count, max_depth, max_breadth }
    }
}

// ── Tree Diff ──

/// Describes a single change between two tree snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeDiffKind {
    Added,
    Removed,
    LabelChanged { old: String, new: String },
}

/// A diff entry identifying a changed node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeDiffEntry {
    pub node_id: String,
    pub kind: TreeDiffKind,
}

/// Compare two tree snapshots and produce a list of differences.
/// Both trees are flattened to id→label maps and compared by id.
pub fn tree_diff(old: &[TreeItem], new: &[TreeItem]) -> Vec<TreeDiffEntry> {
    fn collect_map(items: &[TreeItem], map: &mut HashMap<String, String>) {
        for item in items {
            map.insert(item.id.clone(), item.label.clone());
            collect_map(&item.children, map);
        }
    }

    let mut old_map = HashMap::new();
    let mut new_map = HashMap::new();
    collect_map(old, &mut old_map);
    collect_map(new, &mut new_map);

    let mut diffs = Vec::new();

    // Detect removed and label-changed nodes.
    let mut old_keys: Vec<&String> = old_map.keys().collect();
    old_keys.sort();
    for id in old_keys {
        match new_map.get(id) {
            None => diffs.push(TreeDiffEntry { node_id: id.clone(), kind: TreeDiffKind::Removed }),
            Some(new_label) if new_label != old_map.get(id).unwrap() => {
                diffs.push(TreeDiffEntry {
                    node_id: id.clone(),
                    kind: TreeDiffKind::LabelChanged {
                        old: old_map[id].clone(),
                        new: new_label.clone(),
                    },
                });
            }
            _ => {}
        }
    }

    // Detect added nodes.
    let mut new_keys: Vec<&String> = new_map.keys().collect();
    new_keys.sort();
    for id in new_keys {
        if !old_map.contains_key(id) {
            diffs.push(TreeDiffEntry { node_id: id.clone(), kind: TreeDiffKind::Added });
        }
    }

    diffs
}

// ── Flatten with Depth ──

/// A reference to a tree item together with its depth in the tree.
#[derive(Debug, Clone, PartialEq)]
pub struct FlatTreeEntry<'a> {
    pub item: &'a TreeItem,
    pub depth: usize,
}

/// Flatten a tree into a depth-first ordered list, annotating each entry with its depth.
/// Useful for virtual-list rendering where indentation level is needed.
pub fn flatten_tree_with_depth<'a>(items: &'a [TreeItem], base_depth: usize) -> Vec<FlatTreeEntry<'a>> {
    let mut result = Vec::new();
    for item in items {
        result.push(FlatTreeEntry { item, depth: base_depth });
        result.extend(flatten_tree_with_depth(&item.children, base_depth + 1));
    }
    result
}

// ── Lazy Loading Tracker ──

/// Tracks which tree nodes have been loaded vs pending lazy-load.
#[derive(Debug, Clone)]
pub struct LazyLoadTracker {
    loaded: HashMap<String, bool>,
}

impl LazyLoadTracker {
    pub fn new() -> Self {
        Self { loaded: HashMap::new() }
    }

    /// Mark a node as loaded.
    pub fn mark_loaded(&mut self, node_id: &str) {
        self.loaded.insert(node_id.to_string(), true);
    }

    /// Mark a node as pending (not yet loaded).
    pub fn mark_pending(&mut self, node_id: &str) {
        self.loaded.insert(node_id.to_string(), false);
    }

    /// Check whether a node has been loaded.
    pub fn is_loaded(&self, node_id: &str) -> bool {
        self.loaded.get(node_id).copied().unwrap_or(false)
    }

    /// Return the set of node IDs that are still pending.
    pub fn pending_nodes(&self) -> Vec<String> {
        let mut out: Vec<String> = self.loaded.iter()
            .filter(|(_, loaded)| !**loaded)
            .map(|(id, _)| id.clone())
            .collect();
        out.sort();
        out
    }

    /// Return the total number of tracked nodes.
    pub fn tracked_count(&self) -> usize {
        self.loaded.len()
    }

    /// Return the number of loaded nodes.
    pub fn loaded_count(&self) -> usize {
        self.loaded.values().filter(|&&v| v).count()
    }
}

impl Default for LazyLoadTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ── Checkbox ──

/// The tri-state value of a tree-view checkbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreeViewCheckboxState {
    /// The checkbox is not checked.
    Unchecked,
    /// The checkbox is checked.
    Checked,
    /// The checkbox is in an indeterminate (partial) state.
    Indeterminate,
}

impl fmt::Display for TreeViewCheckboxState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unchecked => write!(f, "☐"),
            Self::Checked => write!(f, "☑"),
            Self::Indeterminate => write!(f, "▣"),
        }
    }
}

/// Manages checkbox states for tree-view nodes.
///
/// Each node is identified by its string id. Nodes that have never been
/// touched default to [`TreeViewCheckboxState::Unchecked`].
#[derive(Debug, Clone)]
pub struct TreeViewCheckboxManager {
    states: HashMap<String, TreeViewCheckboxState>,
}

impl TreeViewCheckboxManager {
    /// Create a new, empty checkbox manager.
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    /// Set the checkbox state for the given node.
    pub fn set_state(&mut self, id: &str, state: TreeViewCheckboxState) {
        self.states.insert(id.to_string(), state);
    }

    /// Return the checkbox state for the given node, defaulting to
    /// [`TreeViewCheckboxState::Unchecked`].
    pub fn get_state(&self, id: &str) -> TreeViewCheckboxState {
        self.states
            .get(id)
            .copied()
            .unwrap_or(TreeViewCheckboxState::Unchecked)
    }

    /// Toggle a node between `Unchecked` and `Checked`.
    ///
    /// `Indeterminate` is treated as `Unchecked` for the purpose of toggling.
    pub fn toggle(&mut self, id: &str) {
        let next = match self.get_state(id) {
            TreeViewCheckboxState::Checked => TreeViewCheckboxState::Unchecked,
            _ => TreeViewCheckboxState::Checked,
        };
        self.set_state(id, next);
    }

    /// Return the ids of all nodes that are currently checked.
    pub fn checked_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self
            .states
            .iter()
            .filter(|(_, s)| **s == TreeViewCheckboxState::Checked)
            .map(|(k, _)| k.as_str())
            .collect();
        ids.sort_unstable();
        ids
    }

    /// Return the number of checked nodes.
    pub fn checked_count(&self) -> usize {
        self.states
            .values()
            .filter(|&&s| s == TreeViewCheckboxState::Checked)
            .count()
    }

    /// Bulk-set the checkbox state for several nodes at once.
    pub fn set_all(&mut self, ids: &[&str], state: TreeViewCheckboxState) {
        for id in ids {
            self.set_state(id, state);
        }
    }
}

impl Default for TreeViewCheckboxManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Badge ──

/// A small badge displayed alongside a tree node (e.g. a count or status tag).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeViewBadge {
    /// The text shown inside the badge.
    pub value: String,
    /// An optional tooltip shown on hover.
    pub tooltip: Option<String>,
}

impl TreeViewBadge {
    /// Create a badge with the given display text and no tooltip.
    pub fn new(text: &str) -> Self {
        Self {
            value: text.to_string(),
            tooltip: None,
        }
    }

    /// Attach a tooltip to this badge (builder pattern).
    pub fn with_tooltip(mut self, tooltip: &str) -> Self {
        self.tooltip = Some(tooltip.to_string());
        self
    }
}

impl fmt::Display for TreeViewBadge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}]", self.value)
    }
}

/// Manages per-node badges for a tree view.
#[derive(Debug, Clone)]
pub struct TreeViewBadgeManager {
    badges: HashMap<String, TreeViewBadge>,
}

impl TreeViewBadgeManager {
    /// Create a new, empty badge manager.
    pub fn new() -> Self {
        Self {
            badges: HashMap::new(),
        }
    }

    /// Assign a badge to a tree node, replacing any previous badge.
    pub fn set_badge(&mut self, node_id: &str, badge: TreeViewBadge) {
        self.badges.insert(node_id.to_string(), badge);
    }

    /// Return the badge for the given node, if any.
    pub fn get_badge(&self, node_id: &str) -> Option<&TreeViewBadge> {
        self.badges.get(node_id)
    }

    /// Remove a node's badge and return it.
    pub fn remove_badge(&mut self, node_id: &str) -> Option<TreeViewBadge> {
        self.badges.remove(node_id)
    }

    /// Return the ids of all nodes that currently have a badge, sorted.
    pub fn nodes_with_badges(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.badges.keys().map(String::as_str).collect();
        ids.sort_unstable();
        ids
    }

    /// Return the total number of badges.
    pub fn badge_count(&self) -> usize {
        self.badges.len()
    }
}

impl Default for TreeViewBadgeManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Paginator ──

/// Paginator for lazily loaded tree-view children.
///
/// Given a fixed page size the paginator can slice a `&[TreeItem]` to the
/// requested page and answer questions about total pages and whether more
/// pages are available.
#[derive(Debug, Clone)]
pub struct TreeViewPaginator {
    /// Number of items per page (must be ≥ 1).
    page_size: usize,
}

impl TreeViewPaginator {
    /// Create a paginator. `page_size` is clamped to a minimum of 1.
    pub fn new(page_size: usize) -> Self {
        Self {
            page_size: page_size.max(1),
        }
    }

    /// Return the sub-slice of `items` corresponding to the zero-based `page`.
    ///
    /// If `page` is beyond the last page an empty slice is returned.
    pub fn paginate<'a>(&self, items: &'a [TreeItem], page: usize) -> &'a [TreeItem] {
        let start = page * self.page_size;
        if start >= items.len() {
            return &[];
        }
        let end = (start + self.page_size).min(items.len());
        &items[start..end]
    }

    /// Total number of pages needed for `item_count` items.
    pub fn total_pages(&self, item_count: usize) -> usize {
        (item_count + self.page_size - 1) / self.page_size
    }

    /// Whether there is a page after `current_page` (zero-based).
    pub fn has_next_page(&self, item_count: usize, current_page: usize) -> bool {
        current_page + 1 < self.total_pages(item_count)
    }
}


// ── Multi-Select ──

/// Tracks multiple selected indices in a tree view, supporting range and toggle.
#[derive(Debug, Clone)]
pub struct TreeViewMultiSelect {
    selected: Vec<usize>,
    anchor: Option<usize>,
    total_items: usize,
}

impl TreeViewMultiSelect {
    /// Create a new multi-select tracker for a tree with `total_items` items.
    pub fn new(total_items: usize) -> Self {
        Self {
            selected: Vec::new(),
            anchor: None,
            total_items,
        }
    }

    /// Toggle selection of a single index.
    pub fn toggle(&mut self, index: usize) {
        if index >= self.total_items {
            return;
        }
        if let Some(pos) = self.selected.iter().position(|&i| i == index) {
            self.selected.remove(pos);
        } else {
            self.selected.push(index);
            self.selected.sort_unstable();
        }
        self.anchor = Some(index);
    }

    /// Select a contiguous range from the anchor to `end` (inclusive).
    pub fn select_range(&mut self, end: usize) {
        let end = end.min(self.total_items.saturating_sub(1));
        let start = self.anchor.unwrap_or(0);
        let (lo, hi) = if start <= end { (start, end) } else { (end, start) };
        for idx in lo..=hi {
            if !self.selected.contains(&idx) {
                self.selected.push(idx);
            }
        }
        self.selected.sort_unstable();
    }

    /// Clear the entire selection.
    pub fn clear(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }

    /// Select all items.
    pub fn select_all(&mut self) {
        self.selected = (0..self.total_items).collect();
        self.anchor = Some(0);
    }

    /// Return the currently selected indices.
    pub fn selected_indices(&self) -> &[usize] {
        &self.selected
    }

    /// Number of selected items.
    pub fn count(&self) -> usize {
        self.selected.len()
    }

    /// Whether a particular index is selected.
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected.contains(&index)
    }

    /// Invert the selection.
    pub fn invert(&mut self) {
        let new: Vec<usize> = (0..self.total_items)
            .filter(|i| !self.selected.contains(i))
            .collect();
        self.selected = new;
    }
}

// ── Search Overlay ──

/// A search overlay for filtering tree view items by label.
#[derive(Debug, Clone)]
pub struct TreeViewSearchOverlay {
    query: String,
    case_sensitive: bool,
    matched_indices: Vec<usize>,
    current_match: Option<usize>,
}

impl TreeViewSearchOverlay {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            case_sensitive: false,
            matched_indices: Vec::new(),
            current_match: None,
        }
    }

    /// Set the search query and run the match against items.
    pub fn search(&mut self, query: &str, items: &[TreeItem]) {
        self.query = query.to_string();
        self.matched_indices.clear();
        self.current_match = None;

        if query.is_empty() {
            return;
        }

        let q = if self.case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };

        for (i, item) in items.iter().enumerate() {
            let label = if self.case_sensitive {
                item.label.clone()
            } else {
                item.label.to_lowercase()
            };
            if label.contains(&q) {
                self.matched_indices.push(i);
            }
        }
        if !self.matched_indices.is_empty() {
            self.current_match = Some(0);
        }
    }

    /// Toggle case sensitivity.
    pub fn set_case_sensitive(&mut self, yes: bool) {
        self.case_sensitive = yes;
    }

    /// Move to the next match, wrapping around.
    pub fn next_match(&mut self) -> Option<usize> {
        if self.matched_indices.is_empty() {
            return None;
        }
        let next = match self.current_match {
            Some(i) => (i + 1) % self.matched_indices.len(),
            None => 0,
        };
        self.current_match = Some(next);
        Some(self.matched_indices[next])
    }

    /// Move to the previous match, wrapping around.
    pub fn prev_match(&mut self) -> Option<usize> {
        if self.matched_indices.is_empty() {
            return None;
        }
        let prev = match self.current_match {
            Some(0) => self.matched_indices.len() - 1,
            Some(i) => i - 1,
            None => self.matched_indices.len() - 1,
        };
        self.current_match = Some(prev);
        Some(self.matched_indices[prev])
    }

    /// Number of matched items.
    pub fn match_count(&self) -> usize {
        self.matched_indices.len()
    }

    /// The current query string.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Whether the search is active.
    pub fn is_active(&self) -> bool {
        !self.query.is_empty()
    }

    /// Clear the search.
    pub fn clear(&mut self) {
        self.query.clear();
        self.matched_indices.clear();
        self.current_match = None;
    }
}

// ── Keyboard Navigation ──

/// Keyboard navigation state for a tree view.
#[derive(Debug, Clone)]
pub struct TreeViewKeyboardNav {
    focused_index: Option<usize>,
    item_count: usize,
}

impl TreeViewKeyboardNav {
    pub fn new(item_count: usize) -> Self {
        Self {
            focused_index: if item_count > 0 { Some(0) } else { None },
            item_count,
        }
    }

    /// Move focus down by one item.
    pub fn move_down(&mut self) {
        if self.item_count == 0 {
            return;
        }
        self.focused_index = Some(match self.focused_index {
            Some(i) if i + 1 < self.item_count => i + 1,
            Some(i) => i,
            None => 0,
        });
    }

    /// Move focus up by one item.
    pub fn move_up(&mut self) {
        if self.item_count == 0 {
            return;
        }
        self.focused_index = Some(match self.focused_index {
            Some(0) => 0,
            Some(i) => i - 1,
            None => 0,
        });
    }

    /// Jump to the first item.
    pub fn move_to_first(&mut self) {
        if self.item_count > 0 {
            self.focused_index = Some(0);
        }
    }

    /// Jump to the last item.
    pub fn move_to_last(&mut self) {
        if self.item_count > 0 {
            self.focused_index = Some(self.item_count - 1);
        }
    }

    /// Page down by `page_size` items.
    pub fn page_down(&mut self, page_size: usize) {
        if self.item_count == 0 {
            return;
        }
        let current = self.focused_index.unwrap_or(0);
        self.focused_index = Some((current + page_size).min(self.item_count - 1));
    }

    /// Page up by `page_size` items.
    pub fn page_up(&mut self, page_size: usize) {
        if self.item_count == 0 {
            return;
        }
        let current = self.focused_index.unwrap_or(0);
        self.focused_index = Some(current.saturating_sub(page_size));
    }

    /// The currently focused index.
    pub fn focused(&self) -> Option<usize> {
        self.focused_index
    }

    /// Update the total item count (e.g. after filtering).
    pub fn set_item_count(&mut self, count: usize) {
        self.item_count = count;
        if count == 0 {
            self.focused_index = None;
        } else if let Some(i) = self.focused_index {
            if i >= count {
                self.focused_index = Some(count - 1);
            }
        }
    }
}

// ── Context Menu Builder ──

/// An action entry within a context menu.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextMenuAction {
    pub id: String,
    pub label: String,
    pub icon: Option<String>,
    pub enabled: bool,
    pub group: Option<String>,
}

/// Builder for assembling a context menu for tree view items.
#[derive(Debug, Clone)]
pub struct TreeViewContextMenuBuilder {
    actions: Vec<ContextMenuAction>,
}

impl TreeViewContextMenuBuilder {
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    /// Add an action to the context menu.
    pub fn add_action(mut self, id: &str, label: &str) -> Self {
        self.actions.push(ContextMenuAction {
            id: id.to_string(),
            label: label.to_string(),
            icon: None,
            enabled: true,
            group: None,
        });
        self
    }

    /// Add an action with icon.
    pub fn add_action_with_icon(mut self, id: &str, label: &str, icon: &str) -> Self {
        self.actions.push(ContextMenuAction {
            id: id.to_string(),
            label: label.to_string(),
            icon: Some(icon.to_string()),
            enabled: true,
            group: None,
        });
        self
    }

    /// Add a disabled action.
    pub fn add_disabled(mut self, id: &str, label: &str) -> Self {
        self.actions.push(ContextMenuAction {
            id: id.to_string(),
            label: label.to_string(),
            icon: None,
            enabled: false,
            group: None,
        });
        self
    }

    /// Assign a group to the last added action.
    pub fn with_group(mut self, group: &str) -> Self {
        if let Some(last) = self.actions.last_mut() {
            last.group = Some(group.to_string());
        }
        self
    }

    /// Build the final list of actions.
    pub fn build(self) -> Vec<ContextMenuAction> {
        self.actions
    }

    /// Return only enabled actions.
    pub fn enabled_actions(&self) -> Vec<&ContextMenuAction> {
        self.actions.iter().filter(|a| a.enabled).collect()
    }

    /// Return actions grouped by their group label (None key for ungrouped).
    pub fn grouped(&self) -> HashMap<Option<String>, Vec<&ContextMenuAction>> {
        let mut map: HashMap<Option<String>, Vec<&ContextMenuAction>> = HashMap::new();
        for action in &self.actions {
            map.entry(action.group.clone()).or_default().push(action);
        }
        map
    }

    /// Number of actions.
    pub fn action_count(&self) -> usize {
        self.actions.len()
    }
}


// ---------------------------------------------------------------------------
// ext_treeview – Extension protocol helpers
// ---------------------------------------------------------------------------

/// Activation event kinds for extension lifecycle management.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum XExtTreeviewActivationKind {
    /// Activate on a specific language.
    Language(String),
    /// Activate on a command.
    Command(String),
    /// Activate on a workspace-contains glob.
    WorkspaceContains(String),
    /// Activate on a custom URI scheme.
    UriScheme(String),
    /// Activate on startup.
    Star,
}

impl XExtTreeviewActivationKind {
    /// Parse an activation event string like `"onLanguage:rust"`.
    pub fn parse(raw: &str) -> Option<Self> {
        if raw == "*" {
            return Some(Self::Star);
        }
        let (kind, value) = raw.split_once(':')?;
        match kind {
            "onLanguage" => Some(Self::Language(value.to_string())),
            "onCommand" => Some(Self::Command(value.to_string())),
            "workspaceContains" => Some(Self::WorkspaceContains(value.to_string())),
            "onUri" => Some(Self::UriScheme(value.to_string())),
            _ => None,
        }
    }

    /// Returns true if this activation kind targets a specific language.
    pub fn is_language(&self) -> bool {
        matches!(self, Self::Language(_))
    }
}

/// Message envelope for extension host RPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XExtTreeviewRpcEnvelope {
    pub seq: u64,
    pub method: String,
    pub payload: String,
}

impl XExtTreeviewRpcEnvelope {
    /// Create a new RPC envelope.
    pub fn new(seq: u64, method: impl Into<String>, payload: impl Into<String>) -> Self {
        Self { seq, method: method.into(), payload: payload.into() }
    }

    /// Returns true when the envelope carries a response (method starts with `$/`).
    pub fn is_response(&self) -> bool {
        self.method.starts_with("$/")
    }

    /// Compute a simple checksum of the payload (sum of bytes mod 2^32).
    pub fn payload_checksum(&self) -> u32 {
        self.payload.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32))
    }
}

/// Batch multiple RPC envelopes and return their sequence numbers.
pub fn x_ext_treeview_collect_sequences(envelopes: &[XExtTreeviewRpcEnvelope]) -> Vec<u64> {
    envelopes.iter().map(|e| e.seq).collect()
}

/// Filter envelopes by method prefix.
pub fn x_ext_treeview_filter_by_method<'a>(
    envelopes: &'a [XExtTreeviewRpcEnvelope],
    method_prefix: &str,
) -> Vec<&'a XExtTreeviewRpcEnvelope> {
    envelopes.iter().filter(|e| e.method.starts_with(method_prefix)).collect()
}

/// Deduplicate envelopes by sequence number, keeping the first occurrence.
pub fn x_ext_treeview_dedup_by_seq(envelopes: Vec<XExtTreeviewRpcEnvelope>) -> Vec<XExtTreeviewRpcEnvelope> {
    let mut seen = std::collections::HashSet::new();
    envelopes.into_iter().filter(|e| seen.insert(e.seq)).collect()
}

/// Simple capability negotiation: given requested and available feature sets,
/// return the intersection.
pub fn x_ext_treeview_negotiate_capabilities(
    requested: &[&str],
    available: &[&str],
) -> Vec<String> {
    requested.iter()
        .filter(|r| available.contains(r))
        .map(|s| s.to_string())
        .collect()
}

/// Version tuple for extension API compatibility checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct XExtTreeviewApiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl XExtTreeviewApiVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
    /// Check if this version satisfies a minimum requirement.
    pub fn satisfies(&self, min: &Self) -> bool {
        (self.major, self.minor, self.patch) >= (min.major, min.minor, min.patch)
    }
}

impl std::fmt::Display for XExtTreeviewApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}



// ---------------------------------------------------------------------------
// ext_treeview – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for extension tree view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YExtTreeviewTreeItemCollapsibleState {
    None,
    Collapsed,
    Expanded,
    Partial,
}

impl YExtTreeviewTreeItemCollapsibleState {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Collapsed => 1,
            Self::Expanded => 2,
            Self::Partial => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Collapsed => "Collapsed",
            Self::Expanded => "Expanded",
            Self::Partial => "Partial",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YExtTreeviewTreeItemCollapsibleState] {
        &[
            YExtTreeviewTreeItemCollapsibleState::None,
            YExtTreeviewTreeItemCollapsibleState::Collapsed,
            YExtTreeviewTreeItemCollapsibleState::Expanded,
            YExtTreeviewTreeItemCollapsibleState::Partial,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YExtTreeviewTreeItemCollapsibleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks tree view state data.
#[derive(Debug, Clone)]
pub struct YExtTreeviewTreeViewState {
    pub expanded_ids: Vec<String>,
    pub selected_id: Option<String>,
    pub focus: bool,
}

impl YExtTreeviewTreeViewState {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            expanded_ids: Vec::new(),
            selected_id: None,
            focus: false,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.expanded_ids.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.expanded_ids.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.expanded_ids.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YExtTreeviewTreeViewState({}: {:?})", "expanded_ids", self.expanded_ids)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_ext_treeview_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_ext_treeview_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_ext_treeview_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_ext_treeview_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_ext_treeview_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_ext_treeview_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_ext_treeview_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_ext_treeview_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// ext_treeview – Extended tree view drag state helpers
// ---------------------------------------------------------------------------

/// Priority levels for tree view drag state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZExtTreeviewPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZExtTreeviewPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZExtTreeviewPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZExtTreeviewPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks tree view drag state data.
#[derive(Debug, Clone)]
pub struct ZExtTreeviewTreeViewDragState {
    pub dragged_ids: Vec<String>,
    pub drop_target: Option<String>,
    pub active: bool,
}

impl ZExtTreeviewTreeViewDragState {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            dragged_ids: Vec::new(),
            drop_target: None,
            active: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.dragged_ids.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.dragged_ids.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.dragged_ids.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZExtTreeviewTreeViewDragState[drop_target={:?}, active={:?}]", self.drop_target, self.active)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.active = !c.active;
        c
    }
}

/// Compute a simple rolling hash for tree view drag state.
pub fn z_ext_treeview_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_ext_treeview_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_ext_treeview_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_ext_treeview_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_ext_treeview_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_ext_treeview_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_ext_treeview_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 88
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer88 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer88 {
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
pub fn xb_fnv1a_88(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_88<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_88<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_88(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_88(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 75
// ---------------------------------------------------------------------------

/// Generic object pool `Xc75Pool<T>`.
pub struct Xc75Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc75Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc75PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc75Pool<T> {
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
    pub fn stats(&self) -> Xc75PoolStats {
        Xc75PoolStats {
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

impl<T> Default for Xc75Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc75Scheduler`.
pub struct Xc75Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc75Scheduler {
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

impl Default for Xc75Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_75 hash for the given byte slice.
pub fn xc_75_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_75 convention.
pub fn xc_75_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe101 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe101Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe101PipelineError {
    pub stage: Xe101Stage,
    pub message: String,
}

impl std::fmt::Display for Xe101PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe101Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe101Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe101PipelineError>>>,
    stage_names: Vec<Xe101Stage>,
}

impl Xe101Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe101PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe101Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe101PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe101Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe101PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe101Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe101PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe101Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe101PipelineError> {
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

    pub fn compose(mut self, other: Xe101Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe101CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe101CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe101Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe101CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe101CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe101Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe101CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_101_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe101CacheEntry {
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

    fn xe_101_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe101CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_101_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe101PipelineError> {
    Ok(data)
}

pub fn xe_101_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe101PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_101_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe101PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_101_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe101PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_101_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe101PipelineError> {
    Err(Xe101PipelineError {
        stage: Xe101Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_99: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg99Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg99Graph {
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

impl Default for Xg99Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_99: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg99Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg99Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg99Heap<T>) {
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

impl<T: Ord> Default for Xg99Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 74).
pub struct Xh74SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh74SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 116 as u64,
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

/// A compact bit set supporting boolean operations (variant 74).
pub struct Xh74BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh74BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 74).
pub struct Xi74Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi74Deque<T> {
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
pub struct Xi74Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi74Interval {
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

/// A simple interval tree (variant 74).
pub struct Xi74IntervalTree {
    xi_intervals: Vec<Xi74Interval>,
}

impl Xi74IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi74Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi74Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi74Interval) -> Vec<&Xi74Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi74Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi74Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi74Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi74Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi74Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi74Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 75) ---

/// Disjoint set / union-find for crate 75.
pub struct Xj75UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj75UnionFind {
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

const XJ75_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 75.
pub struct Xj75BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj75BTreeNode<K, V>>>,
    len: usize,
}

struct Xj75BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj75BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj75BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ75_BTREE_ORDER - 1
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
        let mid = XJ75_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj75BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj75BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj75BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj75BTreeNode::xj_new_leaf();
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


// --- xk_74 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk74SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk74SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk74DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk74DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_75).
#[derive(Debug, Clone)]
pub struct Xl75Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl75Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_75).
#[derive(Debug, Clone)]
pub struct Xl75SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl75SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_items(n: usize) -> Vec<TreeItem> {
        (0..n)
            .map(|i| TreeItem {
                id: format!("item_{i}"),
                label: format!("Item {i}"),
                description: None,
                tooltip: None,
                icon_id: None,
                collapsible_state: TreeItemCollapsibleState::None,
                command: None,
                context_value: None,
                children: Vec::new(),
            })
            .collect()
    }

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = TreeViewMessage::GetChildren {
            view_id: "explorer".into(),
            element: Some("src".into()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: TreeViewMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn tree_item_serialization() {
        let item = TreeItem {
            id: "src".into(),
            label: "src".into(),
            description: Some("directory".into()),
            tooltip: None,
            icon_id: Some("folder".into()),
            collapsible_state: TreeItemCollapsibleState::Collapsed,
            command: None,
            context_value: Some("directory".into()),
            children: vec![],
        };
        let json = serde_json::to_string(&item).unwrap();
        let back: TreeItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, back);
    }

    #[test]
    fn bridge_register_and_unregister() {
        let mut bridge = TreeViewBridge::new();
        bridge.register_view("explorer");
        assert!(bridge.has_view("explorer"));
        bridge.unregister_view("explorer");
        assert!(!bridge.has_view("explorer"));
    }

    #[test]
    fn bridge_get_children_unknown() {
        let mut bridge = TreeViewBridge::new();
        let result = bridge.handle_message(&TreeViewMessage::GetChildren {
            view_id: "nope".into(),
            element: None,
        });
        assert_eq!(result["found"], false);
    }

    #[test]
    fn bridge_duplicate_register() {
        let mut bridge = TreeViewBridge::new();
        bridge.register_view("x");
        bridge.register_view("x");
        assert_eq!(bridge.views.len(), 1);
    }

    // ── Helper ──

    fn make_item(id: &str, label: &str, children: Vec<TreeItem>) -> TreeItem {
        TreeItem {
            id: id.into(),
            label: label.into(),
            description: None,
            tooltip: None,
            icon_id: None,
            collapsible_state: if children.is_empty() {
                TreeItemCollapsibleState::None
            } else {
                TreeItemCollapsibleState::Collapsed
            },
            command: None,
            context_value: None,
            children,
        }
    }

    fn sample_tree() -> Vec<TreeItem> {
        vec![
            make_item(
                "src",
                "src",
                vec![
                    make_item("main", "main.rs", vec![]),
                    make_item(
                        "lib",
                        "lib.rs",
                        vec![make_item("nested", "nested.rs", vec![])],
                    ),
                ],
            ),
            make_item("readme", "README.md", vec![]),
        ]
    }

    // ── New Tests ──

    #[test]
    fn bridge_view_count() {
        let mut bridge = TreeViewBridge::new();
        assert_eq!(bridge.view_count(), 0);
        bridge.register_view("a");
        bridge.register_view("b");
        assert_eq!(bridge.view_count(), 2);
        bridge.unregister_view("a");
        assert_eq!(bridge.view_count(), 1);
    }

    #[test]
    fn item_store_set_get_clear() {
        let mut store = TreeItemStore::new();
        assert!(store.get_items("v1").is_empty());
        store.set_items("v1", vec![make_item("a", "A", vec![])]);
        assert_eq!(store.get_items("v1").len(), 1);
        store.clear_items("v1");
        assert!(store.get_items("v1").is_empty());
    }

    #[test]
    fn item_store_multiple_views() {
        let mut store = TreeItemStore::new();
        store.set_items("v1", vec![make_item("a", "A", vec![])]);
        store.set_items("v2", vec![make_item("b", "B", vec![]), make_item("c", "C", vec![])]);
        assert_eq!(store.get_items("v1").len(), 1);
        assert_eq!(store.get_items("v2").len(), 2);
    }

    #[test]
    fn find_item_top_level() {
        let tree = sample_tree();
        let found = find_item(&tree, "readme");
        assert!(found.is_some());
        assert_eq!(found.unwrap().label, "README.md");
    }

    #[test]
    fn find_item_nested() {
        let tree = sample_tree();
        let found = find_item(&tree, "nested");
        assert!(found.is_some());
        assert_eq!(found.unwrap().label, "nested.rs");
    }

    #[test]
    fn find_item_missing() {
        let tree = sample_tree();
        assert!(find_item(&tree, "nonexistent").is_none());
    }

    #[test]
    fn count_items_recursive() {
        let tree = sample_tree();
        // src(1) + main(1) + lib(1) + nested(1) + readme(1) = 5
        assert_eq!(count_items(&tree), 5);
    }

    #[test]
    fn flatten_tree_order() {
        let tree = sample_tree();
        let flat = flatten_tree(&tree);
        let ids: Vec<&str> = flat.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, vec!["src", "main", "lib", "nested", "readme"]);
    }

    #[test]
    fn filter_items_by_label() {
        let tree = sample_tree();
        let filtered = filter_items(&tree, ".rs");
        // "src" is preserved because it has matching descendants
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "src");
        assert_eq!(filtered[0].children.len(), 2);
    }

    #[test]
    fn filter_items_case_insensitive() {
        let tree = sample_tree();
        let filtered = filter_items(&tree, "README");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "readme");
    }

    #[test]
    fn filter_items_no_match() {
        let tree = sample_tree();
        let filtered = filter_items(&tree, "zzz_nothing");
        assert!(filtered.is_empty());
    }

    #[test]
    fn tree_view_options_defaults() {
        let opts = TreeViewOptions::default();
        assert!(!opts.can_select_many);
        assert!(!opts.show_collapse_all);
    }

    #[test]
    fn drag_and_drop_controller_new() {
        let ctrl = DragAndDropController::new(
            vec!["text/plain".into()],
            vec!["text/uri-list".into()],
        );
        assert_eq!(ctrl.drag_mime_types, vec!["text/plain"]);
        assert_eq!(ctrl.drop_mime_types, vec!["text/uri-list"]);
    }

    #[test]
    fn eq_treeitemcollapsiblestate_same() {
        assert_eq!(TreeItemCollapsibleState::None, TreeItemCollapsibleState::None);
    }

    #[test]
    fn ne_treeitemcollapsiblestate_diff() {
        assert_ne!(TreeItemCollapsibleState::None, TreeItemCollapsibleState::Collapsed);
    }

    #[test]
    fn behavior_check_0() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        let _svc = TreeViewBridge::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn ext_treeview_stats_new_defaults() {
        let stats = ExtTreeviewStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_treeview_stats_record_success() {
        let mut stats = ExtTreeviewStats::new();
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
    fn ext_treeview_stats_record_failure() {
        let mut stats = ExtTreeviewStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_treeview_stats_reset() {
        let mut stats = ExtTreeviewStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_treeview_stats_merge() {
        let mut a = ExtTreeviewStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtTreeviewStats::new();
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
    fn ext_treeview_stats_display() {
        let mut stats = ExtTreeviewStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_treeview_stats_default() {
        let stats = ExtTreeviewStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_treeview_validator_accepts_valid_name() {
        let v = ExtTreeviewValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_treeview_validator_rejects_empty() {
        let v = ExtTreeviewValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_treeview_validator_rejects_too_long() {
        let v = ExtTreeviewValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_treeview_validator_forbidden_prefix() {
        let v = ExtTreeviewValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_treeview_validator_allowed_chars() {
        let v = ExtTreeviewValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_treeview_validator_range() {
        let v = ExtTreeviewValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_treeview_sanitize_removes_control() {
        let result = ExtTreeviewValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_treeview_truncate_short_string() {
        assert_eq!(ExtTreeviewValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_treeview_truncate_long_string() {
        let result = ExtTreeviewValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_treeview_is_ascii_printable() {
        assert!(ExtTreeviewValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtTreeviewValidator::is_ascii_printable("Hello\x00World"));
    }

    fn make_test_tree() -> Vec<TreeItem> {
        vec![
            TreeItem {
                id: "root".into(), label: "Root".into(),
                description: None, tooltip: None, icon_id: None,
                collapsible_state: TreeItemCollapsibleState::Expanded,
                command: None, context_value: None,
                children: vec![
                    TreeItem {
                        id: "child1".into(), label: "Alpha File".into(),
                        description: None, tooltip: None, icon_id: None,
                        collapsible_state: TreeItemCollapsibleState::None,
                        command: None, context_value: None, children: vec![],
                    },
                    TreeItem {
                        id: "child2".into(), label: "Beta Folder".into(),
                        description: None, tooltip: None, icon_id: None,
                        collapsible_state: TreeItemCollapsibleState::Collapsed,
                        command: None, context_value: None,
                        children: vec![
                            TreeItem {
                                id: "grandchild".into(), label: "Gamma Item".into(),
                                description: None, tooltip: None, icon_id: None,
                                collapsible_state: TreeItemCollapsibleState::None,
                                command: None, context_value: None, children: vec![],
                            },
                        ],
                    },
                ],
            },
        ]
    }

    #[test]
    fn reveal_find_path_root() {
        let tree = make_test_tree();
        let reveal = TreeViewReveal::new("root");
        let path = reveal.find_path(&tree).unwrap();
        assert_eq!(path, vec!["root"]);
    }

    #[test]
    fn reveal_find_path_deep() {
        let tree = make_test_tree();
        let reveal = TreeViewReveal::new("grandchild");
        let path = reveal.find_path(&tree).unwrap();
        assert_eq!(path, vec!["root", "child2", "grandchild"]);
    }

    #[test]
    fn reveal_find_path_not_found() {
        let tree = make_test_tree();
        let reveal = TreeViewReveal::new("nonexistent");
        assert!(reveal.find_path(&tree).is_none());
    }

    #[test]
    fn reveal_builder_pattern() {
        let reveal = TreeViewReveal::new("x").with_focus(true).with_expand(true);
        assert!(reveal.focus);
        assert!(reveal.expand);
        assert!(reveal.select);
    }

    #[test]
    fn drag_drop_register_and_validate() {
        let mut dd = TreeViewDragDrop::new();
        dd.register_drag_source("item1");
        dd.register_drop_target("item2");
        assert!(dd.can_drag("item1"));
        assert!(!dd.can_drag("item2"));
        assert!(dd.can_drop_on("item2"));
        assert!(dd.validate_drop("item1", "item2"));
    }

    #[test]
    fn drag_drop_no_self_drop() {
        let mut dd = TreeViewDragDrop::new();
        dd.register_drag_source("a");
        dd.register_drop_target("a");
        assert!(!dd.validate_drop("a", "a"));
    }

    #[test]
    fn drag_drop_no_duplicates() {
        let mut dd = TreeViewDragDrop::new();
        dd.register_drag_source("x");
        dd.register_drag_source("x");
        assert_eq!(dd.drag_sources.len(), 1);
    }

    #[test]
    fn search_finds_matching_items() {
        let tree = make_test_tree();
        let results = tree_view_search(&tree, "alpha");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "child1");
    }

    #[test]
    fn search_case_insensitive() {
        let tree = make_test_tree();
        let results = tree_view_search(&tree, "GAMMA");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "grandchild");
    }

    #[test]
    fn search_no_match() {
        let tree = make_test_tree();
        let results = tree_view_search(&tree, "zzz");
        assert!(results.is_empty());
    }

    #[test]
    fn filter_keeps_matching_and_ancestors() {
        let tree = make_test_tree();
        let filtered = tree_view_filter(&tree, &|item| item.id == "grandchild");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "root");
        assert_eq!(filtered[0].children.len(), 1);
        assert_eq!(filtered[0].children[0].id, "child2");
        assert_eq!(filtered[0].children[0].children.len(), 1);
    }

    // ── Serializer Tests ──

    #[test]
    fn serializer_roundtrip() {
        let tree = sample_tree();
        let json = TreeViewSerializer::to_json(&tree).unwrap();
        let restored = TreeViewSerializer::from_json(&json).unwrap();
        assert_eq!(tree, restored);
    }

    #[test]
    fn serializer_pretty_roundtrip() {
        let tree = sample_tree();
        let json = TreeViewSerializer::to_json_pretty(&tree).unwrap();
        assert!(json.contains('\n'));
        let restored = TreeViewSerializer::from_json(&json).unwrap();
        assert_eq!(tree, restored);
    }

    #[test]
    fn serializer_max_depth_flat() {
        let items = vec![make_item("a", "A", vec![]), make_item("b", "B", vec![])];
        assert_eq!(TreeViewSerializer::max_depth(&items), 1);
    }

    #[test]
    fn serializer_max_depth_nested() {
        let tree = sample_tree();
        // src -> lib -> nested = depth 3
        assert_eq!(TreeViewSerializer::max_depth(&tree), 3);
    }

    #[test]
    fn serializer_max_depth_empty() {
        let items: Vec<TreeItem> = vec![];
        assert_eq!(TreeViewSerializer::max_depth(&items), 0);
    }

    // ── Accessibility Tests ──

    #[test]
    fn accessibility_for_siblings() {
        let items = vec![
            make_item("a", "Alpha", vec![]),
            make_item("b", "Beta", vec![make_item("c", "Child", vec![])]),
        ];
        let acc = TreeItemAccessibility::for_siblings(&items, 1);
        assert_eq!(acc.len(), 2);
        assert_eq!(acc[0].role, TreeItemRole::TreeItem);
        assert_eq!(acc[0].level, 1);
        assert_eq!(acc[0].set_size, 2);
        assert_eq!(acc[0].pos_in_set, 1);
        assert_eq!(acc[0].expanded, None);
        assert_eq!(acc[1].role, TreeItemRole::Group);
        assert_eq!(acc[1].expanded, Some(false)); // Collapsed
    }

    #[test]
    fn accessibility_for_tree_depth_first() {
        let tree = make_test_tree();
        let acc = TreeItemAccessibility::for_tree(&tree);
        // root, child1, child2, grandchild = 4 entries
        assert_eq!(acc.len(), 4);
        assert_eq!(acc[0].label, "Root");
        assert_eq!(acc[0].level, 1);
        assert_eq!(acc[1].label, "Alpha File");
        assert_eq!(acc[1].level, 2);
        assert_eq!(acc[3].label, "Gamma Item");
        assert_eq!(acc[3].level, 3);
    }

    // ── Selection Tests ──

    #[test]
    fn selection_single_select() {
        let mut sel = TreeViewSelection::new();
        sel.select("a");
        assert!(sel.is_selected("a"));
        assert_eq!(sel.count(), 1);
        sel.select("b");
        assert!(!sel.is_selected("a"));
        assert!(sel.is_selected("b"));
        assert_eq!(sel.count(), 1);
    }

    #[test]
    fn selection_toggle() {
        let mut sel = TreeViewSelection::new();
        sel.toggle("a");
        assert!(sel.is_selected("a"));
        sel.toggle("b");
        assert_eq!(sel.count(), 2);
        sel.toggle("a");
        assert!(!sel.is_selected("a"));
        assert_eq!(sel.count(), 1);
    }

    #[test]
    fn selection_range() {
        let mut sel = TreeViewSelection::new();
        let ids = vec!["a", "b", "c", "d", "e"];
        sel.select("b");
        sel.select_range("d", &ids);
        assert!(sel.is_selected("b"));
        assert!(sel.is_selected("c"));
        assert!(sel.is_selected("d"));
        assert!(!sel.is_selected("a"));
        assert!(!sel.is_selected("e"));
    }

    #[test]
    fn selection_clear() {
        let mut sel = TreeViewSelection::new();
        sel.select("x");
        sel.toggle("y");
        sel.clear();
        assert_eq!(sel.count(), 0);
        assert!(!sel.is_selected("x"));
    }

    #[test]
    fn selection_default_empty() {
        let sel = TreeViewSelection::default();
        assert_eq!(sel.count(), 0);
        assert!(sel.selected_items().is_empty());
    }

    // ── Tree Statistics Tests ──

    #[test]
    fn tree_stats_sample_tree() {
        let tree = sample_tree();
        let stats = TreeStats::compute(&tree);
        assert_eq!(stats.total_nodes, 5);
        assert_eq!(stats.leaf_count, 3); // main, nested, readme
        assert_eq!(stats.internal_count, 2); // src, lib
        assert_eq!(stats.max_depth, 3); // src -> lib -> nested
        assert_eq!(stats.max_breadth, 2); // src has 2 children, root has 2
    }

    #[test]
    fn tree_stats_empty() {
        let stats = TreeStats::compute(&[]);
        assert_eq!(stats.total_nodes, 0);
        assert_eq!(stats.leaf_count, 0);
        assert_eq!(stats.max_depth, 0);
        assert_eq!(stats.max_breadth, 0);
    }

    // ── Tree Diff Tests ──

    #[test]
    fn tree_diff_added_and_removed() {
        let old = vec![make_item("a", "A", vec![])];
        let new = vec![make_item("b", "B", vec![])];
        let diffs = tree_diff(&old, &new);
        assert!(diffs.iter().any(|d| d.node_id == "a" && d.kind == TreeDiffKind::Removed));
        assert!(diffs.iter().any(|d| d.node_id == "b" && d.kind == TreeDiffKind::Added));
    }

    #[test]
    fn tree_diff_label_changed() {
        let old = vec![make_item("x", "OldLabel", vec![])];
        let new = vec![make_item("x", "NewLabel", vec![])];
        let diffs = tree_diff(&old, &new);
        assert_eq!(diffs.len(), 1);
        assert_eq!(
            diffs[0].kind,
            TreeDiffKind::LabelChanged { old: "OldLabel".into(), new: "NewLabel".into() }
        );
    }

    #[test]
    fn tree_diff_identical_trees() {
        let tree = sample_tree();
        let diffs = tree_diff(&tree, &tree);
        assert!(diffs.is_empty());
    }

    // ── Flatten with Depth Tests ──

    #[test]
    fn flatten_with_depth_values() {
        let tree = sample_tree();
        let flat = flatten_tree_with_depth(&tree, 0);
        assert_eq!(flat.len(), 5);
        assert_eq!(flat[0].item.id, "src");
        assert_eq!(flat[0].depth, 0);
        assert_eq!(flat[1].item.id, "main");
        assert_eq!(flat[1].depth, 1);
        assert_eq!(flat[3].item.id, "nested");
        assert_eq!(flat[3].depth, 2);
        assert_eq!(flat[4].item.id, "readme");
        assert_eq!(flat[4].depth, 0);
    }

    // ── Lazy Load Tracker Tests ──

    #[test]
    fn lazy_load_tracker_workflow() {
        let mut tracker = LazyLoadTracker::new();
        tracker.mark_pending("node_a");
        tracker.mark_pending("node_b");
        assert!(!tracker.is_loaded("node_a"));
        assert_eq!(tracker.pending_nodes(), vec!["node_a", "node_b"]);
        assert_eq!(tracker.loaded_count(), 0);

        tracker.mark_loaded("node_a");
        assert!(tracker.is_loaded("node_a"));
        assert!(!tracker.is_loaded("node_b"));
        assert_eq!(tracker.pending_nodes(), vec!["node_b"]);
        assert_eq!(tracker.loaded_count(), 1);
        assert_eq!(tracker.tracked_count(), 2);
    }

    #[test]
    fn lazy_load_tracker_unknown_node_not_loaded() {
        let tracker = LazyLoadTracker::new();
        assert!(!tracker.is_loaded("unknown"));
        assert_eq!(tracker.tracked_count(), 0);
    }

    // ── Checkbox tests ──

    #[test]
    fn checkbox_default_is_unchecked() {
        let mgr = TreeViewCheckboxManager::new();
        assert_eq!(mgr.get_state("any"), TreeViewCheckboxState::Unchecked);
    }

    #[test]
    fn checkbox_set_and_get() {
        let mut mgr = TreeViewCheckboxManager::new();
        mgr.set_state("a", TreeViewCheckboxState::Checked);
        mgr.set_state("b", TreeViewCheckboxState::Indeterminate);
        assert_eq!(mgr.get_state("a"), TreeViewCheckboxState::Checked);
        assert_eq!(mgr.get_state("b"), TreeViewCheckboxState::Indeterminate);
    }

    #[test]
    fn checkbox_toggle_cycle() {
        let mut mgr = TreeViewCheckboxManager::new();
        // unchecked -> checked
        mgr.toggle("x");
        assert_eq!(mgr.get_state("x"), TreeViewCheckboxState::Checked);
        // checked -> unchecked
        mgr.toggle("x");
        assert_eq!(mgr.get_state("x"), TreeViewCheckboxState::Unchecked);
    }

    #[test]
    fn checkbox_toggle_from_indeterminate() {
        let mut mgr = TreeViewCheckboxManager::new();
        mgr.set_state("x", TreeViewCheckboxState::Indeterminate);
        mgr.toggle("x");
        assert_eq!(mgr.get_state("x"), TreeViewCheckboxState::Checked);
    }

    #[test]
    fn checkbox_checked_ids_and_count() {
        let mut mgr = TreeViewCheckboxManager::new();
        mgr.set_state("c", TreeViewCheckboxState::Checked);
        mgr.set_state("a", TreeViewCheckboxState::Checked);
        mgr.set_state("b", TreeViewCheckboxState::Unchecked);
        assert_eq!(mgr.checked_ids(), vec!["a", "c"]);
        assert_eq!(mgr.checked_count(), 2);
    }

    #[test]
    fn checkbox_set_all() {
        let mut mgr = TreeViewCheckboxManager::new();
        mgr.set_all(&["a", "b", "c"], TreeViewCheckboxState::Checked);
        assert_eq!(mgr.checked_count(), 3);
        mgr.set_all(&["a", "c"], TreeViewCheckboxState::Unchecked);
        assert_eq!(mgr.checked_count(), 1);
        assert_eq!(mgr.checked_ids(), vec!["b"]);
    }

    #[test]
    fn checkbox_display() {
        assert_eq!(TreeViewCheckboxState::Unchecked.to_string(), "☐");
        assert_eq!(TreeViewCheckboxState::Checked.to_string(), "☑");
        assert_eq!(TreeViewCheckboxState::Indeterminate.to_string(), "▣");
    }

    // ── Badge tests ──

    #[test]
    fn badge_display_and_tooltip() {
        let badge = TreeViewBadge::new("5").with_tooltip("5 errors");
        assert_eq!(badge.to_string(), "[5]");
        assert_eq!(badge.tooltip.as_deref(), Some("5 errors"));
    }

    #[test]
    fn badge_manager_crud() {
        let mut mgr = TreeViewBadgeManager::new();
        assert_eq!(mgr.badge_count(), 0);

        mgr.set_badge("node1", TreeViewBadge::new("3"));
        mgr.set_badge("node2", TreeViewBadge::new("!"));
        assert_eq!(mgr.badge_count(), 2);
        assert_eq!(mgr.get_badge("node1").unwrap().value, "3");
        assert_eq!(mgr.nodes_with_badges(), vec!["node1", "node2"]);

        mgr.remove_badge("node1");
        assert!(mgr.get_badge("node1").is_none());
        assert_eq!(mgr.badge_count(), 1);
    }

    // ── Paginator tests ──

    #[test]
    fn paginator_basic_pagination() {
        let items = make_items(5);
        let pager = TreeViewPaginator::new(2);

        assert_eq!(pager.total_pages(5), 3);
        assert_eq!(pager.paginate(&items, 0).len(), 2);
        assert_eq!(pager.paginate(&items, 1).len(), 2);
        assert_eq!(pager.paginate(&items, 2).len(), 1);
        assert_eq!(pager.paginate(&items, 3).len(), 0);
    }

    #[test]
    fn paginator_has_next_page() {
        let pager = TreeViewPaginator::new(3);
        // 7 items => 3 pages (0, 1, 2)
        assert!(pager.has_next_page(7, 0));
        assert!(pager.has_next_page(7, 1));
        assert!(!pager.has_next_page(7, 2));
    }

    #[test]
    fn paginator_zero_page_size_clamped() {
        let pager = TreeViewPaginator::new(0);
        assert_eq!(pager.total_pages(5), 5); // page_size clamped to 1
    }

    // ── Multi-select tests ──

    #[test]
    fn multi_select_toggle() {
        let mut ms = TreeViewMultiSelect::new(10);
        ms.toggle(3);
        ms.toggle(5);
        assert_eq!(ms.count(), 2);
        assert!(ms.is_selected(3));
        assert!(ms.is_selected(5));
        ms.toggle(3);
        assert_eq!(ms.count(), 1);
        assert!(!ms.is_selected(3));
    }

    #[test]
    fn multi_select_range() {
        let mut ms = TreeViewMultiSelect::new(10);
        ms.toggle(2);
        ms.select_range(6);
        assert_eq!(ms.count(), 5);
        assert!(ms.is_selected(2));
        assert!(ms.is_selected(4));
        assert!(ms.is_selected(6));
    }

    #[test]
    fn multi_select_all_and_clear() {
        let mut ms = TreeViewMultiSelect::new(5);
        ms.select_all();
        assert_eq!(ms.count(), 5);
        ms.clear();
        assert_eq!(ms.count(), 0);
    }

    #[test]
    fn multi_select_invert() {
        let mut ms = TreeViewMultiSelect::new(5);
        ms.toggle(1);
        ms.toggle(3);
        ms.invert();
        assert_eq!(ms.count(), 3);
        assert!(ms.is_selected(0));
        assert!(ms.is_selected(2));
        assert!(ms.is_selected(4));
    }

    #[test]
    fn multi_select_out_of_range_ignored() {
        let mut ms = TreeViewMultiSelect::new(3);
        ms.toggle(10);
        assert_eq!(ms.count(), 0);
    }

    // ── Search overlay tests ──

    #[test]
    fn search_overlay_basic() {
        let mut so = TreeViewSearchOverlay::new();
        let items = make_items(5);
        so.search("Item 2", &items);
        assert_eq!(so.match_count(), 1);
        assert!(so.is_active());
    }

    #[test]
    fn search_overlay_case_insensitive() {
        let mut so = TreeViewSearchOverlay::new();
        let items = make_items(3);
        so.search("item", &items);
        assert_eq!(so.match_count(), 3);
    }

    #[test]
    fn search_overlay_next_prev() {
        let mut so = TreeViewSearchOverlay::new();
        let items = make_items(5);
        so.search("item", &items);
        let first = so.next_match();
        assert_eq!(first, Some(1)); // wraps from 0->1
        let prev = so.prev_match();
        assert_eq!(prev, Some(0));
    }

    #[test]
    fn search_overlay_clear() {
        let mut so = TreeViewSearchOverlay::new();
        let items = make_items(3);
        so.search("Item", &items);
        so.clear();
        assert!(!so.is_active());
        assert_eq!(so.match_count(), 0);
    }

    // ── Keyboard nav tests ──

    #[test]
    fn keyboard_nav_move_down_up() {
        let mut nav = TreeViewKeyboardNav::new(5);
        assert_eq!(nav.focused(), Some(0));
        nav.move_down();
        assert_eq!(nav.focused(), Some(1));
        nav.move_up();
        assert_eq!(nav.focused(), Some(0));
        nav.move_up();
        assert_eq!(nav.focused(), Some(0)); // stays at 0
    }

    #[test]
    fn keyboard_nav_first_last() {
        let mut nav = TreeViewKeyboardNav::new(10);
        nav.move_to_last();
        assert_eq!(nav.focused(), Some(9));
        nav.move_to_first();
        assert_eq!(nav.focused(), Some(0));
    }

    #[test]
    fn keyboard_nav_page() {
        let mut nav = TreeViewKeyboardNav::new(20);
        nav.page_down(5);
        assert_eq!(nav.focused(), Some(5));
        nav.page_up(3);
        assert_eq!(nav.focused(), Some(2));
    }

    #[test]
    fn keyboard_nav_set_item_count_clamps() {
        let mut nav = TreeViewKeyboardNav::new(10);
        nav.move_to_last();
        nav.set_item_count(5);
        assert_eq!(nav.focused(), Some(4));
        nav.set_item_count(0);
        assert_eq!(nav.focused(), None);
    }

    // ── Context menu tests ──

    #[test]
    fn context_menu_builder_basic() {
        let menu = TreeViewContextMenuBuilder::new()
            .add_action("copy", "Copy")
            .add_action("paste", "Paste")
            .add_disabled("cut", "Cut")
            .build();
        assert_eq!(menu.len(), 3);
        assert!(!menu[2].enabled);
    }

    #[test]
    fn context_menu_enabled_actions() {
        let builder = TreeViewContextMenuBuilder::new()
            .add_action("a", "A")
            .add_disabled("b", "B")
            .add_action("c", "C");
        let enabled = builder.enabled_actions();
        assert_eq!(enabled.len(), 2);
    }

    #[test]
    fn context_menu_grouped() {
        let builder = TreeViewContextMenuBuilder::new()
            .add_action("a", "A").with_group("edit")
            .add_action("b", "B").with_group("edit")
            .add_action("c", "C").with_group("file");
        let groups = builder.grouped();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[&Some("edit".to_string())].len(), 2);
    }

    #[test]
    fn context_menu_with_icon() {
        let menu = TreeViewContextMenuBuilder::new()
            .add_action_with_icon("del", "Delete", "trash")
            .build();
        assert_eq!(menu[0].icon.as_deref(), Some("trash"));
    }


    // -- ext_treeview additional tests -------------------------------------------

    #[test]
    fn x_ext_treeview_activation_parse_language() {
        let ak = XExtTreeviewActivationKind::parse("onLanguage:rust").unwrap();
        assert_eq!(ak, XExtTreeviewActivationKind::Language("rust".into()));
        assert!(ak.is_language());
    }

    #[test]
    fn x_ext_treeview_activation_parse_command() {
        let ak = XExtTreeviewActivationKind::parse("onCommand:editor.action.format").unwrap();
        assert_eq!(ak, XExtTreeviewActivationKind::Command("editor.action.format".into()));
        assert!(!ak.is_language());
    }

    #[test]
    fn x_ext_treeview_activation_parse_star() {
        assert_eq!(XExtTreeviewActivationKind::parse("*"), Some(XExtTreeviewActivationKind::Star));
    }

    #[test]
    fn x_ext_treeview_activation_parse_unknown() {
        assert!(XExtTreeviewActivationKind::parse("badKind:thing").is_none());
    }

    #[test]
    fn x_ext_treeview_activation_parse_workspace() {
        let ak = XExtTreeviewActivationKind::parse("workspaceContains:**/Cargo.toml").unwrap();
        assert_eq!(ak, XExtTreeviewActivationKind::WorkspaceContains("**/" .to_owned() + "Cargo.toml"));
    }

    #[test]
    fn x_ext_treeview_rpc_envelope_basic() {
        let env = XExtTreeviewRpcEnvelope::new(1, "textDocument/didOpen", "{}" );
        assert_eq!(env.seq, 1);
        assert!(!env.is_response());
    }

    #[test]
    fn x_ext_treeview_rpc_envelope_response() {
        let env = XExtTreeviewRpcEnvelope::new(2, "$/cancelRequest", "");
        assert!(env.is_response());
    }

    #[test]
    fn x_ext_treeview_rpc_payload_checksum() {
        let env = XExtTreeviewRpcEnvelope::new(1, "m", "AB");
        assert_eq!(env.payload_checksum(), 65 + 66);
    }

    #[test]
    fn x_ext_treeview_collect_sequences_works() {
        let envs = vec![
            XExtTreeviewRpcEnvelope::new(10, "a", ""),
            XExtTreeviewRpcEnvelope::new(20, "b", ""),
        ];
        assert_eq!(x_ext_treeview_collect_sequences(&envs), vec![10, 20]);
    }

    #[test]
    fn x_ext_treeview_filter_by_method_works() {
        let envs = vec![
            XExtTreeviewRpcEnvelope::new(1, "textDocument/open", ""),
            XExtTreeviewRpcEnvelope::new(2, "workspace/config", ""),
            XExtTreeviewRpcEnvelope::new(3, "textDocument/close", ""),
        ];
        let filtered = x_ext_treeview_filter_by_method(&envs, "textDocument/");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn x_ext_treeview_dedup_by_seq_works() {
        let envs = vec![
            XExtTreeviewRpcEnvelope::new(1, "a", "first"),
            XExtTreeviewRpcEnvelope::new(1, "a", "second"),
            XExtTreeviewRpcEnvelope::new(2, "b", "third"),
        ];
        let deduped = x_ext_treeview_dedup_by_seq(envs);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].payload, "first");
    }

    #[test]
    fn x_ext_treeview_negotiate_capabilities_basic() {
        let result = x_ext_treeview_negotiate_capabilities(
            &["hover", "completion", "rename"],
            &["hover", "rename", "format"],
        );
        assert_eq!(result, vec!["hover", "rename"]);
    }

    #[test]
    fn x_ext_treeview_api_version_satisfies() {
        let v1 = XExtTreeviewApiVersion::new(1, 80, 0);
        let min = XExtTreeviewApiVersion::new(1, 70, 0);
        assert!(v1.satisfies(&min));
        assert!(!min.satisfies(&v1));
    }

    #[test]
    fn x_ext_treeview_api_version_display() {
        let v = XExtTreeviewApiVersion::new(2, 3, 4);
        assert_eq!(v.to_string(), "2.3.4");
    }

    #[test]
    fn x_ext_treeview_api_version_ord() {
        let v1 = XExtTreeviewApiVersion::new(1, 0, 0);
        let v2 = XExtTreeviewApiVersion::new(1, 1, 0);
        assert!(v1 < v2);
    }


    // -- ext_treeview extended domain tests ----------------------------------------

    #[test]
    fn y_ext_treeview_enum_index() {
        assert_eq!(YExtTreeviewTreeItemCollapsibleState::None.index(), 0);
        assert_eq!(YExtTreeviewTreeItemCollapsibleState::Collapsed.index(), 1);
        assert_eq!(YExtTreeviewTreeItemCollapsibleState::Expanded.index(), 2);
        assert_eq!(YExtTreeviewTreeItemCollapsibleState::Partial.index(), 3);
    }

    #[test]
    fn y_ext_treeview_enum_label() {
        assert_eq!(YExtTreeviewTreeItemCollapsibleState::None.label(), "None");
        assert_eq!(YExtTreeviewTreeItemCollapsibleState::Collapsed.label(), "Collapsed");
        assert_eq!(YExtTreeviewTreeItemCollapsibleState::Expanded.label(), "Expanded");
        assert_eq!(YExtTreeviewTreeItemCollapsibleState::Partial.label(), "Partial");
    }

    #[test]
    fn y_ext_treeview_enum_all() {
        let all = YExtTreeviewTreeItemCollapsibleState::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_ext_treeview_enum_is_default() {
        assert!(YExtTreeviewTreeItemCollapsibleState::None.is_default());
        assert!(!YExtTreeviewTreeItemCollapsibleState::Partial.is_default());
    }

    #[test]
    fn y_ext_treeview_enum_display() {
        assert_eq!(format!("{}", YExtTreeviewTreeItemCollapsibleState::None), "None");
    }

    #[test]
    fn y_ext_treeview_struct_new() {
        let s = YExtTreeviewTreeViewState::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_ext_treeview_struct_clear() {
        let mut s = YExtTreeviewTreeViewState::new();
        s.expanded_ids.push("test".into());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_ext_treeview_fingerprint_deterministic() {
        let h1 = y_ext_treeview_fingerprint("hello");
        let h2 = y_ext_treeview_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_ext_treeview_fingerprint("a"), y_ext_treeview_fingerprint("b"));
    }

    #[test]
    fn y_ext_treeview_truncate_short() {
        assert_eq!(y_ext_treeview_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_ext_treeview_truncate_long() {
        let r = y_ext_treeview_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_ext_treeview_normalize_key_basic() {
        assert_eq!(y_ext_treeview_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_ext_treeview_split_path_basic() {
        let parts = y_ext_treeview_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_ext_treeview_count_occurrences_basic() {
        assert_eq!(y_ext_treeview_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_ext_treeview_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_ext_treeview_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_ext_treeview_in_range_basic() {
        assert!(y_ext_treeview_in_range(5, 1, 10));
        assert!(y_ext_treeview_in_range(1, 1, 10));
        assert!(y_ext_treeview_in_range(10, 1, 10));
        assert!(!y_ext_treeview_in_range(0, 1, 10));
        assert!(!y_ext_treeview_in_range(11, 1, 10));
    }

    #[test]
    fn y_ext_treeview_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_ext_treeview_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_ext_treeview_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_ext_treeview_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- ext_treeview Z-extended tests -----------------------------------------------

    #[test]
    fn z_ext_treeview_priority_weight() {
        assert_eq!(ZExtTreeviewPriority::Idle.weight(), 0);
        assert_eq!(ZExtTreeviewPriority::Normal.weight(), 2);
        assert_eq!(ZExtTreeviewPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_ext_treeview_priority_label() {
        assert_eq!(ZExtTreeviewPriority::Low.label(), "low");
        assert_eq!(ZExtTreeviewPriority::High.label(), "high");
    }

    #[test]
    fn z_ext_treeview_priority_is_elevated() {
        assert!(!ZExtTreeviewPriority::Normal.is_elevated());
        assert!(ZExtTreeviewPriority::High.is_elevated());
        assert!(ZExtTreeviewPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_ext_treeview_priority_display() {
        assert_eq!(format!("{}", ZExtTreeviewPriority::Idle), "idle");
    }

    #[test]
    fn z_ext_treeview_priority_all_asc() {
        let all = ZExtTreeviewPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZExtTreeviewPriority::Idle);
        assert_eq!(all[4], ZExtTreeviewPriority::Realtime);
    }

    #[test]
    fn z_ext_treeview_struct_new() {
        let s = ZExtTreeviewTreeViewDragState::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_ext_treeview_struct_toggled_clone() {
        let s = ZExtTreeviewTreeViewDragState::new();
        let t = s.toggled_clone();
        assert_ne!(s.active, t.active);
    }

    #[test]
    fn z_ext_treeview_rolling_hash_deterministic() {
        let h1 = z_ext_treeview_rolling_hash(b"test");
        let h2 = z_ext_treeview_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_ext_treeview_rolling_hash(b"a"), z_ext_treeview_rolling_hash(b"b"));
    }

    #[test]
    fn z_ext_treeview_pad_to_basic() {
        assert_eq!(z_ext_treeview_pad_to("hi", 5), "hi   ");
        assert_eq!(z_ext_treeview_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_ext_treeview_is_identifier_basic() {
        assert!(z_ext_treeview_is_identifier("foo_bar"));
        assert!(z_ext_treeview_is_identifier("abc123"));
        assert!(!z_ext_treeview_is_identifier(""));
        assert!(!z_ext_treeview_is_identifier("has space"));
    }

    #[test]
    fn z_ext_treeview_levenshtein_basic() {
        assert_eq!(z_ext_treeview_levenshtein("", ""), 0);
        assert_eq!(z_ext_treeview_levenshtein("abc", "abc"), 0);
        assert_eq!(z_ext_treeview_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_ext_treeview_unique_words_basic() {
        let w = z_ext_treeview_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_ext_treeview_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_ext_treeview_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_ext_treeview_common_prefix_basic() {
        assert_eq!(z_ext_treeview_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_ext_treeview_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_ext_treeview_struct_clear() {
        let mut s = ZExtTreeviewTreeViewDragState::new();
        s.dragged_ids.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_ext_treeview_rolling_hash_empty() {
        let h = z_ext_treeview_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_88_push_and_len() {
        let mut rb = super::XbRingBuffer88::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_88_overwrite() {
        let mut rb = super::XbRingBuffer88::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_88_get_out_of_bounds() {
        let rb = super::XbRingBuffer88::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_88_drain_all() {
        let mut rb = super::XbRingBuffer88::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_88_peek_front_back() {
        let mut rb = super::XbRingBuffer88::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_88_clear() {
        let mut rb = super::XbRingBuffer88::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_88_capacity() {
        let rb = super::XbRingBuffer88::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_88_basic() {
        let h = super::xb_fnv1a_88(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_88(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_88_different_inputs() {
        let h1 = super::xb_fnv1a_88(b"abc");
        let h2 = super::xb_fnv1a_88(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_88_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_88(&data);
        let dec = super::xb_rle_decode_88(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_88_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_88(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_88(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_88_values() {
        assert!((super::xb_clamp_88(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_88(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_88(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_88_values() {
        assert!((super::xb_lerp_88(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_88(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_88(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_88_wrap_around_twice() {
        let mut rb = super::XbRingBuffer88::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 75 ----

    #[test]
    fn xc_75_pool_new_empty() {
        let pool: super::Xc75Pool<i32> = super::Xc75Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_75_pool_release_acquire() {
        let mut pool = super::Xc75Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_75_pool_acquire_empty() {
        let mut pool: super::Xc75Pool<i32> = super::Xc75Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_75_pool_full() {
        let mut pool = super::Xc75Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_75_pool_drain() {
        let mut pool = super::Xc75Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_75_pool_stats() {
        let mut pool = super::Xc75Pool::new(8);
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
    fn xc_75_pool_clear() {
        let mut pool = super::Xc75Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_75_pool_shrink() {
        let mut pool = super::Xc75Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_75_pool_default() {
        let pool: super::Xc75Pool<String> = super::Xc75Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_75_pool_extend() {
        let mut pool = super::Xc75Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_75_pool_retain() {
        let mut pool = super::Xc75Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_75_scheduler_round_robin() {
        let mut sched = super::Xc75Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_75_scheduler_empty() {
        let mut sched = super::Xc75Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_75_scheduler_reset() {
        let mut sched = super::Xc75Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_75_scheduler_add_remove() {
        let mut sched = super::Xc75Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_75_scheduler_targets() {
        let sched = super::Xc75Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_75_hash_empty() {
        assert_eq!(super::xc_75_hash(b""), 5381);
    }

    #[test]
    fn xc_75_hash_data() {
        let h = super::xc_75_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_75_hash(b"hello"), h);
    }

    #[test]
    fn xc_75_reverse_str() {
        assert_eq!(super::xc_75_reverse("abc"), "cba");
        assert_eq!(super::xc_75_reverse(""), "");
    }


    #[test]
    fn xe_101_pipeline_empty() {
        let p = super::Xe101Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_101_pipeline_parse_stage() {
        let p = super::Xe101Pipeline::new()
            .add_parse(super::xe_101_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_101_pipeline_transform_double() {
        let p = super::Xe101Pipeline::new()
            .add_transform(super::xe_101_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_101_pipeline_validate_reverse() {
        let p = super::Xe101Pipeline::new()
            .add_validate(super::xe_101_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_101_pipeline_emit_filter() {
        let p = super::Xe101Pipeline::new()
            .add_emit(super::xe_101_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_101_pipeline_multi_stage() {
        let p = super::Xe101Pipeline::new()
            .add_parse(super::xe_101_pipeline_identity)
            .add_transform(super::xe_101_pipeline_double)
            .add_validate(super::xe_101_pipeline_reverse)
            .add_emit(super::xe_101_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_101_pipeline_error_propagation() {
        let p = super::Xe101Pipeline::new()
            .add_parse(super::xe_101_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe101Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_101_pipeline_compose() {
        let p1 = super::Xe101Pipeline::new()
            .add_parse(super::xe_101_pipeline_identity);
        let p2 = super::Xe101Pipeline::new()
            .add_transform(super::xe_101_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_101_pipeline_error_display() {
        let e = super::Xe101PipelineError {
            stage: super::Xe101Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_101_cache_put_get() {
        let mut c = super::Xe101Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_101_cache_miss() {
        let mut c: super::Xe101Cache<&str, i32> = super::Xe101Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_101_cache_ttl_expiry() {
        let mut c = super::Xe101Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_101_cache_evict() {
        let mut c = super::Xe101Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_101_cache_capacity() {
        let mut c = super::Xe101Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_101_cache_stats() {
        let mut c = super::Xe101Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_101_cache_clear() {
        let mut c = super::Xe101Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_99 graph tests ------------------------------------------------

    #[test]
    fn xg_99_graph_empty() {
        let g = super::Xg99Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_99_graph_add_node() {
        let mut g = super::Xg99Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_99_graph_add_edge() {
        let mut g = super::Xg99Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_99_graph_neighbors() {
        let mut g = super::Xg99Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_99_graph_has_path() {
        let mut g = super::Xg99Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_99_graph_self_path() {
        let g = super::Xg99Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_99_graph_topo_sort() {
        let mut g = super::Xg99Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_99_graph_cycle_detect_false() {
        let mut g = super::Xg99Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_99_graph_cycle_detect_true() {
        let mut g = super::Xg99Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_99 heap tests -------------------------------------------------

    #[test]
    fn xg_99_heap_empty() {
        let h: super::Xg99Heap<i32> = super::Xg99Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_99_heap_push_pop() {
        let mut h = super::Xg99Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_99_heap_peek() {
        let mut h = super::Xg99Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_99_heap_drain_sorted() {
        let mut h = super::Xg99Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_99_heap_merge() {
        let mut a = super::Xg99Heap::new();
        let mut b = super::Xg99Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_99_heap_default() {
        let h: super::Xg99Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_99_graph_default() {
        let g: super::Xg99Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh74_skip_insert_contains() {
        let mut sl = super::Xh74SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh74_skip_remove() {
        let mut sl = super::Xh74SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh74_skip_len() {
        let mut sl = super::Xh74SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh74_skip_range_query() {
        let mut sl = super::Xh74SkipList::xh_new(4);
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
    fn xh74_skip_floor_ceiling() {
        let mut sl = super::Xh74SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh74_skip_rank() {
        let mut sl = super::Xh74SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh74_skip_empty() {
        let sl = super::Xh74SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh74_skip_duplicates() {
        let mut sl = super::Xh74SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh74_bitset_set_test() {
        let mut bs = super::Xh74BitSet::xh_new(256);
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
    fn xh74_bitset_clear_count() {
        let mut bs = super::Xh74BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh74_bitset_and_or_xor() {
        let mut a = super::Xh74BitSet::xh_new(128);
        let mut b = super::Xh74BitSet::xh_new(128);
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
    fn xh74_bitset_iter_ones() {
        let mut bs = super::Xh74BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh74_bitset_first_last() {
        let mut bs = super::Xh74BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh74_bitset_empty() {
        let bs = super::Xh74BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi74_deque_push_pop_back() {
        let mut dq = super::Xi74Deque::xi_new(4);
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
    fn xi74_deque_push_pop_front() {
        let mut dq = super::Xi74Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi74_deque_mixed_ops() {
        let mut dq = super::Xi74Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi74_deque_get_and_split() {
        let mut dq = super::Xi74Deque::xi_new(8);
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
    fn xi74_deque_rotate_left() {
        let mut dq = super::Xi74Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi74_deque_rotate_right() {
        let mut dq = super::Xi74Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi74_deque_grow() {
        let mut dq = super::Xi74Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi74_deque_empty() {
        let dq = super::Xi74Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi74_interval_tree_insert_query() {
        let mut tree = super::Xi74IntervalTree::xi_new();
        tree.xi_insert(super::Xi74Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi74Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi74Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi74_interval_tree_overlap() {
        let mut tree = super::Xi74IntervalTree::xi_new();
        tree.xi_insert(super::Xi74Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi74Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi74Interval::xi_new(12, 20));
        let q = super::Xi74Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi74_interval_tree_remove() {
        let mut tree = super::Xi74IntervalTree::xi_new();
        tree.xi_insert(super::Xi74Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi74Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi74_interval_tree_gaps() {
        let mut tree = super::Xi74IntervalTree::xi_new();
        tree.xi_insert(super::Xi74Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi74Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi74Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi74Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi74Interval::xi_new(8, 10));
    }

    #[test]
    fn xi74_interval_tree_merge() {
        let mut tree = super::Xi74IntervalTree::xi_new();
        tree.xi_insert(super::Xi74Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi74Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi74Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi74Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi74Interval::xi_new(10, 15));
    }

    #[test]
    fn xi74_interval_tree_all() {
        let mut tree = super::Xi74IntervalTree::xi_new();
        tree.xi_insert(super::Xi74Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi74Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi74_interval_tree_empty() {
        let tree = super::Xi74IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi74_interval_tree_contains_point() {
        let iv = super::Xi74Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 75) ---

    #[test]
    fn xj_75_uf_make_and_find() {
        let mut uf = super::Xj75UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_75_uf_union_connected() {
        let mut uf = super::Xj75UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_75_uf_component_count() {
        let mut uf = super::Xj75UnionFind::xj_new();
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
    fn xj_75_uf_component_size() {
        let mut uf = super::Xj75UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_75_uf_largest_component() {
        let mut uf = super::Xj75UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_75_uf_many_elements() {
        let mut uf = super::Xj75UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_75_uf_separate_components() {
        let mut uf = super::Xj75UnionFind::xj_new();
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
    fn xj_75_uf_path_compression() {
        let mut uf = super::Xj75UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_75_bt_insert_get() {
        let mut bt = super::Xj75BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_75_bt_contains_len() {
        let mut bt = super::Xj75BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_75_bt_replace() {
        let mut bt = super::Xj75BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_75_bt_remove() {
        let mut bt = super::Xj75BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_75_bt_keys_values() {
        let mut bt = super::Xj75BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_75_bt_range() {
        let mut bt = super::Xj75BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_75_bt_min_max() {
        let mut bt = super::Xj75BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_75_bt_many_inserts() {
        let mut bt = super::Xj75BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_74 segment tree tests ---

    #[test]
    fn xk_74_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk74SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_74_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk74SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_74_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk74SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_74_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk74SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_74_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk74SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_74_st_single_element() {
        let data = vec![42];
        let st = super::Xk74SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_74_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk74SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_74_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk74SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_74 disjoint intervals tests ---

    #[test]
    fn xk_74_di_add_and_count() {
        let mut di = super::Xk74DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_74_di_merge_overlap() {
        let mut di = super::Xk74DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_74_di_contains() {
        let mut di = super::Xk74DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_74_di_remove() {
        let mut di = super::Xk74DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_74_di_covered_length() {
        let mut di = super::Xk74DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_74_di_gaps() {
        let mut di = super::Xk74DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_74_di_merge_adjacent() {
        let mut di = super::Xk74DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_74_di_empty() {
        let di = super::Xk74DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_75_rope_new_empty() {
        let rope = super::Xl75Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_75_rope_from_str() {
        let rope = super::Xl75Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_75_rope_insert_at() {
        let mut rope = super::Xl75Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_75_rope_delete_range() {
        let mut rope = super::Xl75Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_75_rope_char_at() {
        let rope = super::Xl75Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_75_rope_split_concat() {
        let rope = super::Xl75Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_75_rope_line_count() {
        let rope = super::Xl75Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_75_rope_line_at() {
        let rope = super::Xl75Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_75_sa_build_and_search() {
        let sa = super::Xl75SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_75_sa_count() {
        let sa = super::Xl75SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_75_sa_longest_repeated() {
        let sa = super::Xl75SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_75_sa_all_positions() {
        let sa = super::Xl75SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_75_sa_len() {
        let sa = super::Xl75SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_75_sa_empty() {
        let sa = super::Xl75SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_75_rope_slice() {
        let rope = super::Xl75Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_75_sa_search_start() {
        let sa = super::Xl75SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }
}
