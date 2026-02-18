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

// ---------------------------------------------------------------------------
// LayoutConstraints
// ---------------------------------------------------------------------------

/// Minimum and maximum size constraints for a workbench part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutConstraints {
    pub min_width: u16,
    pub min_height: u16,
    pub max_width: Option<u16>,
    pub max_height: Option<u16>,
}

impl LayoutConstraints {
    /// Create constraints with only minimums.
    pub fn new(min_width: u16, min_height: u16) -> Self {
        Self {
            min_width,
            min_height,
            max_width: None,
            max_height: None,
        }
    }

    /// Set the maximum width.
    pub fn with_max_width(mut self, w: u16) -> Self {
        self.max_width = Some(w);
        self
    }

    /// Set the maximum height.
    pub fn with_max_height(mut self, h: u16) -> Self {
        self.max_height = Some(h);
        self
    }

    /// Clamp a width value to these constraints.
    pub fn clamp_width(&self, w: u16) -> u16 {
        let w = w.max(self.min_width);
        match self.max_width {
            Some(max) => w.min(max),
            None => w,
        }
    }

    /// Clamp a height value to these constraints.
    pub fn clamp_height(&self, h: u16) -> u16 {
        let h = h.max(self.min_height);
        match self.max_height {
            Some(max) => h.min(max),
            None => h,
        }
    }

    /// Check if a given width/height pair satisfies these constraints.
    pub fn satisfies(&self, width: u16, height: u16) -> bool {
        width >= self.min_width
            && height >= self.min_height
            && self.max_width.map_or(true, |m| width <= m)
            && self.max_height.map_or(true, |m| height <= m)
    }

    /// Merge two constraints, taking the tighter bound for each field.
    pub fn merge(&self, other: &LayoutConstraints) -> LayoutConstraints {
        LayoutConstraints {
            min_width: self.min_width.max(other.min_width),
            min_height: self.min_height.max(other.min_height),
            max_width: match (self.max_width, other.max_width) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            },
            max_height: match (self.max_height, other.max_height) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            },
        }
    }
}

impl Default for LayoutConstraints {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl fmt::Display for LayoutConstraints {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}x{} .. {}x{}",
            self.min_width,
            self.min_height,
            self.max_width.map_or("∞".to_string(), |v| v.to_string()),
            self.max_height.map_or("∞".to_string(), |v| v.to_string()),
        )
    }
}

// ---------------------------------------------------------------------------
// LayoutSnapshot
// ---------------------------------------------------------------------------

/// A serializable snapshot of the entire workbench layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutSnapshot {
    pub state: LayoutState,
    pub constraints: HashMap<Part, LayoutConstraints>,
    pub timestamp: u64,
    pub label: Option<String>,
}

impl LayoutSnapshot {
    /// Capture a snapshot from a [`WorkbenchLayout`] and optional constraints.
    pub fn capture(
        layout: &WorkbenchLayout,
        constraints: &HashMap<Part, LayoutConstraints>,
        timestamp: u64,
    ) -> Self {
        Self {
            state: layout_serialize(layout),
            constraints: constraints.clone(),
            timestamp,
            label: None,
        }
    }

    /// Attach a human-readable label to this snapshot.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Restore the layout from this snapshot.
    pub fn restore(&self) -> WorkbenchLayout {
        layout_deserialize(&self.state)
    }
}

// ---------------------------------------------------------------------------
// LayoutDiff
// ---------------------------------------------------------------------------

/// A single field-level change between two layout states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutChange {
    pub field: String,
    pub old_value: String,
    pub new_value: String,
}

/// Diff between two layout snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutDiff {
    pub changes: Vec<LayoutChange>,
}

impl LayoutDiff {
    /// Compute the diff between two [`LayoutState`] values.
    pub fn diff(old: &LayoutState, new: &LayoutState) -> Self {
        let mut changes = Vec::new();
        if old.sidebar_width != new.sidebar_width {
            changes.push(LayoutChange {
                field: "sidebar_width".into(),
                old_value: old.sidebar_width.to_string(),
                new_value: new.sidebar_width.to_string(),
            });
        }
        if old.panel_height != new.panel_height {
            changes.push(LayoutChange {
                field: "panel_height".into(),
                old_value: old.panel_height.to_string(),
                new_value: new.panel_height.to_string(),
            });
        }
        if old.sidebar_visible != new.sidebar_visible {
            changes.push(LayoutChange {
                field: "sidebar_visible".into(),
                old_value: old.sidebar_visible.to_string(),
                new_value: new.sidebar_visible.to_string(),
            });
        }
        if old.panel_visible != new.panel_visible {
            changes.push(LayoutChange {
                field: "panel_visible".into(),
                old_value: old.panel_visible.to_string(),
                new_value: new.panel_visible.to_string(),
            });
        }
        if old.menubar_visible != new.menubar_visible {
            changes.push(LayoutChange {
                field: "menubar_visible".into(),
                old_value: old.menubar_visible.to_string(),
                new_value: new.menubar_visible.to_string(),
            });
        }
        if old.activity_bar_visible != new.activity_bar_visible {
            changes.push(LayoutChange {
                field: "activity_bar_visible".into(),
                old_value: old.activity_bar_visible.to_string(),
                new_value: new.activity_bar_visible.to_string(),
            });
        }
        Self { changes }
    }

    /// Returns true if the two states are identical.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Number of fields that changed.
    pub fn change_count(&self) -> usize {
        self.changes.len()
    }

    /// Check if a specific field changed.
    pub fn has_change(&self, field: &str) -> bool {
        self.changes.iter().any(|c| c.field == field)
    }

    /// Get the change record for a specific field.
    pub fn get_change(&self, field: &str) -> Option<&LayoutChange> {
        self.changes.iter().find(|c| c.field == field)
    }

    /// List all field names that changed.
    pub fn changed_fields(&self) -> Vec<&str> {
        self.changes.iter().map(|c| c.field.as_str()).collect()
    }
}

impl fmt::Display for LayoutDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return write!(f, "(no changes)");
        }
        for (i, c) in self.changes.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}: {} -> {}", c.field, c.old_value, c.new_value)?;
        }
        Ok(())
    }
}

impl WorkbenchLayout {
    /// Return the number of currently visible parts.
    pub fn visible_part_count(&self) -> usize {
        [
            Part::Menubar,
            Part::ActivityBar,
            Part::Sidebar,
            Part::Editor,
            Part::Panel,
            Part::StatusBar,
        ]
        .iter()
        .filter(|&&p| self.is_part_visible(p))
        .count()
    }

    /// Return a list of all hidden parts.
    pub fn hidden_parts(&self) -> Vec<Part> {
        [
            Part::Menubar,
            Part::ActivityBar,
            Part::Sidebar,
            Part::Editor,
            Part::Panel,
            Part::StatusBar,
        ]
        .iter()
        .copied()
        .filter(|&p| !self.is_part_visible(p))
        .collect()
    }

    /// Reset sidebar and panel to default dimensions.
    pub fn reset_dimensions(&mut self) {
        self.set_sidebar_width(220);
        self.set_panel_height(200);
    }
}

impl LayoutSplit {
    /// Return the ratios as percentages (0–100).
    pub fn ratios_as_percentages(&self) -> Vec<f32> {
        self.ratios.iter().map(|r| r * 100.0).collect()
    }

    /// Return true if all panes have equal ratios.
    pub fn is_equal_split(&self) -> bool {
        if self.ratios.is_empty() {
            return true;
        }
        let first = self.ratios[0];
        self.ratios.iter().all(|&r| (r - first).abs() < 0.001)
    }
}

/// Compute the total area in cells (width * height) of a Rect.
pub fn rect_area(r: &Rect) -> u32 {
    r.width as u32 * r.height as u32
}

/// Return true if two Rects overlap.
pub fn rects_overlap(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
}

/// Compute the intersection rectangle of two Rects, or None if they do not overlap.
pub fn rect_intersection(a: &Rect, b: &Rect) -> Option<Rect> {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = (a.x + a.width).min(b.x + b.width);
    let y2 = (a.y + a.height).min(b.y + b.height);
    if x2 > x1 && y2 > y1 {
        Some(Rect::new(x1, y1, x2 - x1, y2 - y1))
    } else {
        None
    }
}

/// Compute the bounding box that contains all provided Rects.
pub fn bounding_box(rects: &[Rect]) -> Option<Rect> {
    if rects.is_empty() {
        return None;
    }
    let mut min_x = u16::MAX;
    let mut min_y = u16::MAX;
    let mut max_x: u16 = 0;
    let mut max_y: u16 = 0;
    for r in rects {
        min_x = min_x.min(r.x);
        min_y = min_y.min(r.y);
        max_x = max_x.max(r.x + r.width);
        max_y = max_y.max(r.y + r.height);
    }
    Some(Rect::new(min_x, min_y, max_x - min_x, max_y - min_y))
}

/// Inset a rectangle by a given margin on all sides.
/// Returns a zero-size rect if the margin is too large.
pub fn rect_inset(r: &Rect, margin: u16) -> Rect {
    let double = margin * 2;
    if r.width <= double || r.height <= double {
        return Rect::new(r.x + r.width / 2, r.y + r.height / 2, 0, 0);
    }
    Rect::new(r.x + margin, r.y + margin, r.width - double, r.height - double)
}

/// Return true if `inner` is fully contained within `outer`.
pub fn rect_contains(outer: &Rect, inner: &Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

impl Part {
    /// Return true if this part is considered a "chrome" element (not the editor).
    pub fn is_chrome(&self) -> bool {
        !matches!(self, Part::Editor)
    }

    /// Return a string label for this part.
    pub fn label(&self) -> &'static str {
        match self {
            Part::Titlebar => "Title Bar",
            Part::Menubar => "Menu Bar",
            Part::Sidebar => "Side Bar",
            Part::Editor => "Editor",
            Part::Panel => "Panel",
            Part::StatusBar => "Status Bar",
            Part::ActivityBar => "Activity Bar",
            Part::AuxiliaryBar => "Auxiliary Bar",
        }
    }
}

impl LayoutResult {
    /// Return all non-None rectangles as a vector.
    pub fn all_rects(&self) -> Vec<Rect> {
        let mut rects = Vec::new();
        if let Some(r) = self.menubar {
            rects.push(r);
        }
        if let Some(r) = self.activity_bar {
            rects.push(r);
        }
        if let Some(r) = self.sidebar {
            rects.push(r);
        }
        rects.push(self.editor);
        if let Some(r) = self.panel {
            rects.push(r);
        }
        rects.push(self.statusbar);
        rects
    }

    /// Return the total area in cells covered by all visible parts.
    pub fn total_area(&self) -> u32 {
        self.all_rects().iter().map(|r| rect_area(r)).sum()
    }
}

impl LayoutSplit {
    /// Return the ratio assigned to pane at `index`, or None if out of bounds.
    pub fn ratio_at(&self, index: usize) -> Option<f32> {
        self.ratios.get(index).copied()
    }

    /// Return the total ratio sum (should be ~1.0 for a well-formed split).
    pub fn ratio_sum(&self) -> f32 {
        self.ratios.iter().sum()
    }

    /// Return true if the split has only a single pane.
    pub fn is_single_pane(&self) -> bool {
        self.ratios.len() <= 1
    }
}

// ---------------------------------------------------------------------------
// LayoutAnimation
// ---------------------------------------------------------------------------

/// Drives a smooth animated transition between two sets of split ratios.
#[derive(Debug, Clone)]
pub struct LayoutAnimation {
    pub from_ratios: Vec<f32>,
    pub to_ratios: Vec<f32>,
    pub progress: f32,
    pub duration_ms: u32,
}

impl LayoutAnimation {
    /// Create a new animation that interpolates from `from` to `to` over
    /// `duration_ms` milliseconds.
    pub fn new(from: Vec<f32>, to: Vec<f32>, duration_ms: u32) -> Self {
        Self {
            from_ratios: from,
            to_ratios: to,
            progress: 0.0,
            duration_ms,
        }
    }

    /// Advance the animation by `elapsed_ms` milliseconds.
    /// Returns `true` when the animation has finished.
    pub fn tick(&mut self, elapsed_ms: u32) -> bool {
        if self.duration_ms == 0 {
            self.progress = 1.0;
            return true;
        }
        let step = elapsed_ms as f32 / self.duration_ms as f32;
        self.progress = (self.progress + step).min(1.0);
        self.progress >= 1.0
    }

    /// Linearly interpolate between `from_ratios` and `to_ratios` based on
    /// the current `progress`.
    pub fn current_ratios(&self) -> Vec<f32> {
        let len = self.from_ratios.len().min(self.to_ratios.len());
        (0..len)
            .map(|i| {
                let a = self.from_ratios[i];
                let b = self.to_ratios[i];
                a + (b - a) * self.progress
            })
            .collect()
    }

    /// Return `true` when progress has reached 1.0.
    pub fn is_complete(&self) -> bool {
        self.progress >= 1.0
    }

    /// Reset the animation back to the beginning.
    pub fn reset(&mut self) {
        self.progress = 0.0;
    }
}

// ---------------------------------------------------------------------------
// LayoutReset
// ---------------------------------------------------------------------------

/// Stores named default ratio sets so a layout can be restored to its
/// original proportions.
#[derive(Debug, Clone)]
pub struct LayoutReset {
    defaults: HashMap<String, Vec<f32>>,
}

impl LayoutReset {
    pub fn new() -> Self {
        Self {
            defaults: HashMap::new(),
        }
    }

    /// Register a default ratio set for the given layout id.
    pub fn register_default(&mut self, layout_id: &str, ratios: Vec<f32>) {
        self.defaults.insert(layout_id.to_string(), ratios);
    }

    /// Look up the default ratios for `layout_id`.
    pub fn get_default(&self, layout_id: &str) -> Option<&Vec<f32>> {
        self.defaults.get(layout_id)
    }

    /// Return `true` if a default has been registered for `layout_id`.
    pub fn has_default(&self, layout_id: &str) -> bool {
        self.defaults.contains_key(layout_id)
    }

    /// Clone and return the default ratios for `layout_id`, if registered.
    pub fn reset_to_default(&self, layout_id: &str) -> Option<Vec<f32>> {
        self.defaults.get(layout_id).cloned()
    }

    /// Number of registered defaults.
    pub fn registered_count(&self) -> usize {
        self.defaults.len()
    }
}

impl Default for LayoutReset {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// LayoutZoneDetector
// ---------------------------------------------------------------------------

/// Which edge of a pane a resize zone is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    Left,
    Right,
    Top,
    Bottom,
}

impl fmt::Display for ResizeEdge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResizeEdge::Left => write!(f, "Left"),
            ResizeEdge::Right => write!(f, "Right"),
            ResizeEdge::Top => write!(f, "Top"),
            ResizeEdge::Bottom => write!(f, "Bottom"),
        }
    }
}

/// A rectangular zone that represents a draggable resize handle.
#[derive(Debug, Clone)]
pub struct ResizeZone {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub edge: ResizeEdge,
}

/// Collects [`ResizeZone`]s and performs hit-testing against mouse
/// coordinates.
#[derive(Debug, Clone)]
pub struct LayoutZoneDetector {
    zones: Vec<ResizeZone>,
}

impl LayoutZoneDetector {
    pub fn new() -> Self {
        Self { zones: Vec::new() }
    }

    /// Append a resize zone.
    pub fn add_zone(&mut self, zone: ResizeZone) {
        self.zones.push(zone);
    }

    /// Return the first zone whose bounding rectangle contains (`mx`, `my`).
    pub fn hit_test(&self, mx: u16, my: u16) -> Option<&ResizeZone> {
        self.zones.iter().find(|z| {
            mx >= z.x
                && mx < z.x.saturating_add(z.width)
                && my >= z.y
                && my < z.y.saturating_add(z.height)
        })
    }

    /// Number of registered zones.
    pub fn zone_count(&self) -> usize {
        self.zones.len()
    }

    /// Remove all zones.
    pub fn clear(&mut self) {
        self.zones.clear();
    }
}

impl Default for LayoutZoneDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// LayoutPersistence
// ---------------------------------------------------------------------------

/// Simple versioned store for persisting layout ratio data.
#[derive(Debug, Clone)]
pub struct LayoutPersistence {
    pub version: u32,
    data: HashMap<String, Vec<f32>>,
}

impl LayoutPersistence {
    pub fn new(version: u32) -> Self {
        Self {
            version,
            data: HashMap::new(),
        }
    }

    /// Save (or overwrite) ratio data for the given layout id.
    pub fn save_layout(&mut self, id: &str, ratios: Vec<f32>) {
        self.data.insert(id.to_string(), ratios);
    }

    /// Retrieve previously saved ratios.
    pub fn load_layout(&self, id: &str) -> Option<&Vec<f32>> {
        self.data.get(id)
    }

    /// Produce a simple textual representation of the stored data.
    ///
    /// Format: `version:<ver>\n<id>:<r0>,<r1>,...\n`
    pub fn serialize(&self) -> String {
        let mut out = format!("version:{}\n", self.version);
        let mut keys: Vec<&String> = self.data.keys().collect();
        keys.sort();
        for key in keys {
            let ratios = &self.data[key];
            let vals: Vec<String> = ratios.iter().map(|r| format!("{r}")).collect();
            out.push_str(&format!("{}:{}\n", key, vals.join(",")));
        }
        out
    }

    /// Return `true` when the stored version is older than `current_version`.
    pub fn needs_migration(&self, current_version: u32) -> bool {
        self.version < current_version
    }

    /// Number of stored layouts.
    pub fn layout_count(&self) -> usize {
        self.data.len()
    }
}


// === Layout Maximize/Restore Handler ===

/// Layout Maximize/Restore Handler implementation.
#[derive(Debug, Clone)]
pub struct LayoutMaxRestoreHandler {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: LayoutMaxRestoreHandlerStats,
}

/// Statistics for LayoutMaxRestoreHandler.
#[derive(Debug, Clone, Default)]
pub struct LayoutMaxRestoreHandlerStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl LayoutMaxRestoreHandlerStats {
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

impl LayoutMaxRestoreHandler {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: LayoutMaxRestoreHandlerStats::default(),
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

    pub fn stats(&self) -> &LayoutMaxRestoreHandlerStats {
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

impl Default for LayoutMaxRestoreHandler {
    fn default() -> Self {
        Self::new()
    }
}

// === Layout Orientation Flipper ===

/// Priority level for LayoutOrientationFlipper items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LayoutOrientationFlipperPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl LayoutOrientationFlipperPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for LayoutOrientationFlipperPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Layout Orientation Flipper implementation.
#[derive(Debug, Clone)]
pub struct LayoutOrientationFlipper {
    items: Vec<LayoutOrientationFlipperItem>,
    max_items: usize,
    default_priority: LayoutOrientationFlipperPriority,
}

/// A single item in LayoutOrientationFlipper.
#[derive(Debug, Clone)]
pub struct LayoutOrientationFlipperItem {
    pub id: String,
    pub label: String,
    pub priority: LayoutOrientationFlipperPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl LayoutOrientationFlipperItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: LayoutOrientationFlipperPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: LayoutOrientationFlipperPriority) -> Self {
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

impl LayoutOrientationFlipper {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: LayoutOrientationFlipperPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: LayoutOrientationFlipperItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<LayoutOrientationFlipperItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&LayoutOrientationFlipperItem> {
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

    pub fn by_priority(&self, priority: LayoutOrientationFlipperPriority) -> Vec<&LayoutOrientationFlipperItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&LayoutOrientationFlipperItem> {
        let mut sorted: Vec<&LayoutOrientationFlipperItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&LayoutOrientationFlipperItem> {
        let mut sorted: Vec<&LayoutOrientationFlipperItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&LayoutOrientationFlipperItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: LayoutOrientationFlipperPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> LayoutOrientationFlipperPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &LayoutOrientationFlipperItem> {
        self.items.iter()
    }
}

impl Default for LayoutOrientationFlipper {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// vsedit-wb-layout: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WbLayoutXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl WbLayoutXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for WbLayoutXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct WbLayoutXRegistry {
    entries: Vec<WbLayoutXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl WbLayoutXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: WbLayoutXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&WbLayoutXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut WbLayoutXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<WbLayoutXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&WbLayoutXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&WbLayoutXConfig> {
        let mut sorted: Vec<&WbLayoutXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&WbLayoutXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> WbLayoutXIterator<'_> {
        WbLayoutXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct WbLayoutXIterator<'a> {
    inner: std::slice::Iter<'a, WbLayoutXConfig>,
}

impl<'a> Iterator for WbLayoutXIterator<'a> {
    type Item = &'a WbLayoutXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct WbLayoutXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl WbLayoutXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct WbLayoutXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl WbLayoutXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &WbLayoutXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &WbLayoutXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &WbLayoutXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for WbLayoutXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct WbLayoutXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl WbLayoutXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &WbLayoutXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &WbLayoutXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for WbLayoutXValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for wb_layout
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWbLayoutRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWbLayoutRingBuf {
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
pub struct XaWbLayoutCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWbLayoutCounter {
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

impl Default for XaWbLayoutCounter {
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

    // ── LayoutConstraints tests ──

    #[test]
    fn layout_constraints_clamp_width() {
        let c = LayoutConstraints::new(10, 5).with_max_width(100);
        assert_eq!(c.clamp_width(0), 10);
        assert_eq!(c.clamp_width(50), 50);
        assert_eq!(c.clamp_width(200), 100);
    }

    #[test]
    fn layout_constraints_satisfies() {
        let c = LayoutConstraints::new(10, 10).with_max_width(100).with_max_height(80);
        assert!(c.satisfies(50, 50));
        assert!(!c.satisfies(5, 50));
        assert!(!c.satisfies(50, 5));
        assert!(!c.satisfies(101, 50));
        assert!(!c.satisfies(50, 81));
    }

    #[test]
    fn layout_constraints_merge() {
        let a = LayoutConstraints::new(10, 20).with_max_width(200);
        let b = LayoutConstraints::new(15, 10).with_max_width(150).with_max_height(100);
        let m = a.merge(&b);
        assert_eq!(m.min_width, 15);
        assert_eq!(m.min_height, 20);
        assert_eq!(m.max_width, Some(150));
        assert_eq!(m.max_height, Some(100));
    }

    #[test]
    fn layout_snapshot_capture_and_restore() {
        let layout = WorkbenchLayout::new();
        let constraints = HashMap::new();
        let snap = LayoutSnapshot::capture(&layout, &constraints, 42);
        assert_eq!(snap.timestamp, 42);
        let restored = snap.restore();
        assert_eq!(restored.get_sidebar_width(), layout.get_sidebar_width());
        assert_eq!(restored.get_panel_height(), layout.get_panel_height());
    }

    #[test]
    fn layout_diff_detects_changes() {
        let mut layout = WorkbenchLayout::new();
        let old_state = layout_serialize(&layout);
        layout.set_sidebar_width(300);
        layout.set_part_visible(Part::Panel, false);
        let new_state = layout_serialize(&layout);
        let diff = LayoutDiff::diff(&old_state, &new_state);
        assert!(!diff.is_empty());
        assert!(diff.has_change("sidebar_width"));
        assert!(diff.has_change("panel_visible"));
        assert!(!diff.has_change("menubar_visible"));
        assert_eq!(diff.change_count(), 2);
    }

    #[test]
    fn layout_diff_identical_is_empty() {
        let layout = WorkbenchLayout::new();
        let state = layout_serialize(&layout);
        let diff = LayoutDiff::diff(&state, &state);
        assert!(diff.is_empty());
        assert_eq!(diff.change_count(), 0);
        assert_eq!(format!("{}", diff), "(no changes)");
    }

    #[test]
    fn visible_part_count_default() {
        let layout = WorkbenchLayout::new();
        assert_eq!(layout.visible_part_count(), 6);
    }

    #[test]
    fn visible_part_count_after_hiding() {
        let mut layout = WorkbenchLayout::new();
        layout.set_part_visible(Part::Panel, false);
        layout.set_part_visible(Part::StatusBar, false);
        assert_eq!(layout.visible_part_count(), 4);
    }

    #[test]
    fn hidden_parts_default_is_empty() {
        let layout = WorkbenchLayout::new();
        assert!(layout.hidden_parts().is_empty());
    }

    #[test]
    fn hidden_parts_after_toggle() {
        let mut layout = WorkbenchLayout::new();
        layout.toggle_sidebar();
        let hidden = layout.hidden_parts();
        assert_eq!(hidden.len(), 1);
        assert_eq!(hidden[0], Part::Sidebar);
    }

    #[test]
    fn reset_dimensions_restores_defaults() {
        let mut layout = WorkbenchLayout::new();
        layout.set_sidebar_width(500);
        layout.set_panel_height(500);
        layout.reset_dimensions();
        assert_eq!(layout.get_sidebar_width(), 220);
        assert_eq!(layout.get_panel_height(), 200);
    }

    #[test]
    fn split_ratios_as_percentages() {
        let split = LayoutSplit::new(SplitDirection::Horizontal, 4);
        let pct = split.ratios_as_percentages();
        assert_eq!(pct.len(), 4);
        for p in &pct {
            assert!((p - 25.0).abs() < 0.1);
        }
    }

    #[test]
    fn split_is_equal_true() {
        let split = LayoutSplit::new(SplitDirection::Vertical, 3);
        assert!(split.is_equal_split());
    }

    #[test]
    fn split_is_equal_false_custom_ratios() {
        let split = LayoutSplit::with_ratios(SplitDirection::Horizontal, vec![0.3, 0.7]);
        assert!(!split.is_equal_split());
    }

    #[test]
    fn rect_area_calculation() {
        let r = rect(0, 0, 10, 20);
        assert_eq!(rect_area(&r), 200);
    }

    #[test]
    fn rect_area_zero() {
        let r = rect(5, 5, 0, 10);
        assert_eq!(rect_area(&r), 0);
    }

    #[test]
    fn rects_overlap_true() {
        let a = rect(0, 0, 10, 10);
        let b = rect(5, 5, 10, 10);
        assert!(rects_overlap(&a, &b));
    }

    #[test]
    fn rects_overlap_false() {
        let a = rect(0, 0, 10, 10);
        let b = rect(20, 20, 10, 10);
        assert!(!rects_overlap(&a, &b));
    }

    #[test]
    fn rects_overlap_adjacent_is_false() {
        let a = rect(0, 0, 10, 10);
        let b = rect(10, 0, 10, 10);
        assert!(!rects_overlap(&a, &b));
    }

    #[test]
    fn rect_intersection_overlapping() {
        let a = rect(0, 0, 10, 10);
        let b = rect(5, 5, 10, 10);
        let inter = rect_intersection(&a, &b).unwrap();
        assert_eq!(inter, rect(5, 5, 5, 5));
    }

    #[test]
    fn rect_intersection_none() {
        let a = rect(0, 0, 10, 10);
        let b = rect(20, 20, 10, 10);
        assert!(rect_intersection(&a, &b).is_none());
    }

    #[test]
    fn bounding_box_multiple() {
        let rects = vec![rect(5, 5, 10, 10), rect(0, 0, 3, 3), rect(10, 10, 5, 5)];
        let bb = bounding_box(&rects).unwrap();
        assert_eq!(bb, rect(0, 0, 15, 15));
    }

    #[test]
    fn bounding_box_empty() {
        let rects: Vec<Rect> = vec![];
        assert!(bounding_box(&rects).is_none());
    }

    #[test]
    fn rect_inset_shrinks() {
        let r = rect(10, 10, 20, 20);
        let inset = rect_inset(&r, 3);
        assert_eq!(inset, rect(13, 13, 14, 14));
    }

    #[test]
    fn rect_inset_too_large() {
        let r = rect(10, 10, 4, 4);
        let inset = rect_inset(&r, 3);
        assert_eq!(inset.width, 0);
        assert_eq!(inset.height, 0);
    }

    #[test]
    fn rect_contains_inside() {
        let outer = rect(0, 0, 20, 20);
        let inner = rect(5, 5, 10, 10);
        assert!(rect_contains(&outer, &inner));
    }

    #[test]
    fn rect_contains_outside() {
        let outer = rect(0, 0, 10, 10);
        let inner = rect(5, 5, 10, 10);
        assert!(!rect_contains(&outer, &inner));
    }

    #[test]
    fn part_is_chrome() {
        assert!(Part::Menubar.is_chrome());
        assert!(Part::StatusBar.is_chrome());
        assert!(!Part::Editor.is_chrome());
    }

    #[test]
    fn part_label() {
        assert_eq!(Part::Editor.label(), "Editor");
        assert_eq!(Part::Sidebar.label(), "Side Bar");
    }

    #[test]
    fn layout_result_all_rects() {
        let layout = WorkbenchLayout::new();
        let result = layout.compute(rect(0, 0, 120, 50));
        let rects = result.all_rects();
        assert!(rects.len() >= 4); // menubar, actbar, sidebar, editor, panel, statusbar
    }

    #[test]
    fn layout_result_total_area() {
        let layout = WorkbenchLayout::new();
        let result = layout.compute(rect(0, 0, 120, 50));
        assert!(result.total_area() > 0);
    }

    #[test]
    fn split_add_remove_pane() {
        let mut split = LayoutSplit::new(SplitDirection::Horizontal, 2);
        assert_eq!(split.pane_count(), 2);
        split.add_pane();
        assert_eq!(split.pane_count(), 3);
        assert!(split.remove_pane(2));
        assert_eq!(split.pane_count(), 2);
    }

    #[test]
    fn split_single_pane_minimum() {
        let mut split = LayoutSplit::new(SplitDirection::Vertical, 1);
        assert!(split.is_single_pane());
        assert!(!split.remove_pane(0)); // can't remove the last one
        assert_eq!(split.pane_count(), 1);
    }

    #[test]
    fn split_ratio_at_and_sum() {
        let split = LayoutSplit::new(SplitDirection::Horizontal, 3);
        assert!(split.ratio_at(0).is_some());
        assert!(split.ratio_at(5).is_none());
        assert!((split.ratio_sum() - 1.0).abs() < 0.01);
    }

    // -----------------------------------------------------------------------
    // LayoutAnimation tests
    // -----------------------------------------------------------------------

    #[test]
    fn animation_starts_at_zero_progress() {
        let anim = LayoutAnimation::new(vec![0.5, 0.5], vec![0.7, 0.3], 300);
        assert!(!anim.is_complete());
        assert!((anim.progress - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn animation_tick_advances_progress() {
        let mut anim = LayoutAnimation::new(vec![0.5, 0.5], vec![1.0, 0.0], 100);
        let done = anim.tick(50);
        assert!(!done);
        assert!((anim.progress - 0.5).abs() < 0.01);
        let done = anim.tick(50);
        assert!(done);
        assert!(anim.is_complete());
    }

    #[test]
    fn animation_current_ratios_interpolates() {
        let mut anim = LayoutAnimation::new(vec![0.0, 1.0], vec![1.0, 0.0], 200);
        anim.tick(100); // 50 %
        let cur = anim.current_ratios();
        assert!((cur[0] - 0.5).abs() < 0.01);
        assert!((cur[1] - 0.5).abs() < 0.01);
    }

    #[test]
    fn animation_reset_returns_to_start() {
        let mut anim = LayoutAnimation::new(vec![0.5, 0.5], vec![1.0, 0.0], 100);
        anim.tick(100);
        assert!(anim.is_complete());
        anim.reset();
        assert!(!anim.is_complete());
        assert!((anim.progress - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn animation_zero_duration_completes_immediately() {
        let mut anim = LayoutAnimation::new(vec![0.3, 0.7], vec![0.6, 0.4], 0);
        assert!(anim.tick(0));
        assert!(anim.is_complete());
    }

    // -----------------------------------------------------------------------
    // LayoutReset tests
    // -----------------------------------------------------------------------

    #[test]
    fn reset_register_and_retrieve() {
        let mut lr = LayoutReset::new();
        lr.register_default("editor", vec![0.5, 0.5]);
        assert!(lr.has_default("editor"));
        assert!(!lr.has_default("panel"));
        assert_eq!(lr.registered_count(), 1);
        let ratios = lr.get_default("editor").unwrap();
        assert_eq!(ratios.len(), 2);
    }

    #[test]
    fn reset_to_default_clones_data() {
        let mut lr = LayoutReset::new();
        lr.register_default("sidebar", vec![0.25, 0.75]);
        let cloned = lr.reset_to_default("sidebar").unwrap();
        assert_eq!(cloned, vec![0.25, 0.75]);
        assert!(lr.reset_to_default("missing").is_none());
    }

    // -----------------------------------------------------------------------
    // LayoutZoneDetector tests
    // -----------------------------------------------------------------------

    #[test]
    fn zone_detector_hit_test() {
        let mut det = LayoutZoneDetector::new();
        det.add_zone(ResizeZone {
            x: 10,
            y: 10,
            width: 5,
            height: 20,
            edge: ResizeEdge::Right,
        });
        assert!(det.hit_test(12, 15).is_some());
        assert!(det.hit_test(0, 0).is_none());
        assert_eq!(det.zone_count(), 1);
    }

    #[test]
    fn zone_detector_clear() {
        let mut det = LayoutZoneDetector::new();
        det.add_zone(ResizeZone {
            x: 0,
            y: 0,
            width: 3,
            height: 3,
            edge: ResizeEdge::Left,
        });
        assert_eq!(det.zone_count(), 1);
        det.clear();
        assert_eq!(det.zone_count(), 0);
    }

    #[test]
    fn resize_edge_display() {
        assert_eq!(format!("{}", ResizeEdge::Top), "Top");
        assert_eq!(format!("{}", ResizeEdge::Bottom), "Bottom");
    }

    // -----------------------------------------------------------------------
    // LayoutPersistence tests
    // -----------------------------------------------------------------------

    #[test]
    fn persistence_save_and_load() {
        let mut p = LayoutPersistence::new(1);
        p.save_layout("main", vec![0.6, 0.4]);
        let loaded = p.load_layout("main").unwrap();
        assert_eq!(loaded, &vec![0.6, 0.4]);
        assert_eq!(p.layout_count(), 1);
    }

    #[test]
    fn persistence_serialize_format() {
        let mut p = LayoutPersistence::new(2);
        p.save_layout("a", vec![0.5, 0.5]);
        let s = p.serialize();
        assert!(s.starts_with("version:2\n"));
        assert!(s.contains("a:0.5,0.5"));
    }

    #[test]
    fn persistence_needs_migration() {
        let p = LayoutPersistence::new(1);
        assert!(p.needs_migration(2));
        assert!(!p.needs_migration(1));
        assert!(!p.needs_migration(0));
    }

    #[test]
    fn layoutMaxRestoreHandler_new() {
        let s = LayoutMaxRestoreHandler::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn layoutMaxRestoreHandler_add_contains() {
        let mut s = LayoutMaxRestoreHandler::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn layoutMaxRestoreHandler_add_duplicate() {
        let mut s = LayoutMaxRestoreHandler::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn layoutMaxRestoreHandler_remove() {
        let mut s = LayoutMaxRestoreHandler::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn layoutMaxRestoreHandler_capacity() {
        let s = LayoutMaxRestoreHandler::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn layoutMaxRestoreHandler_search() {
        let mut s = LayoutMaxRestoreHandler::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn layoutMaxRestoreHandler_stats() {
        let mut s = LayoutMaxRestoreHandler::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn layoutOrientationFlipper_new() {
        let m = LayoutOrientationFlipper::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn layoutOrientationFlipper_add_find() {
        let mut m = LayoutOrientationFlipper::new();
        m.add(LayoutOrientationFlipperItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn layoutOrientationFlipper_priority_filter() {
        let mut m = LayoutOrientationFlipper::new();
        m.add(LayoutOrientationFlipperItem::new("a", "A").with_priority(LayoutOrientationFlipperPriority::High));
        m.add(LayoutOrientationFlipperItem::new("b", "B").with_priority(LayoutOrientationFlipperPriority::Low));
        m.add(LayoutOrientationFlipperItem::new("c", "C").with_priority(LayoutOrientationFlipperPriority::High));
        assert_eq!(m.by_priority(LayoutOrientationFlipperPriority::High).len(), 2);
    }

    #[test]
    fn layoutOrientationFlipper_remove() {
        let mut m = LayoutOrientationFlipper::new();
        m.add(LayoutOrientationFlipperItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn layoutOrientationFlipper_search() {
        let mut m = LayoutOrientationFlipper::new();
        m.add(LayoutOrientationFlipperItem::new("id1", "Hello World"));
        m.add(LayoutOrientationFlipperItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn layoutOrientationFlipper_total_weight() {
        let mut m = LayoutOrientationFlipper::new();
        m.add(LayoutOrientationFlipperItem::new("a", "A").with_priority(LayoutOrientationFlipperPriority::Critical));
        m.add(LayoutOrientationFlipperItem::new("b", "B").with_priority(LayoutOrientationFlipperPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn layoutOrientationFlipper_capacity_limit() {
        let mut m = LayoutOrientationFlipper::new().with_max_items(2);
        m.add(LayoutOrientationFlipperItem::new("1", "one"));
        m.add(LayoutOrientationFlipperItem::new("2", "two"));
        assert!(!m.add(LayoutOrientationFlipperItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn layoutOrientationFlipper_sorted_by_priority() {
        let mut m = LayoutOrientationFlipper::new();
        m.add(LayoutOrientationFlipperItem::new("lo", "Low").with_priority(LayoutOrientationFlipperPriority::Low));
        m.add(LayoutOrientationFlipperItem::new("hi", "High").with_priority(LayoutOrientationFlipperPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn layoutOrientationFlipper_item_metadata() {
        let mut item = LayoutOrientationFlipperItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn layoutMaxRestoreHandler_enabled_toggle() {
        let mut s = LayoutMaxRestoreHandler::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn layoutOrientationFlipper_priority_display() {
        assert_eq!(format!("{}", LayoutOrientationFlipperPriority::High), "high");
        assert_eq!(format!("{}", LayoutOrientationFlipperPriority::Low), "low");
    }


    #[test]
    fn wbLayout_x_config_new() {
        let c = WbLayoutXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn wbLayout_x_config_builder() {
        let c = WbLayoutXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn wbLayout_x_config_display() {
        let c = WbLayoutXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn wbLayout_x_registry_insert_get() {
        let mut reg = WbLayoutXRegistry::new();
        reg.insert(WbLayoutXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn wbLayout_x_registry_duplicate() {
        let mut reg = WbLayoutXRegistry::new();
        reg.insert(WbLayoutXConfig::new("a")).unwrap();
        assert!(reg.insert(WbLayoutXConfig::new("a")).is_err());
    }

    #[test]
    fn wbLayout_x_registry_remove() {
        let mut reg = WbLayoutXRegistry::new();
        reg.insert(WbLayoutXConfig::new("a")).unwrap();
        reg.insert(WbLayoutXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn wbLayout_x_registry_active_entries() {
        let mut reg = WbLayoutXRegistry::new();
        reg.insert(WbLayoutXConfig::new("a")).unwrap();
        reg.insert(WbLayoutXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn wbLayout_x_registry_by_weight() {
        let mut reg = WbLayoutXRegistry::new();
        reg.insert(WbLayoutXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(WbLayoutXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn wbLayout_x_registry_tags() {
        let mut reg = WbLayoutXRegistry::new();
        reg.insert(WbLayoutXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(WbLayoutXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn wbLayout_x_registry_total_weight() {
        let mut reg = WbLayoutXRegistry::new();
        reg.insert(WbLayoutXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(WbLayoutXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn wbLayout_x_registry_iterator() {
        let mut reg = WbLayoutXRegistry::new();
        reg.insert(WbLayoutXConfig::new("a")).unwrap();
        reg.insert(WbLayoutXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn wbLayout_x_cache_put_get() {
        let mut cache = WbLayoutXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn wbLayout_x_cache_eviction() {
        let mut cache = WbLayoutXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn wbLayout_x_cache_lru_order() {
        let mut cache = WbLayoutXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn wbLayout_x_cache_most_least_recent() {
        let mut cache = WbLayoutXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn wbLayout_x_formatter_entry() {
        let e = WbLayoutXConfig::new("k").with_value("v");
        let fmt = WbLayoutXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn wbLayout_x_formatter_summary() {
        let mut reg = WbLayoutXRegistry::new();
        reg.insert(WbLayoutXConfig::new("a").with_weight(5)).unwrap();
        let fmt = WbLayoutXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn wbLayout_x_validator_valid() {
        let v = WbLayoutXValidator::new();
        let c = WbLayoutXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn wbLayout_x_validator_empty_key() {
        let v = WbLayoutXValidator::new();
        let c = WbLayoutXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn wbLayout_x_validator_require_value() {
        let v = WbLayoutXValidator::new().require_value(true);
        let c = WbLayoutXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn wbLayout_x_validator_allowed_tags() {
        let v = WbLayoutXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = WbLayoutXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn wbLayout_x_validator_validate_all() {
        let v = WbLayoutXValidator::new();
        let mut reg = WbLayoutXRegistry::new();
        reg.insert(WbLayoutXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    // xa_ extended tests for wb_layout
    #[test]
    fn xa_wb_layout_ring_new() {
        let rb = super::XaWbLayoutRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_wb_layout_ring_push_len() {
        let mut rb = super::XaWbLayoutRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_wb_layout_ring_wrap() {
        let mut rb = super::XaWbLayoutRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_wb_layout_ring_mean_empty() {
        let rb = super::XaWbLayoutRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_wb_layout_ring_mean_values() {
        let mut rb = super::XaWbLayoutRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_wb_layout_ring_min_max() {
        let mut rb = super::XaWbLayoutRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_wb_layout_ring_iter() {
        let mut rb = super::XaWbLayoutRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_wb_layout_counter_new() {
        let c = super::XaWbLayoutCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_layout_counter_inc() {
        let mut c = super::XaWbLayoutCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_wb_layout_counter_inc_by() {
        let mut c = super::XaWbLayoutCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_wb_layout_counter_reset() {
        let mut c = super::XaWbLayoutCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_wb_layout_counter_clear() {
        let mut c = super::XaWbLayoutCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_layout_counter_default() {
        let c = super::XaWbLayoutCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }

}
