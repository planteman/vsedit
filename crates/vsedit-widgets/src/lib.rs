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

// ---------------------------------------------------------------------------
// WidgetTree – parent/child widget relationships
// ---------------------------------------------------------------------------

/// Node in a widget tree, tracking parent/child and z-order.
#[derive(Debug, Clone)]
pub struct WidgetNode {
    pub id: String,
    pub parent: Option<String>,
    pub children: Vec<String>,
    pub z_order: i32,
    pub bounds: (u16, u16, u16, u16), // x, y, w, h
    pub visible: bool,
}

impl WidgetNode {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            parent: None,
            children: Vec::new(),
            z_order: 0,
            bounds: (0, 0, 0, 0),
            visible: true,
        }
    }

    pub fn with_bounds(mut self, x: u16, y: u16, w: u16, h: u16) -> Self {
        self.bounds = (x, y, w, h);
        self
    }

    pub fn with_z_order(mut self, z: i32) -> Self {
        self.z_order = z;
        self
    }

    pub fn rect(&self) -> Rect {
        let (x, y, w, h) = self.bounds;
        Rect::new(x, y, w, h)
    }

    /// Point-in-rect hit test.
    pub fn contains_point(&self, px: u16, py: u16) -> bool {
        let (x, y, w, h) = self.bounds;
        px >= x && px < x.saturating_add(w) && py >= y && py < y.saturating_add(h)
    }
}

impl fmt::Display for WidgetNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WidgetNode({}, z={}, children={})",
            self.id,
            self.z_order,
            self.children.len()
        )
    }
}

/// Tree of widgets supporting parent-child relationships, z-ordering, hit testing.
#[derive(Debug, Clone)]
pub struct WidgetTree {
    nodes: std::collections::HashMap<String, WidgetNode>,
    root_ids: Vec<String>,
}

impl WidgetTree {
    pub fn new() -> Self {
        Self {
            nodes: std::collections::HashMap::new(),
            root_ids: Vec::new(),
        }
    }

    /// Add a root widget (no parent).
    pub fn add_root(&mut self, node: WidgetNode) {
        let id = node.id.clone();
        self.nodes.insert(id.clone(), node);
        if !self.root_ids.contains(&id) {
            self.root_ids.push(id);
        }
    }

    /// Add a child widget under a parent.
    pub fn add_child(&mut self, parent_id: &str, mut node: WidgetNode) {
        node.parent = Some(parent_id.to_string());
        let child_id = node.id.clone();
        self.nodes.insert(child_id.clone(), node);
        if let Some(parent) = self.nodes.get_mut(parent_id) {
            if !parent.children.contains(&child_id) {
                parent.children.push(child_id);
            }
        }
    }

    pub fn get(&self, id: &str) -> Option<&WidgetNode> {
        self.nodes.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut WidgetNode> {
        self.nodes.get_mut(id)
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn root_ids(&self) -> &[String] {
        &self.root_ids
    }

    /// Depth of a node (0 for roots).
    pub fn depth(&self, id: &str) -> usize {
        let mut d = 0;
        let mut current = id.to_string();
        while let Some(node) = self.nodes.get(&current) {
            if let Some(ref p) = node.parent {
                d += 1;
                current = p.clone();
            } else {
                break;
            }
        }
        d
    }

    /// All descendants of a node (breadth-first).
    pub fn descendants(&self, id: &str) -> Vec<&str> {
        let mut result = Vec::new();
        let mut queue = std::collections::VecDeque::new();
        if let Some(node) = self.nodes.get(id) {
            for c in &node.children {
                queue.push_back(c.as_str());
            }
        }
        while let Some(current) = queue.pop_front() {
            result.push(current);
            if let Some(node) = self.nodes.get(current) {
                for c in &node.children {
                    queue.push_back(c.as_str());
                }
            }
        }
        result
    }

    // --- Z-ordering ---

    /// Return all visible node ids sorted by z-order (ascending = back to front).
    pub fn z_sorted(&self) -> Vec<&str> {
        let mut entries: Vec<(&str, i32)> = self
            .nodes
            .values()
            .filter(|n| n.visible)
            .map(|n| (n.id.as_str(), n.z_order))
            .collect();
        entries.sort_by_key(|(_, z)| *z);
        entries.into_iter().map(|(id, _)| id).collect()
    }

    /// Bring a widget to the front (max z + 1).
    pub fn bring_to_front(&mut self, id: &str) {
        let max_z = self.nodes.values().map(|n| n.z_order).max().unwrap_or(0);
        if let Some(node) = self.nodes.get_mut(id) {
            node.z_order = max_z + 1;
        }
    }

    /// Send a widget to the back (min z - 1).
    pub fn send_to_back(&mut self, id: &str) {
        let min_z = self.nodes.values().map(|n| n.z_order).min().unwrap_or(0);
        if let Some(node) = self.nodes.get_mut(id) {
            node.z_order = min_z - 1;
        }
    }

    // --- Hit testing ---

    /// Find the front-most visible widget that contains the point.
    pub fn hit_test(&self, px: u16, py: u16) -> Option<&str> {
        let sorted = self.z_sorted();
        // Reverse: front-most (highest z) first
        sorted
            .into_iter()
            .rev()
            .find(|id| {
                self.nodes
                    .get(*id)
                    .map(|n| n.contains_point(px, py))
                    .unwrap_or(false)
            })
    }

    /// All visible widgets containing the point, back to front.
    pub fn hit_test_all(&self, px: u16, py: u16) -> Vec<&str> {
        self.z_sorted()
            .into_iter()
            .filter(|id| {
                self.nodes
                    .get(*id)
                    .map(|n| n.contains_point(px, py))
                    .unwrap_or(false)
            })
            .collect()
    }

    // --- Serialization helpers ---

    /// Serialize the tree structure to a flat list of (id, parent, z, bounds) tuples.
    pub fn to_entries(&self) -> Vec<(String, Option<String>, i32, (u16, u16, u16, u16))> {
        let mut entries: Vec<_> = self
            .nodes
            .values()
            .map(|n| (n.id.clone(), n.parent.clone(), n.z_order, n.bounds))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
    }

    /// Reconstruct a tree from entries produced by `to_entries`.
    pub fn from_entries(entries: &[(String, Option<String>, i32, (u16, u16, u16, u16))]) -> Self {
        let mut tree = Self::new();
        // First pass: add all nodes
        for (id, _, z, bounds) in entries {
            let node = WidgetNode::new(id.clone())
                .with_z_order(*z)
                .with_bounds(bounds.0, bounds.1, bounds.2, bounds.3);
            tree.nodes.insert(id.clone(), node);
        }
        // Second pass: set parent/child relationships
        for (id, parent, _, _) in entries {
            if let Some(p) = parent {
                if let Some(node) = tree.nodes.get_mut(id.as_str()) {
                    node.parent = Some(p.clone());
                }
                if let Some(parent_node) = tree.nodes.get_mut(p.as_str()) {
                    if !parent_node.children.contains(id) {
                        parent_node.children.push(id.clone());
                    }
                }
            } else {
                if !tree.root_ids.contains(id) {
                    tree.root_ids.push(id.clone());
                }
            }
        }
        tree
    }
}

impl Default for WidgetTree {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WidgetTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WidgetTree({} nodes, {} roots)",
            self.nodes.len(),
            self.root_ids.len()
        )
    }
}

// ---------------------------------------------------------------------------
// WidgetTree — additional methods
// ---------------------------------------------------------------------------

impl WidgetTree {
    /// Return the path from root to the given node (inclusive).
    pub fn path_to_root(&self, id: &str) -> Vec<&str> {
        let mut path = Vec::new();
        let mut current = id.to_string();
        loop {
            match self.nodes.get(&current) {
                Some(node) => {
                    path.push(node.id.as_str());
                    match &node.parent {
                        Some(p) => current = p.clone(),
                        None => break,
                    }
                }
                None => break,
            }
        }
        path.reverse();
        path
    }

    /// Return all leaf nodes (nodes with no children).
    pub fn leaves(&self) -> Vec<&str> {
        self.nodes
            .values()
            .filter(|n| n.children.is_empty())
            .map(|n| n.id.as_str())
            .collect()
    }

    /// Return the sibling IDs of a given node (excluding itself).
    pub fn siblings(&self, id: &str) -> Vec<&str> {
        let parent_id = match self.nodes.get(id) {
            Some(node) => node.parent.as_deref(),
            None => return Vec::new(),
        };
        match parent_id {
            Some(pid) => match self.nodes.get(pid) {
                Some(parent) => parent
                    .children
                    .iter()
                    .filter(|c| c.as_str() != id)
                    .map(|c| c.as_str())
                    .collect(),
                None => Vec::new(),
            },
            None => self
                .root_ids
                .iter()
                .filter(|r| r.as_str() != id)
                .map(|r| r.as_str())
                .collect(),
        }
    }

    /// Remove a node and all its descendants. Returns the count of removed nodes.
    pub fn remove_subtree(&mut self, id: &str) -> usize {
        let descendants: Vec<String> = self
            .descendants(id)
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut removed = 0;
        for desc in &descendants {
            if self.nodes.remove(desc).is_some() {
                removed += 1;
            }
        }
        // Remove the node itself
        if self.nodes.remove(id).is_some() {
            removed += 1;
        }
        // Remove from parent's children
        for node in self.nodes.values_mut() {
            node.children.retain(|c| c != id);
        }
        self.root_ids.retain(|r| r != id);
        removed
    }

    /// Toggle visibility of a node.
    pub fn toggle_visibility(&mut self, id: &str) -> Option<bool> {
        self.nodes.get_mut(id).map(|n| {
            n.visible = !n.visible;
            n.visible
        })
    }

    /// Count only the visible nodes.
    pub fn visible_count(&self) -> usize {
        self.nodes.values().filter(|n| n.visible).count()
    }
}

// ---------------------------------------------------------------------------
// WidgetId — type-safe widget identifier
// ---------------------------------------------------------------------------

/// A strongly-typed widget identifier for compile-time safety.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WidgetId(String);

impl WidgetId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WidgetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for WidgetId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

// ---------------------------------------------------------------------------
// TextInput – state management for a single-line text input widget
// ---------------------------------------------------------------------------

/// State for a single-line text input widget.
#[derive(Debug, Clone)]
pub struct TextInput {
    content: String,
    cursor: usize,
    selection: Option<(usize, usize)>,
    max_length: Option<usize>,
    placeholder: String,
    focused: bool,
}

impl TextInput {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor: 0,
            selection: None,
            max_length: None,
            placeholder: String::new(),
            focused: false,
        }
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn with_max_length(mut self, max: usize) -> Self {
        self.max_length = Some(max);
        self
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn display_text(&self) -> &str {
        if self.content.is_empty() {
            &self.placeholder
        } else {
            &self.content
        }
    }

    pub fn cursor_position(&self) -> usize {
        self.cursor
    }

    pub fn insert_char(&mut self, ch: char) {
        if let Some(max) = self.max_length {
            if self.content.len() >= max {
                return;
            }
        }
        self.delete_selection();
        let byte_pos = self.byte_offset(self.cursor);
        self.content.insert(byte_pos, ch);
        self.cursor += 1;
    }

    pub fn insert_str(&mut self, s: &str) {
        for ch in s.chars() {
            self.insert_char(ch);
        }
    }

    pub fn delete_back(&mut self) {
        if self.selection.is_some() {
            self.delete_selection();
            return;
        }
        if self.cursor == 0 {
            return;
        }
        let byte_pos = self.byte_offset(self.cursor - 1);
        let end = self.byte_offset(self.cursor);
        self.content.drain(byte_pos..end);
        self.cursor -= 1;
    }

    pub fn delete_forward(&mut self) {
        if self.selection.is_some() {
            self.delete_selection();
            return;
        }
        let len = self.content.chars().count();
        if self.cursor >= len {
            return;
        }
        let byte_pos = self.byte_offset(self.cursor);
        let end = self.byte_offset(self.cursor + 1);
        self.content.drain(byte_pos..end);
    }

    pub fn move_cursor_left(&mut self) {
        self.selection = None;
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_cursor_right(&mut self) {
        self.selection = None;
        let len = self.content.chars().count();
        if self.cursor < len {
            self.cursor += 1;
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.selection = None;
        self.cursor = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.selection = None;
        self.cursor = self.content.chars().count();
    }

    pub fn select_all(&mut self) {
        let len = self.content.chars().count();
        if len > 0 {
            self.selection = Some((0, len));
            self.cursor = len;
        }
    }

    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selection
    }

    pub fn selected_text(&self) -> Option<&str> {
        self.selection.map(|(start, end)| {
            let s = self.byte_offset(start);
            let e = self.byte_offset(end);
            &self.content[s..e]
        })
    }

    pub fn clear(&mut self) {
        self.content.clear();
        self.cursor = 0;
        self.selection = None;
    }

    pub fn set_content(&mut self, s: impl Into<String>) {
        self.content = s.into();
        if let Some(max) = self.max_length {
            self.content.truncate(max);
        }
        let len = self.content.chars().count();
        self.cursor = len;
        self.selection = None;
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    pub fn char_count(&self) -> usize {
        self.content.chars().count()
    }

    fn delete_selection(&mut self) {
        if let Some((start, end)) = self.selection.take() {
            let s = self.byte_offset(start);
            let e = self.byte_offset(end);
            self.content.drain(s..e);
            self.cursor = start;
        }
    }

    fn byte_offset(&self, char_idx: usize) -> usize {
        self.content
            .char_indices()
            .nth(char_idx)
            .map(|(i, _)| i)
            .unwrap_or(self.content.len())
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TextInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TextInput(\"{}\" cursor={})", self.content, self.cursor)
    }
}

// ---------------------------------------------------------------------------
// ProgressBar – computation helper for progress bar rendering
// ---------------------------------------------------------------------------

/// Progress bar computation helper.
#[derive(Debug, Clone)]
pub struct ProgressBar {
    current: u64,
    total: u64,
    label: Option<String>,
}

impl ProgressBar {
    pub fn new(total: u64) -> Self {
        Self {
            current: 0,
            total: total.max(1),
            label: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn set_progress(&mut self, current: u64) {
        self.current = current.min(self.total);
    }

    pub fn increment(&mut self, amount: u64) {
        self.current = (self.current + amount).min(self.total);
    }

    pub fn fraction(&self) -> f64 {
        self.current as f64 / self.total as f64
    }

    pub fn percentage(&self) -> u8 {
        (self.fraction() * 100.0).round() as u8
    }

    /// Returns the filled width in columns for a given total bar width.
    pub fn filled_width(&self, bar_width: u16) -> u16 {
        (self.fraction() * bar_width as f64).round() as u16
    }

    pub fn is_complete(&self) -> bool {
        self.current >= self.total
    }

    pub fn remaining(&self) -> u64 {
        self.total.saturating_sub(self.current)
    }

    pub fn current(&self) -> u64 {
        self.current
    }

    pub fn total(&self) -> u64 {
        self.total
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn reset(&mut self) {
        self.current = 0;
    }

    /// Render a simple text-based progress bar: `[####----] 50%`
    pub fn render_text(&self, width: usize) -> String {
        let inner = width.saturating_sub(2); // account for [ ]
        let filled = (self.fraction() * inner as f64).round() as usize;
        let empty = inner.saturating_sub(filled);
        format!(
            "[{}{}] {}%",
            "#".repeat(filled),
            "-".repeat(empty),
            self.percentage()
        )
    }
}

impl fmt::Display for ProgressBar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref label) = self.label {
            write!(f, "{}: {}%", label, self.percentage())
        } else {
            write!(f, "{}%", self.percentage())
        }
    }
}

// ---------------------------------------------------------------------------
// Checkbox – toggle / checkbox widget state
// ---------------------------------------------------------------------------

/// State for a checkbox / toggle widget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkbox {
    checked: bool,
    label: String,
    enabled: bool,
}

impl Checkbox {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            checked: false,
            label: label.into(),
            enabled: true,
        }
    }

    pub fn with_checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn toggle(&mut self) {
        if self.enabled {
            self.checked = !self.checked;
        }
    }

    pub fn set_checked(&mut self, checked: bool) {
        self.checked = checked;
    }

    pub fn is_checked(&self) -> bool {
        self.checked
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the display string like `[x] Label` or `[ ] Label`.
    pub fn display(&self) -> String {
        let mark = if self.checked { "x" } else { " " };
        format!("[{}] {}", mark, self.label)
    }
}

impl fmt::Display for Checkbox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

// ---------------------------------------------------------------------------
// Dropdown – select / dropdown widget state
// ---------------------------------------------------------------------------

/// State for a dropdown / select widget.
#[derive(Debug, Clone)]
pub struct Dropdown {
    items: Vec<String>,
    selected: Option<usize>,
    open: bool,
    max_visible: usize,
    scroll_offset: usize,
}

impl Dropdown {
    pub fn new(items: Vec<String>) -> Self {
        Self {
            items,
            selected: None,
            open: false,
            max_visible: 8,
            scroll_offset: 0,
        }
    }

    pub fn with_max_visible(mut self, max: usize) -> Self {
        self.max_visible = max.max(1);
        self
    }

    pub fn toggle_open(&mut self) {
        self.open = !self.open;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn select(&mut self, index: usize) {
        if index < self.items.len() {
            self.selected = Some(index);
            self.open = false;
        }
    }

    pub fn select_next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let next = match self.selected {
            Some(i) => (i + 1).min(self.items.len() - 1),
            None => 0,
        };
        self.selected = Some(next);
        self.ensure_visible(next);
    }

    pub fn select_previous(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let prev = match self.selected {
            Some(i) => i.saturating_sub(1),
            None => 0,
        };
        self.selected = Some(prev);
        self.ensure_visible(prev);
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    pub fn selected_item(&self) -> Option<&str> {
        self.selected.and_then(|i| self.items.get(i).map(|s| s.as_str()))
    }

    pub fn items(&self) -> &[String] {
        &self.items
    }

    /// Returns the slice of items visible in the dropdown viewport.
    pub fn visible_items(&self) -> &[String] {
        let end = (self.scroll_offset + self.max_visible).min(self.items.len());
        &self.items[self.scroll_offset..end]
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    fn ensure_visible(&mut self, index: usize) {
        if index < self.scroll_offset {
            self.scroll_offset = index;
        } else if index >= self.scroll_offset + self.max_visible {
            self.scroll_offset = index + 1 - self.max_visible;
        }
    }
}

impl fmt::Display for Dropdown {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.selected_item() {
            Some(item) => write!(f, "Dropdown(\"{}\")", item),
            None => write!(f, "Dropdown(none)"),
        }
    }
}

// ---------------------------------------------------------------------------
// Notification – transient notification widget
// ---------------------------------------------------------------------------

/// Severity level for notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
}

impl fmt::Display for NotificationLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotificationLevel::Info => write!(f, "INFO"),
            NotificationLevel::Warning => write!(f, "WARN"),
            NotificationLevel::Error => write!(f, "ERROR"),
        }
    }
}

/// A notification message with level and optional auto-dismiss duration.
#[derive(Debug, Clone)]
pub struct Notification {
    message: String,
    level: NotificationLevel,
    dismiss_after_ms: Option<u64>,
    dismissed: bool,
}

impl Notification {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: NotificationLevel::Info,
            dismiss_after_ms: Some(5000),
            dismissed: false,
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: NotificationLevel::Warning,
            dismiss_after_ms: Some(8000),
            dismissed: false,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: NotificationLevel::Error,
            dismiss_after_ms: None,
            dismissed: false,
        }
    }

    pub fn with_duration_ms(mut self, ms: u64) -> Self {
        self.dismiss_after_ms = Some(ms);
        self
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn level(&self) -> NotificationLevel {
        self.level
    }

    pub fn dismiss_after_ms(&self) -> Option<u64> {
        self.dismiss_after_ms
    }

    pub fn dismiss(&mut self) {
        self.dismissed = true;
    }

    pub fn is_dismissed(&self) -> bool {
        self.dismissed
    }

    pub fn is_persistent(&self) -> bool {
        self.dismiss_after_ms.is_none()
    }
}

impl fmt::Display for Notification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.level, self.message)
    }
}

/// Queue that manages multiple notifications.
#[derive(Debug, Clone, Default)]
pub struct NotificationQueue {
    notifications: Vec<Notification>,
}

impl NotificationQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, notification: Notification) {
        self.notifications.push(notification);
    }

    pub fn dismiss_all(&mut self) {
        for n in &mut self.notifications {
            n.dismiss();
        }
    }

    pub fn remove_dismissed(&mut self) {
        self.notifications.retain(|n| !n.dismissed);
    }

    pub fn active(&self) -> Vec<&Notification> {
        self.notifications.iter().filter(|n| !n.dismissed).collect()
    }

    pub fn active_count(&self) -> usize {
        self.notifications.iter().filter(|n| !n.dismissed).count()
    }

    pub fn is_empty(&self) -> bool {
        self.active_count() == 0
    }

    pub fn total_count(&self) -> usize {
        self.notifications.len()
    }
}

// ---------------------------------------------------------------------------
// Breadcrumb – path-style breadcrumb widget
// ---------------------------------------------------------------------------

/// A breadcrumb trail (e.g. `File > src > main.rs`).
#[derive(Debug, Clone)]
pub struct Breadcrumb {
    segments: Vec<String>,
    separator: String,
}

impl Breadcrumb {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            separator: " > ".to_string(),
        }
    }

    pub fn with_separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn push(&mut self, segment: impl Into<String>) {
        self.segments.push(segment.into());
    }

    pub fn pop(&mut self) -> Option<String> {
        self.segments.pop()
    }

    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Navigate to a specific depth, truncating everything after.
    pub fn navigate_to(&mut self, depth: usize) {
        self.segments.truncate(depth);
    }

    /// Returns the last segment (current location).
    pub fn current(&self) -> Option<&str> {
        self.segments.last().map(|s| s.as_str())
    }

    /// Render the breadcrumb trail as a single string.
    pub fn render(&self) -> String {
        self.segments.join(&self.separator)
    }
}

impl Default for Breadcrumb {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Breadcrumb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render())
    }
}

// ---------------------------------------------------------------------------
// ScrollState – scrollbar / viewport position tracking
// ---------------------------------------------------------------------------

/// Tracks scroll position for a scrollable viewport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollState {
    offset: usize,
    viewport_size: usize,
    content_size: usize,
}

impl ScrollState {
    pub fn new(viewport_size: usize, content_size: usize) -> Self {
        Self {
            offset: 0,
            viewport_size: viewport_size.max(1),
            content_size,
        }
    }

    pub fn scroll_down(&mut self, lines: usize) {
        let max = self.max_offset();
        self.offset = (self.offset + lines).min(max);
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.offset = self.offset.saturating_sub(lines);
    }

    pub fn scroll_to(&mut self, offset: usize) {
        self.offset = offset.min(self.max_offset());
    }

    pub fn scroll_to_top(&mut self) {
        self.offset = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.offset = self.max_offset();
    }

    pub fn page_down(&mut self) {
        self.scroll_down(self.viewport_size);
    }

    pub fn page_up(&mut self) {
        self.scroll_up(self.viewport_size);
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn viewport_size(&self) -> usize {
        self.viewport_size
    }

    pub fn content_size(&self) -> usize {
        self.content_size
    }

    pub fn set_content_size(&mut self, size: usize) {
        self.content_size = size;
        self.offset = self.offset.min(self.max_offset());
    }

    pub fn max_offset(&self) -> usize {
        self.content_size.saturating_sub(self.viewport_size)
    }

    pub fn is_at_top(&self) -> bool {
        self.offset == 0
    }

    pub fn is_at_bottom(&self) -> bool {
        self.offset >= self.max_offset()
    }

    pub fn needs_scrollbar(&self) -> bool {
        self.content_size > self.viewport_size
    }

    /// Fraction of the content scrolled (0.0 = top, 1.0 = bottom).
    pub fn scroll_fraction(&self) -> f64 {
        let max = self.max_offset();
        if max == 0 {
            0.0
        } else {
            self.offset as f64 / max as f64
        }
    }

    /// Size of the scrollbar thumb relative to the viewport.
    pub fn thumb_size(&self, track_height: u16) -> u16 {
        if self.content_size == 0 {
            return track_height;
        }
        let ratio = self.viewport_size as f64 / self.content_size as f64;
        let size = (ratio * track_height as f64).round() as u16;
        size.max(1).min(track_height)
    }

    /// Offset of the scrollbar thumb within the track.
    pub fn thumb_offset(&self, track_height: u16) -> u16 {
        let thumb = self.thumb_size(track_height);
        let available = track_height.saturating_sub(thumb);
        (self.scroll_fraction() * available as f64).round() as u16
    }

    /// Ensure a particular content line is visible, scrolling if needed.
    pub fn ensure_visible(&mut self, line: usize) {
        if line < self.offset {
            self.offset = line;
        } else if line >= self.offset + self.viewport_size {
            self.offset = line + 1 - self.viewport_size;
        }
    }

    /// Returns the range of visible content indices.
    pub fn visible_range(&self) -> std::ops::Range<usize> {
        let end = (self.offset + self.viewport_size).min(self.content_size);
        self.offset..end
    }
}

impl fmt::Display for ScrollState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Scroll({}/{} viewport={})",
            self.offset, self.content_size, self.viewport_size
        )
    }
}



// ---------------------------------------------------------------------------
// widgets – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for UI widgets library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YWidgetsWidgetVisibility {
    Visible,
    Hidden,
    Collapsed,
    Disabled,
}

impl YWidgetsWidgetVisibility {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Visible => 0,
            Self::Hidden => 1,
            Self::Collapsed => 2,
            Self::Disabled => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Visible => "Visible",
            Self::Hidden => "Hidden",
            Self::Collapsed => "Collapsed",
            Self::Disabled => "Disabled",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YWidgetsWidgetVisibility] {
        &[
            YWidgetsWidgetVisibility::Visible,
            YWidgetsWidgetVisibility::Hidden,
            YWidgetsWidgetVisibility::Collapsed,
            YWidgetsWidgetVisibility::Disabled,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YWidgetsWidgetVisibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks widget bounds data.
#[derive(Debug, Clone)]
pub struct YWidgetsWidgetBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl YWidgetsWidgetBounds {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YWidgetsWidgetBounds({}: {:?})", "x", self.x)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_widgets_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_widgets_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_widgets_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_widgets_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_widgets_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_widgets_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_widgets_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_widgets_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// widgets – Extended widget focus chain helpers
// ---------------------------------------------------------------------------

/// Priority levels for widget focus chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZWidgetsPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZWidgetsPriority {
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
    pub fn all_asc() -> [ZWidgetsPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZWidgetsPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks widget focus chain data.
#[derive(Debug, Clone)]
pub struct ZWidgetsWidgetFocusChain {
    pub widget_ids: Vec<String>,
    pub focused_idx: usize,
    pub wrap_around: bool,
}

impl ZWidgetsWidgetFocusChain {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            widget_ids: Vec::new(),
            focused_idx: 0,
            wrap_around: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.widget_ids.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.widget_ids.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.widget_ids.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZWidgetsWidgetFocusChain[focused_idx={:?}, wrap_around={:?}]", self.focused_idx, self.wrap_around)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.wrap_around = !c.wrap_around;
        c
    }
}

/// Compute a simple rolling hash for widget focus chain.
pub fn z_widgets_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_widgets_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_widgets_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_widgets_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_widgets_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_widgets_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_widgets_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 237
// ---------------------------------------------------------------------------

/// Generic object pool `Xc237Pool<T>`.
pub struct Xc237Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc237Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc237PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc237Pool<T> {
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
    pub fn stats(&self) -> Xc237PoolStats {
        Xc237PoolStats {
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

impl<T> Default for Xc237Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc237Scheduler`.
pub struct Xc237Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc237Scheduler {
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

impl Default for Xc237Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_237 hash for the given byte slice.
pub fn xc_237_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_237 convention.
pub fn xc_237_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_8 deepening: state machine + event bus ---

/// States for the Xd8 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd8State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd8State {
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
pub struct Xd8Transition {
    pub from: Xd8State,
    pub to: Xd8State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd8StateMachine {
    current: Xd8State,
    history: Vec<Xd8Transition>,
    step_counter: usize,
}

impl Xd8StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd8State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd8State {
        self.current
    }

    pub fn history(&self) -> &[Xd8Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd8State) -> Result<Xd8State, String> {
        let allowed = match (self.current, target) {
            (Xd8State::Idle, Xd8State::Running) => true,
            (Xd8State::Running, Xd8State::Paused) => true,
            (Xd8State::Running, Xd8State::Done) => true,
            (Xd8State::Paused, Xd8State::Running) => true,
            (Xd8State::Paused, Xd8State::Done) => true,
            (Xd8State::Done, Xd8State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_8: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd8Transition {
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
            "Xd8SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd8State> {
        let prefix = "Xd8SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd8State::Idle),
            "Running" => Some(Xd8State::Running),
            "Paused" => Some(Xd8State::Paused),
            "Done" => Some(Xd8State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd8State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd8 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd8Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd8Event {
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

type Xd8HandlerFn = Box<dyn Fn(&Xd8Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd8EventBus {
    handlers: Vec<(usize, Option<String>, Xd8HandlerFn)>,
    next_id: usize,
    published: Vec<Xd8Event>,
}

impl Xd8EventBus {
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
        F: Fn(&Xd8Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd8Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd8Event) {
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

    pub fn published_events(&self) -> &[Xd8Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #6
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf6Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf6TrieNode {
    children: std::collections::HashMap<char, Xf6TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf6Trie {
    root: Xf6TrieNode,
    count: usize,
}

impl Xf6Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf6TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf6TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf6TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf6BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf6BloomFilter {
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

    // --- New tests for WidgetTree, z-ordering, hit testing, serialization ---

    #[test]
    fn widget_tree_parent_child() {
        let mut tree = WidgetTree::new();
        tree.add_root(WidgetNode::new("root"));
        tree.add_child("root", WidgetNode::new("child1"));
        tree.add_child("root", WidgetNode::new("child2"));
        assert_eq!(tree.node_count(), 3);
        assert_eq!(tree.depth("root"), 0);
        assert_eq!(tree.depth("child1"), 1);
        let descendants = tree.descendants("root");
        assert_eq!(descendants.len(), 2);
    }

    #[test]
    fn widget_tree_z_ordering() {
        let mut tree = WidgetTree::new();
        tree.add_root(WidgetNode::new("a").with_z_order(1));
        tree.add_root(WidgetNode::new("b").with_z_order(3));
        tree.add_root(WidgetNode::new("c").with_z_order(2));
        let sorted = tree.z_sorted();
        assert_eq!(sorted, vec!["a", "c", "b"]);
        tree.bring_to_front("a");
        let sorted2 = tree.z_sorted();
        assert_eq!(sorted2.last(), Some(&"a"));
    }

    #[test]
    fn widget_tree_hit_test() {
        let mut tree = WidgetTree::new();
        tree.add_root(WidgetNode::new("bg").with_bounds(0, 0, 100, 100).with_z_order(0));
        tree.add_root(WidgetNode::new("fg").with_bounds(10, 10, 20, 20).with_z_order(1));
        // Point in fg area – should hit fg (front-most)
        assert_eq!(tree.hit_test(15, 15), Some("fg"));
        // Point outside fg but inside bg
        assert_eq!(tree.hit_test(5, 5), Some("bg"));
        // Point outside both
        assert_eq!(tree.hit_test(200, 200), None);
    }

    #[test]
    fn widget_tree_hit_test_all() {
        let mut tree = WidgetTree::new();
        tree.add_root(WidgetNode::new("bg").with_bounds(0, 0, 100, 100).with_z_order(0));
        tree.add_root(WidgetNode::new("fg").with_bounds(0, 0, 50, 50).with_z_order(1));
        let hits = tree.hit_test_all(10, 10);
        assert_eq!(hits, vec!["bg", "fg"]);
    }

    #[test]
    fn widget_tree_serialization_roundtrip() {
        let mut tree = WidgetTree::new();
        tree.add_root(WidgetNode::new("root").with_bounds(0, 0, 80, 24).with_z_order(0));
        tree.add_child("root", WidgetNode::new("child").with_bounds(5, 5, 10, 10).with_z_order(1));
        let entries = tree.to_entries();
        let tree2 = WidgetTree::from_entries(&entries);
        assert_eq!(tree2.node_count(), 2);
        assert_eq!(tree2.depth("child"), 1);
        assert_eq!(tree2.root_ids().len(), 1);
    }

    #[test]
    fn widget_node_contains_point() {
        let node = WidgetNode::new("test").with_bounds(10, 20, 30, 40);
        assert!(node.contains_point(10, 20)); // top-left
        assert!(node.contains_point(39, 59)); // bottom-right edge - 1
        assert!(!node.contains_point(40, 60)); // outside
        assert!(!node.contains_point(9, 20)); // left of bounds
    }

    #[test]
    fn widget_tree_send_to_back() {
        let mut tree = WidgetTree::new();
        tree.add_root(WidgetNode::new("a").with_z_order(1));
        tree.add_root(WidgetNode::new("b").with_z_order(2));
        tree.send_to_back("b");
        assert_eq!(tree.z_sorted().first(), Some(&"b"));
    }

    // -- WidgetTree extended tests --------------------------------------------

    #[test]
    fn widget_tree_path_to_root() {
        let mut tree = WidgetTree::new();
        tree.add_root(WidgetNode::new("root"));
        tree.add_child("root", WidgetNode::new("mid"));
        tree.add_child("mid", WidgetNode::new("leaf"));
        let path = tree.path_to_root("leaf");
        assert_eq!(path, vec!["root", "mid", "leaf"]);
    }

    #[test]
    fn widget_tree_leaves() {
        let mut tree = WidgetTree::new();
        tree.add_root(WidgetNode::new("root"));
        tree.add_child("root", WidgetNode::new("child1"));
        tree.add_child("root", WidgetNode::new("child2"));
        tree.add_child("child1", WidgetNode::new("grandchild"));
        let mut leaves = tree.leaves();
        leaves.sort();
        assert_eq!(leaves, vec!["child2", "grandchild"]);
    }

    #[test]
    fn widget_tree_siblings() {
        let mut tree = WidgetTree::new();
        tree.add_root(WidgetNode::new("root"));
        tree.add_child("root", WidgetNode::new("a"));
        tree.add_child("root", WidgetNode::new("b"));
        tree.add_child("root", WidgetNode::new("c"));
        let mut sibs = tree.siblings("b");
        sibs.sort();
        assert_eq!(sibs, vec!["a", "c"]);
    }

    #[test]
    fn widget_tree_remove_subtree() {
        let mut tree = WidgetTree::new();
        tree.add_root(WidgetNode::new("root"));
        tree.add_child("root", WidgetNode::new("child"));
        tree.add_child("child", WidgetNode::new("grandchild"));
        let removed = tree.remove_subtree("child");
        assert_eq!(removed, 2);
        assert_eq!(tree.node_count(), 1);
    }

    #[test]
    fn widget_tree_toggle_visibility() {
        let mut tree = WidgetTree::new();
        tree.add_root(WidgetNode::new("w1"));
        assert_eq!(tree.visible_count(), 1);
        tree.toggle_visibility("w1");
        assert_eq!(tree.visible_count(), 0);
        tree.toggle_visibility("w1");
        assert_eq!(tree.visible_count(), 1);
    }

    #[test]
    fn widget_id_display_and_from() {
        let id = WidgetId::new("my-widget");
        assert_eq!(id.as_str(), "my-widget");
        assert_eq!(format!("{id}"), "my-widget");
        let id2: WidgetId = "other".into();
        assert_ne!(id, id2);
    }

    #[test]
    fn widget_tree_root_siblings() {
        let mut tree = WidgetTree::new();
        tree.add_root(WidgetNode::new("r1"));
        tree.add_root(WidgetNode::new("r2"));
        tree.add_root(WidgetNode::new("r3"));
        let sibs = tree.siblings("r2");
        assert_eq!(sibs.len(), 2);
    }

    // -- TextInput tests ------------------------------------------------------

    #[test]
    fn text_input_insert_and_cursor() {
        let mut ti = TextInput::new();
        ti.insert_char('H');
        ti.insert_char('i');
        assert_eq!(ti.content(), "Hi");
        assert_eq!(ti.cursor_position(), 2);
        assert_eq!(ti.char_count(), 2);
    }

    #[test]
    fn text_input_delete_back() {
        let mut ti = TextInput::new();
        ti.insert_str("abc");
        ti.delete_back();
        assert_eq!(ti.content(), "ab");
        assert_eq!(ti.cursor_position(), 2);
    }

    #[test]
    fn text_input_delete_forward() {
        let mut ti = TextInput::new();
        ti.insert_str("abc");
        ti.move_cursor_home();
        ti.delete_forward();
        assert_eq!(ti.content(), "bc");
        assert_eq!(ti.cursor_position(), 0);
    }

    #[test]
    fn text_input_max_length() {
        let mut ti = TextInput::new().with_max_length(3);
        ti.insert_str("abcdef");
        assert_eq!(ti.content(), "abc");
    }

    #[test]
    fn text_input_select_all_and_delete() {
        let mut ti = TextInput::new();
        ti.insert_str("hello");
        ti.select_all();
        assert_eq!(ti.selected_text(), Some("hello"));
        ti.delete_back();
        assert!(ti.is_empty());
    }

    #[test]
    fn text_input_placeholder_display() {
        let ti = TextInput::new().with_placeholder("Type here...");
        assert_eq!(ti.display_text(), "Type here...");
        let mut ti2 = TextInput::new().with_placeholder("hint");
        ti2.insert_char('x');
        assert_eq!(ti2.display_text(), "x");
    }

    #[test]
    fn text_input_cursor_movement() {
        let mut ti = TextInput::new();
        ti.insert_str("abcd");
        ti.move_cursor_home();
        assert_eq!(ti.cursor_position(), 0);
        ti.move_cursor_right();
        assert_eq!(ti.cursor_position(), 1);
        ti.move_cursor_end();
        assert_eq!(ti.cursor_position(), 4);
        ti.move_cursor_left();
        assert_eq!(ti.cursor_position(), 3);
    }

    #[test]
    fn text_input_set_content() {
        let mut ti = TextInput::new().with_max_length(5);
        ti.set_content("toolong");
        assert_eq!(ti.content(), "toolo");
        assert_eq!(ti.cursor_position(), 5);
    }

    #[test]
    fn text_input_clear() {
        let mut ti = TextInput::new();
        ti.insert_str("data");
        ti.clear();
        assert!(ti.is_empty());
        assert_eq!(ti.cursor_position(), 0);
    }

    #[test]
    fn text_input_display() {
        let mut ti = TextInput::new();
        ti.insert_str("yo");
        let s = format!("{ti}");
        assert!(s.contains("yo"));
        assert!(s.contains("cursor=2"));
    }

    // -- ProgressBar tests ----------------------------------------------------

    #[test]
    fn progress_bar_basic() {
        let mut pb = ProgressBar::new(100);
        assert_eq!(pb.percentage(), 0);
        assert!(!pb.is_complete());
        pb.set_progress(50);
        assert_eq!(pb.percentage(), 50);
        assert_eq!(pb.remaining(), 50);
        pb.set_progress(100);
        assert!(pb.is_complete());
    }

    #[test]
    fn progress_bar_increment() {
        let mut pb = ProgressBar::new(10);
        pb.increment(3);
        pb.increment(3);
        assert_eq!(pb.current(), 6);
        pb.increment(100);
        assert_eq!(pb.current(), 10); // clamped
    }

    #[test]
    fn progress_bar_filled_width() {
        let mut pb = ProgressBar::new(100);
        pb.set_progress(50);
        assert_eq!(pb.filled_width(80), 40);
    }

    #[test]
    fn progress_bar_render_text() {
        let mut pb = ProgressBar::new(4);
        pb.set_progress(2);
        let rendered = pb.render_text(10);
        assert!(rendered.contains("50%"));
        assert!(rendered.starts_with('['));
        assert!(rendered.contains(']'));
    }

    #[test]
    fn progress_bar_display_with_label() {
        let mut pb = ProgressBar::new(200).with_label("Loading");
        pb.set_progress(100);
        let s = format!("{pb}");
        assert_eq!(s, "Loading: 50%");
    }

    #[test]
    fn progress_bar_reset() {
        let mut pb = ProgressBar::new(50);
        pb.set_progress(25);
        pb.reset();
        assert_eq!(pb.current(), 0);
        assert_eq!(pb.percentage(), 0);
    }

    // -- Checkbox tests -------------------------------------------------------

    #[test]
    fn checkbox_toggle() {
        let mut cb = Checkbox::new("Accept terms");
        assert!(!cb.is_checked());
        cb.toggle();
        assert!(cb.is_checked());
        assert_eq!(cb.display(), "[x] Accept terms");
        cb.toggle();
        assert_eq!(cb.display(), "[ ] Accept terms");
    }

    #[test]
    fn checkbox_disabled_no_toggle() {
        let mut cb = Checkbox::new("Read-only");
        cb.set_enabled(false);
        cb.toggle();
        assert!(!cb.is_checked());
    }

    #[test]
    fn checkbox_with_checked() {
        let cb = Checkbox::new("On").with_checked(true);
        assert!(cb.is_checked());
        assert_eq!(cb.label(), "On");
    }

    // -- Dropdown tests -------------------------------------------------------

    #[test]
    fn dropdown_select_and_navigate() {
        let items = vec!["Red".into(), "Green".into(), "Blue".into()];
        let mut dd = Dropdown::new(items);
        assert!(dd.selected_item().is_none());
        dd.select(1);
        assert_eq!(dd.selected_item(), Some("Green"));
        assert!(!dd.is_open()); // select closes dropdown
        dd.select_next();
        assert_eq!(dd.selected_item(), Some("Blue"));
        dd.select_next();
        assert_eq!(dd.selected_item(), Some("Blue")); // stays at end
        dd.select_previous();
        assert_eq!(dd.selected_item(), Some("Green"));
    }

    #[test]
    fn dropdown_toggle_open() {
        let dd_items: Vec<String> = (0..5).map(|i| format!("Item {i}")).collect();
        let mut dd = Dropdown::new(dd_items);
        assert!(!dd.is_open());
        dd.toggle_open();
        assert!(dd.is_open());
        dd.close();
        assert!(!dd.is_open());
    }

    #[test]
    fn dropdown_scroll_and_visible() {
        let items: Vec<String> = (0..20).map(|i| format!("Item {i}")).collect();
        let mut dd = Dropdown::new(items).with_max_visible(5);
        assert_eq!(dd.visible_items().len(), 5);
        assert_eq!(dd.visible_items()[0], "Item 0");
        // Select item 10 to scroll
        dd.select(0);
        for _ in 0..10 {
            dd.select_next();
        }
        assert_eq!(dd.selected_index(), Some(10));
        let vis = dd.visible_items();
        assert!(vis.contains(&"Item 10".to_string()));
    }

    #[test]
    fn dropdown_display() {
        let mut dd = Dropdown::new(vec!["A".into(), "B".into()]);
        assert_eq!(format!("{dd}"), "Dropdown(none)");
        dd.select(0);
        assert_eq!(format!("{dd}"), "Dropdown(\"A\")");
    }

    // -- Notification tests ---------------------------------------------------

    #[test]
    fn notification_levels_and_dismiss() {
        let n = Notification::info("Saved");
        assert_eq!(n.level(), NotificationLevel::Info);
        assert!(!n.is_persistent());
        assert_eq!(n.dismiss_after_ms(), Some(5000));

        let n2 = Notification::error("Crash");
        assert!(n2.is_persistent());
        assert!(!n2.is_dismissed());

        let mut n3 = Notification::warning("Low disk");
        n3.dismiss();
        assert!(n3.is_dismissed());
    }

    #[test]
    fn notification_queue_management() {
        let mut q = NotificationQueue::new();
        assert!(q.is_empty());
        q.push(Notification::info("A"));
        q.push(Notification::error("B"));
        assert_eq!(q.active_count(), 2);
        q.dismiss_all();
        assert_eq!(q.active_count(), 0);
        assert_eq!(q.total_count(), 2);
        q.remove_dismissed();
        assert_eq!(q.total_count(), 0);
    }

    #[test]
    fn notification_display() {
        let n = Notification::warning("Disk full");
        assert_eq!(format!("{n}"), "[WARN] Disk full");
    }

    // -- Breadcrumb tests -----------------------------------------------------

    #[test]
    fn breadcrumb_navigation() {
        let mut bc = Breadcrumb::new();
        bc.push("root");
        bc.push("src");
        bc.push("main.rs");
        assert_eq!(bc.depth(), 3);
        assert_eq!(bc.current(), Some("main.rs"));
        assert_eq!(bc.render(), "root > src > main.rs");

        bc.navigate_to(2);
        assert_eq!(bc.current(), Some("src"));
        assert_eq!(bc.depth(), 2);

        bc.pop();
        assert_eq!(bc.current(), Some("root"));
    }

    #[test]
    fn breadcrumb_custom_separator() {
        let mut bc = Breadcrumb::new().with_separator(" / ");
        bc.push("a");
        bc.push("b");
        assert_eq!(bc.render(), "a / b");
    }

    #[test]
    fn breadcrumb_empty() {
        let bc = Breadcrumb::new();
        assert!(bc.is_empty());
        assert_eq!(bc.current(), None);
        assert_eq!(bc.render(), "");
    }

    // -- ScrollState tests ----------------------------------------------------

    #[test]
    fn scroll_state_basic() {
        let mut ss = ScrollState::new(10, 100);
        assert!(ss.is_at_top());
        assert!(!ss.is_at_bottom());
        assert!(ss.needs_scrollbar());
        assert_eq!(ss.max_offset(), 90);

        ss.scroll_down(5);
        assert_eq!(ss.offset(), 5);
        assert!(!ss.is_at_top());

        ss.scroll_to_bottom();
        assert!(ss.is_at_bottom());
        assert_eq!(ss.offset(), 90);
    }

    #[test]
    fn scroll_state_page_navigation() {
        let mut ss = ScrollState::new(10, 50);
        ss.page_down();
        assert_eq!(ss.offset(), 10);
        ss.page_down();
        assert_eq!(ss.offset(), 20);
        ss.page_up();
        assert_eq!(ss.offset(), 10);
    }

    #[test]
    fn scroll_state_ensure_visible() {
        let mut ss = ScrollState::new(10, 100);
        ss.ensure_visible(15);
        assert_eq!(ss.offset(), 6);
        ss.ensure_visible(3);
        assert_eq!(ss.offset(), 3);
    }

    #[test]
    fn scroll_state_visible_range() {
        let mut ss = ScrollState::new(5, 20);
        assert_eq!(ss.visible_range(), 0..5);
        ss.scroll_down(3);
        assert_eq!(ss.visible_range(), 3..8);
        ss.scroll_to_bottom();
        assert_eq!(ss.visible_range(), 15..20);
    }

    #[test]
    fn scroll_state_thumb_calculation() {
        let ss = ScrollState::new(10, 100);
        let thumb = ss.thumb_size(50);
        assert_eq!(thumb, 5); // 10/100 * 50
        assert_eq!(ss.thumb_offset(50), 0); // at top
    }

    #[test]
    fn scroll_state_no_scrollbar_needed() {
        let ss = ScrollState::new(20, 10);
        assert!(!ss.needs_scrollbar());
        assert_eq!(ss.max_offset(), 0);
        assert!(ss.is_at_top());
        assert!(ss.is_at_bottom()); // content fits
    }

    #[test]
    fn scroll_state_content_resize() {
        let mut ss = ScrollState::new(10, 100);
        ss.scroll_to(90);
        ss.set_content_size(50);
        assert_eq!(ss.offset(), 40); // clamped to new max
    }

    #[test]
    fn scroll_state_fraction() {
        let mut ss = ScrollState::new(10, 110);
        ss.scroll_to(50);
        let frac = ss.scroll_fraction();
        assert!((frac - 0.5).abs() < 0.01);
    }

    #[test]
    fn scroll_state_display() {
        let ss = ScrollState::new(10, 100);
        let s = format!("{ss}");
        assert!(s.contains("0/100"));
    }

    // -- widgets extended domain tests ----------------------------------------

    #[test]
    fn y_widgets_enum_index() {
        assert_eq!(YWidgetsWidgetVisibility::Visible.index(), 0);
        assert_eq!(YWidgetsWidgetVisibility::Hidden.index(), 1);
        assert_eq!(YWidgetsWidgetVisibility::Collapsed.index(), 2);
        assert_eq!(YWidgetsWidgetVisibility::Disabled.index(), 3);
    }

    #[test]
    fn y_widgets_enum_label() {
        assert_eq!(YWidgetsWidgetVisibility::Visible.label(), "Visible");
        assert_eq!(YWidgetsWidgetVisibility::Hidden.label(), "Hidden");
        assert_eq!(YWidgetsWidgetVisibility::Collapsed.label(), "Collapsed");
        assert_eq!(YWidgetsWidgetVisibility::Disabled.label(), "Disabled");
    }

    #[test]
    fn y_widgets_enum_all() {
        let all = YWidgetsWidgetVisibility::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_widgets_enum_is_default() {
        assert!(YWidgetsWidgetVisibility::Visible.is_default());
        assert!(!YWidgetsWidgetVisibility::Disabled.is_default());
    }

    #[test]
    fn y_widgets_enum_display() {
        assert_eq!(format!("{}", YWidgetsWidgetVisibility::Visible), "Visible");
    }

    #[test]
    fn y_widgets_struct_new() {
        let s = YWidgetsWidgetBounds::new();
        let _ = s.summary();
    }

    #[test]
    fn y_widgets_fingerprint_deterministic() {
        let h1 = y_widgets_fingerprint("hello");
        let h2 = y_widgets_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_widgets_fingerprint("a"), y_widgets_fingerprint("b"));
    }

    #[test]
    fn y_widgets_truncate_short() {
        assert_eq!(y_widgets_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_widgets_truncate_long() {
        let r = y_widgets_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_widgets_normalize_key_basic() {
        assert_eq!(y_widgets_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_widgets_split_path_basic() {
        let parts = y_widgets_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_widgets_count_occurrences_basic() {
        assert_eq!(y_widgets_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_widgets_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_widgets_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_widgets_in_range_basic() {
        assert!(y_widgets_in_range(5, 1, 10));
        assert!(y_widgets_in_range(1, 1, 10));
        assert!(y_widgets_in_range(10, 1, 10));
        assert!(!y_widgets_in_range(0, 1, 10));
        assert!(!y_widgets_in_range(11, 1, 10));
    }

    #[test]
    fn y_widgets_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_widgets_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_widgets_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_widgets_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- widgets Z-extended tests -----------------------------------------------

    #[test]
    fn z_widgets_priority_weight() {
        assert_eq!(ZWidgetsPriority::Idle.weight(), 0);
        assert_eq!(ZWidgetsPriority::Normal.weight(), 2);
        assert_eq!(ZWidgetsPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_widgets_priority_label() {
        assert_eq!(ZWidgetsPriority::Low.label(), "low");
        assert_eq!(ZWidgetsPriority::High.label(), "high");
    }

    #[test]
    fn z_widgets_priority_is_elevated() {
        assert!(!ZWidgetsPriority::Normal.is_elevated());
        assert!(ZWidgetsPriority::High.is_elevated());
        assert!(ZWidgetsPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_widgets_priority_display() {
        assert_eq!(format!("{}", ZWidgetsPriority::Idle), "idle");
    }

    #[test]
    fn z_widgets_priority_all_asc() {
        let all = ZWidgetsPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZWidgetsPriority::Idle);
        assert_eq!(all[4], ZWidgetsPriority::Realtime);
    }

    #[test]
    fn z_widgets_struct_new() {
        let s = ZWidgetsWidgetFocusChain::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_widgets_struct_toggled_clone() {
        let s = ZWidgetsWidgetFocusChain::new();
        let t = s.toggled_clone();
        assert_ne!(s.wrap_around, t.wrap_around);
    }

    #[test]
    fn z_widgets_rolling_hash_deterministic() {
        let h1 = z_widgets_rolling_hash(b"test");
        let h2 = z_widgets_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_widgets_rolling_hash(b"a"), z_widgets_rolling_hash(b"b"));
    }

    #[test]
    fn z_widgets_pad_to_basic() {
        assert_eq!(z_widgets_pad_to("hi", 5), "hi   ");
        assert_eq!(z_widgets_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_widgets_is_identifier_basic() {
        assert!(z_widgets_is_identifier("foo_bar"));
        assert!(z_widgets_is_identifier("abc123"));
        assert!(!z_widgets_is_identifier(""));
        assert!(!z_widgets_is_identifier("has space"));
    }

    #[test]
    fn z_widgets_levenshtein_basic() {
        assert_eq!(z_widgets_levenshtein("", ""), 0);
        assert_eq!(z_widgets_levenshtein("abc", "abc"), 0);
        assert_eq!(z_widgets_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_widgets_unique_words_basic() {
        let w = z_widgets_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_widgets_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_widgets_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_widgets_common_prefix_basic() {
        assert_eq!(z_widgets_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_widgets_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_widgets_struct_clear() {
        let mut s = ZWidgetsWidgetFocusChain::new();
        s.widget_ids.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_widgets_rolling_hash_empty() {
        let h = z_widgets_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    // ---- xc_ pool / scheduler tests – block 237 ----

    #[test]
    fn xc_237_pool_new_empty() {
        let pool: super::Xc237Pool<i32> = super::Xc237Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_237_pool_release_acquire() {
        let mut pool = super::Xc237Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_237_pool_acquire_empty() {
        let mut pool: super::Xc237Pool<i32> = super::Xc237Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_237_pool_full() {
        let mut pool = super::Xc237Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_237_pool_drain() {
        let mut pool = super::Xc237Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_237_pool_stats() {
        let mut pool = super::Xc237Pool::new(8);
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
    fn xc_237_pool_clear() {
        let mut pool = super::Xc237Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_237_pool_shrink() {
        let mut pool = super::Xc237Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_237_pool_default() {
        let pool: super::Xc237Pool<String> = super::Xc237Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_237_pool_extend() {
        let mut pool = super::Xc237Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_237_pool_retain() {
        let mut pool = super::Xc237Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_237_scheduler_round_robin() {
        let mut sched = super::Xc237Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_237_scheduler_empty() {
        let mut sched = super::Xc237Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_237_scheduler_reset() {
        let mut sched = super::Xc237Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_237_scheduler_add_remove() {
        let mut sched = super::Xc237Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_237_scheduler_targets() {
        let sched = super::Xc237Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_237_hash_empty() {
        assert_eq!(super::xc_237_hash(b""), 5381);
    }

    #[test]
    fn xc_237_hash_data() {
        let h = super::xc_237_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_237_hash(b"hello"), h);
    }

    #[test]
    fn xc_237_reverse_str() {
        assert_eq!(super::xc_237_reverse("abc"), "cba");
        assert_eq!(super::xc_237_reverse(""), "");
    }


    // --- xd_8 deepening tests ---

    #[test]
    fn xd_8_sm_initial_state() {
        let sm = Xd8StateMachine::new();
        assert_eq!(sm.current_state(), Xd8State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_8_sm_valid_idle_to_running() {
        let mut sm = Xd8StateMachine::new();
        assert!(sm.transition(Xd8State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd8State::Running);
    }

    #[test]
    fn xd_8_sm_valid_running_to_paused() {
        let mut sm = Xd8StateMachine::new();
        sm.transition(Xd8State::Running).unwrap();
        assert!(sm.transition(Xd8State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd8State::Paused);
    }

    #[test]
    fn xd_8_sm_valid_running_to_done() {
        let mut sm = Xd8StateMachine::new();
        sm.transition(Xd8State::Running).unwrap();
        assert!(sm.transition(Xd8State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd8State::Done);
    }

    #[test]
    fn xd_8_sm_valid_paused_to_running() {
        let mut sm = Xd8StateMachine::new();
        sm.transition(Xd8State::Running).unwrap();
        sm.transition(Xd8State::Paused).unwrap();
        assert!(sm.transition(Xd8State::Running).is_ok());
    }

    #[test]
    fn xd_8_sm_valid_done_to_idle() {
        let mut sm = Xd8StateMachine::new();
        sm.transition(Xd8State::Running).unwrap();
        sm.transition(Xd8State::Done).unwrap();
        assert!(sm.transition(Xd8State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd8State::Idle);
    }

    #[test]
    fn xd_8_sm_invalid_idle_to_done() {
        let mut sm = Xd8StateMachine::new();
        assert!(sm.transition(Xd8State::Done).is_err());
    }

    #[test]
    fn xd_8_sm_invalid_idle_to_paused() {
        let mut sm = Xd8StateMachine::new();
        assert!(sm.transition(Xd8State::Paused).is_err());
    }

    #[test]
    fn xd_8_sm_history_tracking() {
        let mut sm = Xd8StateMachine::new();
        sm.transition(Xd8State::Running).unwrap();
        sm.transition(Xd8State::Paused).unwrap();
        sm.transition(Xd8State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd8State::Idle);
        assert_eq!(sm.history()[0].to, Xd8State::Running);
        assert_eq!(sm.history()[1].from, Xd8State::Running);
        assert_eq!(sm.history()[2].to, Xd8State::Done);
    }

    #[test]
    fn xd_8_sm_serialize_deserialize() {
        let mut sm = Xd8StateMachine::new();
        sm.transition(Xd8State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd8StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd8State::Running));
    }

    #[test]
    fn xd_8_sm_deserialize_invalid() {
        assert_eq!(Xd8StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_8_sm_reset() {
        let mut sm = Xd8StateMachine::new();
        sm.transition(Xd8State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd8State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_8_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd8EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd8Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_8_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd8EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd8Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd8Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_8_bus_unsubscribe() {
        let mut bus = Xd8EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_8_event_kind_and_payload() {
        let e = Xd8Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd8Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_8_bus_clear_history() {
        let mut bus = Xd8EventBus::new();
        bus.publish(Xd8Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_8_sm_step_counter_increments() {
        let mut sm = Xd8StateMachine::new();
        sm.transition(Xd8State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd8State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #6 --

    #[test]
    fn xf6_trie_insert_search() {
        let mut t = Xf6Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf6_trie_starts_with() {
        let mut t = Xf6Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf6_trie_remove() {
        let mut t = Xf6Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf6_trie_word_count() {
        let mut t = Xf6Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf6_trie_longest_prefix() {
        let mut t = Xf6Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf6_trie_all_words() {
        let mut t = Xf6Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf6_trie_autocomplete() {
        let mut t = Xf6Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf6_trie_empty_search() {
        let t = Xf6Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf6_bloom_add_contains() {
        let mut bf = Xf6BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf6_bloom_probably_absent() {
        let bf = Xf6BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf6_bloom_false_positive_rate() {
        let mut bf = Xf6BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf6_bloom_clear() {
        let mut bf = Xf6BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf6_bloom_union() {
        let mut a = Xf6BloomFilter::xf_new(512, 2);
        let mut b = Xf6BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf6_bloom_intersection_estimate() {
        let mut a = Xf6BloomFilter::xf_new(512, 2);
        let mut b = Xf6BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf6_bloom_union_size_mismatch() {
        let a = Xf6BloomFilter::xf_new(256, 2);
        let b = Xf6BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }

}
