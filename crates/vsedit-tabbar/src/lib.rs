//! Editor tab bar widget.

use std::collections::HashMap;
use std::fmt;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabKind {
    File,
    Preview,
    Diff,
    Settings,
    Welcome,
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct Tab {
    pub id: String,
    pub label: String,
    pub uri: Option<String>,
    pub kind: TabKind,
    pub dirty: bool,
    pub pinned: bool,
    pub preview: bool,
    pub active: bool,
}

impl Tab {
    pub fn is_pinned(&self) -> bool {
        self.pinned
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
}

impl TabKind {
    pub fn label(&self) -> &'static str {
        match self {
            TabKind::File => "File",
            TabKind::Preview => "Preview",
            TabKind::Diff => "Diff",
            TabKind::Settings => "Settings",
            TabKind::Welcome => "Welcome",
            TabKind::Custom(_) => "Custom",
        }
    }
}

#[derive(Debug)]
pub struct TabGroup {
    tabs: Vec<Tab>,
    active_tab: Option<usize>,
}

impl TabGroup {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: None,
        }
    }

    pub fn add_tab(&mut self, tab: Tab) {
        self.tabs.push(tab);
    }

    pub fn close_tab(&mut self, id: &str) -> bool {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs.remove(pos);
            match self.active_tab {
                Some(idx) if idx == pos => {
                    self.active_tab = if self.tabs.is_empty() {
                        None
                    } else {
                        Some(idx.min(self.tabs.len() - 1))
                    };
                }
                Some(idx) if idx > pos => self.active_tab = Some(idx - 1),
                _ => {}
            }
            true
        } else {
            false
        }
    }

    pub fn activate_tab(&mut self, id: &str) {
        for tab in &mut self.tabs {
            tab.active = false;
        }
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            self.tabs[pos].active = true;
            self.active_tab = Some(pos);
        }
    }

    pub fn get_active_tab(&self) -> Option<&Tab> {
        self.active_tab.and_then(|i| self.tabs.get(i))
    }

    pub fn pin_tab(&mut self, id: &str) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.pinned = true;
        }
    }

    pub fn unpin_tab(&mut self, id: &str) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.pinned = false;
        }
    }

    pub fn mark_dirty(&mut self, id: &str) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.dirty = true;
        }
    }

    pub fn mark_clean(&mut self, id: &str) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.dirty = false;
        }
    }

    pub fn close_saved_tabs(&mut self) {
        let active_id = self.get_active_tab().map(|t| t.id.clone());
        self.tabs.retain(|t| t.dirty || t.pinned);
        self.active_tab = active_id.and_then(|id| self.tabs.iter().position(|t| t.id == id));
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn get_dirty_tabs(&self) -> Vec<&Tab> {
        self.tabs.iter().filter(|t| t.dirty).collect()
    }

    pub fn move_tab(&mut self, id: &str, new_index: usize) -> bool {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            if new_index >= self.tabs.len() {
                return false;
            }
            let tab = self.tabs.remove(pos);
            self.tabs.insert(new_index, tab);
            // Update active_tab index to follow the active tab.
            self.active_tab = self.tabs.iter().position(|t| t.active);
            true
        } else {
            false
        }
    }

    pub fn get_tab(&self, id: &str) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn get_tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn close_all(&mut self) -> Vec<Tab> {
        self.active_tab = None;
        std::mem::take(&mut self.tabs)
    }

    pub fn close_others(&mut self, id: &str) -> Vec<Tab> {
        let mut closed = Vec::new();
        let mut kept = Vec::new();
        for tab in self.tabs.drain(..) {
            if tab.id == id {
                kept.push(tab);
            } else {
                closed.push(tab);
            }
        }
        self.tabs = kept;
        self.active_tab = if self.tabs.is_empty() { None } else { Some(0) };
        closed
    }

    pub fn close_to_right(&mut self, id: &str) -> Vec<Tab> {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            let closed: Vec<Tab> = self.tabs.drain(pos + 1..).collect();
            self.active_tab = self.tabs.iter().position(|t| t.active);
            closed
        } else {
            Vec::new()
        }
    }

    pub fn close_to_left(&mut self, id: &str) -> Vec<Tab> {
        if let Some(pos) = self.tabs.iter().position(|t| t.id == id) {
            let closed: Vec<Tab> = self.tabs.drain(..pos).collect();
            self.active_tab = self.tabs.iter().position(|t| t.active);
            closed
        } else {
            Vec::new()
        }
    }

    pub fn get_pinned_tabs(&self) -> Vec<&Tab> {
        self.tabs.iter().filter(|t| t.pinned).collect()
    }

    pub fn get_preview_tabs(&self) -> Vec<&Tab> {
        self.tabs.iter().filter(|t| t.preview).collect()
    }

    pub fn promote_preview(&mut self, id: &str) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.preview = false;
        }
    }

    pub fn find_by_uri(&self, uri: &str) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.uri.as_deref() == Some(uri))
    }

    pub fn pinned_count(&self) -> usize {
        self.tabs.iter().filter(|t| t.pinned).count()
    }

    pub fn dirty_count(&self) -> usize {
        self.tabs.iter().filter(|t| t.dirty).count()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.label == label)
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn get_tab_index(&self, id: &str) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }
}

impl Default for TabGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TabGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} tabs ({} dirty)", self.tabs.len(), self.dirty_count())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabSizing {
    Fit,
    Fixed,
    Shrink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseButtonPosition {
    Left,
    Right,
    Off,
}

#[derive(Debug, Clone)]
pub struct TabBarConfig {
    pub show_icons: bool,
    pub tab_sizing: TabSizing,
    pub close_button_position: CloseButtonPosition,
}

impl Default for TabBarConfig {
    fn default() -> Self {
        Self {
            show_icons: true,
            tab_sizing: TabSizing::Fit,
            close_button_position: CloseButtonPosition::Right,
        }
    }
}

/// Accumulated statistics for tabbar operations.
#[derive(Debug, Clone, PartialEq)]
pub struct TabbarStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl TabbarStats {
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
    pub fn merge(&mut self, other: &TabbarStats) {
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

impl Default for TabbarStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TabbarStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TabbarStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for tabbar.
#[derive(Debug, Clone)]
pub struct TabbarValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl TabbarValidator {
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

impl Default for TabbarValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a drag-and-drop reorder operation on tabs.
#[derive(Debug, Clone)]
pub struct TabDragReorder {
    /// ID of the tab being dragged.
    pub dragged_tab_id: String,
    /// Current x position of the drag (in pixels).
    pub drag_x: f64,
    /// Whether the drag is currently active.
    pub active: bool,
}

impl TabDragReorder {
    pub fn start(tab_id: impl Into<String>, x: f64) -> Self {
        Self {
            dragged_tab_id: tab_id.into(),
            drag_x: x,
            active: true,
        }
    }

    pub fn update_position(&mut self, x: f64) {
        self.drag_x = x;
    }

    pub fn cancel(&mut self) {
        self.active = false;
    }

    /// Calculate the insert index given tab widths and positions.
    /// `tab_positions` is a slice of (start_x, width) for each tab.
    pub fn calculate_insert_index(&self, tab_positions: &[(f64, f64)]) -> usize {
        for (i, &(start, width)) in tab_positions.iter().enumerate() {
            let mid = start + width / 2.0;
            if self.drag_x < mid {
                return i;
            }
        }
        tab_positions.len()
    }

    /// Finish the drag, applying the reorder to the tab group.
    /// Returns the new index, or `None` if the drag was cancelled or tab not found.
    pub fn finish(&mut self, group: &mut TabGroup, tab_positions: &[(f64, f64)]) -> Option<usize> {
        if !self.active {
            return None;
        }
        self.active = false;
        let new_idx = self.calculate_insert_index(tab_positions);
        let target = new_idx.min(group.tab_count().saturating_sub(1));
        if group.move_tab(&self.dragged_tab_id, target) {
            Some(target)
        } else {
            None
        }
    }
}

/// Manages tab overflow when there are more tabs than visible space allows.
#[derive(Debug, Clone)]
pub struct TabOverflow {
    /// Maximum number of visible tabs.
    pub max_visible: usize,
    /// Index of the first visible tab (scroll offset).
    pub scroll_offset: usize,
    /// Total tab count (updated externally).
    pub total_tabs: usize,
}

impl TabOverflow {
    pub fn new(max_visible: usize) -> Self {
        Self {
            max_visible,
            scroll_offset: 0,
            total_tabs: 0,
        }
    }

    pub fn update_total(&mut self, total: usize) {
        self.total_tabs = total;
        // Clamp scroll_offset
        if self.total_tabs <= self.max_visible {
            self.scroll_offset = 0;
        } else if self.scroll_offset > self.total_tabs - self.max_visible {
            self.scroll_offset = self.total_tabs - self.max_visible;
        }
    }

    /// Returns true if tabs are overflowing.
    pub fn is_overflowing(&self) -> bool {
        self.total_tabs > self.max_visible
    }

    /// Returns the range of visible tab indices.
    pub fn visible_range(&self) -> std::ops::Range<usize> {
        let end = (self.scroll_offset + self.max_visible).min(self.total_tabs);
        self.scroll_offset..end
    }

    /// Scroll left (decrease offset) by one tab.
    pub fn scroll_left(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// Scroll right (increase offset) by one tab.
    pub fn scroll_right(&mut self) {
        if self.total_tabs > self.max_visible && self.scroll_offset < self.total_tabs - self.max_visible {
            self.scroll_offset += 1;
        }
    }

    /// Scroll to make a specific tab index visible.
    pub fn ensure_visible(&mut self, index: usize) {
        if index < self.scroll_offset {
            self.scroll_offset = index;
        } else if index >= self.scroll_offset + self.max_visible {
            self.scroll_offset = index - self.max_visible + 1;
        }
    }

    /// Number of tabs hidden to the left.
    pub fn hidden_left(&self) -> usize {
        self.scroll_offset
    }

    /// Number of tabs hidden to the right.
    pub fn hidden_right(&self) -> usize {
        if self.total_tabs > self.scroll_offset + self.max_visible {
            self.total_tabs - self.scroll_offset - self.max_visible
        } else {
            0
        }
    }

    /// Returns items for a dropdown showing all overflowed tabs.
    pub fn overflow_menu_indices(&self) -> Vec<usize> {
        let visible = self.visible_range();
        (0..self.total_tabs).filter(|i| !visible.contains(i)).collect()
    }
}

/// State of a tab close animation.
#[derive(Debug, Clone)]
pub struct TabCloseAnimation {
    pub tab_id: String,
    pub progress: f64,
    pub duration_ms: u64,
    pub started: bool,
}

impl TabCloseAnimation {
    pub fn new(tab_id: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            tab_id: tab_id.into(),
            progress: 0.0,
            duration_ms,
            started: false,
        }
    }

    pub fn start(&mut self) {
        self.started = true;
        self.progress = 0.0;
    }

    /// Advance the animation by `delta_ms` milliseconds. Returns true if complete.
    pub fn tick(&mut self, delta_ms: u64) -> bool {
        if !self.started {
            return false;
        }
        self.progress += delta_ms as f64 / self.duration_ms as f64;
        if self.progress >= 1.0 {
            self.progress = 1.0;
            true
        } else {
            false
        }
    }

    pub fn is_complete(&self) -> bool {
        self.progress >= 1.0
    }

    /// Current opacity (1.0 = fully visible, 0.0 = fully hidden).
    pub fn opacity(&self) -> f64 {
        1.0 - self.progress
    }

    /// Current width scale (1.0 = full width, 0.0 = collapsed).
    pub fn width_scale(&self) -> f64 {
        1.0 - self.progress
    }
}

/// Create a tab close animation frame state.
pub fn tab_close_animation(tab_id: &str, duration_ms: u64) -> TabCloseAnimation {
    TabCloseAnimation::new(tab_id, duration_ms)
}

// ---------------------------------------------------------------------------
// TabLayout — overflow handling strategies
// ---------------------------------------------------------------------------

/// Strategy for handling tab overflow when tabs exceed available width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabOverflowStrategy {
    /// Tabs shrink proportionally to fit.
    Shrink,
    /// A horizontal scroll region is used.
    Scroll,
    /// Excess tabs are placed in a dropdown menu.
    Dropdown,
}

/// Computes tab layout given available width and tab count.
#[derive(Debug, Clone)]
pub struct TabLayout {
    pub strategy: TabOverflowStrategy,
    /// Minimum width per tab in characters.
    pub min_tab_width: usize,
    /// Maximum width per tab in characters.
    pub max_tab_width: usize,
    /// Available container width.
    pub available_width: usize,
}

impl TabLayout {
    /// Create a new layout calculator.
    pub fn new(available_width: usize, strategy: TabOverflowStrategy) -> Self {
        Self {
            strategy,
            min_tab_width: 8,
            max_tab_width: 40,
            available_width,
        }
    }

    /// Compute the rendered width for each tab given the number of tabs.
    /// Returns a vector of per-tab widths.
    pub fn compute_widths(&self, tab_count: usize) -> Vec<usize> {
        if tab_count == 0 {
            return Vec::new();
        }
        match self.strategy {
            TabOverflowStrategy::Shrink => {
                let ideal = self.available_width / tab_count;
                let clamped = ideal.clamp(self.min_tab_width, self.max_tab_width);
                vec![clamped; tab_count]
            }
            TabOverflowStrategy::Scroll | TabOverflowStrategy::Dropdown => {
                let ideal = self.available_width / tab_count;
                let w = ideal.min(self.max_tab_width).max(self.min_tab_width);
                vec![w; tab_count]
            }
        }
    }

    /// Determine how many tabs are visible (fit within available width).
    pub fn visible_count(&self, tab_count: usize) -> usize {
        if tab_count == 0 {
            return 0;
        }
        let widths = self.compute_widths(tab_count);
        let per_tab = widths[0];
        let fits = self.available_width / per_tab;
        fits.min(tab_count)
    }

    /// Whether overflow is happening.
    pub fn is_overflowing(&self, tab_count: usize) -> bool {
        self.visible_count(tab_count) < tab_count
    }
}

// ---------------------------------------------------------------------------
// Tab drag-and-drop reordering
// ---------------------------------------------------------------------------

/// Describes a drag-and-drop reorder operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabDragOperation {
    /// ID of the tab being dragged.
    pub tab_id: String,
    /// Original index of the tab.
    pub from_index: usize,
    /// Target index for the tab.
    pub to_index: usize,
}

impl TabDragOperation {
    /// Create a new drag operation.
    pub fn new(tab_id: impl Into<String>, from_index: usize, to_index: usize) -> Self {
        Self {
            tab_id: tab_id.into(),
            from_index,
            to_index,
        }
    }

    /// Whether the drag actually moves the tab.
    pub fn is_move(&self) -> bool {
        self.from_index != self.to_index
    }

    /// Distance of the drag in tab positions.
    pub fn distance(&self) -> usize {
        if self.from_index > self.to_index {
            self.from_index - self.to_index
        } else {
            self.to_index - self.from_index
        }
    }
}

/// Apply a drag-and-drop reorder operation to a `TabGroup`.
/// Returns `true` if the operation was applied successfully.
pub fn apply_drag_reorder(group: &mut TabGroup, op: &TabDragOperation) -> bool {
    if !op.is_move() {
        return false;
    }
    group.move_tab(&op.tab_id, op.to_index)
}

// ---------------------------------------------------------------------------
// Tab pinning with pin-area separation
// ---------------------------------------------------------------------------

/// Splits a `TabGroup`'s tabs into pinned and unpinned regions.
#[derive(Debug)]
pub struct PinnedAreaSplit<'a> {
    pub pinned: Vec<&'a Tab>,
    pub unpinned: Vec<&'a Tab>,
}

impl<'a> PinnedAreaSplit<'a> {
    /// Compute the split from a tab group.
    pub fn from_group(group: &'a TabGroup) -> Self {
        let mut pinned = Vec::new();
        let mut unpinned = Vec::new();
        for tab in group.get_tabs() {
            if tab.pinned {
                pinned.push(tab);
            } else {
                unpinned.push(tab);
            }
        }
        Self { pinned, unpinned }
    }

    /// Total number of tabs.
    pub fn total(&self) -> usize {
        self.pinned.len() + self.unpinned.len()
    }

    /// Whether there are any pinned tabs.
    pub fn has_pinned(&self) -> bool {
        !self.pinned.is_empty()
    }

    /// Find a tab by ID across both areas.
    pub fn find(&self, id: &str) -> Option<&'a Tab> {
        self.pinned
            .iter()
            .chain(self.unpinned.iter())
            .find(|t| t.id == id)
            .copied()
    }
}

/// Ensure all pinned tabs appear before unpinned tabs in the group.
/// Returns the number of tabs that were moved.
pub fn sort_pinned_first(group: &mut TabGroup) -> usize {
    let tabs = group.get_tabs().to_vec();
    let mut pinned: Vec<Tab> = tabs.iter().filter(|t| t.pinned).cloned().collect();
    let unpinned: Vec<Tab> = tabs.iter().filter(|t| !t.pinned).cloned().collect();

    let mut moved = 0;
    let original_order: Vec<String> = tabs.iter().map(|t| t.id.clone()).collect();

    pinned.extend(unpinned);
    let new_order: Vec<String> = pinned.iter().map(|t| t.id.clone()).collect();

    for (i, id) in new_order.iter().enumerate() {
        if original_order.get(i) != Some(id) {
            moved += 1;
        }
    }

    // Rebuild group in correct order
    group.close_all();
    for tab in pinned {
        group.add_tab(tab);
    }

    moved
}

// ---------------------------------------------------------------------------
// Tab history — recently closed tabs for reopen support
// ---------------------------------------------------------------------------

/// Tracks recently closed tabs so they can be reopened.
#[derive(Debug, Clone)]
pub struct TabHistory {
    /// Closed tabs stored in LIFO order (most recent last).
    closed: Vec<Tab>,
    /// Maximum number of closed tabs to remember.
    capacity: usize,
}

impl TabHistory {
    /// Create a new history with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            closed: Vec::new(),
            capacity,
        }
    }

    /// Record a tab that was just closed.
    pub fn push(&mut self, tab: Tab) {
        if self.closed.len() >= self.capacity {
            self.closed.remove(0);
        }
        self.closed.push(tab);
    }

    /// Pop the most recently closed tab for reopening, or `None` if empty.
    pub fn pop(&mut self) -> Option<Tab> {
        self.closed.pop()
    }

    /// Peek at the most recently closed tab without removing it.
    pub fn peek(&self) -> Option<&Tab> {
        self.closed.last()
    }

    /// Number of closed tabs in history.
    pub fn len(&self) -> usize {
        self.closed.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.closed.is_empty()
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.closed.clear();
    }

    /// Return an iterator over the closed tabs (oldest first).
    pub fn iter(&self) -> impl Iterator<Item = &Tab> {
        self.closed.iter()
    }

    /// Find a closed tab by URI.
    pub fn find_by_uri(&self, uri: &str) -> Option<&Tab> {
        self.closed.iter().find(|t| t.uri.as_deref() == Some(uri))
    }

    /// Remove and return a specific closed tab by id, if present.
    pub fn remove_by_id(&mut self, id: &str) -> Option<Tab> {
        if let Some(pos) = self.closed.iter().position(|t| t.id == id) {
            Some(self.closed.remove(pos))
        } else {
            None
        }
    }
}

impl Default for TabHistory {
    fn default() -> Self {
        Self::new(20)
    }
}

// ---------------------------------------------------------------------------
// Tab search / filter
// ---------------------------------------------------------------------------

/// Result of matching a tab against a search query.
#[derive(Debug, Clone)]
pub struct TabSearchResult<'a> {
    pub tab: &'a Tab,
    pub score: u32,
}

/// Search tabs by label using a simple substring + prefix-bonus scoring.
pub fn search_tabs<'a>(tabs: &'a [Tab], query: &str) -> Vec<TabSearchResult<'a>> {
    if query.is_empty() {
        return tabs
            .iter()
            .map(|tab| TabSearchResult { tab, score: 0 })
            .collect();
    }
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();
    for tab in tabs {
        let label_lower = tab.label.to_lowercase();
        if let Some(pos) = label_lower.find(&query_lower) {
            let mut score: u32 = 100u32.saturating_sub(pos as u32);
            // Exact match bonus
            if label_lower == query_lower {
                score += 50;
            }
            // Prefix match bonus
            if pos == 0 {
                score += 25;
            }
            results.push(TabSearchResult { tab, score });
        }
    }
    results.sort_by(|a, b| b.score.cmp(&a.score));
    results
}

/// Filter tabs whose label contains the query (case-insensitive).
pub fn filter_tabs<'a>(tabs: &'a [Tab], query: &str) -> Vec<&'a Tab> {
    let query_lower = query.to_lowercase();
    tabs.iter()
        .filter(|t| t.label.to_lowercase().contains(&query_lower))
        .collect()
}

// ---------------------------------------------------------------------------
// Tab sizing calculations
// ---------------------------------------------------------------------------

/// Calculate individual tab widths given labels and constraints.
pub fn calculate_tab_widths(
    labels: &[&str],
    available_width: usize,
    min_width: usize,
    max_width: usize,
    padding: usize,
) -> Vec<usize> {
    if labels.is_empty() {
        return Vec::new();
    }
    let ideal_widths: Vec<usize> = labels
        .iter()
        .map(|l| (l.len() + padding).clamp(min_width, max_width))
        .collect();
    let total_ideal: usize = ideal_widths.iter().sum();
    if total_ideal <= available_width {
        return ideal_widths;
    }
    // Proportionally shrink, respecting min_width
    let scale = available_width as f64 / total_ideal as f64;
    ideal_widths
        .iter()
        .map(|&w| ((w as f64 * scale) as usize).max(min_width))
        .collect()
}

// ---------------------------------------------------------------------------
// Split view management
// ---------------------------------------------------------------------------

/// Represents a split editor pane, each containing its own tab group.
#[derive(Debug)]
pub struct SplitView {
    panes: Vec<TabGroup>,
    active_pane: usize,
}

impl SplitView {
    /// Create a split view with a single empty pane.
    pub fn new() -> Self {
        Self {
            panes: vec![TabGroup::new()],
            active_pane: 0,
        }
    }

    /// Number of panes.
    pub fn pane_count(&self) -> usize {
        self.panes.len()
    }

    /// Get the active pane index.
    pub fn active_pane_index(&self) -> usize {
        self.active_pane
    }

    /// Get a reference to the active pane's tab group.
    pub fn active_pane(&self) -> &TabGroup {
        &self.panes[self.active_pane]
    }

    /// Get a mutable reference to the active pane's tab group.
    pub fn active_pane_mut(&mut self) -> &mut TabGroup {
        &mut self.panes[self.active_pane]
    }

    /// Add a new split pane. Returns the index of the new pane.
    pub fn split(&mut self) -> usize {
        self.panes.push(TabGroup::new());
        self.panes.len() - 1
    }

    /// Focus a specific pane by index. Returns false if out of range.
    pub fn focus_pane(&mut self, index: usize) -> bool {
        if index < self.panes.len() {
            self.active_pane = index;
            true
        } else {
            false
        }
    }

    /// Close a pane by index, moving its tabs nowhere (they are lost).
    /// Cannot close the last remaining pane.
    /// Returns the closed pane's tabs, or `None` if it can't be closed.
    pub fn close_pane(&mut self, index: usize) -> Option<Vec<Tab>> {
        if self.panes.len() <= 1 || index >= self.panes.len() {
            return None;
        }
        let mut removed = self.panes.remove(index);
        let tabs = removed.close_all();
        if self.active_pane >= self.panes.len() {
            self.active_pane = self.panes.len() - 1;
        }
        Some(tabs)
    }

    /// Move a tab from one pane to another.
    /// Returns `true` on success.
    pub fn move_tab_to_pane(
        &mut self,
        tab_id: &str,
        from_pane: usize,
        to_pane: usize,
    ) -> bool {
        if from_pane >= self.panes.len() || to_pane >= self.panes.len() || from_pane == to_pane {
            return false;
        }
        // Find and remove the tab from source pane
        let tab = {
            let src = &self.panes[from_pane];
            src.get_tab(tab_id).cloned()
        };
        if let Some(tab) = tab {
            self.panes[from_pane].close_tab(tab_id);
            self.panes[to_pane].add_tab(tab);
            true
        } else {
            false
        }
    }

    /// Get a reference to a pane by index.
    pub fn get_pane(&self, index: usize) -> Option<&TabGroup> {
        self.panes.get(index)
    }

    /// Total number of tabs across all panes.
    pub fn total_tab_count(&self) -> usize {
        self.panes.iter().map(|p| p.tab_count()).sum()
    }
}

impl Default for SplitView {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Drag-and-drop reordering
// ---------------------------------------------------------------------------

/// The result of a completed drag-and-drop reorder operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DragResult {
    pub tab_id: String,
    pub from_index: usize,
    pub to_index: usize,
}

impl fmt::Display for DragResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Moved tab '{}' from index {} to {}",
            self.tab_id, self.from_index, self.to_index
        )
    }
}

/// Tracks the state of a tab drag-reorder gesture.
#[derive(Debug, Clone, Default)]
pub struct TabBarDragReorder {
    pub dragging: Option<String>,
    pub original_index: Option<usize>,
    pub current_index: Option<usize>,
}

impl TabBarDragReorder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin dragging the tab identified by `tab_id` from `index`.
    pub fn start_drag(&mut self, tab_id: &str, index: usize) {
        self.dragging = Some(tab_id.to_string());
        self.original_index = Some(index);
        self.current_index = Some(index);
    }

    /// Update the position the dragged tab has been moved to.
    pub fn move_to(&mut self, index: usize) {
        if self.dragging.is_some() {
            self.current_index = Some(index);
        }
    }

    /// Complete the drag and return the result if the tab actually moved.
    pub fn end_drag(&mut self) -> Option<DragResult> {
        let tab_id = self.dragging.take()?;
        let from = self.original_index.take()?;
        let to = self.current_index.take()?;
        if from == to {
            return None;
        }
        Some(DragResult {
            tab_id,
            from_index: from,
            to_index: to,
        })
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging.is_some()
    }

    /// Cancel the current drag without producing a result.
    pub fn cancel(&mut self) {
        self.dragging = None;
        self.original_index = None;
        self.current_index = None;
    }
}

// ---------------------------------------------------------------------------
// Tab-bar overflow
// ---------------------------------------------------------------------------

/// Manages the overflow dropdown that appears when tabs exceed visible space.
#[derive(Debug, Clone)]
pub struct TabBarOverflow {
    pub visible_count: usize,
    pub total_count: usize,
    pub overflow_items: Vec<String>,
}

impl TabBarOverflow {
    pub fn new(visible: usize, total: usize) -> Self {
        Self {
            visible_count: visible,
            total_count: total,
            overflow_items: Vec::new(),
        }
    }

    pub fn has_overflow(&self) -> bool {
        self.total_count > self.visible_count
    }

    pub fn overflow_count(&self) -> usize {
        self.total_count.saturating_sub(self.visible_count)
    }

    pub fn add_overflow_item(&mut self, label: &str) {
        self.overflow_items.push(label.to_string());
    }

    /// A short label such as "+3 more" for use in the UI.
    pub fn overflow_label(&self) -> String {
        let n = self.overflow_count();
        if n == 0 {
            String::new()
        } else {
            format!("+{n} more")
        }
    }

    pub fn clear(&mut self) {
        self.overflow_items.clear();
    }
}

// ---------------------------------------------------------------------------
// Preview / transient tabs
// ---------------------------------------------------------------------------

/// Manages a single preview (transient) tab that can be promoted to permanent.
#[derive(Debug, Clone, Default)]
pub struct TabBarPreview {
    pub preview_tab_id: Option<String>,
    pub auto_close_on_edit: bool,
}

impl TabBarPreview {
    pub fn new() -> Self {
        Self {
            preview_tab_id: None,
            auto_close_on_edit: true,
        }
    }

    pub fn set_preview(&mut self, tab_id: &str) {
        self.preview_tab_id = Some(tab_id.to_string());
    }

    pub fn clear_preview(&mut self) {
        self.preview_tab_id = None;
    }

    pub fn is_preview(&self, tab_id: &str) -> bool {
        self.preview_tab_id.as_deref() == Some(tab_id)
    }

    /// Promote the preview tab to a permanent tab.
    /// Returns `true` if `tab_id` was the current preview.
    pub fn promote_to_permanent(&mut self, tab_id: &str) -> bool {
        if self.is_preview(tab_id) {
            self.preview_tab_id = None;
            true
        } else {
            false
        }
    }

    pub fn has_preview(&self) -> bool {
        self.preview_tab_id.is_some()
    }
}

// ---------------------------------------------------------------------------
// Close confirmation
// ---------------------------------------------------------------------------

/// Why a tab close requires confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseReason {
    Dirty,
    Pinned,
    LastTab,
}

impl fmt::Display for CloseReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CloseReason::Dirty => write!(f, "unsaved changes"),
            CloseReason::Pinned => write!(f, "tab is pinned"),
            CloseReason::LastTab => write!(f, "last remaining tab"),
        }
    }
}

/// A pending close request that may need user confirmation.
#[derive(Debug, Clone)]
pub struct CloseRequest {
    pub tab_id: String,
    pub reason: CloseReason,
    pub confirmed: bool,
}

/// Collects and resolves close-confirmation requests.
#[derive(Debug, Clone, Default)]
pub struct TabCloseConfirmation {
    pub pending: Vec<CloseRequest>,
}

impl TabCloseConfirmation {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new close request for the given tab.
    pub fn request_close(&mut self, tab_id: &str, reason: CloseReason) {
        self.pending.push(CloseRequest {
            tab_id: tab_id.to_string(),
            reason,
            confirmed: false,
        });
    }

    /// Confirm the close of a tab. Returns `true` if a matching request was found.
    pub fn confirm(&mut self, tab_id: &str) -> bool {
        for req in &mut self.pending {
            if req.tab_id == tab_id && !req.confirmed {
                req.confirmed = true;
                return true;
            }
        }
        false
    }

    /// Reject (remove) a close request. Returns `true` if a matching request was found.
    pub fn reject(&mut self, tab_id: &str) -> bool {
        let before = self.pending.len();
        self.pending.retain(|r| r.tab_id != tab_id);
        self.pending.len() < before
    }

    /// Number of unconfirmed pending close requests.
    pub fn pending_count(&self) -> usize {
        self.pending.iter().filter(|r| !r.confirmed).count()
    }

    /// A tab needs confirmation before closing if it is dirty or pinned.
    pub fn needs_confirmation(tab: &Tab) -> bool {
        tab.dirty || tab.pinned
    }
}


// === Tab Group Drag Handler ===

/// Tab Group Drag Handler implementation.
#[derive(Debug, Clone)]
pub struct TabGroupDragHandler {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: TabGroupDragHandlerStats,
}

/// Statistics for TabGroupDragHandler.
#[derive(Debug, Clone, Default)]
pub struct TabGroupDragHandlerStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl TabGroupDragHandlerStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl TabGroupDragHandler {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: TabGroupDragHandlerStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &TabGroupDragHandlerStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for TabGroupDragHandler {
    fn default() -> Self {
        Self::new()
    }
}

// === Tab Context Menu Builder ===

/// Priority level for TabContextMenuBuilder items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TabContextMenuBuilderPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl TabContextMenuBuilderPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for TabContextMenuBuilderPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Tab Context Menu Builder implementation.
#[derive(Debug, Clone)]
pub struct TabContextMenuBuilder {
    items: Vec<TabContextMenuBuilderItem>,
    max_items: usize,
    default_priority: TabContextMenuBuilderPriority,
}

/// A single item in TabContextMenuBuilder.
#[derive(Debug, Clone)]
pub struct TabContextMenuBuilderItem {
    pub id: String,
    pub label: String,
    pub priority: TabContextMenuBuilderPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl TabContextMenuBuilderItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: TabContextMenuBuilderPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: TabContextMenuBuilderPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl TabContextMenuBuilder {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: TabContextMenuBuilderPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: TabContextMenuBuilderItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<TabContextMenuBuilderItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&TabContextMenuBuilderItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: TabContextMenuBuilderPriority) -> Vec<&TabContextMenuBuilderItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&TabContextMenuBuilderItem> {
        let mut sorted: Vec<&TabContextMenuBuilderItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&TabContextMenuBuilderItem> {
        let mut sorted: Vec<&TabContextMenuBuilderItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&TabContextMenuBuilderItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: TabContextMenuBuilderPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> TabContextMenuBuilderPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &TabContextMenuBuilderItem> {
        self.items.iter()
    }
}

impl Default for TabContextMenuBuilder {
    fn default() -> Self {
        Self::new()
    }
}


// ─── TabBar Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for closed tabs.
#[derive(Debug, Clone)]
pub struct TabBarRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> TabBarRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for TabBarRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TabBarRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── TabBar Builder & Validator ─────────────────────────────

/// Builder for constructing tab bar configurations.
#[derive(Debug, Clone)]
pub struct TabBarBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl TabBarBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<TabBarCfg, TabBarBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(TabBarBuildErr { errors }); }
        Ok(TabBarCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated tab bar configuration.
#[derive(Debug, Clone)]
pub struct TabBarCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl TabBarCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &TabBarCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for TabBarCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TabBarCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct TabBarBuildErr { pub errors: Vec<String> }

impl fmt::Display for TabBarBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TabBarBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for TabBarBuildErr {}


// ---------------------------------------------------------------------------
// Tab bar state and ordering — extended utilities (zw)
// ---------------------------------------------------------------------------

/// Metric accumulator for tabbar operations.
#[derive(Debug, Clone)]
pub struct ZwMetrics {
    samples: Vec<f64>,
    label: String,
}

impl ZwMetrics {
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

/// Sliding-window rate counter for tabbar.
#[derive(Debug, Clone)]
pub struct ZwRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl ZwRateWindow {
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

/// A small LRU-style cache for tabbar lookups.
#[derive(Debug, Clone)]
pub struct ZwLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZwLruCache {
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
// xa_ extended helpers for tabbar
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaTabbarRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaTabbarRingBuf {
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
pub struct XaTabbarCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaTabbarCounter {
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

impl Default for XaTabbarCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 172
// ---------------------------------------------------------------------------

/// Generic object pool `Xc172Pool<T>`.
pub struct Xc172Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc172Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc172PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc172Pool<T> {
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
    pub fn stats(&self) -> Xc172PoolStats {
        Xc172PoolStats {
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

impl<T> Default for Xc172Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc172Scheduler`.
pub struct Xc172Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc172Scheduler {
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

impl Default for Xc172Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_172 hash for the given byte slice.
pub fn xc_172_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_172 convention.
pub fn xc_172_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_41 deepening: state machine + event bus ---

/// States for the Xd41 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd41State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd41State {
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
pub struct Xd41Transition {
    pub from: Xd41State,
    pub to: Xd41State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd41StateMachine {
    current: Xd41State,
    history: Vec<Xd41Transition>,
    step_counter: usize,
}

impl Xd41StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd41State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd41State {
        self.current
    }

    pub fn history(&self) -> &[Xd41Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd41State) -> Result<Xd41State, String> {
        let allowed = match (self.current, target) {
            (Xd41State::Idle, Xd41State::Running) => true,
            (Xd41State::Running, Xd41State::Paused) => true,
            (Xd41State::Running, Xd41State::Done) => true,
            (Xd41State::Paused, Xd41State::Running) => true,
            (Xd41State::Paused, Xd41State::Done) => true,
            (Xd41State::Done, Xd41State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_41: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd41Transition {
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
            "Xd41SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd41State> {
        let prefix = "Xd41SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd41State::Idle),
            "Running" => Some(Xd41State::Running),
            "Paused" => Some(Xd41State::Paused),
            "Done" => Some(Xd41State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd41State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd41 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd41Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd41Event {
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

type Xd41HandlerFn = Box<dyn Fn(&Xd41Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd41EventBus {
    handlers: Vec<(usize, Option<String>, Xd41HandlerFn)>,
    next_id: usize,
    published: Vec<Xd41Event>,
}

impl Xd41EventBus {
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
        F: Fn(&Xd41Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd41Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd41Event) {
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

    pub fn published_events(&self) -> &[Xd41Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #39
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf39Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf39TrieNode {
    children: std::collections::HashMap<char, Xf39TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf39Trie {
    root: Xf39TrieNode,
    count: usize,
}

impl Xf39Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf39TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf39TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf39TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf39BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf39BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 171).
pub struct Xh171SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh171SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 213 as u64,
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

/// A compact bit set supporting boolean operations (variant 171).
pub struct Xh171BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh171BitSet {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tab(id: &str) -> Tab {
        Tab {
            id: id.to_string(),
            label: id.to_string(),
            uri: None,
            kind: TabKind::File,
            dirty: false,
            pinned: false,
            preview: false,
            active: false,
        }
    }

    fn make_tab_with_uri(id: &str, uri: &str) -> Tab {
        Tab {
            id: id.to_string(),
            label: id.to_string(),
            uri: Some(uri.to_string()),
            kind: TabKind::File,
            dirty: false,
            pinned: false,
            preview: false,
            active: false,
        }
    }

    #[test]
    fn add_activate_close() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        group.add_tab(make_tab("b"));
        group.activate_tab("b");
        assert_eq!(group.get_active_tab().unwrap().id, "b");
        assert!(group.close_tab("b"));
        assert_eq!(group.tab_count(), 1);
    }

    #[test]
    fn dirty_and_close_saved() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        group.add_tab(make_tab("b"));
        group.mark_dirty("a");
        assert_eq!(group.get_dirty_tabs().len(), 1);
        group.close_saved_tabs();
        assert_eq!(group.tab_count(), 1);
        assert_eq!(group.tabs[0].id, "a");
    }

    #[test]
    fn pin_and_unpin() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("x"));
        group.pin_tab("x");
        assert!(group.tabs[0].pinned);
        group.close_saved_tabs();
        assert_eq!(group.tab_count(), 1);
        group.unpin_tab("x");
        group.close_saved_tabs();
        assert_eq!(group.tab_count(), 0);
    }

    #[test]
    fn move_tab_reorders() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        group.add_tab(make_tab("b"));
        group.add_tab(make_tab("c"));
        assert!(group.move_tab("a", 2));
        assert_eq!(group.get_tabs()[0].id, "b");
        assert_eq!(group.get_tabs()[2].id, "a");
    }

    #[test]
    fn move_tab_invalid_index() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        assert!(!group.move_tab("a", 5));
        assert!(!group.move_tab("missing", 0));
    }

    #[test]
    fn get_tab_returns_correct() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("x"));
        assert!(group.get_tab("x").is_some());
        assert!(group.get_tab("missing").is_none());
    }

    #[test]
    fn close_all_drains() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        group.add_tab(make_tab("b"));
        let closed = group.close_all();
        assert_eq!(closed.len(), 2);
        assert_eq!(group.tab_count(), 0);
    }

    #[test]
    fn close_others_keeps_target() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        group.add_tab(make_tab("b"));
        group.add_tab(make_tab("c"));
        let closed = group.close_others("b");
        assert_eq!(closed.len(), 2);
        assert_eq!(group.tab_count(), 1);
        assert_eq!(group.get_tabs()[0].id, "b");
    }

    #[test]
    fn close_to_right_and_left() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        group.add_tab(make_tab("b"));
        group.add_tab(make_tab("c"));
        group.add_tab(make_tab("d"));
        let right = group.close_to_right("b");
        assert_eq!(right.len(), 2);
        assert_eq!(group.tab_count(), 2);

        let left = group.close_to_left("b");
        assert_eq!(left.len(), 1);
        assert_eq!(group.tab_count(), 1);
        assert_eq!(group.get_tabs()[0].id, "b");
    }

    #[test]
    fn pinned_and_preview_tabs() {
        let mut group = TabGroup::new();
        let mut t1 = make_tab("a");
        t1.pinned = true;
        let mut t2 = make_tab("b");
        t2.preview = true;
        group.add_tab(t1);
        group.add_tab(t2);
        group.add_tab(make_tab("c"));
        assert_eq!(group.get_pinned_tabs().len(), 1);
        assert_eq!(group.get_preview_tabs().len(), 1);
        group.promote_preview("b");
        assert_eq!(group.get_preview_tabs().len(), 0);
    }

    #[test]
    fn find_by_uri_works() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab_with_uri("a", "file:///a.rs"));
        group.add_tab(make_tab("b"));
        assert_eq!(group.find_by_uri("file:///a.rs").unwrap().id, "a");
        assert!(group.find_by_uri("file:///missing").is_none());
    }

    #[test]
    fn tab_bar_config_defaults() {
        let config = TabBarConfig::default();
        assert!(config.show_icons);
        assert_eq!(config.tab_sizing, TabSizing::Fit);
        assert_eq!(config.close_button_position, CloseButtonPosition::Right);
    }

    #[test]
    fn tab_sizing_and_close_button_variants() {
        let _fit = TabSizing::Fit;
        let _fixed = TabSizing::Fixed;
        let _shrink = TabSizing::Shrink;
        let _left = CloseButtonPosition::Left;
        let _right = CloseButtonPosition::Right;
        let _off = CloseButtonPosition::Off;
        assert_ne!(TabSizing::Fit, TabSizing::Fixed);
        assert_ne!(CloseButtonPosition::Left, CloseButtonPosition::Off);
    }

    #[test]
    fn eq_tabkind_same() {
        assert_eq!(TabKind::File, TabKind::File);
    }

    #[test]
    fn ne_tabkind_diff() {
        assert_ne!(TabKind::File, TabKind::Preview);
    }

    #[test]
    fn eq_tabsizing_same() {
        assert_eq!(TabSizing::Fit, TabSizing::Fit);
    }

    #[test]
    fn ne_tabsizing_diff() {
        assert_ne!(TabSizing::Fit, TabSizing::Fixed);
    }

    #[test]
    fn eq_closebuttonposition_same() {
        assert_eq!(CloseButtonPosition::Left, CloseButtonPosition::Left);
    }

    #[test]
    fn ne_closebuttonposition_diff() {
        assert_ne!(CloseButtonPosition::Left, CloseButtonPosition::Right);
    }

    #[test]
    fn drag_reorder_calculate_insert_start() {
        let drag = TabDragReorder::start("t1", 10.0);
        let positions = vec![(0.0, 100.0), (100.0, 100.0), (200.0, 100.0)];
        assert_eq!(drag.calculate_insert_index(&positions), 0);
    }

    #[test]
    fn drag_reorder_calculate_insert_middle() {
        let drag = TabDragReorder::start("t1", 160.0);
        let positions = vec![(0.0, 100.0), (100.0, 100.0), (200.0, 100.0)];
        assert_eq!(drag.calculate_insert_index(&positions), 2);
    }

    #[test]
    fn drag_reorder_calculate_insert_end() {
        let drag = TabDragReorder::start("t1", 500.0);
        let positions = vec![(0.0, 100.0), (100.0, 100.0), (200.0, 100.0)];
        assert_eq!(drag.calculate_insert_index(&positions), 3);
    }

    #[test]
    fn drag_reorder_cancel() {
        let mut drag = TabDragReorder::start("t1", 50.0);
        assert!(drag.active);
        drag.cancel();
        assert!(!drag.active);
    }

    #[test]
    fn drag_reorder_update_position() {
        let mut drag = TabDragReorder::start("t1", 50.0);
        drag.update_position(150.0);
        assert_eq!(drag.drag_x, 150.0);
    }

    #[test]
    fn tab_overflow_not_overflowing() {
        let mut ov = TabOverflow::new(5);
        ov.update_total(3);
        assert!(!ov.is_overflowing());
        assert_eq!(ov.visible_range(), 0..3);
        assert_eq!(ov.hidden_left(), 0);
        assert_eq!(ov.hidden_right(), 0);
    }

    #[test]
    fn tab_overflow_overflowing() {
        let mut ov = TabOverflow::new(3);
        ov.update_total(7);
        assert!(ov.is_overflowing());
        assert_eq!(ov.visible_range(), 0..3);
        assert_eq!(ov.hidden_right(), 4);
    }

    #[test]
    fn tab_overflow_scroll_right_and_left() {
        let mut ov = TabOverflow::new(3);
        ov.update_total(7);
        ov.scroll_right();
        assert_eq!(ov.scroll_offset, 1);
        assert_eq!(ov.visible_range(), 1..4);
        assert_eq!(ov.hidden_left(), 1);
        assert_eq!(ov.hidden_right(), 3);
        ov.scroll_left();
        assert_eq!(ov.scroll_offset, 0);
    }

    #[test]
    fn tab_overflow_ensure_visible() {
        let mut ov = TabOverflow::new(3);
        ov.update_total(10);
        ov.ensure_visible(5);
        assert!(ov.visible_range().contains(&5));
        ov.ensure_visible(1);
        assert!(ov.visible_range().contains(&1));
    }

    #[test]
    fn tab_overflow_menu_indices() {
        let mut ov = TabOverflow::new(3);
        ov.update_total(5);
        let menu = ov.overflow_menu_indices();
        assert_eq!(menu, vec![3, 4]);
    }

    #[test]
    fn tab_overflow_clamp_scroll_on_update() {
        let mut ov = TabOverflow::new(3);
        ov.update_total(10);
        ov.scroll_offset = 8;
        ov.update_total(10); // should clamp to 7
        assert_eq!(ov.scroll_offset, 7);
        ov.update_total(2); // less than max_visible
        assert_eq!(ov.scroll_offset, 0);
    }

    #[test]
    fn close_animation_tick_progress() {
        let mut anim = tab_close_animation("tab1", 200);
        anim.start();
        assert!(!anim.is_complete());
        assert_eq!(anim.opacity(), 1.0);
        let done = anim.tick(100);
        assert!(!done);
        assert!((anim.progress - 0.5).abs() < f64::EPSILON);
        assert!((anim.opacity() - 0.5).abs() < f64::EPSILON);
        assert!((anim.width_scale() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn close_animation_completes() {
        let mut anim = tab_close_animation("tab1", 100);
        anim.start();
        let done = anim.tick(100);
        assert!(done);
        assert!(anim.is_complete());
        assert!((anim.opacity()).abs() < f64::EPSILON);
    }

    #[test]
    fn close_animation_not_started() {
        let mut anim = tab_close_animation("tab1", 100);
        assert!(!anim.tick(50));
        assert!(!anim.is_complete());
    }

    #[test]
    fn close_animation_overshoot_clamped() {
        let mut anim = tab_close_animation("tab1", 100);
        anim.start();
        anim.tick(200);
        assert_eq!(anim.progress, 1.0);
        assert!(anim.is_complete());
    }

    #[test]
    fn drag_reorder_finish_on_cancelled() {
        let mut drag = TabDragReorder::start("t1", 50.0);
        drag.cancel();
        let mut group = TabGroup::new();
        let result = drag.finish(&mut group, &[]);
        assert!(result.is_none());
    }

    #[test]
    fn tab_overflow_scroll_right_clamped() {
        let mut ov = TabOverflow::new(3);
        ov.update_total(3);
        ov.scroll_right(); // Should not scroll past end
        assert_eq!(ov.scroll_offset, 0);
    }

    #[test]
    fn tab_overflow_scroll_left_clamped() {
        let mut ov = TabOverflow::new(3);
        ov.update_total(10);
        ov.scroll_left(); // Already at 0
        assert_eq!(ov.scroll_offset, 0);
    }

    #[test]
    fn tabbar_stats_new_defaults() {
        let stats = TabbarStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn tabbar_stats_record_success() {
        let mut stats = TabbarStats::new();
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
    fn tabbar_stats_record_failure() {
        let mut stats = TabbarStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn tabbar_stats_reset() {
        let mut stats = TabbarStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn tabbar_stats_merge() {
        let mut a = TabbarStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = TabbarStats::new();
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
    fn tabbar_stats_display() {
        let mut stats = TabbarStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn tabbar_stats_default() {
        let stats = TabbarStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn tabbar_validator_accepts_valid_name() {
        let v = TabbarValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn tabbar_validator_rejects_empty() {
        let v = TabbarValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn tabbar_validator_rejects_too_long() {
        let v = TabbarValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn tabbar_validator_forbidden_prefix() {
        let v = TabbarValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn tabbar_validator_allowed_chars() {
        let v = TabbarValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn tabbar_validator_range() {
        let v = TabbarValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn tabbar_sanitize_removes_control() {
        let result = TabbarValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn tabbar_truncate_short_string() {
        assert_eq!(TabbarValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn tabbar_truncate_long_string() {
        let result = TabbarValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn tabbar_is_ascii_printable() {
        assert!(TabbarValidator::is_ascii_printable("Hello World 123"));
        assert!(!TabbarValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn tab_is_pinned() {
        let mut tab = make_tab("a");
        assert!(!tab.is_pinned());
        tab.pinned = true;
        assert!(tab.is_pinned());
    }

    #[test]
    fn tab_is_dirty() {
        let mut tab = make_tab("a");
        assert!(!tab.is_dirty());
        tab.dirty = true;
        assert!(tab.is_dirty());
    }

    #[test]
    fn tabgroup_pinned_count() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        group.add_tab(make_tab("b"));
        group.add_tab(make_tab("c"));
        assert_eq!(group.pinned_count(), 0);
        group.pin_tab("a");
        group.pin_tab("c");
        assert_eq!(group.pinned_count(), 2);
    }

    #[test]
    fn tabgroup_dirty_count() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        group.add_tab(make_tab("b"));
        assert_eq!(group.dirty_count(), 0);
        group.mark_dirty("b");
        assert_eq!(group.dirty_count(), 1);
    }

    #[test]
    fn tabgroup_find_by_label() {
        let mut group = TabGroup::new();
        let mut tab = make_tab("id1");
        tab.label = "My Label".to_string();
        group.add_tab(tab);
        group.add_tab(make_tab("id2"));
        assert_eq!(group.find_by_label("My Label").unwrap().id, "id1");
        assert!(group.find_by_label("nonexistent").is_none());
    }

    #[test]
    fn tabgroup_is_empty() {
        let mut group = TabGroup::new();
        assert!(group.is_empty());
        group.add_tab(make_tab("a"));
        assert!(!group.is_empty());
    }

    #[test]
    fn tabkind_label() {
        assert_eq!(TabKind::File.label(), "File");
        assert_eq!(TabKind::Preview.label(), "Preview");
        assert_eq!(TabKind::Diff.label(), "Diff");
        assert_eq!(TabKind::Settings.label(), "Settings");
        assert_eq!(TabKind::Welcome.label(), "Welcome");
        assert_eq!(TabKind::Custom("foo".to_string()).label(), "Custom");
    }

    #[test]
    fn tabgroup_display() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        group.add_tab(make_tab("b"));
        group.mark_dirty("a");
        let s = format!("{group}");
        assert_eq!(s, "2 tabs (1 dirty)");
    }

    #[test]
    fn tabgroup_get_tab_index() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        group.add_tab(make_tab("b"));
        group.add_tab(make_tab("c"));
        assert_eq!(group.get_tab_index("a"), Some(0));
        assert_eq!(group.get_tab_index("c"), Some(2));
        assert_eq!(group.get_tab_index("missing"), None);
    }

    // -- TabLayout tests -----------------------------------------------------

    #[test]
    fn layout_shrink_computes_widths() {
        let layout = TabLayout::new(200, TabOverflowStrategy::Shrink);
        let widths = layout.compute_widths(5);
        assert_eq!(widths.len(), 5);
        assert_eq!(widths[0], 40); // 200/5 = 40, clamped to max 40
    }

    #[test]
    fn layout_overflow_detection() {
        let layout = TabLayout::new(100, TabOverflowStrategy::Scroll);
        // 100 / 20 = 5 tabs visible at min_width=8 → actually 100/8=12
        // With 20 tabs: ideal=5, clamped to min 8 → 100/8 = 12 visible, 20 total → overflow
        assert!(layout.is_overflowing(20));
        assert!(!layout.is_overflowing(3));
    }

    // -- TabDragOperation tests ----------------------------------------------

    #[test]
    fn drag_operation_applies_reorder() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        group.add_tab(make_tab("b"));
        group.add_tab(make_tab("c"));
        let op = TabDragOperation::new("a", 0, 2);
        assert!(op.is_move());
        assert_eq!(op.distance(), 2);
        assert!(apply_drag_reorder(&mut group, &op));
        assert_eq!(group.get_tab_index("a"), Some(2));
    }

    #[test]
    fn drag_noop_when_same_index() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        let op = TabDragOperation::new("a", 0, 0);
        assert!(!op.is_move());
        assert!(!apply_drag_reorder(&mut group, &op));
    }

    // -- PinnedAreaSplit tests ------------------------------------------------

    #[test]
    fn pinned_area_split_separates() {
        let mut group = TabGroup::new();
        let mut t1 = make_tab("a");
        t1.pinned = true;
        group.add_tab(t1);
        group.add_tab(make_tab("b"));

        let split = PinnedAreaSplit::from_group(&group);
        assert_eq!(split.pinned.len(), 1);
        assert_eq!(split.unpinned.len(), 1);
        assert!(split.has_pinned());
        assert_eq!(split.total(), 2);
        assert!(split.find("a").is_some());
        assert!(split.find("missing").is_none());
    }

    #[test]
    fn sort_pinned_first_reorders() {
        let mut group = TabGroup::new();
        group.add_tab(make_tab("a"));
        let mut pinned = make_tab("b");
        pinned.pinned = true;
        group.add_tab(pinned);
        group.add_tab(make_tab("c"));
        let moved = sort_pinned_first(&mut group);
        assert!(moved > 0);
        assert_eq!(group.get_tab_index("b"), Some(0));
    }

    // -- TabHistory tests ----------------------------------------------------

    #[test]
    fn tab_history_push_pop() {
        let mut history = TabHistory::new(5);
        assert!(history.is_empty());
        history.push(make_tab("a"));
        history.push(make_tab("b"));
        assert_eq!(history.len(), 2);
        let reopened = history.pop().unwrap();
        assert_eq!(reopened.id, "b");
        let reopened = history.pop().unwrap();
        assert_eq!(reopened.id, "a");
        assert!(history.pop().is_none());
    }

    #[test]
    fn tab_history_capacity_eviction() {
        let mut history = TabHistory::new(2);
        history.push(make_tab("a"));
        history.push(make_tab("b"));
        history.push(make_tab("c"));
        assert_eq!(history.len(), 2);
        // "a" should have been evicted
        assert!(history.find_by_uri("a").is_none());
        let t = history.pop().unwrap();
        assert_eq!(t.id, "c");
    }

    #[test]
    fn tab_history_find_by_uri_and_remove() {
        let mut history = TabHistory::new(10);
        history.push(make_tab_with_uri("t1", "file:///foo.rs"));
        history.push(make_tab("t2"));
        assert!(history.find_by_uri("file:///foo.rs").is_some());
        assert!(history.find_by_uri("file:///bar.rs").is_none());
        let removed = history.remove_by_id("t1").unwrap();
        assert_eq!(removed.id, "t1");
        assert_eq!(history.len(), 1);
        assert!(history.remove_by_id("missing").is_none());
    }

    // -- search_tabs / filter_tabs tests -------------------------------------

    #[test]
    fn search_tabs_scores_and_order() {
        let tabs = vec![
            make_tab("readme"),
            make_tab("lib"),
            make_tab("main"),
        ];
        // Change labels to something meaningful
        let mut tabs = tabs;
        tabs[0].label = "README.md".to_string();
        tabs[1].label = "lib.rs".to_string();
        tabs[2].label = "main.rs".to_string();

        let results = search_tabs(&tabs, "main");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tab.id, "main");

        // Empty query returns all
        let all = search_tabs(&tabs, "");
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn filter_tabs_case_insensitive() {
        let mut tabs = vec![make_tab("a"), make_tab("b"), make_tab("c")];
        tabs[0].label = "Cargo.toml".to_string();
        tabs[1].label = "cargo.lock".to_string();
        tabs[2].label = "README.md".to_string();

        let filtered = filter_tabs(&tabs, "cargo");
        assert_eq!(filtered.len(), 2);
    }

    // -- calculate_tab_widths tests ------------------------------------------

    #[test]
    fn tab_widths_fit_within_available() {
        let labels = vec!["ab", "cdef", "g"];
        let widths = calculate_tab_widths(&labels, 200, 5, 40, 4);
        // All should fit: "ab"+4=6, "cdef"+4=8, "g"+4=5 → total=19 ≤ 200
        assert_eq!(widths, vec![6, 8, 5]);
    }

    #[test]
    fn tab_widths_shrink_proportionally() {
        let labels = vec!["abcdefghij"; 10]; // 10 + 4 = 14 each, total 140
        let widths = calculate_tab_widths(&labels, 50, 3, 40, 4);
        // Must shrink. Each should be >= min_width
        for &w in &widths {
            assert!(w >= 3);
        }
        assert_eq!(widths.len(), 10);
    }

    // -- SplitView tests -----------------------------------------------------

    #[test]
    fn split_view_basic_operations() {
        let mut sv = SplitView::new();
        assert_eq!(sv.pane_count(), 1);
        assert_eq!(sv.active_pane_index(), 0);

        sv.active_pane_mut().add_tab(make_tab("a"));
        assert_eq!(sv.total_tab_count(), 1);

        let idx = sv.split();
        assert_eq!(idx, 1);
        assert_eq!(sv.pane_count(), 2);

        assert!(sv.focus_pane(1));
        assert_eq!(sv.active_pane_index(), 1);
        assert!(!sv.focus_pane(99));
    }

    #[test]
    fn split_view_move_tab_between_panes() {
        let mut sv = SplitView::new();
        sv.active_pane_mut().add_tab(make_tab("t1"));
        sv.active_pane_mut().add_tab(make_tab("t2"));
        let pane1 = sv.split();

        assert!(sv.move_tab_to_pane("t1", 0, pane1));
        assert_eq!(sv.get_pane(0).unwrap().tab_count(), 1);
        assert_eq!(sv.get_pane(pane1).unwrap().tab_count(), 1);
        assert_eq!(sv.total_tab_count(), 2);

        // Can't move to same pane or invalid pane
        assert!(!sv.move_tab_to_pane("t2", 0, 0));
        assert!(!sv.move_tab_to_pane("t2", 0, 99));
    }

    #[test]
    fn split_view_close_pane() {
        let mut sv = SplitView::new();
        sv.active_pane_mut().add_tab(make_tab("a"));
        let p1 = sv.split();
        sv.focus_pane(p1);
        sv.active_pane_mut().add_tab(make_tab("b"));

        // Can't close the last pane
        let tabs = sv.close_pane(p1);
        assert!(tabs.is_some());
        assert_eq!(tabs.unwrap().len(), 1);
        assert_eq!(sv.pane_count(), 1);

        // Now only one pane left — can't close it
        assert!(sv.close_pane(0).is_none());
    }

    // -----------------------------------------------------------------------
    // TabBarDragReorder tests
    // -----------------------------------------------------------------------

    #[test]
    fn drag_reorder_basic_flow() {
        let mut dr = TabBarDragReorder::new();
        assert!(!dr.is_dragging());

        dr.start_drag("tab1", 0);
        assert!(dr.is_dragging());

        dr.move_to(2);
        let result = dr.end_drag().expect("should produce a result");
        assert_eq!(result.tab_id, "tab1");
        assert_eq!(result.from_index, 0);
        assert_eq!(result.to_index, 2);
        assert!(!dr.is_dragging());
    }

    #[test]
    fn drag_reorder_no_move() {
        let mut dr = TabBarDragReorder::new();
        dr.start_drag("tab1", 3);
        // End without moving — same index, should return None
        assert!(dr.end_drag().is_none());
    }

    #[test]
    fn drag_reorder_cancel_resets_state() {
        let mut dr = TabBarDragReorder::new();
        dr.start_drag("tab1", 1);
        dr.move_to(4);
        dr.cancel();
        assert!(!dr.is_dragging());
        assert!(dr.end_drag().is_none());
    }

    #[test]
    fn drag_result_display() {
        let result = DragResult {
            tab_id: "file.rs".to_string(),
            from_index: 0,
            to_index: 3,
        };
        assert_eq!(result.to_string(), "Moved tab 'file.rs' from index 0 to 3");
    }

    // -----------------------------------------------------------------------
    // TabBarOverflow tests
    // -----------------------------------------------------------------------

    #[test]
    fn overflow_basic() {
        let ov = TabBarOverflow::new(5, 8);
        assert!(ov.has_overflow());
        assert_eq!(ov.overflow_count(), 3);
        assert_eq!(ov.overflow_label(), "+3 more");
    }

    #[test]
    fn overflow_none() {
        let ov = TabBarOverflow::new(10, 10);
        assert!(!ov.has_overflow());
        assert_eq!(ov.overflow_count(), 0);
        assert_eq!(ov.overflow_label(), "");
    }

    #[test]
    fn overflow_items_and_clear() {
        let mut ov = TabBarOverflow::new(2, 5);
        ov.add_overflow_item("tab3");
        ov.add_overflow_item("tab4");
        assert_eq!(ov.overflow_items.len(), 2);
        ov.clear();
        assert!(ov.overflow_items.is_empty());
    }

    // -----------------------------------------------------------------------
    // TabBarPreview tests
    // -----------------------------------------------------------------------

    #[test]
    fn preview_set_and_promote() {
        let mut p = TabBarPreview::new();
        assert!(!p.has_preview());

        p.set_preview("tmp");
        assert!(p.has_preview());
        assert!(p.is_preview("tmp"));
        assert!(!p.is_preview("other"));

        assert!(p.promote_to_permanent("tmp"));
        assert!(!p.has_preview());
        // Promoting again returns false
        assert!(!p.promote_to_permanent("tmp"));
    }

    #[test]
    fn preview_clear() {
        let mut p = TabBarPreview::new();
        p.set_preview("x");
        p.clear_preview();
        assert!(!p.has_preview());
    }

    // -----------------------------------------------------------------------
    // TabCloseConfirmation tests
    // -----------------------------------------------------------------------

    #[test]
    fn close_confirm_flow() {
        let mut cc = TabCloseConfirmation::new();
        cc.request_close("t1", CloseReason::Dirty);
        cc.request_close("t2", CloseReason::Pinned);
        assert_eq!(cc.pending_count(), 2);

        assert!(cc.confirm("t1"));
        assert_eq!(cc.pending_count(), 1);

        assert!(cc.reject("t2"));
        assert_eq!(cc.pending_count(), 0);
    }

    #[test]
    fn close_needs_confirmation() {
        let clean = make_tab("clean");
        assert!(!TabCloseConfirmation::needs_confirmation(&clean));

        let mut dirty = make_tab("dirty");
        dirty.dirty = true;
        assert!(TabCloseConfirmation::needs_confirmation(&dirty));

        let mut pinned = make_tab("pinned");
        pinned.pinned = true;
        assert!(TabCloseConfirmation::needs_confirmation(&pinned));
    }

    #[test]
    fn close_reason_display() {
        assert_eq!(CloseReason::Dirty.to_string(), "unsaved changes");
        assert_eq!(CloseReason::Pinned.to_string(), "tab is pinned");
        assert_eq!(CloseReason::LastTab.to_string(), "last remaining tab");
    }

    #[test]
    fn tabGroupDragHandler_new() {
        let s = TabGroupDragHandler::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn tabGroupDragHandler_add_contains() {
        let mut s = TabGroupDragHandler::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn tabGroupDragHandler_add_duplicate() {
        let mut s = TabGroupDragHandler::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn tabGroupDragHandler_remove() {
        let mut s = TabGroupDragHandler::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn tabGroupDragHandler_capacity() {
        let s = TabGroupDragHandler::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn tabGroupDragHandler_search() {
        let mut s = TabGroupDragHandler::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn tabGroupDragHandler_stats() {
        let mut s = TabGroupDragHandler::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn tabContextMenuBuilder_new() {
        let m = TabContextMenuBuilder::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn tabContextMenuBuilder_add_find() {
        let mut m = TabContextMenuBuilder::new();
        m.add(TabContextMenuBuilderItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn tabContextMenuBuilder_priority_filter() {
        let mut m = TabContextMenuBuilder::new();
        m.add(TabContextMenuBuilderItem::new("a", "A").with_priority(TabContextMenuBuilderPriority::High));
        m.add(TabContextMenuBuilderItem::new("b", "B").with_priority(TabContextMenuBuilderPriority::Low));
        m.add(TabContextMenuBuilderItem::new("c", "C").with_priority(TabContextMenuBuilderPriority::High));
        assert_eq!(m.by_priority(TabContextMenuBuilderPriority::High).len(), 2);
    }

    #[test]
    fn tabContextMenuBuilder_remove() {
        let mut m = TabContextMenuBuilder::new();
        m.add(TabContextMenuBuilderItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn tabContextMenuBuilder_search() {
        let mut m = TabContextMenuBuilder::new();
        m.add(TabContextMenuBuilderItem::new("id1", "Hello World"));
        m.add(TabContextMenuBuilderItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn tabContextMenuBuilder_total_weight() {
        let mut m = TabContextMenuBuilder::new();
        m.add(TabContextMenuBuilderItem::new("a", "A").with_priority(TabContextMenuBuilderPriority::Critical));
        m.add(TabContextMenuBuilderItem::new("b", "B").with_priority(TabContextMenuBuilderPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn tabContextMenuBuilder_capacity_limit() {
        let mut m = TabContextMenuBuilder::new().with_max_items(2);
        m.add(TabContextMenuBuilderItem::new("1", "one"));
        m.add(TabContextMenuBuilderItem::new("2", "two"));
        assert!(!m.add(TabContextMenuBuilderItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn tabContextMenuBuilder_sorted_by_priority() {
        let mut m = TabContextMenuBuilder::new();
        m.add(TabContextMenuBuilderItem::new("lo", "Low").with_priority(TabContextMenuBuilderPriority::Low));
        m.add(TabContextMenuBuilderItem::new("hi", "High").with_priority(TabContextMenuBuilderPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn tabContextMenuBuilder_item_metadata() {
        let mut item = TabContextMenuBuilderItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn tabGroupDragHandler_enabled_toggle() {
        let mut s = TabGroupDragHandler::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn tabContextMenuBuilder_priority_display() {
        assert_eq!(format!("{}", TabContextMenuBuilderPriority::High), "high");
        assert_eq!(format!("{}", TabContextMenuBuilderPriority::Low), "low");
    }


    #[test]
    fn tabbar_ringbuf_push_get() {
        let mut rb = TabBarRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn tabbar_ringbuf_overflow() {
        let mut rb = TabBarRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn tabbar_ringbuf_clear() {
        let mut rb = TabBarRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn tabbar_ringbuf_newest_oldest() {
        let mut rb = TabBarRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn tabbar_ringbuf_to_vec() {
        let mut rb = TabBarRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn tabbar_ringbuf_is_full() {
        let mut rb = TabBarRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn tabbar_builder_valid() {
        let cfg = TabBarBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn tabbar_builder_empty_name() {
        let r = TabBarBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn tabbar_builder_bad_priority() {
        assert!(TabBarBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn tabbar_builder_zero_max() {
        assert!(TabBarBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn tabbar_cfg_merge() {
        let mut a = TabBarBuilder::new("a").property("x", "1").build().unwrap();
        let b = TabBarBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn tabbar_cfg_display() {
        let cfg = TabBarBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }


    #[test]
    fn zw_metrics_empty() {
        let m = ZwMetrics::new("tabbar");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zw_metrics_record_and_mean() {
        let mut m = ZwMetrics::new("tabbar");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zw_metrics_min_max() {
        let mut m = ZwMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zw_metrics_variance_and_std() {
        let mut m = ZwMetrics::new("v");
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
    fn zw_metrics_percentile() {
        let mut m = ZwMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn zw_metrics_merge() {
        let mut a = ZwMetrics::new("a");
        a.record(1.0);
        let mut b = ZwMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn zw_metrics_reset() {
        let mut m = ZwMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn zw_rate_window_empty() {
        let rw = ZwRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn zw_rate_window_tick_and_rate() {
        let mut rw = ZwRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn zw_lru_cache_basic() {
        let mut c = ZwLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn zw_lru_cache_contains_and_keys() {
        let mut c = ZwLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn zw_lru_cache_remove() {
        let mut c = ZwLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn zw_metrics_sum() {
        let mut m = ZwMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zw_metrics_label() {
        let m = ZwMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn zw_lru_cache_clear() {
        let mut c = ZwLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for tabbar
    #[test]
    fn xa_tabbar_ring_new() {
        let rb = super::XaTabbarRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_tabbar_ring_push_len() {
        let mut rb = super::XaTabbarRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_tabbar_ring_wrap() {
        let mut rb = super::XaTabbarRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_tabbar_ring_mean_empty() {
        let rb = super::XaTabbarRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_tabbar_ring_mean_values() {
        let mut rb = super::XaTabbarRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_tabbar_ring_min_max() {
        let mut rb = super::XaTabbarRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_tabbar_ring_iter() {
        let mut rb = super::XaTabbarRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_tabbar_counter_new() {
        let c = super::XaTabbarCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_tabbar_counter_inc() {
        let mut c = super::XaTabbarCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_tabbar_counter_inc_by() {
        let mut c = super::XaTabbarCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_tabbar_counter_reset() {
        let mut c = super::XaTabbarCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_tabbar_counter_clear() {
        let mut c = super::XaTabbarCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_tabbar_counter_default() {
        let c = super::XaTabbarCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 172 ----

    #[test]
    fn xc_172_pool_new_empty() {
        let pool: super::Xc172Pool<i32> = super::Xc172Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_172_pool_release_acquire() {
        let mut pool = super::Xc172Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_172_pool_acquire_empty() {
        let mut pool: super::Xc172Pool<i32> = super::Xc172Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_172_pool_full() {
        let mut pool = super::Xc172Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_172_pool_drain() {
        let mut pool = super::Xc172Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_172_pool_stats() {
        let mut pool = super::Xc172Pool::new(8);
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
    fn xc_172_pool_clear() {
        let mut pool = super::Xc172Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_172_pool_shrink() {
        let mut pool = super::Xc172Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_172_pool_default() {
        let pool: super::Xc172Pool<String> = super::Xc172Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_172_pool_extend() {
        let mut pool = super::Xc172Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_172_pool_retain() {
        let mut pool = super::Xc172Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_172_scheduler_round_robin() {
        let mut sched = super::Xc172Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_172_scheduler_empty() {
        let mut sched = super::Xc172Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_172_scheduler_reset() {
        let mut sched = super::Xc172Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_172_scheduler_add_remove() {
        let mut sched = super::Xc172Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_172_scheduler_targets() {
        let sched = super::Xc172Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_172_hash_empty() {
        assert_eq!(super::xc_172_hash(b""), 5381);
    }

    #[test]
    fn xc_172_hash_data() {
        let h = super::xc_172_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_172_hash(b"hello"), h);
    }

    #[test]
    fn xc_172_reverse_str() {
        assert_eq!(super::xc_172_reverse("abc"), "cba");
        assert_eq!(super::xc_172_reverse(""), "");
    }


    // --- xd_41 deepening tests ---

    #[test]
    fn xd_41_sm_initial_state() {
        let sm = Xd41StateMachine::new();
        assert_eq!(sm.current_state(), Xd41State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_41_sm_valid_idle_to_running() {
        let mut sm = Xd41StateMachine::new();
        assert!(sm.transition(Xd41State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd41State::Running);
    }

    #[test]
    fn xd_41_sm_valid_running_to_paused() {
        let mut sm = Xd41StateMachine::new();
        sm.transition(Xd41State::Running).unwrap();
        assert!(sm.transition(Xd41State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd41State::Paused);
    }

    #[test]
    fn xd_41_sm_valid_running_to_done() {
        let mut sm = Xd41StateMachine::new();
        sm.transition(Xd41State::Running).unwrap();
        assert!(sm.transition(Xd41State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd41State::Done);
    }

    #[test]
    fn xd_41_sm_valid_paused_to_running() {
        let mut sm = Xd41StateMachine::new();
        sm.transition(Xd41State::Running).unwrap();
        sm.transition(Xd41State::Paused).unwrap();
        assert!(sm.transition(Xd41State::Running).is_ok());
    }

    #[test]
    fn xd_41_sm_valid_done_to_idle() {
        let mut sm = Xd41StateMachine::new();
        sm.transition(Xd41State::Running).unwrap();
        sm.transition(Xd41State::Done).unwrap();
        assert!(sm.transition(Xd41State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd41State::Idle);
    }

    #[test]
    fn xd_41_sm_invalid_idle_to_done() {
        let mut sm = Xd41StateMachine::new();
        assert!(sm.transition(Xd41State::Done).is_err());
    }

    #[test]
    fn xd_41_sm_invalid_idle_to_paused() {
        let mut sm = Xd41StateMachine::new();
        assert!(sm.transition(Xd41State::Paused).is_err());
    }

    #[test]
    fn xd_41_sm_history_tracking() {
        let mut sm = Xd41StateMachine::new();
        sm.transition(Xd41State::Running).unwrap();
        sm.transition(Xd41State::Paused).unwrap();
        sm.transition(Xd41State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd41State::Idle);
        assert_eq!(sm.history()[0].to, Xd41State::Running);
        assert_eq!(sm.history()[1].from, Xd41State::Running);
        assert_eq!(sm.history()[2].to, Xd41State::Done);
    }

    #[test]
    fn xd_41_sm_serialize_deserialize() {
        let mut sm = Xd41StateMachine::new();
        sm.transition(Xd41State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd41StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd41State::Running));
    }

    #[test]
    fn xd_41_sm_deserialize_invalid() {
        assert_eq!(Xd41StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_41_sm_reset() {
        let mut sm = Xd41StateMachine::new();
        sm.transition(Xd41State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd41State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_41_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd41EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd41Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_41_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd41EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd41Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd41Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_41_bus_unsubscribe() {
        let mut bus = Xd41EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_41_event_kind_and_payload() {
        let e = Xd41Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd41Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_41_bus_clear_history() {
        let mut bus = Xd41EventBus::new();
        bus.publish(Xd41Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_41_sm_step_counter_increments() {
        let mut sm = Xd41StateMachine::new();
        sm.transition(Xd41State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd41State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #39 --

    #[test]
    fn xf39_trie_insert_search() {
        let mut t = Xf39Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf39_trie_starts_with() {
        let mut t = Xf39Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf39_trie_remove() {
        let mut t = Xf39Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf39_trie_word_count() {
        let mut t = Xf39Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf39_trie_longest_prefix() {
        let mut t = Xf39Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf39_trie_all_words() {
        let mut t = Xf39Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf39_trie_autocomplete() {
        let mut t = Xf39Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf39_trie_empty_search() {
        let t = Xf39Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf39_bloom_add_contains() {
        let mut bf = Xf39BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf39_bloom_probably_absent() {
        let bf = Xf39BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf39_bloom_false_positive_rate() {
        let mut bf = Xf39BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf39_bloom_clear() {
        let mut bf = Xf39BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf39_bloom_union() {
        let mut a = Xf39BloomFilter::xf_new(512, 2);
        let mut b = Xf39BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf39_bloom_intersection_estimate() {
        let mut a = Xf39BloomFilter::xf_new(512, 2);
        let mut b = Xf39BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf39_bloom_union_size_mismatch() {
        let a = Xf39BloomFilter::xf_new(256, 2);
        let b = Xf39BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh171_skip_insert_contains() {
        let mut sl = super::Xh171SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh171_skip_remove() {
        let mut sl = super::Xh171SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh171_skip_len() {
        let mut sl = super::Xh171SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh171_skip_range_query() {
        let mut sl = super::Xh171SkipList::xh_new(4);
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
    fn xh171_skip_floor_ceiling() {
        let mut sl = super::Xh171SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh171_skip_rank() {
        let mut sl = super::Xh171SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh171_skip_empty() {
        let sl = super::Xh171SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh171_skip_duplicates() {
        let mut sl = super::Xh171SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh171_bitset_set_test() {
        let mut bs = super::Xh171BitSet::xh_new(256);
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
    fn xh171_bitset_clear_count() {
        let mut bs = super::Xh171BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh171_bitset_and_or_xor() {
        let mut a = super::Xh171BitSet::xh_new(128);
        let mut b = super::Xh171BitSet::xh_new(128);
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
    fn xh171_bitset_iter_ones() {
        let mut bs = super::Xh171BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh171_bitset_first_last() {
        let mut bs = super::Xh171BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh171_bitset_empty() {
        let bs = super::Xh171BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}