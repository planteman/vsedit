//! Activity bar.

use std::collections::HashMap;
use std::fmt;

/// Errors that can occur when operating on the activity bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityBarError {
    ItemNotFound(String),
    DuplicateItem(String),
    BarHidden,
}

impl fmt::Display for ActivityBarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ItemNotFound(id) => write!(f, "item not found: {id}"),
            Self::DuplicateItem(id) => write!(f, "duplicate item: {id}"),
            Self::BarHidden => write!(f, "activity bar is hidden"),
        }
    }
}

/// Position of the activity bar in the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityBarPosition {
    Side,
    Top,
    Hidden,
}

impl fmt::Display for ActivityBarPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Side => write!(f, "Side"),
            Self::Top => write!(f, "Top"),
            Self::Hidden => write!(f, "Hidden"),
        }
    }
}

/// An item displayed in the activity bar.
#[derive(Debug, Clone)]
pub struct ActivityBarItem {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub badge: Option<String>,
    pub active: bool,
    pub visible: bool,
    pub order: i32,
}

impl ActivityBarItem {
    /// Returns `true` if this item has a badge set.
    pub fn has_badge(&self) -> bool {
        self.badge.is_some()
    }

    /// Toggles the active state of this item.
    pub fn toggle_active(&mut self) {
        self.active = !self.active;
    }
}

impl fmt::Display for ActivityBarItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.id, self.title)
    }
}

/// Builder for constructing an [`ActivityBarItem`].
pub struct ActivityBarItemBuilder {
    id: String,
    title: String,
    icon: String,
    badge: Option<String>,
    active: bool,
    visible: bool,
    order: i32,
}

impl ActivityBarItemBuilder {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: String::new(),
            badge: None,
            active: false,
            visible: true,
            order: 0,
        }
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }

    pub fn badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    pub fn build(self) -> ActivityBarItem {
        ActivityBarItem {
            id: self.id,
            title: self.title,
            icon: self.icon,
            badge: self.badge,
            active: self.active,
            visible: self.visible,
            order: self.order,
        }
    }
}

/// The activity bar containing sidebar navigation items.
pub struct ActivityBar {
    pub items: Vec<ActivityBarItem>,
    position: ActivityBarPosition,
}

impl ActivityBar {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            position: ActivityBarPosition::Side,
        }
    }

    pub fn add_item(&mut self, item: ActivityBarItem) {
        self.items.push(item);
    }

    pub fn remove_item(&mut self, id: &str) -> bool {
        let len = self.items.len();
        self.items.retain(|i| i.id != id);
        self.items.len() != len
    }

    /// Activates the item with the given id, deactivating all others.
    pub fn activate(&mut self, id: &str) {
        for item in &mut self.items {
            item.active = item.id == id;
        }
    }

    pub fn get_active(&self) -> Option<&ActivityBarItem> {
        self.items.iter().find(|i| i.active)
    }

    pub fn set_badge(&mut self, id: &str, badge: Option<String>) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.badge = badge;
        }
    }

    pub fn set_position(&mut self, position: ActivityBarPosition) {
        self.position = position;
    }

    pub fn position(&self) -> ActivityBarPosition {
        self.position
    }

    pub fn get_visible_items(&self) -> Vec<&ActivityBarItem> {
        self.items.iter().filter(|i| i.visible).collect()
    }

    /// Adds an item, returning an error if an item with the same id already exists.
    pub fn try_add_item(&mut self, item: ActivityBarItem) -> Result<(), ActivityBarError> {
        if self.items.iter().any(|i| i.id == item.id) {
            return Err(ActivityBarError::DuplicateItem(item.id));
        }
        self.items.push(item);
        Ok(())
    }

    pub fn get_item(&self, id: &str) -> Option<&ActivityBarItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn set_visibility(&mut self, id: &str, visible: bool) -> Result<(), ActivityBarError> {
        self.items
            .iter_mut()
            .find(|i| i.id == id)
            .map(|item| item.visible = visible)
            .ok_or_else(|| ActivityBarError::ItemNotFound(id.to_string()))
    }

    /// Sorts items by their `order` field (ascending).
    pub fn sort_items(&mut self) {
        self.items.sort_by_key(|i| i.order);
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn clear_all_badges(&mut self) {
        for item in &mut self.items {
            item.badge = None;
        }
    }

    /// Returns items whose title contains the given substring (case-insensitive).
    /// Returns a reference to the items slice.
    pub fn items(&self) -> &[ActivityBarItem] {
        &self.items
    }

    /// Returns items whose title contains the given substring (case-insensitive).
    pub fn find_by_title(&self, query: &str) -> Vec<&ActivityBarItem> {
        let query_lower = query.to_lowercase();
        self.items
            .iter()
            .filter(|i| i.title.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// Returns `true` if the activity bar contains no items.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Removes all items from the activity bar.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Sets all items to inactive.
    pub fn deactivate_all(&mut self) {
        for item in &mut self.items {
            item.active = false;
        }
    }

    /// Returns references to all items that have a badge set.
    pub fn get_items_with_badge(&self) -> Vec<&ActivityBarItem> {
        self.items.iter().filter(|i| i.has_badge()).collect()
    }

    /// Moves an item to a new order value, returning an error if the item is not found.
    pub fn move_item(&mut self, id: &str, new_order: i32) -> Result<(), ActivityBarError> {
        let item = self
            .items
            .iter_mut()
            .find(|i| i.id == id)
            .ok_or_else(|| ActivityBarError::ItemNotFound(id.to_string()))?;
        item.order = new_order;
        Ok(())
    }
}

impl Default for ActivityBar {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ActivityBar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ActivityBar({} items, position={})",
            self.items.len(),
            self.position
        )
    }
}

/// Accumulated statistics for wb-activity operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbActivityStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbActivityStats {
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
    pub fn merge(&mut self, other: &WbActivityStats) {
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

impl Default for WbActivityStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbActivityStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbActivityStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-activity.
#[derive(Debug, Clone)]
pub struct WbActivityValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbActivityValidator {
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

impl Default for WbActivityValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// A badge displaying a notification count or dot indicator on an activity bar item.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivityBarBadge {
    /// The count to display. If 0, shows a dot indicator instead.
    pub count: u32,
    /// Optional tooltip text for the badge.
    pub tooltip: Option<String>,
    /// Badge color as a CSS-style string (e.g., "#ff0000").
    pub color: String,
}

impl ActivityBarBadge {
    /// Create a badge with a count.
    pub fn with_count(count: u32) -> Self {
        Self {
            count,
            tooltip: None,
            color: "#007acc".to_string(),
        }
    }

    /// Create a dot badge (no count).
    pub fn dot() -> Self {
        Self {
            count: 0,
            tooltip: None,
            color: "#007acc".to_string(),
        }
    }

    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = color.into();
        self
    }

    /// Returns true if this is a dot badge (count == 0).
    pub fn is_dot(&self) -> bool {
        self.count == 0
    }

    /// Format the badge for display. Counts > 99 show "99+".
    pub fn display_text(&self) -> String {
        if self.count == 0 {
            "●".to_string()
        } else if self.count > 99 {
            "99+".to_string()
        } else {
            self.count.to_string()
        }
    }

    /// Increment the count by one.
    pub fn increment(&mut self) {
        self.count = self.count.saturating_add(1);
    }

    /// Decrement the count by one (minimum 0).
    pub fn decrement(&mut self) {
        self.count = self.count.saturating_sub(1);
    }
}

impl fmt::Display for ActivityBarBadge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_text())
    }
}

/// Manages drag-to-reorder operations for activity bar items.
#[derive(Debug, Clone)]
pub struct ActivityBarDragReorder {
    /// ID of the item being dragged.
    pub dragged_id: String,
    /// Index of the drop target.
    pub target_index: Option<usize>,
    /// Whether the drag is active.
    pub active: bool,
}

impl ActivityBarDragReorder {
    pub fn start(id: impl Into<String>) -> Self {
        Self {
            dragged_id: id.into(),
            target_index: None,
            active: true,
        }
    }

    pub fn update_target(&mut self, index: usize) {
        self.target_index = Some(index);
    }

    pub fn cancel(&mut self) {
        self.active = false;
        self.target_index = None;
    }

    /// Apply the reorder to an ActivityBar. Returns the new index, or None if cancelled.
    pub fn apply(&mut self, bar: &mut ActivityBar) -> Option<usize> {
        if !self.active {
            return None;
        }
        self.active = false;
        let target = self.target_index?;

        // Find current position
        let current_pos = bar.items.iter().position(|i| i.id == self.dragged_id)?;
        if current_pos == target || target >= bar.items.len() {
            return Some(current_pos);
        }

        let item = bar.items.remove(current_pos);
        let insert_at = target.min(bar.items.len());
        bar.items.insert(insert_at, item);
        // Update order fields
        for (i, item) in bar.items.iter_mut().enumerate() {
            item.order = i as i32;
        }
        Some(insert_at)
    }
}

/// Serialized representation of activity bar state.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivityBarState {
    pub position: ActivityBarPosition,
    pub item_order: Vec<String>,
    pub hidden_items: Vec<String>,
    pub active_item: Option<String>,
}

/// Serialize the current activity bar state for persistence.
pub fn activity_bar_serialize(bar: &ActivityBar) -> ActivityBarState {
    let item_order: Vec<String> = bar.items.iter().map(|i| i.id.clone()).collect();
    let hidden_items: Vec<String> = bar.items.iter().filter(|i| !i.visible).map(|i| i.id.clone()).collect();
    let active_item = bar.get_active().map(|i| i.id.clone());
    ActivityBarState {
        position: bar.position(),
        item_order,
        hidden_items,
        active_item,
    }
}

/// Restore activity bar order from a serialized state.
/// Reorders items to match `state.item_order`, sets visibility and active state.
pub fn activity_bar_restore(bar: &mut ActivityBar, state: &ActivityBarState) {
    bar.set_position(state.position);

    // Reorder: for each id in state.item_order, find it in bar.items and collect
    let mut reordered = Vec::new();
    for id in &state.item_order {
        if let Some(pos) = bar.items.iter().position(|i| i.id == *id) {
            reordered.push(bar.items.remove(pos));
        }
    }
    // Append any remaining items not in the saved order
    reordered.append(&mut bar.items);
    bar.items = reordered;

    // Set visibility
    for item in &mut bar.items {
        if state.hidden_items.contains(&item.id) {
            item.visible = false;
        }
    }

    // Set active
    if let Some(ref active_id) = state.active_item {
        bar.activate(active_id);
    }
}

// ---------------------------------------------------------------------------
// ActivityBarLayout – overflow handling
// ---------------------------------------------------------------------------

/// Layout result for rendering the activity bar with a fixed capacity.
#[derive(Debug, Clone)]
pub struct ActivityBarLayout {
    pub visible_items: Vec<String>,
    pub overflow_items: Vec<String>,
    pub capacity: usize,
}

impl ActivityBarLayout {
    /// Compute layout from a bar given a maximum visible capacity.
    pub fn compute(bar: &ActivityBar, capacity: usize) -> Self {
        let visible: Vec<&ActivityBarItem> = bar.get_visible_items();
        let mut vis_ids = Vec::new();
        let mut overflow_ids = Vec::new();
        for (i, item) in visible.iter().enumerate() {
            if i < capacity {
                vis_ids.push(item.id.clone());
            } else {
                overflow_ids.push(item.id.clone());
            }
        }
        Self {
            visible_items: vis_ids,
            overflow_items: overflow_ids,
            capacity,
        }
    }

    pub fn has_overflow(&self) -> bool {
        !self.overflow_items.is_empty()
    }

    pub fn overflow_count(&self) -> usize {
        self.overflow_items.len()
    }

    pub fn total_count(&self) -> usize {
        self.visible_items.len() + self.overflow_items.len()
    }
}

impl fmt::Display for ActivityBarLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Layout({} visible, {} overflow)",
            self.visible_items.len(),
            self.overflow_items.len()
        )
    }
}

// ---------------------------------------------------------------------------
// ActivityBadgeCounter – numeric badge tracking
// ---------------------------------------------------------------------------

/// Tracks numeric badge counters for activity bar items.
#[derive(Debug, Clone)]
pub struct ActivityBadgeCounter {
    counts: std::collections::HashMap<String, u32>,
}

impl ActivityBadgeCounter {
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Set the badge count for an item.
    pub fn set(&mut self, id: impl Into<String>, count: u32) {
        self.counts.insert(id.into(), count);
    }

    /// Increment badge count by 1, returning the new value.
    pub fn increment(&mut self, id: impl Into<String>) -> u32 {
        let entry = self.counts.entry(id.into()).or_insert(0);
        *entry += 1;
        *entry
    }

    /// Decrement badge count by 1 (saturating), returning the new value.
    pub fn decrement(&mut self, id: &str) -> u32 {
        if let Some(entry) = self.counts.get_mut(id) {
            *entry = entry.saturating_sub(1);
            *entry
        } else {
            0
        }
    }

    /// Get count for an item (0 if not set).
    pub fn get(&self, id: &str) -> u32 {
        self.counts.get(id).copied().unwrap_or(0)
    }

    /// Clear count for an item.
    pub fn clear(&mut self, id: &str) {
        self.counts.remove(id);
    }

    /// Clear all counters.
    pub fn clear_all(&mut self) {
        self.counts.clear();
    }

    /// Total across all items.
    pub fn total(&self) -> u32 {
        self.counts.values().sum()
    }

    /// Items with non-zero counts.
    pub fn active_items(&self) -> Vec<(&str, u32)> {
        let mut items: Vec<(&str, u32)> = self
            .counts
            .iter()
            .filter(|(_, v)| **v > 0)
            .map(|(k, v)| (k.as_str(), *v))
            .collect();
        items.sort_by_key(|(k, _)| k.to_string());
        items
    }

    /// Format count as a badge string (e.g., "99+" for large values).
    pub fn format_badge(count: u32) -> String {
        if count == 0 {
            String::new()
        } else if count > 99 {
            "99+".to_string()
        } else {
            count.to_string()
        }
    }
}

impl Default for ActivityBadgeCounter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ActivityItemGroup – grouping with separators
// ---------------------------------------------------------------------------

/// A group of activity bar items that appear together with optional separators.
#[derive(Debug, Clone)]
pub struct ActivityItemGroup {
    pub label: String,
    pub item_ids: Vec<String>,
    pub collapsed: bool,
}

impl ActivityItemGroup {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            item_ids: Vec::new(),
            collapsed: false,
        }
    }

    pub fn add_item(&mut self, id: impl Into<String>) {
        self.item_ids.push(id.into());
    }

    pub fn remove_item(&mut self, id: &str) -> bool {
        if let Some(pos) = self.item_ids.iter().position(|s| s == id) {
            self.item_ids.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn contains(&self, id: &str) -> bool {
        self.item_ids.iter().any(|s| s == id)
    }

    pub fn toggle_collapse(&mut self) {
        self.collapsed = !self.collapsed;
    }

    pub fn len(&self) -> usize {
        self.item_ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.item_ids.is_empty()
    }
}

impl fmt::Display for ActivityItemGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = if self.collapsed { "collapsed" } else { "expanded" };
        write!(f, "Group({}, {} items, {})", self.label, self.item_ids.len(), state)
    }
}

/// Manages multiple item groups for the activity bar.
#[derive(Debug, Clone)]
pub struct ActivityGroupManager {
    groups: Vec<ActivityItemGroup>,
}

impl ActivityGroupManager {
    pub fn new() -> Self {
        Self {
            groups: Vec::new(),
        }
    }

    pub fn add_group(&mut self, group: ActivityItemGroup) {
        self.groups.push(group);
    }

    pub fn find_group(&self, label: &str) -> Option<&ActivityItemGroup> {
        self.groups.iter().find(|g| g.label == label)
    }

    pub fn find_group_mut(&mut self, label: &str) -> Option<&mut ActivityItemGroup> {
        self.groups.iter_mut().find(|g| g.label == label)
    }

    /// Find which group contains the given item id.
    pub fn group_for_item(&self, id: &str) -> Option<&ActivityItemGroup> {
        self.groups.iter().find(|g| g.contains(id))
    }

    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Total items across all groups.
    pub fn total_items(&self) -> usize {
        self.groups.iter().map(|g| g.len()).sum()
    }

    /// Flattened order of all item ids (respecting group ordering).
    pub fn flattened_order(&self) -> Vec<&str> {
        self.groups
            .iter()
            .filter(|g| !g.collapsed)
            .flat_map(|g| g.item_ids.iter().map(|s| s.as_str()))
            .collect()
    }

    pub fn groups(&self) -> &[ActivityItemGroup] {
        &self.groups
    }
}

impl Default for ActivityGroupManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ActivityBar — additional methods
// ---------------------------------------------------------------------------

impl ActivityBar {
    /// Returns the index of the item with the given id, or `None`.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.items.iter().position(|i| i.id == id)
    }

    /// Swaps the positions of two items by their ids.
    /// Returns `true` if both items were found and swapped.
    pub fn swap_items(&mut self, id_a: &str, id_b: &str) -> bool {
        let pos_a = self.items.iter().position(|i| i.id == id_a);
        let pos_b = self.items.iter().position(|i| i.id == id_b);
        match (pos_a, pos_b) {
            (Some(a), Some(b)) => {
                self.items.swap(a, b);
                true
            }
            _ => false,
        }
    }

    /// Returns `true` if the activity bar is currently hidden.
    pub fn is_hidden(&self) -> bool {
        self.position == ActivityBarPosition::Hidden
    }

    /// Returns a count of visible items.
    pub fn visible_count(&self) -> usize {
        self.items.iter().filter(|i| i.visible).count()
    }

    /// Returns a count of hidden items.
    pub fn hidden_count(&self) -> usize {
        self.items.iter().filter(|i| !i.visible).count()
    }

    /// Returns a count of items with badges.
    pub fn badge_count(&self) -> usize {
        self.items.iter().filter(|i| i.badge.is_some()).count()
    }
}

// ---------------------------------------------------------------------------
// ActivityBarItem — additional methods
// ---------------------------------------------------------------------------

impl ActivityBarItem {
    /// Clears the badge on this item.
    pub fn clear_badge(&mut self) {
        self.badge = None;
    }

    /// Returns `true` if this item is both visible and active.
    pub fn is_visible_and_active(&self) -> bool {
        self.visible && self.active
    }
}

// ---------------------------------------------------------------------------
// ActivityItemGroup — additional methods
// ---------------------------------------------------------------------------

impl ActivityItemGroup {
    /// Returns items as a slice.
    pub fn items(&self) -> &[String] {
        &self.item_ids
    }

    /// Reverses the order of items in this group.
    pub fn reverse(&mut self) {
        self.item_ids.reverse();
    }
}

// ---------------------------------------------------------------------------
// ActivityGroupManager — additional methods
// ---------------------------------------------------------------------------

impl ActivityGroupManager {
    /// Removes a group by label. Returns `true` if found and removed.
    pub fn remove_group(&mut self, label: &str) -> bool {
        let before = self.groups.len();
        self.groups.retain(|g| g.label != label);
        self.groups.len() < before
    }

    /// Returns `true` if any group contains the item id.
    pub fn contains_item(&self, id: &str) -> bool {
        self.groups.iter().any(|g| g.contains(id))
    }

    /// Returns all collapsed groups.
    pub fn collapsed_groups(&self) -> Vec<&ActivityItemGroup> {
        self.groups.iter().filter(|g| g.collapsed).collect()
    }
}

// ---------------------------------------------------------------------------
// ActivityBadgeCounter — additional methods
// ---------------------------------------------------------------------------

impl ActivityBadgeCounter {
    /// Returns `true` if any counter is non-zero.
    pub fn has_any(&self) -> bool {
        self.counts.values().any(|&v| v > 0)
    }

    /// Number of items with non-zero counts.
    pub fn active_count(&self) -> usize {
        self.counts.values().filter(|&&v| v > 0).count()
    }
}

// ---------------------------------------------------------------------------
// DragReorderSession – extended drag reorder handler
// ---------------------------------------------------------------------------

/// Extended drag reorder handler that tracks both dragging state and
/// the original order for revert support.
///
/// Unlike [`ActivityBarDragReorder`], this tracks the original order
/// to allow undo after a drop.
#[derive(Debug, Clone)]
pub struct DragReorderSession {
    /// The ID of the item being dragged.
    dragging: Option<String>,
    /// The original order of the item being dragged.
    original_order: Option<i32>,
}

impl Default for DragReorderSession {
    fn default() -> Self {
        Self {
            dragging: None,
            original_order: None,
        }
    }
}

impl DragReorderSession {
    /// Create a new reorder handler.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start dragging an item.
    pub fn start_drag(&mut self, item_id: &str, current_order: i32) {
        self.dragging = Some(item_id.to_string());
        self.original_order = Some(current_order);
    }

    /// Whether a drag is in progress.
    pub fn is_dragging(&self) -> bool {
        self.dragging.is_some()
    }

    /// Get the ID of the item being dragged.
    pub fn dragging_id(&self) -> Option<&str> {
        self.dragging.as_deref()
    }

    /// Complete the drag by applying the new order to the activity bar.
    pub fn drop(&mut self, bar: &mut ActivityBar, new_order: i32) -> Result<(), ActivityBarError> {
        if let Some(id) = self.dragging.take() {
            self.original_order = None;
            bar.move_item(&id, new_order)
        } else {
            Ok(())
        }
    }

    /// Cancel the drag operation.
    pub fn cancel(&mut self) {
        self.dragging = None;
        self.original_order = None;
    }

    /// Calculate the reordered list of item IDs given a drop target index.
    pub fn compute_reorder(items: &[ActivityBarItem], from_id: &str, to_index: usize) -> Vec<String> {
        let mut ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
        if let Some(pos) = ids.iter().position(|id| id == from_id) {
            let item = ids.remove(pos);
            let insert_at = to_index.min(ids.len());
            ids.insert(insert_at, item);
        }
        ids
    }
}

// ---------------------------------------------------------------------------
// ActivityBadgeAnimator – pulse effect for badges
// ---------------------------------------------------------------------------

/// Drives a pulse animation on an activity bar badge.
#[derive(Debug, Clone)]
pub struct ActivityBadgeAnimator {
    /// Items currently animating.
    animating: std::collections::HashMap<String, BadgeAnimation>,
}

#[derive(Debug, Clone)]
struct BadgeAnimation {
    /// Number of pulses remaining.
    pulses_remaining: u32,
    /// Total pulse count.
    total_pulses: u32,
}

impl Default for ActivityBadgeAnimator {
    fn default() -> Self {
        Self {
            animating: std::collections::HashMap::new(),
        }
    }
}

impl ActivityBadgeAnimator {
    /// Create a new animator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start pulsing a badge.
    pub fn start_pulse(&mut self, item_id: impl Into<String>, pulse_count: u32) {
        self.animating.insert(
            item_id.into(),
            BadgeAnimation {
                pulses_remaining: pulse_count,
                total_pulses: pulse_count,
            },
        );
    }

    /// Advance one animation tick. Returns items that completed.
    pub fn tick(&mut self) -> Vec<String> {
        let mut completed = Vec::new();
        for (id, anim) in &mut self.animating {
            if anim.pulses_remaining > 0 {
                anim.pulses_remaining -= 1;
            }
            if anim.pulses_remaining == 0 {
                completed.push(id.clone());
            }
        }
        for id in &completed {
            self.animating.remove(id);
        }
        completed
    }

    /// Whether any animations are active.
    pub fn is_animating(&self) -> bool {
        !self.animating.is_empty()
    }

    /// Number of items currently animating.
    pub fn animating_count(&self) -> usize {
        self.animating.len()
    }

    /// Progress of a specific animation (0.0–1.0).
    pub fn progress(&self, item_id: &str) -> Option<f64> {
        self.animating.get(item_id).map(|a| {
            if a.total_pulses == 0 {
                1.0
            } else {
                1.0 - (a.pulses_remaining as f64 / a.total_pulses as f64)
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Custom activity bar item registration
// ---------------------------------------------------------------------------

/// Registry for custom (extension-contributed) activity bar items.
#[derive(Debug, Clone)]
pub struct CustomActivityRegistry {
    items: Vec<CustomActivityItem>,
}

/// A custom activity bar contribution.
#[derive(Debug, Clone)]
pub struct CustomActivityItem {
    /// Unique ID.
    pub id: String,
    /// Display title.
    pub title: String,
    /// Icon identifier.
    pub icon: String,
    /// Extension that contributed this item.
    pub extension_id: String,
}

impl Default for CustomActivityRegistry {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl CustomActivityRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a custom item.
    pub fn register(&mut self, item: CustomActivityItem) -> bool {
        if self.items.iter().any(|i| i.id == item.id) {
            return false;
        }
        self.items.push(item);
        true
    }

    /// Unregister by ID.
    pub fn unregister(&mut self, id: &str) -> bool {
        let len = self.items.len();
        self.items.retain(|i| i.id != id);
        self.items.len() < len
    }

    /// Get all items from a specific extension.
    pub fn by_extension(&self, extension_id: &str) -> Vec<&CustomActivityItem> {
        self.items
            .iter()
            .filter(|i| i.extension_id == extension_id)
            .collect()
    }

    /// Number of custom items.
    pub fn count(&self) -> usize {
        self.items.len()
    }
}

// ---------------------------------------------------------------------------
// Activity bar context menu builder
// ---------------------------------------------------------------------------

/// Builds a context menu for an activity bar item.
#[derive(Debug, Clone)]
pub struct ActivityContextMenu {
    entries: Vec<ContextMenuEntry>,
}

/// A context menu entry.
#[derive(Debug, Clone)]
pub struct ContextMenuEntry {
    /// Label text.
    pub label: String,
    /// Action ID.
    pub action_id: String,
    /// Whether the entry is enabled.
    pub enabled: bool,
}

impl Default for ActivityContextMenu {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl ActivityContextMenu {
    /// Create an empty context menu.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a menu entry.
    pub fn add(&mut self, label: impl Into<String>, action_id: impl Into<String>) {
        self.entries.push(ContextMenuEntry {
            label: label.into(),
            action_id: action_id.into(),
            enabled: true,
        });
    }

    /// Add a disabled entry.
    pub fn add_disabled(&mut self, label: impl Into<String>, action_id: impl Into<String>) {
        self.entries.push(ContextMenuEntry {
            label: label.into(),
            action_id: action_id.into(),
            enabled: false,
        });
    }

    /// Get all entries.
    pub fn entries(&self) -> &[ContextMenuEntry] {
        &self.entries
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the menu is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get only enabled entries.
    pub fn enabled_entries(&self) -> Vec<&ContextMenuEntry> {
        self.entries.iter().filter(|e| e.enabled).collect()
    }
}


// ---------------------------------------------------------------------------
// ActivityTooltipRenderer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ActivityTooltipRenderer {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl ActivityTooltipRenderer {
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

impl Default for ActivityTooltipRenderer {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for ActivityTooltipRenderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "ActivityTooltipRenderer({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// ActivityDragHandle
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ActivityDragHandle {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl ActivityDragHandle {
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

impl Default for ActivityDragHandle {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for ActivityDragHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "ActivityDragHandle({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// ActivityTooltipRendererSnapshot — point-in-time snapshot of ActivityTooltipRenderer state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ActivityTooltipRendererSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl ActivityTooltipRendererSnapshot {
    pub fn capture(source: &ActivityTooltipRenderer, timestamp: u64) -> Self {
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

impl fmt::Display for ActivityTooltipRendererSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// ActivityDragHandleStats — aggregate statistics for ActivityDragHandle
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ActivityDragHandleStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl ActivityDragHandleStats {
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

impl fmt::Display for ActivityDragHandleStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// ActivityTooltipRendererConfig — configuration for ActivityTooltipRenderer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ActivityTooltipRendererConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl ActivityTooltipRendererConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for ActivityTooltipRendererConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for ActivityTooltipRendererConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// ActivityBadge
// ---------------------------------------------------------------------------

/// Badge displayed on an activity bar icon.
#[derive(Debug, Clone, PartialEq)]
pub enum ActivityBadge {
    Count(u32),
    Dot,
    None,
}

impl ActivityBadge {
    pub fn display_text(&self) -> String {
        match self {
            ActivityBadge::Count(n) => {
                if *n > 99 {
                    "99+".to_string()
                } else {
                    n.to_string()
                }
            }
            ActivityBadge::Dot => "●".to_string(),
            ActivityBadge::None => String::new(),
        }
    }

    pub fn is_visible(&self) -> bool {
        !matches!(self, ActivityBadge::None)
    }

    pub fn increment(&mut self) {
        if let ActivityBadge::Count(n) = self {
            *n += 1;
        }
    }

    pub fn decrement(&mut self) {
        if let ActivityBadge::Count(n) = self {
            *n = n.saturating_sub(1);
        }
    }

    pub fn clear(&mut self) {
        *self = ActivityBadge::None;
    }

    pub fn merge_badges(a: &ActivityBadge, b: &ActivityBadge) -> ActivityBadge {
        match (a, b) {
            (ActivityBadge::Count(x), ActivityBadge::Count(y)) => ActivityBadge::Count(x + y),
            (ActivityBadge::Count(x), _) => ActivityBadge::Count(*x),
            (_, ActivityBadge::Count(y)) => ActivityBadge::Count(*y),
            (ActivityBadge::Dot, _) | (_, ActivityBadge::Dot) => ActivityBadge::Dot,
            _ => ActivityBadge::None,
        }
    }
}

// ---------------------------------------------------------------------------
// ActivityBarLayout
// ---------------------------------------------------------------------------

/// Compute activity bar layout positions.
#[derive(Debug, Clone)]
pub struct ActivityBarLayoutV2 {
    pub item_height: u32,
    pub overflow_threshold: usize,
}

impl ActivityBarLayoutV2 {
    pub fn new(item_height: u32, overflow_threshold: usize) -> Self {
        Self {
            item_height,
            overflow_threshold,
        }
    }

    pub fn visible_items(&self, total: usize) -> usize {
        total.min(self.overflow_threshold)
    }

    pub fn total_height(&self, item_count: usize) -> u32 {
        self.item_height * self.visible_items(item_count) as u32
    }

    pub fn item_at_y(&self, y: u32) -> Option<usize> {
        if self.item_height == 0 {
            return None;
        }
        Some((y / self.item_height) as usize)
    }

    pub fn needs_overflow_menu(&self, total: usize) -> bool {
        total > self.overflow_threshold
    }
}

// ---------------------------------------------------------------------------
// ActivityDragReorder
// ---------------------------------------------------------------------------

/// Manage drag-and-drop reordering of activity bar items.
#[derive(Debug, Clone)]
pub struct ActivityDragReorder {
    order: Vec<String>,
    drag_source: Option<usize>,
    drag_target: Option<usize>,
    dragging: bool,
}

impl ActivityDragReorder {
    pub fn new(order: Vec<String>) -> Self {
        Self {
            order,
            drag_source: None,
            drag_target: None,
            dragging: false,
        }
    }

    pub fn drag_start(&mut self, index: usize) {
        if index < self.order.len() {
            self.drag_source = Some(index);
            self.dragging = true;
        }
    }

    pub fn drag_over(&mut self, index: usize) {
        if self.dragging && index < self.order.len() {
            self.drag_target = Some(index);
        }
    }

    pub fn drag_end(&mut self) {
        self.dragging = false;
        self.drag_source = None;
        self.drag_target = None;
    }

    pub fn preview_order(&self) -> Vec<String> {
        match (self.drag_source, self.drag_target) {
            (Some(src), Some(tgt)) if src != tgt && src < self.order.len() && tgt < self.order.len() => {
                let mut result = self.order.clone();
                let item = result.remove(src);
                result.insert(tgt, item);
                result
            }
            _ => self.order.clone(),
        }
    }

    pub fn commit_reorder(&mut self) {
        if let (Some(src), Some(tgt)) = (self.drag_source, self.drag_target) {
            if src != tgt && src < self.order.len() && tgt < self.order.len() {
                let item = self.order.remove(src);
                self.order.insert(tgt, item);
            }
        }
        self.drag_end();
    }

    pub fn reset(&mut self) {
        self.drag_end();
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging
    }
}


/// Configuration manager for wb_activity functionality.
pub struct WbActivityConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl WbActivityConfig {
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

    pub fn merge(&mut self, other: &WbActivityConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for wb_activity operations.
pub struct WbActivityRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl WbActivityRateTracker {
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

/// Validation result collector for wb_activity.
pub struct WbActivityValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl WbActivityValidationCollector {
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

    pub fn merge(&mut self, other: &WbActivityValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Activity bar item management — extended utilities (yc)
// ---------------------------------------------------------------------------

/// Metric accumulator for activity operations.
#[derive(Debug, Clone)]
pub struct YcMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YcMetrics {
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

/// Sliding-window rate counter for activity.
#[derive(Debug, Clone)]
pub struct YcRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YcRateWindow {
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

/// A small LRU-style cache for activity lookups.
#[derive(Debug, Clone)]
pub struct YcLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YcLruCache {
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
// xa_ extended helpers for wb_activity
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWbActivityRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWbActivityRingBuf {
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
pub struct XaWbActivityCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWbActivityCounter {
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

impl Default for XaWbActivityCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 200
// ---------------------------------------------------------------------------

/// Generic object pool `Xc200Pool<T>`.
pub struct Xc200Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc200Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc200PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc200Pool<T> {
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
    pub fn stats(&self) -> Xc200PoolStats {
        Xc200PoolStats {
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

impl<T> Default for Xc200Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc200Scheduler`.
pub struct Xc200Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc200Scheduler {
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

impl Default for Xc200Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_200 hash for the given byte slice.
pub fn xc_200_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_200 convention.
pub fn xc_200_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_61 deepening: state machine + event bus ---

/// States for the Xd61 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd61State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd61State {
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
pub struct Xd61Transition {
    pub from: Xd61State,
    pub to: Xd61State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd61StateMachine {
    current: Xd61State,
    history: Vec<Xd61Transition>,
    step_counter: usize,
}

impl Xd61StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd61State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd61State {
        self.current
    }

    pub fn history(&self) -> &[Xd61Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd61State) -> Result<Xd61State, String> {
        let allowed = match (self.current, target) {
            (Xd61State::Idle, Xd61State::Running) => true,
            (Xd61State::Running, Xd61State::Paused) => true,
            (Xd61State::Running, Xd61State::Done) => true,
            (Xd61State::Paused, Xd61State::Running) => true,
            (Xd61State::Paused, Xd61State::Done) => true,
            (Xd61State::Done, Xd61State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_61: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd61Transition {
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
            "Xd61SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd61State> {
        let prefix = "Xd61SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd61State::Idle),
            "Running" => Some(Xd61State::Running),
            "Paused" => Some(Xd61State::Paused),
            "Done" => Some(Xd61State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd61State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd61 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd61Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd61Event {
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

type Xd61HandlerFn = Box<dyn Fn(&Xd61Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd61EventBus {
    handlers: Vec<(usize, Option<String>, Xd61HandlerFn)>,
    next_id: usize,
    published: Vec<Xd61Event>,
}

impl Xd61EventBus {
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
        F: Fn(&Xd61Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd61Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd61Event) {
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

    pub fn published_events(&self) -> &[Xd61Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #59
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf59Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf59TrieNode {
    children: std::collections::HashMap<char, Xf59TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf59Trie {
    root: Xf59TrieNode,
    count: usize,
}

impl Xf59Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf59TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf59TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf59TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf59BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf59BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 199).
pub struct Xh199SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh199SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 241 as u64,
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

/// A compact bit set supporting boolean operations (variant 199).
pub struct Xh199BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh199BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 199).
pub struct Xi199Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi199Deque<T> {
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
pub struct Xi199Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi199Interval {
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

/// A simple interval tree (variant 199).
pub struct Xi199IntervalTree {
    xi_intervals: Vec<Xi199Interval>,
}

impl Xi199IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi199Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi199Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi199Interval) -> Vec<&Xi199Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi199Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi199Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi199Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi199Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi199Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi199Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 199) ---

/// Disjoint set / union-find for crate 199.
pub struct Xj199UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj199UnionFind {
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

const XJ199_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 199.
pub struct Xj199BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj199BTreeNode<K, V>>>,
    len: usize,
}

struct Xj199BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj199BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj199BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ199_BTREE_ORDER - 1
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
        let mid = XJ199_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj199BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj199BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj199BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj199BTreeNode::xj_new_leaf();
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


// --- xk_199 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk199SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk199SegmentTree {
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
pub struct Xk199DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk199DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_199).
#[derive(Debug, Clone)]
pub struct Xl199Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl199Rope {
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

/// Suffix array for efficient string searching (xl_199).
#[derive(Debug, Clone)]
pub struct Xl199SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl199SuffixArray {
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

    fn make_item(id: &str, order: i32) -> ActivityBarItem {
        ActivityBarItem {
            id: id.to_string(),
            title: format!("Item {id}"),
            icon: "icon".to_string(),
            badge: None,
            active: false,
            visible: true,
            order,
        }
    }

    #[test]
    fn add_and_activate() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("explorer", 0));
        bar.add_item(make_item("search", 1));
        bar.activate("search");
        let active = bar.get_active().unwrap();
        assert_eq!(active.id, "search");
    }

    #[test]
    fn remove_item() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("explorer", 0));
        assert!(bar.remove_item("explorer"));
        assert!(!bar.remove_item("explorer"));
    }

    #[test]
    fn visible_items_and_badge() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("explorer", 0));
        let mut hidden = make_item("debug", 1);
        hidden.visible = false;
        bar.add_item(hidden);
        assert_eq!(bar.get_visible_items().len(), 1);
        bar.set_badge("explorer", Some("3".to_string()));
        assert_eq!(
            bar.get_visible_items()[0].badge.as_deref(),
            Some("3")
        );
    }

    #[test]
    fn set_position() {
        let mut bar = ActivityBar::new();
        assert_eq!(bar.position(), ActivityBarPosition::Side);
        bar.set_position(ActivityBarPosition::Hidden);
        assert_eq!(bar.position(), ActivityBarPosition::Hidden);
    }

    #[test]
    fn try_add_item_rejects_duplicate() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("explorer", 0));
        let result = bar.try_add_item(make_item("explorer", 1));
        assert_eq!(result, Err(ActivityBarError::DuplicateItem("explorer".to_string())));
    }

    #[test]
    fn try_add_item_succeeds_for_unique() {
        let mut bar = ActivityBar::new();
        assert!(bar.try_add_item(make_item("explorer", 0)).is_ok());
        assert!(bar.try_add_item(make_item("search", 1)).is_ok());
        assert_eq!(bar.item_count(), 2);
    }

    #[test]
    fn get_item_found_and_missing() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("explorer", 0));
        assert!(bar.get_item("explorer").is_some());
        assert!(bar.get_item("missing").is_none());
    }

    #[test]
    fn set_visibility_updates_item() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("explorer", 0));
        assert!(bar.set_visibility("explorer", false).is_ok());
        assert!(!bar.get_item("explorer").unwrap().visible);
        assert_eq!(bar.get_visible_items().len(), 0);
    }

    #[test]
    fn set_visibility_returns_error_for_missing() {
        let mut bar = ActivityBar::new();
        let result = bar.set_visibility("nope", true);
        assert_eq!(result, Err(ActivityBarError::ItemNotFound("nope".to_string())));
    }

    #[test]
    fn sort_items_by_order() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("c", 3));
        bar.add_item(make_item("a", 1));
        bar.add_item(make_item("b", 2));
        bar.sort_items();
        let ids: Vec<&str> = bar.get_visible_items().iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn item_count_reflects_additions_and_removals() {
        let mut bar = ActivityBar::new();
        assert_eq!(bar.item_count(), 0);
        bar.add_item(make_item("a", 0));
        bar.add_item(make_item("b", 1));
        assert_eq!(bar.item_count(), 2);
        bar.remove_item("a");
        assert_eq!(bar.item_count(), 1);
    }

    #[test]
    fn clear_all_badges() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("a", 0));
        bar.add_item(make_item("b", 1));
        bar.set_badge("a", Some("5".to_string()));
        bar.set_badge("b", Some("!".to_string()));
        bar.clear_all_badges();
        assert!(bar.get_item("a").unwrap().badge.is_none());
        assert!(bar.get_item("b").unwrap().badge.is_none());
    }

    #[test]
    fn find_by_title_case_insensitive() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("explorer", 0));
        bar.add_item(make_item("search", 1));
        let results = bar.find_by_title("ITEM");
        assert_eq!(results.len(), 2);
        let results = bar.find_by_title("explorer");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "explorer");
    }

    #[test]
    fn find_by_title_returns_empty_on_no_match() {
        let bar = ActivityBar::new();
        assert!(bar.find_by_title("nothing").is_empty());
    }

    #[test]
    fn builder_creates_item_with_defaults() {
        let item = ActivityBarItemBuilder::new("ext", "Extensions")
            .icon("puzzle")
            .order(3)
            .build();
        assert_eq!(item.id, "ext");
        assert_eq!(item.title, "Extensions");
        assert_eq!(item.icon, "puzzle");
        assert_eq!(item.order, 3);
        assert!(item.visible);
        assert!(!item.active);
        assert!(item.badge.is_none());
    }

    #[test]
    fn builder_with_all_fields() {
        let item = ActivityBarItemBuilder::new("scm", "Source Control")
            .icon("git")
            .badge("2")
            .active(true)
            .visible(false)
            .order(5)
            .build();
        assert_eq!(item.badge.as_deref(), Some("2"));
        assert!(item.active);
        assert!(!item.visible);
    }

    #[test]
    fn display_impls() {
        assert_eq!(format!("{}", ActivityBarPosition::Side), "Side");
        assert_eq!(format!("{}", ActivityBarPosition::Top), "Top");
        assert_eq!(format!("{}", ActivityBarPosition::Hidden), "Hidden");

        let item = make_item("explorer", 0);
        assert_eq!(format!("{}", item), "[explorer] Item explorer");
    }

    #[test]
    fn error_display() {
        let e = ActivityBarError::ItemNotFound("x".into());
        assert_eq!(format!("{e}"), "item not found: x");
        let e = ActivityBarError::DuplicateItem("y".into());
        assert_eq!(format!("{e}"), "duplicate item: y");
        let e = ActivityBarError::BarHidden;
        assert_eq!(format!("{e}"), "activity bar is hidden");
    }

    #[test]
    fn eq_activitybarposition_same() {
        assert_eq!(ActivityBarPosition::Side, ActivityBarPosition::Side);
    }

    #[test]
    fn ne_activitybarposition_diff() {
        assert_ne!(ActivityBarPosition::Side, ActivityBarPosition::Top);
    }

    #[test]
    fn display_activitybarerror_variants() {
        assert!(!ActivityBarError::BarHidden.to_string().is_empty());
    }

    #[test]
    fn display_activitybarposition_variants() {
        assert!(!ActivityBarPosition::Side.to_string().is_empty());
        assert!(!ActivityBarPosition::Top.to_string().is_empty());
        assert!(!ActivityBarPosition::Hidden.to_string().is_empty());
    }

    #[test]
    fn badge_with_count() {
        let badge = ActivityBarBadge::with_count(5);
        assert_eq!(badge.count, 5);
        assert!(!badge.is_dot());
        assert_eq!(badge.display_text(), "5");
    }

    #[test]
    fn badge_dot() {
        let badge = ActivityBarBadge::dot();
        assert!(badge.is_dot());
        assert_eq!(badge.display_text(), "●");
    }

    #[test]
    fn badge_large_count_shows_99_plus() {
        let badge = ActivityBarBadge::with_count(150);
        assert_eq!(badge.display_text(), "99+");
    }

    #[test]
    fn badge_increment_decrement() {
        let mut badge = ActivityBarBadge::with_count(5);
        badge.increment();
        assert_eq!(badge.count, 6);
        badge.decrement();
        badge.decrement();
        assert_eq!(badge.count, 4);
    }

    #[test]
    fn badge_decrement_at_zero() {
        let mut badge = ActivityBarBadge::dot();
        badge.decrement();
        assert_eq!(badge.count, 0);
    }

    #[test]
    fn badge_with_tooltip_and_color() {
        let badge = ActivityBarBadge::with_count(3)
            .with_tooltip("3 notifications")
            .with_color("#ff0000");
        assert_eq!(badge.tooltip.as_deref(), Some("3 notifications"));
        assert_eq!(badge.color, "#ff0000");
    }

    #[test]
    fn badge_display() {
        let badge = ActivityBarBadge::with_count(42);
        assert_eq!(format!("{badge}"), "42");
    }

    #[test]
    fn drag_reorder_start_and_cancel() {
        let mut drag = ActivityBarDragReorder::start("explorer");
        assert!(drag.active);
        drag.cancel();
        assert!(!drag.active);
        assert!(drag.target_index.is_none());
    }

    #[test]
    fn drag_reorder_update_target() {
        let mut drag = ActivityBarDragReorder::start("explorer");
        drag.update_target(2);
        assert_eq!(drag.target_index, Some(2));
    }

    #[test]
    fn drag_reorder_apply_moves_item() {
        let mut bar = ActivityBar::new();
        bar.add_item(ActivityBarItemBuilder::new("a", "A").order(0).build());
        bar.add_item(ActivityBarItemBuilder::new("b", "B").order(1).build());
        bar.add_item(ActivityBarItemBuilder::new("c", "C").order(2).build());
        let mut drag = ActivityBarDragReorder::start("a");
        drag.update_target(2);
        let result = drag.apply(&mut bar);
        assert_eq!(result, Some(2));
        assert_eq!(bar.items[0].id, "b");
        assert_eq!(bar.items[1].id, "c");
        assert_eq!(bar.items[2].id, "a");
    }

    #[test]
    fn drag_reorder_cancelled_returns_none() {
        let mut bar = ActivityBar::new();
        bar.add_item(ActivityBarItemBuilder::new("a", "A").build());
        let mut drag = ActivityBarDragReorder::start("a");
        drag.cancel();
        assert!(drag.apply(&mut bar).is_none());
    }

    #[test]
    fn serialize_activity_bar() {
        let mut bar = ActivityBar::new();
        bar.add_item(ActivityBarItemBuilder::new("explorer", "Explorer").order(0).build());
        bar.add_item(ActivityBarItemBuilder::new("search", "Search").order(1).visible(false).build());
        bar.activate("explorer");
        let state = activity_bar_serialize(&bar);
        assert_eq!(state.item_order, vec!["explorer", "search"]);
        assert_eq!(state.hidden_items, vec!["search"]);
        assert_eq!(state.active_item, Some("explorer".to_string()));
        assert_eq!(state.position, ActivityBarPosition::Side);
    }

    #[test]
    fn restore_activity_bar() {
        let mut bar = ActivityBar::new();
        bar.add_item(ActivityBarItemBuilder::new("search", "Search").build());
        bar.add_item(ActivityBarItemBuilder::new("explorer", "Explorer").build());
        let state = ActivityBarState {
            position: ActivityBarPosition::Top,
            item_order: vec!["explorer".to_string(), "search".to_string()],
            hidden_items: vec!["search".to_string()],
            active_item: Some("explorer".to_string()),
        };
        activity_bar_restore(&mut bar, &state);
        assert_eq!(bar.position(), ActivityBarPosition::Top);
        assert_eq!(bar.items[0].id, "explorer");
        assert_eq!(bar.items[1].id, "search");
        assert!(!bar.items[1].visible);
        assert!(bar.get_active().unwrap().id == "explorer");
    }

    #[test]
    fn serialize_empty_bar() {
        let bar = ActivityBar::new();
        let state = activity_bar_serialize(&bar);
        assert!(state.item_order.is_empty());
        assert!(state.active_item.is_none());
    }

    #[test]
    fn drag_reorder_no_target_returns_none() {
        let mut bar = ActivityBar::new();
        bar.add_item(ActivityBarItemBuilder::new("a", "A").build());
        let mut drag = ActivityBarDragReorder::start("a");
        // No target set
        assert!(drag.apply(&mut bar).is_none());
    }

    #[test]
    fn badge_equality() {
        let a = ActivityBarBadge::with_count(5);
        let b = ActivityBarBadge::with_count(5);
        assert_eq!(a, b);
    }

    #[test]
    fn wb_activity_stats_new_defaults() {
        let stats = WbActivityStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_activity_stats_record_success() {
        let mut stats = WbActivityStats::new();
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
    fn wb_activity_stats_record_failure() {
        let mut stats = WbActivityStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_activity_stats_reset() {
        let mut stats = WbActivityStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_activity_stats_merge() {
        let mut a = WbActivityStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbActivityStats::new();
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
    fn wb_activity_stats_display() {
        let mut stats = WbActivityStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_activity_stats_default() {
        let stats = WbActivityStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wbactivity_validator_accepts_and_rejects() {
        let mut v = WbActivityValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn wbactivity_validator_warnings() {
        let mut v = WbActivityValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn wbactivity_validator_clear_and_merge() {
        let mut v = WbActivityValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = WbActivityValidationCollector::new();
        a.add_error("a_err");
        let mut b = WbActivityValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
    }

    #[test]
    fn is_empty_on_new_bar() {
        let bar = ActivityBar::new();
        assert!(bar.is_empty());
    }

    #[test]
    fn is_empty_after_adding_item() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("a", 0));
        assert!(!bar.is_empty());
    }

    #[test]
    fn clear_removes_all_items() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("a", 0));
        bar.add_item(make_item("b", 1));
        bar.clear();
        assert!(bar.is_empty());
        assert_eq!(bar.item_count(), 0);
    }

    #[test]
    fn deactivate_all_sets_all_inactive() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("a", 0));
        bar.add_item(make_item("b", 1));
        bar.activate("a");
        assert!(bar.get_active().is_some());
        bar.deactivate_all();
        assert!(bar.get_active().is_none());
    }

    #[test]
    fn get_items_with_badge_filters_correctly() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("a", 0));
        bar.add_item(make_item("b", 1));
        bar.add_item(make_item("c", 2));
        bar.set_badge("a", Some("3".to_string()));
        bar.set_badge("c", Some("!".to_string()));
        let badged = bar.get_items_with_badge();
        assert_eq!(badged.len(), 2);
        let ids: Vec<&str> = badged.iter().map(|i| i.id.as_str()).collect();
        assert!(ids.contains(&"a"));
        assert!(ids.contains(&"c"));
    }

    #[test]
    fn item_has_badge_and_toggle_active() {
        let mut item = make_item("x", 0);
        assert!(!item.has_badge());
        item.badge = Some("1".to_string());
        assert!(item.has_badge());
        assert!(!item.active);
        item.toggle_active();
        assert!(item.active);
        item.toggle_active();
        assert!(!item.active);
    }

    #[test]
    fn move_item_updates_order() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("a", 0));
        bar.add_item(make_item("b", 1));
        assert!(bar.move_item("a", 10).is_ok());
        assert_eq!(bar.get_item("a").unwrap().order, 10);
        bar.sort_items();
        assert_eq!(bar.items[0].id, "b");
        assert_eq!(bar.items[1].id, "a");
    }

    #[test]
    fn move_item_returns_error_for_missing() {
        let mut bar = ActivityBar::new();
        let result = bar.move_item("nonexistent", 5);
        assert_eq!(
            result,
            Err(ActivityBarError::ItemNotFound("nonexistent".to_string()))
        );
    }

    #[test]
    fn activity_bar_display() {
        let mut bar = ActivityBar::new();
        assert_eq!(format!("{bar}"), "ActivityBar(0 items, position=Side)");
        bar.add_item(make_item("a", 0));
        bar.add_item(make_item("b", 1));
        bar.set_position(ActivityBarPosition::Top);
        assert_eq!(format!("{bar}"), "ActivityBar(2 items, position=Top)");
    }

    // --- New tests for layout, badges, groups ---

    #[test]
    fn layout_overflow() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("a", 0));
        bar.add_item(make_item("b", 1));
        bar.add_item(make_item("c", 2));
        let layout = ActivityBarLayout::compute(&bar, 2);
        assert!(layout.has_overflow());
        assert_eq!(layout.overflow_count(), 1);
        assert_eq!(layout.visible_items.len(), 2);
        assert_eq!(layout.total_count(), 3);
    }

    #[test]
    fn layout_no_overflow() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("a", 0));
        let layout = ActivityBarLayout::compute(&bar, 5);
        assert!(!layout.has_overflow());
        assert_eq!(layout.overflow_count(), 0);
    }

    #[test]
    fn badge_counter_increment_decrement() {
        let mut bc = ActivityBadgeCounter::new();
        assert_eq!(bc.increment("explorer"), 1);
        assert_eq!(bc.increment("explorer"), 2);
        assert_eq!(bc.decrement("explorer"), 1);
        assert_eq!(bc.get("explorer"), 1);
        assert_eq!(bc.total(), 1);
    }

    #[test]
    fn badge_counter_format() {
        assert_eq!(ActivityBadgeCounter::format_badge(0), "");
        assert_eq!(ActivityBadgeCounter::format_badge(42), "42");
        assert_eq!(ActivityBadgeCounter::format_badge(100), "99+");
    }

    #[test]
    fn badge_counter_active_items() {
        let mut bc = ActivityBadgeCounter::new();
        bc.set("a", 5);
        bc.set("b", 0);
        bc.set("c", 3);
        let active = bc.active_items();
        assert_eq!(active.len(), 2);
        assert_eq!(active[0], ("a", 5));
        assert_eq!(active[1], ("c", 3));
    }

    #[test]
    fn item_group_basic() {
        let mut group = ActivityItemGroup::new("Primary");
        group.add_item("explorer");
        group.add_item("search");
        assert_eq!(group.len(), 2);
        assert!(group.contains("explorer"));
        assert!(group.remove_item("explorer"));
        assert_eq!(group.len(), 1);
        assert!(!group.contains("explorer"));
    }

    #[test]
    fn group_manager_flattened() {
        let mut mgr = ActivityGroupManager::new();
        let mut g1 = ActivityItemGroup::new("Primary");
        g1.add_item("a");
        g1.add_item("b");
        let mut g2 = ActivityItemGroup::new("Secondary");
        g2.add_item("c");
        mgr.add_group(g1);
        mgr.add_group(g2);
        assert_eq!(mgr.total_items(), 3);
        assert_eq!(mgr.flattened_order(), vec!["a", "b", "c"]);
        assert_eq!(mgr.group_for_item("c").unwrap().label, "Secondary");
    }

    #[test]
    fn group_manager_collapse_hides_items() {
        let mut mgr = ActivityGroupManager::new();
        let mut g = ActivityItemGroup::new("G");
        g.add_item("x");
        g.collapsed = true;
        mgr.add_group(g);
        assert!(mgr.flattened_order().is_empty());
    }

    // -- ActivityBar additional methods -------------------------------------

    #[test]
    fn activity_bar_index_of() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("a", 0));
        bar.add_item(make_item("b", 1));
        assert_eq!(bar.index_of("a"), Some(0));
        assert_eq!(bar.index_of("b"), Some(1));
        assert_eq!(bar.index_of("z"), None);
    }

    #[test]
    fn activity_bar_swap_items() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("a", 0));
        bar.add_item(make_item("b", 1));
        bar.add_item(make_item("c", 2));
        assert!(bar.swap_items("a", "c"));
        assert_eq!(bar.items[0].id, "c");
        assert_eq!(bar.items[2].id, "a");
        assert!(!bar.swap_items("a", "nonexistent"));
    }

    #[test]
    fn activity_bar_is_hidden() {
        let mut bar = ActivityBar::new();
        assert!(!bar.is_hidden());
        bar.set_position(ActivityBarPosition::Hidden);
        assert!(bar.is_hidden());
    }

    #[test]
    fn activity_bar_visible_hidden_badge_counts() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("a", 0));
        let mut hidden = make_item("b", 1);
        hidden.visible = false;
        bar.add_item(hidden);
        bar.set_badge("a", Some("3".to_string()));
        assert_eq!(bar.visible_count(), 1);
        assert_eq!(bar.hidden_count(), 1);
        assert_eq!(bar.badge_count(), 1);
    }

    // -- ActivityBarItem additional methods ---------------------------------

    #[test]
    fn activity_bar_item_clear_badge_and_visible_active() {
        let mut item = make_item("x", 0);
        item.badge = Some("5".to_string());
        item.clear_badge();
        assert!(item.badge.is_none());
        assert!(!item.is_visible_and_active());
        item.active = true;
        assert!(item.is_visible_and_active());
        item.visible = false;
        assert!(!item.is_visible_and_active());
    }

    // -- ActivityItemGroup additional methods -------------------------------

    #[test]
    fn activity_item_group_items_and_reverse() {
        let mut g = ActivityItemGroup::new("Test");
        g.add_item("a");
        g.add_item("b");
        g.add_item("c");
        assert_eq!(g.items(), &["a", "b", "c"]);
        g.reverse();
        assert_eq!(g.items(), &["c", "b", "a"]);
    }

    // -- ActivityGroupManager additional methods ----------------------------

    #[test]
    fn group_manager_remove_and_contains() {
        let mut mgr = ActivityGroupManager::new();
        let mut g = ActivityItemGroup::new("Primary");
        g.add_item("explorer");
        mgr.add_group(g);
        assert!(mgr.contains_item("explorer"));
        assert!(!mgr.contains_item("search"));
        assert!(mgr.remove_group("Primary"));
        assert!(!mgr.remove_group("Primary"));
        assert_eq!(mgr.group_count(), 0);
    }

    #[test]
    fn group_manager_collapsed_groups() {
        let mut mgr = ActivityGroupManager::new();
        let g1 = ActivityItemGroup::new("Open");
        let mut g2 = ActivityItemGroup::new("Closed");
        g2.collapsed = true;
        mgr.add_group(g1);
        mgr.add_group(g2);
        let collapsed = mgr.collapsed_groups();
        assert_eq!(collapsed.len(), 1);
        assert_eq!(collapsed[0].label, "Closed");
    }

    // -- ActivityBadgeCounter additional methods ----------------------------

    #[test]
    fn badge_counter_has_any_and_active_count() {
        let mut bc = ActivityBadgeCounter::new();
        assert!(!bc.has_any());
        assert_eq!(bc.active_count(), 0);
        bc.set("a", 5);
        bc.set("b", 0);
        assert!(bc.has_any());
        assert_eq!(bc.active_count(), 1);
    }

    // -- DragReorderSession tests --

    #[test]
    fn session_drag_start_and_cancel() {
        let mut dr = DragReorderSession::new();
        dr.start_drag("explorer", 0);
        assert!(dr.is_dragging());
        assert_eq!(dr.dragging_id(), Some("explorer"));
        dr.cancel();
        assert!(!dr.is_dragging());
    }

    #[test]
    fn session_drag_drop() {
        let mut bar = ActivityBar::new();
        bar.add_item(make_item("a", 0));
        bar.add_item(make_item("b", 1));
        let mut dr = DragReorderSession::new();
        dr.start_drag("a", 0);
        dr.drop(&mut bar, 1).unwrap();
        assert!(!dr.is_dragging());
    }

    #[test]
    fn compute_reorder() {
        let items = vec![make_item("a", 0), make_item("b", 1), make_item("c", 2)];
        let result = DragReorderSession::compute_reorder(&items, "a", 2);
        assert_eq!(result, vec!["b", "c", "a"]);
    }

    // -- ActivityBadgeAnimator tests --

    #[test]
    fn animator_pulse() {
        let mut anim = ActivityBadgeAnimator::new();
        anim.start_pulse("explorer", 3);
        assert!(anim.is_animating());
        assert_eq!(anim.animating_count(), 1);
        assert!(anim.progress("explorer").unwrap() < 0.01);

        anim.tick();
        let p = anim.progress("explorer").unwrap();
        assert!(p > 0.3 && p < 0.4);

        anim.tick();
        anim.tick();
        assert!(!anim.is_animating());
    }

    #[test]
    fn animator_tick_returns_completed() {
        let mut anim = ActivityBadgeAnimator::new();
        anim.start_pulse("a", 1);
        let completed = anim.tick();
        assert_eq!(completed, vec!["a"]);
    }

    // -- CustomActivityRegistry tests --

    #[test]
    fn custom_registry_register() {
        let mut reg = CustomActivityRegistry::new();
        let item = CustomActivityItem {
            id: "test".into(),
            title: "Test".into(),
            icon: "icon".into(),
            extension_id: "ext1".into(),
        };
        assert!(reg.register(item));
        assert_eq!(reg.count(), 1);
    }

    #[test]
    fn custom_registry_duplicate() {
        let mut reg = CustomActivityRegistry::new();
        let item = CustomActivityItem {
            id: "test".into(),
            title: "Test".into(),
            icon: "icon".into(),
            extension_id: "ext1".into(),
        };
        reg.register(item.clone());
        assert!(!reg.register(item));
    }

    #[test]
    fn custom_registry_by_extension() {
        let mut reg = CustomActivityRegistry::new();
        reg.register(CustomActivityItem {
            id: "a".into(), title: "A".into(), icon: "i".into(), extension_id: "ext1".into(),
        });
        reg.register(CustomActivityItem {
            id: "b".into(), title: "B".into(), icon: "i".into(), extension_id: "ext2".into(),
        });
        assert_eq!(reg.by_extension("ext1").len(), 1);
    }

    // -- ActivityContextMenu tests --

    #[test]
    fn context_menu_add() {
        let mut menu = ActivityContextMenu::new();
        menu.add("Hide", "hide");
        menu.add_disabled("Move", "move");
        assert_eq!(menu.len(), 2);
        assert_eq!(menu.enabled_entries().len(), 1);
    }

    #[test]
    fn context_menu_empty() {
        let menu = ActivityContextMenu::new();
        assert!(menu.is_empty());
    }

    #[test] fn activityTooltipRenderer_new() { let s = ActivityTooltipRenderer::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn activityTooltipRenderer_add() { let mut s = ActivityTooltipRenderer::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn activityTooltipRenderer_remove() { let mut s = ActivityTooltipRenderer::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn activityTooltipRenderer_config() { let mut s = ActivityTooltipRenderer::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn activityTooltipRenderer_nav() { let mut s = ActivityTooltipRenderer::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn activityTooltipRenderer_filter() { let mut s = ActivityTooltipRenderer::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn activityTooltipRenderer_display() { assert!(format!("{}", ActivityTooltipRenderer::new()).contains("ActivityTooltipRenderer")); }
    #[test] fn activityDragHandle_new() { let s = ActivityDragHandle::new(); assert!(s.is_empty()); }
    #[test] fn activityDragHandle_add() { let mut s = ActivityDragHandle::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn activityDragHandle_active() { let mut s = ActivityDragHandle::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn activityDragHandle_error() { let mut s = ActivityDragHandle::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn activityDragHandle_rm_group() { let mut s = ActivityDragHandle::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn activityDragHandle_display() { assert!(format!("{}", ActivityDragHandle::new()).contains("ActivityDragHandle")); }


    #[test] fn activityTooltipRenderer_snap_capture() {
        let s = ActivityTooltipRenderer::new();
        let snap = ActivityTooltipRendererSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn activityTooltipRenderer_snap_stale() {
        let s = ActivityTooltipRenderer::new();
        let snap = ActivityTooltipRendererSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn activityTooltipRenderer_snap_diff() {
        let s = ActivityTooltipRenderer::new();
        let s1v = ActivityTooltipRendererSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn activityTooltipRenderer_snap_display() {
        let s = ActivityTooltipRenderer::new();
        let snap = ActivityTooltipRendererSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn activityDragHandle_stats_record() {
        let mut st = ActivityDragHandleStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn activityDragHandle_stats_hit_ratio() {
        let mut st = ActivityDragHandleStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn activityDragHandle_stats_merge() {
        let mut a = ActivityDragHandleStats::new();
        a.total_adds = 5;
        let mut b = ActivityDragHandleStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn activityDragHandle_stats_display() {
        let st = ActivityDragHandleStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn activityTooltipRenderer_config_default() {
        let c = ActivityTooltipRendererConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn activityTooltipRenderer_config_builder() {
        let c = ActivityTooltipRendererConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn activityTooltipRenderer_config_labels() {
        let mut c = ActivityTooltipRendererConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn activityTooltipRenderer_config_cleanup_threshold() {
        let c = ActivityTooltipRendererConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn activityTooltipRenderer_config_display() {
        assert!(format!("{}", ActivityTooltipRendererConfig::new()).contains("Config"));
    }
    #[test] fn activityDragHandle_stats_peaks() {
        let mut st = ActivityDragHandleStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- ActivityBadge -----------------------------------------------------

    #[test]
    fn badge_count_display() {
        assert_eq!(ActivityBadge::Count(5).display_text(), "5");
        assert_eq!(ActivityBadge::Count(100).display_text(), "99+");
    }

    #[test]
    fn badge_dot_display() {
        assert_eq!(ActivityBadge::Dot.display_text(), "●");
    }

    #[test]
    fn badge_visibility() {
        assert!(ActivityBadge::Count(1).is_visible());
        assert!(ActivityBadge::Dot.is_visible());
        assert!(!ActivityBadge::None.is_visible());
    }

    #[test]
    fn activity_badge_inc_dec_v2() {
        let mut b = ActivityBadge::Count(3);
        b.increment();
        assert_eq!(b, ActivityBadge::Count(4));
        b.decrement();
        b.decrement();
        assert_eq!(b, ActivityBadge::Count(2));
    }

    #[test]
    fn badge_clear() {
        let mut b = ActivityBadge::Count(5);
        b.clear();
        assert_eq!(b, ActivityBadge::None);
    }

    #[test]
    fn badge_merge() {
        let merged = ActivityBadge::merge_badges(&ActivityBadge::Count(3), &ActivityBadge::Count(7));
        assert_eq!(merged, ActivityBadge::Count(10));
    }

    // -- ActivityBarLayout -------------------------------------------------

    #[test]
    fn layout_visible_items() {
        let layout = ActivityBarLayoutV2::new(48, 5);
        assert_eq!(layout.visible_items(3), 3);
        assert_eq!(layout.visible_items(10), 5);
    }

    #[test]
    fn layout_total_height() {
        let layout = ActivityBarLayoutV2::new(48, 5);
        assert_eq!(layout.total_height(3), 144);
    }

    #[test]
    fn layout_item_at_y() {
        let layout = ActivityBarLayoutV2::new(48, 10);
        assert_eq!(layout.item_at_y(0), Some(0));
        assert_eq!(layout.item_at_y(49), Some(1));
    }

    #[test]
    fn activity_layout_overflow_v2() {
        let layout = ActivityBarLayoutV2::new(48, 5);
        assert!(!layout.needs_overflow_menu(5));
        assert!(layout.needs_overflow_menu(6));
    }

    // -- ActivityDragReorder -----------------------------------------------

    #[test]
    fn drag_reorder_basic() {
        let mut drag = ActivityDragReorder::new(vec!["a".into(), "b".into(), "c".into()]);
        drag.drag_start(0);
        assert!(drag.is_dragging());
        drag.drag_over(2);
        let preview = drag.preview_order();
        assert_eq!(preview, vec!["b", "c", "a"]);
    }

    #[test]
    fn drag_commit() {
        let mut drag = ActivityDragReorder::new(vec!["x".into(), "y".into(), "z".into()]);
        drag.drag_start(2);
        drag.drag_over(0);
        drag.commit_reorder();
        assert!(!drag.is_dragging());
    }

    #[test]
    fn drag_reset() {
        let mut drag = ActivityDragReorder::new(vec!["a".into()]);
        drag.drag_start(0);
        drag.reset();
        assert!(!drag.is_dragging());
    }


    #[test]
    fn wb_activity_config_new() {
        let cfg = WbActivityConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn wb_activity_config_set_get() {
        let mut cfg = WbActivityConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn wb_activity_config_remove() {
        let mut cfg = WbActivityConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn wb_activity_config_keys_sorted() {
        let mut cfg = WbActivityConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn wb_activity_config_bump_version() {
        let mut cfg = WbActivityConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn wb_activity_config_clear() {
        let mut cfg = WbActivityConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn wb_activity_config_merge() {
        let mut cfg1 = WbActivityConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = WbActivityConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn wb_activity_config_disable() {
        let mut cfg = WbActivityConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn wb_activity_rate_tracker_empty() {
        let rt = WbActivityRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn wb_activity_rate_tracker_record() {
        let mut rt = WbActivityRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn wb_activity_rate_tracker_prune() {
        let mut rt = WbActivityRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn wb_activity_validator_valid() {
        let v = WbActivityValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn wb_activity_validator_errors() {
        let mut v = WbActivityValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn wb_activity_validator_clear() {
        let mut v = WbActivityValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn wb_activity_validator_merge() {
        let mut v1 = WbActivityValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = WbActivityValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn wb_activity_rate_tracker_clear() {
        let mut rt = WbActivityRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn yc_metrics_empty() {
        let m = YcMetrics::new("activity");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yc_metrics_record_and_mean() {
        let mut m = YcMetrics::new("activity");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yc_metrics_min_max() {
        let mut m = YcMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yc_metrics_variance_and_std() {
        let mut m = YcMetrics::new("v");
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
    fn yc_metrics_percentile() {
        let mut m = YcMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yc_metrics_merge() {
        let mut a = YcMetrics::new("a");
        a.record(1.0);
        let mut b = YcMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yc_metrics_reset() {
        let mut m = YcMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yc_rate_window_empty() {
        let rw = YcRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yc_rate_window_tick_and_rate() {
        let mut rw = YcRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yc_lru_cache_basic() {
        let mut c = YcLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yc_lru_cache_contains_and_keys() {
        let mut c = YcLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yc_lru_cache_remove() {
        let mut c = YcLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yc_metrics_sum() {
        let mut m = YcMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yc_metrics_label() {
        let m = YcMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yc_lru_cache_clear() {
        let mut c = YcLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for wb_activity
    #[test]
    fn xa_wb_activity_ring_new() {
        let rb = super::XaWbActivityRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_wb_activity_ring_push_len() {
        let mut rb = super::XaWbActivityRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_wb_activity_ring_wrap() {
        let mut rb = super::XaWbActivityRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_wb_activity_ring_mean_empty() {
        let rb = super::XaWbActivityRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_wb_activity_ring_mean_values() {
        let mut rb = super::XaWbActivityRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_wb_activity_ring_min_max() {
        let mut rb = super::XaWbActivityRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_wb_activity_ring_iter() {
        let mut rb = super::XaWbActivityRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_wb_activity_counter_new() {
        let c = super::XaWbActivityCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_activity_counter_inc() {
        let mut c = super::XaWbActivityCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_wb_activity_counter_inc_by() {
        let mut c = super::XaWbActivityCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_wb_activity_counter_reset() {
        let mut c = super::XaWbActivityCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_wb_activity_counter_clear() {
        let mut c = super::XaWbActivityCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_activity_counter_default() {
        let c = super::XaWbActivityCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 200 ----

    #[test]
    fn xc_200_pool_new_empty() {
        let pool: super::Xc200Pool<i32> = super::Xc200Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_200_pool_release_acquire() {
        let mut pool = super::Xc200Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_200_pool_acquire_empty() {
        let mut pool: super::Xc200Pool<i32> = super::Xc200Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_200_pool_full() {
        let mut pool = super::Xc200Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_200_pool_drain() {
        let mut pool = super::Xc200Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_200_pool_stats() {
        let mut pool = super::Xc200Pool::new(8);
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
    fn xc_200_pool_clear() {
        let mut pool = super::Xc200Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_200_pool_shrink() {
        let mut pool = super::Xc200Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_200_pool_default() {
        let pool: super::Xc200Pool<String> = super::Xc200Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_200_pool_extend() {
        let mut pool = super::Xc200Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_200_pool_retain() {
        let mut pool = super::Xc200Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_200_scheduler_round_robin() {
        let mut sched = super::Xc200Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_200_scheduler_empty() {
        let mut sched = super::Xc200Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_200_scheduler_reset() {
        let mut sched = super::Xc200Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_200_scheduler_add_remove() {
        let mut sched = super::Xc200Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_200_scheduler_targets() {
        let sched = super::Xc200Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_200_hash_empty() {
        assert_eq!(super::xc_200_hash(b""), 5381);
    }

    #[test]
    fn xc_200_hash_data() {
        let h = super::xc_200_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_200_hash(b"hello"), h);
    }

    #[test]
    fn xc_200_reverse_str() {
        assert_eq!(super::xc_200_reverse("abc"), "cba");
        assert_eq!(super::xc_200_reverse(""), "");
    }


    // --- xd_61 deepening tests ---

    #[test]
    fn xd_61_sm_initial_state() {
        let sm = Xd61StateMachine::new();
        assert_eq!(sm.current_state(), Xd61State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_61_sm_valid_idle_to_running() {
        let mut sm = Xd61StateMachine::new();
        assert!(sm.transition(Xd61State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd61State::Running);
    }

    #[test]
    fn xd_61_sm_valid_running_to_paused() {
        let mut sm = Xd61StateMachine::new();
        sm.transition(Xd61State::Running).unwrap();
        assert!(sm.transition(Xd61State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd61State::Paused);
    }

    #[test]
    fn xd_61_sm_valid_running_to_done() {
        let mut sm = Xd61StateMachine::new();
        sm.transition(Xd61State::Running).unwrap();
        assert!(sm.transition(Xd61State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd61State::Done);
    }

    #[test]
    fn xd_61_sm_valid_paused_to_running() {
        let mut sm = Xd61StateMachine::new();
        sm.transition(Xd61State::Running).unwrap();
        sm.transition(Xd61State::Paused).unwrap();
        assert!(sm.transition(Xd61State::Running).is_ok());
    }

    #[test]
    fn xd_61_sm_valid_done_to_idle() {
        let mut sm = Xd61StateMachine::new();
        sm.transition(Xd61State::Running).unwrap();
        sm.transition(Xd61State::Done).unwrap();
        assert!(sm.transition(Xd61State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd61State::Idle);
    }

    #[test]
    fn xd_61_sm_invalid_idle_to_done() {
        let mut sm = Xd61StateMachine::new();
        assert!(sm.transition(Xd61State::Done).is_err());
    }

    #[test]
    fn xd_61_sm_invalid_idle_to_paused() {
        let mut sm = Xd61StateMachine::new();
        assert!(sm.transition(Xd61State::Paused).is_err());
    }

    #[test]
    fn xd_61_sm_history_tracking() {
        let mut sm = Xd61StateMachine::new();
        sm.transition(Xd61State::Running).unwrap();
        sm.transition(Xd61State::Paused).unwrap();
        sm.transition(Xd61State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd61State::Idle);
        assert_eq!(sm.history()[0].to, Xd61State::Running);
        assert_eq!(sm.history()[1].from, Xd61State::Running);
        assert_eq!(sm.history()[2].to, Xd61State::Done);
    }

    #[test]
    fn xd_61_sm_serialize_deserialize() {
        let mut sm = Xd61StateMachine::new();
        sm.transition(Xd61State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd61StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd61State::Running));
    }

    #[test]
    fn xd_61_sm_deserialize_invalid() {
        assert_eq!(Xd61StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_61_sm_reset() {
        let mut sm = Xd61StateMachine::new();
        sm.transition(Xd61State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd61State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_61_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd61EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd61Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_61_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd61EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd61Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd61Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_61_bus_unsubscribe() {
        let mut bus = Xd61EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_61_event_kind_and_payload() {
        let e = Xd61Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd61Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_61_bus_clear_history() {
        let mut bus = Xd61EventBus::new();
        bus.publish(Xd61Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_61_sm_step_counter_increments() {
        let mut sm = Xd61StateMachine::new();
        sm.transition(Xd61State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd61State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #59 --

    #[test]
    fn xf59_trie_insert_search() {
        let mut t = Xf59Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf59_trie_starts_with() {
        let mut t = Xf59Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf59_trie_remove() {
        let mut t = Xf59Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf59_trie_word_count() {
        let mut t = Xf59Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf59_trie_longest_prefix() {
        let mut t = Xf59Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf59_trie_all_words() {
        let mut t = Xf59Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf59_trie_autocomplete() {
        let mut t = Xf59Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf59_trie_empty_search() {
        let t = Xf59Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf59_bloom_add_contains() {
        let mut bf = Xf59BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf59_bloom_probably_absent() {
        let bf = Xf59BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf59_bloom_false_positive_rate() {
        let mut bf = Xf59BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf59_bloom_clear() {
        let mut bf = Xf59BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf59_bloom_union() {
        let mut a = Xf59BloomFilter::xf_new(512, 2);
        let mut b = Xf59BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf59_bloom_intersection_estimate() {
        let mut a = Xf59BloomFilter::xf_new(512, 2);
        let mut b = Xf59BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf59_bloom_union_size_mismatch() {
        let a = Xf59BloomFilter::xf_new(256, 2);
        let b = Xf59BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh199_skip_insert_contains() {
        let mut sl = super::Xh199SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh199_skip_remove() {
        let mut sl = super::Xh199SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh199_skip_len() {
        let mut sl = super::Xh199SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh199_skip_range_query() {
        let mut sl = super::Xh199SkipList::xh_new(4);
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
    fn xh199_skip_floor_ceiling() {
        let mut sl = super::Xh199SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh199_skip_rank() {
        let mut sl = super::Xh199SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh199_skip_empty() {
        let sl = super::Xh199SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh199_skip_duplicates() {
        let mut sl = super::Xh199SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh199_bitset_set_test() {
        let mut bs = super::Xh199BitSet::xh_new(256);
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
    fn xh199_bitset_clear_count() {
        let mut bs = super::Xh199BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh199_bitset_and_or_xor() {
        let mut a = super::Xh199BitSet::xh_new(128);
        let mut b = super::Xh199BitSet::xh_new(128);
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
    fn xh199_bitset_iter_ones() {
        let mut bs = super::Xh199BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh199_bitset_first_last() {
        let mut bs = super::Xh199BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh199_bitset_empty() {
        let bs = super::Xh199BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi199_deque_push_pop_back() {
        let mut dq = super::Xi199Deque::xi_new(4);
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
    fn xi199_deque_push_pop_front() {
        let mut dq = super::Xi199Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi199_deque_mixed_ops() {
        let mut dq = super::Xi199Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi199_deque_get_and_split() {
        let mut dq = super::Xi199Deque::xi_new(8);
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
    fn xi199_deque_rotate_left() {
        let mut dq = super::Xi199Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi199_deque_rotate_right() {
        let mut dq = super::Xi199Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi199_deque_grow() {
        let mut dq = super::Xi199Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi199_deque_empty() {
        let dq = super::Xi199Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi199_interval_tree_insert_query() {
        let mut tree = super::Xi199IntervalTree::xi_new();
        tree.xi_insert(super::Xi199Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi199Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi199Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi199_interval_tree_overlap() {
        let mut tree = super::Xi199IntervalTree::xi_new();
        tree.xi_insert(super::Xi199Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi199Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi199Interval::xi_new(12, 20));
        let q = super::Xi199Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi199_interval_tree_remove() {
        let mut tree = super::Xi199IntervalTree::xi_new();
        tree.xi_insert(super::Xi199Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi199Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi199_interval_tree_gaps() {
        let mut tree = super::Xi199IntervalTree::xi_new();
        tree.xi_insert(super::Xi199Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi199Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi199Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi199Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi199Interval::xi_new(8, 10));
    }

    #[test]
    fn xi199_interval_tree_merge() {
        let mut tree = super::Xi199IntervalTree::xi_new();
        tree.xi_insert(super::Xi199Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi199Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi199Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi199Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi199Interval::xi_new(10, 15));
    }

    #[test]
    fn xi199_interval_tree_all() {
        let mut tree = super::Xi199IntervalTree::xi_new();
        tree.xi_insert(super::Xi199Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi199Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi199_interval_tree_empty() {
        let tree = super::Xi199IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi199_interval_tree_contains_point() {
        let iv = super::Xi199Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 199) ---

    #[test]
    fn xj_199_uf_make_and_find() {
        let mut uf = super::Xj199UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_199_uf_union_connected() {
        let mut uf = super::Xj199UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_199_uf_component_count() {
        let mut uf = super::Xj199UnionFind::xj_new();
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
    fn xj_199_uf_component_size() {
        let mut uf = super::Xj199UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_199_uf_largest_component() {
        let mut uf = super::Xj199UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_199_uf_many_elements() {
        let mut uf = super::Xj199UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_199_uf_separate_components() {
        let mut uf = super::Xj199UnionFind::xj_new();
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
    fn xj_199_uf_path_compression() {
        let mut uf = super::Xj199UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_199_bt_insert_get() {
        let mut bt = super::Xj199BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_199_bt_contains_len() {
        let mut bt = super::Xj199BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_199_bt_replace() {
        let mut bt = super::Xj199BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_199_bt_remove() {
        let mut bt = super::Xj199BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_199_bt_keys_values() {
        let mut bt = super::Xj199BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_199_bt_range() {
        let mut bt = super::Xj199BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_199_bt_min_max() {
        let mut bt = super::Xj199BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_199_bt_many_inserts() {
        let mut bt = super::Xj199BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_199 segment tree tests ---

    #[test]
    fn xk_199_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk199SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_199_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk199SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_199_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk199SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_199_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk199SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_199_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk199SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_199_st_single_element() {
        let data = vec![42];
        let st = super::Xk199SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_199_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk199SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_199_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk199SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_199 disjoint intervals tests ---

    #[test]
    fn xk_199_di_add_and_count() {
        let mut di = super::Xk199DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_199_di_merge_overlap() {
        let mut di = super::Xk199DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_199_di_contains() {
        let mut di = super::Xk199DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_199_di_remove() {
        let mut di = super::Xk199DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_199_di_covered_length() {
        let mut di = super::Xk199DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_199_di_gaps() {
        let mut di = super::Xk199DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_199_di_merge_adjacent() {
        let mut di = super::Xk199DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_199_di_empty() {
        let di = super::Xk199DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_199_rope_new_empty() {
        let rope = super::Xl199Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_199_rope_from_str() {
        let rope = super::Xl199Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_199_rope_insert_at() {
        let mut rope = super::Xl199Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_199_rope_delete_range() {
        let mut rope = super::Xl199Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_199_rope_char_at() {
        let rope = super::Xl199Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_199_rope_split_concat() {
        let rope = super::Xl199Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_199_rope_line_count() {
        let rope = super::Xl199Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_199_rope_line_at() {
        let rope = super::Xl199Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_199_sa_build_and_search() {
        let sa = super::Xl199SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_199_sa_count() {
        let sa = super::Xl199SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_199_sa_longest_repeated() {
        let sa = super::Xl199SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_199_sa_all_positions() {
        let sa = super::Xl199SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_199_sa_len() {
        let sa = super::Xl199SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_199_sa_empty() {
        let sa = super::Xl199SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_199_rope_slice() {
        let rope = super::Xl199Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_199_sa_search_start() {
        let sa = super::Xl199SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }
}
