//! Base widget system for vsedit TUI.
use std::fmt;
use vsedit_tui::{Frame, Rect};

/// Trait that all vsedit widgets implement.
pub trait Widget {
    fn render(&self, frame: &mut Frame, area: Rect);
}

/// A widget that can receive focus.
pub trait Focusable: Widget {
    fn is_focused(&self) -> bool;
    fn set_focused(&mut self, focused: bool);
}

/// Focus direction for navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    Next,
    Previous,
    Up,
    Down,
    Left,
    Right,
}

/// Focus manager that tracks which widget has focus.
pub struct FocusManager {
    focused_id: Option<String>,
    focus_order: Vec<String>,
}

impl FocusManager {
    pub fn new() -> Self {
        Self {
            focused_id: None,
            focus_order: Vec::new(),
        }
    }

    pub fn register(&mut self, id: impl Into<String>) {
        let id = id.into();
        if !self.focus_order.contains(&id) {
            self.focus_order.push(id);
        }
    }

    pub fn set_focus(&mut self, id: &str) {
        if self.focus_order.iter().any(|s| s == id) {
            self.focused_id = Some(id.to_string());
        }
    }

    pub fn focused(&self) -> Option<&str> {
        self.focused_id.as_deref()
    }

    pub fn move_focus(&mut self, direction: FocusDirection) {
        match direction {
            FocusDirection::Next | FocusDirection::Down | FocusDirection::Right => {
                self.focus_next();
            }
            FocusDirection::Previous | FocusDirection::Up | FocusDirection::Left => {
                self.focus_previous();
            }
        }
    }

    pub fn focus_next(&mut self) {
        if self.focus_order.is_empty() {
            return;
        }
        let next = match &self.focused_id {
            Some(current) => {
                let idx = self
                    .focus_order
                    .iter()
                    .position(|s| s == current)
                    .unwrap_or(0);
                (idx + 1) % self.focus_order.len()
            }
            None => 0,
        };
        self.focused_id = Some(self.focus_order[next].clone());
    }

    pub fn focus_previous(&mut self) {
        if self.focus_order.is_empty() {
            return;
        }
        let prev = match &self.focused_id {
            Some(current) => {
                let idx = self
                    .focus_order
                    .iter()
                    .position(|s| s == current)
                    .unwrap_or(0);
                if idx == 0 {
                    self.focus_order.len() - 1
                } else {
                    idx - 1
                }
            }
            None => self.focus_order.len() - 1,
        };
        self.focused_id = Some(self.focus_order[prev].clone());
    }
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for FocusManager {
    fn clone(&self) -> Self {
        Self {
            focused_id: self.focused_id.clone(),
            focus_order: self.focus_order.clone(),
        }
    }
}

impl PartialEq for FocusManager {
    fn eq(&self, other: &Self) -> bool {
        self.focused_id == other.focused_id && self.focus_order == other.focus_order
    }
}

impl std::fmt::Debug for FocusManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FocusManager")
            .field("focused_id", &self.focused_id)
            .field("focus_order", &self.focus_order)
            .finish()
    }
}

impl std::fmt::Display for FocusManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.focused_id {
            Some(id) => write!(f, "FocusManager(focused={}, count={})", id, self.focus_order.len()),
            None => write!(f, "FocusManager(unfocused, count={})", self.focus_order.len()),
        }
    }
}

impl FocusManager {
    /// Returns the number of registered focusable widgets.
    pub fn count(&self) -> usize {
        self.focus_order.len()
    }

    /// Returns true if the given id is currently focused.
    pub fn is_focused(&self, id: &str) -> bool {
        self.focused_id.as_deref() == Some(id)
    }

    /// Unregisters a widget id, adjusting focus if needed.
    pub fn unregister(&mut self, id: &str) {
        if let Some(pos) = self.focus_order.iter().position(|s| s == id) {
            self.focus_order.remove(pos);
            if self.focused_id.as_deref() == Some(id) {
                self.focused_id = if self.focus_order.is_empty() {
                    None
                } else {
                    let new_idx = pos.min(self.focus_order.len() - 1);
                    Some(self.focus_order[new_idx].clone())
                };
            }
        }
    }

    /// Clears all registered widgets and focus state.
    pub fn clear(&mut self) {
        self.focus_order.clear();
        self.focused_id = None;
    }

    /// Returns the index of the currently focused widget, if any.
    pub fn focused_index(&self) -> Option<usize> {
        let id = self.focused_id.as_deref()?;
        self.focus_order.iter().position(|s| s == id)
    }

    /// Returns an ordered slice of all registered widget ids.
    pub fn order(&self) -> &[String] {
        &self.focus_order
    }
}

/// Errors that can occur during widget operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WidgetError {
    /// The widget id was not found.
    NotFound(String),
    /// A required field was missing during widget construction.
    MissingField(&'static str),
    /// A validation constraint was violated.
    InvalidValue { field: &'static str, reason: String },
}

impl std::fmt::Display for WidgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WidgetError::NotFound(id) => write!(f, "widget '{}' not found", id),
            WidgetError::MissingField(name) => write!(f, "missing required field '{}'", name),
            WidgetError::InvalidValue { field, reason } => {
                write!(f, "invalid value for '{}': {}", field, reason)
            }
        }
    }
}

impl std::error::Error for WidgetError {}

/// Builder for creating [`Panel`] widgets with validation.
pub struct PanelBuilder {
    title: Option<String>,
    border_style: Option<vsedit_styles::Style>,
    min_width: Option<u16>,
    min_height: Option<u16>,
}

impl PanelBuilder {
    pub fn new() -> Self {
        Self {
            title: None,
            border_style: None,
            min_width: None,
            min_height: None,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn border_style(mut self, style: vsedit_styles::Style) -> Self {
        self.border_style = Some(style);
        self
    }

    pub fn min_width(mut self, w: u16) -> Self {
        self.min_width = Some(w);
        self
    }

    pub fn min_height(mut self, h: u16) -> Self {
        self.min_height = Some(h);
        self
    }

    /// Builds the panel, returning an error if the title is missing or empty.
    pub fn build(self) -> Result<Panel, WidgetError> {
        let title = self.title.ok_or(WidgetError::MissingField("title"))?;
        if title.is_empty() {
            return Err(WidgetError::InvalidValue {
                field: "title",
                reason: "title must not be empty".into(),
            });
        }
        Ok(Panel {
            title,
            border_style: self.border_style.unwrap_or_else(|| {
                vsedit_styles::Style::default().fg(vsedit_styles::ThemeDefaults::border())
            }),
            min_width: self.min_width.unwrap_or(0),
            min_height: self.min_height.unwrap_or(0),
        })
    }
}

impl Default for PanelBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A bordered panel widget.
#[derive(Clone, PartialEq)]
pub struct Panel {
    pub title: String,
    pub border_style: vsedit_styles::Style,
    pub min_width: u16,
    pub min_height: u16,
}

impl std::fmt::Debug for Panel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Panel")
            .field("title", &self.title)
            .field("min_width", &self.min_width)
            .field("min_height", &self.min_height)
            .finish()
    }
}

impl std::fmt::Display for Panel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Panel(\"{}\")", self.title)
    }
}

impl Panel {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            border_style: vsedit_styles::Style::default()
                .fg(vsedit_styles::ThemeDefaults::border()),
            min_width: 0,
            min_height: 0,
        }
    }

    pub fn builder() -> PanelBuilder {
        PanelBuilder::new()
    }

    pub fn with_style(mut self, style: vsedit_styles::Style) -> Self {
        self.border_style = style;
        self
    }

    /// Returns true if the given area satisfies the panel's minimum dimensions.
    pub fn fits(&self, area: Rect) -> bool {
        area.width >= self.min_width && area.height >= self.min_height
    }

    /// Compute the inner content area after accounting for borders (1-cell on each side).
    pub fn inner_area(&self, area: Rect) -> Option<Rect> {
        if area.width < 2 || area.height < 2 {
            return None;
        }
        Some(Rect::new(
            area.x + 1,
            area.y + 1,
            area.width - 2,
            area.height - 2,
        ))
    }
}

impl Widget for Panel {
    fn render(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Block, Borders};

        let block = Block::default()
            .title(self.title.as_str())
            .borders(Borders::ALL)
            .border_style(self.border_style);
        frame.render_widget(block, area);
    }
}

/// Accumulated statistics for widgets operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetsStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WidgetsStats {
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
    pub fn merge(&mut self, other: &WidgetsStats) {
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

impl Default for WidgetsStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WidgetsStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WidgetsStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for widgets.
#[derive(Debug, Clone)]
pub struct WidgetsValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WidgetsValidator {
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

impl Default for WidgetsValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// A circular chain of focusable widget IDs with explicit group tracking.
#[derive(Debug, Clone, PartialEq)]
pub struct FocusChain {
    groups: Vec<FocusGroup>,
}

/// A named group within a focus chain.
#[derive(Debug, Clone, PartialEq)]
pub struct FocusGroup {
    pub name: String,
    pub widget_ids: Vec<String>,
}

impl FocusGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), widget_ids: Vec::new() }
    }

    pub fn add(&mut self, id: impl Into<String>) {
        let id = id.into();
        if !self.widget_ids.contains(&id) {
            self.widget_ids.push(id);
        }
    }

    pub fn len(&self) -> usize { self.widget_ids.len() }
    pub fn is_empty(&self) -> bool { self.widget_ids.is_empty() }
}

impl FocusChain {
    pub fn new() -> Self { Self { groups: Vec::new() } }

    pub fn add_group(&mut self, group: FocusGroup) {
        self.groups.push(group);
    }

    /// Returns all widget IDs in focus order (group by group).
    pub fn all_ids(&self) -> Vec<&str> {
        self.groups.iter()
            .flat_map(|g| g.widget_ids.iter().map(|s| s.as_str()))
            .collect()
    }

    /// Find the next focusable ID after `current` in the chain, wrapping around.
    pub fn next_after(&self, current: &str) -> Option<&str> {
        let ids = self.all_ids();
        if ids.is_empty() { return None; }
        let pos = ids.iter().position(|id| *id == current)?;
        let next = (pos + 1) % ids.len();
        Some(ids[next])
    }

    /// Find the previous focusable ID before `current` in the chain.
    pub fn prev_before(&self, current: &str) -> Option<&str> {
        let ids = self.all_ids();
        if ids.is_empty() { return None; }
        let pos = ids.iter().position(|id| *id == current)?;
        let prev = if pos == 0 { ids.len() - 1 } else { pos - 1 };
        Some(ids[prev])
    }

    /// Total number of focusable widgets across all groups.
    pub fn total_count(&self) -> usize {
        self.groups.iter().map(|g| g.widget_ids.len()).sum()
    }

    /// Find which group a widget belongs to.
    pub fn group_of(&self, id: &str) -> Option<&str> {
        self.groups.iter()
            .find(|g| g.widget_ids.iter().any(|w| w == id))
            .map(|g| g.name.as_str())
    }
}

impl Default for FocusChain {
    fn default() -> Self { Self::new() }
}

/// Measurement constraints for a widget's layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WidgetMeasure {
    pub preferred_width: u16,
    pub preferred_height: u16,
    pub min_width: u16,
    pub min_height: u16,
    pub max_width: u16,
    pub max_height: u16,
}

impl WidgetMeasure {
    pub fn new(preferred_width: u16, preferred_height: u16) -> Self {
        Self {
            preferred_width,
            preferred_height,
            min_width: 0,
            min_height: 0,
            max_width: u16::MAX,
            max_height: u16::MAX,
        }
    }

    pub fn with_min(mut self, width: u16, height: u16) -> Self {
        self.min_width = width;
        self.min_height = height;
        self
    }

    pub fn with_max(mut self, width: u16, height: u16) -> Self {
        self.max_width = width;
        self.max_height = height;
        self
    }

    /// Constrain an area to respect this measure's min/max bounds.
    pub fn constrain(&self, area: Rect) -> Rect {
        let w = area.width.max(self.min_width).min(self.max_width);
        let h = area.height.max(self.min_height).min(self.max_height);
        Rect::new(area.x, area.y, w, h)
    }

    /// Check if an area satisfies the minimum size requirements.
    pub fn satisfies_min(&self, area: Rect) -> bool {
        area.width >= self.min_width && area.height >= self.min_height
    }

    /// Compute the preferred area starting at (0,0).
    pub fn preferred_rect(&self) -> Rect {
        Rect::new(0, 0, self.preferred_width, self.preferred_height)
    }
}

impl fmt::Display for WidgetMeasure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WidgetMeasure({}x{}, min={}x{}, max={}x{})",
            self.preferred_width, self.preferred_height,
            self.min_width, self.min_height,
            self.max_width, self.max_height)
    }
}

/// Visibility state for a widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Visible,
    Hidden,
    Collapsed,
}

impl Visibility {
    pub fn is_visible(&self) -> bool { *self == Visibility::Visible }
    pub fn takes_space(&self) -> bool { *self != Visibility::Collapsed }
}

impl fmt::Display for Visibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Visibility::Visible => write!(f, "Visible"),
            Visibility::Hidden => write!(f, "Hidden"),
            Visibility::Collapsed => write!(f, "Collapsed"),
        }
    }
}

/// Toggle visibility of a widget, recalculating layout areas.
/// Returns the new visibility state and the adjusted area (zero-sized if collapsed).
pub fn widget_visibility_toggle(current: Visibility, area: Rect) -> (Visibility, Rect) {
    match current {
        Visibility::Visible => (Visibility::Hidden, area),
        Visibility::Hidden => (Visibility::Collapsed, Rect::new(area.x, area.y, 0, 0)),
        Visibility::Collapsed => (Visibility::Visible, area),
    }
}

/// Compute layout for a list of widgets with visibility, distributing available width equally among visible widgets.
pub fn layout_visible_widgets(
    widgets: &[(String, Visibility, WidgetMeasure)],
    available: Rect,
) -> Vec<(String, Rect)> {
    let visible: Vec<&(String, Visibility, WidgetMeasure)> = widgets.iter()
        .filter(|(_, vis, _)| vis.takes_space())
        .collect();
    if visible.is_empty() { return Vec::new(); }
    let per_width = available.width / visible.len() as u16;
    let mut x = available.x;
    visible.iter().map(|(name, _, measure)| {
        let w = per_width.max(measure.min_width).min(measure.max_width);
        let h = available.height.max(measure.min_height).min(measure.max_height);
        let rect = Rect::new(x, available.y, w, h);
        x += w;
        (name.clone(), rect)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_manager_empty() {
        let fm = FocusManager::new();
        assert!(fm.focused().is_none());
    }

    #[test]
    fn focus_manager_register_and_set() {
        let mut fm = FocusManager::new();
        fm.register("editor");
        fm.register("sidebar");
        fm.set_focus("editor");
        assert_eq!(fm.focused(), Some("editor"));
    }

    #[test]
    fn focus_manager_cycle_next() {
        let mut fm = FocusManager::new();
        fm.register("a");
        fm.register("b");
        fm.register("c");
        fm.focus_next();
        assert_eq!(fm.focused(), Some("a"));
        fm.focus_next();
        assert_eq!(fm.focused(), Some("b"));
        fm.focus_next();
        assert_eq!(fm.focused(), Some("c"));
        fm.focus_next();
        assert_eq!(fm.focused(), Some("a"));
    }

    #[test]
    fn focus_manager_cycle_previous() {
        let mut fm = FocusManager::new();
        fm.register("a");
        fm.register("b");
        fm.register("c");
        fm.focus_previous();
        assert_eq!(fm.focused(), Some("c"));
        fm.focus_previous();
        assert_eq!(fm.focused(), Some("b"));
        fm.focus_previous();
        assert_eq!(fm.focused(), Some("a"));
        fm.focus_previous();
        assert_eq!(fm.focused(), Some("c"));
    }

    #[test]
    fn focus_manager_move_direction() {
        let mut fm = FocusManager::new();
        fm.register("x");
        fm.register("y");
        fm.move_focus(FocusDirection::Next);
        assert_eq!(fm.focused(), Some("x"));
        fm.move_focus(FocusDirection::Next);
        assert_eq!(fm.focused(), Some("y"));
        fm.move_focus(FocusDirection::Previous);
        assert_eq!(fm.focused(), Some("x"));
    }

    #[test]
    fn focus_manager_no_duplicate_register() {
        let mut fm = FocusManager::new();
        fm.register("a");
        fm.register("a");
        assert_eq!(fm.focus_order.len(), 1);
    }

    #[test]
    fn panel_construction() {
        let panel = Panel::new("Test Panel");
        assert_eq!(panel.title, "Test Panel");
    }

    #[test]
    fn panel_with_style() {
        let style = vsedit_styles::Style::default().fg(vsedit_styles::Color::Red);
        let panel = Panel::new("Styled").with_style(style);
        assert_eq!(panel.border_style, style);
    }

    #[test]
    fn focus_manager_unregister_focused() {
        let mut fm = FocusManager::new();
        fm.register("a");
        fm.register("b");
        fm.register("c");
        fm.set_focus("b");
        fm.unregister("b");
        // Focus should shift to next available widget at that index
        assert_eq!(fm.focused(), Some("c"));
        assert_eq!(fm.count(), 2);
    }

    #[test]
    fn focus_manager_unregister_last() {
        let mut fm = FocusManager::new();
        fm.register("only");
        fm.set_focus("only");
        fm.unregister("only");
        assert!(fm.focused().is_none());
        assert_eq!(fm.count(), 0);
    }

    #[test]
    fn focus_manager_clear() {
        let mut fm = FocusManager::new();
        fm.register("a");
        fm.register("b");
        fm.set_focus("a");
        fm.clear();
        assert!(fm.focused().is_none());
        assert_eq!(fm.count(), 0);
    }

    #[test]
    fn focus_manager_is_focused() {
        let mut fm = FocusManager::new();
        fm.register("editor");
        fm.set_focus("editor");
        assert!(fm.is_focused("editor"));
        assert!(!fm.is_focused("sidebar"));
    }

    #[test]
    fn focus_manager_focused_index() {
        let mut fm = FocusManager::new();
        fm.register("a");
        fm.register("b");
        fm.register("c");
        fm.set_focus("b");
        assert_eq!(fm.focused_index(), Some(1));
    }

    #[test]
    fn focus_manager_display() {
        let mut fm = FocusManager::new();
        fm.register("x");
        assert_eq!(format!("{}", fm), "FocusManager(unfocused, count=1)");
        fm.set_focus("x");
        assert_eq!(format!("{}", fm), "FocusManager(focused=x, count=1)");
    }

    #[test]
    fn focus_manager_clone_eq() {
        let mut fm = FocusManager::new();
        fm.register("a");
        fm.set_focus("a");
        let fm2 = fm.clone();
        assert_eq!(fm, fm2);
    }

    #[test]
    fn panel_builder_success() {
        let panel = PanelBuilder::new()
            .title("My Panel")
            .min_width(10)
            .min_height(5)
            .build()
            .unwrap();
        assert_eq!(panel.title, "My Panel");
        assert_eq!(panel.min_width, 10);
        assert_eq!(panel.min_height, 5);
    }

    #[test]
    fn panel_builder_missing_title() {
        let result = PanelBuilder::new().build();
        assert_eq!(result, Err(WidgetError::MissingField("title")));
    }

    #[test]
    fn panel_builder_empty_title() {
        let result = PanelBuilder::new().title("").build();
        assert!(matches!(result, Err(WidgetError::InvalidValue { .. })));
    }

    #[test]
    fn panel_inner_area() {
        let panel = Panel::new("Test");
        let area = Rect::new(0, 0, 20, 10);
        let inner = panel.inner_area(area).unwrap();
        assert_eq!(inner, Rect::new(1, 1, 18, 8));
    }

    #[test]
    fn panel_inner_area_too_small() {
        let panel = Panel::new("Test");
        let area = Rect::new(0, 0, 1, 1);
        assert!(panel.inner_area(area).is_none());
    }

    #[test]
    fn panel_fits() {
        let panel = PanelBuilder::new()
            .title("Sized")
            .min_width(10)
            .min_height(5)
            .build()
            .unwrap();
        assert!(panel.fits(Rect::new(0, 0, 20, 10)));
        assert!(!panel.fits(Rect::new(0, 0, 5, 10)));
        assert!(!panel.fits(Rect::new(0, 0, 20, 3)));
    }

    #[test]
    fn panel_display_debug() {
        let panel = Panel::new("Hello");
        assert_eq!(format!("{}", panel), "Panel(\"Hello\")");
        let debug = format!("{:?}", panel);
        assert!(debug.contains("Panel"));
        assert!(debug.contains("Hello"));
    }

    #[test]
    fn panel_clone_eq() {
        let p1 = Panel::new("Clone");
        let p2 = p1.clone();
        assert_eq!(p1, p2);
    }

    #[test]
    fn widget_error_display() {
        let e1 = WidgetError::NotFound("foo".into());
        assert_eq!(format!("{}", e1), "widget 'foo' not found");

        let e2 = WidgetError::MissingField("title");
        assert_eq!(format!("{}", e2), "missing required field 'title'");

        let e3 = WidgetError::InvalidValue {
            field: "width",
            reason: "must be positive".into(),
        };
        assert!(format!("{}", e3).contains("must be positive"));
    }

    #[test]
    fn widget_error_is_std_error() {
        let err: Box<dyn std::error::Error> =
            Box::new(WidgetError::NotFound("bar".into()));
        assert!(err.to_string().contains("bar"));
    }

    #[test]
    fn focus_direction_all_variants() {
        let dirs = [
            FocusDirection::Next,
            FocusDirection::Previous,
            FocusDirection::Up,
            FocusDirection::Down,
            FocusDirection::Left,
            FocusDirection::Right,
        ];
        // Ensure all variants are distinct
        for (i, a) in dirs.iter().enumerate() {
            for (j, b) in dirs.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b);
                }
            }
        }
    }

    #[test]
    fn widgets_stats_new_defaults() {
        let stats = WidgetsStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn widgets_stats_record_success() {
        let mut stats = WidgetsStats::new();
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
    fn widgets_stats_record_failure() {
        let mut stats = WidgetsStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn widgets_stats_reset() {
        let mut stats = WidgetsStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn widgets_stats_merge() {
        let mut a = WidgetsStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WidgetsStats::new();
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
    fn widgets_stats_display() {
        let mut stats = WidgetsStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn widgets_stats_default() {
        let stats = WidgetsStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn widgets_validator_accepts_valid_name() {
        let v = WidgetsValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn widgets_validator_rejects_empty() {
        let v = WidgetsValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn widgets_validator_rejects_too_long() {
        let v = WidgetsValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn widgets_validator_forbidden_prefix() {
        let v = WidgetsValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn widgets_validator_allowed_chars() {
        let v = WidgetsValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn widgets_validator_range() {
        let v = WidgetsValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn widgets_sanitize_removes_control() {
        let result = WidgetsValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn widgets_truncate_short_string() {
        assert_eq!(WidgetsValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn widgets_truncate_long_string() {
        let result = WidgetsValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn widgets_is_ascii_printable() {
        assert!(WidgetsValidator::is_ascii_printable("Hello World 123"));
        assert!(!WidgetsValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn focus_chain_navigation() {
        let mut chain = FocusChain::new();
        let mut g = FocusGroup::new("main");
        g.add("editor");
        g.add("sidebar");
        g.add("panel");
        chain.add_group(g);
        assert_eq!(chain.next_after("editor"), Some("sidebar"));
        assert_eq!(chain.next_after("panel"), Some("editor")); // wraps
        assert_eq!(chain.prev_before("editor"), Some("panel")); // wraps back
    }

    #[test]
    fn focus_chain_group_of() {
        let mut chain = FocusChain::new();
        let mut g1 = FocusGroup::new("editors");
        g1.add("e1");
        let mut g2 = FocusGroup::new("panels");
        g2.add("p1");
        chain.add_group(g1);
        chain.add_group(g2);
        assert_eq!(chain.group_of("e1"), Some("editors"));
        assert_eq!(chain.group_of("p1"), Some("panels"));
        assert_eq!(chain.group_of("unknown"), None);
    }

    #[test]
    fn focus_chain_total_count() {
        let mut chain = FocusChain::new();
        let mut g = FocusGroup::new("g");
        g.add("a");
        g.add("b");
        chain.add_group(g);
        assert_eq!(chain.total_count(), 2);
    }

    #[test]
    fn widget_measure_constrain() {
        let m = WidgetMeasure::new(50, 20).with_min(10, 5).with_max(100, 40);
        let small = Rect::new(0, 0, 5, 3);
        let constrained = m.constrain(small);
        assert_eq!(constrained.width, 10);
        assert_eq!(constrained.height, 5);
    }

    #[test]
    fn widget_measure_satisfies_min() {
        let m = WidgetMeasure::new(50, 20).with_min(10, 5);
        assert!(m.satisfies_min(Rect::new(0, 0, 20, 10)));
        assert!(!m.satisfies_min(Rect::new(0, 0, 5, 10)));
    }

    #[test]
    fn widget_measure_preferred_rect() {
        let m = WidgetMeasure::new(80, 24);
        let r = m.preferred_rect();
        assert_eq!(r.width, 80);
        assert_eq!(r.height, 24);
    }

    #[test]
    fn visibility_toggle_cycle() {
        let area = Rect::new(0, 0, 100, 50);
        let (v1, a1) = widget_visibility_toggle(Visibility::Visible, area);
        assert_eq!(v1, Visibility::Hidden);
        assert_eq!(a1, area);
        let (v2, a2) = widget_visibility_toggle(v1, a1);
        assert_eq!(v2, Visibility::Collapsed);
        assert_eq!(a2.width, 0);
        let (v3, _) = widget_visibility_toggle(v2, area);
        assert_eq!(v3, Visibility::Visible);
    }

    #[test]
    fn visibility_properties() {
        assert!(Visibility::Visible.is_visible());
        assert!(!Visibility::Hidden.is_visible());
        assert!(Visibility::Hidden.takes_space());
        assert!(!Visibility::Collapsed.takes_space());
    }

    #[test]
    fn layout_visible_widgets_basic() {
        let widgets = vec![
            ("a".into(), Visibility::Visible, WidgetMeasure::new(50, 20)),
            ("b".into(), Visibility::Collapsed, WidgetMeasure::new(50, 20)),
            ("c".into(), Visibility::Visible, WidgetMeasure::new(50, 20)),
        ];
        let result = layout_visible_widgets(&widgets, Rect::new(0, 0, 100, 30));
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "a");
        assert_eq!(result[1].0, "c");
    }

    #[test]
    fn widget_measure_display() {
        let m = WidgetMeasure::new(80, 24).with_min(10, 5);
        let s = format!("{}", m);
        assert!(s.contains("80x24"));
        assert!(s.contains("min=10x5"));
    }

    #[test]
    fn focus_group_no_duplicates() {
        let mut g = FocusGroup::new("test");
        g.add("x");
        g.add("x");
        assert_eq!(g.len(), 1);
    }
}
