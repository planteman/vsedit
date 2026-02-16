//! Ext API: Status bar.
//!
//! RPC bridge between the extension host and the main thread for status bar items.

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
    fn ext_statusbar_validator_accepts_valid_name() {
        let v = ExtStatusbarValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_statusbar_validator_rejects_empty() {
        let v = ExtStatusbarValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_statusbar_validator_rejects_too_long() {
        let v = ExtStatusbarValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_statusbar_validator_forbidden_prefix() {
        let v = ExtStatusbarValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_statusbar_validator_allowed_chars() {
        let v = ExtStatusbarValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_statusbar_validator_range() {
        let v = ExtStatusbarValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_statusbar_sanitize_removes_control() {
        let result = ExtStatusbarValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_statusbar_truncate_short_string() {
        assert_eq!(ExtStatusbarValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_statusbar_truncate_long_string() {
        let result = ExtStatusbarValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_statusbar_is_ascii_printable() {
        assert!(ExtStatusbarValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtStatusbarValidator::is_ascii_printable("Hello\x00World"));
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
}
