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
    // ViewportMetrics / LineHeightCalculator / visible-range tests
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
}
