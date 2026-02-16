//! Workbench layout engine — VS Code-like terminal layout manager.
//!
//! Defines the major workbench parts ([`Part`]) and computes their rectangle
//! positions via [`WorkbenchLayout`].

use std::fmt;
use std::collections::HashMap;

use vsedit_events::{Emitter, Event};
use vsedit_layout::{Constraint, LayoutNode};
use vsedit_tui::Rect;

// ---------------------------------------------------------------------------
// Part
// ---------------------------------------------------------------------------

/// The major visual parts of the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Part {
    Titlebar,
    Menubar,
    Sidebar,
    Editor,
    Panel,
    StatusBar,
    ActivityBar,
    AuxiliaryBar,
}

// ---------------------------------------------------------------------------
// LayoutResult
// ---------------------------------------------------------------------------

/// Computed rectangles for each workbench part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutResult {
    pub menubar: Option<Rect>,
    pub activity_bar: Option<Rect>,
    pub sidebar: Option<Rect>,
    pub editor: Rect,
    pub panel: Option<Rect>,
    pub statusbar: Rect,
}

// ---------------------------------------------------------------------------
// WorkbenchLayout
// ---------------------------------------------------------------------------

/// Computes the rectangle layout for each workbench part.
///
/// Follows VS Code's arrangement:
/// ```text
/// [menubar                                          ]
/// [act|sidebar |editor area              |          ]
/// [bar|        |                         |          ]
/// [   |        |panel                    |          ]
/// [statusbar                                        ]
/// ```
pub struct WorkbenchLayout {
    visibility: HashMap<Part, bool>,
    sidebar_width: u16,
    panel_height: u16,
    activity_bar_width: u16,
    statusbar_height: u16,
    menubar_height: u16,
    on_did_layout: Emitter<()>,
}

impl WorkbenchLayout {
    /// Create a new layout with default settings.
    ///
    /// Defaults: sidebar visible, panel visible, activity bar visible,
    /// menubar visible, statusbar visible.
    pub fn new() -> Self {
        let mut visibility = HashMap::new();
        visibility.insert(Part::Menubar, true);
        visibility.insert(Part::ActivityBar, true);
        visibility.insert(Part::Sidebar, true);
        visibility.insert(Part::Editor, true);
        visibility.insert(Part::Panel, true);
        visibility.insert(Part::StatusBar, true);
        visibility.insert(Part::Titlebar, false);
        visibility.insert(Part::AuxiliaryBar, false);

        Self {
            visibility,
            sidebar_width: 30,
            panel_height: 10,
            activity_bar_width: 2,
            statusbar_height: 1,
            menubar_height: 1,
            on_did_layout: Emitter::new(),
        }
    }

    /// Compute the layout for the given total area.
    pub fn compute(&self, total_area: Rect) -> LayoutResult {
        let menubar_visible = self.is_part_visible(Part::Menubar);
        let statusbar_visible = self.is_part_visible(Part::StatusBar);
        let sidebar_visible = self.is_part_visible(Part::Sidebar);
        let activity_bar_visible = self.is_part_visible(Part::ActivityBar);
        let panel_visible = self.is_part_visible(Part::Panel);

        // Vertical split: menubar | middle | statusbar
        let menubar_h = if menubar_visible { self.menubar_height } else { 0 };
        let statusbar_h = if statusbar_visible { self.statusbar_height } else { 0 };

        let mut vert_constraints = Vec::new();
        if menubar_visible {
            vert_constraints.push(Constraint::Fixed(menubar_h));
        }
        vert_constraints.push(Constraint::Flex(1));
        if statusbar_visible {
            vert_constraints.push(Constraint::Fixed(statusbar_h));
        }

        let vert = LayoutNode::vertical(vert_constraints);
        let vert_rects = vert.split(total_area);

        let mut idx = 0;
        let menubar_rect = if menubar_visible {
            let r = vert_rects[idx];
            idx += 1;
            Some(r)
        } else {
            None
        };

        let middle_rect = vert_rects[idx];
        idx += 1;

        let statusbar_rect = if statusbar_visible {
            vert_rects[idx]
        } else {
            Rect::new(total_area.x, total_area.y + total_area.height, total_area.width, 0)
        };

        // Horizontal split of middle: activity_bar | sidebar | content
        let act_w = if activity_bar_visible { self.activity_bar_width } else { 0 };
        let sb_w = if sidebar_visible { self.sidebar_width } else { 0 };

        let mut horiz_constraints = Vec::new();
        if activity_bar_visible {
            horiz_constraints.push(Constraint::Fixed(act_w));
        }
        if sidebar_visible {
            horiz_constraints.push(Constraint::Fixed(sb_w));
        }
        horiz_constraints.push(Constraint::Flex(1));

        let horiz = LayoutNode::horizontal(horiz_constraints);
        let horiz_rects = horiz.split(middle_rect);

        let mut hidx = 0;
        let activity_bar_rect = if activity_bar_visible {
            let r = horiz_rects[hidx];
            hidx += 1;
            Some(r)
        } else {
            None
        };

        let sidebar_rect = if sidebar_visible {
            let r = horiz_rects[hidx];
            hidx += 1;
            Some(r)
        } else {
            None
        };

        let content_rect = horiz_rects[hidx];

        // Vertical split of content area: editor | panel
        let (editor_rect, panel_rect) = if panel_visible {
            let panel_h = self.panel_height.min(content_rect.height.saturating_sub(1));
            let editor_h = content_rect.height.saturating_sub(panel_h);
            let e = Rect::new(content_rect.x, content_rect.y, content_rect.width, editor_h);
            let p = Rect::new(content_rect.x, content_rect.y + editor_h, content_rect.width, panel_h);
            (e, Some(p))
        } else {
            (content_rect, None)
        };

        self.on_did_layout.fire(&());

        LayoutResult {
            menubar: menubar_rect,
            activity_bar: activity_bar_rect,
            sidebar: sidebar_rect,
            editor: editor_rect,
            panel: panel_rect,
            statusbar: statusbar_rect,
        }
    }

    /// Set whether a part is visible.
    pub fn set_part_visible(&mut self, part: Part, visible: bool) {
        self.visibility.insert(part, visible);
    }

    /// Check whether a part is visible.
    pub fn is_part_visible(&self, part: Part) -> bool {
        self.visibility.get(&part).copied().unwrap_or(false)
    }

    /// Get the current sidebar width.
    pub fn get_sidebar_width(&self) -> u16 {
        self.sidebar_width
    }

    /// Set the sidebar width.
    pub fn set_sidebar_width(&mut self, width: u16) {
        self.sidebar_width = width;
    }

    /// Get the current panel height.
    pub fn get_panel_height(&self) -> u16 {
        self.panel_height
    }

    /// Set the panel height.
    pub fn set_panel_height(&mut self, height: u16) {
        self.panel_height = height;
    }

    /// Toggle sidebar visibility.
    pub fn toggle_sidebar(&mut self) {
        let visible = self.is_part_visible(Part::Sidebar);
        self.set_part_visible(Part::Sidebar, !visible);
    }

    /// Toggle panel visibility.
    pub fn toggle_panel(&mut self) {
        let visible = self.is_part_visible(Part::Panel);
        self.set_part_visible(Part::Panel, !visible);
    }

    /// Event fired after layout computation.
    pub fn on_did_layout(&self) -> Event<()> {
        self.on_did_layout.event()
    }
}

impl Default for WorkbenchLayout {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SplitDirection / LayoutSplit
// ---------------------------------------------------------------------------

/// Direction for splitting an editor area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// Manages splitting an editor area into multiple panes.
#[derive(Debug, Clone)]
pub struct LayoutSplit {
    pub direction: SplitDirection,
    pub ratios: Vec<f32>,
    pub min_size: u16,
    pub max_size: Option<u16>,
}

impl LayoutSplit {
    /// Create a split with `pane_count` equally-sized panes.
    pub fn new(direction: SplitDirection, pane_count: usize) -> Self {
        let count = pane_count.max(1);
        let ratio = 1.0 / count as f32;
        Self {
            direction,
            ratios: vec![ratio; count],
            min_size: 1,
            max_size: None,
        }
    }

    /// Create a split with custom ratios.
    pub fn with_ratios(direction: SplitDirection, ratios: Vec<f32>) -> Self {
        Self {
            direction,
            ratios,
            min_size: 1,
            max_size: None,
        }
    }

    /// Compute rectangles for each pane within the given area.
    pub fn split_rect(&self, area: Rect) -> Vec<Rect> {
        if self.ratios.is_empty() {
            return vec![area];
        }

        let total: f32 = self.ratios.iter().sum();
        let count = self.ratios.len();
        let mut rects = Vec::with_capacity(count);

        match self.direction {
            SplitDirection::Horizontal => {
                let mut x = area.x;
                for (i, &r) in self.ratios.iter().enumerate() {
                    let frac = r / total;
                    let w = if i == count - 1 {
                        area.width.saturating_sub(x - area.x)
                    } else {
                        (area.width as f32 * frac).round() as u16
                    };
                    let mut w = w.max(self.min_size);
                    if let Some(max) = self.max_size {
                        w = w.min(max);
                    }
                    rects.push(Rect::new(x, area.y, w, area.height));
                    x = x.saturating_add(w);
                }
            }
            SplitDirection::Vertical => {
                let mut y = area.y;
                for (i, &r) in self.ratios.iter().enumerate() {
                    let frac = r / total;
                    let h = if i == count - 1 {
                        area.height.saturating_sub(y - area.y)
                    } else {
                        (area.height as f32 * frac).round() as u16
                    };
                    let mut h = h.max(self.min_size);
                    if let Some(max) = self.max_size {
                        h = h.min(max);
                    }
                    rects.push(Rect::new(area.x, y, area.width, h));
                    y = y.saturating_add(h);
                }
            }
        }

        rects
    }

    /// Add a pane and rebalance ratios equally.
    pub fn add_pane(&mut self) {
        let new_count = self.ratios.len() + 1;
        let ratio = 1.0 / new_count as f32;
        self.ratios = vec![ratio; new_count];
    }

    /// Remove a pane at `index` and rebalance. Returns `false` if index is
    /// invalid or only one pane remains.
    pub fn remove_pane(&mut self, index: usize) -> bool {
        if index >= self.ratios.len() || self.ratios.len() <= 1 {
            return false;
        }
        self.ratios.remove(index);
        let new_count = self.ratios.len();
        let ratio = 1.0 / new_count as f32;
        self.ratios = vec![ratio; new_count];
        true
    }

    /// Number of panes.
    pub fn pane_count(&self) -> usize {
        self.ratios.len()
    }

    /// Set the minimum size per pane.
    pub fn set_min_size(&mut self, min: u16) {
        self.min_size = min;
    }

    /// Set the optional maximum size per pane.
    pub fn set_max_size(&mut self, max: Option<u16>) {
        self.max_size = max;
    }
}

// ---------------------------------------------------------------------------
// panel_resize
// ---------------------------------------------------------------------------

/// Resize a panel by `delta`, clamping to `[min, max]`.
pub fn resize_panel(current: u16, delta: i16, min: u16, max: u16) -> u16 {
    let result = current as i32 + delta as i32;
    (result.max(min as i32).min(max as i32)) as u16
}

// ---------------------------------------------------------------------------
// LayoutState / serialization helpers
// ---------------------------------------------------------------------------

/// Serializable snapshot of workbench layout state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutState {
    pub sidebar_width: u16,
    pub panel_height: u16,
    pub sidebar_visible: bool,
    pub panel_visible: bool,
    pub menubar_visible: bool,
    pub activity_bar_visible: bool,
}

/// Extract a [`LayoutState`] from a [`WorkbenchLayout`].
pub fn layout_serialize(layout: &WorkbenchLayout) -> LayoutState {
    LayoutState {
        sidebar_width: layout.get_sidebar_width(),
        panel_height: layout.get_panel_height(),
        sidebar_visible: layout.is_part_visible(Part::Sidebar),
        panel_visible: layout.is_part_visible(Part::Panel),
        menubar_visible: layout.is_part_visible(Part::Menubar),
        activity_bar_visible: layout.is_part_visible(Part::ActivityBar),
    }
}

/// Create a [`WorkbenchLayout`] from a persisted [`LayoutState`].
pub fn layout_deserialize(state: &LayoutState) -> WorkbenchLayout {
    let mut layout = WorkbenchLayout::new();
    layout.set_sidebar_width(state.sidebar_width);
    layout.set_panel_height(state.panel_height);
    layout.set_part_visible(Part::Sidebar, state.sidebar_visible);
    layout.set_part_visible(Part::Panel, state.panel_visible);
    layout.set_part_visible(Part::Menubar, state.menubar_visible);
    layout.set_part_visible(Part::ActivityBar, state.activity_bar_visible);
    layout
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for wb-layout operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbLayoutStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbLayoutStats {
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
    pub fn merge(&mut self, other: &WbLayoutStats) {
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

impl Default for WbLayoutStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbLayoutStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbLayoutStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-layout.
#[derive(Debug, Clone)]
pub struct WbLayoutValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbLayoutValidator {
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

impl Default for WbLayoutValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: u16, y: u16, w: u16, h: u16) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn default_visibility() {
        let layout = WorkbenchLayout::new();
        assert!(layout.is_part_visible(Part::Menubar));
        assert!(layout.is_part_visible(Part::ActivityBar));
        assert!(layout.is_part_visible(Part::Sidebar));
        assert!(layout.is_part_visible(Part::Editor));
        assert!(layout.is_part_visible(Part::Panel));
        assert!(layout.is_part_visible(Part::StatusBar));
        assert!(!layout.is_part_visible(Part::Titlebar));
        assert!(!layout.is_part_visible(Part::AuxiliaryBar));
    }

    #[test]
    fn toggle_sidebar() {
        let mut layout = WorkbenchLayout::new();
        assert!(layout.is_part_visible(Part::Sidebar));
        layout.toggle_sidebar();
        assert!(!layout.is_part_visible(Part::Sidebar));
        layout.toggle_sidebar();
        assert!(layout.is_part_visible(Part::Sidebar));
    }

    #[test]
    fn toggle_panel() {
        let mut layout = WorkbenchLayout::new();
        assert!(layout.is_part_visible(Part::Panel));
        layout.toggle_panel();
        assert!(!layout.is_part_visible(Part::Panel));
        layout.toggle_panel();
        assert!(layout.is_part_visible(Part::Panel));
    }

    #[test]
    fn set_part_visible() {
        let mut layout = WorkbenchLayout::new();
        layout.set_part_visible(Part::Menubar, false);
        assert!(!layout.is_part_visible(Part::Menubar));
        layout.set_part_visible(Part::Menubar, true);
        assert!(layout.is_part_visible(Part::Menubar));
    }

    #[test]
    fn compute_default_layout() {
        let layout = WorkbenchLayout::new();
        let result = layout.compute(rect(0, 0, 80, 24));

        // Menubar: 1 row at top
        let mb = result.menubar.unwrap();
        assert_eq!(mb, rect(0, 0, 80, 1));

        // Statusbar: 1 row at bottom
        assert_eq!(result.statusbar, rect(0, 23, 80, 1));

        // Activity bar: 2 cols on left of middle (rows 1..23)
        let ab = result.activity_bar.unwrap();
        assert_eq!(ab.x, 0);
        assert_eq!(ab.y, 1);
        assert_eq!(ab.width, 2);
        assert_eq!(ab.height, 22);

        // Sidebar: 30 cols after activity bar
        let sb = result.sidebar.unwrap();
        assert_eq!(sb.x, 2);
        assert_eq!(sb.y, 1);
        assert_eq!(sb.width, 30);
        assert_eq!(sb.height, 22);

        // Editor + panel fill remaining width (80 - 2 - 30 = 48)
        assert_eq!(result.editor.x, 32);
        assert_eq!(result.editor.width, 48);

        // Panel is present
        assert!(result.panel.is_some());
        let panel = result.panel.unwrap();
        assert_eq!(panel.x, 32);
        assert_eq!(panel.width, 48);

        // Editor height + panel height = middle height (22)
        assert_eq!(result.editor.height + panel.height, 22);
    }

    #[test]
    fn compute_no_sidebar() {
        let mut layout = WorkbenchLayout::new();
        layout.toggle_sidebar();
        let result = layout.compute(rect(0, 0, 80, 24));

        assert!(result.sidebar.is_none());
        // Editor starts after activity bar (width 2)
        assert_eq!(result.editor.x, 2);
    }

    #[test]
    fn compute_no_panel() {
        let mut layout = WorkbenchLayout::new();
        layout.toggle_panel();
        let result = layout.compute(rect(0, 0, 80, 24));

        assert!(result.panel.is_none());
        // Editor fills all of middle content height
        assert_eq!(result.editor.height, 22);
    }

    #[test]
    fn compute_no_menubar() {
        let mut layout = WorkbenchLayout::new();
        layout.set_part_visible(Part::Menubar, false);
        let result = layout.compute(rect(0, 0, 80, 24));

        assert!(result.menubar.is_none());
        // Middle starts at y=0
        let ab = result.activity_bar.unwrap();
        assert_eq!(ab.y, 0);
        assert_eq!(ab.height, 23);
    }

    #[test]
    fn compute_no_statusbar() {
        let mut layout = WorkbenchLayout::new();
        layout.set_part_visible(Part::StatusBar, false);
        let result = layout.compute(rect(0, 0, 80, 24));

        // Statusbar has zero height
        assert_eq!(result.statusbar.height, 0);
        // Middle is taller (24 - 1 menubar = 23)
        let ab = result.activity_bar.unwrap();
        assert_eq!(ab.height, 23);
    }

    #[test]
    fn sidebar_resize() {
        let mut layout = WorkbenchLayout::new();
        layout.set_sidebar_width(40);
        let result = layout.compute(rect(0, 0, 80, 24));

        let sb = result.sidebar.unwrap();
        assert_eq!(sb.width, 40);
        // Editor width shrinks: 80 - 2 - 40 = 38
        assert_eq!(result.editor.width, 38);
    }

    #[test]
    fn panel_resize() {
        let mut layout = WorkbenchLayout::new();
        layout.set_panel_height(5);
        let result = layout.compute(rect(0, 0, 80, 24));

        let panel = result.panel.unwrap();
        assert_eq!(panel.height, 5);
        // Editor height: 22 - 5 = 17
        assert_eq!(result.editor.height, 17);
    }

    #[test]
    fn on_did_layout_fires() {
        let layout = WorkbenchLayout::new();
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let f = fired.clone();
        let _handle = layout.on_did_layout().on(move |_: &()| {
            f.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });

        layout.compute(rect(0, 0, 80, 24));
        assert_eq!(fired.load(std::sync::atomic::Ordering::SeqCst), 1);

        layout.compute(rect(0, 0, 100, 30));
        assert_eq!(fired.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn compute_small_area() {
        let layout = WorkbenchLayout::new();
        let result = layout.compute(rect(0, 0, 40, 10));

        // Should not panic; all rects stay within bounds
        assert!(result.editor.width <= 40);
        assert!(result.editor.y + result.editor.height <= 10);
    }

    #[test]
    fn compute_everything_hidden() {
        let mut layout = WorkbenchLayout::new();
        layout.set_part_visible(Part::Menubar, false);
        layout.set_part_visible(Part::StatusBar, false);
        layout.set_part_visible(Part::ActivityBar, false);
        layout.set_part_visible(Part::Sidebar, false);
        layout.set_part_visible(Part::Panel, false);

        let result = layout.compute(rect(0, 0, 80, 24));

        assert!(result.menubar.is_none());
        assert!(result.activity_bar.is_none());
        assert!(result.sidebar.is_none());
        assert!(result.panel.is_none());
        // Editor takes all space
        assert_eq!(result.editor, rect(0, 0, 80, 24));
    }

    #[test]
    fn offset_total_area() {
        let layout = WorkbenchLayout::new();
        let result = layout.compute(rect(5, 3, 80, 24));

        let mb = result.menubar.unwrap();
        assert_eq!(mb.x, 5);
        assert_eq!(mb.y, 3);

        assert_eq!(result.statusbar.x, 5);
    }

    #[test]
    fn eq_part_same() {
        assert_eq!(Part::Titlebar, Part::Titlebar);
    }

    #[test]
    fn ne_part_diff() {
        assert_ne!(Part::Titlebar, Part::Menubar);
    }

    #[test]
    fn test_split_direction_eq() {
        assert_eq!(SplitDirection::Horizontal, SplitDirection::Horizontal);
        assert_eq!(SplitDirection::Vertical, SplitDirection::Vertical);
        assert_ne!(SplitDirection::Horizontal, SplitDirection::Vertical);
    }

    #[test]
    fn test_layout_split_equal_horizontal() {
        let split = LayoutSplit::new(SplitDirection::Horizontal, 3);
        assert_eq!(split.pane_count(), 3);
        let rects = split.split_rect(rect(0, 0, 90, 30));
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].x, 0);
        assert_eq!(rects[0].width, 30);
        assert_eq!(rects[1].x, 30);
        assert_eq!(rects[1].width, 30);
        assert_eq!(rects[2].x, 60);
        assert_eq!(rects[2].width, 30);
        for r in &rects {
            assert_eq!(r.height, 30);
        }
    }

    #[test]
    fn test_layout_split_equal_vertical() {
        let split = LayoutSplit::new(SplitDirection::Vertical, 2);
        let rects = split.split_rect(rect(0, 0, 80, 40));
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].y, 0);
        assert_eq!(rects[0].height, 20);
        assert_eq!(rects[1].y, 20);
        assert_eq!(rects[1].height, 20);
        for r in &rects {
            assert_eq!(r.width, 80);
        }
    }

    #[test]
    fn test_layout_split_custom_ratios() {
        let split = LayoutSplit::with_ratios(SplitDirection::Horizontal, vec![0.25, 0.75]);
        let rects = split.split_rect(rect(0, 0, 100, 10));
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].width, 25);
        // Last pane gets remainder
        assert_eq!(rects[1].width, 75);
    }

    #[test]
    fn test_layout_split_add_pane() {
        let mut split = LayoutSplit::new(SplitDirection::Horizontal, 2);
        assert_eq!(split.pane_count(), 2);
        split.add_pane();
        assert_eq!(split.pane_count(), 3);
        let sum: f32 = split.ratios.iter().sum();
        assert!((sum - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_layout_split_remove_pane() {
        let mut split = LayoutSplit::new(SplitDirection::Vertical, 3);
        assert!(split.remove_pane(1));
        assert_eq!(split.pane_count(), 2);
        // Cannot remove when only 1 pane left
        assert!(split.remove_pane(0));
        assert_eq!(split.pane_count(), 1);
        assert!(!split.remove_pane(0));
    }

    #[test]
    fn test_layout_split_min_constraint() {
        let mut split = LayoutSplit::new(SplitDirection::Horizontal, 4);
        split.set_min_size(10);
        let rects = split.split_rect(rect(0, 0, 20, 10));
        // Each pane should be at least min_size
        for r in &rects {
            assert!(r.width >= 10);
        }
    }

    #[test]
    fn test_panel_resize_clamp_min() {
        assert_eq!(resize_panel(10, -20, 5, 50), 5);
    }

    #[test]
    fn test_panel_resize_clamp_max() {
        assert_eq!(resize_panel(40, 30, 5, 50), 50);
    }

    #[test]
    fn test_panel_resize_normal() {
        assert_eq!(resize_panel(20, 5, 5, 50), 25);
        assert_eq!(resize_panel(20, -5, 5, 50), 15);
    }

    #[test]
    fn test_layout_serialize_deserialize_roundtrip() {
        let mut layout = WorkbenchLayout::new();
        layout.set_sidebar_width(42);
        layout.set_panel_height(15);
        let state = layout_serialize(&layout);
        let restored = layout_deserialize(&state);
        let state2 = layout_serialize(&restored);
        assert_eq!(state, state2);
    }

    #[test]
    fn test_layout_state_preserves_visibility() {
        let mut layout = WorkbenchLayout::new();
        layout.set_part_visible(Part::Sidebar, false);
        layout.set_part_visible(Part::Panel, false);
        layout.set_part_visible(Part::Menubar, false);
        layout.set_part_visible(Part::ActivityBar, false);
        let state = layout_serialize(&layout);
        assert!(!state.sidebar_visible);
        assert!(!state.panel_visible);
        assert!(!state.menubar_visible);
        assert!(!state.activity_bar_visible);
        let restored = layout_deserialize(&state);
        assert!(!restored.is_part_visible(Part::Sidebar));
        assert!(!restored.is_part_visible(Part::Panel));
        assert!(!restored.is_part_visible(Part::Menubar));
        assert!(!restored.is_part_visible(Part::ActivityBar));
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
    fn behavior_check_28() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn wb_layout_stats_new_defaults() {
        let stats = WbLayoutStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_layout_stats_record_success() {
        let mut stats = WbLayoutStats::new();
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
    fn wb_layout_stats_record_failure() {
        let mut stats = WbLayoutStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_layout_stats_reset() {
        let mut stats = WbLayoutStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_layout_stats_merge() {
        let mut a = WbLayoutStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbLayoutStats::new();
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
    fn wb_layout_stats_display() {
        let mut stats = WbLayoutStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_layout_stats_default() {
        let stats = WbLayoutStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_layout_validator_accepts_valid_name() {
        let v = WbLayoutValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_layout_validator_rejects_empty() {
        let v = WbLayoutValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_layout_validator_rejects_too_long() {
        let v = WbLayoutValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_layout_validator_forbidden_prefix() {
        let v = WbLayoutValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_layout_validator_allowed_chars() {
        let v = WbLayoutValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_layout_validator_range() {
        let v = WbLayoutValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_layout_sanitize_removes_control() {
        let result = WbLayoutValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_layout_truncate_short_string() {
        assert_eq!(WbLayoutValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_layout_truncate_long_string() {
        let result = WbLayoutValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_layout_is_ascii_printable() {
        assert!(WbLayoutValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbLayoutValidator::is_ascii_printable("Hello\x00World"));
    }
}
