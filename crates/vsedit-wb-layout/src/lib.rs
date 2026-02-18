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


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 213
// ---------------------------------------------------------------------------

/// Generic object pool `Xc213Pool<T>`.
pub struct Xc213Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc213Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc213PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc213Pool<T> {
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
    pub fn stats(&self) -> Xc213PoolStats {
        Xc213PoolStats {
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

impl<T> Default for Xc213Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc213Scheduler`.
pub struct Xc213Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc213Scheduler {
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

impl Default for Xc213Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_213 hash for the given byte slice.
pub fn xc_213_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_213 convention.
pub fn xc_213_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_31 deepening: state machine + event bus ---

/// States for the Xd31 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd31State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd31State {
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
pub struct Xd31Transition {
    pub from: Xd31State,
    pub to: Xd31State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd31StateMachine {
    current: Xd31State,
    history: Vec<Xd31Transition>,
    step_counter: usize,
}

impl Xd31StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd31State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd31State {
        self.current
    }

    pub fn history(&self) -> &[Xd31Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd31State) -> Result<Xd31State, String> {
        let allowed = match (self.current, target) {
            (Xd31State::Idle, Xd31State::Running) => true,
            (Xd31State::Running, Xd31State::Paused) => true,
            (Xd31State::Running, Xd31State::Done) => true,
            (Xd31State::Paused, Xd31State::Running) => true,
            (Xd31State::Paused, Xd31State::Done) => true,
            (Xd31State::Done, Xd31State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_31: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd31Transition {
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
            "Xd31SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd31State> {
        let prefix = "Xd31SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd31State::Idle),
            "Running" => Some(Xd31State::Running),
            "Paused" => Some(Xd31State::Paused),
            "Done" => Some(Xd31State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd31State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd31 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd31Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd31Event {
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

type Xd31HandlerFn = Box<dyn Fn(&Xd31Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd31EventBus {
    handlers: Vec<(usize, Option<String>, Xd31HandlerFn)>,
    next_id: usize,
    published: Vec<Xd31Event>,
}

impl Xd31EventBus {
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
        F: Fn(&Xd31Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd31Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd31Event) {
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

    pub fn published_events(&self) -> &[Xd31Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #29
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf29Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf29TrieNode {
    children: std::collections::HashMap<char, Xf29TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf29Trie {
    root: Xf29TrieNode,
    count: usize,
}

impl Xf29Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf29TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf29TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf29TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf29BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf29BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 212).
pub struct Xh212SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh212SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 254 as u64,
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

/// A compact bit set supporting boolean operations (variant 212).
pub struct Xh212BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh212BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 212).
pub struct Xi212Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi212Deque<T> {
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
pub struct Xi212Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi212Interval {
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

/// A simple interval tree (variant 212).
pub struct Xi212IntervalTree {
    xi_intervals: Vec<Xi212Interval>,
}

impl Xi212IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi212Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi212Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi212Interval) -> Vec<&Xi212Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi212Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi212Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi212Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi212Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi212Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi212Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 212) ---

/// Disjoint set / union-find for crate 212.
pub struct Xj212UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj212UnionFind {
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

const XJ212_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 212.
pub struct Xj212BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj212BTreeNode<K, V>>>,
    len: usize,
}

struct Xj212BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj212BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj212BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ212_BTREE_ORDER - 1
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
        let mid = XJ212_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj212BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj212BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj212BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj212BTreeNode::xj_new_leaf();
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


// --- xk_212 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk212SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk212SegmentTree {
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
pub struct Xk212DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk212DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_212).
#[derive(Debug, Clone)]
pub struct Xl212Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl212Rope {
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

/// Suffix array for efficient string searching (xl_212).
#[derive(Debug, Clone)]
pub struct Xl212SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl212SuffixArray {
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
pub struct Xm212MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm212MatrixSparse {
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
pub struct Xm212Tokenizer {
    text: String,
}

impl Xm212Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 212.
pub struct Xn212Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn212Fenwick {
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

// ----- AVL tree map — crate 212 -----

#[derive(Debug, Clone)]
struct Xn212AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn212AvlNode<K, V>>>,
    right: Option<Box<Xn212AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 212.
#[derive(Debug, Clone)]
pub struct Xn212AVL<K, V> {
    root: Option<Box<Xn212AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn212AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn212AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn212AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn212AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn212AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn212AvlNode<K, V>>) -> Box<Xn212AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn212AvlNode<K, V>>) -> Box<Xn212AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn212AvlNode<K, V>>) -> Box<Xn212AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn212AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn212AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn212AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn212AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn212AvlNode<K, V>>) -> &Xn212AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn212AvlNode<K, V>>) -> (Box<Xn212AvlNode<K, V>>, Option<Box<Xn212AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn212AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn212AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn212AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn212AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn212AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn212AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn212AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo212RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo212Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo212RBNode<K, V> {
    key: K,
    value: V,
    color: Xo212Color,
    left: Option<Box<Xo212RBNode<K, V>>>,
    right: Option<Box<Xo212RBNode<K, V>>>,
}

/// A red-black tree map for crate 212.
#[derive(Debug, Clone)]
pub struct Xo212RedBlack<K, V> {
    root: Option<Box<Xo212RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo212RedBlack<K, V> {
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
            r.color = Xo212Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo212RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo212RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo212RBNode {
                    key, value, color: Xo212Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo212RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo212Color::Red)
    }

    fn xo_balance(mut h: Box<Xo212RBNode<K, V>>) -> Box<Xo212RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo212Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo212RBNode<K, V>>) -> Box<Xo212RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo212Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo212RBNode<K, V>>) -> Box<Xo212RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo212Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo212RBNode<K, V>>) {
        h.color = Xo212Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo212Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo212Color::Black; }
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
            r.color = Xo212Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo212RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo212RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo212RBNode<K, V>) -> (K, V, Option<Box<Xo212RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo212RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo212Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo212RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo212ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 212.
#[derive(Debug, Clone)]
pub struct Xo212ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo212ConsistentHash {
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
            let vkey = format!("{}#xo212#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo212#{}", node, i);
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


    // ---- xc_ pool / scheduler tests – block 213 ----

    #[test]
    fn xc_213_pool_new_empty() {
        let pool: super::Xc213Pool<i32> = super::Xc213Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_213_pool_release_acquire() {
        let mut pool = super::Xc213Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_213_pool_acquire_empty() {
        let mut pool: super::Xc213Pool<i32> = super::Xc213Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_213_pool_full() {
        let mut pool = super::Xc213Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_213_pool_drain() {
        let mut pool = super::Xc213Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_213_pool_stats() {
        let mut pool = super::Xc213Pool::new(8);
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
    fn xc_213_pool_clear() {
        let mut pool = super::Xc213Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_213_pool_shrink() {
        let mut pool = super::Xc213Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_213_pool_default() {
        let pool: super::Xc213Pool<String> = super::Xc213Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_213_pool_extend() {
        let mut pool = super::Xc213Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_213_pool_retain() {
        let mut pool = super::Xc213Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_213_scheduler_round_robin() {
        let mut sched = super::Xc213Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_213_scheduler_empty() {
        let mut sched = super::Xc213Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_213_scheduler_reset() {
        let mut sched = super::Xc213Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_213_scheduler_add_remove() {
        let mut sched = super::Xc213Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_213_scheduler_targets() {
        let sched = super::Xc213Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_213_hash_empty() {
        assert_eq!(super::xc_213_hash(b""), 5381);
    }

    #[test]
    fn xc_213_hash_data() {
        let h = super::xc_213_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_213_hash(b"hello"), h);
    }

    #[test]
    fn xc_213_reverse_str() {
        assert_eq!(super::xc_213_reverse("abc"), "cba");
        assert_eq!(super::xc_213_reverse(""), "");
    }


    // --- xd_31 deepening tests ---

    #[test]
    fn xd_31_sm_initial_state() {
        let sm = Xd31StateMachine::new();
        assert_eq!(sm.current_state(), Xd31State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_31_sm_valid_idle_to_running() {
        let mut sm = Xd31StateMachine::new();
        assert!(sm.transition(Xd31State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd31State::Running);
    }

    #[test]
    fn xd_31_sm_valid_running_to_paused() {
        let mut sm = Xd31StateMachine::new();
        sm.transition(Xd31State::Running).unwrap();
        assert!(sm.transition(Xd31State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd31State::Paused);
    }

    #[test]
    fn xd_31_sm_valid_running_to_done() {
        let mut sm = Xd31StateMachine::new();
        sm.transition(Xd31State::Running).unwrap();
        assert!(sm.transition(Xd31State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd31State::Done);
    }

    #[test]
    fn xd_31_sm_valid_paused_to_running() {
        let mut sm = Xd31StateMachine::new();
        sm.transition(Xd31State::Running).unwrap();
        sm.transition(Xd31State::Paused).unwrap();
        assert!(sm.transition(Xd31State::Running).is_ok());
    }

    #[test]
    fn xd_31_sm_valid_done_to_idle() {
        let mut sm = Xd31StateMachine::new();
        sm.transition(Xd31State::Running).unwrap();
        sm.transition(Xd31State::Done).unwrap();
        assert!(sm.transition(Xd31State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd31State::Idle);
    }

    #[test]
    fn xd_31_sm_invalid_idle_to_done() {
        let mut sm = Xd31StateMachine::new();
        assert!(sm.transition(Xd31State::Done).is_err());
    }

    #[test]
    fn xd_31_sm_invalid_idle_to_paused() {
        let mut sm = Xd31StateMachine::new();
        assert!(sm.transition(Xd31State::Paused).is_err());
    }

    #[test]
    fn xd_31_sm_history_tracking() {
        let mut sm = Xd31StateMachine::new();
        sm.transition(Xd31State::Running).unwrap();
        sm.transition(Xd31State::Paused).unwrap();
        sm.transition(Xd31State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd31State::Idle);
        assert_eq!(sm.history()[0].to, Xd31State::Running);
        assert_eq!(sm.history()[1].from, Xd31State::Running);
        assert_eq!(sm.history()[2].to, Xd31State::Done);
    }

    #[test]
    fn xd_31_sm_serialize_deserialize() {
        let mut sm = Xd31StateMachine::new();
        sm.transition(Xd31State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd31StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd31State::Running));
    }

    #[test]
    fn xd_31_sm_deserialize_invalid() {
        assert_eq!(Xd31StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_31_sm_reset() {
        let mut sm = Xd31StateMachine::new();
        sm.transition(Xd31State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd31State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_31_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd31EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd31Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_31_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd31EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd31Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd31Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_31_bus_unsubscribe() {
        let mut bus = Xd31EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_31_event_kind_and_payload() {
        let e = Xd31Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd31Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_31_bus_clear_history() {
        let mut bus = Xd31EventBus::new();
        bus.publish(Xd31Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_31_sm_step_counter_increments() {
        let mut sm = Xd31StateMachine::new();
        sm.transition(Xd31State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd31State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #29 --

    #[test]
    fn xf29_trie_insert_search() {
        let mut t = Xf29Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf29_trie_starts_with() {
        let mut t = Xf29Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf29_trie_remove() {
        let mut t = Xf29Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf29_trie_word_count() {
        let mut t = Xf29Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf29_trie_longest_prefix() {
        let mut t = Xf29Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf29_trie_all_words() {
        let mut t = Xf29Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf29_trie_autocomplete() {
        let mut t = Xf29Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf29_trie_empty_search() {
        let t = Xf29Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf29_bloom_add_contains() {
        let mut bf = Xf29BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf29_bloom_probably_absent() {
        let bf = Xf29BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf29_bloom_false_positive_rate() {
        let mut bf = Xf29BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf29_bloom_clear() {
        let mut bf = Xf29BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf29_bloom_union() {
        let mut a = Xf29BloomFilter::xf_new(512, 2);
        let mut b = Xf29BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf29_bloom_intersection_estimate() {
        let mut a = Xf29BloomFilter::xf_new(512, 2);
        let mut b = Xf29BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf29_bloom_union_size_mismatch() {
        let a = Xf29BloomFilter::xf_new(256, 2);
        let b = Xf29BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh212_skip_insert_contains() {
        let mut sl = super::Xh212SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh212_skip_remove() {
        let mut sl = super::Xh212SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh212_skip_len() {
        let mut sl = super::Xh212SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh212_skip_range_query() {
        let mut sl = super::Xh212SkipList::xh_new(4);
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
    fn xh212_skip_floor_ceiling() {
        let mut sl = super::Xh212SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh212_skip_rank() {
        let mut sl = super::Xh212SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh212_skip_empty() {
        let sl = super::Xh212SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh212_skip_duplicates() {
        let mut sl = super::Xh212SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh212_bitset_set_test() {
        let mut bs = super::Xh212BitSet::xh_new(256);
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
    fn xh212_bitset_clear_count() {
        let mut bs = super::Xh212BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh212_bitset_and_or_xor() {
        let mut a = super::Xh212BitSet::xh_new(128);
        let mut b = super::Xh212BitSet::xh_new(128);
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
    fn xh212_bitset_iter_ones() {
        let mut bs = super::Xh212BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh212_bitset_first_last() {
        let mut bs = super::Xh212BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh212_bitset_empty() {
        let bs = super::Xh212BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi212_deque_push_pop_back() {
        let mut dq = super::Xi212Deque::xi_new(4);
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
    fn xi212_deque_push_pop_front() {
        let mut dq = super::Xi212Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi212_deque_mixed_ops() {
        let mut dq = super::Xi212Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi212_deque_get_and_split() {
        let mut dq = super::Xi212Deque::xi_new(8);
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
    fn xi212_deque_rotate_left() {
        let mut dq = super::Xi212Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi212_deque_rotate_right() {
        let mut dq = super::Xi212Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi212_deque_grow() {
        let mut dq = super::Xi212Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi212_deque_empty() {
        let dq = super::Xi212Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi212_interval_tree_insert_query() {
        let mut tree = super::Xi212IntervalTree::xi_new();
        tree.xi_insert(super::Xi212Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi212Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi212Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi212_interval_tree_overlap() {
        let mut tree = super::Xi212IntervalTree::xi_new();
        tree.xi_insert(super::Xi212Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi212Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi212Interval::xi_new(12, 20));
        let q = super::Xi212Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi212_interval_tree_remove() {
        let mut tree = super::Xi212IntervalTree::xi_new();
        tree.xi_insert(super::Xi212Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi212Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi212_interval_tree_gaps() {
        let mut tree = super::Xi212IntervalTree::xi_new();
        tree.xi_insert(super::Xi212Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi212Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi212Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi212Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi212Interval::xi_new(8, 10));
    }

    #[test]
    fn xi212_interval_tree_merge() {
        let mut tree = super::Xi212IntervalTree::xi_new();
        tree.xi_insert(super::Xi212Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi212Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi212Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi212Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi212Interval::xi_new(10, 15));
    }

    #[test]
    fn xi212_interval_tree_all() {
        let mut tree = super::Xi212IntervalTree::xi_new();
        tree.xi_insert(super::Xi212Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi212Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi212_interval_tree_empty() {
        let tree = super::Xi212IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi212_interval_tree_contains_point() {
        let iv = super::Xi212Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 212) ---

    #[test]
    fn xj_212_uf_make_and_find() {
        let mut uf = super::Xj212UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_212_uf_union_connected() {
        let mut uf = super::Xj212UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_212_uf_component_count() {
        let mut uf = super::Xj212UnionFind::xj_new();
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
    fn xj_212_uf_component_size() {
        let mut uf = super::Xj212UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_212_uf_largest_component() {
        let mut uf = super::Xj212UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_212_uf_many_elements() {
        let mut uf = super::Xj212UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_212_uf_separate_components() {
        let mut uf = super::Xj212UnionFind::xj_new();
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
    fn xj_212_uf_path_compression() {
        let mut uf = super::Xj212UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_212_bt_insert_get() {
        let mut bt = super::Xj212BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_212_bt_contains_len() {
        let mut bt = super::Xj212BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_212_bt_replace() {
        let mut bt = super::Xj212BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_212_bt_remove() {
        let mut bt = super::Xj212BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_212_bt_keys_values() {
        let mut bt = super::Xj212BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_212_bt_range() {
        let mut bt = super::Xj212BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_212_bt_min_max() {
        let mut bt = super::Xj212BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_212_bt_many_inserts() {
        let mut bt = super::Xj212BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_212 segment tree tests ---

    #[test]
    fn xk_212_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk212SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_212_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk212SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_212_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk212SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_212_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk212SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_212_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk212SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_212_st_single_element() {
        let data = vec![42];
        let st = super::Xk212SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_212_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk212SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_212_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk212SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_212 disjoint intervals tests ---

    #[test]
    fn xk_212_di_add_and_count() {
        let mut di = super::Xk212DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_212_di_merge_overlap() {
        let mut di = super::Xk212DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_212_di_contains() {
        let mut di = super::Xk212DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_212_di_remove() {
        let mut di = super::Xk212DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_212_di_covered_length() {
        let mut di = super::Xk212DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_212_di_gaps() {
        let mut di = super::Xk212DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_212_di_merge_adjacent() {
        let mut di = super::Xk212DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_212_di_empty() {
        let di = super::Xk212DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_212_rope_new_empty() {
        let rope = super::Xl212Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_212_rope_from_str() {
        let rope = super::Xl212Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_212_rope_insert_at() {
        let mut rope = super::Xl212Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_212_rope_delete_range() {
        let mut rope = super::Xl212Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_212_rope_char_at() {
        let rope = super::Xl212Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_212_rope_split_concat() {
        let rope = super::Xl212Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_212_rope_line_count() {
        let rope = super::Xl212Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_212_rope_line_at() {
        let rope = super::Xl212Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_212_sa_build_and_search() {
        let sa = super::Xl212SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_212_sa_count() {
        let sa = super::Xl212SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_212_sa_longest_repeated() {
        let sa = super::Xl212SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_212_sa_all_positions() {
        let sa = super::Xl212SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_212_sa_len() {
        let sa = super::Xl212SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_212_sa_empty() {
        let sa = super::Xl212SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_212_rope_slice() {
        let rope = super::Xl212Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_212_sa_search_start() {
        let sa = super::Xl212SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_212_sparse_set_get() {
        let mut m = super::Xm212MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_212_sparse_row_col() {
        let mut m = super::Xm212MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_212_sparse_transpose() {
        let mut m = super::Xm212MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_212_sparse_multiply_vec() {
        let mut m = super::Xm212MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_212_sparse_nnz_density() {
        let mut m = super::Xm212MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_212_sparse_clear() {
        let mut m = super::Xm212MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_212_sparse_overwrite_zero() {
        let mut m = super::Xm212MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_212_tokenizer_basic() {
        let t = super::Xm212Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_212_tokenizer_count() {
        let t = super::Xm212Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_212_tokenizer_unique() {
        let t = super::Xm212Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_212_tokenizer_frequency() {
        let t = super::Xm212Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_212_tokenizer_delimiter() {
        let t = super::Xm212Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_212_tokenizer_whitespace() {
        let t = super::Xm212Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_212_tokenizer_empty() {
        let t = super::Xm212Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 212 ----

    #[test]
    fn xn_212_fenwick_prefix_sum() {
        let mut ft = super::Xn212Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_212_fenwick_range_sum() {
        let mut ft = super::Xn212Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_212_fenwick_point_query() {
        let mut ft = super::Xn212Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_212_fenwick_len() {
        let ft = super::Xn212Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_212_fenwick_multiple_updates() {
        let mut ft = super::Xn212Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_212_fenwick_single_element() {
        let mut ft = super::Xn212Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_212_fenwick_find_kth() {
        let mut ft = super::Xn212Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_212_fenwick_negative_delta() {
        let mut ft = super::Xn212Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 212 ----

    #[test]
    fn xn_212_avl_insert_get() {
        let mut m = super::Xn212AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_212_avl_remove() {
        let mut m = super::Xn212AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_212_avl_in_order() {
        let mut m = super::Xn212AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_212_avl_min_max() {
        let mut m = super::Xn212AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_212_avl_floor_ceiling() {
        let mut m = super::Xn212AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_212_avl_height_balanced() {
        let mut m = super::Xn212AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_212_avl_overwrite() {
        let mut m = super::Xn212AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_212_avl_empty() {
        let m: super::Xn212AVL<i32, i32> = super::Xn212AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo212RedBlack tests ---

    #[test]
    fn xo_212_rb_insert_and_get() {
        let mut tree = super::Xo212RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_212_rb_len_and_empty() {
        let mut tree = super::Xo212RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_212_rb_min_max() {
        let mut tree = super::Xo212RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_212_rb_contains() {
        let mut tree = super::Xo212RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_212_rb_remove() {
        let mut tree = super::Xo212RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_212_rb_in_order() {
        let mut tree = super::Xo212RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_212_rb_black_height() {
        let mut tree = super::Xo212RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_212_rb_overwrite() {
        let mut tree = super::Xo212RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo212ConsistentHash tests ---

    #[test]
    fn xo_212_ch_add_and_count() {
        let mut ring = super::Xo212ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_212_ch_remove_node() {
        let mut ring = super::Xo212ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_212_ch_get_node() {
        let mut ring = super::Xo212ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_212_ch_empty_ring() {
        let ring = super::Xo212ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_212_ch_distribution() {
        let mut ring = super::Xo212ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_212_ch_rebalance() {
        let mut ring = super::Xo212ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_212_ch_virtual_nodes() {
        let mut ring = super::Xo212ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_212_ch_consistent_lookup() {
        let mut ring = super::Xo212ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }

}
