//! Editor view parts: view zones, overlay widgets, content widgets, glyph margins,
//! minimap configuration, and breadcrumb navigation.

use std::collections::HashMap;
use std::fmt;

/// Line number rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineNumberMode {
    Absolute,
    Relative,
    Interval(u32),
}

/// Format a line number for display.
pub fn format_line_number(line: u32, current_line: u32, mode: LineNumberMode) -> String {
    match mode {
        LineNumberMode::Absolute => format!("{line}"),
        LineNumberMode::Relative => {
            if line == current_line {
                format!("{line}")
            } else {
                format!("{}", (line as i64 - current_line as i64).unsigned_abs())
            }
        }
        LineNumberMode::Interval(n) => {
            if line == current_line || line % n == 0 {
                format!("{line}")
            } else {
                String::new()
            }
        }
    }
}

/// Ruler position.
pub struct Ruler {
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewZone {
    pub id: u64,
    pub after_line: u32,
    pub height_in_lines: u32,
    pub content: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayWidget {
    pub id: String,
    pub position_top: u32,
    pub position_left: u32,
    pub content: String,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentWidget {
    pub id: String,
    pub line: u32,
    pub column: u32,
    pub content: String,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphMarginWidget {
    pub line: u32,
    pub glyph: String,
    pub tooltip: Option<String>,
}

/// Reference to a widget at a specific line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WidgetRef<'a> {
    Zone(&'a ViewZone),
    Overlay(&'a OverlayWidget),
    Content(&'a ContentWidget),
    GlyphMargin(&'a GlyphMarginWidget),
}

/// Side of the editor where the minimap is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinimapSide {
    Left,
    Right,
}

/// Minimap configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct Minimap {
    pub enabled: bool,
    pub side: MinimapSide,
    pub scale: f64,
    pub max_column: u32,
}

impl Default for Minimap {
    fn default() -> Self {
        Self {
            enabled: true,
            side: MinimapSide::Right,
            scale: 1.0,
            max_column: 120,
        }
    }
}

/// Kind of breadcrumb item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreadcrumbKind {
    File,
    Module,
    Class,
    Function,
    Variable,
}

/// A single breadcrumb navigation item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbItem {
    pub label: String,
    pub kind: BreadcrumbKind,
    pub uri: String,
}

/// Breadcrumb navigation bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbBar {
    pub items: Vec<BreadcrumbItem>,
}

impl BreadcrumbBar {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn push(&mut self, item: BreadcrumbItem) {
        self.items.push(item);
    }

    pub fn pop(&mut self) -> Option<BreadcrumbItem> {
        self.items.pop()
    }

    /// Returns a slash-separated path string of all breadcrumb labels.
    pub fn get_path_string(&self) -> String {
        self.items
            .iter()
            .map(|i| i.label.as_str())
            .collect::<Vec<_>>()
            .join(" / ")
    }
}

impl Default for BreadcrumbBar {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EditorViewParts {
    pub zones: Vec<ViewZone>,
    pub overlays: Vec<OverlayWidget>,
    pub content_widgets: Vec<ContentWidget>,
    pub glyph_margins: Vec<GlyphMarginWidget>,
    next_zone_id: u64,
}

impl EditorViewParts {
    pub fn new() -> Self {
        Self {
            zones: Vec::new(),
            overlays: Vec::new(),
            content_widgets: Vec::new(),
            glyph_margins: Vec::new(),
            next_zone_id: 1,
        }
    }

    pub fn add_view_zone(&mut self, after_line: u32, height_in_lines: u32) -> u64 {
        let id = self.next_zone_id;
        self.next_zone_id += 1;
        self.zones.push(ViewZone {
            id,
            after_line,
            height_in_lines,
            content: None,
        });
        id
    }

    pub fn remove_view_zone(&mut self, id: u64) {
        self.zones.retain(|z| z.id != id);
    }

    /// Update the height of an existing view zone. Returns `true` if found.
    pub fn update_view_zone(&mut self, id: u64, height_in_lines: u32) -> bool {
        if let Some(zone) = self.zones.iter_mut().find(|z| z.id == id) {
            zone.height_in_lines = height_in_lines;
            true
        } else {
            false
        }
    }

    pub fn add_overlay(&mut self, widget: OverlayWidget) {
        self.overlays.push(widget);
    }

    pub fn remove_overlay(&mut self, id: &str) {
        self.overlays.retain(|o| o.id != id);
    }

    /// Set the visibility of an overlay widget. Returns `true` if found.
    pub fn set_overlay_visible(&mut self, id: &str, visible: bool) -> bool {
        if let Some(o) = self.overlays.iter_mut().find(|o| o.id == id) {
            o.visible = visible;
            true
        } else {
            false
        }
    }

    /// Returns all currently visible overlay widgets.
    pub fn get_visible_overlays(&self) -> Vec<&OverlayWidget> {
        self.overlays.iter().filter(|o| o.visible).collect()
    }

    pub fn add_content_widget(&mut self, widget: ContentWidget) {
        self.content_widgets.push(widget);
    }

    pub fn remove_content_widget(&mut self, id: &str) {
        self.content_widgets.retain(|w| w.id != id);
    }

    /// Set the visibility of a content widget. Returns `true` if found.
    pub fn set_content_widget_visible(&mut self, id: &str, visible: bool) -> bool {
        if let Some(w) = self.content_widgets.iter_mut().find(|w| w.id == id) {
            w.visible = visible;
            true
        } else {
            false
        }
    }

    pub fn add_glyph_margin(&mut self, widget: GlyphMarginWidget) {
        self.glyph_margins.push(widget);
    }

    pub fn get_view_zones(&self) -> &[ViewZone] {
        &self.zones
    }

    /// Returns references to all widgets associated with the given line.
    pub fn get_widgets_at_line(&self, line: u32) -> Vec<WidgetRef<'_>> {
        let mut result = Vec::new();
        for z in &self.zones {
            if z.after_line == line {
                result.push(WidgetRef::Zone(z));
            }
        }
        for o in &self.overlays {
            if o.position_top == line {
                result.push(WidgetRef::Overlay(o));
            }
        }
        for c in &self.content_widgets {
            if c.line == line {
                result.push(WidgetRef::Content(c));
            }
        }
        for g in &self.glyph_margins {
            if g.line == line {
                result.push(WidgetRef::GlyphMargin(g));
            }
        }
        result
    }

    /// Total number of widgets across all categories.
    pub fn total_widget_count(&self) -> usize {
        self.zones.len()
            + self.overlays.len()
            + self.content_widgets.len()
            + self.glyph_margins.len()
    }

    /// Remove all widgets from every category.
    pub fn clear_all(&mut self) {
        self.zones.clear();
        self.overlays.clear();
        self.content_widgets.clear();
        self.glyph_margins.clear();
    }
}

impl Default for EditorViewParts {
    fn default() -> Self {
        Self::new()
    }
}

/// Priority level for ordering viewpart rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ViewPartPriority {
    /// Rendered first (e.g. glyph margins, line numbers).
    High,
    /// Default rendering order (e.g. content widgets).
    Normal,
    /// Rendered last (e.g. decorative overlays).
    Low,
}

/// A tagged viewpart entry used for priority-based sorting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrioritizedViewPart {
    pub name: String,
    pub priority: ViewPartPriority,
    pub visible: bool,
    pub render_time_us: u64,
}

/// Aggregate metrics computed from a collection of viewparts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewPartMetrics {
    pub total_parts: usize,
    pub visible_count: usize,
    pub hidden_count: usize,
    pub total_render_time_us: u64,
}

/// Compute aggregate metrics from a slice of prioritized viewparts.
pub fn compute_viewpart_metrics(parts: &[PrioritizedViewPart]) -> ViewPartMetrics {
    let visible_count = parts.iter().filter(|p| p.visible).count();
    let total_render_time_us = parts.iter().map(|p| p.render_time_us).sum();
    ViewPartMetrics {
        total_parts: parts.len(),
        visible_count,
        hidden_count: parts.len() - visible_count,
        total_render_time_us,
    }
}

/// Sort viewparts by priority (High before Normal before Low), preserving
/// insertion order among equal priorities (stable sort).
pub fn sort_viewparts_by_priority(parts: &mut [PrioritizedViewPart]) {
    parts.sort_by(|a, b| a.priority.cmp(&b.priority));
}

// ---------------------------------------------------------------------------
// View zone overlap detection
// ---------------------------------------------------------------------------

/// A pair of overlapping view zone IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewZoneOverlap {
    pub zone_a: u64,
    pub zone_b: u64,
}

/// Detect overlapping view zones.
///
/// Two zones overlap if they occupy the same line range. A zone starting at
/// `after_line` occupies lines `after_line+1 ..= after_line+height_in_lines`.
pub fn detect_view_zone_overlaps(zones: &[ViewZone]) -> Vec<ViewZoneOverlap> {
    let mut overlaps = Vec::new();
    for i in 0..zones.len() {
        let a_start = zones[i].after_line + 1;
        let a_end = zones[i].after_line + zones[i].height_in_lines;
        for j in (i + 1)..zones.len() {
            let b_start = zones[j].after_line + 1;
            let b_end = zones[j].after_line + zones[j].height_in_lines;
            if a_start <= b_end && b_start <= a_end {
                overlaps.push(ViewZoneOverlap {
                    zone_a: zones[i].id,
                    zone_b: zones[j].id,
                });
            }
        }
    }
    overlaps
}

// ---------------------------------------------------------------------------
// Widget layout computation
// ---------------------------------------------------------------------------

/// Computed layout for a widget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetLayout {
    pub top: u32,
    pub left: u32,
    pub width: u32,
    pub height: u32,
}

/// Compute the bounding layout for an overlay widget given editor dimensions.
pub fn compute_overlay_layout(
    widget: &OverlayWidget,
    editor_width: u32,
    editor_height: u32,
) -> WidgetLayout {
    let content_width = widget.content.len() as u32;
    let width = content_width.min(editor_width.saturating_sub(widget.position_left));
    let height = 1_u32.min(editor_height.saturating_sub(widget.position_top));
    WidgetLayout {
        top: widget.position_top,
        left: widget.position_left,
        width,
        height,
    }
}

// ---------------------------------------------------------------------------
// Breadcrumb path resolution
// ---------------------------------------------------------------------------

/// Resolve the breadcrumb trail for a given file path by splitting segments.
pub fn resolve_breadcrumb_path(file_path: &str) -> BreadcrumbBar {
    let mut bar = BreadcrumbBar::new();
    let segments: Vec<&str> = file_path.split('/').filter(|s| !s.is_empty()).collect();
    for (i, seg) in segments.iter().enumerate() {
        let kind = if i == segments.len() - 1 {
            BreadcrumbKind::File
        } else {
            BreadcrumbKind::Module
        };
        bar.push(BreadcrumbItem {
            label: seg.to_string(),
            kind,
            uri: format!("file:///{}", segments[..=i].join("/")),
        });
    }
    bar
}

// ---------------------------------------------------------------------------
// Glyph margin configuration
// ---------------------------------------------------------------------------

/// Configuration for the glyph margin column.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphMarginConfig {
    pub enabled: bool,
    pub width_chars: u32,
    pub decorations_enabled: bool,
}

impl Default for GlyphMarginConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            width_chars: 2,
            decorations_enabled: true,
        }
    }
}

impl GlyphMarginConfig {
    /// Returns the effective pixel-equivalent width (chars × char_width).
    pub fn effective_width(&self, char_width: u32) -> u32 {
        if self.enabled {
            self.width_chars * char_width
        } else {
            0
        }
    }
}

// ---------------------------------------------------------------------------
// Gutter decorations and indent guides
// ---------------------------------------------------------------------------

/// Type of gutter decoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GutterDecorationType {
    Breakpoint,
    ConditionalBreakpoint,
    Logpoint,
    Bookmark,
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for GutterDecorationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GutterDecorationType::Breakpoint => write!(f, "breakpoint"),
            GutterDecorationType::ConditionalBreakpoint => write!(f, "conditional-breakpoint"),
            GutterDecorationType::Logpoint => write!(f, "logpoint"),
            GutterDecorationType::Bookmark => write!(f, "bookmark"),
            GutterDecorationType::Error => write!(f, "error"),
            GutterDecorationType::Warning => write!(f, "warning"),
            GutterDecorationType::Info => write!(f, "info"),
        }
    }
}

/// A decoration in the editor gutter (left margin).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GutterDecoration {
    pub line: u32,
    pub decoration_type: GutterDecorationType,
    pub tooltip: Option<String>,
    pub enabled: bool,
}

impl GutterDecoration {
    pub fn new(line: u32, decoration_type: GutterDecorationType) -> Self {
        Self {
            line,
            decoration_type,
            tooltip: None,
            enabled: true,
        }
    }

    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Get the glyph character for this decoration type.
    pub fn glyph(&self) -> &'static str {
        match self.decoration_type {
            GutterDecorationType::Breakpoint => "●",
            GutterDecorationType::ConditionalBreakpoint => "◆",
            GutterDecorationType::Logpoint => "◇",
            GutterDecorationType::Bookmark => "★",
            GutterDecorationType::Error => "✗",
            GutterDecorationType::Warning => "⚠",
            GutterDecorationType::Info => "ℹ",
        }
    }
}

impl std::fmt::Display for GutterDecoration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} L{}: {}", self.glyph(), self.line, self.decoration_type)
    }
}

/// Manages a collection of gutter decorations.
#[derive(Debug, Clone, Default)]
pub struct GutterDecorationManager {
    decorations: Vec<GutterDecoration>,
}

impl GutterDecorationManager {
    pub fn new() -> Self {
        Self { decorations: Vec::new() }
    }

    pub fn add(&mut self, decoration: GutterDecoration) {
        self.decorations.push(decoration);
    }

    pub fn remove_at_line(&mut self, line: u32) {
        self.decorations.retain(|d| d.line != line);
    }

    pub fn remove_by_type(&mut self, line: u32, dtype: GutterDecorationType) {
        self.decorations.retain(|d| !(d.line == line && d.decoration_type == dtype));
    }

    pub fn get_at_line(&self, line: u32) -> Vec<&GutterDecoration> {
        self.decorations.iter().filter(|d| d.line == line).collect()
    }

    pub fn toggle_breakpoint(&mut self, line: u32) {
        let has_bp = self.decorations.iter().any(|d| d.line == line && d.decoration_type == GutterDecorationType::Breakpoint);
        if has_bp {
            self.remove_by_type(line, GutterDecorationType::Breakpoint);
        } else {
            self.add(GutterDecoration::new(line, GutterDecorationType::Breakpoint));
        }
    }

    pub fn count(&self) -> usize {
        self.decorations.len()
    }

    pub fn count_by_type(&self, dtype: GutterDecorationType) -> usize {
        self.decorations.iter().filter(|d| d.decoration_type == dtype).count()
    }

    /// Get all decoration lines sorted.
    pub fn decorated_lines(&self) -> Vec<u32> {
        let mut lines: Vec<u32> = self.decorations.iter().map(|d| d.line).collect();
        lines.sort();
        lines.dedup();
        lines
    }
}

/// An indent guide for rendering vertical indent lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentGuide {
    pub line: u32,
    pub indent_level: u32,
    pub is_active: bool,
}

/// Compute indent guides for a range of lines.
///
/// `indentation_levels` is a slice where each element is the indentation level
/// (number of indent units) for the corresponding line. `active_line` is the
/// currently focused line (0-based index into the slice).
pub fn compute_indent_guides(
    indentation_levels: &[u32],
    active_line: usize,
    start_line: u32,
) -> Vec<IndentGuide> {
    let active_indent = indentation_levels.get(active_line).copied().unwrap_or(0);
    let mut guides = Vec::new();

    for (i, &level) in indentation_levels.iter().enumerate() {
        for indent in 1..=level {
            guides.push(IndentGuide {
                line: start_line + i as u32,
                indent_level: indent,
                is_active: indent <= active_indent && i == active_line,
            });
        }
    }

    guides
}

/// Compute indentation levels from line content.
///
/// Returns the number of `tab_size`-column indent units for each line.
pub fn compute_indentation_levels(lines: &[&str], tab_size: u32) -> Vec<u32> {
    lines
        .iter()
        .map(|line| {
            let mut col: u32 = 0;
            for ch in line.chars() {
                match ch {
                    ' ' => col += 1,
                    '\t' => col += tab_size - (col % tab_size),
                    _ => break,
                }
            }
            col / tab_size
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Additional helpers
// ---------------------------------------------------------------------------

impl EditorViewParts {
    /// Returns the number of view zones.
    pub fn view_zone_count(&self) -> usize {
        self.zones.len()
    }

    /// Returns the number of overlay widgets.
    pub fn overlay_count(&self) -> usize {
        self.overlays.len()
    }

    /// Returns the number of content widgets.
    pub fn content_widget_count(&self) -> usize {
        self.content_widgets.len()
    }
}

impl std::fmt::Display for EditorViewParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "EditorViewParts(zones: {}, overlays: {}, content: {}, glyphs: {})",
            self.zones.len(),
            self.overlays.len(),
            self.content_widgets.len(),
            self.glyph_margins.len(),
        )
    }
}

impl Minimap {
    /// Builder: set the minimap side.
    pub fn with_side(mut self, side: MinimapSide) -> Self {
        self.side = side;
        self
    }

    /// Builder: set the minimap scale.
    pub fn with_scale(mut self, scale: f64) -> Self {
        self.scale = scale;
        self
    }
}

impl std::fmt::Display for Minimap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = if self.enabled { "enabled" } else { "disabled" };
        let side = match self.side {
            MinimapSide::Left => "left",
            MinimapSide::Right => "right",
        };
        write!(f, "Minimap({state}, {side}, scale={:.1}, max_col={})", self.scale, self.max_column)
    }
}

impl ViewZone {
    /// Compute the pixel height of this zone given a line height in pixels.
    pub fn height_pixels(&self, line_height: u32) -> u32 {
        self.height_in_lines * line_height
    }
}

impl GutterDecoration {
    /// Returns `true` if this decoration is a breakpoint (regular or conditional).
    pub fn is_breakpoint(&self) -> bool {
        matches!(
            self.decoration_type,
            GutterDecorationType::Breakpoint | GutterDecorationType::ConditionalBreakpoint
        )
    }
}

impl GutterDecorationType {
    /// Returns `true` if this type represents a diagnostic (Error, Warning, or Info).
    pub fn is_diagnostic(&self) -> bool {
        matches!(self, Self::Error | Self::Warning | Self::Info)
    }
}

// ---------------------------------------------------------------------------
// Gutter width computation
// ---------------------------------------------------------------------------

/// Compute the width in characters needed to display line numbers up to `max_line`.
pub fn line_number_width(max_line: u32) -> u32 {
    if max_line == 0 {
        return 1;
    }
    let mut digits = 0u32;
    let mut n = max_line;
    while n > 0 {
        digits += 1;
        n /= 10;
    }
    digits
}

/// Compute the total gutter width in characters, combining line-number width
/// and glyph-margin width.
pub fn compute_gutter_width(max_line: u32, glyph_margin: &GlyphMarginConfig) -> u32 {
    let ln_width = line_number_width(max_line);
    let glyph_width = if glyph_margin.enabled { glyph_margin.width_chars } else { 0 };
    ln_width + glyph_width
}

// ---------------------------------------------------------------------------
// Minimap scaling helpers
// ---------------------------------------------------------------------------

impl Minimap {
    /// Compute the scaled column width for the minimap.
    pub fn scaled_max_column(&self) -> u32 {
        (self.max_column as f64 * self.scale).round() as u32
    }

    /// Compute how many source lines fit in a minimap of the given pixel height,
    /// using the provided line height (in pixels) and scale factor.
    pub fn visible_lines(&self, viewport_height_px: u32, line_height_px: u32) -> u32 {
        if !self.enabled || line_height_px == 0 {
            return 0;
        }
        let scaled_line = (line_height_px as f64 * self.scale).max(1.0);
        (viewport_height_px as f64 / scaled_line).floor() as u32
    }

    /// Map a document line number to a minimap y-pixel offset.
    pub fn line_to_y(&self, line: u32, first_visible_line: u32, line_height_px: u32) -> u32 {
        let relative = line.saturating_sub(first_visible_line);
        let scaled_line = (line_height_px as f64 * self.scale).max(1.0);
        (relative as f64 * scaled_line).round() as u32
    }
}

// ---------------------------------------------------------------------------
// View part visibility toggles
// ---------------------------------------------------------------------------

/// Tracks the visibility state of standard editor view parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewPartVisibility {
    pub line_numbers: bool,
    pub glyph_margin: bool,
    pub minimap: bool,
    pub breadcrumbs: bool,
    pub indent_guides: bool,
}

impl Default for ViewPartVisibility {
    fn default() -> Self {
        Self {
            line_numbers: true,
            glyph_margin: true,
            minimap: true,
            breadcrumbs: true,
            indent_guides: true,
        }
    }
}

impl ViewPartVisibility {
    /// Create a new visibility state with all parts visible.
    pub fn all_visible() -> Self {
        Self::default()
    }

    /// Create a visibility state with all parts hidden.
    pub fn all_hidden() -> Self {
        Self {
            line_numbers: false,
            glyph_margin: false,
            minimap: false,
            breadcrumbs: false,
            indent_guides: false,
        }
    }

    /// Count how many parts are currently visible.
    pub fn visible_count(&self) -> usize {
        [self.line_numbers, self.glyph_margin, self.minimap, self.breadcrumbs, self.indent_guides]
            .iter()
            .filter(|&&v| v)
            .count()
    }

    /// Toggle a specific part by name. Returns `true` if the name was recognized.
    pub fn toggle(&mut self, name: &str) -> bool {
        match name {
            "line_numbers" => { self.line_numbers = !self.line_numbers; true }
            "glyph_margin" => { self.glyph_margin = !self.glyph_margin; true }
            "minimap" => { self.minimap = !self.minimap; true }
            "breadcrumbs" => { self.breadcrumbs = !self.breadcrumbs; true }
            "indent_guides" => { self.indent_guides = !self.indent_guides; true }
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Layout calculation for view part columns
// ---------------------------------------------------------------------------

/// Describes the horizontal layout of the editor's left-side columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorColumnLayout {
    pub glyph_margin_width: u32,
    pub line_number_width: u32,
    pub content_left: u32,
}

/// Compute the horizontal column layout given editor configuration.
pub fn compute_column_layout(
    max_line: u32,
    glyph_config: &GlyphMarginConfig,
    char_width: u32,
    visibility: &ViewPartVisibility,
) -> EditorColumnLayout {
    let glyph_w = if visibility.glyph_margin {
        glyph_config.effective_width(char_width)
    } else {
        0
    };
    let ln_w = if visibility.line_numbers {
        line_number_width(max_line) * char_width
    } else {
        0
    };
    EditorColumnLayout {
        glyph_margin_width: glyph_w,
        line_number_width: ln_w,
        content_left: glyph_w + ln_w,
    }
}

/// Collect all view zone IDs from an EditorViewParts instance.
pub fn collect_view_zone_ids(parts: &EditorViewParts) -> Vec<u64> {
    parts.get_view_zones().iter().map(|z| z.id).collect()
}

/// Compute the total extra height (in lines) contributed by all view zones.
pub fn total_view_zone_height(parts: &EditorViewParts) -> u32 {
    parts.get_view_zones().iter().map(|z| z.height_in_lines).sum()
}

/// Return all overlay widget IDs.
pub fn overlay_widget_ids(parts: &EditorViewParts) -> Vec<String> {
    parts.get_visible_overlays().iter().map(|o| o.id.clone()).collect()
}

/// Find a content widget by its ID, returning a reference if found.
pub fn find_content_widget<'a>(parts: &'a EditorViewParts, id: &str) -> Option<&'a ContentWidget> {
    parts.content_widgets.iter().find(|w| w.id == id)
}

/// Count glyph margin widgets on a specific line.
pub fn glyph_count_on_line(parts: &EditorViewParts, line: u32) -> usize {
    parts.glyph_margins.iter().filter(|g| g.line == line).count()
}

/// Return the set of unique lines that have any view zone after them.
pub fn view_zone_lines(parts: &EditorViewParts) -> Vec<u32> {
    let mut lines: Vec<u32> = parts
        .get_view_zones()
        .iter()
        .map(|z| z.after_line)
        .collect();
    lines.sort();
    lines.dedup();
    lines
}

/// Determine if a line has any widget (zone, overlay, content, or glyph margin).
pub fn line_has_widget(parts: &EditorViewParts, line: u32) -> bool {
    !parts.get_widgets_at_line(line).is_empty()
}

/// Compute the maximum view zone height across all zones.
pub fn max_view_zone_height(parts: &EditorViewParts) -> u32 {
    parts
        .get_view_zones()
        .iter()
        .map(|z| z.height_in_lines)
        .max()
        .unwrap_or(0)
}

/// Return the number of lines that the given line number mode would render
/// as non-empty for lines 1..=total_lines with current_line.
pub fn visible_line_number_count(total_lines: u32, current_line: u32, mode: LineNumberMode) -> u32 {
    (1..=total_lines)
        .filter(|&line| !format_line_number(line, current_line, mode).is_empty())
        .count() as u32
}

// ---------------------------------------------------------------------------
// View-part analysis utilities
// ---------------------------------------------------------------------------

/// Compute aggregate metrics across a slice of prioritized view parts.
pub fn compute_view_part_metrics(parts: &[PrioritizedViewPart]) -> ViewPartMetrics {
    let total_parts = parts.len();
    let visible_count = parts.iter().filter(|p| p.visible).count();
    let hidden_count = total_parts - visible_count;
    let total_render_time_us = parts.iter().map(|p| p.render_time_us).sum();
    ViewPartMetrics {
        total_parts,
        visible_count,
        hidden_count,
        total_render_time_us,
    }
}

/// Return only the view parts that are currently visible, sorted by
/// priority (High first, then Normal, then Low).
pub fn visible_parts_sorted(parts: &[PrioritizedViewPart]) -> Vec<&PrioritizedViewPart> {
    let mut visible: Vec<&PrioritizedViewPart> = parts.iter().filter(|p| p.visible).collect();
    visible.sort_by_key(|p| match p.priority {
        ViewPartPriority::High => 0u8,
        ViewPartPriority::Normal => 1,
        ViewPartPriority::Low => 2,
    });
    visible
}

/// Check whether two `WidgetLayout` rectangles overlap.
pub fn layouts_overlap(a: &WidgetLayout, b: &WidgetLayout) -> bool {
    let a_right = a.left + a.width;
    let a_bottom = a.top + a.height;
    let b_right = b.left + b.width;
    let b_bottom = b.top + b.height;
    a.left < b_right && b.left < a_right && a.top < b_bottom && b.top < a_bottom
}

/// Compute the bounding box that contains all the given widget layouts.
pub fn bounding_box(layouts: &[WidgetLayout]) -> Option<WidgetLayout> {
    if layouts.is_empty() {
        return None;
    }
    let min_top = layouts.iter().map(|l| l.top).min().unwrap();
    let min_left = layouts.iter().map(|l| l.left).min().unwrap();
    let max_bottom = layouts.iter().map(|l| l.top + l.height).max().unwrap();
    let max_right = layouts.iter().map(|l| l.left + l.width).max().unwrap();
    Some(WidgetLayout {
        top: min_top,
        left: min_left,
        width: max_right - min_left,
        height: max_bottom - min_top,
    })
}

/// Build a `BreadcrumbBar` from a `/`-separated path string.
pub fn breadcrumbs_from_path(path: &str) -> BreadcrumbBar {
    let items: Vec<BreadcrumbItem> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .enumerate()
        .map(|(i, segment)| {
            let kind = if i == 0 {
                BreadcrumbKind::Module
            } else {
                BreadcrumbKind::File
            };
            BreadcrumbItem {
                label: segment.to_string(),
                kind,
                uri: String::new(),
            }
        })
        .collect();
    BreadcrumbBar { items }
}

/// Calculate total gutter width from glyph margin config and line-number width.
pub fn total_gutter_width(glyph: &GlyphMarginConfig, line_number_chars: u32) -> u32 {
    let glyph_width = if glyph.enabled { glyph.width_chars } else { 0 };
    glyph_width + line_number_chars
}

// -- ViewPartOverlap detection -----------------------------------------------

/// Represents a decoration with a line range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecorationRange {
    pub id: String,
    pub start_line: u32,
    pub end_line: u32,
    pub priority: i32,
}

/// Detected overlap between two decorations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewPartOverlap {
    pub first_id: String,
    pub second_id: String,
    pub overlap_start: u32,
    pub overlap_end: u32,
}

/// Find all overlapping decoration pairs.
pub fn detect_overlaps(ranges: &[DecorationRange]) -> Vec<ViewPartOverlap> {
    let mut overlaps = Vec::new();
    for i in 0..ranges.len() {
        for j in (i + 1)..ranges.len() {
            let a = &ranges[i];
            let b = &ranges[j];
            let start = a.start_line.max(b.start_line);
            let end = a.end_line.min(b.end_line);
            if start <= end {
                overlaps.push(ViewPartOverlap {
                    first_id: a.id.clone(),
                    second_id: b.id.clone(),
                    overlap_start: start,
                    overlap_end: end,
                });
            }
        }
    }
    overlaps
}

// -- ViewPartGutterIcons with priority ordering ------------------------------

/// A gutter icon with a priority for ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GutterIcon {
    pub line: u32,
    pub icon_id: String,
    pub priority: i32,
    pub tooltip: Option<String>,
}

/// Sort gutter icons by line, then by priority (descending).
pub fn sort_gutter_icons(icons: &mut [GutterIcon]) {
    icons.sort_by(|a, b| a.line.cmp(&b.line).then(b.priority.cmp(&a.priority)));
}

/// Return the highest priority icon per line.
pub fn top_icons_per_line(icons: &[GutterIcon]) -> Vec<&GutterIcon> {
    let mut sorted: Vec<&GutterIcon> = icons.iter().collect();
    sorted.sort_by(|a, b| a.line.cmp(&b.line).then(b.priority.cmp(&a.priority)));
    let mut result: Vec<&GutterIcon> = Vec::new();
    let mut last_line = None;
    for icon in sorted {
        if last_line != Some(icon.line) {
            result.push(icon);
            last_line = Some(icon.line);
        }
    }
    result
}

// -- ViewPartContentWidget positioning ---------------------------------------

/// Preferred position of a content widget relative to a cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetPosition {
    Above,
    Below,
    Exact,
}

impl fmt::Display for WidgetPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WidgetPosition::Above => f.write_str("above"),
            WidgetPosition::Below => f.write_str("below"),
            WidgetPosition::Exact => f.write_str("exact"),
        }
    }
}

/// Resolve widget position considering viewport bounds.
pub fn resolve_widget_position(
    widget_height: u32,
    cursor_line: u32,
    viewport_start: u32,
    viewport_end: u32,
) -> WidgetPosition {
    if cursor_line <= viewport_start + widget_height {
        WidgetPosition::Below
    } else if cursor_line + widget_height > viewport_end {
        WidgetPosition::Above
    } else {
        WidgetPosition::Below
    }
}

// -- Viewpart damage tracking ------------------------------------------------

/// A dirty region that needs re-rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageRegion {
    pub start_line: u32,
    pub end_line: u32,
}

/// Tracks dirty regions for partial re-render.
#[derive(Debug, Default)]
pub struct DamageTracker {
    regions: Vec<DamageRegion>,
}

impl DamageTracker {
    pub fn new() -> Self {
        Self { regions: Vec::new() }
    }

    /// Mark a range of lines as dirty.
    pub fn mark_dirty(&mut self, start: u32, end: u32) {
        if start > end {
            return;
        }
        self.regions.push(DamageRegion {
            start_line: start,
            end_line: end,
        });
        self.merge_regions();
    }

    /// Clear all damage.
    pub fn clear(&mut self) {
        self.regions.clear();
    }

    /// Check if any region is dirty.
    pub fn is_dirty(&self) -> bool {
        !self.regions.is_empty()
    }

    /// Return the merged dirty regions.
    pub fn regions(&self) -> &[DamageRegion] {
        &self.regions
    }

    /// Check if a specific line is in a dirty region.
    pub fn is_line_dirty(&self, line: u32) -> bool {
        self.regions.iter().any(|r| line >= r.start_line && line <= r.end_line)
    }

    /// Total number of dirty lines.
    pub fn dirty_line_count(&self) -> u32 {
        self.regions.iter().map(|r| r.end_line - r.start_line + 1).sum()
    }

    fn merge_regions(&mut self) {
        if self.regions.len() < 2 {
            return;
        }
        self.regions.sort_by_key(|r| r.start_line);
        let mut merged = vec![self.regions[0].clone()];
        for r in &self.regions[1..] {
            let last = merged.last_mut().unwrap();
            if r.start_line <= last.end_line + 1 {
                last.end_line = last.end_line.max(r.end_line);
            } else {
                merged.push(r.clone());
            }
        }
        self.regions = merged;
    }
}

impl fmt::Display for DamageTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DamageTracker({} regions, {} dirty lines)", self.regions.len(), self.dirty_line_count())
    }
}


// ---------------------------------------------------------------------------
// ViewpartBracketGuides
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ViewpartBracketGuides {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl ViewpartBracketGuides {
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

impl Default for ViewpartBracketGuides {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for ViewpartBracketGuides {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "ViewpartBracketGuides({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// ViewpartFoldIndicators
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ViewpartFoldIndicators {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl ViewpartFoldIndicators {
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

impl Default for ViewpartFoldIndicators {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for ViewpartFoldIndicators {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "ViewpartFoldIndicators({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// ViewpartBracketGuidesSnapshot — point-in-time snapshot of ViewpartBracketGuides state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ViewpartBracketGuidesSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl ViewpartBracketGuidesSnapshot {
    pub fn capture(source: &ViewpartBracketGuides, timestamp: u64) -> Self {
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

impl fmt::Display for ViewpartBracketGuidesSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// ViewpartFoldIndicatorsStats — aggregate statistics for ViewpartFoldIndicators
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ViewpartFoldIndicatorsStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl ViewpartFoldIndicatorsStats {
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

impl fmt::Display for ViewpartFoldIndicatorsStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// ViewpartBracketGuidesConfig — configuration for ViewpartBracketGuides
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ViewpartBracketGuidesConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl ViewpartBracketGuidesConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for ViewpartBracketGuidesConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for ViewpartBracketGuidesConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_remove_view_zones() {
        let mut parts = EditorViewParts::new();
        let id1 = parts.add_view_zone(10, 3);
        let id2 = parts.add_view_zone(20, 5);
        assert_eq!(parts.get_view_zones().len(), 2);
        parts.remove_view_zone(id1);
        assert_eq!(parts.get_view_zones().len(), 1);
        assert_eq!(parts.get_view_zones()[0].id, id2);
    }

    #[test]
    fn zone_ids_increment() {
        let mut parts = EditorViewParts::new();
        let id1 = parts.add_view_zone(1, 1);
        let id2 = parts.add_view_zone(2, 1);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn add_and_remove_overlay() {
        let mut parts = EditorViewParts::new();
        parts.add_overlay(OverlayWidget {
            id: "find".into(),
            position_top: 0,
            position_left: 100,
            content: "Find widget".into(),
            visible: true,
        });
        assert_eq!(parts.overlays.len(), 1);
        parts.remove_overlay("find");
        assert!(parts.overlays.is_empty());
    }

    #[test]
    fn add_glyph_margin() {
        let mut parts = EditorViewParts::new();
        parts.add_glyph_margin(GlyphMarginWidget {
            line: 5,
            glyph: "●".into(),
            tooltip: Some("Breakpoint".into()),
        });
        assert_eq!(parts.glyph_margins.len(), 1);
        assert_eq!(parts.glyph_margins[0].line, 5);
    }

    #[test]
    fn add_and_remove_content_widget() {
        let mut parts = EditorViewParts::new();
        parts.add_content_widget(ContentWidget {
            id: "hint1".into(),
            line: 10,
            column: 5,
            content: "type hint".into(),
            visible: true,
        });
        parts.add_content_widget(ContentWidget {
            id: "hint2".into(),
            line: 20,
            column: 1,
            content: "parameter hint".into(),
            visible: true,
        });
        assert_eq!(parts.content_widgets.len(), 2);
        parts.remove_content_widget("hint1");
        assert_eq!(parts.content_widgets.len(), 1);
        assert_eq!(parts.content_widgets[0].id, "hint2");
    }

    #[test]
    fn total_widget_count() {
        let mut parts = EditorViewParts::new();
        parts.add_view_zone(1, 2);
        parts.add_overlay(OverlayWidget {
            id: "o1".into(),
            position_top: 0,
            position_left: 0,
            content: "overlay".into(),
            visible: true,
        });
        parts.add_content_widget(ContentWidget {
            id: "c1".into(),
            line: 1,
            column: 1,
            content: "cw".into(),
            visible: true,
        });
        parts.add_glyph_margin(GlyphMarginWidget {
            line: 1,
            glyph: "!".into(),
            tooltip: None,
        });
        assert_eq!(parts.total_widget_count(), 4);
    }

    #[test]
    fn clear_all() {
        let mut parts = EditorViewParts::new();
        parts.add_view_zone(1, 1);
        parts.add_overlay(OverlayWidget {
            id: "o".into(),
            position_top: 0,
            position_left: 0,
            content: "x".into(),
            visible: true,
        });
        parts.add_content_widget(ContentWidget {
            id: "c".into(),
            line: 1,
            column: 1,
            content: "y".into(),
            visible: true,
        });
        parts.add_glyph_margin(GlyphMarginWidget {
            line: 1,
            glyph: "g".into(),
            tooltip: None,
        });
        assert_eq!(parts.total_widget_count(), 4);
        parts.clear_all();
        assert_eq!(parts.total_widget_count(), 0);
    }

    #[test]
    fn update_view_zone_height() {
        let mut parts = EditorViewParts::new();
        let id = parts.add_view_zone(5, 3);
        assert!(parts.update_view_zone(id, 10));
        assert_eq!(parts.zones[0].height_in_lines, 10);
        assert!(!parts.update_view_zone(999, 1));
    }

    #[test]
    fn set_overlay_visible_and_get_visible() {
        let mut parts = EditorViewParts::new();
        parts.add_overlay(OverlayWidget {
            id: "a".into(),
            position_top: 0,
            position_left: 0,
            content: "A".into(),
            visible: true,
        });
        parts.add_overlay(OverlayWidget {
            id: "b".into(),
            position_top: 0,
            position_left: 0,
            content: "B".into(),
            visible: true,
        });
        assert_eq!(parts.get_visible_overlays().len(), 2);
        assert!(parts.set_overlay_visible("a", false));
        assert_eq!(parts.get_visible_overlays().len(), 1);
        assert_eq!(parts.get_visible_overlays()[0].id, "b");
        assert!(!parts.set_overlay_visible("nonexistent", false));
    }

    #[test]
    fn set_content_widget_visible() {
        let mut parts = EditorViewParts::new();
        parts.add_content_widget(ContentWidget {
            id: "cw1".into(),
            line: 1,
            column: 1,
            content: "hint".into(),
            visible: true,
        });
        assert!(parts.set_content_widget_visible("cw1", false));
        assert!(!parts.content_widgets[0].visible);
        assert!(parts.set_content_widget_visible("cw1", true));
        assert!(parts.content_widgets[0].visible);
        assert!(!parts.set_content_widget_visible("missing", false));
    }

    #[test]
    fn get_widgets_at_line() {
        let mut parts = EditorViewParts::new();
        parts.add_view_zone(5, 2);
        parts.add_overlay(OverlayWidget {
            id: "ol".into(),
            position_top: 5,
            position_left: 0,
            content: "overlay at 5".into(),
            visible: true,
        });
        parts.add_content_widget(ContentWidget {
            id: "cw".into(),
            line: 5,
            column: 1,
            content: "content at 5".into(),
            visible: true,
        });
        parts.add_glyph_margin(GlyphMarginWidget {
            line: 5,
            glyph: "●".into(),
            tooltip: None,
        });
        parts.add_glyph_margin(GlyphMarginWidget {
            line: 10,
            glyph: "!".into(),
            tooltip: None,
        });
        let refs = parts.get_widgets_at_line(5);
        assert_eq!(refs.len(), 4);
        assert!(matches!(refs[0], WidgetRef::Zone(_)));
        assert!(matches!(refs[1], WidgetRef::Overlay(_)));
        assert!(matches!(refs[2], WidgetRef::Content(_)));
        assert!(matches!(refs[3], WidgetRef::GlyphMargin(_)));
        assert_eq!(parts.get_widgets_at_line(10).len(), 1);
        assert_eq!(parts.get_widgets_at_line(99).len(), 0);
    }

    #[test]
    fn minimap_defaults() {
        let m = Minimap::default();
        assert!(m.enabled);
        assert_eq!(m.side, MinimapSide::Right);
        assert!((m.scale - 1.0).abs() < f64::EPSILON);
        assert_eq!(m.max_column, 120);
    }

    #[test]
    fn minimap_custom() {
        let m = Minimap {
            enabled: false,
            side: MinimapSide::Left,
            scale: 0.5,
            max_column: 80,
        };
        assert!(!m.enabled);
        assert_eq!(m.side, MinimapSide::Left);
        assert!((m.scale - 0.5).abs() < f64::EPSILON);
        assert_eq!(m.max_column, 80);
    }

    #[test]
    fn breadcrumb_bar_push_pop_path() {
        let mut bar = BreadcrumbBar::new();
        bar.push(BreadcrumbItem {
            label: "src".into(),
            kind: BreadcrumbKind::Module,
            uri: "file:///src".into(),
        });
        bar.push(BreadcrumbItem {
            label: "editor.rs".into(),
            kind: BreadcrumbKind::File,
            uri: "file:///src/editor.rs".into(),
        });
        bar.push(BreadcrumbItem {
            label: "Editor".into(),
            kind: BreadcrumbKind::Class,
            uri: "file:///src/editor.rs#Editor".into(),
        });
        assert_eq!(bar.get_path_string(), "src / editor.rs / Editor");
        let popped = bar.pop().unwrap();
        assert_eq!(popped.label, "Editor");
        assert_eq!(popped.kind, BreadcrumbKind::Class);
        assert_eq!(bar.get_path_string(), "src / editor.rs");
    }

    #[test]
    fn breadcrumb_bar_empty() {
        let bar = BreadcrumbBar::new();
        assert_eq!(bar.get_path_string(), "");
        assert_eq!(bar.items.len(), 0);
    }

    // --- ViewPartMetrics / priority tests ---

    fn sample_parts() -> Vec<PrioritizedViewPart> {
        vec![
            PrioritizedViewPart {
                name: "glyph_margin".into(),
                priority: ViewPartPriority::High,
                visible: true,
                render_time_us: 120,
            },
            PrioritizedViewPart {
                name: "content_widget".into(),
                priority: ViewPartPriority::Normal,
                visible: true,
                render_time_us: 300,
            },
            PrioritizedViewPart {
                name: "decorative_overlay".into(),
                priority: ViewPartPriority::Low,
                visible: false,
                render_time_us: 50,
            },
            PrioritizedViewPart {
                name: "minimap".into(),
                priority: ViewPartPriority::Normal,
                visible: true,
                render_time_us: 500,
            },
        ]
    }

    #[test]
    fn compute_metrics_counts() {
        let parts = sample_parts();
        let m = compute_viewpart_metrics(&parts);
        assert_eq!(m.total_parts, 4);
        assert_eq!(m.visible_count, 3);
        assert_eq!(m.hidden_count, 1);
    }

    #[test]
    fn compute_metrics_render_time() {
        let parts = sample_parts();
        let m = compute_viewpart_metrics(&parts);
        assert_eq!(m.total_render_time_us, 120 + 300 + 50 + 500);
    }

    #[test]
    fn compute_metrics_empty_slice() {
        let m = compute_viewpart_metrics(&[]);
        assert_eq!(m.total_parts, 0);
        assert_eq!(m.visible_count, 0);
        assert_eq!(m.hidden_count, 0);
        assert_eq!(m.total_render_time_us, 0);
    }

    #[test]
    fn sort_viewparts_ordering() {
        let mut parts = sample_parts();
        sort_viewparts_by_priority(&mut parts);
        assert_eq!(parts[0].priority, ViewPartPriority::High);
        assert_eq!(parts[1].priority, ViewPartPriority::Normal);
        assert_eq!(parts[2].priority, ViewPartPriority::Normal);
        assert_eq!(parts[3].priority, ViewPartPriority::Low);
    }

    #[test]
    fn sort_viewparts_stable_within_priority() {
        let mut parts = vec![
            PrioritizedViewPart {
                name: "first_normal".into(),
                priority: ViewPartPriority::Normal,
                visible: true,
                render_time_us: 10,
            },
            PrioritizedViewPart {
                name: "second_normal".into(),
                priority: ViewPartPriority::Normal,
                visible: true,
                render_time_us: 20,
            },
            PrioritizedViewPart {
                name: "high".into(),
                priority: ViewPartPriority::High,
                visible: true,
                render_time_us: 5,
            },
        ];
        sort_viewparts_by_priority(&mut parts);
        assert_eq!(parts[0].name, "high");
        // stable sort preserves insertion order within Normal
        assert_eq!(parts[1].name, "first_normal");
        assert_eq!(parts[2].name, "second_normal");
    }

    #[test]
    fn priority_enum_ordering() {
        assert!(ViewPartPriority::High < ViewPartPriority::Normal);
        assert!(ViewPartPriority::Normal < ViewPartPriority::Low);
        assert!(ViewPartPriority::High < ViewPartPriority::Low);
    }

    #[test]
    fn detect_no_overlaps() {
        let zones = vec![
            ViewZone { id: 1, after_line: 5, height_in_lines: 3, content: None },
            ViewZone { id: 2, after_line: 20, height_in_lines: 2, content: None },
        ];
        assert!(detect_view_zone_overlaps(&zones).is_empty());
    }

    #[test]
    fn detect_overlapping_zones() {
        let zones = vec![
            ViewZone { id: 1, after_line: 5, height_in_lines: 5, content: None },
            ViewZone { id: 2, after_line: 8, height_in_lines: 3, content: None },
        ];
        let overlaps = detect_view_zone_overlaps(&zones);
        assert_eq!(overlaps.len(), 1);
        assert_eq!(overlaps[0].zone_a, 1);
        assert_eq!(overlaps[0].zone_b, 2);
    }

    #[test]
    fn compute_overlay_layout_basic() {
        let widget = OverlayWidget {
            id: "test".into(),
            position_top: 5,
            position_left: 10,
            content: "Hello World".into(),
            visible: true,
        };
        let layout = compute_overlay_layout(&widget, 80, 24);
        assert_eq!(layout.top, 5);
        assert_eq!(layout.left, 10);
        assert_eq!(layout.width, 11);
        assert_eq!(layout.height, 1);
    }

    #[test]
    fn compute_overlay_layout_clamps_width() {
        let widget = OverlayWidget {
            id: "wide".into(),
            position_top: 0,
            position_left: 75,
            content: "A very long content string".into(),
            visible: true,
        };
        let layout = compute_overlay_layout(&widget, 80, 24);
        assert_eq!(layout.width, 5); // 80 - 75
    }

    #[test]
    fn resolve_breadcrumb_path_basic() {
        let bar = resolve_breadcrumb_path("src/editor/main.rs");
        assert_eq!(bar.items.len(), 3);
        assert_eq!(bar.items[0].label, "src");
        assert_eq!(bar.items[0].kind, BreadcrumbKind::Module);
        assert_eq!(bar.items[2].label, "main.rs");
        assert_eq!(bar.items[2].kind, BreadcrumbKind::File);
        assert_eq!(bar.get_path_string(), "src / editor / main.rs");
    }

    #[test]
    fn glyph_margin_config_default() {
        let cfg = GlyphMarginConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.width_chars, 2);
        assert!(cfg.decorations_enabled);
    }

    #[test]
    fn glyph_margin_effective_width() {
        let cfg = GlyphMarginConfig::default();
        assert_eq!(cfg.effective_width(8), 16);

        let disabled = GlyphMarginConfig { enabled: false, ..Default::default() };
        assert_eq!(disabled.effective_width(8), 0);
    }

    #[test]
    fn gutter_decoration_new_and_display() {
        let d = GutterDecoration::new(5, GutterDecorationType::Breakpoint);
        assert_eq!(d.line, 5);
        assert!(d.enabled);
        assert_eq!(d.glyph(), "●");
        let s = format!("{d}");
        assert!(s.contains("breakpoint"));
        assert!(s.contains("L5"));
    }

    #[test]
    fn gutter_decoration_with_tooltip() {
        let d = GutterDecoration::new(10, GutterDecorationType::Error)
            .with_tooltip("Syntax error on line 10");
        assert_eq!(d.tooltip.as_deref(), Some("Syntax error on line 10"));
    }

    #[test]
    fn gutter_decoration_type_display() {
        assert_eq!(format!("{}", GutterDecorationType::Breakpoint), "breakpoint");
        assert_eq!(format!("{}", GutterDecorationType::Bookmark), "bookmark");
        assert_eq!(format!("{}", GutterDecorationType::Warning), "warning");
    }

    #[test]
    fn gutter_decoration_glyphs() {
        let types_and_glyphs = vec![
            (GutterDecorationType::Breakpoint, "●"),
            (GutterDecorationType::ConditionalBreakpoint, "◆"),
            (GutterDecorationType::Bookmark, "★"),
            (GutterDecorationType::Error, "✗"),
            (GutterDecorationType::Warning, "⚠"),
            (GutterDecorationType::Info, "ℹ"),
        ];
        for (dtype, expected_glyph) in types_and_glyphs {
            let d = GutterDecoration::new(1, dtype);
            assert_eq!(d.glyph(), expected_glyph);
        }
    }

    #[test]
    fn gutter_manager_add_and_query() {
        let mut mgr = GutterDecorationManager::new();
        mgr.add(GutterDecoration::new(5, GutterDecorationType::Breakpoint));
        mgr.add(GutterDecoration::new(5, GutterDecorationType::Bookmark));
        mgr.add(GutterDecoration::new(10, GutterDecorationType::Error));
        assert_eq!(mgr.count(), 3);
        assert_eq!(mgr.get_at_line(5).len(), 2);
        assert_eq!(mgr.get_at_line(10).len(), 1);
        assert_eq!(mgr.get_at_line(1).len(), 0);
    }

    #[test]
    fn gutter_manager_toggle_breakpoint() {
        let mut mgr = GutterDecorationManager::new();
        mgr.toggle_breakpoint(5);
        assert_eq!(mgr.count_by_type(GutterDecorationType::Breakpoint), 1);
        mgr.toggle_breakpoint(5);
        assert_eq!(mgr.count_by_type(GutterDecorationType::Breakpoint), 0);
    }

    #[test]
    fn gutter_manager_remove_by_type() {
        let mut mgr = GutterDecorationManager::new();
        mgr.add(GutterDecoration::new(5, GutterDecorationType::Breakpoint));
        mgr.add(GutterDecoration::new(5, GutterDecorationType::Bookmark));
        mgr.remove_by_type(5, GutterDecorationType::Breakpoint);
        assert_eq!(mgr.count(), 1);
        assert_eq!(mgr.get_at_line(5)[0].decoration_type, GutterDecorationType::Bookmark);
    }

    #[test]
    fn gutter_manager_decorated_lines() {
        let mut mgr = GutterDecorationManager::new();
        mgr.add(GutterDecoration::new(10, GutterDecorationType::Breakpoint));
        mgr.add(GutterDecoration::new(5, GutterDecorationType::Error));
        mgr.add(GutterDecoration::new(10, GutterDecorationType::Bookmark));
        let lines = mgr.decorated_lines();
        assert_eq!(lines, vec![5, 10]);
    }

    #[test]
    fn compute_indent_guides_basic() {
        let levels = vec![0, 1, 2, 2, 1, 0];
        let guides = compute_indent_guides(&levels, 2, 1);
        // Line at index 2 (indent level 2) should have guides at level 1 and 2
        let line3_guides: Vec<_> = guides.iter().filter(|g| g.line == 3).collect();
        assert_eq!(line3_guides.len(), 2);
    }

    #[test]
    fn compute_indent_guides_active_line() {
        let levels = vec![0, 1, 2];
        let guides = compute_indent_guides(&levels, 2, 1);
        let active_guides: Vec<_> = guides.iter().filter(|g| g.is_active).collect();
        assert!(!active_guides.is_empty());
    }

    #[test]
    fn compute_indentation_levels_spaces() {
        let lines = vec!["fn main() {", "    let x = 1;", "        nested();", "}"];
        let levels = compute_indentation_levels(&lines, 4);
        assert_eq!(levels, vec![0, 1, 2, 0]);
    }

    #[test]
    fn compute_indentation_levels_tabs() {
        let lines = vec!["no indent", "\tone tab", "\t\ttwo tabs"];
        let levels = compute_indentation_levels(&lines, 4);
        assert_eq!(levels, vec![0, 1, 2]);
    }

    #[test]
    fn compute_indentation_levels_empty() {
        let levels = compute_indentation_levels(&[], 4);
        assert!(levels.is_empty());
    }

    #[test]
    fn view_zone_count() {
        let mut parts = EditorViewParts::new();
        assert_eq!(parts.view_zone_count(), 0);
        parts.add_view_zone(1, 3);
        parts.add_view_zone(5, 2);
        assert_eq!(parts.view_zone_count(), 2);
    }

    #[test]
    fn overlay_and_content_widget_count() {
        let mut parts = EditorViewParts::new();
        assert_eq!(parts.overlay_count(), 0);
        assert_eq!(parts.content_widget_count(), 0);
        parts.add_overlay(OverlayWidget {
            id: "o1".into(), position_top: 0, position_left: 0,
            content: "test".into(), visible: true,
        });
        parts.add_content_widget(ContentWidget {
            id: "c1".into(), line: 1, column: 1,
            content: "hint".into(), visible: true,
        });
        assert_eq!(parts.overlay_count(), 1);
        assert_eq!(parts.content_widget_count(), 1);
    }

    #[test]
    fn editor_view_parts_display() {
        let parts = EditorViewParts::new();
        let s = format!("{parts}");
        assert!(s.contains("zones: 0"));
        assert!(s.contains("overlays: 0"));
    }

    #[test]
    fn minimap_builder_and_display() {
        let m = Minimap::default()
            .with_side(MinimapSide::Left)
            .with_scale(2.0);
        assert_eq!(m.side, MinimapSide::Left);
        assert!((m.scale - 2.0).abs() < f64::EPSILON);
        let s = format!("{m}");
        assert!(s.contains("left"));
        assert!(s.contains("enabled"));
    }

    #[test]
    fn view_zone_height_pixels() {
        let zone = ViewZone { id: 1, after_line: 0, height_in_lines: 5, content: None };
        assert_eq!(zone.height_pixels(20), 100);
        assert_eq!(zone.height_pixels(0), 0);
    }

    #[test]
    fn gutter_decoration_is_breakpoint() {
        let bp = GutterDecoration::new(1, GutterDecorationType::Breakpoint);
        assert!(bp.is_breakpoint());
        let cbp = GutterDecoration::new(2, GutterDecorationType::ConditionalBreakpoint);
        assert!(cbp.is_breakpoint());
        let bm = GutterDecoration::new(3, GutterDecorationType::Bookmark);
        assert!(!bm.is_breakpoint());
    }

    #[test]
    fn gutter_decoration_type_is_diagnostic() {
        assert!(GutterDecorationType::Error.is_diagnostic());
        assert!(GutterDecorationType::Warning.is_diagnostic());
        assert!(GutterDecorationType::Info.is_diagnostic());
        assert!(!GutterDecorationType::Breakpoint.is_diagnostic());
        assert!(!GutterDecorationType::Bookmark.is_diagnostic());
    }

    #[test]
    fn line_number_width_single_digit() {
        assert_eq!(line_number_width(0), 1);
        assert_eq!(line_number_width(1), 1);
        assert_eq!(line_number_width(9), 1);
    }

    #[test]
    fn line_number_width_multi_digit() {
        assert_eq!(line_number_width(10), 2);
        assert_eq!(line_number_width(99), 2);
        assert_eq!(line_number_width(100), 3);
        assert_eq!(line_number_width(9999), 4);
        assert_eq!(line_number_width(10000), 5);
    }

    #[test]
    fn compute_gutter_width_with_and_without_margin() {
        let with_margin = GlyphMarginConfig { enabled: true, width_chars: 2, decorations_enabled: true };
        assert_eq!(compute_gutter_width(99, &with_margin), 4); // 2 digits + 2 glyph
        let without_margin = GlyphMarginConfig { enabled: false, width_chars: 2, decorations_enabled: true };
        assert_eq!(compute_gutter_width(99, &without_margin), 2); // 2 digits + 0 glyph
    }

    #[test]
    fn minimap_scaled_max_column() {
        let mut m = Minimap::default();
        assert_eq!(m.scaled_max_column(), 120);
        m.scale = 0.5;
        assert_eq!(m.scaled_max_column(), 60);
        m.scale = 2.0;
        assert_eq!(m.scaled_max_column(), 240);
    }

    #[test]
    fn minimap_visible_lines_disabled() {
        let m = Minimap { enabled: false, ..Minimap::default() };
        assert_eq!(m.visible_lines(1000, 20), 0);
    }

    #[test]
    fn minimap_visible_lines_basic() {
        let m = Minimap::default(); // scale=1.0
        assert_eq!(m.visible_lines(200, 20), 10);
    }

    #[test]
    fn minimap_line_to_y_basic() {
        let m = Minimap::default();
        assert_eq!(m.line_to_y(10, 5, 20), 100); // (10-5)*20 = 100
        assert_eq!(m.line_to_y(5, 5, 20), 0);
    }

    #[test]
    fn view_part_visibility_defaults_all_visible() {
        let v = ViewPartVisibility::default();
        assert_eq!(v.visible_count(), 5);
        assert!(v.line_numbers);
        assert!(v.minimap);
    }

    #[test]
    fn view_part_visibility_all_hidden() {
        let v = ViewPartVisibility::all_hidden();
        assert_eq!(v.visible_count(), 0);
    }

    #[test]
    fn view_part_visibility_toggle() {
        let mut v = ViewPartVisibility::default();
        assert!(v.toggle("minimap"));
        assert!(!v.minimap);
        assert_eq!(v.visible_count(), 4);
        assert!(v.toggle("minimap"));
        assert!(v.minimap);
        assert!(!v.toggle("nonexistent"));
    }

    #[test]
    fn compute_column_layout_basic() {
        let glyph = GlyphMarginConfig::default();
        let vis = ViewPartVisibility::default();
        let layout = compute_column_layout(100, &glyph, 8, &vis);
        // line_number_width(100)=3, glyph=2*8=16, ln=3*8=24
        assert_eq!(layout.glyph_margin_width, 16);
        assert_eq!(layout.line_number_width, 24);
        assert_eq!(layout.content_left, 40);
    }

    #[test]
    fn compute_column_layout_hidden_parts() {
        let glyph = GlyphMarginConfig::default();
        let vis = ViewPartVisibility::all_hidden();
        let layout = compute_column_layout(100, &glyph, 8, &vis);
        assert_eq!(layout.glyph_margin_width, 0);
        assert_eq!(layout.line_number_width, 0);
        assert_eq!(layout.content_left, 0);
    }

    #[test]
    fn collect_view_zone_ids_returns_all() {
        let mut parts = EditorViewParts::new();
        let id1 = parts.add_view_zone(1, 3);
        let id2 = parts.add_view_zone(5, 2);
        let ids = collect_view_zone_ids(&parts);
        assert_eq!(ids, vec![id1, id2]);
    }

    #[test]
    fn total_view_zone_height_sums() {
        let mut parts = EditorViewParts::new();
        parts.add_view_zone(1, 3);
        parts.add_view_zone(5, 2);
        assert_eq!(total_view_zone_height(&parts), 5);
    }

    #[test]
    fn total_view_zone_height_empty() {
        let parts = EditorViewParts::new();
        assert_eq!(total_view_zone_height(&parts), 0);
    }

    #[test]
    fn overlay_widget_ids_visible_only() {
        let mut parts = EditorViewParts::new();
        parts.add_overlay(OverlayWidget {
            id: "o1".into(), position_top: 0, position_left: 0,
            content: "hi".into(), visible: true,
        });
        parts.add_overlay(OverlayWidget {
            id: "o2".into(), position_top: 0, position_left: 0,
            content: "bye".into(), visible: false,
        });
        let ids = overlay_widget_ids(&parts);
        assert_eq!(ids, vec!["o1".to_string()]);
    }

    #[test]
    fn find_content_widget_found() {
        let mut parts = EditorViewParts::new();
        parts.add_content_widget(ContentWidget {
            id: "cw1".into(), line: 5, column: 0,
            content: "note".into(), visible: true,
        });
        assert!(find_content_widget(&parts, "cw1").is_some());
    }

    #[test]
    fn find_content_widget_not_found() {
        let parts = EditorViewParts::new();
        assert!(find_content_widget(&parts, "nope").is_none());
    }

    #[test]
    fn glyph_count_on_line_filters() {
        let mut parts = EditorViewParts::new();
        parts.add_glyph_margin(GlyphMarginWidget { line: 3, glyph: "●".into(), tooltip: None });
        parts.add_glyph_margin(GlyphMarginWidget { line: 3, glyph: "▶".into(), tooltip: None });
        parts.add_glyph_margin(GlyphMarginWidget { line: 5, glyph: "●".into(), tooltip: None });
        assert_eq!(glyph_count_on_line(&parts, 3), 2);
        assert_eq!(glyph_count_on_line(&parts, 5), 1);
        assert_eq!(glyph_count_on_line(&parts, 1), 0);
    }

    #[test]
    fn view_zone_lines_unique_sorted() {
        let mut parts = EditorViewParts::new();
        parts.add_view_zone(5, 1);
        parts.add_view_zone(2, 1);
        parts.add_view_zone(5, 2);
        let lines = view_zone_lines(&parts);
        assert_eq!(lines, vec![2, 5]);
    }

    #[test]
    fn line_has_widget_true_for_zone() {
        let mut parts = EditorViewParts::new();
        parts.add_view_zone(3, 2);
        assert!(line_has_widget(&parts, 3));
    }

    #[test]
    fn max_view_zone_height_picks_largest() {
        let mut parts = EditorViewParts::new();
        parts.add_view_zone(1, 3);
        parts.add_view_zone(2, 7);
        parts.add_view_zone(3, 1);
        assert_eq!(max_view_zone_height(&parts), 7);
    }

    #[test]
    fn visible_line_number_count_absolute() {
        let count = visible_line_number_count(10, 5, LineNumberMode::Absolute);
        assert_eq!(count, 10);
    }

    #[test]
    fn visible_line_number_count_interval() {
        // With interval 5 and current_line=1: lines 1,5,10 are shown
        let count = visible_line_number_count(10, 1, LineNumberMode::Interval(5));
        assert_eq!(count, 3); // 1 (current), 5, 10
    }

    // -- compute_view_part_metrics ---------------------------------------------

    #[test]
    fn view_part_metrics_basic() {
        let parts = vec![
            PrioritizedViewPart { name: "a".into(), priority: ViewPartPriority::High, visible: true, render_time_us: 100 },
            PrioritizedViewPart { name: "b".into(), priority: ViewPartPriority::Low, visible: false, render_time_us: 50 },
        ];
        let m = compute_view_part_metrics(&parts);
        assert_eq!(m.total_parts, 2);
        assert_eq!(m.visible_count, 1);
        assert_eq!(m.hidden_count, 1);
        assert_eq!(m.total_render_time_us, 150);
    }

    // -- visible_parts_sorted --------------------------------------------------

    #[test]
    fn visible_parts_sorted_by_priority() {
        let parts = vec![
            PrioritizedViewPart { name: "low".into(), priority: ViewPartPriority::Low, visible: true, render_time_us: 0 },
            PrioritizedViewPart { name: "high".into(), priority: ViewPartPriority::High, visible: true, render_time_us: 0 },
            PrioritizedViewPart { name: "hidden".into(), priority: ViewPartPriority::High, visible: false, render_time_us: 0 },
        ];
        let sorted = visible_parts_sorted(&parts);
        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].name, "high");
        assert_eq!(sorted[1].name, "low");
    }

    // -- layouts_overlap -------------------------------------------------------

    #[test]
    fn layouts_overlap_true() {
        let a = WidgetLayout { top: 0, left: 0, width: 10, height: 10 };
        let b = WidgetLayout { top: 5, left: 5, width: 10, height: 10 };
        assert!(layouts_overlap(&a, &b));
    }

    #[test]
    fn layouts_overlap_false() {
        let a = WidgetLayout { top: 0, left: 0, width: 5, height: 5 };
        let b = WidgetLayout { top: 10, left: 10, width: 5, height: 5 };
        assert!(!layouts_overlap(&a, &b));
    }

    // -- bounding_box ----------------------------------------------------------

    #[test]
    fn bounding_box_computed() {
        let layouts = vec![
            WidgetLayout { top: 5, left: 10, width: 20, height: 10 },
            WidgetLayout { top: 0, left: 0, width: 5, height: 5 },
        ];
        let bb = bounding_box(&layouts).unwrap();
        assert_eq!(bb.top, 0);
        assert_eq!(bb.left, 0);
        assert_eq!(bb.width, 30); // 0..30
        assert_eq!(bb.height, 15); // 0..15
    }

    #[test]
    fn bounding_box_empty() {
        assert!(bounding_box(&[]).is_none());
    }

    // -- breadcrumbs_from_path -------------------------------------------------

    #[test]
    fn breadcrumbs_from_path_basic() {
        let bar = breadcrumbs_from_path("/src/components/App.tsx");
        assert_eq!(bar.items.len(), 3);
        assert_eq!(bar.items[0].label, "src");
        assert_eq!(bar.items[2].label, "App.tsx");
    }

    // -- total_gutter_width ----------------------------------------------------

    #[test]
    fn total_gutter_width_enabled() {
        let config = GlyphMarginConfig { enabled: true, width_chars: 2, decorations_enabled: true };
        assert_eq!(total_gutter_width(&config, 4), 6);
    }

    #[test]
    fn total_gutter_width_disabled() {
        let config = GlyphMarginConfig { enabled: false, width_chars: 2, decorations_enabled: false };
        assert_eq!(total_gutter_width(&config, 4), 4);
    }

    // -- ViewPartOverlap tests ------------------------------------------------

    #[test]
    fn detect_overlaps_none() {
        let ranges = vec![
            DecorationRange { id: "a".into(), start_line: 1, end_line: 5, priority: 0 },
            DecorationRange { id: "b".into(), start_line: 6, end_line: 10, priority: 0 },
        ];
        assert!(detect_overlaps(&ranges).is_empty());
    }

    #[test]
    fn detect_overlaps_found() {
        let ranges = vec![
            DecorationRange { id: "a".into(), start_line: 1, end_line: 8, priority: 0 },
            DecorationRange { id: "b".into(), start_line: 5, end_line: 12, priority: 0 },
        ];
        let overlaps = detect_overlaps(&ranges);
        assert_eq!(overlaps.len(), 1);
        assert_eq!(overlaps[0].overlap_start, 5);
        assert_eq!(overlaps[0].overlap_end, 8);
    }

    #[test]
    fn detect_overlaps_multiple() {
        let ranges = vec![
            DecorationRange { id: "a".into(), start_line: 1, end_line: 10, priority: 0 },
            DecorationRange { id: "b".into(), start_line: 5, end_line: 15, priority: 0 },
            DecorationRange { id: "c".into(), start_line: 8, end_line: 20, priority: 0 },
        ];
        let overlaps = detect_overlaps(&ranges);
        assert_eq!(overlaps.len(), 3);
    }

    // -- GutterIcon tests -----------------------------------------------------

    #[test]
    fn sort_gutter_icons_by_line_and_priority() {
        let mut icons = vec![
            GutterIcon { line: 2, icon_id: "low".into(), priority: 1, tooltip: None },
            GutterIcon { line: 1, icon_id: "high".into(), priority: 10, tooltip: None },
            GutterIcon { line: 2, icon_id: "high".into(), priority: 5, tooltip: None },
        ];
        sort_gutter_icons(&mut icons);
        assert_eq!(icons[0].line, 1);
        assert_eq!(icons[1].icon_id, "high");
        assert_eq!(icons[1].priority, 5);
    }

    #[test]
    fn top_icons_per_line_picks_highest() {
        let icons = vec![
            GutterIcon { line: 1, icon_id: "low".into(), priority: 1, tooltip: None },
            GutterIcon { line: 1, icon_id: "high".into(), priority: 10, tooltip: None },
            GutterIcon { line: 2, icon_id: "only".into(), priority: 5, tooltip: None },
        ];
        let tops = top_icons_per_line(&icons);
        assert_eq!(tops.len(), 2);
        assert_eq!(tops[0].icon_id, "high");
        assert_eq!(tops[1].icon_id, "only");
    }

    // -- WidgetPosition tests -------------------------------------------------

    #[test]
    fn widget_position_display() {
        assert_eq!(WidgetPosition::Above.to_string(), "above");
        assert_eq!(WidgetPosition::Below.to_string(), "below");
        assert_eq!(WidgetPosition::Exact.to_string(), "exact");
    }

    #[test]
    fn resolve_widget_position_below() {
        assert_eq!(resolve_widget_position(5, 3, 1, 50), WidgetPosition::Below);
    }

    #[test]
    fn resolve_widget_position_above() {
        assert_eq!(resolve_widget_position(5, 48, 1, 50), WidgetPosition::Above);
    }

    // -- DamageTracker tests --------------------------------------------------

    #[test]
    fn damage_tracker_mark_and_query() {
        let mut tracker = DamageTracker::new();
        assert!(!tracker.is_dirty());
        tracker.mark_dirty(5, 10);
        assert!(tracker.is_dirty());
        assert!(tracker.is_line_dirty(7));
        assert!(!tracker.is_line_dirty(11));
    }

    #[test]
    fn damage_tracker_merge_adjacent() {
        let mut tracker = DamageTracker::new();
        tracker.mark_dirty(1, 5);
        tracker.mark_dirty(6, 10);
        assert_eq!(tracker.regions().len(), 1);
        assert_eq!(tracker.regions()[0].start_line, 1);
        assert_eq!(tracker.regions()[0].end_line, 10);
    }

    #[test]
    fn damage_tracker_dirty_line_count() {
        let mut tracker = DamageTracker::new();
        tracker.mark_dirty(1, 5);
        tracker.mark_dirty(10, 12);
        assert_eq!(tracker.dirty_line_count(), 8); // 5 + 3
    }

    #[test]
    fn damage_tracker_clear() {
        let mut tracker = DamageTracker::new();
        tracker.mark_dirty(1, 10);
        tracker.clear();
        assert!(!tracker.is_dirty());
    }

    #[test]
    fn damage_tracker_display() {
        let mut tracker = DamageTracker::new();
        tracker.mark_dirty(1, 5);
        let s = tracker.to_string();
        assert!(s.contains("1 regions"));
        assert!(s.contains("5 dirty lines"));
    }

    #[test] fn viewpartBracketGuides_new() { let s = ViewpartBracketGuides::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn viewpartBracketGuides_add() { let mut s = ViewpartBracketGuides::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn viewpartBracketGuides_remove() { let mut s = ViewpartBracketGuides::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn viewpartBracketGuides_config() { let mut s = ViewpartBracketGuides::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn viewpartBracketGuides_nav() { let mut s = ViewpartBracketGuides::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn viewpartBracketGuides_filter() { let mut s = ViewpartBracketGuides::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn viewpartBracketGuides_display() { assert!(format!("{}", ViewpartBracketGuides::new()).contains("ViewpartBracketGuides")); }
    #[test] fn viewpartFoldIndicators_new() { let s = ViewpartFoldIndicators::new(); assert!(s.is_empty()); }
    #[test] fn viewpartFoldIndicators_add() { let mut s = ViewpartFoldIndicators::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn viewpartFoldIndicators_active() { let mut s = ViewpartFoldIndicators::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn viewpartFoldIndicators_error() { let mut s = ViewpartFoldIndicators::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn viewpartFoldIndicators_rm_group() { let mut s = ViewpartFoldIndicators::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn viewpartFoldIndicators_display() { assert!(format!("{}", ViewpartFoldIndicators::new()).contains("ViewpartFoldIndicators")); }


    #[test] fn viewpartBracketGuides_snap_capture() {
        let s = ViewpartBracketGuides::new();
        let snap = ViewpartBracketGuidesSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn viewpartBracketGuides_snap_stale() {
        let s = ViewpartBracketGuides::new();
        let snap = ViewpartBracketGuidesSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn viewpartBracketGuides_snap_diff() {
        let s = ViewpartBracketGuides::new();
        let s1v = ViewpartBracketGuidesSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn viewpartBracketGuides_snap_display() {
        let s = ViewpartBracketGuides::new();
        let snap = ViewpartBracketGuidesSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn viewpartFoldIndicators_stats_record() {
        let mut st = ViewpartFoldIndicatorsStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn viewpartFoldIndicators_stats_hit_ratio() {
        let mut st = ViewpartFoldIndicatorsStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn viewpartFoldIndicators_stats_merge() {
        let mut a = ViewpartFoldIndicatorsStats::new();
        a.total_adds = 5;
        let mut b = ViewpartFoldIndicatorsStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn viewpartFoldIndicators_stats_display() {
        let st = ViewpartFoldIndicatorsStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn viewpartBracketGuides_config_default() {
        let c = ViewpartBracketGuidesConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn viewpartBracketGuides_config_builder() {
        let c = ViewpartBracketGuidesConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn viewpartBracketGuides_config_labels() {
        let mut c = ViewpartBracketGuidesConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn viewpartBracketGuides_config_cleanup_threshold() {
        let c = ViewpartBracketGuidesConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn viewpartBracketGuides_config_display() {
        assert!(format!("{}", ViewpartBracketGuidesConfig::new()).contains("Config"));
    }
    #[test] fn viewpartFoldIndicators_stats_peaks() {
        let mut st = ViewpartFoldIndicatorsStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

}
