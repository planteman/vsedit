//! Editor tab bar widget.

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
}
