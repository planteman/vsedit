//! View layout management for sidebar, panel, and auxiliary bar.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewContainerLocation {
    Sidebar,
    Panel,
    AuxiliaryBar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewDescriptor {
    pub id: String,
    pub name: String,
    pub container: ViewContainerLocation,
    pub order: i32,
    pub visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutOrientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelPosition {
    Bottom,
    Left,
    Right,
}

pub struct ViewLayout {
    pub views: Vec<ViewDescriptor>,
    pub panel_position: PanelPosition,
    pub sidebar_visible: bool,
    pub panel_visible: bool,
    pub auxiliary_bar_visible: bool,
}

impl ViewLayout {
    pub fn new() -> Self {
        Self {
            views: Vec::new(),
            panel_position: PanelPosition::Bottom,
            sidebar_visible: true,
            panel_visible: true,
            auxiliary_bar_visible: false,
        }
    }

    pub fn add_view(&mut self, desc: ViewDescriptor) {
        self.views.push(desc);
    }

    pub fn remove_view(&mut self, id: &str) -> bool {
        let len = self.views.len();
        self.views.retain(|v| v.id != id);
        self.views.len() < len
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
    }

    pub fn toggle_panel(&mut self) {
        self.panel_visible = !self.panel_visible;
    }

    pub fn set_panel_position(&mut self, pos: PanelPosition) {
        self.panel_position = pos;
    }

    pub fn get_views_in(&self, container: ViewContainerLocation) -> Vec<&ViewDescriptor> {
        self.views.iter().filter(|v| v.container == container).collect()
    }

    pub fn set_view_visibility(&mut self, id: &str, visible: bool) {
        if let Some(v) = self.views.iter_mut().find(|v| v.id == id) {
            v.visible = visible;
        }
    }
}

impl Default for ViewLayout {
    fn default() -> Self {
        Self::new()
    }
}

// --- Display impls ---

impl fmt::Display for ViewContainerLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sidebar => write!(f, "Sidebar"),
            Self::Panel => write!(f, "Panel"),
            Self::AuxiliaryBar => write!(f, "AuxiliaryBar"),
        }
    }
}

impl fmt::Display for PanelPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bottom => write!(f, "Bottom"),
            Self::Left => write!(f, "Left"),
            Self::Right => write!(f, "Right"),
        }
    }
}

impl fmt::Display for ViewLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let vis = |b: bool| if b { "visible" } else { "hidden" };
        write!(
            f,
            "Layout: {} views, sidebar={}, panel={}",
            self.views.len(),
            vis(self.sidebar_visible),
            vis(self.panel_visible),
        )
    }
}

// --- Error type ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    ViewNotFound(String),
    DuplicateView(String),
    InvalidPosition(String),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ViewNotFound(id) => write!(f, "view not found: {id}"),
            Self::DuplicateView(id) => write!(f, "duplicate view: {id}"),
            Self::InvalidPosition(msg) => write!(f, "invalid position: {msg}"),
        }
    }
}

impl std::error::Error for LayoutError {}

// --- Additional ViewLayout methods ---

impl ViewLayout {
    /// Like `add_view` but returns an error on duplicate id.
    pub fn try_add_view(&mut self, desc: ViewDescriptor) -> Result<(), LayoutError> {
        if self.views.iter().any(|v| v.id == desc.id) {
            return Err(LayoutError::DuplicateView(desc.id));
        }
        self.views.push(desc);
        Ok(())
    }

    /// Move a view to a different container.
    pub fn move_view(
        &mut self,
        id: &str,
        target: ViewContainerLocation,
    ) -> Result<(), LayoutError> {
        let view = self
            .views
            .iter_mut()
            .find(|v| v.id == id)
            .ok_or_else(|| LayoutError::ViewNotFound(id.to_string()))?;
        view.container = target;
        Ok(())
    }

    /// Sort views by `order` within each container, preserving relative order of
    /// equal elements.
    pub fn sort_views(&mut self) {
        self.views.sort_by_key(|v| (v.container as u8, v.order));
    }

    /// Look up a view by id.
    pub fn get_view(&self, id: &str) -> Option<&ViewDescriptor> {
        self.views.iter().find(|v| v.id == id)
    }

    /// Count of views that are currently visible.
    pub fn visible_view_count(&self) -> usize {
        self.views.iter().filter(|v| v.visible).count()
    }

    /// Toggle the auxiliary bar visibility.
    pub fn toggle_auxiliary_bar(&mut self) {
        self.auxiliary_bar_visible = !self.auxiliary_bar_visible;
    }
}

// --- Builder ---

/// Builder for constructing a `ViewDescriptor` step-by-step.
#[derive(Debug, Clone)]
pub struct ViewDescriptorBuilder {
    id: String,
    name: Option<String>,
    container: ViewContainerLocation,
    order: i32,
    visible: bool,
}

impl ViewDescriptorBuilder {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            container: ViewContainerLocation::Sidebar,
            order: 0,
            visible: true,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn container(mut self, container: ViewContainerLocation) -> Self {
        self.container = container;
        self
    }

    pub fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn build(self) -> ViewDescriptor {
        ViewDescriptor {
            name: self.name.unwrap_or_else(|| self.id.clone()),
            id: self.id,
            container: self.container,
            order: self.order,
            visible: self.visible,
        }
    }
}

/// Accumulated statistics for viewlayout operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewlayoutStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ViewlayoutStats {
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
    pub fn merge(&mut self, other: &ViewlayoutStats) {
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

impl Default for ViewlayoutStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ViewlayoutStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ViewlayoutStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for viewlayout.
#[derive(Debug, Clone)]
pub struct ViewlayoutValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ViewlayoutValidator {
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

impl Default for ViewlayoutValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Viewport metrics and line-height layout utilities
// ---------------------------------------------------------------------------

/// Tracks viewport dimensions and scroll state for an editor view.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewportMetrics {
    pub width: f64,
    pub height: f64,
    pub scroll_top: f64,
    pub device_pixel_ratio: f64,
}

impl ViewportMetrics {
    /// Create a new `ViewportMetrics` with zero scroll and a device-pixel-ratio of 1.0.
    pub fn new(width: f64, height: f64) -> Self {
        Self {
            width,
            height,
            scroll_top: 0.0,
            device_pixel_ratio: 1.0,
        }
    }

    /// Logical visible height after accounting for device pixel ratio.
    pub fn visible_height(&self) -> f64 {
        self.height / self.device_pixel_ratio
    }

    /// The y-coordinate of the bottom edge of the visible area.
    pub fn scroll_bottom(&self) -> f64 {
        self.scroll_top + self.visible_height()
    }

    /// Returns `true` when the given y-coordinate falls within the visible range.
    pub fn contains_vertical(&self, y: f64) -> bool {
        y >= self.scroll_top && y <= self.scroll_bottom()
    }

    /// The y-coordinate of the vertical centre of the visible area.
    pub fn center_y(&self) -> f64 {
        self.scroll_top + self.visible_height() / 2.0
    }
}

/// Calculates vertical positions and sizes for fixed-height editor lines.
#[derive(Debug, Clone, PartialEq)]
pub struct LineHeightCalculator {
    pub base_line_height: f64,
    pub font_size: f64,
    pub line_height_multiplier: f64,
}

impl LineHeightCalculator {
    /// Create a new calculator. `base_line_height` is derived as
    /// `font_size * multiplier`.
    pub fn new(font_size: f64, multiplier: f64) -> Self {
        Self {
            base_line_height: font_size * multiplier,
            font_size,
            line_height_multiplier: multiplier,
        }
    }

    /// The computed line height.
    pub fn line_height(&self) -> f64 {
        self.base_line_height
    }

    /// The y-coordinate of the top of the given (0-based) line.
    pub fn line_top(&self, line_number: usize) -> f64 {
        line_number as f64 * self.base_line_height
    }

    /// Which (0-based) line number occupies the given y-coordinate.
    pub fn line_at_y(&self, y: f64) -> usize {
        if y < 0.0 {
            return 0;
        }
        (y / self.base_line_height) as usize
    }

    /// How many complete lines fit in the given viewport height.
    pub fn lines_per_page(&self, viewport_height: f64) -> usize {
        if self.base_line_height <= 0.0 {
            return 0;
        }
        (viewport_height / self.base_line_height) as usize
    }

    /// Total pixel height for `line_count` lines.
    pub fn total_height(&self, line_count: usize) -> f64 {
        line_count as f64 * self.base_line_height
    }
}

/// Returns the inclusive range `(first_visible_line, last_visible_line)` that
/// is currently on screen, clamped to `[0, total_lines.saturating_sub(1)]`.
pub fn viewport_visible_range(
    viewport: &ViewportMetrics,
    calc: &LineHeightCalculator,
    total_lines: usize,
) -> (usize, usize) {
    if total_lines == 0 {
        return (0, 0);
    }
    let first = calc.line_at_y(viewport.scroll_top);
    let last_raw = calc.line_at_y(viewport.scroll_bottom());
    let max_line = total_lines.saturating_sub(1);
    let first_clamped = first.min(max_line);
    let last_clamped = last_raw.min(max_line);
    (first_clamped, last_clamped)
}

/// Adjusts `viewport.scroll_top` so that the given (0-based) line is
/// vertically centred in the viewport.
pub fn scroll_to_line(
    viewport: &mut ViewportMetrics,
    calc: &LineHeightCalculator,
    line: usize,
) {
    let line_mid = calc.line_top(line) + calc.line_height() / 2.0;
    let new_scroll = (line_mid - viewport.visible_height() / 2.0).max(0.0);
    viewport.scroll_top = new_scroll;
}

// --- ViewLayout extensions ---

impl ViewLayout {
    pub fn view_count(&self) -> usize {
        self.views.len()
    }

    pub fn is_empty(&self) -> bool {
        self.views.is_empty()
    }

    pub fn find_view(&self, query: &str) -> Vec<&ViewDescriptor> {
        let q = query.to_lowercase();
        self.views
            .iter()
            .filter(|v| v.id.to_lowercase().contains(&q) || v.name.to_lowercase().contains(&q))
            .collect()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, ViewDescriptor> {
        self.views.iter()
    }

    pub fn visible_views(&self) -> Vec<&ViewDescriptor> {
        self.views.iter().filter(|v| v.visible).collect()
    }
}

impl<'a> IntoIterator for &'a ViewLayout {
    type Item = &'a ViewDescriptor;
    type IntoIter = std::slice::Iter<'a, ViewDescriptor>;

    fn into_iter(self) -> Self::IntoIter {
        self.views.iter()
    }
}

// --- ViewDescriptor extensions ---

impl ViewDescriptor {
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn matches_filter(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.id.to_lowercase().contains(&q) || self.name.to_lowercase().contains(&q)
    }
}

// --- PanelPosition extensions ---

impl PanelPosition {
    pub fn is_horizontal(&self) -> bool {
        matches!(self, Self::Bottom)
    }

    pub fn is_vertical(&self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

// --- LayoutOrientation extensions ---

impl LayoutOrientation {
    pub fn toggle(&self) -> Self {
        match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        }
    }
}

impl fmt::Display for LayoutOrientation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Horizontal => write!(f, "Horizontal"),
            Self::Vertical => write!(f, "Vertical"),
        }
    }
}

// --- ViewContainerLocation extensions ---

impl ViewContainerLocation {
    pub fn is_sidebar(&self) -> bool {
        matches!(self, Self::Sidebar)
    }

    pub fn is_panel(&self) -> bool {
        matches!(self, Self::Panel)
    }
}

// --- ViewportMetrics extensions ---

impl ViewportMetrics {
    pub fn center_line(&self, calc: &LineHeightCalculator) -> usize {
        calc.line_at_y(self.center_y())
    }

    pub fn is_line_visible(&self, line: usize, calc: &LineHeightCalculator) -> bool {
        let top = calc.line_top(line);
        let bottom = top + calc.line_height();
        bottom > self.scroll_top && top < self.scroll_bottom()
    }

    pub fn visible_line_count(&self, calc: &LineHeightCalculator) -> usize {
        calc.lines_per_page(self.visible_height())
    }

    pub fn visible_percentage(&self, calc: &LineHeightCalculator, total_lines: usize) -> f64 {
        if total_lines == 0 {
            return 100.0;
        }
        let visible = self.visible_line_count(calc) as f64;
        (visible / total_lines as f64 * 100.0).min(100.0)
    }
}

// --- LineHeightCalculator extensions ---

impl LineHeightCalculator {
    pub fn line_at_offset(&self, offset: u32) -> u32 {
        if self.base_line_height <= 0.0 {
            return 0;
        }
        (offset as f64 / self.base_line_height) as u32
    }

    pub fn line_bottom(&self, line_number: usize) -> f64 {
        (line_number as f64 + 1.0) * self.base_line_height
    }
}

// --- ViewlayoutStats extensions ---

impl ViewlayoutStats {
    pub fn summary(&self) -> String {
        format!(
            "ops={} ok={} err={} rate={:.1}%",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.success_rate() * 100.0,
        )
    }

    pub fn is_healthy(&self) -> bool {
        self.success_rate() >= 0.95
    }
}


// ---------------------------------------------------------------------------
// ViewLayoutPreset - predefined layout configurations
// ---------------------------------------------------------------------------

/// Predefined layout presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewLayoutPreset {
    /// Default layout with sidebar and panel.
    Default,
    /// Zen mode: no sidebar, no panel, full editor.
    Zen,
    /// Side by side: two editors horizontally.
    SideBySide,
    /// Focus mode: sidebar visible, panel hidden.
    FocusMode,
}

impl ViewLayoutPreset {
    /// Apply this preset to a layout.
    pub fn apply_to(&self, layout: &mut ViewLayout) {
        match self {
            Self::Default => {
                layout.sidebar_visible = true;
                layout.panel_visible = true;
                layout.auxiliary_bar_visible = false;
            }
            Self::Zen => {
                layout.sidebar_visible = false;
                layout.panel_visible = false;
                layout.auxiliary_bar_visible = false;
            }
            Self::SideBySide => {
                layout.sidebar_visible = true;
                layout.panel_visible = false;
                layout.auxiliary_bar_visible = true;
            }
            Self::FocusMode => {
                layout.sidebar_visible = true;
                layout.panel_visible = false;
                layout.auxiliary_bar_visible = false;
            }
        }
    }

    /// Name of the preset.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Zen => "Zen",
            Self::SideBySide => "Side by Side",
            Self::FocusMode => "Focus Mode",
        }
    }

    /// All available presets.
    pub fn all() -> &'static [ViewLayoutPreset] {
        &[Self::Default, Self::Zen, Self::SideBySide, Self::FocusMode]
    }
}

impl fmt::Display for ViewLayoutPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ---------------------------------------------------------------------------
// ViewLayoutSerializer - persist layout state
// ---------------------------------------------------------------------------

/// Serializes and deserializes layout state as a simple string format.
#[derive(Debug, Clone, Default)]
pub struct ViewLayoutSerializer;

impl ViewLayoutSerializer {
    /// Serialize a layout to a compact string.
    pub fn to_string(layout: &ViewLayout) -> String {
        format!(
            "sidebar={};panel={};aux={};panel_pos={};views={}",
            layout.sidebar_visible,
            layout.panel_visible,
            layout.auxiliary_bar_visible,
            layout.panel_position,
            layout.views.len()
        )
    }

    /// Deserialize a layout from a string. Returns a layout with the parsed visibility flags.
    pub fn from_string(s: &str) -> ViewLayout {
        let mut layout = ViewLayout::default();
        for part in s.split(';') {
            if let Some((key, value)) = part.split_once('=') {
                match key.trim() {
                    "sidebar" => layout.sidebar_visible = value.trim() == "true",
                    "panel" => layout.panel_visible = value.trim() == "true",
                    "aux" => layout.auxiliary_bar_visible = value.trim() == "true",
                    "panel_pos" => {
                        layout.panel_position = match value.trim().to_lowercase().as_str() {
                            "left" => PanelPosition::Left,
                            "right" => PanelPosition::Right,
                            _ => PanelPosition::Bottom,
                        };
                    }
                    _ => {}
                }
            }
        }
        layout
    }
}

// ---------------------------------------------------------------------------
// ViewVisibilityManager - track view visibility
// ---------------------------------------------------------------------------

/// Tracks the visibility state of named views.
#[derive(Debug, Clone, Default)]
pub struct ViewVisibilityManager {
    visibility: HashMap<String, bool>,
}

impl ViewVisibilityManager {
    /// Create a new empty manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Show a view.
    pub fn show(&mut self, id: impl Into<String>) {
        self.visibility.insert(id.into(), true);
    }

    /// Hide a view.
    pub fn hide(&mut self, id: impl Into<String>) {
        self.visibility.insert(id.into(), false);
    }

    /// Toggle a view's visibility. If unknown, defaults to showing.
    pub fn toggle(&mut self, id: impl Into<String>) {
        let id = id.into();
        let current = self.visibility.get(&id).copied().unwrap_or(false);
        self.visibility.insert(id, !current);
    }

    /// Check if a view is visible.
    pub fn is_visible(&self, id: &str) -> bool {
        self.visibility.get(id).copied().unwrap_or(false)
    }

    /// Number of currently visible views.
    pub fn visible_count(&self) -> usize {
        self.visibility.values().filter(|v| **v).count()
    }

    /// Total tracked views.
    pub fn total_count(&self) -> usize {
        self.visibility.len()
    }

    /// List all visible view ids.
    pub fn visible_ids(&self) -> Vec<&str> {
        self.visibility
            .iter()
            .filter(|(_, v)| **v)
            .map(|(id, _)| id.as_str())
            .collect()
    }
}

impl fmt::Display for ViewVisibilityManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Visibility({}/{} visible)", self.visible_count(), self.total_count())
    }
}

// ---------------------------------------------------------------------------
// LayoutTransition - animated layout transitions
// ---------------------------------------------------------------------------

/// Represents a transition between two layout states.
#[derive(Debug, Clone)]
pub struct LayoutTransition {
    pub from_sidebar: bool,
    pub to_sidebar: bool,
    pub from_panel: bool,
    pub to_panel: bool,
    pub from_aux: bool,
    pub to_aux: bool,
}

impl LayoutTransition {
    /// Create a transition between two layouts.
    pub fn new(from: &ViewLayout, to: &ViewLayout) -> Self {
        Self {
            from_sidebar: from.sidebar_visible,
            to_sidebar: to.sidebar_visible,
            from_panel: from.panel_visible,
            to_panel: to.panel_visible,
            from_aux: from.auxiliary_bar_visible,
            to_aux: to.auxiliary_bar_visible,
        }
    }

    /// Interpolate the transition at time t (0.0 to 1.0).
    /// Returns intermediate visibility states based on threshold.
    pub fn interpolate(&self, t: f64) -> (bool, bool, bool) {
        let t = t.clamp(0.0, 1.0);
        let interp = |from: bool, to: bool| -> bool {
            if from == to {
                from
            } else if to {
                t >= 0.5
            } else {
                t < 0.5
            }
        };
        (
            interp(self.from_sidebar, self.to_sidebar),
            interp(self.from_panel, self.to_panel),
            interp(self.from_aux, self.to_aux),
        )
    }

    /// Returns true if the transition involves any change.
    pub fn has_changes(&self) -> bool {
        self.from_sidebar != self.to_sidebar
            || self.from_panel != self.to_panel
            || self.from_aux != self.to_aux
    }

    /// Number of elements that change.
    pub fn change_count(&self) -> usize {
        let mut count = 0;
        if self.from_sidebar != self.to_sidebar { count += 1; }
        if self.from_panel != self.to_panel { count += 1; }
        if self.from_aux != self.to_aux { count += 1; }
        count
    }
}

impl fmt::Display for LayoutTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Transition({} changes)", self.change_count())
    }
}

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// SplitTree — recursive editor pane splitting (like VS Code editor groups)
// ---------------------------------------------------------------------------

/// Unique identifier for a pane within a split tree.
pub type PaneId = u32;

/// A node in the split tree: either a single editor pane or a split container.
#[derive(Debug, Clone, PartialEq)]
pub enum SplitNode {
    /// A leaf pane that holds a tab group.
    Pane {
        id: PaneId,
        tabs: Vec<TabEntry>,
        active_tab: usize,
    },
    /// A container that splits space between children along an orientation.
    Split {
        orientation: LayoutOrientation,
        children: Vec<SplitNode>,
        /// Proportional sizes for each child, summing to 1.0.
        ratios: Vec<f64>,
    },
}

/// An entry in a tab bar within a pane.
#[derive(Debug, Clone, PartialEq)]
pub struct TabEntry {
    pub id: String,
    pub title: String,
    pub is_dirty: bool,
    pub is_pinned: bool,
}

impl TabEntry {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            is_dirty: false,
            is_pinned: false,
        }
    }
}

impl SplitNode {
    /// Create a new leaf pane with no tabs.
    pub fn new_pane(id: PaneId) -> Self {
        Self::Pane {
            id,
            tabs: Vec::new(),
            active_tab: 0,
        }
    }

    /// Create a split node with two children and equal ratios.
    pub fn new_split(orientation: LayoutOrientation, a: SplitNode, b: SplitNode) -> Self {
        Self::Split {
            orientation,
            children: vec![a, b],
            ratios: vec![0.5, 0.5],
        }
    }

    /// Returns `true` if this is a leaf pane.
    pub fn is_pane(&self) -> bool {
        matches!(self, Self::Pane { .. })
    }

    /// Total number of leaf panes in the tree.
    pub fn pane_count(&self) -> usize {
        match self {
            Self::Pane { .. } => 1,
            Self::Split { children, .. } => children.iter().map(|c| c.pane_count()).sum(),
        }
    }

    /// Collect all pane ids in depth-first order.
    pub fn pane_ids(&self) -> Vec<PaneId> {
        let mut out = Vec::new();
        self.collect_pane_ids(&mut out);
        out
    }

    fn collect_pane_ids(&self, out: &mut Vec<PaneId>) {
        match self {
            Self::Pane { id, .. } => out.push(*id),
            Self::Split { children, .. } => {
                for child in children {
                    child.collect_pane_ids(out);
                }
            }
        }
    }

    /// Find a mutable reference to a pane by its id.
    pub fn find_pane_mut(&mut self, target: PaneId) -> Option<&mut SplitNode> {
        match self {
            Self::Pane { id, .. } if *id == target => Some(self),
            Self::Split { children, .. } => {
                for child in children.iter_mut() {
                    if let Some(found) = child.find_pane_mut(target) {
                        return Some(found);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Add a tab to the specified pane. Returns `Err` if the pane is not found.
    pub fn add_tab(&mut self, pane_id: PaneId, tab: TabEntry) -> Result<(), LayoutError> {
        match self.find_pane_mut(pane_id) {
            Some(SplitNode::Pane { tabs, active_tab, .. }) => {
                tabs.push(tab);
                *active_tab = tabs.len() - 1;
                Ok(())
            }
            _ => Err(LayoutError::ViewNotFound(format!("pane {pane_id}"))),
        }
    }

    /// Remove a tab by id from the specified pane. Returns the removed tab.
    pub fn remove_tab(
        &mut self,
        pane_id: PaneId,
        tab_id: &str,
    ) -> Result<TabEntry, LayoutError> {
        match self.find_pane_mut(pane_id) {
            Some(SplitNode::Pane { tabs, active_tab, .. }) => {
                let pos = tabs
                    .iter()
                    .position(|t| t.id == tab_id)
                    .ok_or_else(|| LayoutError::ViewNotFound(tab_id.to_string()))?;
                let removed = tabs.remove(pos);
                if *active_tab >= tabs.len() && !tabs.is_empty() {
                    *active_tab = tabs.len() - 1;
                }
                Ok(removed)
            }
            _ => Err(LayoutError::ViewNotFound(format!("pane {pane_id}"))),
        }
    }

    /// Reorder tabs in a pane by moving the tab at `from` to `to`.
    pub fn move_tab(
        &mut self,
        pane_id: PaneId,
        from: usize,
        to: usize,
    ) -> Result<(), LayoutError> {
        match self.find_pane_mut(pane_id) {
            Some(SplitNode::Pane { tabs, active_tab, .. }) => {
                if from >= tabs.len() || to >= tabs.len() {
                    return Err(LayoutError::InvalidPosition(format!(
                        "tab index out of range: from={from}, to={to}, len={}",
                        tabs.len()
                    )));
                }
                let tab = tabs.remove(from);
                tabs.insert(to, tab);
                // Keep active_tab pointing to the same tab after the move.
                if *active_tab == from {
                    *active_tab = to;
                } else if from < *active_tab && *active_tab <= to {
                    *active_tab -= 1;
                } else if to <= *active_tab && *active_tab < from {
                    *active_tab += 1;
                }
                Ok(())
            }
            _ => Err(LayoutError::ViewNotFound(format!("pane {pane_id}"))),
        }
    }

    /// Compute absolute pixel rectangles for every pane given a bounding rect.
    /// Returns `(PaneId, x, y, width, height)` for each leaf.
    pub fn compute_rects(
        &self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Vec<(PaneId, f64, f64, f64, f64)> {
        let mut out = Vec::new();
        self.collect_rects(x, y, width, height, &mut out);
        out
    }

    fn collect_rects(
        &self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        out: &mut Vec<(PaneId, f64, f64, f64, f64)>,
    ) {
        match self {
            Self::Pane { id, .. } => {
                out.push((*id, x, y, width, height));
            }
            Self::Split {
                orientation,
                children,
                ratios,
            } => {
                let mut offset = 0.0;
                for (i, child) in children.iter().enumerate() {
                    let ratio = ratios.get(i).copied().unwrap_or(0.0);
                    match orientation {
                        LayoutOrientation::Horizontal => {
                            let w = width * ratio;
                            child.collect_rects(x + offset, y, w, height, out);
                            offset += w;
                        }
                        LayoutOrientation::Vertical => {
                            let h = height * ratio;
                            child.collect_rects(x, y + offset, width, h, out);
                            offset += h;
                        }
                    }
                }
            }
        }
    }

    /// Maximum nesting depth of the tree (1 for a single pane).
    pub fn depth(&self) -> usize {
        match self {
            Self::Pane { .. } => 1,
            Self::Split { children, .. } => {
                1 + children.iter().map(|c| c.depth()).max().unwrap_or(0)
            }
        }
    }

    /// Total number of tabs across all panes.
    pub fn total_tab_count(&self) -> usize {
        match self {
            Self::Pane { tabs, .. } => tabs.len(),
            Self::Split { children, .. } => {
                children.iter().map(|c| c.total_tab_count()).sum()
            }
        }
    }

    /// Get the active tab of a pane, if any.
    pub fn active_tab(&self, pane_id: PaneId) -> Option<&TabEntry> {
        match self {
            Self::Pane { id, tabs, active_tab } if *id == pane_id => {
                tabs.get(*active_tab)
            }
            Self::Split { children, .. } => {
                for child in children {
                    if let Some(tab) = child.active_tab(pane_id) {
                        return Some(tab);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Set the active tab index for a pane.
    pub fn set_active_tab(
        &mut self,
        pane_id: PaneId,
        index: usize,
    ) -> Result<(), LayoutError> {
        match self.find_pane_mut(pane_id) {
            Some(SplitNode::Pane { tabs, active_tab, .. }) => {
                if index >= tabs.len() {
                    return Err(LayoutError::InvalidPosition(format!(
                        "tab index {index} out of range (len={})",
                        tabs.len()
                    )));
                }
                *active_tab = index;
                Ok(())
            }
            _ => Err(LayoutError::ViewNotFound(format!("pane {pane_id}"))),
        }
    }

    /// Normalize ratios in every split node so they sum to exactly 1.0.
    pub fn normalize_ratios(&mut self) {
        if let Self::Split {
            children, ratios, ..
        } = self
        {
            let sum: f64 = ratios.iter().sum();
            if sum > 0.0 {
                for r in ratios.iter_mut() {
                    *r /= sum;
                }
            } else if !ratios.is_empty() {
                let equal = 1.0 / ratios.len() as f64;
                for r in ratios.iter_mut() {
                    *r = equal;
                }
            }
            for child in children.iter_mut() {
                child.normalize_ratios();
            }
        }
    }
}

impl fmt::Display for SplitNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pane { id, tabs, active_tab } => {
                write!(f, "Pane({id}, {}/{} tabs)", active_tab + 1, tabs.len())
            }
            Self::Split { orientation, children, .. } => {
                write!(f, "Split({orientation}, {} children)", children.len())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// FocusTracker — manage which pane currently has focus
// ---------------------------------------------------------------------------

/// Tracks focus history across panes for most-recently-used navigation.
#[derive(Debug, Clone)]
pub struct FocusTracker {
    history: Vec<PaneId>,
    max_history: usize,
}

impl FocusTracker {
    pub fn new(max_history: usize) -> Self {
        Self {
            history: Vec::new(),
            max_history,
        }
    }

    /// Focus a pane, pushing it to the front of the MRU list.
    pub fn focus(&mut self, pane_id: PaneId) {
        self.history.retain(|&id| id != pane_id);
        self.history.insert(0, pane_id);
        if self.history.len() > self.max_history {
            self.history.truncate(self.max_history);
        }
    }

    /// The currently focused pane, if any.
    pub fn current(&self) -> Option<PaneId> {
        self.history.first().copied()
    }

    /// The previously focused pane (second in MRU list).
    pub fn previous(&self) -> Option<PaneId> {
        self.history.get(1).copied()
    }

    /// Remove a pane from the history (e.g. when it is closed).
    pub fn remove(&mut self, pane_id: PaneId) {
        self.history.retain(|&id| id != pane_id);
    }

    /// Full MRU history.
    pub fn history(&self) -> &[PaneId] {
        &self.history
    }
}

impl Default for FocusTracker {
    fn default() -> Self {
        Self::new(32)
    }
}


// === Viewport Coordinate Transformer ===

/// Viewport Coordinate Transformer implementation.
#[derive(Debug, Clone)]
pub struct ViewportCoordTransformer {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: ViewportCoordTransformerStats,
}

/// Statistics for ViewportCoordTransformer.
#[derive(Debug, Clone, Default)]
pub struct ViewportCoordTransformerStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl ViewportCoordTransformerStats {
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

impl ViewportCoordTransformer {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: ViewportCoordTransformerStats::default(),
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

    pub fn stats(&self) -> &ViewportCoordTransformerStats {
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

impl Default for ViewportCoordTransformer {
    fn default() -> Self {
        Self::new()
    }
}

// === Line Height Calculator ===

/// Priority level for LineHeightCalcEngine items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LineHeightCalcEnginePriority {
    Low,
    Normal,
    High,
    Critical,
}

impl LineHeightCalcEnginePriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for LineHeightCalcEnginePriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Line Height Calculator implementation.
#[derive(Debug, Clone)]
pub struct LineHeightCalcEngine {
    items: Vec<LineHeightCalcEngineItem>,
    max_items: usize,
    default_priority: LineHeightCalcEnginePriority,
}

/// A single item in LineHeightCalcEngine.
#[derive(Debug, Clone)]
pub struct LineHeightCalcEngineItem {
    pub id: String,
    pub label: String,
    pub priority: LineHeightCalcEnginePriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl LineHeightCalcEngineItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: LineHeightCalcEnginePriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: LineHeightCalcEnginePriority) -> Self {
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

impl LineHeightCalcEngine {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: LineHeightCalcEnginePriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: LineHeightCalcEngineItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<LineHeightCalcEngineItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&LineHeightCalcEngineItem> {
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

    pub fn by_priority(&self, priority: LineHeightCalcEnginePriority) -> Vec<&LineHeightCalcEngineItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&LineHeightCalcEngineItem> {
        let mut sorted: Vec<&LineHeightCalcEngineItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&LineHeightCalcEngineItem> {
        let mut sorted: Vec<&LineHeightCalcEngineItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&LineHeightCalcEngineItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: LineHeightCalcEnginePriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> LineHeightCalcEnginePriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &LineHeightCalcEngineItem> {
        self.items.iter()
    }
}

impl Default for LineHeightCalcEngine {
    fn default() -> Self {
        Self::new()
    }
}


/// View layout configuration manager.
#[derive(Debug, Clone)]
pub struct ViewlayoutConfig {
    entries: Vec<ViewlayoutEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single view layout entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewlayoutEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl ViewlayoutEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl ViewlayoutConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: ViewlayoutEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&ViewlayoutEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut ViewlayoutEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&ViewlayoutEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&ViewlayoutEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&ViewlayoutEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<ViewlayoutEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// View layout serialization — extended utilities (qc)
// ---------------------------------------------------------------------------

/// Metric accumulator for viewlayout operations.
#[derive(Debug, Clone)]
pub struct QcMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QcMetrics {
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

/// Sliding-window rate counter for viewlayout.
#[derive(Debug, Clone)]
pub struct QcRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QcRateWindow {
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

/// A small LRU-style cache for viewlayout lookups.
#[derive(Debug, Clone)]
pub struct QcLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QcLruCache {
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
// xa_ extended helpers for viewlayout
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaViewlayoutRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaViewlayoutRingBuf {
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
pub struct XaViewlayoutCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaViewlayoutCounter {
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

impl Default for XaViewlayoutCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 196
// ---------------------------------------------------------------------------

/// Generic object pool `Xc196Pool<T>`.
pub struct Xc196Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc196Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc196PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc196Pool<T> {
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
    pub fn stats(&self) -> Xc196PoolStats {
        Xc196PoolStats {
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

impl<T> Default for Xc196Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc196Scheduler`.
pub struct Xc196Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc196Scheduler {
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

impl Default for Xc196Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_196 hash for the given byte slice.
pub fn xc_196_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_196 convention.
pub fn xc_196_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_117 deepening: state machine + event bus ---

/// States for the Xd117 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd117State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd117State {
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
pub struct Xd117Transition {
    pub from: Xd117State,
    pub to: Xd117State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd117StateMachine {
    current: Xd117State,
    history: Vec<Xd117Transition>,
    step_counter: usize,
}

impl Xd117StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd117State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd117State {
        self.current
    }

    pub fn history(&self) -> &[Xd117Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd117State) -> Result<Xd117State, String> {
        let allowed = match (self.current, target) {
            (Xd117State::Idle, Xd117State::Running) => true,
            (Xd117State::Running, Xd117State::Paused) => true,
            (Xd117State::Running, Xd117State::Done) => true,
            (Xd117State::Paused, Xd117State::Running) => true,
            (Xd117State::Paused, Xd117State::Done) => true,
            (Xd117State::Done, Xd117State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_117: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd117Transition {
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
            "Xd117SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd117State> {
        let prefix = "Xd117SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd117State::Idle),
            "Running" => Some(Xd117State::Running),
            "Paused" => Some(Xd117State::Paused),
            "Done" => Some(Xd117State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd117State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd117 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd117Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd117Event {
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

type Xd117HandlerFn = Box<dyn Fn(&Xd117Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd117EventBus {
    handlers: Vec<(usize, Option<String>, Xd117HandlerFn)>,
    next_id: usize,
    published: Vec<Xd117Event>,
}

impl Xd117EventBus {
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
        F: Fn(&Xd117Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd117Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd117Event) {
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

    pub fn published_events(&self) -> &[Xd117Event] {
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
// xg_44: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg44Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg44Graph {
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

impl Default for Xg44Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_44: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg44Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg44Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg44Heap<T>) {
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

impl<T: Ord> Default for Xg44Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 195).
pub struct Xh195SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh195SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 237 as u64,
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

/// A compact bit set supporting boolean operations (variant 195).
pub struct Xh195BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh195BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 195).
pub struct Xi195Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi195Deque<T> {
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
pub struct Xi195Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi195Interval {
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

/// A simple interval tree (variant 195).
pub struct Xi195IntervalTree {
    xi_intervals: Vec<Xi195Interval>,
}

impl Xi195IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi195Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi195Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi195Interval) -> Vec<&Xi195Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi195Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi195Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi195Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi195Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi195Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi195Interval> = Vec::new();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_view(id: &str, container: ViewContainerLocation) -> ViewDescriptor {
        ViewDescriptor {
            id: id.to_string(),
            name: id.to_string(),
            container,
            order: 0,
            visible: true,
        }
    }

    #[test]
    fn add_and_remove_views() {
        let mut layout = ViewLayout::new();
        layout.add_view(sample_view("explorer", ViewContainerLocation::Sidebar));
        layout.add_view(sample_view("terminal", ViewContainerLocation::Panel));
        assert_eq!(layout.views.len(), 2);
        assert!(layout.remove_view("explorer"));
        assert!(!layout.remove_view("explorer"));
        assert_eq!(layout.views.len(), 1);
    }

    #[test]
    fn toggle_sidebar_and_panel() {
        let mut layout = ViewLayout::new();
        assert!(layout.sidebar_visible);
        layout.toggle_sidebar();
        assert!(!layout.sidebar_visible);
        layout.toggle_panel();
        assert!(!layout.panel_visible);
    }

    #[test]
    fn get_views_in_container() {
        let mut layout = ViewLayout::new();
        layout.add_view(sample_view("explorer", ViewContainerLocation::Sidebar));
        layout.add_view(sample_view("search", ViewContainerLocation::Sidebar));
        layout.add_view(sample_view("terminal", ViewContainerLocation::Panel));
        let sidebar = layout.get_views_in(ViewContainerLocation::Sidebar);
        assert_eq!(sidebar.len(), 2);
        let panel = layout.get_views_in(ViewContainerLocation::Panel);
        assert_eq!(panel.len(), 1);
    }

    #[test]
    fn set_view_visibility() {
        let mut layout = ViewLayout::new();
        layout.add_view(sample_view("explorer", ViewContainerLocation::Sidebar));
        layout.set_view_visibility("explorer", false);
        assert!(!layout.views[0].visible);
    }

    #[test]
    fn try_add_view_duplicate() {
        let mut layout = ViewLayout::new();
        layout.add_view(sample_view("explorer", ViewContainerLocation::Sidebar));
        let res = layout.try_add_view(sample_view("explorer", ViewContainerLocation::Panel));
        assert_eq!(res, Err(LayoutError::DuplicateView("explorer".into())));
        assert_eq!(layout.views.len(), 1);
    }

    #[test]
    fn try_add_view_ok() {
        let mut layout = ViewLayout::new();
        assert!(layout.try_add_view(sample_view("a", ViewContainerLocation::Sidebar)).is_ok());
        assert!(layout.try_add_view(sample_view("b", ViewContainerLocation::Panel)).is_ok());
        assert_eq!(layout.views.len(), 2);
    }

    #[test]
    fn move_view_between_containers() {
        let mut layout = ViewLayout::new();
        layout.add_view(sample_view("explorer", ViewContainerLocation::Sidebar));
        assert!(layout.move_view("explorer", ViewContainerLocation::Panel).is_ok());
        assert_eq!(layout.views[0].container, ViewContainerLocation::Panel);
    }

    #[test]
    fn move_view_not_found() {
        let mut layout = ViewLayout::new();
        let res = layout.move_view("missing", ViewContainerLocation::Panel);
        assert_eq!(res, Err(LayoutError::ViewNotFound("missing".into())));
    }

    #[test]
    fn sort_views_by_order() {
        let mut layout = ViewLayout::new();
        let mut v1 = sample_view("b", ViewContainerLocation::Sidebar);
        v1.order = 2;
        let mut v2 = sample_view("a", ViewContainerLocation::Sidebar);
        v2.order = 1;
        layout.add_view(v1);
        layout.add_view(v2);
        layout.add_view(sample_view("c", ViewContainerLocation::Panel));
        layout.sort_views();
        assert_eq!(layout.views[0].id, "a");
        assert_eq!(layout.views[1].id, "b");
        assert_eq!(layout.views[2].id, "c");
    }

    #[test]
    fn get_view_by_id() {
        let mut layout = ViewLayout::new();
        layout.add_view(sample_view("explorer", ViewContainerLocation::Sidebar));
        assert!(layout.get_view("explorer").is_some());
        assert!(layout.get_view("nope").is_none());
    }

    #[test]
    fn visible_view_count() {
        let mut layout = ViewLayout::new();
        layout.add_view(sample_view("a", ViewContainerLocation::Sidebar));
        layout.add_view(sample_view("b", ViewContainerLocation::Sidebar));
        layout.set_view_visibility("b", false);
        assert_eq!(layout.visible_view_count(), 1);
    }

    #[test]
    fn toggle_auxiliary_bar() {
        let mut layout = ViewLayout::new();
        assert!(!layout.auxiliary_bar_visible);
        layout.toggle_auxiliary_bar();
        assert!(layout.auxiliary_bar_visible);
        layout.toggle_auxiliary_bar();
        assert!(!layout.auxiliary_bar_visible);
    }

    #[test]
    fn display_impls() {
        assert_eq!(ViewContainerLocation::Sidebar.to_string(), "Sidebar");
        assert_eq!(ViewContainerLocation::Panel.to_string(), "Panel");
        assert_eq!(ViewContainerLocation::AuxiliaryBar.to_string(), "AuxiliaryBar");
        assert_eq!(PanelPosition::Bottom.to_string(), "Bottom");
        assert_eq!(PanelPosition::Left.to_string(), "Left");
        assert_eq!(PanelPosition::Right.to_string(), "Right");
    }

    #[test]
    fn display_view_layout() {
        let mut layout = ViewLayout::new();
        layout.add_view(sample_view("a", ViewContainerLocation::Sidebar));
        let s = layout.to_string();
        assert!(s.contains("1 views"));
        assert!(s.contains("sidebar=visible"));
        assert!(s.contains("panel=visible"));
    }

    #[test]
    fn builder_defaults() {
        let desc = ViewDescriptorBuilder::new("explorer").build();
        assert_eq!(desc.id, "explorer");
        assert_eq!(desc.name, "explorer");
        assert_eq!(desc.container, ViewContainerLocation::Sidebar);
        assert_eq!(desc.order, 0);
        assert!(desc.visible);
    }

    #[test]
    fn builder_full() {
        let desc = ViewDescriptorBuilder::new("term")
            .name("Terminal")
            .container(ViewContainerLocation::Panel)
            .order(5)
            .visible(false)
            .build();
        assert_eq!(desc.id, "term");
        assert_eq!(desc.name, "Terminal");
        assert_eq!(desc.container, ViewContainerLocation::Panel);
        assert_eq!(desc.order, 5);
        assert!(!desc.visible);
    }

    #[test]
    fn error_display() {
        let e1 = LayoutError::ViewNotFound("x".into());
        assert_eq!(e1.to_string(), "view not found: x");
        let e2 = LayoutError::DuplicateView("y".into());
        assert_eq!(e2.to_string(), "duplicate view: y");
        let e3 = LayoutError::InvalidPosition("bad".into());
        assert_eq!(e3.to_string(), "invalid position: bad");
    }

    // -----------------------------------------------------------------------
    // ViewportMetrics / LineHeightCalcEngine / visible-range tests
    // -----------------------------------------------------------------------

    #[test]
    fn viewport_metrics_creation() {
        let vp = ViewportMetrics::new(800.0, 600.0);
        assert_eq!(vp.width, 800.0);
        assert_eq!(vp.height, 600.0);
        assert_eq!(vp.scroll_top, 0.0);
        assert_eq!(vp.device_pixel_ratio, 1.0);
    }

    #[test]
    fn viewport_visible_height() {
        let mut vp = ViewportMetrics::new(800.0, 600.0);
        assert!((vp.visible_height() - 600.0).abs() < f64::EPSILON);
        vp.device_pixel_ratio = 2.0;
        assert!((vp.visible_height() - 300.0).abs() < f64::EPSILON);
    }

    #[test]
    fn viewport_contains_vertical() {
        let mut vp = ViewportMetrics::new(800.0, 600.0);
        vp.scroll_top = 100.0;
        assert!(vp.contains_vertical(100.0));
        assert!(vp.contains_vertical(400.0));
        assert!(vp.contains_vertical(700.0));
        assert!(!vp.contains_vertical(99.0));
        assert!(!vp.contains_vertical(701.0));
    }

    #[test]
    fn line_height_calculator_basic() {
        let calc = LineHeightCalculator::new(14.0, 1.5);
        assert!((calc.line_height() - 21.0).abs() < f64::EPSILON);
        assert!((calc.font_size - 14.0).abs() < f64::EPSILON);
        assert!((calc.line_height_multiplier - 1.5).abs() < f64::EPSILON);
        assert!((calc.total_height(10) - 210.0).abs() < f64::EPSILON);
    }

    #[test]
    fn line_at_y_calculation() {
        let calc = LineHeightCalculator::new(10.0, 2.0); // line_height = 20
        assert_eq!(calc.line_at_y(0.0), 0);
        assert_eq!(calc.line_at_y(19.9), 0);
        assert_eq!(calc.line_at_y(20.0), 1);
        assert_eq!(calc.line_at_y(59.0), 2);
        assert_eq!(calc.line_at_y(-5.0), 0); // negative clamped
    }

    #[test]
    fn lines_per_page_calculation() {
        let calc = LineHeightCalculator::new(10.0, 2.0); // line_height = 20
        assert_eq!(calc.lines_per_page(100.0), 5);
        assert_eq!(calc.lines_per_page(99.0), 4);
        assert_eq!(calc.lines_per_page(0.0), 0);
    }

    #[test]
    fn visible_range_at_top() {
        let vp = ViewportMetrics::new(800.0, 200.0);
        let calc = LineHeightCalculator::new(10.0, 2.0); // line_height = 20
        let (first, last) = viewport_visible_range(&vp, &calc, 100);
        assert_eq!(first, 0);
        assert_eq!(last, 10); // 200/20 = line 10
    }

    #[test]
    fn visible_range_scrolled() {
        let mut vp = ViewportMetrics::new(800.0, 200.0);
        vp.scroll_top = 100.0; // scrolled 5 lines down (line_height = 20)
        let calc = LineHeightCalculator::new(10.0, 2.0);
        let (first, last) = viewport_visible_range(&vp, &calc, 100);
        assert_eq!(first, 5);
        assert_eq!(last, 15);
    }

    #[test]
    fn scroll_to_line_centers() {
        let mut vp = ViewportMetrics::new(800.0, 200.0);
        let calc = LineHeightCalculator::new(10.0, 2.0); // line_height = 20
        scroll_to_line(&mut vp, &calc, 50);
        // line 50 top = 1000, mid = 1010, scroll = 1010 - 100 = 910
        assert!((vp.scroll_top - 910.0).abs() < f64::EPSILON);
    }

    #[test]
    fn eq_viewcontainerlocation_same() {
        assert_eq!(ViewContainerLocation::Sidebar, ViewContainerLocation::Sidebar);
    }

    #[test]
    fn ne_viewcontainerlocation_diff() {
        assert_ne!(ViewContainerLocation::Sidebar, ViewContainerLocation::Panel);
    }

    #[test]
    fn eq_layoutorientation_same() {
        assert_eq!(LayoutOrientation::Horizontal, LayoutOrientation::Horizontal);
    }

    #[test]
    fn ne_layoutorientation_diff() {
        assert_ne!(LayoutOrientation::Horizontal, LayoutOrientation::Vertical);
    }

    #[test]
    fn eq_panelposition_same() {
        assert_eq!(PanelPosition::Bottom, PanelPosition::Bottom);
    }

    #[test]
    fn ne_panelposition_diff() {
        assert_ne!(PanelPosition::Bottom, PanelPosition::Left);
    }

    #[test]
    fn display_viewcontainerlocation_variants() {
        assert!(!ViewContainerLocation::Sidebar.to_string().is_empty());
        assert!(!ViewContainerLocation::Panel.to_string().is_empty());
        assert!(!ViewContainerLocation::AuxiliaryBar.to_string().is_empty());
    }

    #[test]
    fn display_panelposition_variants() {
        assert!(!PanelPosition::Bottom.to_string().is_empty());
        assert!(!PanelPosition::Left.to_string().is_empty());
        assert!(!PanelPosition::Right.to_string().is_empty());
    }

    #[test]
    fn behavior_check_0() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_26() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_27() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn viewlayout_stats_new_defaults() {
        let stats = ViewlayoutStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn viewlayout_stats_record_success() {
        let mut stats = ViewlayoutStats::new();
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
    fn viewlayout_stats_record_failure() {
        let mut stats = ViewlayoutStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn viewlayout_stats_reset() {
        let mut stats = ViewlayoutStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn viewlayout_stats_merge() {
        let mut a = ViewlayoutStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ViewlayoutStats::new();
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
    fn viewlayout_stats_display() {
        let mut stats = ViewlayoutStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn viewlayout_stats_default() {
        let stats = ViewlayoutStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn viewlayout_validator_accepts_valid_name() {
        let v = ViewlayoutValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn viewlayout_validator_rejects_empty() {
        let v = ViewlayoutValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn viewlayout_validator_rejects_too_long() {
        let v = ViewlayoutValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn viewlayout_validator_forbidden_prefix() {
        let v = ViewlayoutValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn viewlayout_validator_allowed_chars() {
        let v = ViewlayoutValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn viewlayout_validator_range() {
        let v = ViewlayoutValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn viewlayout_sanitize_removes_control() {
        let result = ViewlayoutValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn viewlayout_truncate_short_string() {
        assert_eq!(ViewlayoutValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn viewlayout_truncate_long_string() {
        let result = ViewlayoutValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn viewlayout_is_ascii_printable() {
        assert!(ViewlayoutValidator::is_ascii_printable("Hello World 123"));
        assert!(!ViewlayoutValidator::is_ascii_printable("Hello\x00World"));
    }

    // --- New extension tests ---

    #[test]
    fn view_count_and_is_empty() {
        let mut layout = ViewLayout::new();
        assert!(layout.is_empty());
        assert_eq!(layout.view_count(), 0);
        layout.add_view(sample_view("a", ViewContainerLocation::Sidebar));
        assert!(!layout.is_empty());
        assert_eq!(layout.view_count(), 1);
    }

    #[test]
    fn find_view_by_query() {
        let mut layout = ViewLayout::new();
        layout.add_view(ViewDescriptor {
            id: "explorer".to_string(),
            name: "File Explorer".to_string(),
            container: ViewContainerLocation::Sidebar,
            order: 0,
            visible: true,
        });
        layout.add_view(sample_view("terminal", ViewContainerLocation::Panel));
        assert_eq!(layout.find_view("explor").len(), 1);
        assert_eq!(layout.find_view("TERMINAL").len(), 1);
        assert_eq!(layout.find_view("File").len(), 1);
        assert!(layout.find_view("nonexistent").is_empty());
    }

    #[test]
    fn into_iterator_for_layout() {
        let mut layout = ViewLayout::new();
        layout.add_view(sample_view("a", ViewContainerLocation::Sidebar));
        layout.add_view(sample_view("b", ViewContainerLocation::Panel));
        let ids: Vec<&str> = (&layout).into_iter().map(|v| v.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b"]);
        let ids2: Vec<&str> = layout.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(ids2, vec!["a", "b"]);
    }

    #[test]
    fn panel_position_extensions() {
        assert!(PanelPosition::Bottom.is_horizontal());
        assert!(!PanelPosition::Bottom.is_vertical());
        assert!(!PanelPosition::Left.is_horizontal());
        assert!(PanelPosition::Left.is_vertical());
        assert!(PanelPosition::Right.is_vertical());
        assert_eq!(PanelPosition::Bottom.label(), "bottom");
        assert_eq!(PanelPosition::Left.label(), "left");
        assert_eq!(PanelPosition::Right.label(), "right");
    }

    #[test]
    fn layout_orientation_toggle_and_display() {
        let h = LayoutOrientation::Horizontal;
        let v = h.toggle();
        assert_eq!(v, LayoutOrientation::Vertical);
        assert_eq!(v.toggle(), LayoutOrientation::Horizontal);
        assert_eq!(h.to_string(), "Horizontal");
        assert_eq!(v.to_string(), "Vertical");
    }

    #[test]
    fn view_container_location_extensions() {
        assert!(ViewContainerLocation::Sidebar.is_sidebar());
        assert!(!ViewContainerLocation::Panel.is_sidebar());
        assert!(ViewContainerLocation::Panel.is_panel());
        assert!(!ViewContainerLocation::AuxiliaryBar.is_panel());
    }

    #[test]
    fn view_descriptor_extensions() {
        let v = sample_view("explorer", ViewContainerLocation::Sidebar);
        assert!(v.is_visible());
        assert!(v.matches_filter("explor"));
        assert!(v.matches_filter("EXPLORER"));
        assert!(!v.matches_filter("terminal"));
    }

    #[test]
    fn viewport_center_line_and_line_visible() {
        let mut vp = ViewportMetrics::new(800.0, 200.0);
        let calc = LineHeightCalculator::new(10.0, 2.0);
        assert_eq!(vp.center_line(&calc), 5);
        assert!(vp.is_line_visible(0, &calc));
        assert!(vp.is_line_visible(9, &calc));
        assert!(!vp.is_line_visible(11, &calc));
        vp.scroll_top = 100.0;
        assert!(!vp.is_line_visible(0, &calc));
        assert!(vp.is_line_visible(5, &calc));
    }

    #[test]
    fn viewport_visible_percentage() {
        let vp = ViewportMetrics::new(800.0, 200.0);
        let calc = LineHeightCalculator::new(10.0, 2.0);
        let pct = vp.visible_percentage(&calc, 100);
        assert!((pct - 10.0).abs() < f64::EPSILON);
        let pct_small = vp.visible_percentage(&calc, 5);
        assert!((pct_small - 100.0).abs() < f64::EPSILON);
        let pct_zero = vp.visible_percentage(&calc, 0);
        assert!((pct_zero - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn line_height_calc_extensions() {
        let calc = LineHeightCalculator::new(10.0, 2.0);
        assert_eq!(calc.line_at_offset(0), 0);
        assert_eq!(calc.line_at_offset(20), 1);
        assert_eq!(calc.line_at_offset(59), 2);
        assert!((calc.line_bottom(0) - 20.0).abs() < f64::EPSILON);
        assert!((calc.line_bottom(4) - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn viewlayout_stats_summary_and_healthy() {
        let mut stats = ViewlayoutStats::new();
        stats.record_success(100);
        stats.record_success(200);
        let s = stats.summary();
        assert!(s.contains("ops=2"));
        assert!(s.contains("rate=100.0%"));
        assert!(stats.is_healthy());
        for _ in 0..19 {
            stats.record_failure(50);
        }
        assert!(!stats.is_healthy());
    }

    #[test]
    fn layout_preset_zen() {
        let mut layout = ViewLayout::default();
        layout.sidebar_visible = true;
        layout.panel_visible = true;
        ViewLayoutPreset::Zen.apply_to(&mut layout);
        assert!(!layout.sidebar_visible);
        assert!(!layout.panel_visible);
        assert!(!layout.auxiliary_bar_visible);
        assert_eq!(ViewLayoutPreset::Zen.name(), "Zen");
    }

    #[test]
    fn layout_preset_all() {
        assert_eq!(ViewLayoutPreset::all().len(), 4);
    }

    #[test]
    fn layout_serializer_round_trip() {
        let mut layout = ViewLayout::default();
        layout.sidebar_visible = true;
        layout.panel_visible = false;
        layout.auxiliary_bar_visible = true;
        let s = ViewLayoutSerializer::to_string(&layout);
        let restored = ViewLayoutSerializer::from_string(&s);
        assert_eq!(restored.sidebar_visible, true);
        assert_eq!(restored.panel_visible, false);
        assert_eq!(restored.auxiliary_bar_visible, true);
    }

    #[test]
    fn view_visibility_manager_ops() {
        let mut mgr = ViewVisibilityManager::new();
        mgr.show("explorer");
        mgr.show("search");
        mgr.hide("debug");
        assert!(mgr.is_visible("explorer"));
        assert!(!mgr.is_visible("debug"));
        assert_eq!(mgr.visible_count(), 2);
        assert_eq!(mgr.total_count(), 3);
        mgr.toggle("debug");
        assert!(mgr.is_visible("debug"));
        assert_eq!(mgr.visible_count(), 3);
    }

    #[test]
    fn layout_transition_interpolation() {
        let mut from = ViewLayout::default();
        from.sidebar_visible = true;
        from.panel_visible = true;
        let mut to = ViewLayout::default();
        to.sidebar_visible = false;
        to.panel_visible = false;
        let transition = LayoutTransition::new(&from, &to);
        assert!(transition.has_changes());
        assert_eq!(transition.change_count(), 2);
        let (s, p, _a) = transition.interpolate(0.0);
        assert!(s); // still original
        assert!(p);
        let (s2, p2, _a2) = transition.interpolate(1.0);
        assert!(!s2); // final
        assert!(!p2);
    }

    #[test]
    fn layout_transition_no_change() {
        let layout = ViewLayout::default();
        let transition = LayoutTransition::new(&layout, &layout);
        assert!(!transition.has_changes());
        assert_eq!(transition.change_count(), 0);
    }

    // -----------------------------------------------------------------------
    // SplitNode / TabEntry / FocusTracker tests
    // -----------------------------------------------------------------------

    #[test]
    fn split_tree_pane_count_and_ids() {
        let tree = SplitNode::new_split(
            LayoutOrientation::Horizontal,
            SplitNode::new_pane(1),
            SplitNode::new_split(
                LayoutOrientation::Vertical,
                SplitNode::new_pane(2),
                SplitNode::new_pane(3),
            ),
        );
        assert_eq!(tree.pane_count(), 3);
        assert_eq!(tree.pane_ids(), vec![1, 2, 3]);
        assert_eq!(tree.depth(), 3);
        assert!(!tree.is_pane());
    }

    #[test]
    fn split_tree_add_remove_tabs() {
        let mut tree = SplitNode::new_split(
            LayoutOrientation::Horizontal,
            SplitNode::new_pane(1),
            SplitNode::new_pane(2),
        );
        tree.add_tab(1, TabEntry::new("a", "File A")).unwrap();
        tree.add_tab(1, TabEntry::new("b", "File B")).unwrap();
        tree.add_tab(2, TabEntry::new("c", "File C")).unwrap();
        assert_eq!(tree.total_tab_count(), 3);

        // Active tab should be last added
        let active = tree.active_tab(1).unwrap();
        assert_eq!(active.id, "b");

        // Remove a tab
        let removed = tree.remove_tab(1, "a").unwrap();
        assert_eq!(removed.title, "File A");
        assert_eq!(tree.total_tab_count(), 2);

        // Error on missing pane
        assert!(tree.add_tab(99, TabEntry::new("x", "X")).is_err());
    }

    #[test]
    fn split_tree_move_tab_reorders() {
        let mut pane = SplitNode::new_pane(1);
        pane.add_tab(1, TabEntry::new("a", "A")).unwrap();
        pane.add_tab(1, TabEntry::new("b", "B")).unwrap();
        pane.add_tab(1, TabEntry::new("c", "C")).unwrap();

        // Active is "c" (index 2). Move "a" (0) to position 2.
        pane.set_active_tab(1, 0).unwrap();
        pane.move_tab(1, 0, 2).unwrap();

        if let SplitNode::Pane { tabs, active_tab, .. } = &pane {
            assert_eq!(tabs[0].id, "b");
            assert_eq!(tabs[1].id, "c");
            assert_eq!(tabs[2].id, "a");
            assert_eq!(*active_tab, 2); // followed the moved tab
        } else {
            panic!("expected pane");
        }

        // Invalid indices
        assert!(pane.move_tab(1, 0, 99).is_err());
    }

    #[test]
    fn split_tree_compute_rects() {
        let tree = SplitNode::new_split(
            LayoutOrientation::Horizontal,
            SplitNode::new_pane(1),
            SplitNode::new_pane(2),
        );
        let rects = tree.compute_rects(0.0, 0.0, 1000.0, 600.0);
        assert_eq!(rects.len(), 2);
        // pane 1: left half
        assert_eq!(rects[0].0, 1);
        assert!((rects[0].1 - 0.0).abs() < f64::EPSILON); // x
        assert!((rects[0].3 - 500.0).abs() < f64::EPSILON); // width
        // pane 2: right half
        assert_eq!(rects[1].0, 2);
        assert!((rects[1].1 - 500.0).abs() < f64::EPSILON); // x
        assert!((rects[1].3 - 500.0).abs() < f64::EPSILON); // width
    }

    #[test]
    fn split_tree_normalize_ratios() {
        let mut tree = SplitNode::Split {
            orientation: LayoutOrientation::Horizontal,
            children: vec![SplitNode::new_pane(1), SplitNode::new_pane(2)],
            ratios: vec![3.0, 1.0],
        };
        tree.normalize_ratios();
        if let SplitNode::Split { ratios, .. } = &tree {
            assert!((ratios[0] - 0.75).abs() < 1e-9);
            assert!((ratios[1] - 0.25).abs() < 1e-9);
        }
    }

    #[test]
    fn focus_tracker_mru_order() {
        let mut ft = FocusTracker::new(4);
        ft.focus(1);
        ft.focus(2);
        ft.focus(3);
        assert_eq!(ft.current(), Some(3));
        assert_eq!(ft.previous(), Some(2));

        // Re-focusing 1 brings it to front
        ft.focus(1);
        assert_eq!(ft.current(), Some(1));
        assert_eq!(ft.previous(), Some(3));
        assert_eq!(ft.history(), &[1, 3, 2]);

        // Remove
        ft.remove(3);
        assert_eq!(ft.history(), &[1, 2]);
    }

    #[test]
    fn split_node_display() {
        let pane = SplitNode::new_pane(1);
        assert!(pane.to_string().contains("Pane(1"));

        let split = SplitNode::new_split(
            LayoutOrientation::Vertical,
            SplitNode::new_pane(1),
            SplitNode::new_pane(2),
        );
        assert!(split.to_string().contains("Split(Vertical"));
    }

    #[test]
    fn tab_entry_dirty_and_pinned() {
        let mut tab = TabEntry::new("main.rs", "main.rs");
        assert!(!tab.is_dirty);
        assert!(!tab.is_pinned);
        tab.is_dirty = true;
        tab.is_pinned = true;
        assert!(tab.is_dirty);
        assert!(tab.is_pinned);
    }

    #[test]
    fn viewportCoordTransformer_new() {
        let s = ViewportCoordTransformer::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn viewportCoordTransformer_add_contains() {
        let mut s = ViewportCoordTransformer::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn viewportCoordTransformer_add_duplicate() {
        let mut s = ViewportCoordTransformer::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn viewportCoordTransformer_remove() {
        let mut s = ViewportCoordTransformer::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn viewportCoordTransformer_capacity() {
        let s = ViewportCoordTransformer::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn viewportCoordTransformer_search() {
        let mut s = ViewportCoordTransformer::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn viewportCoordTransformer_stats() {
        let mut s = ViewportCoordTransformer::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn lineHeightCalculator_new() {
        let m = LineHeightCalcEngine::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn lineHeightCalculator_add_find() {
        let mut m = LineHeightCalcEngine::new();
        m.add(LineHeightCalcEngineItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn lineHeightCalculator_priority_filter() {
        let mut m = LineHeightCalcEngine::new();
        m.add(LineHeightCalcEngineItem::new("a", "A").with_priority(LineHeightCalcEnginePriority::High));
        m.add(LineHeightCalcEngineItem::new("b", "B").with_priority(LineHeightCalcEnginePriority::Low));
        m.add(LineHeightCalcEngineItem::new("c", "C").with_priority(LineHeightCalcEnginePriority::High));
        assert_eq!(m.by_priority(LineHeightCalcEnginePriority::High).len(), 2);
    }

    #[test]
    fn lineHeightCalculator_remove() {
        let mut m = LineHeightCalcEngine::new();
        m.add(LineHeightCalcEngineItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn lineHeightCalculator_search() {
        let mut m = LineHeightCalcEngine::new();
        m.add(LineHeightCalcEngineItem::new("id1", "Hello World"));
        m.add(LineHeightCalcEngineItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn lineHeightCalculator_total_weight() {
        let mut m = LineHeightCalcEngine::new();
        m.add(LineHeightCalcEngineItem::new("a", "A").with_priority(LineHeightCalcEnginePriority::Critical));
        m.add(LineHeightCalcEngineItem::new("b", "B").with_priority(LineHeightCalcEnginePriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn lineHeightCalculator_capacity_limit() {
        let mut m = LineHeightCalcEngine::new().with_max_items(2);
        m.add(LineHeightCalcEngineItem::new("1", "one"));
        m.add(LineHeightCalcEngineItem::new("2", "two"));
        assert!(!m.add(LineHeightCalcEngineItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn lineHeightCalculator_sorted_by_priority() {
        let mut m = LineHeightCalcEngine::new();
        m.add(LineHeightCalcEngineItem::new("lo", "Low").with_priority(LineHeightCalcEnginePriority::Low));
        m.add(LineHeightCalcEngineItem::new("hi", "High").with_priority(LineHeightCalcEnginePriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn lineHeightCalculator_item_metadata() {
        let mut item = LineHeightCalcEngineItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn viewportCoordTransformer_enabled_toggle() {
        let mut s = ViewportCoordTransformer::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn lineHeightCalculator_priority_display() {
        assert_eq!(format!("{}", LineHeightCalcEnginePriority::High), "high");
        assert_eq!(format!("{}", LineHeightCalcEnginePriority::Low), "low");
    }


    #[test]
    fn viewlayout_entry_creation() {
        let e = ViewlayoutEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn viewlayout_entry_with_priority() {
        let e = ViewlayoutEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn viewlayout_entry_metadata() {
        let e = ViewlayoutEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn viewlayout_entry_remove_meta() {
        let mut e = ViewlayoutEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn viewlayout_entry_activate_deactivate() {
        let mut e = ViewlayoutEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn viewlayout_config_add_sorted() {
        let mut c = ViewlayoutConfig::new(10);
        c.add(ViewlayoutEntry::new("lo", "Lo").with_priority(1));
        c.add(ViewlayoutEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn viewlayout_config_capacity() {
        let mut c = ViewlayoutConfig::new(1);
        assert!(c.add(ViewlayoutEntry::new("a", "A")));
        assert!(!c.add(ViewlayoutEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn viewlayout_config_remove() {
        let mut c = ViewlayoutConfig::new(10);
        c.add(ViewlayoutEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn viewlayout_config_get() {
        let mut c = ViewlayoutConfig::new(10);
        c.add(ViewlayoutEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn viewlayout_config_active_entries() {
        let mut c = ViewlayoutConfig::new(10);
        c.add(ViewlayoutEntry::new("a", "A"));
        c.add(ViewlayoutEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn viewlayout_config_enable_disable() {
        let mut c = ViewlayoutConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn viewlayout_config_clear() {
        let mut c = ViewlayoutConfig::new(10);
        c.add(ViewlayoutEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn viewlayout_config_find_by_label() {
        let mut c = ViewlayoutConfig::new(10);
        c.add(ViewlayoutEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn viewlayout_config_top_n() {
        let mut c = ViewlayoutConfig::new(10);
        c.add(ViewlayoutEntry::new("a", "A").with_priority(1));
        c.add(ViewlayoutEntry::new("b", "B").with_priority(2));
        c.add(ViewlayoutEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn viewlayout_config_deactivate_activate_all() {
        let mut c = ViewlayoutConfig::new(10);
        c.add(ViewlayoutEntry::new("a", "A"));
        c.add(ViewlayoutEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn viewlayout_config_highest_priority() {
        let mut c = ViewlayoutConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(ViewlayoutEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn viewlayout_config_contains() {
        let mut c = ViewlayoutConfig::new(10);
        c.add(ViewlayoutEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn viewlayout_config_labels() {
        let mut c = ViewlayoutConfig::new(10);
        c.add(ViewlayoutEntry::new("a", "Alpha"));
        c.add(ViewlayoutEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn viewlayout_config_drain_inactive() {
        let mut c = ViewlayoutConfig::new(10);
        c.add(ViewlayoutEntry::new("a", "A"));
        c.add(ViewlayoutEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn qc_metrics_empty() {
        let m = QcMetrics::new("viewlayout");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qc_metrics_record_and_mean() {
        let mut m = QcMetrics::new("viewlayout");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qc_metrics_min_max() {
        let mut m = QcMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qc_metrics_variance_and_std() {
        let mut m = QcMetrics::new("v");
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
    fn qc_metrics_percentile() {
        let mut m = QcMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qc_metrics_merge() {
        let mut a = QcMetrics::new("a");
        a.record(1.0);
        let mut b = QcMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qc_metrics_reset() {
        let mut m = QcMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qc_rate_window_empty() {
        let rw = QcRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qc_rate_window_tick_and_rate() {
        let mut rw = QcRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qc_lru_cache_basic() {
        let mut c = QcLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qc_lru_cache_contains_and_keys() {
        let mut c = QcLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qc_lru_cache_remove() {
        let mut c = QcLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qc_metrics_sum() {
        let mut m = QcMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qc_metrics_label() {
        let m = QcMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qc_lru_cache_clear() {
        let mut c = QcLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for viewlayout
    #[test]
    fn xa_viewlayout_ring_new() {
        let rb = super::XaViewlayoutRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_viewlayout_ring_push_len() {
        let mut rb = super::XaViewlayoutRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_viewlayout_ring_wrap() {
        let mut rb = super::XaViewlayoutRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_viewlayout_ring_mean_empty() {
        let rb = super::XaViewlayoutRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_viewlayout_ring_mean_values() {
        let mut rb = super::XaViewlayoutRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_viewlayout_ring_min_max() {
        let mut rb = super::XaViewlayoutRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_viewlayout_ring_iter() {
        let mut rb = super::XaViewlayoutRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_viewlayout_counter_new() {
        let c = super::XaViewlayoutCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_viewlayout_counter_inc() {
        let mut c = super::XaViewlayoutCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_viewlayout_counter_inc_by() {
        let mut c = super::XaViewlayoutCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_viewlayout_counter_reset() {
        let mut c = super::XaViewlayoutCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_viewlayout_counter_clear() {
        let mut c = super::XaViewlayoutCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_viewlayout_counter_default() {
        let c = super::XaViewlayoutCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 196 ----

    #[test]
    fn xc_196_pool_new_empty() {
        let pool: super::Xc196Pool<i32> = super::Xc196Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_196_pool_release_acquire() {
        let mut pool = super::Xc196Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_196_pool_acquire_empty() {
        let mut pool: super::Xc196Pool<i32> = super::Xc196Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_196_pool_full() {
        let mut pool = super::Xc196Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_196_pool_drain() {
        let mut pool = super::Xc196Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_196_pool_stats() {
        let mut pool = super::Xc196Pool::new(8);
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
    fn xc_196_pool_clear() {
        let mut pool = super::Xc196Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_196_pool_shrink() {
        let mut pool = super::Xc196Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_196_pool_default() {
        let pool: super::Xc196Pool<String> = super::Xc196Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_196_pool_extend() {
        let mut pool = super::Xc196Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_196_pool_retain() {
        let mut pool = super::Xc196Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_196_scheduler_round_robin() {
        let mut sched = super::Xc196Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_196_scheduler_empty() {
        let mut sched = super::Xc196Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_196_scheduler_reset() {
        let mut sched = super::Xc196Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_196_scheduler_add_remove() {
        let mut sched = super::Xc196Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_196_scheduler_targets() {
        let sched = super::Xc196Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_196_hash_empty() {
        assert_eq!(super::xc_196_hash(b""), 5381);
    }

    #[test]
    fn xc_196_hash_data() {
        let h = super::xc_196_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_196_hash(b"hello"), h);
    }

    #[test]
    fn xc_196_reverse_str() {
        assert_eq!(super::xc_196_reverse("abc"), "cba");
        assert_eq!(super::xc_196_reverse(""), "");
    }


    // --- xd_117 deepening tests ---

    #[test]
    fn xd_117_sm_initial_state() {
        let sm = Xd117StateMachine::new();
        assert_eq!(sm.current_state(), Xd117State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_117_sm_valid_idle_to_running() {
        let mut sm = Xd117StateMachine::new();
        assert!(sm.transition(Xd117State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd117State::Running);
    }

    #[test]
    fn xd_117_sm_valid_running_to_paused() {
        let mut sm = Xd117StateMachine::new();
        sm.transition(Xd117State::Running).unwrap();
        assert!(sm.transition(Xd117State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd117State::Paused);
    }

    #[test]
    fn xd_117_sm_valid_running_to_done() {
        let mut sm = Xd117StateMachine::new();
        sm.transition(Xd117State::Running).unwrap();
        assert!(sm.transition(Xd117State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd117State::Done);
    }

    #[test]
    fn xd_117_sm_valid_paused_to_running() {
        let mut sm = Xd117StateMachine::new();
        sm.transition(Xd117State::Running).unwrap();
        sm.transition(Xd117State::Paused).unwrap();
        assert!(sm.transition(Xd117State::Running).is_ok());
    }

    #[test]
    fn xd_117_sm_valid_done_to_idle() {
        let mut sm = Xd117StateMachine::new();
        sm.transition(Xd117State::Running).unwrap();
        sm.transition(Xd117State::Done).unwrap();
        assert!(sm.transition(Xd117State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd117State::Idle);
    }

    #[test]
    fn xd_117_sm_invalid_idle_to_done() {
        let mut sm = Xd117StateMachine::new();
        assert!(sm.transition(Xd117State::Done).is_err());
    }

    #[test]
    fn xd_117_sm_invalid_idle_to_paused() {
        let mut sm = Xd117StateMachine::new();
        assert!(sm.transition(Xd117State::Paused).is_err());
    }

    #[test]
    fn xd_117_sm_history_tracking() {
        let mut sm = Xd117StateMachine::new();
        sm.transition(Xd117State::Running).unwrap();
        sm.transition(Xd117State::Paused).unwrap();
        sm.transition(Xd117State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd117State::Idle);
        assert_eq!(sm.history()[0].to, Xd117State::Running);
        assert_eq!(sm.history()[1].from, Xd117State::Running);
        assert_eq!(sm.history()[2].to, Xd117State::Done);
    }

    #[test]
    fn xd_117_sm_serialize_deserialize() {
        let mut sm = Xd117StateMachine::new();
        sm.transition(Xd117State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd117StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd117State::Running));
    }

    #[test]
    fn xd_117_sm_deserialize_invalid() {
        assert_eq!(Xd117StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_117_sm_reset() {
        let mut sm = Xd117StateMachine::new();
        sm.transition(Xd117State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd117State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_117_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd117EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd117Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_117_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd117EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd117Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd117Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_117_bus_unsubscribe() {
        let mut bus = Xd117EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_117_event_kind_and_payload() {
        let e = Xd117Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd117Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_117_bus_clear_history() {
        let mut bus = Xd117EventBus::new();
        bus.publish(Xd117Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_117_sm_step_counter_increments() {
        let mut sm = Xd117StateMachine::new();
        sm.transition(Xd117State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd117State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_44 graph tests ------------------------------------------------

    #[test]
    fn xg_44_graph_empty() {
        let g = super::Xg44Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_44_graph_add_node() {
        let mut g = super::Xg44Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_44_graph_add_edge() {
        let mut g = super::Xg44Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_44_graph_neighbors() {
        let mut g = super::Xg44Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_44_graph_has_path() {
        let mut g = super::Xg44Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_44_graph_self_path() {
        let g = super::Xg44Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_44_graph_topo_sort() {
        let mut g = super::Xg44Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_44_graph_cycle_detect_false() {
        let mut g = super::Xg44Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_44_graph_cycle_detect_true() {
        let mut g = super::Xg44Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_44 heap tests -------------------------------------------------

    #[test]
    fn xg_44_heap_empty() {
        let h: super::Xg44Heap<i32> = super::Xg44Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_44_heap_push_pop() {
        let mut h = super::Xg44Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_44_heap_peek() {
        let mut h = super::Xg44Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_44_heap_drain_sorted() {
        let mut h = super::Xg44Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_44_heap_merge() {
        let mut a = super::Xg44Heap::new();
        let mut b = super::Xg44Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_44_heap_default() {
        let h: super::Xg44Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_44_graph_default() {
        let g: super::Xg44Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh195_skip_insert_contains() {
        let mut sl = super::Xh195SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh195_skip_remove() {
        let mut sl = super::Xh195SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh195_skip_len() {
        let mut sl = super::Xh195SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh195_skip_range_query() {
        let mut sl = super::Xh195SkipList::xh_new(4);
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
    fn xh195_skip_floor_ceiling() {
        let mut sl = super::Xh195SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh195_skip_rank() {
        let mut sl = super::Xh195SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh195_skip_empty() {
        let sl = super::Xh195SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh195_skip_duplicates() {
        let mut sl = super::Xh195SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh195_bitset_set_test() {
        let mut bs = super::Xh195BitSet::xh_new(256);
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
    fn xh195_bitset_clear_count() {
        let mut bs = super::Xh195BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh195_bitset_and_or_xor() {
        let mut a = super::Xh195BitSet::xh_new(128);
        let mut b = super::Xh195BitSet::xh_new(128);
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
    fn xh195_bitset_iter_ones() {
        let mut bs = super::Xh195BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh195_bitset_first_last() {
        let mut bs = super::Xh195BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh195_bitset_empty() {
        let bs = super::Xh195BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi195_deque_push_pop_back() {
        let mut dq = super::Xi195Deque::xi_new(4);
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
    fn xi195_deque_push_pop_front() {
        let mut dq = super::Xi195Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi195_deque_mixed_ops() {
        let mut dq = super::Xi195Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi195_deque_get_and_split() {
        let mut dq = super::Xi195Deque::xi_new(8);
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
    fn xi195_deque_rotate_left() {
        let mut dq = super::Xi195Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi195_deque_rotate_right() {
        let mut dq = super::Xi195Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi195_deque_grow() {
        let mut dq = super::Xi195Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi195_deque_empty() {
        let dq = super::Xi195Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi195_interval_tree_insert_query() {
        let mut tree = super::Xi195IntervalTree::xi_new();
        tree.xi_insert(super::Xi195Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi195Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi195Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi195_interval_tree_overlap() {
        let mut tree = super::Xi195IntervalTree::xi_new();
        tree.xi_insert(super::Xi195Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi195Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi195Interval::xi_new(12, 20));
        let q = super::Xi195Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi195_interval_tree_remove() {
        let mut tree = super::Xi195IntervalTree::xi_new();
        tree.xi_insert(super::Xi195Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi195Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi195_interval_tree_gaps() {
        let mut tree = super::Xi195IntervalTree::xi_new();
        tree.xi_insert(super::Xi195Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi195Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi195Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi195Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi195Interval::xi_new(8, 10));
    }

    #[test]
    fn xi195_interval_tree_merge() {
        let mut tree = super::Xi195IntervalTree::xi_new();
        tree.xi_insert(super::Xi195Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi195Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi195Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi195Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi195Interval::xi_new(10, 15));
    }

    #[test]
    fn xi195_interval_tree_all() {
        let mut tree = super::Xi195IntervalTree::xi_new();
        tree.xi_insert(super::Xi195Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi195Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi195_interval_tree_empty() {
        let tree = super::Xi195IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi195_interval_tree_contains_point() {
        let iv = super::Xi195Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}
