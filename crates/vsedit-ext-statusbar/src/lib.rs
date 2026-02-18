//! Ext API: Status bar.
//!
//! RPC bridge between the extension host and the main thread for status bar items.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_statusbar";

// ── Error Types ──

/// Errors that can occur when operating on status bar items.
#[derive(Debug, Clone, PartialEq)]
pub enum StatusBarError {
    /// The referenced item does not exist.
    ItemNotFound(String),
    /// An item with this id already exists.
    DuplicateItem(String),
    /// The provided value failed validation.
    InvalidField { field: &'static str, reason: String },
}

impl fmt::Display for StatusBarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatusBarError::ItemNotFound(id) => write!(f, "status bar item not found: {id}"),
            StatusBarError::DuplicateItem(id) => {
                write!(f, "status bar item already exists: {id}")
            }
            StatusBarError::InvalidField { field, reason } => {
                write!(f, "invalid field '{field}': {reason}")
            }
        }
    }
}

impl std::error::Error for StatusBarError {}

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StatusBarMessage {
    CreateItem {
        id: String,
        alignment: StatusBarAlignment,
        priority: i32,
    },
    UpdateItem {
        id: String,
        text: Option<String>,
        tooltip: Option<String>,
        command: Option<String>,
    },
    ShowItem {
        id: String,
    },
    HideItem {
        id: String,
    },
    DisposeItem {
        id: String,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum StatusBarAlignment {
    Left,
    Right,
}

impl StatusBarAlignment {
    /// Returns `true` if this is `StatusBarAlignment::Left`.
    pub fn is_left(&self) -> bool {
        matches!(self, StatusBarAlignment::Left)
    }

    /// Returns `true` if this is `StatusBarAlignment::Right`.
    pub fn is_right(&self) -> bool {
        matches!(self, StatusBarAlignment::Right)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatusBarItem {
    pub id: String,
    pub text: String,
    pub tooltip: Option<String>,
    pub command: Option<String>,
    pub alignment: StatusBarAlignment,
    pub priority: i32,
    pub is_visible: bool,
}

// ── Bridge ──

pub struct StatusBarBridge {
    items: Vec<StatusBarItem>,
}

impl StatusBarBridge {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn create_item(&mut self, id: &str, alignment: StatusBarAlignment, priority: i32) {
        if !self.items.iter().any(|i| i.id == id) {
            self.items.push(StatusBarItem {
                id: id.to_string(),
                text: String::new(),
                tooltip: None,
                command: None,
                alignment,
                priority,
                is_visible: false,
            });
        }
    }

    pub fn get_item(&self, id: &str) -> Option<&StatusBarItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn show_item(&mut self, id: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.is_visible = true;
        }
    }

    pub fn hide_item(&mut self, id: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.is_visible = false;
        }
    }

    pub fn dispose_item(&mut self, id: &str) {
        self.items.retain(|i| i.id != id);
    }

    pub fn handle_message(&mut self, msg: &StatusBarMessage) -> serde_json::Value {
        match msg {
            StatusBarMessage::CreateItem {
                id,
                alignment,
                priority,
            } => {
                self.create_item(id, *alignment, *priority);
                serde_json::json!({"created": true})
            }
            StatusBarMessage::UpdateItem {
                id,
                text,
                tooltip,
                command,
            } => {
                if let Some(item) = self.items.iter_mut().find(|i| i.id == *id) {
                    if let Some(t) = text {
                        item.text = t.clone();
                    }
                    if tooltip.is_some() {
                        item.tooltip = tooltip.clone();
                    }
                    if command.is_some() {
                        item.command = command.clone();
                    }
                    serde_json::json!({"updated": true})
                } else {
                    serde_json::json!({"error": "not found"})
                }
            }
            StatusBarMessage::ShowItem { id } => {
                self.show_item(id);
                serde_json::json!({"shown": true})
            }
            StatusBarMessage::HideItem { id } => {
                self.hide_item(id);
                serde_json::json!({"hidden": true})
            }
            StatusBarMessage::DisposeItem { id } => {
                self.dispose_item(id);
                serde_json::json!({"disposed": true})
            }
        }
    }
}

impl Default for StatusBarBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusBarBridge {
    /// Return the number of registered items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Return the number of currently visible items.
    pub fn visible_count(&self) -> usize {
        self.items.iter().filter(|i| i.is_visible).count()
    }

    /// Return all items for a given alignment, sorted by descending priority.
    pub fn items_by_alignment(&self, alignment: StatusBarAlignment) -> Vec<&StatusBarItem> {
        let mut out: Vec<&StatusBarItem> = self
            .items
            .iter()
            .filter(|i| i.alignment == alignment)
            .collect();
        out.sort_by(|a, b| b.priority.cmp(&a.priority));
        out
    }

    /// Return only the visible items, sorted by descending priority.
    pub fn visible_items(&self) -> Vec<&StatusBarItem> {
        let mut out: Vec<&StatusBarItem> = self.items.iter().filter(|i| i.is_visible).collect();
        out.sort_by(|a, b| b.priority.cmp(&a.priority));
        out
    }

    /// Create an item, returning an error if it already exists.
    pub fn try_create_item(
        &mut self,
        id: &str,
        alignment: StatusBarAlignment,
        priority: i32,
    ) -> Result<(), StatusBarError> {
        if self.items.iter().any(|i| i.id == id) {
            return Err(StatusBarError::DuplicateItem(id.to_string()));
        }
        self.items.push(StatusBarItem {
            id: id.to_string(),
            text: String::new(),
            tooltip: None,
            command: None,
            alignment,
            priority,
            is_visible: false,
        });
        Ok(())
    }

    /// Update an item's fields, returning an error if the item does not exist.
    pub fn update_item(
        &mut self,
        id: &str,
        text: Option<&str>,
        tooltip: Option<&str>,
        command: Option<&str>,
    ) -> Result<(), StatusBarError> {
        let item = self
            .items
            .iter_mut()
            .find(|i| i.id == id)
            .ok_or_else(|| StatusBarError::ItemNotFound(id.to_string()))?;
        if let Some(t) = text {
            item.text = t.to_string();
        }
        if let Some(tt) = tooltip {
            item.tooltip = Some(tt.to_string());
        }
        if let Some(c) = command {
            item.command = Some(c.to_string());
        }
        Ok(())
    }

    /// Dispose all items, returning the number of items removed.
    pub fn dispose_all(&mut self) -> usize {
        let count = self.items.len();
        self.items.clear();
        count
    }

    /// Get a mutable reference to an item.
    pub fn get_item_mut(&mut self, id: &str) -> Option<&mut StatusBarItem> {
        self.items.iter_mut().find(|i| i.id == id)
    }

    /// Find items whose text contains the given substring (case-sensitive).
    pub fn find_by_text(&self, text: &str) -> Vec<&StatusBarItem> {
        self.items.iter().filter(|i| i.text.contains(text)).collect()
    }

    /// Return all items sorted by priority (descending), with stable tie-breaking by id.
    pub fn sorted_items(&self) -> Vec<&StatusBarItem> {
        let mut refs: Vec<&StatusBarItem> = self.items.iter().collect();
        refs.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        refs
    }

    /// Toggle the visibility of an item, returning an error if it does not exist.
    pub fn toggle_visibility(&mut self, id: &str) -> Result<(), StatusBarError> {
        let item = self
            .items
            .iter_mut()
            .find(|i| i.id == id)
            .ok_or_else(|| StatusBarError::ItemNotFound(id.to_string()))?;
        item.is_visible = !item.is_visible;
        Ok(())
    }

    /// Return items that have a command attached.
    pub fn get_items_with_command(&self) -> Vec<&StatusBarItem> {
        self.items.iter().filter(|i| i.command.is_some()).collect()
    }
}

// ── StatusBarItem helpers ──

impl StatusBarItem {
    /// Returns `true` if the item has a command attached.
    pub fn has_command(&self) -> bool {
        self.command.is_some()
    }

    /// Returns a human-readable description of the item count context,
    /// e.g. "1 item" or "3 items", based on the character length of the text.
    pub fn age_description(&self) -> String {
        let count = self.text.len();
        if count == 1 {
            "1 item".to_string()
        } else {
            format!("{count} items")
        }
    }

    /// Returns the display text, falling back to the id if text is empty.
    pub fn display_text(&self) -> &str {
        if self.text.is_empty() {
            &self.id
        } else {
            &self.text
        }
    }
}

impl fmt::Display for StatusBarItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let vis = if self.is_visible { "visible" } else { "hidden" };
        write!(f, "[{}] {} ({})", self.id, self.display_text(), vis)
    }
}

impl fmt::Display for StatusBarAlignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatusBarAlignment::Left => f.write_str("Left"),
            StatusBarAlignment::Right => f.write_str("Right"),
        }
    }
}

// ── Builder ──

/// Builder for constructing a [`StatusBarItem`] with validation.
#[derive(Debug, Clone)]
pub struct StatusBarItemBuilder {
    id: Option<String>,
    text: String,
    tooltip: Option<String>,
    command: Option<String>,
    alignment: StatusBarAlignment,
    priority: i32,
    is_visible: bool,
}

impl StatusBarItemBuilder {
    pub fn new() -> Self {
        Self {
            id: None,
            text: String::new(),
            tooltip: None,
            command: None,
            alignment: StatusBarAlignment::Left,
            priority: 0,
            is_visible: false,
        }
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }

    pub fn alignment(mut self, alignment: StatusBarAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.is_visible = visible;
        self
    }

    /// Validate and build the [`StatusBarItem`].
    pub fn build(self) -> Result<StatusBarItem, StatusBarError> {
        let id = self.id.ok_or_else(|| StatusBarError::InvalidField {
            field: "id",
            reason: "id is required".into(),
        })?;
        if id.is_empty() {
            return Err(StatusBarError::InvalidField {
                field: "id",
                reason: "id must not be empty".into(),
            });
        }
        Ok(StatusBarItem {
            id,
            text: self.text,
            tooltip: self.tooltip,
            command: self.command,
            alignment: self.alignment,
            priority: self.priority,
            is_visible: self.is_visible,
        })
    }
}

impl Default for StatusBarItemBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the statusbar extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

/// Accumulated statistics for ext-statusbar operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtStatusbarStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtStatusbarStats {
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
    pub fn merge(&mut self, other: &ExtStatusbarStats) {
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

impl Default for ExtStatusbarStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtStatusbarStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtStatusbarStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-statusbar.
#[derive(Debug, Clone)]
pub struct ExtStatusbarValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtStatusbarValidator {
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

impl Default for ExtStatusbarValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ── Priority Sorting ──

/// Sort status bar items by priority (descending), with stable tie-breaking by id.
pub fn sort_items_by_priority(items: &mut [StatusBarItem]) {
    items.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
}

/// Return items sorted by priority without modifying the original slice.
pub fn sorted_by_priority(items: &[StatusBarItem]) -> Vec<&StatusBarItem> {
    let mut refs: Vec<&StatusBarItem> = items.iter().collect();
    refs.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
    refs
}

// ── Visibility Manager ──

/// Manages visibility of status bar items grouped by extension namespace.
#[derive(Debug, Clone, Default)]
pub struct StatusBarVisibilityManager {
    /// Maps extension namespace -> visibility flag
    hidden_namespaces: std::collections::HashSet<String>,
}

impl StatusBarVisibilityManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Hide all items belonging to the given extension namespace.
    pub fn hide_namespace(&mut self, namespace: &str) {
        self.hidden_namespaces.insert(namespace.to_string());
    }

    /// Show all items belonging to the given extension namespace.
    pub fn show_namespace(&mut self, namespace: &str) {
        self.hidden_namespaces.remove(namespace);
    }

    /// Returns `true` if the namespace is currently hidden.
    pub fn is_namespace_hidden(&self, namespace: &str) -> bool {
        self.hidden_namespaces.contains(namespace)
    }

    /// Extract the namespace from an item ID (everything before the first '.').
    pub fn extract_namespace(item_id: &str) -> &str {
        item_id.split('.').next().unwrap_or(item_id)
    }

    /// Filter items, removing those whose namespace is hidden.
    pub fn filter_visible<'a>(&self, items: &'a [StatusBarItem]) -> Vec<&'a StatusBarItem> {
        items
            .iter()
            .filter(|item| {
                let ns = Self::extract_namespace(&item.id);
                !self.hidden_namespaces.contains(ns)
            })
            .collect()
    }

    /// Return the number of hidden namespaces.
    pub fn hidden_count(&self) -> usize {
        self.hidden_namespaces.len()
    }
}

// ── Layout Computation ──

/// Represents the computed layout of the status bar.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusBarLayout {
    /// Items on the left side, sorted by descending priority.
    pub left_items: Vec<String>,
    /// Items on the right side, sorted by descending priority.
    pub right_items: Vec<String>,
}

/// Compute the layout of visible status bar items.
///
/// Visible items are split by alignment and sorted by descending priority.
/// Items with the same priority are sorted alphabetically by ID.
pub fn status_bar_layout(items: &[StatusBarItem]) -> StatusBarLayout {
    let visible: Vec<&StatusBarItem> = items.iter().filter(|i| i.is_visible).collect();

    let mut left: Vec<&StatusBarItem> = visible
        .iter()
        .filter(|i| i.alignment == StatusBarAlignment::Left)
        .copied()
        .collect();
    let mut right: Vec<&StatusBarItem> = visible
        .iter()
        .filter(|i| i.alignment == StatusBarAlignment::Right)
        .copied()
        .collect();

    left.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
    right.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));

    StatusBarLayout {
        left_items: left.into_iter().map(|i| i.id.clone()).collect(),
        right_items: right.into_iter().map(|i| i.id.clone()).collect(),
    }
}

// ---------------------------------------------------------------------------
// StatusBarGroup — logical grouping of items
// ---------------------------------------------------------------------------

/// A named group of status bar items for bulk operations.
#[derive(Debug, Clone)]
pub struct StatusBarGroup {
    pub name: String,
    pub item_ids: Vec<String>,
    pub visible: bool,
    pub priority_offset: i32,
}

impl StatusBarGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            item_ids: Vec::new(),
            visible: true,
            priority_offset: 0,
        }
    }

    /// Add an item ID to this group.
    pub fn add_item(&mut self, id: impl Into<String>) {
        self.item_ids.push(id.into());
    }

    /// Remove an item ID from this group.
    pub fn remove_item(&mut self, id: &str) {
        self.item_ids.retain(|i| i != id);
    }

    /// Number of items in this group.
    pub fn item_count(&self) -> usize {
        self.item_ids.len()
    }

    /// Returns true if the group contains the given item ID.
    pub fn contains(&self, id: &str) -> bool {
        self.item_ids.iter().any(|i| i == id)
    }
}

impl fmt::Display for StatusBarGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let vis = if self.visible { "visible" } else { "hidden" };
        write!(f, "Group({}, {} items, {})", self.name, self.item_ids.len(), vis)
    }
}

/// Manages multiple status bar groups and applies bulk visibility operations.
#[derive(Debug, Clone, Default)]
pub struct StatusBarGroupManager {
    pub groups: Vec<StatusBarGroup>,
}

impl StatusBarGroupManager {
    pub fn new() -> Self {
        Self { groups: Vec::new() }
    }

    /// Add a new group.
    pub fn add_group(&mut self, group: StatusBarGroup) {
        self.groups.push(group);
    }

    /// Find a group by name.
    pub fn find_group(&self, name: &str) -> Option<&StatusBarGroup> {
        self.groups.iter().find(|g| g.name == name)
    }

    /// Find a mutable group by name.
    pub fn find_group_mut(&mut self, name: &str) -> Option<&mut StatusBarGroup> {
        self.groups.iter_mut().find(|g| g.name == name)
    }

    /// Show all items in a group on the bridge.
    pub fn show_group(&mut self, name: &str, bridge: &mut StatusBarBridge) {
        if let Some(group) = self.groups.iter_mut().find(|g| g.name == name) {
            group.visible = true;
            for id in &group.item_ids {
                bridge.show_item(id);
            }
        }
    }

    /// Hide all items in a group on the bridge.
    pub fn hide_group(&mut self, name: &str, bridge: &mut StatusBarBridge) {
        if let Some(group) = self.groups.iter_mut().find(|g| g.name == name) {
            group.visible = false;
            for id in &group.item_ids {
                bridge.hide_item(id);
            }
        }
    }

    /// Toggle visibility of a group.
    pub fn toggle_group(&mut self, name: &str, bridge: &mut StatusBarBridge) {
        let is_visible = self.groups.iter().find(|g| g.name == name).map(|g| g.visible);
        match is_visible {
            Some(true) => self.hide_group(name, bridge),
            Some(false) => self.show_group(name, bridge),
            None => {}
        }
    }

    /// Total number of groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Find which group an item belongs to, if any.
    pub fn group_for_item(&self, id: &str) -> Option<&StatusBarGroup> {
        self.groups.iter().find(|g| g.contains(id))
    }
}

// ---------------------------------------------------------------------------
// AnimationState — for animated status bar items
// ---------------------------------------------------------------------------

/// Animation state for a status bar item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnimationState {
    /// No animation.
    None,
    /// Spinning / loading indicator.
    Spinning,
    /// Pulsing / attention indicator.
    Pulsing,
    /// Fading in/out.
    Fading,
}

impl Default for AnimationState {
    fn default() -> Self {
        AnimationState::None
    }
}

impl fmt::Display for AnimationState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnimationState::None => write!(f, "none"),
            AnimationState::Spinning => write!(f, "spinning"),
            AnimationState::Pulsing => write!(f, "pulsing"),
            AnimationState::Fading => write!(f, "fading"),
        }
    }
}

/// Extended status bar item with animation support.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnimatedStatusBarItem {
    pub item: StatusBarItem,
    pub animation: AnimationState,
    pub progress: Option<f32>,
}

impl AnimatedStatusBarItem {
    pub fn new(item: StatusBarItem) -> Self {
        Self {
            item,
            animation: AnimationState::None,
            progress: None,
        }
    }

    /// Set the animation state.
    pub fn set_animation(&mut self, state: AnimationState) {
        self.animation = state;
    }

    /// Set progress (0.0 to 1.0), automatically enabling spinning animation.
    pub fn set_progress(&mut self, progress: f32) {
        self.progress = Some(progress.clamp(0.0, 1.0));
        if self.animation == AnimationState::None {
            self.animation = AnimationState::Spinning;
        }
    }

    /// Clear progress and stop animation.
    pub fn clear_progress(&mut self) {
        self.progress = None;
        self.animation = AnimationState::None;
    }

    /// Whether this item is currently animating.
    pub fn is_animating(&self) -> bool {
        self.animation != AnimationState::None
    }

    /// Render the item text with animation indicator.
    pub fn render_text(&self) -> String {
        let prefix = match self.animation {
            AnimationState::None => "",
            AnimationState::Spinning => "$(sync~spin) ",
            AnimationState::Pulsing => "$(pulse) ",
            AnimationState::Fading => "$(fade) ",
        };
        if let Some(pct) = self.progress {
            format!("{}{} ({:.0}%)", prefix, self.item.display_text(), pct * 100.0)
        } else {
            format!("{}{}", prefix, self.item.display_text())
        }
    }
}

impl fmt::Display for AnimatedStatusBarItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.item, self.animation)
    }
}

// ---------------------------------------------------------------------------
// Bulk visibility operations
// ---------------------------------------------------------------------------

impl StatusBarBridge {
    /// Show all items matching a predicate.
    pub fn show_matching(&mut self, predicate: impl Fn(&StatusBarItem) -> bool) -> usize {
        let mut count = 0;
        for item in &mut self.items {
            if predicate(item) && !item.is_visible {
                item.is_visible = true;
                count += 1;
            }
        }
        count
    }

    /// Hide all items matching a predicate.
    pub fn hide_matching(&mut self, predicate: impl Fn(&StatusBarItem) -> bool) -> usize {
        let mut count = 0;
        for item in &mut self.items {
            if predicate(item) && item.is_visible {
                item.is_visible = false;
                count += 1;
            }
        }
        count
    }

    /// Show all items.
    pub fn show_all(&mut self) -> usize {
        self.show_matching(|_| true)
    }

    /// Hide all items.
    pub fn hide_all(&mut self) -> usize {
        self.hide_matching(|_| true)
    }

    /// Show all items with a specific alignment.
    pub fn show_by_alignment(&mut self, alignment: StatusBarAlignment) -> usize {
        self.show_matching(|item| item.alignment == alignment)
    }

    /// Hide all items with a specific alignment.
    pub fn hide_by_alignment(&mut self, alignment: StatusBarAlignment) -> usize {
        self.hide_matching(|item| item.alignment == alignment)
    }

    /// Return items sorted by priority within a specific alignment.
    pub fn priority_sorted_by_alignment(&self, alignment: StatusBarAlignment) -> Vec<&StatusBarItem> {
        let mut items: Vec<&StatusBarItem> = self.items.iter()
            .filter(|i| i.alignment == alignment && i.is_visible)
            .collect();
        items.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.id.cmp(&b.id)));
        items
    }
}

// ---------------------------------------------------------------------------
// StatusBarBridge — batch and rendering helpers
// ---------------------------------------------------------------------------

impl StatusBarBridge {
    /// Create multiple items at once. Returns the number of items actually created
    /// (skipping duplicates).
    pub fn create_items(&mut self, specs: &[(&str, StatusBarAlignment, i32)]) -> usize {
        let mut created = 0;
        for &(id, alignment, priority) in specs {
            if !self.items.iter().any(|i| i.id == id) {
                self.create_item(id, alignment, priority);
                created += 1;
            }
        }
        created
    }

    /// Render a simple text representation of the status bar layout.
    /// Returns `"[left items] | [right items]"` with items separated by spaces.
    pub fn render_text(&self) -> String {
        let mut left: Vec<&StatusBarItem> = self.items.iter()
            .filter(|i| i.is_visible && i.alignment == StatusBarAlignment::Left)
            .collect();
        left.sort_by(|a, b| b.priority.cmp(&a.priority));

        let mut right: Vec<&StatusBarItem> = self.items.iter()
            .filter(|i| i.is_visible && i.alignment == StatusBarAlignment::Right)
            .collect();
        right.sort_by(|a, b| b.priority.cmp(&a.priority));

        let left_text: Vec<&str> = left.iter().map(|i| i.display_text()).collect();
        let right_text: Vec<&str> = right.iter().map(|i| i.display_text()).collect();
        format!("{} | {}", left_text.join("  "), right_text.join("  "))
    }

    /// Find an item by its command string.
    pub fn find_by_command(&self, command: &str) -> Vec<&StatusBarItem> {
        self.items.iter()
            .filter(|i| i.command.as_deref() == Some(command))
            .collect()
    }

    /// Return a JSON array of all visible items (for serialization to frontend).
    pub fn to_json(&self) -> serde_json::Value {
        let items: Vec<serde_json::Value> = self.visible_items()
            .iter()
            .map(|i| serde_json::json!({
                "id": i.id,
                "text": i.display_text(),
                "alignment": format!("{}", i.alignment),
                "priority": i.priority,
            }))
            .collect();
        serde_json::Value::Array(items)
    }

    /// Update the priority of an item. Returns Ok(()) on success or an error
    /// if the item doesn't exist.
    pub fn set_priority(&mut self, id: &str, priority: i32) -> Result<(), StatusBarError> {
        let item = self.items.iter_mut()
            .find(|i| i.id == id)
            .ok_or_else(|| StatusBarError::ItemNotFound(id.to_string()))?;
        item.priority = priority;
        Ok(())
    }

    /// Count items that have non-empty text set.
    pub fn items_with_text_count(&self) -> usize {
        self.items.iter().filter(|i| !i.text.is_empty()).count()
    }
}

// ---------------------------------------------------------------------------
// StatusBarRenderer — terminal-width-aware rendering
// ---------------------------------------------------------------------------

/// Renders status bar items for a fixed terminal width, truncating as needed.
#[derive(Debug, Clone)]
pub struct StatusBarRenderer {
    /// Total available terminal columns.
    pub width: usize,
    /// Separator between left and right sections.
    pub separator: String,
    /// Ellipsis string used when truncating.
    pub ellipsis: String,
    /// Minimum characters to keep per item before truncating it away entirely.
    pub min_item_width: usize,
}

impl StatusBarRenderer {
    pub fn new(width: usize) -> Self {
        Self {
            width,
            separator: " | ".to_string(),
            ellipsis: "…".to_string(),
            min_item_width: 3,
        }
    }

    /// Set a custom separator string.
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    /// Set the minimum width before an item is dropped entirely.
    pub fn with_min_item_width(mut self, w: usize) -> Self {
        self.min_item_width = w;
        self
    }

    /// Truncate a single text string to fit within `max_chars`, appending
    /// the configured ellipsis if truncation occurs.
    pub fn truncate_text(&self, text: &str, max_chars: usize) -> String {
        let char_count = text.chars().count();
        if char_count <= max_chars {
            return text.to_string();
        }
        if max_chars <= self.ellipsis.chars().count() {
            return self.ellipsis.chars().take(max_chars).collect();
        }
        let keep = max_chars - self.ellipsis.chars().count();
        let truncated: String = text.chars().take(keep).collect();
        format!("{}{}", truncated, self.ellipsis)
    }

    /// Render a set of items into a single line that fits within `self.width`.
    ///
    /// Left-aligned items appear first, then the separator, then right-aligned
    /// items. Items within each group are separated by two spaces and sorted by
    /// descending priority.
    pub fn render(&self, items: &[StatusBarItem]) -> String {
        let sep_len = self.separator.chars().count();
        let item_sep = "  ";
        let item_sep_len = item_sep.chars().count();

        let mut left: Vec<&StatusBarItem> = items
            .iter()
            .filter(|i| i.is_visible && i.alignment == StatusBarAlignment::Left)
            .collect();
        let mut right: Vec<&StatusBarItem> = items
            .iter()
            .filter(|i| i.is_visible && i.alignment == StatusBarAlignment::Right)
            .collect();

        left.sort_by(|a, b| b.priority.cmp(&a.priority));
        right.sort_by(|a, b| b.priority.cmp(&a.priority));

        let left_texts: Vec<String> = left.iter().map(|i| i.display_text().to_string()).collect();
        let right_texts: Vec<String> = right.iter().map(|i| i.display_text().to_string()).collect();

        let joined_left = left_texts.join(item_sep);
        let joined_right = right_texts.join(item_sep);
        let full = format!("{}{}{}", joined_left, self.separator, joined_right);

        if full.chars().count() <= self.width {
            return full;
        }

        // Budget: split available space between left and right
        let usable = self.width.saturating_sub(sep_len);
        let left_budget = usable / 2;
        let right_budget = usable.saturating_sub(left_budget);

        let trunc_left = self.truncate_section(&left_texts, left_budget, item_sep_len);
        let trunc_right = self.truncate_section(&right_texts, right_budget, item_sep_len);

        format!("{}{}{}", trunc_left, self.separator, trunc_right)
    }

    /// Truncate a section of item texts to fit within a character budget.
    fn truncate_section(&self, texts: &[String], budget: usize, sep_len: usize) -> String {
        if texts.is_empty() {
            return String::new();
        }

        let mut result: Vec<String> = Vec::new();
        let mut remaining = budget;

        for (i, text) in texts.iter().enumerate() {
            let need_sep = if i > 0 { sep_len } else { 0 };
            if remaining < need_sep + self.min_item_width {
                break;
            }
            remaining -= need_sep;
            let available = remaining;
            let truncated = self.truncate_text(text, available);
            let used = truncated.chars().count();
            remaining = remaining.saturating_sub(used);
            result.push(truncated);
        }

        result.join("  ")
    }
}

impl Default for StatusBarRenderer {
    fn default() -> Self {
        Self::new(80)
    }
}

// ---------------------------------------------------------------------------
// StatusBarColorStyle — styling metadata for items
// ---------------------------------------------------------------------------

/// Color/style metadata that can be attached to a status bar item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusBarColorStyle {
    /// Foreground color as a CSS-style hex string, e.g. `"#ffffff"`.
    pub foreground: Option<String>,
    /// Background color as a CSS-style hex string.
    pub background: Option<String>,
    /// Whether the text should be rendered bold.
    pub bold: bool,
    /// Whether the text should be rendered italic.
    pub italic: bool,
}

impl StatusBarColorStyle {
    pub fn new() -> Self {
        Self {
            foreground: None,
            background: None,
            bold: false,
            italic: false,
        }
    }

    /// Create a style with only a foreground color.
    pub fn fg(color: impl Into<String>) -> Self {
        Self {
            foreground: Some(color.into()),
            ..Self::new()
        }
    }

    /// Create a style with foreground and background colors.
    pub fn fg_bg(fg: impl Into<String>, bg: impl Into<String>) -> Self {
        Self {
            foreground: Some(fg.into()),
            background: Some(bg.into()),
            ..Self::new()
        }
    }

    /// Return a new style with bold enabled.
    pub fn with_bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Return a new style with italic enabled.
    pub fn with_italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// Validate that color strings are well-formed hex colors (`#rrggbb`).
    pub fn validate(&self) -> Result<(), StatusBarError> {
        if let Some(ref fg) = self.foreground {
            Self::validate_hex_color(fg)?;
        }
        if let Some(ref bg) = self.background {
            Self::validate_hex_color(bg)?;
        }
        Ok(())
    }

    fn validate_hex_color(s: &str) -> Result<(), StatusBarError> {
        let valid = s.len() == 7
            && s.starts_with('#')
            && s[1..].chars().all(|c| c.is_ascii_hexdigit());
        if !valid {
            return Err(StatusBarError::InvalidField {
                field: "color",
                reason: format!("'{}' is not a valid #rrggbb color", s),
            });
        }
        Ok(())
    }

    /// Merge another style on top of this one. Non-`None` fields in `other`
    /// override values in `self`.
    pub fn merge(&self, other: &StatusBarColorStyle) -> StatusBarColorStyle {
        StatusBarColorStyle {
            foreground: other.foreground.clone().or_else(|| self.foreground.clone()),
            background: other.background.clone().or_else(|| self.background.clone()),
            bold: other.bold || self.bold,
            italic: other.italic || self.italic,
        }
    }
}

impl Default for StatusBarColorStyle {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for StatusBarColorStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fg = self.foreground.as_deref().unwrap_or("inherit");
        let bg = self.background.as_deref().unwrap_or("inherit");
        let mut flags = Vec::new();
        if self.bold {
            flags.push("bold");
        }
        if self.italic {
            flags.push("italic");
        }
        let flag_str = if flags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", flags.join(", "))
        };
        write!(f, "fg={} bg={}{}", fg, bg, flag_str)
    }
}

// ---------------------------------------------------------------------------
// ClickAction — dispatching click actions for status bar items
// ---------------------------------------------------------------------------

/// Represents an action to be dispatched when a status bar item is clicked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum ClickAction {
    /// Run an editor command by name.
    RunCommand { command: String, args: Vec<String> },
    /// Open a URL in the default browser.
    OpenUrl { url: String },
    /// Show a quick-pick menu with the given options.
    ShowQuickPick { items: Vec<String> },
    /// Do nothing (the item is informational only).
    None,
}

impl Default for ClickAction {
    fn default() -> Self {
        ClickAction::None
    }
}

impl fmt::Display for ClickAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClickAction::RunCommand { command, args } => {
                if args.is_empty() {
                    write!(f, "cmd:{}", command)
                } else {
                    write!(f, "cmd:{}({})", command, args.join(", "))
                }
            }
            ClickAction::OpenUrl { url } => write!(f, "url:{}", url),
            ClickAction::ShowQuickPick { items } => {
                write!(f, "pick:[{}]", items.join(", "))
            }
            ClickAction::None => write!(f, "none"),
        }
    }
}

/// Manages the mapping from status bar item IDs to click actions.
#[derive(Debug, Clone, Default)]
pub struct ClickActionDispatcher {
    actions: Vec<(String, ClickAction)>,
}

impl ClickActionDispatcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a click action for an item.
    pub fn register(&mut self, item_id: impl Into<String>, action: ClickAction) {
        let id = item_id.into();
        if let Some(entry) = self.actions.iter_mut().find(|(k, _)| k == &id) {
            entry.1 = action;
        } else {
            self.actions.push((id, action));
        }
    }

    /// Remove the click action for an item.
    pub fn unregister(&mut self, item_id: &str) {
        self.actions.retain(|(k, _)| k != item_id);
    }

    /// Look up the click action for an item.
    pub fn get(&self, item_id: &str) -> Option<&ClickAction> {
        self.actions.iter().find(|(k, _)| k == item_id).map(|(_, v)| v)
    }

    /// Dispatch a click for the given item ID. Returns the action if one is
    /// registered, or `ClickAction::None` otherwise.
    pub fn dispatch(&self, item_id: &str) -> ClickAction {
        self.get(item_id).cloned().unwrap_or(ClickAction::None)
    }

    /// Return the number of registered actions.
    pub fn action_count(&self) -> usize {
        self.actions.len()
    }

    /// Return all item IDs that have a `RunCommand` action.
    pub fn command_items(&self) -> Vec<&str> {
        self.actions
            .iter()
            .filter_map(|(id, a)| match a {
                ClickAction::RunCommand { .. } => Some(id.as_str()),
                _ => Option::None,
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// TooltipManager — rich tooltip content for items
// ---------------------------------------------------------------------------

/// Manages rich tooltip content for status bar items.
#[derive(Debug, Clone, Default)]
pub struct TooltipManager {
    tooltips: Vec<(String, TooltipContent)>,
}

/// Rich tooltip content that can include multiple lines and a title.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TooltipContent {
    pub title: Option<String>,
    pub lines: Vec<String>,
}

impl TooltipContent {
    pub fn simple(text: impl Into<String>) -> Self {
        Self {
            title: None,
            lines: vec![text.into()],
        }
    }

    pub fn titled(title: impl Into<String>, lines: Vec<String>) -> Self {
        Self {
            title: Some(title.into()),
            lines,
        }
    }

    /// Render the tooltip as a single multi-line string.
    pub fn render(&self) -> String {
        let mut parts = Vec::new();
        if let Some(ref t) = self.title {
            parts.push(t.clone());
            parts.push("─".repeat(t.chars().count()));
        }
        parts.extend(self.lines.iter().cloned());
        parts.join("\n")
    }

    /// Total number of lines in the rendered tooltip.
    pub fn line_count(&self) -> usize {
        let header = if self.title.is_some() { 2 } else { 0 };
        header + self.lines.len()
    }
}

impl fmt::Display for TooltipContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render())
    }
}

impl TooltipManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set tooltip content for an item.
    pub fn set(&mut self, item_id: impl Into<String>, content: TooltipContent) {
        let id = item_id.into();
        if let Some(entry) = self.tooltips.iter_mut().find(|(k, _)| k == &id) {
            entry.1 = content;
        } else {
            self.tooltips.push((id, content));
        }
    }

    /// Get tooltip content for an item.
    pub fn get(&self, item_id: &str) -> Option<&TooltipContent> {
        self.tooltips.iter().find(|(k, _)| k == item_id).map(|(_, v)| v)
    }

    /// Remove tooltip content for an item.
    pub fn remove(&mut self, item_id: &str) {
        self.tooltips.retain(|(k, _)| k != item_id);
    }

    /// Number of items with tooltip content.
    pub fn count(&self) -> usize {
        self.tooltips.len()
    }
}

/// Render a compact one-line summary of a set of status bar items.
pub fn render_summary(items: &[StatusBarItem]) -> String {
    let visible = items.iter().filter(|i| i.is_visible).count();
    let total = items.len();
    let left = items.iter().filter(|i| i.alignment == StatusBarAlignment::Left).count();
    let right = items.iter().filter(|i| i.alignment == StatusBarAlignment::Right).count();
    format!("{}/{} visible, {} left, {} right", visible, total, left, right)
}

// ---------------------------------------------------------------------------
// StatusbarPriorityManager - statusbar priority manager
// ---------------------------------------------------------------------------

/// Severity level for statusbar priority manager issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StatusbarPriorityManagerSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for StatusbarPriorityManagerSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [StatusbarPriorityManager].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusbarPriorityManagerEntry {
    pub id: String,
    pub label: String,
    pub severity: StatusbarPriorityManagerSeverity,
    pub detail: Option<String>,
    pub item_count: usize,
    enabled: bool,
}

impl StatusbarPriorityManagerEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: StatusbarPriorityManagerSeverity::Low,
            detail: None,
            item_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: StatusbarPriorityManagerSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_item_count(mut self, val: usize) -> Self {
        self.item_count = val;
        self
    }

    pub fn has_items(&self) -> bool {
        self.enabled && self.severity >= StatusbarPriorityManagerSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.item_count, det)
    }
}

impl fmt::Display for StatusbarPriorityManagerEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [StatusbarPriorityManagerEntry] items.
#[derive(Debug, Clone)]
pub struct StatusbarPriorityManager {
    entries: Vec<StatusbarPriorityManagerEntry>,
    name: String,
    capacity: usize,
}

impl StatusbarPriorityManager {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: StatusbarPriorityManagerEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<StatusbarPriorityManagerEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&StatusbarPriorityManagerEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn item_count(&self) -> usize { self.entries.len() }

    pub fn has_items(&self) -> bool {
        self.entries.iter().any(|e| e.has_items())
    }

    pub fn entries_by_severity(&self, severity: StatusbarPriorityManagerSeverity) -> Vec<&StatusbarPriorityManagerEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= StatusbarPriorityManagerSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&StatusbarPriorityManagerEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&StatusbarPriorityManagerEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// StatusbarTooltipBuilder - statusbar tooltip builder
// ---------------------------------------------------------------------------

/// Configuration for [StatusbarTooltipBuilder].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusbarTooltipBuilderConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub max_priority: usize,
}

impl StatusbarTooltipBuilderConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, max_priority: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_max_priority(mut self, val: usize) -> Self { self.max_priority = val; self }
}

impl Default for StatusbarTooltipBuilderConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [StatusbarTooltipBuilder].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusbarTooltipBuilderItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl StatusbarTooltipBuilderItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn has_tooltip(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for StatusbarTooltipBuilderItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [StatusbarTooltipBuilderItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct StatusbarTooltipBuilder {
    config: StatusbarTooltipBuilderConfig,
    items: Vec<StatusbarTooltipBuilderItem>,
}

impl StatusbarTooltipBuilder {
    pub fn new(config: StatusbarTooltipBuilderConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: StatusbarTooltipBuilderItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<StatusbarTooltipBuilderItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&StatusbarTooltipBuilderItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn max_priority(&self) -> usize { self.items.len() }

    pub fn has_tooltip(&self) -> bool {
        self.items.iter().any(|i| i.has_tooltip())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&StatusbarTooltipBuilderItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&StatusbarTooltipBuilderItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &StatusbarTooltipBuilderConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



/// Configuration manager for ext_statusbar functionality.
pub struct ExtStatusbarConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl ExtStatusbarConfig {
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

    pub fn merge(&mut self, other: &ExtStatusbarConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for ext_statusbar operations.
pub struct ExtStatusbarRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl ExtStatusbarRateTracker {
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

/// Validation result collector for ext_statusbar.
pub struct ExtStatusbarValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl ExtStatusbarValidationCollector {
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

    pub fn merge(&mut self, other: &ExtStatusbarValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Status bar item registration — extended utilities (yk)
// ---------------------------------------------------------------------------

/// Metric accumulator for ext_sb operations.
#[derive(Debug, Clone)]
pub struct YkMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YkMetrics {
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

/// Sliding-window rate counter for ext_sb.
#[derive(Debug, Clone)]
pub struct YkRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YkRateWindow {
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

/// A small LRU-style cache for ext_sb lookups.
#[derive(Debug, Clone)]
pub struct YkLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YkLruCache {
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
// xa_ extended helpers for ext_statusbar
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaExtStatusbarRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaExtStatusbarRingBuf {
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
pub struct XaExtStatusbarCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaExtStatusbarCounter {
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

impl Default for XaExtStatusbarCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 70
// ---------------------------------------------------------------------------

/// Generic object pool `Xc70Pool<T>`.
pub struct Xc70Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc70Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc70PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc70Pool<T> {
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
    pub fn stats(&self) -> Xc70PoolStats {
        Xc70PoolStats {
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

impl<T> Default for Xc70Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc70Scheduler`.
pub struct Xc70Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc70Scheduler {
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

impl Default for Xc70Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_70 hash for the given byte slice.
pub fn xc_70_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_70 convention.
pub fn xc_70_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_90 deepening: state machine + event bus ---

/// States for the Xd90 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd90State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd90State {
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
pub struct Xd90Transition {
    pub from: Xd90State,
    pub to: Xd90State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd90StateMachine {
    current: Xd90State,
    history: Vec<Xd90Transition>,
    step_counter: usize,
}

impl Xd90StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd90State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd90State {
        self.current
    }

    pub fn history(&self) -> &[Xd90Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd90State) -> Result<Xd90State, String> {
        let allowed = match (self.current, target) {
            (Xd90State::Idle, Xd90State::Running) => true,
            (Xd90State::Running, Xd90State::Paused) => true,
            (Xd90State::Running, Xd90State::Done) => true,
            (Xd90State::Paused, Xd90State::Running) => true,
            (Xd90State::Paused, Xd90State::Done) => true,
            (Xd90State::Done, Xd90State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_90: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd90Transition {
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
            "Xd90SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd90State> {
        let prefix = "Xd90SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd90State::Idle),
            "Running" => Some(Xd90State::Running),
            "Paused" => Some(Xd90State::Paused),
            "Done" => Some(Xd90State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd90State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd90 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd90Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd90Event {
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

type Xd90HandlerFn = Box<dyn Fn(&Xd90Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd90EventBus {
    handlers: Vec<(usize, Option<String>, Xd90HandlerFn)>,
    next_id: usize,
    published: Vec<Xd90Event>,
}

impl Xd90EventBus {
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
        F: Fn(&Xd90Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd90Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd90Event) {
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

    pub fn published_events(&self) -> &[Xd90Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = StatusBarMessage::CreateItem {
            id: "sb1".into(),
            alignment: StatusBarAlignment::Left,
            priority: 100,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: StatusBarMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn item_serialization() {
        let item = StatusBarItem {
            id: "sb1".into(),
            text: "$(sync~spin)".into(),
            tooltip: Some("Syncing".into()),
            command: Some("sync.run".into()),
            alignment: StatusBarAlignment::Right,
            priority: 50,
            is_visible: true,
        };
        let json = serde_json::to_string(&item).unwrap();
        let back: StatusBarItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, back);
    }

    #[test]
    fn bridge_create_show_hide() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("sb1", StatusBarAlignment::Left, 100);
        assert!(!bridge.get_item("sb1").unwrap().is_visible);
        bridge.show_item("sb1");
        assert!(bridge.get_item("sb1").unwrap().is_visible);
        bridge.hide_item("sb1");
        assert!(!bridge.get_item("sb1").unwrap().is_visible);
    }

    #[test]
    fn bridge_dispose() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("sb1", StatusBarAlignment::Right, 0);
        bridge.dispose_item("sb1");
        assert!(bridge.get_item("sb1").is_none());
    }

    #[test]
    fn bridge_update_text() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("sb1", StatusBarAlignment::Left, 0);
        let msg = StatusBarMessage::UpdateItem {
            id: "sb1".into(),
            text: Some("Ready".into()),
            tooltip: None,
            command: None,
        };
        bridge.handle_message(&msg);
        assert_eq!(bridge.get_item("sb1").unwrap().text, "Ready");
    }

    #[test]
    fn error_display() {
        let err = StatusBarError::ItemNotFound("sb99".into());
        assert_eq!(err.to_string(), "status bar item not found: sb99");

        let err = StatusBarError::DuplicateItem("sb1".into());
        assert_eq!(err.to_string(), "status bar item already exists: sb1");

        let err = StatusBarError::InvalidField {
            field: "id",
            reason: "empty".into(),
        };
        assert_eq!(err.to_string(), "invalid field 'id': empty");
    }

    #[test]
    fn try_create_duplicate() {
        let mut bridge = StatusBarBridge::new();
        bridge.try_create_item("sb1", StatusBarAlignment::Left, 0).unwrap();
        let err = bridge.try_create_item("sb1", StatusBarAlignment::Left, 0).unwrap_err();
        assert_eq!(err, StatusBarError::DuplicateItem("sb1".into()));
    }

    #[test]
    fn update_item_not_found() {
        let mut bridge = StatusBarBridge::new();
        let err = bridge.update_item("nope", Some("x"), None, None).unwrap_err();
        assert_eq!(err, StatusBarError::ItemNotFound("nope".into()));
    }

    #[test]
    fn items_by_alignment_sorted() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("a", StatusBarAlignment::Left, 10);
        bridge.create_item("b", StatusBarAlignment::Left, 50);
        bridge.create_item("c", StatusBarAlignment::Right, 30);
        let left = bridge.items_by_alignment(StatusBarAlignment::Left);
        assert_eq!(left.len(), 2);
        assert_eq!(left[0].id, "b"); // higher priority first
        assert_eq!(left[1].id, "a");
    }

    #[test]
    fn visible_items_and_counts() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("a", StatusBarAlignment::Left, 10);
        bridge.create_item("b", StatusBarAlignment::Right, 20);
        assert_eq!(bridge.item_count(), 2);
        assert_eq!(bridge.visible_count(), 0);
        bridge.show_item("b");
        assert_eq!(bridge.visible_count(), 1);
        let vis = bridge.visible_items();
        assert_eq!(vis.len(), 1);
        assert_eq!(vis[0].id, "b");
    }

    #[test]
    fn dispose_all() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("a", StatusBarAlignment::Left, 0);
        bridge.create_item("b", StatusBarAlignment::Right, 0);
        assert_eq!(bridge.dispose_all(), 2);
        assert_eq!(bridge.item_count(), 0);
    }

    #[test]
    fn builder_success() {
        let item = StatusBarItemBuilder::new()
            .id("sb1")
            .text("Hello")
            .tooltip("A tooltip")
            .command("cmd.run")
            .alignment(StatusBarAlignment::Right)
            .priority(42)
            .visible(true)
            .build()
            .unwrap();
        assert_eq!(item.id, "sb1");
        assert_eq!(item.text, "Hello");
        assert_eq!(item.tooltip.as_deref(), Some("A tooltip"));
        assert_eq!(item.command.as_deref(), Some("cmd.run"));
        assert_eq!(item.alignment, StatusBarAlignment::Right);
        assert_eq!(item.priority, 42);
        assert!(item.is_visible);
    }

    #[test]
    fn builder_missing_id() {
        let err = StatusBarItemBuilder::new().text("x").build().unwrap_err();
        assert!(matches!(err, StatusBarError::InvalidField { field: "id", .. }));
    }

    #[test]
    fn builder_empty_id() {
        let err = StatusBarItemBuilder::new().id("").build().unwrap_err();
        assert!(matches!(err, StatusBarError::InvalidField { field: "id", .. }));
    }

    #[test]
    fn item_display_text_fallback() {
        let item = StatusBarItemBuilder::new()
            .id("sb1")
            .build()
            .unwrap();
        assert_eq!(item.display_text(), "sb1"); // falls back to id
        assert!(!item.has_command());
    }

    #[test]
    fn item_display_trait() {
        let item = StatusBarItemBuilder::new()
            .id("sb1")
            .text("Running")
            .visible(true)
            .build()
            .unwrap();
        let s = format!("{item}");
        assert_eq!(s, "[sb1] Running (visible)");
    }

    #[test]
    fn alignment_display() {
        assert_eq!(StatusBarAlignment::Left.to_string(), "Left");
        assert_eq!(StatusBarAlignment::Right.to_string(), "Right");
    }

    #[test]
    fn ext_statusbar_stats_new_defaults() {
        let stats = ExtStatusbarStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_statusbar_stats_record_success() {
        let mut stats = ExtStatusbarStats::new();
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
    fn ext_statusbar_stats_record_failure() {
        let mut stats = ExtStatusbarStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_statusbar_stats_reset() {
        let mut stats = ExtStatusbarStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_statusbar_stats_merge() {
        let mut a = ExtStatusbarStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtStatusbarStats::new();
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
    fn ext_statusbar_stats_display() {
        let mut stats = ExtStatusbarStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_statusbar_stats_default() {
        let stats = ExtStatusbarStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn extstatusbar_validator_accepts_and_rejects() {
        let mut v = ExtStatusbarValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn extstatusbar_validator_warnings() {
        let mut v = ExtStatusbarValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn extstatusbar_validator_clear_and_merge() {
        let mut v = ExtStatusbarValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = ExtStatusbarValidationCollector::new();
        a.add_error("a_err");
        let mut b = ExtStatusbarValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
    }

    #[test]
    fn sort_items_by_priority_descending() {
        let mut items = vec![
            StatusBarItemBuilder::new().id("a").priority(10).build().unwrap(),
            StatusBarItemBuilder::new().id("b").priority(50).build().unwrap(),
            StatusBarItemBuilder::new().id("c").priority(30).build().unwrap(),
        ];
        sort_items_by_priority(&mut items);
        assert_eq!(items[0].id, "b");
        assert_eq!(items[1].id, "c");
        assert_eq!(items[2].id, "a");
    }

    #[test]
    fn sort_items_by_priority_stable_tie() {
        let mut items = vec![
            StatusBarItemBuilder::new().id("beta").priority(10).build().unwrap(),
            StatusBarItemBuilder::new().id("alpha").priority(10).build().unwrap(),
        ];
        sort_items_by_priority(&mut items);
        assert_eq!(items[0].id, "alpha");
        assert_eq!(items[1].id, "beta");
    }

    #[test]
    fn sorted_by_priority_nonmutating() {
        let items = vec![
            StatusBarItemBuilder::new().id("a").priority(5).build().unwrap(),
            StatusBarItemBuilder::new().id("b").priority(20).build().unwrap(),
        ];
        let sorted = sorted_by_priority(&items);
        assert_eq!(sorted[0].id, "b");
        assert_eq!(items[0].id, "a"); // original unchanged
    }

    #[test]
    fn visibility_manager_hide_show() {
        let mut mgr = StatusBarVisibilityManager::new();
        mgr.hide_namespace("git");
        assert!(mgr.is_namespace_hidden("git"));
        mgr.show_namespace("git");
        assert!(!mgr.is_namespace_hidden("git"));
    }

    #[test]
    fn visibility_manager_filter() {
        let mut mgr = StatusBarVisibilityManager::new();
        mgr.hide_namespace("git");
        let items = vec![
            StatusBarItemBuilder::new().id("git.branch").build().unwrap(),
            StatusBarItemBuilder::new().id("rust.status").build().unwrap(),
            StatusBarItemBuilder::new().id("git.sync").build().unwrap(),
        ];
        let visible = mgr.filter_visible(&items);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, "rust.status");
    }

    #[test]
    fn visibility_manager_extract_namespace() {
        assert_eq!(StatusBarVisibilityManager::extract_namespace("git.branch"), "git");
        assert_eq!(StatusBarVisibilityManager::extract_namespace("standalone"), "standalone");
    }

    #[test]
    fn status_bar_layout_splits_and_sorts() {
        let items = vec![
            StatusBarItemBuilder::new().id("l1").alignment(StatusBarAlignment::Left).priority(10).visible(true).build().unwrap(),
            StatusBarItemBuilder::new().id("r1").alignment(StatusBarAlignment::Right).priority(20).visible(true).build().unwrap(),
            StatusBarItemBuilder::new().id("l2").alignment(StatusBarAlignment::Left).priority(30).visible(true).build().unwrap(),
            StatusBarItemBuilder::new().id("hidden").alignment(StatusBarAlignment::Left).priority(100).visible(false).build().unwrap(),
        ];
        let layout = status_bar_layout(&items);
        assert_eq!(layout.left_items, vec!["l2", "l1"]);
        assert_eq!(layout.right_items, vec!["r1"]);
    }

    #[test]
    fn status_bar_layout_empty() {
        let layout = status_bar_layout(&[]);
        assert!(layout.left_items.is_empty());
        assert!(layout.right_items.is_empty());
    }

    #[test]
    fn find_by_text_matches() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("a", StatusBarAlignment::Left, 0);
        bridge.create_item("b", StatusBarAlignment::Left, 0);
        bridge.create_item("c", StatusBarAlignment::Right, 0);
        bridge.update_item("a", Some("Hello World"), None, None).unwrap();
        bridge.update_item("b", Some("Hello Rust"), None, None).unwrap();
        bridge.update_item("c", Some("Goodbye"), None, None).unwrap();
        let found = bridge.find_by_text("Hello");
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|i| i.text.contains("Hello")));
        let empty = bridge.find_by_text("missing");
        assert!(empty.is_empty());
    }

    #[test]
    fn sorted_items_by_priority() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("low", StatusBarAlignment::Left, 1);
        bridge.create_item("high", StatusBarAlignment::Right, 100);
        bridge.create_item("mid", StatusBarAlignment::Left, 50);
        let sorted = bridge.sorted_items();
        assert_eq!(sorted[0].id, "high");
        assert_eq!(sorted[1].id, "mid");
        assert_eq!(sorted[2].id, "low");
    }

    #[test]
    fn age_description_formatting() {
        let item = StatusBarItemBuilder::new().id("x").text("A").build().unwrap();
        assert_eq!(item.age_description(), "1 item");
        let item2 = StatusBarItemBuilder::new().id("y").text("ABC").build().unwrap();
        assert_eq!(item2.age_description(), "3 items");
        let item3 = StatusBarItemBuilder::new().id("z").build().unwrap();
        assert_eq!(item3.age_description(), "0 items");
    }

    #[test]
    fn alignment_is_left_is_right() {
        assert!(StatusBarAlignment::Left.is_left());
        assert!(!StatusBarAlignment::Left.is_right());
        assert!(StatusBarAlignment::Right.is_right());
        assert!(!StatusBarAlignment::Right.is_left());
    }

    #[test]
    fn toggle_visibility_works() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("sb1", StatusBarAlignment::Left, 0);
        assert!(!bridge.get_item("sb1").unwrap().is_visible);
        bridge.toggle_visibility("sb1").unwrap();
        assert!(bridge.get_item("sb1").unwrap().is_visible);
        bridge.toggle_visibility("sb1").unwrap();
        assert!(!bridge.get_item("sb1").unwrap().is_visible);
    }

    #[test]
    fn toggle_visibility_not_found() {
        let mut bridge = StatusBarBridge::new();
        let err = bridge.toggle_visibility("nope").unwrap_err();
        assert_eq!(err, StatusBarError::ItemNotFound("nope".into()));
    }

    #[test]
    fn get_items_with_command_filters() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("a", StatusBarAlignment::Left, 0);
        bridge.create_item("b", StatusBarAlignment::Left, 0);
        bridge.create_item("c", StatusBarAlignment::Right, 0);
        bridge.update_item("a", None, None, Some("cmd.a")).unwrap();
        bridge.update_item("c", None, None, Some("cmd.c")).unwrap();
        let with_cmd = bridge.get_items_with_command();
        assert_eq!(with_cmd.len(), 2);
        assert!(with_cmd.iter().any(|i| i.id == "a"));
        assert!(with_cmd.iter().any(|i| i.id == "c"));
    }

    // ---- StatusBarGroup tests ----

    #[test]
    fn status_bar_group_add_remove() {
        let mut group = StatusBarGroup::new("git");
        group.add_item("git.branch");
        group.add_item("git.sync");
        assert_eq!(group.item_count(), 2);
        assert!(group.contains("git.branch"));
        group.remove_item("git.branch");
        assert_eq!(group.item_count(), 1);
        assert!(!group.contains("git.branch"));
    }

    #[test]
    fn group_manager_show_hide() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("git.branch", StatusBarAlignment::Left, 10);
        bridge.create_item("git.sync", StatusBarAlignment::Left, 5);
        bridge.create_item("lsp.status", StatusBarAlignment::Right, 8);

        let mut mgr = StatusBarGroupManager::new();
        let mut git_group = StatusBarGroup::new("git");
        git_group.add_item("git.branch");
        git_group.add_item("git.sync");
        mgr.add_group(git_group);

        mgr.show_group("git", &mut bridge);
        assert!(bridge.get_item("git.branch").unwrap().is_visible);
        assert!(bridge.get_item("git.sync").unwrap().is_visible);

        mgr.hide_group("git", &mut bridge);
        assert!(!bridge.get_item("git.branch").unwrap().is_visible);
        assert!(!bridge.get_item("git.sync").unwrap().is_visible);
    }

    #[test]
    fn animated_item_progress() {
        let item = StatusBarItemBuilder::new()
            .id("build")
            .text("Building...")
            .build()
            .unwrap();
        let mut animated = AnimatedStatusBarItem::new(item);
        assert!(!animated.is_animating());

        animated.set_progress(0.5);
        assert!(animated.is_animating());
        assert_eq!(animated.animation, AnimationState::Spinning);
        let rendered = animated.render_text();
        assert!(rendered.contains("50%"));

        animated.clear_progress();
        assert!(!animated.is_animating());
    }

    #[test]
    fn bulk_show_hide_all() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("a", StatusBarAlignment::Left, 1);
        bridge.create_item("b", StatusBarAlignment::Right, 2);
        bridge.create_item("c", StatusBarAlignment::Left, 3);

        let shown = bridge.show_all();
        assert_eq!(shown, 3);
        assert_eq!(bridge.visible_count(), 3);

        let hidden = bridge.hide_all();
        assert_eq!(hidden, 3);
        assert_eq!(bridge.visible_count(), 0);
    }

    #[test]
    fn bulk_show_by_alignment() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("a", StatusBarAlignment::Left, 1);
        bridge.create_item("b", StatusBarAlignment::Right, 2);
        bridge.create_item("c", StatusBarAlignment::Left, 3);

        bridge.show_by_alignment(StatusBarAlignment::Left);
        assert!(bridge.get_item("a").unwrap().is_visible);
        assert!(!bridge.get_item("b").unwrap().is_visible);
        assert!(bridge.get_item("c").unwrap().is_visible);
    }

    #[test]
    fn group_manager_toggle() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("x", StatusBarAlignment::Left, 1);

        let mut mgr = StatusBarGroupManager::new();
        let mut group = StatusBarGroup::new("test");
        group.add_item("x");
        mgr.add_group(group);

        mgr.toggle_group("test", &mut bridge);
        assert!(!bridge.get_item("x").unwrap().is_visible);

        mgr.toggle_group("test", &mut bridge);
        assert!(bridge.get_item("x").unwrap().is_visible);
    }

    // -- New tests ----------------------------------------------------------

    #[test]
    fn create_items_batch() {
        let mut bridge = StatusBarBridge::new();
        let specs = vec![
            ("a", StatusBarAlignment::Left, 10),
            ("b", StatusBarAlignment::Right, 5),
            ("a", StatusBarAlignment::Left, 20), // duplicate
        ];
        let created = bridge.create_items(&specs);
        assert_eq!(created, 2);
        assert_eq!(bridge.item_count(), 2);
    }

    #[test]
    fn render_text_layout() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("branch", StatusBarAlignment::Left, 10);
        bridge.create_item("line", StatusBarAlignment::Right, 5);
        bridge.show_item("branch");
        bridge.show_item("line");
        let _ = bridge.update_item("branch", Some("main"), None, None);
        let _ = bridge.update_item("line", Some("Ln 42"), None, None);

        let text = bridge.render_text();
        assert!(text.contains("main"));
        assert!(text.contains("Ln 42"));
        assert!(text.contains("|"));
    }

    #[test]
    fn find_by_command_returns_matches() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("a", StatusBarAlignment::Left, 1);
        bridge.create_item("b", StatusBarAlignment::Left, 2);
        let _ = bridge.update_item("a", None, None, Some("editor.action.format"));
        let _ = bridge.update_item("b", None, None, Some("editor.action.format"));

        let found = bridge.find_by_command("editor.action.format");
        assert_eq!(found.len(), 2);
        assert!(bridge.find_by_command("nonexistent").is_empty());
    }

    #[test]
    fn to_json_returns_array() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("git", StatusBarAlignment::Left, 10);
        bridge.show_item("git");
        let _ = bridge.update_item("git", Some("main"), None, None);

        let json = bridge.to_json();
        let arr = json.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], "git");
        assert_eq!(arr[0]["text"], "main");
    }

    #[test]
    fn set_priority_updates_item() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("x", StatusBarAlignment::Left, 1);
        assert!(bridge.set_priority("x", 100).is_ok());
        assert_eq!(bridge.get_item("x").unwrap().priority, 100);
        assert!(bridge.set_priority("missing", 1).is_err());
    }

    #[test]
    fn render_summary_output() {
        let items = vec![
            StatusBarItemBuilder::new().id("a").visible(true).alignment(StatusBarAlignment::Left).build().unwrap(),
            StatusBarItemBuilder::new().id("b").visible(false).alignment(StatusBarAlignment::Right).build().unwrap(),
        ];
        let s = render_summary(&items);
        assert!(s.contains("1/2 visible"));
        assert!(s.contains("1 left"));
        assert!(s.contains("1 right"));
    }

    // -- StatusBarRenderer tests -------------------------------------------

    #[test]
    fn renderer_truncate_text_short() {
        let r = StatusBarRenderer::new(80);
        assert_eq!(r.truncate_text("hello", 10), "hello");
    }

    #[test]
    fn renderer_truncate_text_long() {
        let r = StatusBarRenderer::new(80);
        let result = r.truncate_text("hello world", 6);
        assert_eq!(result, "hello…");
        assert_eq!(result.chars().count(), 6);
    }

    #[test]
    fn renderer_render_fits_in_width() {
        let r = StatusBarRenderer::new(120);
        let items = vec![
            StatusBarItemBuilder::new().id("branch").text("main").alignment(StatusBarAlignment::Left).priority(10).visible(true).build().unwrap(),
            StatusBarItemBuilder::new().id("line").text("Ln 42").alignment(StatusBarAlignment::Right).priority(5).visible(true).build().unwrap(),
        ];
        let out = r.render(&items);
        assert!(out.contains("main"));
        assert!(out.contains("Ln 42"));
        assert!(out.chars().count() <= 120);
    }

    #[test]
    fn renderer_render_truncates_to_width() {
        let r = StatusBarRenderer::new(20);
        let items = vec![
            StatusBarItemBuilder::new().id("a").text("A very long left item").alignment(StatusBarAlignment::Left).priority(10).visible(true).build().unwrap(),
            StatusBarItemBuilder::new().id("b").text("A very long right item").alignment(StatusBarAlignment::Right).priority(5).visible(true).build().unwrap(),
        ];
        let out = r.render(&items);
        assert!(out.chars().count() <= 20);
    }

    #[test]
    fn renderer_hides_invisible_items() {
        let r = StatusBarRenderer::new(80);
        let items = vec![
            StatusBarItemBuilder::new().id("vis").text("Visible").alignment(StatusBarAlignment::Left).priority(10).visible(true).build().unwrap(),
            StatusBarItemBuilder::new().id("hid").text("Hidden").alignment(StatusBarAlignment::Left).priority(20).visible(false).build().unwrap(),
        ];
        let out = r.render(&items);
        assert!(out.contains("Visible"));
        assert!(!out.contains("Hidden"));
    }

    // -- StatusBarColorStyle tests -----------------------------------------

    #[test]
    fn color_style_validate_good() {
        let style = StatusBarColorStyle::fg_bg("#ff0000", "#00ff00");
        assert!(style.validate().is_ok());
    }

    #[test]
    fn color_style_validate_bad() {
        let style = StatusBarColorStyle::fg("red");
        assert!(style.validate().is_err());
    }

    #[test]
    fn color_style_merge() {
        let base = StatusBarColorStyle::fg("#ffffff").with_bold();
        let overlay = StatusBarColorStyle {
            foreground: None,
            background: Some("#000000".into()),
            bold: false,
            italic: true,
        };
        let merged = base.merge(&overlay);
        assert_eq!(merged.foreground.as_deref(), Some("#ffffff"));
        assert_eq!(merged.background.as_deref(), Some("#000000"));
        assert!(merged.bold);
        assert!(merged.italic);
    }

    #[test]
    fn color_style_display() {
        let style = StatusBarColorStyle::fg("#aabbcc").with_bold();
        let s = format!("{style}");
        assert!(s.contains("#aabbcc"));
        assert!(s.contains("bold"));
    }

    #[test]
    fn color_style_serialization_roundtrip() {
        let style = StatusBarColorStyle::fg_bg("#112233", "#445566").with_italic();
        let json = serde_json::to_string(&style).unwrap();
        let back: StatusBarColorStyle = serde_json::from_str(&json).unwrap();
        assert_eq!(style, back);
    }

    // -- ClickAction / ClickActionDispatcher tests -------------------------

    #[test]
    fn click_action_dispatch_registered() {
        let mut dispatcher = ClickActionDispatcher::new();
        dispatcher.register("git.branch", ClickAction::RunCommand {
            command: "git.checkout".into(),
            args: vec![],
        });
        let action = dispatcher.dispatch("git.branch");
        assert!(matches!(action, ClickAction::RunCommand { .. }));
        assert_eq!(dispatcher.action_count(), 1);
    }

    #[test]
    fn click_action_dispatch_unregistered() {
        let dispatcher = ClickActionDispatcher::new();
        assert_eq!(dispatcher.dispatch("unknown"), ClickAction::None);
    }

    #[test]
    fn click_action_unregister() {
        let mut dispatcher = ClickActionDispatcher::new();
        dispatcher.register("x", ClickAction::OpenUrl { url: "https://example.com".into() });
        assert_eq!(dispatcher.action_count(), 1);
        dispatcher.unregister("x");
        assert_eq!(dispatcher.action_count(), 0);
    }

    #[test]
    fn click_action_command_items() {
        let mut dispatcher = ClickActionDispatcher::new();
        dispatcher.register("a", ClickAction::RunCommand { command: "cmd.a".into(), args: vec![] });
        dispatcher.register("b", ClickAction::OpenUrl { url: "https://b.com".into() });
        dispatcher.register("c", ClickAction::RunCommand { command: "cmd.c".into(), args: vec!["--flag".into()] });
        let cmds = dispatcher.command_items();
        assert_eq!(cmds.len(), 2);
        assert!(cmds.contains(&"a"));
        assert!(cmds.contains(&"c"));
    }

    #[test]
    fn click_action_display() {
        let a = ClickAction::RunCommand { command: "foo".into(), args: vec!["bar".into()] };
        assert_eq!(format!("{a}"), "cmd:foo(bar)");
        let b = ClickAction::OpenUrl { url: "https://x.com".into() };
        assert_eq!(format!("{b}"), "url:https://x.com");
        assert_eq!(format!("{}", ClickAction::None), "none");
    }

    #[test]
    fn click_action_serialization_roundtrip() {
        let action = ClickAction::ShowQuickPick { items: vec!["a".into(), "b".into()] };
        let json = serde_json::to_string(&action).unwrap();
        let back: ClickAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, back);
    }

    // -- TooltipManager / TooltipContent tests -----------------------------

    #[test]
    fn tooltip_simple_render() {
        let tc = TooltipContent::simple("Hello tooltip");
        assert_eq!(tc.render(), "Hello tooltip");
        assert_eq!(tc.line_count(), 1);
    }

    #[test]
    fn tooltip_titled_render() {
        let tc = TooltipContent::titled("Git Branch", vec!["main".into(), "3 commits ahead".into()]);
        let rendered = tc.render();
        assert!(rendered.contains("Git Branch"));
        assert!(rendered.contains("main"));
        assert!(rendered.contains("3 commits ahead"));
        assert_eq!(tc.line_count(), 4); // title + separator + 2 lines
    }

    #[test]
    fn tooltip_manager_set_get_remove() {
        let mut mgr = TooltipManager::new();
        mgr.set("branch", TooltipContent::simple("Current branch: main"));
        assert_eq!(mgr.count(), 1);
        let tt = mgr.get("branch").unwrap();
        assert_eq!(tt.lines[0], "Current branch: main");
        mgr.remove("branch");
        assert_eq!(mgr.count(), 0);
        assert!(mgr.get("branch").is_none());
    }

    #[test]
    fn tooltip_manager_overwrite() {
        let mut mgr = TooltipManager::new();
        mgr.set("x", TooltipContent::simple("first"));
        mgr.set("x", TooltipContent::simple("second"));
        assert_eq!(mgr.count(), 1);
        assert_eq!(mgr.get("x").unwrap().lines[0], "second");
    }

    #[test]
    fn tooltip_content_serialization_roundtrip() {
        let tc = TooltipContent::titled("Info", vec!["line1".into(), "line2".into()]);
        let json = serde_json::to_string(&tc).unwrap();
        let back: TooltipContent = serde_json::from_str(&json).unwrap();
        assert_eq!(tc, back);
    }

#[test]
    fn statusbarprioritymanager_severity_ordering() {
        assert!(StatusbarPriorityManagerSeverity::Critical > StatusbarPriorityManagerSeverity::High);
        assert!(StatusbarPriorityManagerSeverity::High > StatusbarPriorityManagerSeverity::Medium);
        assert!(StatusbarPriorityManagerSeverity::Medium > StatusbarPriorityManagerSeverity::Low);
    }

    #[test]
    fn statusbarprioritymanager_severity_display() {
        assert_eq!(StatusbarPriorityManagerSeverity::Low.to_string(), "low");
        assert_eq!(StatusbarPriorityManagerSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn statusbarprioritymanager_entry_creation() {
        let e = StatusbarPriorityManagerEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, StatusbarPriorityManagerSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn statusbarprioritymanager_entry_builder() {
        let e = StatusbarPriorityManagerEntry::new("e2", "Entry 2")
            .with_severity(StatusbarPriorityManagerSeverity::High)
            .with_detail("some detail")
            .with_item_count(42);
        assert_eq!(e.severity, StatusbarPriorityManagerSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.item_count, 42);
    }

    #[test]
    fn statusbarprioritymanager_entry_enable_disable() {
        let mut e = StatusbarPriorityManagerEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn statusbarprioritymanager_add_and_count() {
        let mut mgr = StatusbarPriorityManager::new("test");
        mgr.add(StatusbarPriorityManagerEntry::new("a", "A"));
        mgr.add(StatusbarPriorityManagerEntry::new("b", "B").with_severity(StatusbarPriorityManagerSeverity::High));
        assert_eq!(mgr.item_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn statusbarprioritymanager_remove() {
        let mut mgr = StatusbarPriorityManager::new("test");
        mgr.add(StatusbarPriorityManagerEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn statusbarprioritymanager_capacity() {
        let mut mgr = StatusbarPriorityManager::new("test").with_capacity(1);
        assert!(mgr.add(StatusbarPriorityManagerEntry::new("a", "A")));
        assert!(!mgr.add(StatusbarPriorityManagerEntry::new("b", "B")));
    }

    #[test]
    fn statusbarprioritymanager_sorted_by_severity() {
        let mut mgr = StatusbarPriorityManager::new("test");
        mgr.add(StatusbarPriorityManagerEntry::new("lo", "Low"));
        mgr.add(StatusbarPriorityManagerEntry::new("hi", "High").with_severity(StatusbarPriorityManagerSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, StatusbarPriorityManagerSeverity::Critical);
    }

    #[test]
    fn statusbarprioritymanager_summary() {
        let mgr = StatusbarPriorityManager::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn statusbartooltipbuilder_config_defaults() {
        let cfg = StatusbarTooltipBuilderConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn statusbartooltipbuilder_item_creation() {
        let item = StatusbarTooltipBuilderItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn statusbartooltipbuilder_add_and_get() {
        let mut mgr = StatusbarTooltipBuilder::new(StatusbarTooltipBuilderConfig::new("test"));
        mgr.add(StatusbarTooltipBuilderItem::new("k1", "v1"));
        assert_eq!(mgr.max_priority(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn statusbartooltipbuilder_remove_item() {
        let mut mgr = StatusbarTooltipBuilder::new(StatusbarTooltipBuilderConfig::new("test"));
        mgr.add(StatusbarTooltipBuilderItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn statusbartooltipbuilder_sorted_by_priority() {
        let mut mgr = StatusbarTooltipBuilder::new(StatusbarTooltipBuilderConfig::new("test"));
        mgr.add(StatusbarTooltipBuilderItem::new("lo", "low").with_priority(1));
        mgr.add(StatusbarTooltipBuilderItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn statusbartooltipbuilder_items_with_tag() {
        let mut mgr = StatusbarTooltipBuilder::new(StatusbarTooltipBuilderConfig::new("test"));
        mgr.add(StatusbarTooltipBuilderItem::new("a", "1").with_tag("x"));
        mgr.add(StatusbarTooltipBuilderItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn statusbartooltipbuilder_report() {
        let mgr = StatusbarTooltipBuilder::new(StatusbarTooltipBuilderConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn ext_statusbar_config_new() {
        let cfg = ExtStatusbarConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn ext_statusbar_config_set_get() {
        let mut cfg = ExtStatusbarConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn ext_statusbar_config_remove() {
        let mut cfg = ExtStatusbarConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn ext_statusbar_config_keys_sorted() {
        let mut cfg = ExtStatusbarConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn ext_statusbar_config_bump_version() {
        let mut cfg = ExtStatusbarConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn ext_statusbar_config_clear() {
        let mut cfg = ExtStatusbarConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn ext_statusbar_config_merge() {
        let mut cfg1 = ExtStatusbarConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = ExtStatusbarConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn ext_statusbar_config_disable() {
        let mut cfg = ExtStatusbarConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn ext_statusbar_rate_tracker_empty() {
        let rt = ExtStatusbarRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn ext_statusbar_rate_tracker_record() {
        let mut rt = ExtStatusbarRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn ext_statusbar_rate_tracker_prune() {
        let mut rt = ExtStatusbarRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn ext_statusbar_validator_valid() {
        let v = ExtStatusbarValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn ext_statusbar_validator_errors() {
        let mut v = ExtStatusbarValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn ext_statusbar_validator_clear() {
        let mut v = ExtStatusbarValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn ext_statusbar_validator_merge() {
        let mut v1 = ExtStatusbarValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = ExtStatusbarValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn ext_statusbar_rate_tracker_clear() {
        let mut rt = ExtStatusbarRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn yk_metrics_empty() {
        let m = YkMetrics::new("ext_sb");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yk_metrics_record_and_mean() {
        let mut m = YkMetrics::new("ext_sb");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yk_metrics_min_max() {
        let mut m = YkMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yk_metrics_variance_and_std() {
        let mut m = YkMetrics::new("v");
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
    fn yk_metrics_percentile() {
        let mut m = YkMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yk_metrics_merge() {
        let mut a = YkMetrics::new("a");
        a.record(1.0);
        let mut b = YkMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yk_metrics_reset() {
        let mut m = YkMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yk_rate_window_empty() {
        let rw = YkRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yk_rate_window_tick_and_rate() {
        let mut rw = YkRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yk_lru_cache_basic() {
        let mut c = YkLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yk_lru_cache_contains_and_keys() {
        let mut c = YkLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yk_lru_cache_remove() {
        let mut c = YkLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yk_metrics_sum() {
        let mut m = YkMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yk_metrics_label() {
        let m = YkMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yk_lru_cache_clear() {
        let mut c = YkLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for ext_statusbar
    #[test]
    fn xa_ext_statusbar_ring_new() {
        let rb = super::XaExtStatusbarRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_ext_statusbar_ring_push_len() {
        let mut rb = super::XaExtStatusbarRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_ext_statusbar_ring_wrap() {
        let mut rb = super::XaExtStatusbarRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_ext_statusbar_ring_mean_empty() {
        let rb = super::XaExtStatusbarRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_ext_statusbar_ring_mean_values() {
        let mut rb = super::XaExtStatusbarRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_ext_statusbar_ring_min_max() {
        let mut rb = super::XaExtStatusbarRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_ext_statusbar_ring_iter() {
        let mut rb = super::XaExtStatusbarRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_ext_statusbar_counter_new() {
        let c = super::XaExtStatusbarCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_statusbar_counter_inc() {
        let mut c = super::XaExtStatusbarCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_ext_statusbar_counter_inc_by() {
        let mut c = super::XaExtStatusbarCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_ext_statusbar_counter_reset() {
        let mut c = super::XaExtStatusbarCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_ext_statusbar_counter_clear() {
        let mut c = super::XaExtStatusbarCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_statusbar_counter_default() {
        let c = super::XaExtStatusbarCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 70 ----

    #[test]
    fn xc_70_pool_new_empty() {
        let pool: super::Xc70Pool<i32> = super::Xc70Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_70_pool_release_acquire() {
        let mut pool = super::Xc70Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_70_pool_acquire_empty() {
        let mut pool: super::Xc70Pool<i32> = super::Xc70Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_70_pool_full() {
        let mut pool = super::Xc70Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_70_pool_drain() {
        let mut pool = super::Xc70Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_70_pool_stats() {
        let mut pool = super::Xc70Pool::new(8);
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
    fn xc_70_pool_clear() {
        let mut pool = super::Xc70Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_70_pool_shrink() {
        let mut pool = super::Xc70Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_70_pool_default() {
        let pool: super::Xc70Pool<String> = super::Xc70Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_70_pool_extend() {
        let mut pool = super::Xc70Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_70_pool_retain() {
        let mut pool = super::Xc70Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_70_scheduler_round_robin() {
        let mut sched = super::Xc70Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_70_scheduler_empty() {
        let mut sched = super::Xc70Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_70_scheduler_reset() {
        let mut sched = super::Xc70Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_70_scheduler_add_remove() {
        let mut sched = super::Xc70Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_70_scheduler_targets() {
        let sched = super::Xc70Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_70_hash_empty() {
        assert_eq!(super::xc_70_hash(b""), 5381);
    }

    #[test]
    fn xc_70_hash_data() {
        let h = super::xc_70_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_70_hash(b"hello"), h);
    }

    #[test]
    fn xc_70_reverse_str() {
        assert_eq!(super::xc_70_reverse("abc"), "cba");
        assert_eq!(super::xc_70_reverse(""), "");
    }


    // --- xd_90 deepening tests ---

    #[test]
    fn xd_90_sm_initial_state() {
        let sm = Xd90StateMachine::new();
        assert_eq!(sm.current_state(), Xd90State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_90_sm_valid_idle_to_running() {
        let mut sm = Xd90StateMachine::new();
        assert!(sm.transition(Xd90State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd90State::Running);
    }

    #[test]
    fn xd_90_sm_valid_running_to_paused() {
        let mut sm = Xd90StateMachine::new();
        sm.transition(Xd90State::Running).unwrap();
        assert!(sm.transition(Xd90State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd90State::Paused);
    }

    #[test]
    fn xd_90_sm_valid_running_to_done() {
        let mut sm = Xd90StateMachine::new();
        sm.transition(Xd90State::Running).unwrap();
        assert!(sm.transition(Xd90State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd90State::Done);
    }

    #[test]
    fn xd_90_sm_valid_paused_to_running() {
        let mut sm = Xd90StateMachine::new();
        sm.transition(Xd90State::Running).unwrap();
        sm.transition(Xd90State::Paused).unwrap();
        assert!(sm.transition(Xd90State::Running).is_ok());
    }

    #[test]
    fn xd_90_sm_valid_done_to_idle() {
        let mut sm = Xd90StateMachine::new();
        sm.transition(Xd90State::Running).unwrap();
        sm.transition(Xd90State::Done).unwrap();
        assert!(sm.transition(Xd90State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd90State::Idle);
    }

    #[test]
    fn xd_90_sm_invalid_idle_to_done() {
        let mut sm = Xd90StateMachine::new();
        assert!(sm.transition(Xd90State::Done).is_err());
    }

    #[test]
    fn xd_90_sm_invalid_idle_to_paused() {
        let mut sm = Xd90StateMachine::new();
        assert!(sm.transition(Xd90State::Paused).is_err());
    }

    #[test]
    fn xd_90_sm_history_tracking() {
        let mut sm = Xd90StateMachine::new();
        sm.transition(Xd90State::Running).unwrap();
        sm.transition(Xd90State::Paused).unwrap();
        sm.transition(Xd90State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd90State::Idle);
        assert_eq!(sm.history()[0].to, Xd90State::Running);
        assert_eq!(sm.history()[1].from, Xd90State::Running);
        assert_eq!(sm.history()[2].to, Xd90State::Done);
    }

    #[test]
    fn xd_90_sm_serialize_deserialize() {
        let mut sm = Xd90StateMachine::new();
        sm.transition(Xd90State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd90StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd90State::Running));
    }

    #[test]
    fn xd_90_sm_deserialize_invalid() {
        assert_eq!(Xd90StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_90_sm_reset() {
        let mut sm = Xd90StateMachine::new();
        sm.transition(Xd90State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd90State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_90_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd90EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd90Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_90_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd90EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd90Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd90Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_90_bus_unsubscribe() {
        let mut bus = Xd90EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_90_event_kind_and_payload() {
        let e = Xd90Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd90Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_90_bus_clear_history() {
        let mut bus = Xd90EventBus::new();
        bus.publish(Xd90Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_90_sm_step_counter_increments() {
        let mut sm = Xd90StateMachine::new();
        sm.transition(Xd90State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd90State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }

}
