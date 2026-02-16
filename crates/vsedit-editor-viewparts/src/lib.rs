//! Editor view parts: view zones, overlay widgets, content widgets, glyph margins,
//! minimap configuration, and breadcrumb navigation.

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
}
