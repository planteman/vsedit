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

}
