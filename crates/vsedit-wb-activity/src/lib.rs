//! Activity bar.

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
    fn wb_activity_validator_accepts_valid_name() {
        let v = WbActivityValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_activity_validator_rejects_empty() {
        let v = WbActivityValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_activity_validator_rejects_too_long() {
        let v = WbActivityValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_activity_validator_forbidden_prefix() {
        let v = WbActivityValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_activity_validator_allowed_chars() {
        let v = WbActivityValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_activity_validator_range() {
        let v = WbActivityValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_activity_sanitize_removes_control() {
        let result = WbActivityValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_activity_truncate_short_string() {
        assert_eq!(WbActivityValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_activity_truncate_long_string() {
        let result = WbActivityValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_activity_is_ascii_printable() {
        assert!(WbActivityValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbActivityValidator::is_ascii_printable("Hello\x00World"));
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
}
