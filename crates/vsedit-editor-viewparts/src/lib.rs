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

// ── GlyphMarginCalculator ────────────────────────────────────────────────

/// Computes glyph margin widths based on decorations and line count.
pub struct GlyphMarginCalculator;

impl GlyphMarginCalculator {
    /// Compute the margin width in pixels from the number of decoration lanes.
    pub fn margin_width(decoration_lanes: usize, lane_width: u32) -> u32 {
        if decoration_lanes == 0 { return 0; }
        decoration_lanes as u32 * lane_width
    }

    /// Return the maximum decoration count from a list of per-line decoration counts.
    pub fn max_decoration_count(per_line_counts: &[usize]) -> usize {
        per_line_counts.iter().copied().max().unwrap_or(0)
    }

    /// Compute the gutter width needed for line numbers of the given total lines.
    pub fn gutter_width_for_line_count(total_lines: usize, char_width: u32) -> u32 {
        if total_lines == 0 { return char_width; }
        let digits = (total_lines as f64).log10().floor() as u32 + 1;
        digits * char_width + char_width // extra padding
    }

    /// Total left margin = glyph margin + line number gutter.
    pub fn total_left_margin(decoration_lanes: usize, lane_width: u32, total_lines: usize, char_width: u32) -> u32 {
        Self::margin_width(decoration_lanes, lane_width) + Self::gutter_width_for_line_count(total_lines, char_width)
    }
}

// ── LineNumberFormatter ─────────────────────────────────────────────────

/// Formats line numbers with padding and optional relative numbering.
pub struct LineNumberFormatter;

impl LineNumberFormatter {
    /// Format a line number with left-padding to the given width.
    pub fn format_line_number(line: usize, width: usize) -> String {
        format!("{:>width$}", line, width = width)
    }

    /// Format as a relative line number (distance from current line).
    pub fn format_relative(line: usize, current_line: usize, width: usize) -> String {
        if line == current_line {
            Self::format_line_number(line, width)
        } else {
            let diff = if line > current_line { line - current_line } else { current_line - line };
            format!("{:>width$}", diff, width = width)
        }
    }

    /// Returns a fold indicator string: "▶" for collapsed, "▼" for expanded, " " for none.
    pub fn fold_indicator(is_foldable: bool, is_collapsed: bool) -> &'static str {
        if !is_foldable { " " }
        else if is_collapsed { "▶" }
        else { "▼" }
    }

    /// Compute the display width needed for line numbers.
    pub fn required_width(max_line: usize) -> usize {
        if max_line == 0 { return 1; }
        (max_line as f64).log10().floor() as usize + 1
    }
}

// ── RulerRenderer ───────────────────────────────────────────────────────

/// Computes column ruler positions.
#[derive(Debug, Clone)]
pub struct RulerRenderer {
    ruler_columns: Vec<usize>,
}

impl RulerRenderer {
    pub fn new(ruler_columns: Vec<usize>) -> Self {
        let mut cols = ruler_columns;
        cols.sort_unstable();
        cols.dedup();
        Self { ruler_columns: cols }
    }

    pub fn ruler_count(&self) -> usize { self.ruler_columns.len() }

    /// Returns the rulers visible within the given column range [start, end).
    pub fn visible_rulers_in_range(&self, start_col: usize, end_col: usize) -> Vec<usize> {
        self.ruler_columns.iter().copied().filter(|&c| c >= start_col && c < end_col).collect()
    }

    /// Check if there is a ruler at the given column.
    pub fn ruler_at_column(&self, col: usize) -> bool {
        self.ruler_columns.contains(&col)
    }

    pub fn columns(&self) -> &[usize] { &self.ruler_columns }

    /// Maximum ruler column, if any.
    pub fn max_column(&self) -> Option<usize> {
        self.ruler_columns.last().copied()
    }
}

impl fmt::Display for RulerRenderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rulers({:?})", self.ruler_columns)
    }
}


// ---------------------------------------------------------------------------
// editor_viewparts – Editor text helpers
// ---------------------------------------------------------------------------

/// A half-open range within a document `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XEditorViewpartsTextSpan {
    pub start: usize,
    pub end: usize,
}

impl XEditorViewpartsTextSpan {
    pub fn new(start: usize, end: usize) -> Self {
        let (s, e) = if start <= end { (start, end) } else { (end, start) };
        Self { start: s, end: e }
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Extract the spanned slice from `text`.
    pub fn extract<'a>(&self, text: &'a str) -> &'a str {
        &text[self.start..self.end]
    }

    /// Returns true if `pos` is contained within this span.
    pub fn contains(&self, pos: usize) -> bool {
        pos >= self.start && pos < self.end
    }

    /// Returns the overlap with `other`, if any.
    pub fn intersect(&self, other: &Self) -> Option<Self> {
        let s = self.start.max(other.start);
        let e = self.end.min(other.end);
        if s < e { Some(Self { start: s, end: e }) } else { None }
    }

    /// Merge two spans into the smallest enclosing span.
    pub fn union(&self, other: &Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Shift the span by `delta` positions to the right.
    pub fn shift(&self, delta: usize) -> Self {
        Self { start: self.start + delta, end: self.end + delta }
    }
}

/// Count the number of lines in `text`.
pub fn x_editor_viewparts_count_lines(text: &str) -> usize {
    if text.is_empty() { return 0; }
    text.lines().count()
}

/// Return the byte offset of the start of line `n` (0-based).
pub fn x_editor_viewparts_line_start_offset(text: &str, line: usize) -> Option<usize> {
    let mut current = 0usize;
    for (i, l) in text.split('\n').enumerate() {
        if i == line { return Some(current); }
        current += l.len() + 1;
    }
    None
}

/// Compute the indentation level (number of leading spaces) of a line.
pub fn x_editor_viewparts_indent_level(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

/// Trim trailing whitespace from every line in `text`.
pub fn x_editor_viewparts_trim_trailing(text: &str) -> String {
    text.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Detect the dominant line ending in `text` (`"\n"` or `"\r\n"`).
pub fn x_editor_viewparts_detect_eol(text: &str) -> &'static str {
    let crlf = text.matches("\r\n").count();
    let lf = text.matches('\n').count().saturating_sub(crlf);
    if crlf > lf { "\r\n" } else { "\n" }
}

/// Simple word-boundary based tokenizer: split on whitespace and punctuation.
pub fn x_editor_viewparts_tokenize(text: &str) -> Vec<&str> {
    text.split(|c: char| c.is_whitespace() || ".,;:!?()[]{}".contains(c))
        .filter(|s| !s.is_empty())
        .collect()
}



// ---------------------------------------------------------------------------
// editor_viewparts – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for editor view parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YEditorViewpartsViewPartZone {
    Top,
    Bottom,
    Left,
    Right,
}

impl YEditorViewpartsViewPartZone {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Top => 0,
            Self::Bottom => 1,
            Self::Left => 2,
            Self::Right => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Top => "Top",
            Self::Bottom => "Bottom",
            Self::Left => "Left",
            Self::Right => "Right",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YEditorViewpartsViewPartZone] {
        &[
            YEditorViewpartsViewPartZone::Top,
            YEditorViewpartsViewPartZone::Bottom,
            YEditorViewpartsViewPartZone::Left,
            YEditorViewpartsViewPartZone::Right,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YEditorViewpartsViewPartZone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks view parts data.
#[derive(Debug, Clone)]
pub struct YEditorViewpartsViewPartRegistry {
    pub parts: Vec<(String, bool)>,
    pub visible_count: usize,
    pub layout_version: u32,
}

impl YEditorViewpartsViewPartRegistry {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            parts: Vec::new(),
            visible_count: 0,
            layout_version: 0,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.parts.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YEditorViewpartsViewPartRegistry({}: {:?})", "parts", self.parts)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_editor_viewparts_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_editor_viewparts_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_editor_viewparts_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_editor_viewparts_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_editor_viewparts_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_editor_viewparts_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_editor_viewparts_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_editor_viewparts_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// editor_viewparts – Extended view part snapshot helpers
// ---------------------------------------------------------------------------

/// Priority levels for view part snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZEditorViewpartsPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZEditorViewpartsPriority {
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
    pub fn all_asc() -> [ZEditorViewpartsPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZEditorViewpartsPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks view part snapshot data.
#[derive(Debug, Clone)]
pub struct ZEditorViewpartsViewPartSnapshot {
    pub part_ids: Vec<String>,
    pub timestamp_ms: u64,
    pub layout_hash: u64,
}

impl ZEditorViewpartsViewPartSnapshot {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            part_ids: Vec::new(),
            timestamp_ms: 0,
            layout_hash: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.part_ids.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.part_ids.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.part_ids.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZEditorViewpartsViewPartSnapshot[timestamp_ms={:?}, layout_hash={:?}]", self.timestamp_ms, self.layout_hash)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for view part snapshot.
pub fn z_editor_viewparts_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_editor_viewparts_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_editor_viewparts_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_editor_viewparts_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_editor_viewparts_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_editor_viewparts_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_editor_viewparts_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 81
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer81 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer81 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_81(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_81<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_81<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_81(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_81(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 40
// ---------------------------------------------------------------------------

/// Generic object pool `Xc40Pool<T>`.
pub struct Xc40Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc40Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc40PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc40Pool<T> {
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
    pub fn stats(&self) -> Xc40PoolStats {
        Xc40PoolStats {
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

impl<T> Default for Xc40Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc40Scheduler`.
pub struct Xc40Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc40Scheduler {
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

impl Default for Xc40Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_40 hash for the given byte slice.
pub fn xc_40_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_40 convention.
pub fn xc_40_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe94 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe94Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe94PipelineError {
    pub stage: Xe94Stage,
    pub message: String,
}

impl std::fmt::Display for Xe94PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe94Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe94Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe94PipelineError>>>,
    stage_names: Vec<Xe94Stage>,
}

impl Xe94Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe94PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe94Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe94PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe94Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe94PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe94Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe94PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe94Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe94PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe94Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe94CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe94CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe94Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe94CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe94CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe94Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe94CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_94_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe94CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_94_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe94CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_94_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe94PipelineError> {
    Ok(data)
}

pub fn xe_94_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe94PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_94_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe94PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_94_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe94PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_94_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe94PipelineError> {
    Err(Xe94PipelineError {
        stage: Xe94Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_92: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg92Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg92Graph {
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

impl Default for Xg92Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_92: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg92Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg92Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg92Heap<T>) {
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

impl<T: Ord> Default for Xg92Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 39).
pub struct Xh39SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh39SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 81 as u64,
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

/// A compact bit set supporting boolean operations (variant 39).
pub struct Xh39BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh39BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 39).
pub struct Xi39Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi39Deque<T> {
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
pub struct Xi39Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi39Interval {
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

/// A simple interval tree (variant 39).
pub struct Xi39IntervalTree {
    xi_intervals: Vec<Xi39Interval>,
}

impl Xi39IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi39Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi39Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi39Interval) -> Vec<&Xi39Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi39Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi39Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi39Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi39Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi39Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi39Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 39) ---

/// Disjoint set / union-find for crate 39.
pub struct Xj39UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj39UnionFind {
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

const XJ39_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 39.
pub struct Xj39BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj39BTreeNode<K, V>>>,
    len: usize,
}

struct Xj39BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj39BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj39BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ39_BTREE_ORDER - 1
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
        let mid = XJ39_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj39BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj39BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj39BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj39BTreeNode::xj_new_leaf();
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


// --- xk_39 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk39SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk39SegmentTree {
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
pub struct Xk39DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk39DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_39).
#[derive(Debug, Clone)]
pub struct Xl39Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl39Rope {
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

/// Suffix array for efficient string searching (xl_39).
#[derive(Debug, Clone)]
pub struct Xl39SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl39SuffixArray {
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


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm39MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm39MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm39Tokenizer {
    text: String,
}

impl Xm39Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 39.
pub struct Xn39Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn39Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 39 -----

#[derive(Debug, Clone)]
struct Xn39AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn39AvlNode<K, V>>>,
    right: Option<Box<Xn39AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 39.
#[derive(Debug, Clone)]
pub struct Xn39AVL<K, V> {
    root: Option<Box<Xn39AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn39AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn39AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn39AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn39AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn39AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn39AvlNode<K, V>>) -> Box<Xn39AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn39AvlNode<K, V>>) -> Box<Xn39AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn39AvlNode<K, V>>) -> Box<Xn39AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn39AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn39AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn39AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn39AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn39AvlNode<K, V>>) -> &Xn39AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn39AvlNode<K, V>>) -> (Box<Xn39AvlNode<K, V>>, Option<Box<Xn39AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn39AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn39AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn39AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn39AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn39AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn39AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn39AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo39RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo39Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo39RBNode<K, V> {
    key: K,
    value: V,
    color: Xo39Color,
    left: Option<Box<Xo39RBNode<K, V>>>,
    right: Option<Box<Xo39RBNode<K, V>>>,
}

/// A red-black tree map for crate 39.
#[derive(Debug, Clone)]
pub struct Xo39RedBlack<K, V> {
    root: Option<Box<Xo39RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo39RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo39Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo39RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo39RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo39RBNode {
                    key, value, color: Xo39Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo39RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo39Color::Red)
    }

    fn xo_balance(mut h: Box<Xo39RBNode<K, V>>) -> Box<Xo39RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo39Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo39RBNode<K, V>>) -> Box<Xo39RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo39Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo39RBNode<K, V>>) -> Box<Xo39RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo39Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo39RBNode<K, V>>) {
        h.color = Xo39Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo39Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo39Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo39Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo39RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo39RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo39RBNode<K, V>) -> (K, V, Option<Box<Xo39RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo39RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo39Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo39RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo39ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 39.
#[derive(Debug, Clone)]
pub struct Xo39ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo39ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo39#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo39#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 39).
#[derive(Debug)]
pub struct Xp39SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp39Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp39Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp39Node<K, V>>>,
    xp_right: Option<Box<Xp39Node<K, V>>>,
}

impl<K: Ord, V> Xp39Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp39SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp39SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp39Node<K, V>>>, key: &K) -> Option<Box<Xp39Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp39Node<K, V>>) -> Box<Xp39Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp39Node<K, V>>) -> Box<Xp39Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp39Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp39Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp39Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq39Treap ---------------

use std::cmp::Ordering as Xq39Ord;

struct Xq39TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq39TreapNode<K, V>>>,
    right: Option<Box<Xq39TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq39Treap<K, V> {
    root: Option<Box<Xq39TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq39TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_39_size<K, V>(node: &Option<Box<Xq39TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_39_update_size<K, V>(node: &mut Xq39TreapNode<K, V>) {
    node.size = 1 + xq_39_size(&node.left) + xq_39_size(&node.right);
}

fn xq_39_rotate_right<K, V>(mut node: Box<Xq39TreapNode<K, V>>) -> Box<Xq39TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_39_update_size(&mut node);
    left.right = Some(node);
    xq_39_update_size(&mut left);
    left
}

fn xq_39_rotate_left<K, V>(mut node: Box<Xq39TreapNode<K, V>>) -> Box<Xq39TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_39_update_size(&mut node);
    right.left = Some(node);
    xq_39_update_size(&mut right);
    right
}

fn xq_39_insert_node<K: Ord, V>(
    node: Option<Box<Xq39TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq39TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq39TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq39Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq39Ord::Less => {
                let (new_left, old) = xq_39_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_39_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_39_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq39Ord::Greater => {
                let (new_right, old) = xq_39_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_39_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_39_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_39_remove_node<K: Ord, V>(
    node: Option<Box<Xq39TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq39TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq39Ord::Less => {
                let (new_left, old) = xq_39_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_39_update_size(&mut n);
                (Some(n), old)
            }
            Xq39Ord::Greater => {
                let (new_right, old) = xq_39_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_39_update_size(&mut n);
                (Some(n), old)
            }
            Xq39Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_39_rotate_right(n);
                    let (new_right, old) = xq_39_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_39_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_39_rotate_left(n);
                    let (new_left, old) = xq_39_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_39_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_39_find_min<K, V>(node: &Option<Box<Xq39TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_39_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_39_find_max<K, V>(node: &Option<Box<Xq39TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_39_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_39_rank<K: Ord, V>(node: &Option<Box<Xq39TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq39Ord::Less => xq_39_rank(&n.left, key),
            Xq39Ord::Equal => xq_39_size(&n.left),
            Xq39Ord::Greater => 1 + xq_39_size(&n.left) + xq_39_rank(&n.right, key),
        },
    }
}

fn xq_39_kth<K, V>(node: &Option<Box<Xq39TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_39_size(&n.left);
        if k < left_size {
            xq_39_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_39_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_39_in_order<K: Clone, V>(node: &Option<Box<Xq39TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_39_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_39_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq39Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 39 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_39_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq39Ord::Equal => return Some(&n.value),
                Xq39Ord::Less => cur = &n.left,
                Xq39Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_39_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_39_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_39_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_39_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_39_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_39_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_39_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq39VEBTree ---------------

pub struct Xq39VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq39VEBTree>>,
    clusters: Vec<Option<Box<Xq39VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq39VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq39VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq39VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr39KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr39KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr39BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr39KDNode {
    xr_point: Xr39KDPoint,
    xr_left: Option<Box<Xr39KDNode>>,
    xr_right: Option<Box<Xr39KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr39KDTree {
    xr_root: Option<Box<Xr39KDNode>>,
    xr_size: usize,
}

impl Xr39KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr39KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr39KDNode>>,
        point: Xr39KDPoint,
        depth: usize,
    ) -> Box<Xr39KDNode> {
        match node {
            None => Box::new(Xr39KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr39KDPoint) -> Option<Xr39KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr39KDNode>,
        query: &Xr39KDPoint,
        depth: usize,
        best: &mut Xr39KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr39KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr39KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr39KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr39KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr39KDNode>>, pts: &mut Vec<Xr39KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr39KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr39BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr39BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs39PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs39PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs39PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs39PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs39ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs39ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs39ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs39RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs39RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs39RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs39CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs39CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs39CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
    }
}


// --- xt_ Fibonacci Heap ---

/// A node in a Fibonacci heap, storing a key and value with parent/child/sibling pointers.
#[derive(Debug, Clone)]
pub struct XtFibNode<K: Ord + Clone, V: Clone> {
    pub xt_key: K,
    pub xt_value: V,
    xt_degree: usize,
    xt_marked: bool,
    xt_children: Vec<usize>,
    xt_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XtFibNode<K, V> {
    /// Create a new Fibonacci heap node.
    pub fn xt_new(key: K, value: V) -> Self {
        Self {
            xt_key: key,
            xt_value: value,
            xt_degree: 0,
            xt_marked: false,
            xt_children: Vec::new(),
            xt_parent: None,
        }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibNode(key={}, val={}, deg={})", self.xt_key, self.xt_value, self.xt_degree)
    }
}

/// Fibonacci heap with lazy consolidation for amortized O(1) insert and decrease-key.
#[derive(Debug, Clone)]
pub struct XtFibonacciHeap<K: Ord + Clone, V: Clone> {
    xt_nodes: Vec<XtFibNode<K, V>>,
    xt_roots: Vec<usize>,
    xt_min_idx: Option<usize>,
    xt_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XtFibonacciHeap<K, V> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibonacciHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibHeap(size={}, roots={})", self.xt_size, self.xt_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XtFibonacciHeap<K, V> {
    /// Create an empty Fibonacci heap.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_roots: Vec::new(),
            xt_min_idx: None,
            xt_size: 0,
        }
    }

    /// Return the number of elements.
    pub fn xt_len(&self) -> usize {
        self.xt_size
    }

    /// Check if the heap is empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_size == 0
    }

    /// Insert a key-value pair, returning its node index.
    pub fn xt_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xt_nodes.len();
        self.xt_nodes.push(XtFibNode::xt_new(key, value));
        self.xt_roots.push(idx);
        match self.xt_min_idx {
            None => self.xt_min_idx = Some(idx),
            Some(mi) => {
                if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                    self.xt_min_idx = Some(idx);
                }
            }
        }
        self.xt_size += 1;
        idx
    }

    /// Peek at the minimum key-value pair.
    pub fn xt_find_min(&self) -> Option<(&K, &V)> {
        self.xt_min_idx.map(|i| (&self.xt_nodes[i].xt_key, &self.xt_nodes[i].xt_value))
    }

    /// Extract the minimum element.
    pub fn xt_extract_min(&mut self) -> Option<(K, V)> {
        let mi = self.xt_min_idx?;
        let children = self.xt_nodes[mi].xt_children.clone();
        for &c in &children {
            self.xt_nodes[c].xt_parent = None;
            self.xt_roots.push(c);
        }
        self.xt_roots.retain(|&r| r != mi);
        if self.xt_roots.is_empty() {
            self.xt_min_idx = None;
        } else {
            self.xt_min_idx = Some(self.xt_roots[0]);
            self.xt_consolidate();
        }
        self.xt_size -= 1;
        let node = &self.xt_nodes[mi];
        Some((node.xt_key.clone(), node.xt_value.clone()))
    }

    fn xt_consolidate(&mut self) {
        let max_deg = (self.xt_size as f64).log2().ceil() as usize + 2;
        let mut degree_table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xt_roots.clone();
        self.xt_roots.clear();
        for root in roots {
            let mut x = root;
            let mut d = self.xt_nodes[x].xt_degree;
            while d < degree_table.len() {
                if let Some(y) = degree_table[d] {
                    degree_table[d] = None;
                    let (parent, child) = if self.xt_nodes[x].xt_key <= self.xt_nodes[y].xt_key {
                        (x, y)
                    } else {
                        (y, x)
                    };
                    self.xt_nodes[parent].xt_children.push(child);
                    self.xt_nodes[child].xt_parent = Some(parent);
                    self.xt_nodes[parent].xt_degree += 1;
                    self.xt_nodes[child].xt_marked = false;
                    x = parent;
                    d = self.xt_nodes[x].xt_degree;
                } else {
                    break;
                }
            }
            if d < degree_table.len() {
                degree_table[d] = Some(x);
            }
            self.xt_roots.push(x);
        }
        self.xt_roots.sort();
        self.xt_roots.dedup();
        self.xt_min_idx = self.xt_roots.iter().copied()
            .min_by(|&a, &b| self.xt_nodes[a].xt_key.cmp(&self.xt_nodes[b].xt_key));
    }

    /// Decrease the key of a node (key must be smaller than current).
    pub fn xt_decrease_key(&mut self, idx: usize, new_key: K) {
        if new_key >= self.xt_nodes[idx].xt_key {
            return;
        }
        self.xt_nodes[idx].xt_key = new_key;
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[p].xt_key {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
        if let Some(mi) = self.xt_min_idx {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                self.xt_min_idx = Some(idx);
            }
        }
    }

    fn xt_cut(&mut self, x: usize, p: usize) {
        self.xt_nodes[p].xt_children.retain(|&c| c != x);
        self.xt_nodes[p].xt_degree = self.xt_nodes[p].xt_children.len();
        self.xt_nodes[x].xt_parent = None;
        self.xt_nodes[x].xt_marked = false;
        self.xt_roots.push(x);
    }

    fn xt_cascading_cut(&mut self, idx: usize) {
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if !self.xt_nodes[idx].xt_marked {
                self.xt_nodes[idx].xt_marked = true;
            } else {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
    }

    /// Merge another Fibonacci heap into this one.
    pub fn xt_merge(&mut self, other: &mut XtFibonacciHeap<K, V>) {
        let offset = self.xt_nodes.len();
        for mut node in other.xt_nodes.drain(..) {
            node.xt_parent = node.xt_parent.map(|p| p + offset);
            node.xt_children = node.xt_children.iter().map(|&c| c + offset).collect();
            self.xt_nodes.push(node);
        }
        for r in other.xt_roots.drain(..) {
            self.xt_roots.push(r + offset);
        }
        match (self.xt_min_idx, other.xt_min_idx) {
            (None, Some(oi)) => self.xt_min_idx = Some(oi + offset),
            (Some(si), Some(oi)) => {
                let oi2 = oi + offset;
                if self.xt_nodes[oi2].xt_key < self.xt_nodes[si].xt_key {
                    self.xt_min_idx = Some(oi2);
                }
            }
            _ => {}
        }
        self.xt_size += other.xt_size;
        other.xt_size = 0;
        other.xt_min_idx = None;
    }

    /// Return all keys in sorted order (destructive).
    pub fn xt_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xt_size);
        while let Some(pair) = self.xt_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_roots.clear();
        self.xt_min_idx = None;
        self.xt_size = 0;
    }
}

// --- xt_ Doubly-Linked List with Cursors ---

/// A node in a doubly-linked list with prev/next indices.
#[derive(Debug, Clone)]
pub struct XtDllNode<T: Clone> {
    pub xt_value: T,
    xt_prev: Option<usize>,
    xt_next: Option<usize>,
    xt_active: bool,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDllNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DllNode({})", self.xt_value)
    }
}

/// Doubly-linked list with O(1) insertion/deletion at any position via cursor indices.
#[derive(Debug, Clone)]
pub struct XtDoublyLinkedList<T: Clone> {
    xt_nodes: Vec<XtDllNode<T>>,
    xt_head: Option<usize>,
    xt_tail: Option<usize>,
    xt_len: usize,
    xt_free: Vec<usize>,
}

impl<T: Clone> Default for XtDoublyLinkedList<T> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDoublyLinkedList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DLL(len={})", self.xt_len)
    }
}

impl<T: Clone> XtDoublyLinkedList<T> {
    /// Create an empty doubly-linked list.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_head: None,
            xt_tail: None,
            xt_len: 0,
            xt_free: Vec::new(),
        }
    }

    /// Return the length.
    pub fn xt_len(&self) -> usize {
        self.xt_len
    }

    /// Check if empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_len == 0
    }

    fn xt_alloc(&mut self, value: T) -> usize {
        if let Some(idx) = self.xt_free.pop() {
            self.xt_nodes[idx] = XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            };
            idx
        } else {
            let idx = self.xt_nodes.len();
            self.xt_nodes.push(XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            });
            idx
        }
    }

    /// Push a value to the front, returning its index.
    pub fn xt_push_front(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_head {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_head) => {
                self.xt_nodes[idx].xt_next = Some(old_head);
                self.xt_nodes[old_head].xt_prev = Some(idx);
                self.xt_head = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Push a value to the back, returning its index.
    pub fn xt_push_back(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_tail {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_tail) => {
                self.xt_nodes[idx].xt_prev = Some(old_tail);
                self.xt_nodes[old_tail].xt_next = Some(idx);
                self.xt_tail = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value after the given index, returning the new index.
    pub fn xt_insert_after(&mut self, after: usize, value: T) -> usize {
        if !self.xt_nodes[after].xt_active {
            return self.xt_push_back(value);
        }
        let idx = self.xt_alloc(value);
        let next = self.xt_nodes[after].xt_next;
        self.xt_nodes[after].xt_next = Some(idx);
        self.xt_nodes[idx].xt_prev = Some(after);
        self.xt_nodes[idx].xt_next = next;
        if let Some(n) = next {
            self.xt_nodes[n].xt_prev = Some(idx);
        } else {
            self.xt_tail = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value before the given index, returning the new index.
    pub fn xt_insert_before(&mut self, before: usize, value: T) -> usize {
        if !self.xt_nodes[before].xt_active {
            return self.xt_push_front(value);
        }
        let idx = self.xt_alloc(value);
        let prev = self.xt_nodes[before].xt_prev;
        self.xt_nodes[before].xt_prev = Some(idx);
        self.xt_nodes[idx].xt_next = Some(before);
        self.xt_nodes[idx].xt_prev = prev;
        if let Some(p) = prev {
            self.xt_nodes[p].xt_next = Some(idx);
        } else {
            self.xt_head = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Remove the node at the given index.
    pub fn xt_remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.xt_nodes.len() || !self.xt_nodes[idx].xt_active {
            return None;
        }
        let prev = self.xt_nodes[idx].xt_prev;
        let next = self.xt_nodes[idx].xt_next;
        match prev {
            Some(p) => self.xt_nodes[p].xt_next = next,
            None => self.xt_head = next,
        }
        match next {
            Some(n) => self.xt_nodes[n].xt_prev = prev,
            None => self.xt_tail = prev,
        }
        self.xt_nodes[idx].xt_active = false;
        self.xt_nodes[idx].xt_prev = None;
        self.xt_nodes[idx].xt_next = None;
        self.xt_free.push(idx);
        self.xt_len -= 1;
        Some(self.xt_nodes[idx].xt_value.clone())
    }

    /// Pop from front.
    pub fn xt_pop_front(&mut self) -> Option<T> {
        self.xt_head.and_then(|h| self.xt_remove(h))
    }

    /// Pop from back.
    pub fn xt_pop_back(&mut self) -> Option<T> {
        self.xt_tail.and_then(|t| self.xt_remove(t))
    }

    /// Peek at the front value.
    pub fn xt_peek_front(&self) -> Option<&T> {
        self.xt_head.map(|h| &self.xt_nodes[h].xt_value)
    }

    /// Peek at the back value.
    pub fn xt_peek_back(&self) -> Option<&T> {
        self.xt_tail.map(|t| &self.xt_nodes[t].xt_value)
    }

    /// Get value at a given index.
    pub fn xt_get(&self, idx: usize) -> Option<&T> {
        if idx < self.xt_nodes.len() && self.xt_nodes[idx].xt_active {
            Some(&self.xt_nodes[idx].xt_value)
        } else {
            None
        }
    }

    /// Iterate from head to tail.
    pub fn xt_iter_forward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_next;
        }
        result
    }

    /// Iterate from tail to head.
    pub fn xt_iter_backward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_tail;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_prev;
        }
        result
    }

    /// Collect all values into a Vec (front to back).
    pub fn xt_to_vec(&self) -> Vec<T> {
        self.xt_iter_forward().into_iter().cloned().collect()
    }

    /// Clear the list.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_head = None;
        self.xt_tail = None;
        self.xt_len = 0;
        self.xt_free.clear();
    }

    /// Return the head cursor index.
    pub fn xt_head_cursor(&self) -> Option<usize> {
        self.xt_head
    }

    /// Return the tail cursor index.
    pub fn xt_tail_cursor(&self) -> Option<usize> {
        self.xt_tail
    }

    /// Move cursor to next.
    pub fn xt_cursor_next(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_next
        } else {
            None
        }
    }

    /// Move cursor to prev.
    pub fn xt_cursor_prev(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_prev
        } else {
            None
        }
    }

    /// Reverse the list in place.
    pub fn xt_reverse(&mut self) {
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            let next = self.xt_nodes[idx].xt_next;
            let prev = self.xt_nodes[idx].xt_prev;
            self.xt_nodes[idx].xt_next = prev;
            self.xt_nodes[idx].xt_prev = next;
            cur = next;
        }
        std::mem::swap(&mut self.xt_head, &mut self.xt_tail);
    }
}


// --- xu_ Binomial Heap ---

/// A node in a binomial heap.
#[derive(Debug, Clone)]
pub struct XuBinomialNode<K: Ord + Clone, V: Clone> {
    pub xu_key: K,
    pub xu_value: V,
    xu_degree: usize,
    xu_children: Vec<usize>,
    xu_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XuBinomialNode<K, V> {
    /// Create a new binomial node.
    pub fn xu_new(key: K, value: V) -> Self {
        Self { xu_key: key, xu_value: value, xu_degree: 0, xu_children: Vec::new(), xu_parent: None }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinNode(key={}, deg={})", self.xu_key, self.xu_degree)
    }
}

/// Binomial heap with O(log n) insert, extract-min, and merge.
#[derive(Debug, Clone)]
pub struct XuBinomialHeap<K: Ord + Clone, V: Clone> {
    xu_nodes: Vec<XuBinomialNode<K, V>>,
    xu_roots: Vec<usize>,
    xu_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XuBinomialHeap<K, V> {
    fn default() -> Self { Self::xu_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinHeap(size={}, trees={})", self.xu_size, self.xu_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XuBinomialHeap<K, V> {
    /// Create an empty binomial heap.
    pub fn xu_new() -> Self {
        Self { xu_nodes: Vec::new(), xu_roots: Vec::new(), xu_size: 0 }
    }

    /// Return the number of elements.
    pub fn xu_len(&self) -> usize { self.xu_size }

    /// Check if the heap is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_size == 0 }

    /// Insert a key-value pair.
    pub fn xu_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xu_nodes.len();
        self.xu_nodes.push(XuBinomialNode::xu_new(key, value));
        self.xu_add_root(idx);
        self.xu_size += 1;
        self.xu_consolidate();
        idx
    }

    fn xu_add_root(&mut self, idx: usize) {
        self.xu_nodes[idx].xu_parent = None;
        self.xu_roots.push(idx);
    }

    fn xu_consolidate(&mut self) {
        let max_deg = (self.xu_size as f64).log2().ceil() as usize + 2;
        let mut table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xu_roots.clone();
        self.xu_roots.clear();
        for root in roots {
            let mut x = root;
            loop {
                let d = self.xu_nodes[x].xu_degree;
                if d >= table.len() { break; }
                match table[d] {
                    None => { table[d] = Some(x); break; }
                    Some(y) => {
                        table[d] = None;
                        let (p, c) = if self.xu_nodes[x].xu_key <= self.xu_nodes[y].xu_key { (x, y) } else { (y, x) };
                        self.xu_nodes[p].xu_children.push(c);
                        self.xu_nodes[c].xu_parent = Some(p);
                        self.xu_nodes[p].xu_degree += 1;
                        x = p;
                    }
                }
            }
        }
        for slot in &table {
            if let Some(r) = slot {
                self.xu_roots.push(*r);
            }
        }
        self.xu_roots.sort_by_key(|&r| self.xu_nodes[r].xu_degree);
    }

    /// Peek at the minimum.
    pub fn xu_find_min(&self) -> Option<(&K, &V)> {
        self.xu_roots.iter()
            .min_by(|&&a, &&b| self.xu_nodes[a].xu_key.cmp(&self.xu_nodes[b].xu_key))
            .map(|&i| (&self.xu_nodes[i].xu_key, &self.xu_nodes[i].xu_value))
    }

    /// Extract the minimum element.
    pub fn xu_extract_min(&mut self) -> Option<(K, V)> {
        if self.xu_roots.is_empty() { return None; }
        let min_pos = self.xu_roots.iter().enumerate()
            .min_by(|(_, a), (_, b)| self.xu_nodes[**a].xu_key.cmp(&self.xu_nodes[**b].xu_key))
            .map(|(pos, _)| pos)?;
        let min_idx = self.xu_roots.remove(min_pos);
        let children = self.xu_nodes[min_idx].xu_children.clone();
        for &c in &children {
            self.xu_nodes[c].xu_parent = None;
            self.xu_roots.push(c);
        }
        self.xu_size -= 1;
        if !self.xu_roots.is_empty() {
            self.xu_consolidate();
        }
        let n = &self.xu_nodes[min_idx];
        Some((n.xu_key.clone(), n.xu_value.clone()))
    }

    /// Merge another binomial heap into this one.
    pub fn xu_merge(&mut self, other: &mut XuBinomialHeap<K, V>) {
        let off = self.xu_nodes.len();
        for mut n in other.xu_nodes.drain(..) {
            n.xu_parent = n.xu_parent.map(|p| p + off);
            n.xu_children = n.xu_children.iter().map(|&c| c + off).collect();
            self.xu_nodes.push(n);
        }
        for r in other.xu_roots.drain(..) {
            self.xu_roots.push(r + off);
        }
        self.xu_size += other.xu_size;
        other.xu_size = 0;
        self.xu_consolidate();
    }

    /// Drain all elements in sorted order.
    pub fn xu_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xu_size);
        while let Some(pair) = self.xu_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xu_clear(&mut self) {
        self.xu_nodes.clear();
        self.xu_roots.clear();
        self.xu_size = 0;
    }
}

// --- xu_ Disjoint Sparse Table ---

/// Disjoint sparse table for O(1) range queries on static data with an associative operation.
#[derive(Debug, Clone)]
pub struct XuDisjointSparseTable<T: Clone> {
    xu_table: Vec<Vec<T>>,
    xu_data: Vec<T>,
    xu_len: usize,
    xu_levels: usize,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XuDisjointSparseTable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DST(len={}, levels={})", self.xu_len, self.xu_levels)
    }
}

impl<T: Clone + Default + std::ops::Add<Output = T>> XuDisjointSparseTable<T> {
    /// Build a disjoint sparse table for range-sum queries.
    pub fn xu_build(data: &[T]) -> Self {
        let n = data.len();
        if n == 0 {
            return Self { xu_table: Vec::new(), xu_data: Vec::new(), xu_len: 0, xu_levels: 0 };
        }
        let levels = (n as f64).log2().ceil() as usize + 1;
        let mut table = Vec::with_capacity(levels);
        for level in 0..levels {
            let block = 1 << level;
            let mut row = data.to_vec();
            let mut mid = block;
            while mid < n {
                // Build prefix sums going left from mid
                if mid > 0 && mid - 1 < n {
                    let start = if mid >= block { mid - block } else { 0 };
                    let mut i = mid.saturating_sub(1);
                    loop {
                        if i < start { break; }
                        if i + 1 < mid && i + 1 < n {
                            row[i] = row[i].clone() + row[i + 1].clone();
                        }
                        if i == start { break; }
                        i -= 1;
                    }
                }
                // Build prefix sums going right from mid
                let end = std::cmp::min(mid + block, n);
                for i in (mid + 1)..end {
                    row[i] = row[i - 1].clone() + row[i].clone();
                }
                mid += 2 * block;
            }
            table.push(row);
        }
        Self { xu_table: table, xu_data: data.to_vec(), xu_len: n, xu_levels: levels }
    }

    /// Query the sum of elements in the range [l, r] (inclusive).
    pub fn xu_query(&self, l: usize, r: usize) -> T {
        if l == r {
            return self.xu_data[l].clone();
        }
        if l >= self.xu_len || r >= self.xu_len || l > r {
            return T::default();
        }
        // Find the highest bit where l and r differ
        let xor = l ^ r;
        if xor == 0 {
            return self.xu_data[l].clone();
        }
        let level = (usize::BITS - xor.leading_zeros() - 1) as usize;
        if level < self.xu_levels && l < self.xu_table[level].len() && r < self.xu_table[level].len() {
            self.xu_table[level][l].clone() + self.xu_table[level][r].clone()
        } else {
            // Fallback: linear sum
            let mut sum = self.xu_data[l].clone();
            for i in (l + 1)..=r {
                sum = sum + self.xu_data[i].clone();
            }
            sum
        }
    }

    /// Return the length.
    pub fn xu_len(&self) -> usize { self.xu_len }

    /// Check if empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_len == 0 }

    /// Get element at index.
    pub fn xu_get(&self, idx: usize) -> Option<&T> {
        self.xu_data.get(idx)
    }
}

// --- xu_ Monotonic Stack ---

/// Monotonic stack that maintains elements in non-decreasing or non-increasing order.
#[derive(Debug, Clone)]
pub struct XuMonotonicStack<T: Clone + Ord> {
    xu_data: Vec<T>,
    xu_increasing: bool,
}

impl<T: Clone + Ord + std::fmt::Display> std::fmt::Display for XuMonotonicStack<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MonoStack(len={}, inc={})", self.xu_data.len(), self.xu_increasing)
    }
}

impl<T: Clone + Ord> XuMonotonicStack<T> {
    /// Create a monotonically increasing stack.
    pub fn xu_increasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: true }
    }

    /// Create a monotonically decreasing stack.
    pub fn xu_decreasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: false }
    }

    /// Push a value, popping elements that violate the monotonic invariant.
    pub fn xu_push(&mut self, value: T) -> Vec<T> {
        let mut popped = Vec::new();
        if self.xu_increasing {
            while let Some(top) = self.xu_data.last() {
                if *top > value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        } else {
            while let Some(top) = self.xu_data.last() {
                if *top < value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        }
        self.xu_data.push(value);
        popped
    }

    /// Peek at the top.
    pub fn xu_peek(&self) -> Option<&T> { self.xu_data.last() }

    /// Pop from top.
    pub fn xu_pop(&mut self) -> Option<T> { self.xu_data.pop() }

    /// Length.
    pub fn xu_len(&self) -> usize { self.xu_data.len() }

    /// Is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_data.is_empty() }

    /// Get all elements.
    pub fn xu_as_slice(&self) -> &[T] { &self.xu_data }

    /// Clear the stack.
    pub fn xu_clear(&mut self) { self.xu_data.clear(); }
}


// --- xv_ Cartesian Tree ---

/// A node in a Cartesian tree (BST by key, heap by priority).
#[derive(Debug, Clone)]
pub struct XvCartesianNode<K: Ord + Clone, P: Ord + Clone> {
    pub xv_key: K,
    pub xv_priority: P,
    xv_left: Option<Box<XvCartesianNode<K, P>>>,
    xv_right: Option<Box<XvCartesianNode<K, P>>>,
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianNode<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartNode(k={}, p={})", self.xv_key, self.xv_priority)
    }
}

/// Cartesian tree — BST by key, min-heap by priority. Used for range-minimum queries.
#[derive(Debug, Clone)]
pub struct XvCartesianTree<K: Ord + Clone, P: Ord + Clone> {
    xv_root: Option<Box<XvCartesianNode<K, P>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, P: Ord + Clone> Default for XvCartesianTree<K, P> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianTree<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, P: Ord + Clone> XvCartesianTree<K, P> {
    /// Create an empty Cartesian tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Return the number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Check if empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    /// Insert a (key, priority) pair maintaining BST-by-key and min-heap-by-priority.
    pub fn xv_insert(&mut self, key: K, priority: P) {
        self.xv_root = Self::xv_insert_node(self.xv_root.take(), key, priority);
        self.xv_size += 1;
    }

    fn xv_insert_node(node: Option<Box<XvCartesianNode<K, P>>>, key: K, priority: P) -> Option<Box<XvCartesianNode<K, P>>> {
        match node {
            None => Some(Box::new(XvCartesianNode { xv_key: key, xv_priority: priority, xv_left: None, xv_right: None })),
            Some(mut n) => {
                if key < n.xv_key {
                    n.xv_left = Self::xv_insert_node(n.xv_left.take(), key.clone(), priority.clone());
                    if n.xv_left.as_ref().is_some_and(|l| l.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_right(n);
                    }
                    Some(n)
                } else {
                    n.xv_right = Self::xv_insert_node(n.xv_right.take(), key.clone(), priority.clone());
                    if n.xv_right.as_ref().is_some_and(|r| r.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_left(n);
                    }
                    Some(n)
                }
            }
        }
    }

    fn xv_rotate_right(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        left.xv_right = Some(node);
        left
    }

    fn xv_rotate_left(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        right.xv_left = Some(node);
        right
    }

    /// Search for a key.
    pub fn xv_contains(&self, key: &K) -> bool {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search(node: &Option<Box<XvCartesianNode<K, P>>>, key: &K) -> bool {
        match node {
            None => false,
            Some(n) => {
                if *key == n.xv_key { true }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// In-order traversal returning keys.
    pub fn xv_inorder(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder_walk(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder_walk(node: &Option<Box<XvCartesianNode<K, P>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder_walk(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder_walk(&n.xv_right, result);
        }
    }

    /// Get the root priority (minimum priority).
    pub fn xv_min_priority(&self) -> Option<&P> {
        self.xv_root.as_ref().map(|n| &n.xv_priority)
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Build from a sequence of (key, priority) pairs.
    pub fn xv_from_pairs(pairs: &[(K, P)]) -> Self {
        let mut tree = Self::xv_new();
        for (k, p) in pairs { tree.xv_insert(k.clone(), p.clone()); }
        tree
    }

    /// Height of the tree.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvCartesianNode<K, P>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(
                Self::xv_node_height(&n.xv_left),
                Self::xv_node_height(&n.xv_right),
            ),
        }
    }
}

// --- xv_ Weight-Balanced Tree ---

/// A node in a weight-balanced tree (BB[α] tree).
#[derive(Debug, Clone)]
pub struct XvWBNode<K: Ord + Clone, V: Clone> {
    pub xv_key: K,
    pub xv_value: V,
    xv_left: Option<Box<XvWBNode<K, V>>>,
    xv_right: Option<Box<XvWBNode<K, V>>>,
    xv_weight: usize,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWBNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBNode(k={}, w={})", self.xv_key, self.xv_weight)
    }
}

/// Weight-balanced tree (BB[α] tree) with α = 0.29 for balanced operations.
#[derive(Debug, Clone)]
pub struct XvWeightBalancedTree<K: Ord + Clone, V: Clone> {
    xv_root: Option<Box<XvWBNode<K, V>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XvWeightBalancedTree<K, V> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWeightBalancedTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, V: Clone> XvWeightBalancedTree<K, V> {
    const ALPHA: f64 = 0.29;

    /// Create an empty weight-balanced tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Is the tree empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    fn xv_weight(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node { None => 1, Some(n) => n.xv_weight }
    }

    fn xv_update_weight(node: &mut Box<XvWBNode<K, V>>) {
        node.xv_weight = Self::xv_weight(&node.xv_left) + Self::xv_weight(&node.xv_right);
    }

    fn xv_is_balanced(node: &Box<XvWBNode<K, V>>) -> bool {
        let lw = Self::xv_weight(&node.xv_left) as f64;
        let rw = Self::xv_weight(&node.xv_right) as f64;
        let total = node.xv_weight as f64;
        lw >= Self::ALPHA * total && rw >= Self::ALPHA * total
    }

    /// Insert a key-value pair.
    pub fn xv_insert(&mut self, key: K, value: V) {
        let inserted = Self::xv_insert_node(self.xv_root.take(), key, value);
        self.xv_root = inserted.0;
        if inserted.1 { self.xv_size += 1; }
    }

    fn xv_insert_node(node: Option<Box<XvWBNode<K, V>>>, key: K, value: V) -> (Option<Box<XvWBNode<K, V>>>, bool) {
        match node {
            None => {
                let n = Box::new(XvWBNode { xv_key: key, xv_value: value, xv_left: None, xv_right: None, xv_weight: 2 });
                (Some(n), true)
            }
            Some(mut n) => {
                let inserted;
                if key < n.xv_key {
                    let r = Self::xv_insert_node(n.xv_left.take(), key, value);
                    n.xv_left = r.0;
                    inserted = r.1;
                } else if key > n.xv_key {
                    let r = Self::xv_insert_node(n.xv_right.take(), key, value);
                    n.xv_right = r.0;
                    inserted = r.1;
                } else {
                    n.xv_value = value;
                    return (Some(n), false);
                }
                Self::xv_update_weight(&mut n);
                let n = Self::xv_rebalance(n);
                (Some(n), inserted)
            }
        }
    }

    fn xv_rebalance(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if !Self::xv_is_balanced(&node) {
            let lw = Self::xv_weight(&node.xv_left);
            let rw = Self::xv_weight(&node.xv_right);
            if lw < rw {
                node = Self::xv_rotate_left_wb(node);
            } else {
                node = Self::xv_rotate_right_wb(node);
            }
        }
        node
    }

    fn xv_rotate_left_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_right.is_none() { return node; }
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        Self::xv_update_weight(&mut node);
        right.xv_left = Some(node);
        Self::xv_update_weight(&mut right);
        right
    }

    fn xv_rotate_right_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_left.is_none() { return node; }
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        Self::xv_update_weight(&mut node);
        left.xv_right = Some(node);
        Self::xv_update_weight(&mut left);
        left
    }

    /// Look up a key.
    pub fn xv_get(&self, key: &K) -> Option<&V> {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search<'a>(node: &'a Option<Box<XvWBNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xv_key { Some(&n.xv_value) }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xv_contains(&self, key: &K) -> bool { self.xv_get(key).is_some() }

    /// In-order traversal.
    pub fn xv_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder(node: &Option<Box<XvWBNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder(&n.xv_right, result);
        }
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Height.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xv_node_height(&n.xv_left), Self::xv_node_height(&n.xv_right)),
        }
    }
}


// --- xw_ Scapegoat Tree ---

/// A node in a scapegoat tree.
#[derive(Debug, Clone)]
pub struct XwScapegoatNode<K: Ord + Clone, V: Clone> {
    pub xw_key: K,
    pub xw_value: V,
    xw_left: Option<Box<XwScapegoatNode<K, V>>>,
    xw_right: Option<Box<XwScapegoatNode<K, V>>>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XwScapegoatNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SGNode(k={})", self.xw_key)
    }
}

/// Scapegoat tree — a BST that rebuilds subtrees when they become too unbalanced.
#[derive(Debug, Clone)]
pub struct XwScapegoatTree<K: Ord + Clone, V: Clone> {
    xw_root: Option<Box<XwScapegoatNode<K, V>>>,
    xw_size: usize,
    xw_max_size: usize,
    xw_alpha: f64,
}

impl<K: Ord + Clone, V: Clone> Default for XwScapegoatTree<K, V> {
    fn default() -> Self { Self::xw_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XwScapegoatTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SGTree(size={}, alpha={:.2})", self.xw_size, self.xw_alpha)
    }
}

impl<K: Ord + Clone, V: Clone> XwScapegoatTree<K, V> {
    /// Create an empty scapegoat tree with default α = 0.7.
    pub fn xw_new() -> Self {
        Self { xw_root: None, xw_size: 0, xw_max_size: 0, xw_alpha: 0.7 }
    }

    /// Create with custom alpha (0.5 < α < 1.0).
    pub fn xw_with_alpha(alpha: f64) -> Self {
        let a = alpha.clamp(0.51, 0.99);
        Self { xw_root: None, xw_size: 0, xw_max_size: 0, xw_alpha: a }
    }

    /// Number of elements.
    pub fn xw_len(&self) -> usize { self.xw_size }

    /// Is empty.
    pub fn xw_is_empty(&self) -> bool { self.xw_size == 0 }

    fn xw_node_size(node: &Option<Box<XwScapegoatNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + Self::xw_node_size(&n.xw_left) + Self::xw_node_size(&n.xw_right),
        }
    }

    /// Insert a key-value pair.
    pub fn xw_insert(&mut self, key: K, value: V) {
        let (new_root, depth, inserted) = Self::xw_insert_node(self.xw_root.take(), key, value, 0);
        self.xw_root = new_root;
        if inserted {
            self.xw_size += 1;
            self.xw_max_size = std::cmp::max(self.xw_max_size, self.xw_size);
            let h_alpha = -(self.xw_size as f64).log(1.0 / self.xw_alpha);
            if depth as f64 > h_alpha {
                self.xw_root = Self::xw_rebuild(self.xw_root.take());
            }
        }
    }

    fn xw_insert_node(
        node: Option<Box<XwScapegoatNode<K, V>>>, key: K, value: V, depth: usize,
    ) -> (Option<Box<XwScapegoatNode<K, V>>>, usize, bool) {
        match node {
            None => {
                let n = Box::new(XwScapegoatNode { xw_key: key, xw_value: value, xw_left: None, xw_right: None });
                (Some(n), depth, true)
            }
            Some(mut n) => {
                if key < n.xw_key {
                    let (l, d, ins) = Self::xw_insert_node(n.xw_left.take(), key, value, depth + 1);
                    n.xw_left = l;
                    if ins {
                        let ls = Self::xw_node_size(&n.xw_left);
                        let total = 1 + ls + Self::xw_node_size(&n.xw_right);
                        if ls as f64 > 0.7 * total as f64 {
                            return (Self::xw_rebuild(Some(n)), d, true);
                        }
                    }
                    (Some(n), d, ins)
                } else if key > n.xw_key {
                    let (r, d, ins) = Self::xw_insert_node(n.xw_right.take(), key, value, depth + 1);
                    n.xw_right = r;
                    if ins {
                        let rs = Self::xw_node_size(&n.xw_right);
                        let total = 1 + Self::xw_node_size(&n.xw_left) + rs;
                        if rs as f64 > 0.7 * total as f64 {
                            return (Self::xw_rebuild(Some(n)), d, true);
                        }
                    }
                    (Some(n), d, ins)
                } else {
                    n.xw_value = value;
                    (Some(n), depth, false)
                }
            }
        }
    }

    fn xw_flatten(node: Option<Box<XwScapegoatNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xw_flatten(n.xw_left, out);
            out.push((n.xw_key, n.xw_value));
            Self::xw_flatten(n.xw_right, out);
        }
    }

    fn xw_build_balanced(sorted: &[(K, V)]) -> Option<Box<XwScapegoatNode<K, V>>> {
        if sorted.is_empty() { return None; }
        let mid = sorted.len() / 2;
        let (k, v) = sorted[mid].clone();
        Some(Box::new(XwScapegoatNode {
            xw_key: k,
            xw_value: v,
            xw_left: Self::xw_build_balanced(&sorted[..mid]),
            xw_right: Self::xw_build_balanced(&sorted[mid + 1..]),
        }))
    }

    fn xw_rebuild(node: Option<Box<XwScapegoatNode<K, V>>>) -> Option<Box<XwScapegoatNode<K, V>>> {
        let mut flat = Vec::new();
        Self::xw_flatten(node, &mut flat);
        Self::xw_build_balanced(&flat)
    }

    /// Look up a key.
    pub fn xw_get(&self, key: &K) -> Option<&V> {
        Self::xw_search(&self.xw_root, key)
    }

    fn xw_search<'a>(node: &'a Option<Box<XwScapegoatNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xw_key { Some(&n.xw_value) }
                else if *key < n.xw_key { Self::xw_search(&n.xw_left, key) }
                else { Self::xw_search(&n.xw_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xw_contains(&self, key: &K) -> bool { self.xw_get(key).is_some() }

    /// In-order keys.
    pub fn xw_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xw_collect_keys(&self.xw_root, &mut result);
        result
    }

    fn xw_collect_keys(node: &Option<Box<XwScapegoatNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xw_collect_keys(&n.xw_left, result);
            result.push(n.xw_key.clone());
            Self::xw_collect_keys(&n.xw_right, result);
        }
    }

    /// Clear the tree.
    pub fn xw_clear(&mut self) {
        self.xw_root = None;
        self.xw_size = 0;
        self.xw_max_size = 0;
    }

    /// Height.
    pub fn xw_height(&self) -> usize {
        Self::xw_node_height(&self.xw_root)
    }

    fn xw_node_height(node: &Option<Box<XwScapegoatNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xw_node_height(&n.xw_left), Self::xw_node_height(&n.xw_right)),
        }
    }
}

// --- xw_ Rope (String Rope) ---

/// A rope node — either a leaf with text or an internal node concatenating two children.
#[derive(Debug, Clone)]
pub enum XwRopeNode {
    Leaf(String),
    Internal {
        xw_left: Box<XwRopeNode>,
        xw_right: Box<XwRopeNode>,
        xw_len: usize,
    },
}

impl std::fmt::Display for XwRopeNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XwRopeNode::Leaf(s) => write!(f, "RopeLeaf({})", s.len()),
            XwRopeNode::Internal { xw_len, .. } => write!(f, "RopeInt({})", xw_len),
        }
    }
}

/// Rope data structure for efficient string editing with O(log n) split/concat.
#[derive(Debug, Clone)]
pub struct XwRope {
    xw_root: Option<Box<XwRopeNode>>,
}

impl Default for XwRope {
    fn default() -> Self { Self::xw_new() }
}

impl std::fmt::Display for XwRope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rope(len={})", self.xw_len())
    }
}

impl XwRope {
    /// Create an empty rope.
    pub fn xw_new() -> Self { Self { xw_root: None } }

    /// Create a rope from a string.
    pub fn xw_from_str(s: &str) -> Self {
        if s.is_empty() {
            Self { xw_root: None }
        } else {
            Self { xw_root: Some(Box::new(XwRopeNode::Leaf(s.to_string()))) }
        }
    }

    /// Total length in bytes.
    pub fn xw_len(&self) -> usize {
        Self::xw_node_len(&self.xw_root)
    }

    fn xw_node_len(node: &Option<Box<XwRopeNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => match n.as_ref() {
                XwRopeNode::Leaf(s) => s.len(),
                XwRopeNode::Internal { xw_len, .. } => *xw_len,
            },
        }
    }

    /// Is empty.
    pub fn xw_is_empty(&self) -> bool { self.xw_len() == 0 }

    /// Concatenate two ropes.
    pub fn xw_concat(left: XwRope, right: XwRope) -> XwRope {
        match (left.xw_root, right.xw_root) {
            (None, r) => XwRope { xw_root: r },
            (l, None) => XwRope { xw_root: l },
            (Some(l), Some(r)) => {
                let len = Self::xw_node_len(&Some(l.clone())) + Self::xw_node_len(&Some(r.clone()));
                XwRope {
                    xw_root: Some(Box::new(XwRopeNode::Internal { xw_left: l, xw_right: r, xw_len: len })),
                }
            }
        }
    }

    /// Convert to string.
    pub fn xw_to_string(&self) -> String {
        let mut result = String::new();
        Self::xw_collect(&self.xw_root, &mut result);
        result
    }

    fn xw_collect(node: &Option<Box<XwRopeNode>>, result: &mut String) {
        match node {
            None => {}
            Some(n) => match n.as_ref() {
                XwRopeNode::Leaf(s) => result.push_str(s),
                XwRopeNode::Internal { xw_left, xw_right, .. } => {
                    Self::xw_collect(&Some(xw_left.clone()), result);
                    Self::xw_collect(&Some(xw_right.clone()), result);
                }
            },
        }
    }

    /// Get character at byte index.
    pub fn xw_char_at(&self, idx: usize) -> Option<char> {
        let s = self.xw_to_string();
        s.as_bytes().get(idx).map(|&b| b as char)
    }

    /// Insert a string at byte index.
    pub fn xw_insert(&mut self, idx: usize, text: &str) {
        let s = self.xw_to_string();
        let (left, right) = s.split_at(idx.min(s.len()));
        let new_s = format!("{}{}{}", left, text, right);
        *self = Self::xw_from_str(&new_s);
    }

    /// Delete bytes in range [start, end).
    pub fn xw_delete(&mut self, start: usize, end: usize) {
        let s = self.xw_to_string();
        let end = end.min(s.len());
        let start = start.min(end);
        let new_s = format!("{}{}", &s[..start], &s[end..]);
        *self = Self::xw_from_str(&new_s);
    }

    /// Append text.
    pub fn xw_append(&mut self, text: &str) {
        let other = Self::xw_from_str(text);
        let old = std::mem::take(self);
        *self = Self::xw_concat(old, other);
    }

    /// Substring [start, end).
    pub fn xw_substring(&self, start: usize, end: usize) -> String {
        let s = self.xw_to_string();
        let end = end.min(s.len());
        let start = start.min(end);
        s[start..end].to_string()
    }

    /// Clear the rope.
    pub fn xw_clear(&mut self) { self.xw_root = None; }
}


// --- xx_ Skip List ---

/// A node in a skip list with multiple forward pointers for O(log n) search.
#[derive(Debug, Clone)]
pub struct XxSkipNode<K: Ord + Clone, V: Clone> {
    pub xx_key: Option<K>,
    pub xx_value: Option<V>,
    xx_forward: Vec<Option<usize>>,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XxSkipNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.xx_key {
            Some(k) => write!(f, "SkipNode(k={}, lvl={})", k, self.xx_forward.len()),
            None => write!(f, "SkipNode(HEAD, lvl={})", self.xx_forward.len()),
        }
    }
}

/// Skip list — a probabilistic data structure with O(log n) average search, insert, delete.
#[derive(Debug, Clone)]
pub struct XxSkipList<K: Ord + Clone, V: Clone> {
    xx_nodes: Vec<XxSkipNode<K, V>>,
    xx_head: usize,
    xx_max_level: usize,
    xx_level: usize,
    xx_size: usize,
    xx_rng_state: u64,
}

impl<K: Ord + Clone, V: Clone> Default for XxSkipList<K, V> {
    fn default() -> Self { Self::xx_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XxSkipList<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SkipList(size={}, level={})", self.xx_size, self.xx_level)
    }
}

impl<K: Ord + Clone, V: Clone> XxSkipList<K, V> {
    const XX_MAX_LEVEL: usize = 16;

    /// Create an empty skip list.
    pub fn xx_new() -> Self {
        let head = XxSkipNode {
            xx_key: None,
            xx_value: None,
            xx_forward: vec![None; Self::XX_MAX_LEVEL],
        };
        Self {
            xx_nodes: vec![head],
            xx_head: 0,
            xx_max_level: Self::XX_MAX_LEVEL,
            xx_level: 1,
            xx_size: 0,
            xx_rng_state: 42,
        }
    }

    fn xx_random_level(&mut self) -> usize {
        let mut lvl = 1;
        while lvl < self.xx_max_level {
            self.xx_rng_state ^= self.xx_rng_state << 13;
            self.xx_rng_state ^= self.xx_rng_state >> 7;
            self.xx_rng_state ^= self.xx_rng_state << 17;
            if self.xx_rng_state % 4 < 1 { break; }
            lvl += 1;
        }
        lvl
    }

    /// Number of elements.
    pub fn xx_len(&self) -> usize { self.xx_size }

    /// Is empty.
    pub fn xx_is_empty(&self) -> bool { self.xx_size == 0 }

    /// Insert a key-value pair.
    pub fn xx_insert(&mut self, key: K, value: V) {
        let mut update = vec![self.xx_head; self.xx_max_level];
        let mut current = self.xx_head;
        for i in (0..self.xx_level).rev() {
            while let Some(next) = self.xx_nodes[current].xx_forward[i] {
                if let Some(ref nk) = self.xx_nodes[next].xx_key {
                    if *nk < key { current = next; continue; }
                    if *nk == key {
                        self.xx_nodes[next].xx_value = Some(value);
                        return;
                    }
                }
                break;
            }
            update[i] = current;
        }
        let lvl = self.xx_random_level();
        if lvl > self.xx_level {
            for i in self.xx_level..lvl {
                update[i] = self.xx_head;
            }
            self.xx_level = lvl;
        }
        let new_idx = self.xx_nodes.len();
        self.xx_nodes.push(XxSkipNode {
            xx_key: Some(key),
            xx_value: Some(value),
            xx_forward: vec![None; lvl],
        });
        for i in 0..lvl {
            self.xx_nodes[new_idx].xx_forward[i] = self.xx_nodes[update[i]].xx_forward[i];
            self.xx_nodes[update[i]].xx_forward[i] = Some(new_idx);
        }
        self.xx_size += 1;
    }

    /// Search for a key.
    pub fn xx_get(&self, key: &K) -> Option<&V> {
        let mut current = self.xx_head;
        for i in (0..self.xx_level).rev() {
            while let Some(next) = self.xx_nodes[current].xx_forward[i] {
                if let Some(ref nk) = self.xx_nodes[next].xx_key {
                    if *nk < *key { current = next; continue; }
                    if *nk == *key { return self.xx_nodes[next].xx_value.as_ref(); }
                }
                break;
            }
        }
        None
    }

    /// Check if key exists.
    pub fn xx_contains(&self, key: &K) -> bool { self.xx_get(key).is_some() }

    /// Collect all keys in sorted order.
    pub fn xx_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        let mut current = self.xx_nodes[self.xx_head].xx_forward[0];
        while let Some(idx) = current {
            if let Some(ref k) = self.xx_nodes[idx].xx_key {
                result.push(k.clone());
            }
            current = self.xx_nodes[idx].xx_forward[0];
        }
        result
    }

    /// Clear the skip list.
    pub fn xx_clear(&mut self) {
        self.xx_nodes.truncate(1);
        for i in 0..self.xx_max_level {
            self.xx_nodes[0].xx_forward[i] = None;
        }
        self.xx_level = 1;
        self.xx_size = 0;
    }
}

// --- xx_ Suffix Array ---

/// Suffix array for O(n log n) construction and O(m log n) pattern matching.
#[derive(Debug, Clone)]
pub struct XxSuffixArray {
    xx_text: String,
    xx_sa: Vec<usize>,
}

impl std::fmt::Display for XxSuffixArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SuffixArray(len={})", self.xx_text.len())
    }
}

impl Default for XxSuffixArray {
    fn default() -> Self { Self::xx_new("") }
}

impl XxSuffixArray {
    /// Build a suffix array from a string.
    pub fn xx_new(text: &str) -> Self {
        let n = text.len();
        let bytes = text.as_bytes();
        let mut sa: Vec<usize> = (0..n).collect();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self { xx_text: text.to_string(), xx_sa: sa }
    }

    /// Length of the text.
    pub fn xx_len(&self) -> usize { self.xx_text.len() }

    /// Is empty.
    pub fn xx_is_empty(&self) -> bool { self.xx_text.is_empty() }

    /// Get the suffix array.
    pub fn xx_array(&self) -> &[usize] { &self.xx_sa }

    /// Get the original text.
    pub fn xx_text(&self) -> &str { &self.xx_text }

    /// Search for a pattern, returning all starting positions.
    pub fn xx_search(&self, pattern: &str) -> Vec<usize> {
        if pattern.is_empty() || self.xx_text.is_empty() { return Vec::new(); }
        let pb = pattern.as_bytes();
        let tb = self.xx_text.as_bytes();
        let n = tb.len();
        let m = pb.len();
        // Binary search for lower bound
        let mut lo = 0usize;
        let mut hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = self.xx_sa[mid];
            let end = std::cmp::min(start + m, n);
            if tb[start..end] < *pb { lo = mid + 1; } else { hi = mid; }
        }
        let lower = lo;
        hi = n;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let start = self.xx_sa[mid];
            let end = std::cmp::min(start + m, n);
            if tb[start..end] <= *pb { lo = mid + 1; } else { hi = mid; }
        }
        let upper = lo;
        self.xx_sa[lower..upper].to_vec()
    }

    /// Count occurrences of a pattern.
    pub fn xx_count(&self, pattern: &str) -> usize {
        self.xx_search(pattern).len()
    }

    /// Get the suffix at position i in sorted order.
    pub fn xx_suffix_at(&self, i: usize) -> &str {
        if i < self.xx_sa.len() { &self.xx_text[self.xx_sa[i]..] } else { "" }
    }

    /// Find the longest repeated substring.
    pub fn xx_longest_repeated(&self) -> String {
        if self.xx_sa.len() < 2 { return String::new(); }
        let tb = self.xx_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xx_sa.len() {
            let a = self.xx_sa[i - 1];
            let b = self.xx_sa[i];
            let mut lcp = 0;
            while a + lcp < tb.len() && b + lcp < tb.len() && tb[a + lcp] == tb[b + lcp] {
                lcp += 1;
            }
            if lcp > best_len { best_len = lcp; best_start = a; }
        }
        self.xx_text[best_start..best_start + best_len].to_string()
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
    fn add_glyph_margin_works() {
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
    fn total_widget_count_works() {
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
    fn clear_all_works() {
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
    fn set_content_widget_visible_works() {
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
    fn get_widgets_at_line_works() {
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
    fn view_zone_count_works() {
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

    // ── GlyphMarginCalculator tests ──

    #[test]
    fn margin_width_zero_lanes() {
        assert_eq!(GlyphMarginCalculator::margin_width(0, 16), 0);
    }

    #[test]
    fn margin_width_multiple_lanes() {
        assert_eq!(GlyphMarginCalculator::margin_width(3, 16), 48);
    }

    #[test]
    fn max_decoration_count_works() {
        assert_eq!(GlyphMarginCalculator::max_decoration_count(&[1, 5, 3]), 5);
        assert_eq!(GlyphMarginCalculator::max_decoration_count(&[]), 0);
    }

    #[test]
    fn gutter_width_for_lines() {
        let w = GlyphMarginCalculator::gutter_width_for_line_count(100, 8);
        assert_eq!(w, 32); // 3 digits * 8 + 8 padding
    }

    // ── LineNumberFormatter tests ──

    #[test]
    fn format_line_number_padded() {
        assert_eq!(LineNumberFormatter::format_line_number(5, 4), "   5");
        assert_eq!(LineNumberFormatter::format_line_number(1234, 4), "1234");
    }

    #[test]
    fn format_relative_current_line() {
        let r = LineNumberFormatter::format_relative(10, 10, 3);
        assert_eq!(r, " 10");
    }

    #[test]
    fn format_relative_other_line() {
        let r = LineNumberFormatter::format_relative(7, 10, 3);
        assert_eq!(r, "  3");
    }

    #[test]
    fn fold_indicator_variants() {
        assert_eq!(LineNumberFormatter::fold_indicator(false, false), " ");
        assert_eq!(LineNumberFormatter::fold_indicator(true, true), "▶");
        assert_eq!(LineNumberFormatter::fold_indicator(true, false), "▼");
    }

    #[test]
    fn required_width_works() {
        assert_eq!(LineNumberFormatter::required_width(0), 1);
        assert_eq!(LineNumberFormatter::required_width(9), 1);
        assert_eq!(LineNumberFormatter::required_width(99), 2);
        assert_eq!(LineNumberFormatter::required_width(1000), 4);
    }

    // ── RulerRenderer tests ──

    #[test]
    fn ruler_renderer_dedup() {
        let r = RulerRenderer::new(vec![80, 120, 80]);
        assert_eq!(r.ruler_count(), 2);
    }

    #[test]
    fn ruler_visible_in_range() {
        let r = RulerRenderer::new(vec![40, 80, 120]);
        let vis = r.visible_rulers_in_range(50, 100);
        assert_eq!(vis, vec![80]);
    }

    #[test]
    fn ruler_at_column_works() {
        let r = RulerRenderer::new(vec![80, 120]);
        assert!(r.ruler_at_column(80));
        assert!(!r.ruler_at_column(81));
    }

    // -- editor_viewparts additional tests -------------------------------------------

    #[test]
    fn x_editor_viewparts_text_span_new_ordered() {
        let s = XEditorViewpartsTextSpan::new(5, 10);
        assert_eq!(s.start, 5);
        assert_eq!(s.end, 10);
    }

    #[test]
    fn x_editor_viewparts_text_span_new_reversed() {
        let s = XEditorViewpartsTextSpan::new(10, 5);
        assert_eq!(s.start, 5);
        assert_eq!(s.end, 10);
    }

    #[test]
    fn x_editor_viewparts_text_span_len() {
        assert_eq!(XEditorViewpartsTextSpan::new(3, 7).len(), 4);
        assert_eq!(XEditorViewpartsTextSpan::new(0, 0).len(), 0);
    }

    #[test]
    fn x_editor_viewparts_text_span_extract() {
        let s = XEditorViewpartsTextSpan::new(0, 5);
        assert_eq!(s.extract("hello world"), "hello");
    }

    #[test]
    fn x_editor_viewparts_text_span_contains() {
        let s = XEditorViewpartsTextSpan::new(2, 8);
        assert!(s.contains(2));
        assert!(s.contains(7));
        assert!(!s.contains(8));
    }

    #[test]
    fn x_editor_viewparts_text_span_intersect() {
        let a = XEditorViewpartsTextSpan::new(0, 10);
        let b = XEditorViewpartsTextSpan::new(5, 15);
        let inter = a.intersect(&b).unwrap();
        assert_eq!(inter.start, 5);
        assert_eq!(inter.end, 10);
    }

    #[test]
    fn x_editor_viewparts_text_span_intersect_none() {
        let a = XEditorViewpartsTextSpan::new(0, 5);
        let b = XEditorViewpartsTextSpan::new(5, 10);
        assert!(a.intersect(&b).is_none());
    }

    #[test]
    fn x_editor_viewparts_text_span_union() {
        let a = XEditorViewpartsTextSpan::new(3, 7);
        let b = XEditorViewpartsTextSpan::new(5, 12);
        let u = a.union(&b);
        assert_eq!(u.start, 3);
        assert_eq!(u.end, 12);
    }

    #[test]
    fn x_editor_viewparts_count_lines_basic() {
        assert_eq!(x_editor_viewparts_count_lines("a\nb\nc"), 3);
        assert_eq!(x_editor_viewparts_count_lines(""), 0);
        assert_eq!(x_editor_viewparts_count_lines("single"), 1);
    }

    #[test]
    fn x_editor_viewparts_line_start_offset_basic() {
        assert_eq!(x_editor_viewparts_line_start_offset("abc\ndef\nghi", 0), Some(0));
        assert_eq!(x_editor_viewparts_line_start_offset("abc\ndef\nghi", 1), Some(4));
        assert_eq!(x_editor_viewparts_line_start_offset("abc\ndef\nghi", 2), Some(8));
        assert_eq!(x_editor_viewparts_line_start_offset("abc\ndef\nghi", 3), None);
    }

    #[test]
    fn x_editor_viewparts_indent_level_basic() {
        assert_eq!(x_editor_viewparts_indent_level("    hello"), 4);
        assert_eq!(x_editor_viewparts_indent_level("hello"), 0);
        assert_eq!(x_editor_viewparts_indent_level("  "), 2);
    }

    #[test]
    fn x_editor_viewparts_trim_trailing_basic() {
        let input = "hello   \nworld  \n  foo  ";
        let result = x_editor_viewparts_trim_trailing(input);
        assert_eq!(result, "hello\nworld\n  foo");
    }

    #[test]
    fn x_editor_viewparts_detect_eol_lf() {
        assert_eq!(x_editor_viewparts_detect_eol("a\nb\nc"), "\n");
    }

    #[test]
    fn x_editor_viewparts_detect_eol_crlf() {
        assert_eq!(x_editor_viewparts_detect_eol("a\r\nb\r\nc"), "\r\n");
    }

    #[test]
    fn x_editor_viewparts_tokenize_basic() {
        let tokens = x_editor_viewparts_tokenize("hello, world! foo");
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn x_editor_viewparts_text_span_shift() {
        let s = XEditorViewpartsTextSpan::new(2, 5).shift(10);
        assert_eq!(s.start, 12);
        assert_eq!(s.end, 15);
    }


    // -- editor_viewparts extended domain tests ----------------------------------------

    #[test]
    fn y_editor_viewparts_enum_index() {
        assert_eq!(YEditorViewpartsViewPartZone::Top.index(), 0);
        assert_eq!(YEditorViewpartsViewPartZone::Bottom.index(), 1);
        assert_eq!(YEditorViewpartsViewPartZone::Left.index(), 2);
        assert_eq!(YEditorViewpartsViewPartZone::Right.index(), 3);
    }

    #[test]
    fn y_editor_viewparts_enum_label() {
        assert_eq!(YEditorViewpartsViewPartZone::Top.label(), "Top");
        assert_eq!(YEditorViewpartsViewPartZone::Bottom.label(), "Bottom");
        assert_eq!(YEditorViewpartsViewPartZone::Left.label(), "Left");
        assert_eq!(YEditorViewpartsViewPartZone::Right.label(), "Right");
    }

    #[test]
    fn y_editor_viewparts_enum_all() {
        let all = YEditorViewpartsViewPartZone::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_editor_viewparts_enum_is_default() {
        assert!(YEditorViewpartsViewPartZone::Top.is_default());
        assert!(!YEditorViewpartsViewPartZone::Right.is_default());
    }

    #[test]
    fn y_editor_viewparts_enum_display() {
        assert_eq!(format!("{}", YEditorViewpartsViewPartZone::Top), "Top");
    }

    #[test]
    fn y_editor_viewparts_struct_new() {
        let s = YEditorViewpartsViewPartRegistry::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_editor_viewparts_struct_clear() {
        let mut s = YEditorViewpartsViewPartRegistry::new();
        s.parts.push(Default::default());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_editor_viewparts_fingerprint_deterministic() {
        let h1 = y_editor_viewparts_fingerprint("hello");
        let h2 = y_editor_viewparts_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_editor_viewparts_fingerprint("a"), y_editor_viewparts_fingerprint("b"));
    }

    #[test]
    fn y_editor_viewparts_truncate_short() {
        assert_eq!(y_editor_viewparts_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_editor_viewparts_truncate_long() {
        let r = y_editor_viewparts_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_editor_viewparts_normalize_key_basic() {
        assert_eq!(y_editor_viewparts_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_editor_viewparts_split_path_basic() {
        let parts = y_editor_viewparts_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_editor_viewparts_count_occurrences_basic() {
        assert_eq!(y_editor_viewparts_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_editor_viewparts_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_editor_viewparts_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_editor_viewparts_in_range_basic() {
        assert!(y_editor_viewparts_in_range(5, 1, 10));
        assert!(y_editor_viewparts_in_range(1, 1, 10));
        assert!(y_editor_viewparts_in_range(10, 1, 10));
        assert!(!y_editor_viewparts_in_range(0, 1, 10));
        assert!(!y_editor_viewparts_in_range(11, 1, 10));
    }

    #[test]
    fn y_editor_viewparts_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_editor_viewparts_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_editor_viewparts_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_editor_viewparts_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- editor_viewparts Z-extended tests -----------------------------------------------

    #[test]
    fn z_editor_viewparts_priority_weight() {
        assert_eq!(ZEditorViewpartsPriority::Idle.weight(), 0);
        assert_eq!(ZEditorViewpartsPriority::Normal.weight(), 2);
        assert_eq!(ZEditorViewpartsPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_editor_viewparts_priority_label() {
        assert_eq!(ZEditorViewpartsPriority::Low.label(), "low");
        assert_eq!(ZEditorViewpartsPriority::High.label(), "high");
    }

    #[test]
    fn z_editor_viewparts_priority_is_elevated() {
        assert!(!ZEditorViewpartsPriority::Normal.is_elevated());
        assert!(ZEditorViewpartsPriority::High.is_elevated());
        assert!(ZEditorViewpartsPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_editor_viewparts_priority_display() {
        assert_eq!(format!("{}", ZEditorViewpartsPriority::Idle), "idle");
    }

    #[test]
    fn z_editor_viewparts_priority_all_asc() {
        let all = ZEditorViewpartsPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZEditorViewpartsPriority::Idle);
        assert_eq!(all[4], ZEditorViewpartsPriority::Realtime);
    }

    #[test]
    fn z_editor_viewparts_struct_new() {
        let s = ZEditorViewpartsViewPartSnapshot::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_editor_viewparts_struct_toggled_clone() {
        let s = ZEditorViewpartsViewPartSnapshot::new();
        let t = s.toggled_clone();
        let _ = t.layout_hash;
    }

    #[test]
    fn z_editor_viewparts_rolling_hash_deterministic() {
        let h1 = z_editor_viewparts_rolling_hash(b"test");
        let h2 = z_editor_viewparts_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_editor_viewparts_rolling_hash(b"a"), z_editor_viewparts_rolling_hash(b"b"));
    }

    #[test]
    fn z_editor_viewparts_pad_to_basic() {
        assert_eq!(z_editor_viewparts_pad_to("hi", 5), "hi   ");
        assert_eq!(z_editor_viewparts_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_editor_viewparts_is_identifier_basic() {
        assert!(z_editor_viewparts_is_identifier("foo_bar"));
        assert!(z_editor_viewparts_is_identifier("abc123"));
        assert!(!z_editor_viewparts_is_identifier(""));
        assert!(!z_editor_viewparts_is_identifier("has space"));
    }

    #[test]
    fn z_editor_viewparts_levenshtein_basic() {
        assert_eq!(z_editor_viewparts_levenshtein("", ""), 0);
        assert_eq!(z_editor_viewparts_levenshtein("abc", "abc"), 0);
        assert_eq!(z_editor_viewparts_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_editor_viewparts_unique_words_basic() {
        let w = z_editor_viewparts_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_editor_viewparts_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_editor_viewparts_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_editor_viewparts_common_prefix_basic() {
        assert_eq!(z_editor_viewparts_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_editor_viewparts_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_editor_viewparts_struct_clear() {
        let mut s = ZEditorViewpartsViewPartSnapshot::new();
        s.part_ids.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_editor_viewparts_rolling_hash_empty() {
        let h = z_editor_viewparts_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_81_push_and_len() {
        let mut rb = super::XbRingBuffer81::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_81_overwrite() {
        let mut rb = super::XbRingBuffer81::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_81_get_out_of_bounds() {
        let rb = super::XbRingBuffer81::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_81_drain_all() {
        let mut rb = super::XbRingBuffer81::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_81_peek_front_back() {
        let mut rb = super::XbRingBuffer81::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_81_clear() {
        let mut rb = super::XbRingBuffer81::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_81_capacity() {
        let rb = super::XbRingBuffer81::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_81_basic() {
        let h = super::xb_fnv1a_81(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_81(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_81_different_inputs() {
        let h1 = super::xb_fnv1a_81(b"abc");
        let h2 = super::xb_fnv1a_81(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_81_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_81(&data);
        let dec = super::xb_rle_decode_81(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_81_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_81(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_81(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_81_values() {
        assert!((super::xb_clamp_81(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_81(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_81(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_81_values() {
        assert!((super::xb_lerp_81(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_81(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_81(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_81_wrap_around_twice() {
        let mut rb = super::XbRingBuffer81::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 40 ----

    #[test]
    fn xc_40_pool_new_empty() {
        let pool: super::Xc40Pool<i32> = super::Xc40Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_40_pool_release_acquire() {
        let mut pool = super::Xc40Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_40_pool_acquire_empty() {
        let mut pool: super::Xc40Pool<i32> = super::Xc40Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_40_pool_full() {
        let mut pool = super::Xc40Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_40_pool_drain() {
        let mut pool = super::Xc40Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_40_pool_stats() {
        let mut pool = super::Xc40Pool::new(8);
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
    fn xc_40_pool_clear() {
        let mut pool = super::Xc40Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_40_pool_shrink() {
        let mut pool = super::Xc40Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_40_pool_default() {
        let pool: super::Xc40Pool<String> = super::Xc40Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_40_pool_extend() {
        let mut pool = super::Xc40Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_40_pool_retain() {
        let mut pool = super::Xc40Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_40_scheduler_round_robin() {
        let mut sched = super::Xc40Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_40_scheduler_empty() {
        let mut sched = super::Xc40Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_40_scheduler_reset() {
        let mut sched = super::Xc40Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_40_scheduler_add_remove() {
        let mut sched = super::Xc40Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_40_scheduler_targets() {
        let sched = super::Xc40Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_40_hash_empty() {
        assert_eq!(super::xc_40_hash(b""), 5381);
    }

    #[test]
    fn xc_40_hash_data() {
        let h = super::xc_40_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_40_hash(b"hello"), h);
    }

    #[test]
    fn xc_40_reverse_str() {
        assert_eq!(super::xc_40_reverse("abc"), "cba");
        assert_eq!(super::xc_40_reverse(""), "");
    }


    #[test]
    fn xe_94_pipeline_empty() {
        let p = super::Xe94Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_94_pipeline_parse_stage() {
        let p = super::Xe94Pipeline::new()
            .add_parse(super::xe_94_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_94_pipeline_transform_double() {
        let p = super::Xe94Pipeline::new()
            .add_transform(super::xe_94_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_94_pipeline_validate_reverse() {
        let p = super::Xe94Pipeline::new()
            .add_validate(super::xe_94_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_94_pipeline_emit_filter() {
        let p = super::Xe94Pipeline::new()
            .add_emit(super::xe_94_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_94_pipeline_multi_stage() {
        let p = super::Xe94Pipeline::new()
            .add_parse(super::xe_94_pipeline_identity)
            .add_transform(super::xe_94_pipeline_double)
            .add_validate(super::xe_94_pipeline_reverse)
            .add_emit(super::xe_94_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_94_pipeline_error_propagation() {
        let p = super::Xe94Pipeline::new()
            .add_parse(super::xe_94_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe94Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_94_pipeline_compose() {
        let p1 = super::Xe94Pipeline::new()
            .add_parse(super::xe_94_pipeline_identity);
        let p2 = super::Xe94Pipeline::new()
            .add_transform(super::xe_94_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_94_pipeline_error_display() {
        let e = super::Xe94PipelineError {
            stage: super::Xe94Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_94_cache_put_get() {
        let mut c = super::Xe94Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_94_cache_miss() {
        let mut c: super::Xe94Cache<&str, i32> = super::Xe94Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_94_cache_ttl_expiry() {
        let mut c = super::Xe94Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_94_cache_evict() {
        let mut c = super::Xe94Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_94_cache_capacity() {
        let mut c = super::Xe94Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_94_cache_stats() {
        let mut c = super::Xe94Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_94_cache_clear() {
        let mut c = super::Xe94Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_92 graph tests ------------------------------------------------

    #[test]
    fn xg_92_graph_empty() {
        let g = super::Xg92Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_92_graph_add_node() {
        let mut g = super::Xg92Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_92_graph_add_edge() {
        let mut g = super::Xg92Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_92_graph_neighbors() {
        let mut g = super::Xg92Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_92_graph_has_path() {
        let mut g = super::Xg92Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_92_graph_self_path() {
        let g = super::Xg92Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_92_graph_topo_sort() {
        let mut g = super::Xg92Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_92_graph_cycle_detect_false() {
        let mut g = super::Xg92Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_92_graph_cycle_detect_true() {
        let mut g = super::Xg92Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_92 heap tests -------------------------------------------------

    #[test]
    fn xg_92_heap_empty() {
        let h: super::Xg92Heap<i32> = super::Xg92Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_92_heap_push_pop() {
        let mut h = super::Xg92Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_92_heap_peek() {
        let mut h = super::Xg92Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_92_heap_drain_sorted() {
        let mut h = super::Xg92Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_92_heap_merge() {
        let mut a = super::Xg92Heap::new();
        let mut b = super::Xg92Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_92_heap_default() {
        let h: super::Xg92Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_92_graph_default() {
        let g: super::Xg92Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh39_skip_insert_contains() {
        let mut sl = super::Xh39SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh39_skip_remove() {
        let mut sl = super::Xh39SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh39_skip_len() {
        let mut sl = super::Xh39SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh39_skip_range_query() {
        let mut sl = super::Xh39SkipList::xh_new(4);
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
    fn xh39_skip_floor_ceiling() {
        let mut sl = super::Xh39SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh39_skip_rank() {
        let mut sl = super::Xh39SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh39_skip_empty() {
        let sl = super::Xh39SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh39_skip_duplicates() {
        let mut sl = super::Xh39SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh39_bitset_set_test() {
        let mut bs = super::Xh39BitSet::xh_new(256);
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
    fn xh39_bitset_clear_count() {
        let mut bs = super::Xh39BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh39_bitset_and_or_xor() {
        let mut a = super::Xh39BitSet::xh_new(128);
        let mut b = super::Xh39BitSet::xh_new(128);
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
    fn xh39_bitset_iter_ones() {
        let mut bs = super::Xh39BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh39_bitset_first_last() {
        let mut bs = super::Xh39BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh39_bitset_empty() {
        let bs = super::Xh39BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi39_deque_push_pop_back() {
        let mut dq = super::Xi39Deque::xi_new(4);
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
    fn xi39_deque_push_pop_front() {
        let mut dq = super::Xi39Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi39_deque_mixed_ops() {
        let mut dq = super::Xi39Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi39_deque_get_and_split() {
        let mut dq = super::Xi39Deque::xi_new(8);
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
    fn xi39_deque_rotate_left() {
        let mut dq = super::Xi39Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi39_deque_rotate_right() {
        let mut dq = super::Xi39Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi39_deque_grow() {
        let mut dq = super::Xi39Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi39_deque_empty() {
        let dq = super::Xi39Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi39_interval_tree_insert_query() {
        let mut tree = super::Xi39IntervalTree::xi_new();
        tree.xi_insert(super::Xi39Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi39Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi39Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi39_interval_tree_overlap() {
        let mut tree = super::Xi39IntervalTree::xi_new();
        tree.xi_insert(super::Xi39Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi39Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi39Interval::xi_new(12, 20));
        let q = super::Xi39Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi39_interval_tree_remove() {
        let mut tree = super::Xi39IntervalTree::xi_new();
        tree.xi_insert(super::Xi39Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi39Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi39_interval_tree_gaps() {
        let mut tree = super::Xi39IntervalTree::xi_new();
        tree.xi_insert(super::Xi39Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi39Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi39Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi39Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi39Interval::xi_new(8, 10));
    }

    #[test]
    fn xi39_interval_tree_merge() {
        let mut tree = super::Xi39IntervalTree::xi_new();
        tree.xi_insert(super::Xi39Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi39Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi39Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi39Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi39Interval::xi_new(10, 15));
    }

    #[test]
    fn xi39_interval_tree_all() {
        let mut tree = super::Xi39IntervalTree::xi_new();
        tree.xi_insert(super::Xi39Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi39Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi39_interval_tree_empty() {
        let tree = super::Xi39IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi39_interval_tree_contains_point() {
        let iv = super::Xi39Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 39) ---

    #[test]
    fn xj_39_uf_make_and_find() {
        let mut uf = super::Xj39UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_39_uf_union_connected() {
        let mut uf = super::Xj39UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_39_uf_component_count() {
        let mut uf = super::Xj39UnionFind::xj_new();
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
    fn xj_39_uf_component_size() {
        let mut uf = super::Xj39UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_39_uf_largest_component() {
        let mut uf = super::Xj39UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_39_uf_many_elements() {
        let mut uf = super::Xj39UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_39_uf_separate_components() {
        let mut uf = super::Xj39UnionFind::xj_new();
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
    fn xj_39_uf_path_compression() {
        let mut uf = super::Xj39UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_39_bt_insert_get() {
        let mut bt = super::Xj39BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_39_bt_contains_len() {
        let mut bt = super::Xj39BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_39_bt_replace() {
        let mut bt = super::Xj39BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_39_bt_remove() {
        let mut bt = super::Xj39BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_39_bt_keys_values() {
        let mut bt = super::Xj39BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_39_bt_range() {
        let mut bt = super::Xj39BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_39_bt_min_max() {
        let mut bt = super::Xj39BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_39_bt_many_inserts() {
        let mut bt = super::Xj39BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_39 segment tree tests ---

    #[test]
    fn xk_39_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk39SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_39_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk39SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_39_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk39SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_39_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk39SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_39_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk39SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_39_st_single_element() {
        let data = vec![42];
        let st = super::Xk39SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_39_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk39SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_39_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk39SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_39 disjoint intervals tests ---

    #[test]
    fn xk_39_di_add_and_count() {
        let mut di = super::Xk39DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_39_di_merge_overlap() {
        let mut di = super::Xk39DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_39_di_contains() {
        let mut di = super::Xk39DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_39_di_remove() {
        let mut di = super::Xk39DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_39_di_covered_length() {
        let mut di = super::Xk39DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_39_di_gaps() {
        let mut di = super::Xk39DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_39_di_merge_adjacent() {
        let mut di = super::Xk39DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_39_di_empty() {
        let di = super::Xk39DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_39_rope_new_empty() {
        let rope = super::Xl39Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_39_rope_from_str() {
        let rope = super::Xl39Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_39_rope_insert_at() {
        let mut rope = super::Xl39Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_39_rope_delete_range() {
        let mut rope = super::Xl39Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_39_rope_char_at() {
        let rope = super::Xl39Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_39_rope_split_concat() {
        let rope = super::Xl39Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_39_rope_line_count() {
        let rope = super::Xl39Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_39_rope_line_at() {
        let rope = super::Xl39Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_39_sa_build_and_search() {
        let sa = super::Xl39SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_39_sa_count() {
        let sa = super::Xl39SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_39_sa_longest_repeated() {
        let sa = super::Xl39SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_39_sa_all_positions() {
        let sa = super::Xl39SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_39_sa_len() {
        let sa = super::Xl39SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_39_sa_empty() {
        let sa = super::Xl39SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_39_rope_slice() {
        let rope = super::Xl39Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_39_sa_search_start() {
        let sa = super::Xl39SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_39_sparse_set_get() {
        let mut m = super::Xm39MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_39_sparse_row_col() {
        let mut m = super::Xm39MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_39_sparse_transpose() {
        let mut m = super::Xm39MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_39_sparse_multiply_vec() {
        let mut m = super::Xm39MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_39_sparse_nnz_density() {
        let mut m = super::Xm39MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_39_sparse_clear() {
        let mut m = super::Xm39MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_39_sparse_overwrite_zero() {
        let mut m = super::Xm39MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_39_tokenizer_basic() {
        let t = super::Xm39Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_39_tokenizer_count() {
        let t = super::Xm39Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_39_tokenizer_unique() {
        let t = super::Xm39Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_39_tokenizer_frequency() {
        let t = super::Xm39Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_39_tokenizer_delimiter() {
        let t = super::Xm39Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_39_tokenizer_whitespace() {
        let t = super::Xm39Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_39_tokenizer_empty() {
        let t = super::Xm39Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 39 ----

    #[test]
    fn xn_39_fenwick_prefix_sum() {
        let mut ft = super::Xn39Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_39_fenwick_range_sum() {
        let mut ft = super::Xn39Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_39_fenwick_point_query() {
        let mut ft = super::Xn39Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_39_fenwick_len() {
        let ft = super::Xn39Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_39_fenwick_multiple_updates() {
        let mut ft = super::Xn39Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_39_fenwick_single_element() {
        let mut ft = super::Xn39Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_39_fenwick_find_kth() {
        let mut ft = super::Xn39Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_39_fenwick_negative_delta() {
        let mut ft = super::Xn39Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 39 ----

    #[test]
    fn xn_39_avl_insert_get() {
        let mut m = super::Xn39AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_39_avl_remove() {
        let mut m = super::Xn39AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_39_avl_in_order() {
        let mut m = super::Xn39AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_39_avl_min_max() {
        let mut m = super::Xn39AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_39_avl_floor_ceiling() {
        let mut m = super::Xn39AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_39_avl_height_balanced() {
        let mut m = super::Xn39AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_39_avl_overwrite() {
        let mut m = super::Xn39AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_39_avl_empty() {
        let m: super::Xn39AVL<i32, i32> = super::Xn39AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo39RedBlack tests ---

    #[test]
    fn xo_39_rb_insert_and_get() {
        let mut tree = super::Xo39RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_39_rb_len_and_empty() {
        let mut tree = super::Xo39RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_39_rb_min_max() {
        let mut tree = super::Xo39RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_39_rb_contains() {
        let mut tree = super::Xo39RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_39_rb_remove() {
        let mut tree = super::Xo39RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_39_rb_in_order() {
        let mut tree = super::Xo39RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_39_rb_black_height() {
        let mut tree = super::Xo39RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_39_rb_overwrite() {
        let mut tree = super::Xo39RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo39ConsistentHash tests ---

    #[test]
    fn xo_39_ch_add_and_count() {
        let mut ring = super::Xo39ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_39_ch_remove_node() {
        let mut ring = super::Xo39ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_39_ch_get_node() {
        let mut ring = super::Xo39ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_39_ch_empty_ring() {
        let ring = super::Xo39ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_39_ch_distribution() {
        let mut ring = super::Xo39ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_39_ch_rebalance() {
        let mut ring = super::Xo39ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_39_ch_virtual_nodes() {
        let mut ring = super::Xo39ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_39_ch_consistent_lookup() {
        let mut ring = super::Xo39ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_39_splay_insert_get() {
        let mut t = super::Xp39SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_39_splay_remove() {
        let mut t = super::Xp39SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_39_splay_count_increases() {
        let mut t = super::Xp39SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_39_splay_depth() {
        let mut t = super::Xp39SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_39_splay_len_empty() {
        let t = super::Xp39SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_39_splay_min_max() {
        let mut t = super::Xp39SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_39_splay_overwrite() {
        let mut t = super::Xp39SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_39_splay_remove_missing() {
        let mut t = super::Xp39SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_39 treap tests ----
    #[test]
    fn xq_39_treap_empty() {
        let t = super::Xq39Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_39_treap_insert_get() {
        let mut t = super::Xq39Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_39_treap_overwrite() {
        let mut t = super::Xq39Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_39_treap_remove() {
        let mut t = super::Xq39Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_39_treap_min_max() {
        let mut t = super::Xq39Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_39_treap_rank() {
        let mut t = super::Xq39Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_39_treap_kth() {
        let mut t = super::Xq39Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_39_treap_in_order() {
        let mut t = super::Xq39Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_39 VEB tree tests ----
    #[test]
    fn xq_39_veb_empty() {
        let v = super::Xq39VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_39_veb_insert_contains() {
        let mut v = super::Xq39VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_39_veb_min_max() {
        let mut v = super::Xq39VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_39_veb_delete() {
        let mut v = super::Xq39VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_39_veb_successor() {
        let mut v = super::Xq39VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_39_veb_predecessor() {
        let mut v = super::Xq39VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_39_veb_count() {
        let mut v = super::Xq39VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_39_veb_duplicate_insert() {
        let mut v = super::Xq39VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_39_kdtree_empty() {
        let tree = super::Xr39KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_39_kdtree_insert_one() {
        let mut tree = super::Xr39KDTree::xr_new();
        tree.xr_insert(super::Xr39KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_39_kdtree_insert_multiple() {
        let mut tree = super::Xr39KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr39KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_39_kdtree_nearest_neighbor() {
        let mut tree = super::Xr39KDTree::xr_new();
        tree.xr_insert(super::Xr39KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr39KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr39KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_39_kdtree_nn_empty() {
        let tree = super::Xr39KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr39KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_39_kdtree_range_search() {
        let mut tree = super::Xr39KDTree::xr_new();
        tree.xr_insert(super::Xr39KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr39KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr39KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_39_kdtree_range_empty() {
        let mut tree = super::Xr39KDTree::xr_new();
        tree.xr_insert(super::Xr39KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_39_kdtree_all_points() {
        let mut tree = super::Xr39KDTree::xr_new();
        tree.xr_insert(super::Xr39KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr39KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_39_kdtree_depth() {
        let mut tree = super::Xr39KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr39KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_39_kdtree_bounding_box() {
        let mut tree = super::Xr39KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr39KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr39KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_39_persistent_array_new() {
        let arr = super::Xs39PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_39_persistent_array_push() {
        let mut arr = super::Xs39PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_39_persistent_array_set() {
        let mut arr = super::Xs39PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_39_persistent_array_diff() {
        let mut arr = super::Xs39PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_39_persistent_array_rollback() {
        let mut arr = super::Xs39PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_39_persistent_array_history() {
        let mut arr = super::Xs39PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_39_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs39PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_39_persistent_array_from_vec() {
        let arr = super::Xs39PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_39_concurrent_queue_new() {
        let q = super::Xs39ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_39_concurrent_queue_push_pop() {
        let mut q = super::Xs39ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_39_concurrent_queue_full() {
        let mut q = super::Xs39ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_39_concurrent_queue_drain() {
        let mut q = super::Xs39ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_39_concurrent_queue_try_pop() {
        let mut q = super::Xs39ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_39_concurrent_queue_clear() {
        let mut q = super::Xs39ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_39_range_map_new() {
        let rm = super::Xs39RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_39_range_map_insert_get() {
        let mut rm = super::Xs39RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_39_range_map_overlap() {
        let mut rm = super::Xs39RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_39_range_map_remove() {
        let mut rm = super::Xs39RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_39_range_map_gaps() {
        let mut rm = super::Xs39RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_39_range_map_coverage() {
        let mut rm = super::Xs39RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_39_range_map_contains() {
        let mut rm = super::Xs39RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_39_range_map_clear() {
        let mut rm = super::Xs39RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_39_circular_buffer_new() {
        let buf = super::Xs39CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_39_circular_buffer_push_pop() {
        let mut buf = super::Xs39CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_39_circular_buffer_overwrite() {
        let mut buf = super::Xs39CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_39_circular_buffer_peek() {
        let mut buf = super::Xs39CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_39_circular_buffer_is_full() {
        let mut buf = super::Xs39CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_39_circular_buffer_iter() {
        let mut buf = super::Xs39CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_39_circular_buffer_clear() {
        let mut buf = super::Xs39CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_39_circular_buffer_to_vec() {
        let mut buf = super::Xs39CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }


    // --- xt_ Fibonacci Heap tests ---

    #[test]
    fn xt_fib_heap_new() {
        let h = super::XtFibonacciHeap::<i32, &str>::xt_new();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_len(), 0);
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_insert_find_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(5, "five");
        h.xt_insert(3, "three");
        h.xt_insert(7, "seven");
        assert_eq!(h.xt_len(), 3);
        assert_eq!(h.xt_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xt_fib_heap_extract_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "ten");
        h.xt_insert(2, "two");
        h.xt_insert(8, "eight");
        h.xt_insert(1, "one");
        assert_eq!(h.xt_extract_min(), Some((1, "one")));
        assert_eq!(h.xt_extract_min(), Some((2, "two")));
        assert_eq!(h.xt_len(), 2);
    }

    #[test]
    fn xt_fib_heap_extract_all_sorted() {
        let mut h = super::XtFibonacciHeap::xt_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xt_insert(v, v * 10);
        }
        let sorted = h.xt_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xt_fib_heap_decrease_key() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "a");
        let idx = h.xt_insert(20, "b");
        h.xt_insert(15, "c");
        h.xt_decrease_key(idx, 5);
        assert_eq!(h.xt_find_min(), Some((&5, &"b")));
    }

    #[test]
    fn xt_fib_heap_merge() {
        let mut h1 = super::XtFibonacciHeap::xt_new();
        h1.xt_insert(3, "three");
        h1.xt_insert(7, "seven");
        let mut h2 = super::XtFibonacciHeap::xt_new();
        h2.xt_insert(1, "one");
        h2.xt_insert(5, "five");
        h1.xt_merge(&mut h2);
        assert_eq!(h1.xt_len(), 4);
        assert_eq!(h1.xt_find_min(), Some((&1, &"one")));
        assert!(h2.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_clear() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "a");
        h.xt_insert(2, "b");
        h.xt_clear();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_single_element() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(42, "answer");
        assert_eq!(h.xt_extract_min(), Some((42, "answer")));
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_display() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "one");
        let s = format!("{}", h);
        assert!(s.contains("FibHeap"));
    }

    #[test]
    fn xt_fib_heap_default() {
        let h = super::XtFibonacciHeap::<i32, i32>::default();
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_node_display() {
        let n = super::XtFibNode::xt_new(10, "ten");
        let s = format!("{}", n);
        assert!(s.contains("FibNode"));
    }

    // --- xt_ Doubly-Linked List tests ---

    #[test]
    fn xt_dll_new() {
        let dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert!(dll.xt_is_empty());
        assert_eq!(dll.xt_len(), 0);
    }

    #[test]
    fn xt_dll_push_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_front(1);
        dll.xt_push_front(2);
        dll.xt_push_front(3);
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_push_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_pop_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_front(), Some(10));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_pop_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_back(), Some(20));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_insert_after() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(3);
        dll.xt_insert_after(a, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_insert_before() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let b = dll.xt_push_back(3);
        dll.xt_insert_before(b, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_remove_middle() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let mid = dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_remove(mid);
        assert_eq!(dll.xt_to_vec(), vec![1, 3]);
    }

    #[test]
    fn xt_dll_peek() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_peek_front(), Some(&10));
        assert_eq!(dll.xt_peek_back(), Some(&20));
    }

    #[test]
    fn xt_dll_get() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let idx = dll.xt_push_back(42);
        assert_eq!(dll.xt_get(idx), Some(&42));
        assert_eq!(dll.xt_get(999), None);
    }

    #[test]
    fn xt_dll_iter_backward() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        let rev: Vec<&i32> = dll.xt_iter_backward();
        assert_eq!(rev, vec![&3, &2, &1]);
    }

    #[test]
    fn xt_dll_cursor_navigation() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        dll.xt_push_back(30);
        let c = dll.xt_head_cursor().unwrap();
        assert_eq!(dll.xt_get(c), Some(&10));
        let c2 = dll.xt_cursor_next(c).unwrap();
        assert_eq!(dll.xt_get(c2), Some(&20));
        let c3 = dll.xt_cursor_next(c2).unwrap();
        assert_eq!(dll.xt_get(c3), Some(&30));
        assert_eq!(dll.xt_cursor_next(c3), None);
        let c2b = dll.xt_cursor_prev(c3).unwrap();
        assert_eq!(dll.xt_get(c2b), Some(&20));
    }

    #[test]
    fn xt_dll_reverse() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_reverse();
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_clear() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_clear();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_default() {
        let dll = super::XtDoublyLinkedList::<i32>::default();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_display() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let s = format!("{}", dll);
        assert!(s.contains("DLL"));
    }

    #[test]
    fn xt_dll_reuse_freed_slots() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_remove(a);
        let c = dll.xt_push_back(3);
        assert_eq!(c, a);
        assert_eq!(dll.xt_to_vec(), vec![2, 3]);
    }

    #[test]
    fn xt_dll_tail_cursor() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        let tc = dll.xt_tail_cursor().unwrap();
        assert_eq!(dll.xt_get(tc), Some(&2));
    }

    #[test]
    fn xt_dll_empty_operations() {
        let mut dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert_eq!(dll.xt_pop_front(), None);
        assert_eq!(dll.xt_pop_back(), None);
        assert_eq!(dll.xt_peek_front(), None);
        assert_eq!(dll.xt_peek_back(), None);
        assert_eq!(dll.xt_head_cursor(), None);
        assert_eq!(dll.xt_tail_cursor(), None);
    }


    // --- xu_ Binomial Heap tests ---

    #[test]
    fn xu_bin_heap_new() {
        let h = super::XuBinomialHeap::<i32, &str>::xu_new();
        assert!(h.xu_is_empty());
        assert_eq!(h.xu_len(), 0);
    }

    #[test]
    fn xu_bin_heap_insert_find_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(5, "five");
        h.xu_insert(3, "three");
        h.xu_insert(7, "seven");
        assert_eq!(h.xu_len(), 3);
        assert_eq!(h.xu_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xu_bin_heap_extract_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(10, "a");
        h.xu_insert(2, "b");
        h.xu_insert(8, "c");
        h.xu_insert(1, "d");
        assert_eq!(h.xu_extract_min(), Some((1, "d")));
        assert_eq!(h.xu_extract_min(), Some((2, "b")));
    }

    #[test]
    fn xu_bin_heap_sorted_drain() {
        let mut h = super::XuBinomialHeap::xu_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xu_insert(v, v * 10);
        }
        let sorted = h.xu_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xu_bin_heap_merge() {
        let mut h1 = super::XuBinomialHeap::xu_new();
        h1.xu_insert(3, "a");
        h1.xu_insert(7, "b");
        let mut h2 = super::XuBinomialHeap::xu_new();
        h2.xu_insert(1, "c");
        h2.xu_insert(5, "d");
        h1.xu_merge(&mut h2);
        assert_eq!(h1.xu_len(), 4);
        assert_eq!(h1.xu_find_min(), Some((&1, &"c")));
    }

    #[test]
    fn xu_bin_heap_clear() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "a");
        h.xu_clear();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_heap_display() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "x");
        assert!(format!("{}", h).contains("BinHeap"));
    }

    #[test]
    fn xu_bin_heap_default() {
        let h = super::XuBinomialHeap::<i32, i32>::default();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_node_display() {
        let n = super::XuBinomialNode::xu_new(5, "v");
        assert!(format!("{}", n).contains("BinNode"));
    }

    #[test]
    fn xu_bin_heap_single() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(42, "answer");
        assert_eq!(h.xu_extract_min(), Some((42, "answer")));
        assert!(h.xu_is_empty());
    }

    // --- xu_ Disjoint Sparse Table tests ---

    #[test]
    fn xu_dst_build() {
        let data = vec![1, 2, 3, 4, 5];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_len(), 5);
        assert!(!dst.xu_is_empty());
    }

    #[test]
    fn xu_dst_single_element_query() {
        let data = vec![10, 20, 30];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_query(0, 0), 10);
        assert_eq!(dst.xu_query(1, 1), 20);
        assert_eq!(dst.xu_query(2, 2), 30);
    }

    #[test]
    fn xu_dst_get() {
        let data = vec![5, 10, 15];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_get(0), Some(&5));
        assert_eq!(dst.xu_get(2), Some(&15));
        assert_eq!(dst.xu_get(10), None);
    }

    #[test]
    fn xu_dst_empty() {
        let dst = super::XuDisjointSparseTable::<i32>::xu_build(&[]);
        assert!(dst.xu_is_empty());
        assert_eq!(dst.xu_len(), 0);
    }

    #[test]
    fn xu_dst_display() {
        let data = vec![1, 2, 3];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert!(format!("{}", dst).contains("DST"));
    }

    // --- xu_ Monotonic Stack tests ---

    #[test]
    fn xu_mono_stack_increasing() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        assert!(s.xu_is_empty());
        let popped = s.xu_push(3);
        assert!(popped.is_empty());
        let popped = s.xu_push(5);
        assert!(popped.is_empty());
        let popped = s.xu_push(2);
        assert_eq!(popped, vec![5, 3]);
        assert_eq!(s.xu_as_slice(), &[2]);
    }

    #[test]
    fn xu_mono_stack_decreasing() {
        let mut s = super::XuMonotonicStack::xu_decreasing();
        s.xu_push(2);
        s.xu_push(1);
        let popped = s.xu_push(5);
        assert_eq!(popped, vec![1, 2]);
        assert_eq!(s.xu_as_slice(), &[5]);
    }

    #[test]
    fn xu_mono_stack_peek_pop() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(3);
        s.xu_push(5);
        assert_eq!(s.xu_peek(), Some(&5));
        assert_eq!(s.xu_pop(), Some(5));
        assert_eq!(s.xu_len(), 2);
    }

    #[test]
    fn xu_mono_stack_clear() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(2);
        s.xu_clear();
        assert!(s.xu_is_empty());
    }

    #[test]
    fn xu_mono_stack_display() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        assert!(format!("{}", s).contains("MonoStack"));
    }


    // --- xv_ Cartesian Tree tests ---

    #[test]
    fn xv_cart_tree_new() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_cart_tree_insert_contains() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        t.xv_insert(3, 2);
        t.xv_insert(7, 3);
        assert!(t.xv_contains(&5));
        assert!(t.xv_contains(&3));
        assert!(t.xv_contains(&7));
        assert!(!t.xv_contains(&4));
        assert_eq!(t.xv_len(), 3);
    }

    #[test]
    fn xv_cart_tree_inorder() {
        let mut t = super::XvCartesianTree::xv_new();
        for (k, p) in [(5, 3), (3, 1), (7, 2), (1, 5), (9, 4)] {
            t.xv_insert(k, p);
        }
        let keys = t.xv_inorder();
        assert_eq!(keys, vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn xv_cart_tree_min_priority() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 10);
        t.xv_insert(3, 2);
        t.xv_insert(7, 5);
        assert_eq!(t.xv_min_priority(), Some(&2));
    }

    #[test]
    fn xv_cart_tree_from_pairs() {
        let t = super::XvCartesianTree::xv_from_pairs(&[(3, 1), (1, 3), (5, 2)]);
        assert_eq!(t.xv_len(), 3);
        assert!(t.xv_contains(&1));
    }

    #[test]
    fn xv_cart_tree_height() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        assert!(t.xv_height() >= 1);
    }

    #[test]
    fn xv_cart_tree_clear() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(1, 1);
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_tree_display() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("CartTree"));
    }

    #[test]
    fn xv_cart_tree_default() {
        let t = super::XvCartesianTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_node_display() {
        let n = super::XvCartesianNode { xv_key: 1, xv_priority: 2, xv_left: None, xv_right: None };
        assert!(format!("{}", n).contains("CartNode"));
    }

    // --- xv_ Weight-Balanced Tree tests ---

    #[test]
    fn xv_wb_tree_new() {
        let t = super::XvWeightBalancedTree::<i32, &str>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_wb_tree_insert_get() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "five");
        t.xv_insert(3, "three");
        t.xv_insert(7, "seven");
        assert_eq!(t.xv_get(&5), Some(&"five"));
        assert_eq!(t.xv_get(&3), Some(&"three"));
        assert_eq!(t.xv_get(&7), Some(&"seven"));
        assert_eq!(t.xv_get(&4), None);
    }

    #[test]
    fn xv_wb_tree_contains() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(10, "a");
        assert!(t.xv_contains(&10));
        assert!(!t.xv_contains(&20));
    }

    #[test]
    fn xv_wb_tree_keys_sorted() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xv_insert(k, k * 10);
        }
        assert_eq!(t.xv_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xv_wb_tree_replace_value() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "old");
        t.xv_insert(5, "new");
        assert_eq!(t.xv_get(&5), Some(&"new"));
        assert_eq!(t.xv_len(), 1);
    }

    #[test]
    fn xv_wb_tree_height() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in 1..=15 {
            t.xv_insert(k, k);
        }
        assert!(t.xv_height() <= 20);
    }

    #[test]
    fn xv_wb_tree_clear() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(1, "a");
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_tree_display() {
        let t = super::XvWeightBalancedTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("WBTree"));
    }

    #[test]
    fn xv_wb_tree_default() {
        let t = super::XvWeightBalancedTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_node_display() {
        let n = super::XvWBNode { xv_key: 1, xv_value: "a", xv_left: None, xv_right: None, xv_weight: 2 };
        assert!(format!("{}", n).contains("WBNode"));
    }


    // --- xw_ Scapegoat Tree tests ---

    #[test]
    fn xw_sg_tree_new() {
        let t = super::XwScapegoatTree::<i32, &str>::xw_new();
        assert!(t.xw_is_empty());
        assert_eq!(t.xw_len(), 0);
    }

    #[test]
    fn xw_sg_tree_insert_get() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(5, "five");
        t.xw_insert(3, "three");
        t.xw_insert(7, "seven");
        assert_eq!(t.xw_get(&5), Some(&"five"));
        assert_eq!(t.xw_get(&3), Some(&"three"));
        assert_eq!(t.xw_get(&4), None);
    }

    #[test]
    fn xw_sg_tree_contains() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(10, "a");
        assert!(t.xw_contains(&10));
        assert!(!t.xw_contains(&20));
    }

    #[test]
    fn xw_sg_tree_keys_sorted() {
        let mut t = super::XwScapegoatTree::xw_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xw_insert(k, k * 10);
        }
        assert_eq!(t.xw_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xw_sg_tree_sequential_inserts() {
        let mut t = super::XwScapegoatTree::xw_new();
        for k in 1..=20 {
            t.xw_insert(k, k);
        }
        assert_eq!(t.xw_len(), 20);
        assert!(t.xw_height() <= 15);
    }

    #[test]
    fn xw_sg_tree_replace_value() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(5, "old");
        t.xw_insert(5, "new");
        assert_eq!(t.xw_get(&5), Some(&"new"));
        assert_eq!(t.xw_len(), 1);
    }

    #[test]
    fn xw_sg_tree_clear() {
        let mut t = super::XwScapegoatTree::xw_new();
        t.xw_insert(1, "a");
        t.xw_clear();
        assert!(t.xw_is_empty());
    }

    #[test]
    fn xw_sg_tree_display() {
        let t = super::XwScapegoatTree::<i32, i32>::xw_new();
        assert!(format!("{}", t).contains("SGTree"));
    }

    #[test]
    fn xw_sg_tree_default() {
        let t = super::XwScapegoatTree::<i32, i32>::default();
        assert!(t.xw_is_empty());
    }

    #[test]
    fn xw_sg_node_display() {
        let n = super::XwScapegoatNode { xw_key: 1, xw_value: "a", xw_left: None, xw_right: None };
        assert!(format!("{}", n).contains("SGNode"));
    }

    // --- xw_ Rope tests ---

    #[test]
    fn xw_rope_new() {
        let r = super::XwRope::xw_new();
        assert!(r.xw_is_empty());
        assert_eq!(r.xw_len(), 0);
    }

    #[test]
    fn xw_rope_from_str() {
        let r = super::XwRope::xw_from_str("hello");
        assert_eq!(r.xw_len(), 5);
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_concat() {
        let a = super::XwRope::xw_from_str("hello ");
        let b = super::XwRope::xw_from_str("world");
        let c = super::XwRope::xw_concat(a, b);
        assert_eq!(c.xw_to_string(), "hello world");
    }

    #[test]
    fn xw_rope_insert() {
        let mut r = super::XwRope::xw_from_str("helo");
        r.xw_insert(3, "l");
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_delete() {
        let mut r = super::XwRope::xw_from_str("hello world");
        r.xw_delete(5, 11);
        assert_eq!(r.xw_to_string(), "hello");
    }

    #[test]
    fn xw_rope_append() {
        let mut r = super::XwRope::xw_from_str("hello");
        r.xw_append(" world");
        assert_eq!(r.xw_to_string(), "hello world");
    }

    #[test]
    fn xw_rope_substring() {
        let r = super::XwRope::xw_from_str("hello world");
        assert_eq!(r.xw_substring(6, 11), "world");
    }

    #[test]
    fn xw_rope_char_at() {
        let r = super::XwRope::xw_from_str("abc");
        assert_eq!(r.xw_char_at(0), Some('a'));
        assert_eq!(r.xw_char_at(2), Some('c'));
    }

    #[test]
    fn xw_rope_clear() {
        let mut r = super::XwRope::xw_from_str("text");
        r.xw_clear();
        assert!(r.xw_is_empty());
    }

    #[test]
    fn xw_rope_display() {
        let r = super::XwRope::xw_from_str("test");
        assert!(format!("{}", r).contains("Rope"));
    }

    #[test]
    fn xw_rope_default() {
        let r = super::XwRope::default();
        assert!(r.xw_is_empty());
    }

    #[test]
    fn xw_rope_empty_ops() {
        let r = super::XwRope::xw_new();
        assert_eq!(r.xw_to_string(), "");
        assert_eq!(r.xw_substring(0, 5), "");
    }


    // --- xx_ Skip List tests ---

    #[test]
    fn xx_skip_list_new() {
        let sl = super::XxSkipList::<i32, &str>::xx_new();
        assert!(sl.xx_is_empty());
        assert_eq!(sl.xx_len(), 0);
    }

    #[test]
    fn xx_skip_list_insert_get() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(5, "five");
        sl.xx_insert(3, "three");
        sl.xx_insert(7, "seven");
        assert_eq!(sl.xx_get(&5), Some(&"five"));
        assert_eq!(sl.xx_get(&3), Some(&"three"));
        assert_eq!(sl.xx_get(&7), Some(&"seven"));
        assert_eq!(sl.xx_get(&4), None);
    }

    #[test]
    fn xx_skip_list_contains() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(10, "a");
        assert!(sl.xx_contains(&10));
        assert!(!sl.xx_contains(&20));
    }

    #[test]
    fn xx_skip_list_keys_sorted() {
        let mut sl = super::XxSkipList::xx_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            sl.xx_insert(k, k * 10);
        }
        assert_eq!(sl.xx_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xx_skip_list_replace() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(5, "old");
        sl.xx_insert(5, "new");
        assert_eq!(sl.xx_get(&5), Some(&"new"));
    }

    #[test]
    fn xx_skip_list_many() {
        let mut sl = super::XxSkipList::xx_new();
        for k in 1..=50 {
            sl.xx_insert(k, k);
        }
        assert_eq!(sl.xx_len(), 50);
        for k in 1..=50 {
            assert!(sl.xx_contains(&k));
        }
    }

    #[test]
    fn xx_skip_list_clear() {
        let mut sl = super::XxSkipList::xx_new();
        sl.xx_insert(1, "a");
        sl.xx_clear();
        assert!(sl.xx_is_empty());
    }

    #[test]
    fn xx_skip_list_display() {
        let sl = super::XxSkipList::<i32, i32>::xx_new();
        assert!(format!("{}", sl).contains("SkipList"));
    }

    #[test]
    fn xx_skip_list_default() {
        let sl = super::XxSkipList::<i32, i32>::default();
        assert!(sl.xx_is_empty());
    }

    #[test]
    fn xx_skip_node_display() {
        let n = super::XxSkipNode::<i32, i32> { xx_key: Some(5), xx_value: Some(50), xx_forward: vec![None] };
        assert!(format!("{}", n).contains("SkipNode"));
    }

    // --- xx_ Suffix Array tests ---

    #[test]
    fn xx_suffix_array_new() {
        let sa = super::XxSuffixArray::xx_new("banana");
        assert_eq!(sa.xx_len(), 6);
        assert!(!sa.xx_is_empty());
    }

    #[test]
    fn xx_suffix_array_search() {
        let sa = super::XxSuffixArray::xx_new("banana");
        let pos = sa.xx_search("ana");
        assert_eq!(pos.len(), 2);
    }

    #[test]
    fn xx_suffix_array_count() {
        let sa = super::XxSuffixArray::xx_new("abcabcabc");
        assert_eq!(sa.xx_count("abc"), 3);
    }

    #[test]
    fn xx_suffix_array_no_match() {
        let sa = super::XxSuffixArray::xx_new("hello");
        assert_eq!(sa.xx_count("xyz"), 0);
    }

    #[test]
    fn xx_suffix_array_suffix_at() {
        let sa = super::XxSuffixArray::xx_new("abc");
        let s = sa.xx_suffix_at(0);
        assert!(!s.is_empty());
    }

    #[test]
    fn xx_suffix_array_longest_repeated() {
        let sa = super::XxSuffixArray::xx_new("banana");
        let lr = sa.xx_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xx_suffix_array_empty() {
        let sa = super::XxSuffixArray::xx_new("");
        assert!(sa.xx_is_empty());
        assert_eq!(sa.xx_search("a").len(), 0);
    }

    #[test]
    fn xx_suffix_array_display() {
        let sa = super::XxSuffixArray::xx_new("test");
        assert!(format!("{}", sa).contains("SuffixArray"));
    }

    #[test]
    fn xx_suffix_array_default() {
        let sa = super::XxSuffixArray::default();
        assert!(sa.xx_is_empty());
    }

    #[test]
    fn xx_suffix_array_text() {
        let sa = super::XxSuffixArray::xx_new("hello");
        assert_eq!(sa.xx_text(), "hello");
    }

}
