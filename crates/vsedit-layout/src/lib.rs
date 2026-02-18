//! Flexbox-like terminal layout engine.
use std::collections::HashMap;
use std::fmt;

use vsedit_tui::Rect;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during layout operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    /// A percentage constraint exceeded 100.
    InvalidPercentage(u16),
    /// The sum of fixed constraints exceeds the available space.
    OverflowFixed { total_fixed: u16, available: u16 },
    /// A flex factor of zero was provided (would cause division issues).
    ZeroFlexFactor,
    /// No constraints were provided when at least one was expected.
    EmptyConstraints,
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPercentage(p) => write!(f, "percentage {p} exceeds 100"),
            Self::OverflowFixed {
                total_fixed,
                available,
            } => write!(
                f,
                "total fixed size {total_fixed} exceeds available space {available}"
            ),
            Self::ZeroFlexFactor => write!(f, "flex factor must be non-zero"),
            Self::EmptyConstraints => write!(f, "at least one constraint is required"),
        }
    }
}

impl std::error::Error for LayoutError {}

// ---------------------------------------------------------------------------
// Direction
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Horizontal,
    Vertical,
}

impl Direction {
    /// Returns the perpendicular direction.
    pub fn perpendicular(self) -> Self {
        match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Horizontal => write!(f, "horizontal"),
            Self::Vertical => write!(f, "vertical"),
        }
    }
}

// ---------------------------------------------------------------------------
// Constraint
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constraint {
    Fixed(u16),
    Percentage(u16),
    Min(u16),
    Max(u16),
    /// Flex grow factor — remaining space is distributed proportionally.
    Flex(u16),
}

impl Constraint {
    /// Validate that the constraint values are sensible.
    pub fn validate(&self) -> Result<(), LayoutError> {
        match *self {
            Self::Percentage(p) if p > 100 => Err(LayoutError::InvalidPercentage(p)),
            Self::Flex(0) => Err(LayoutError::ZeroFlexFactor),
            _ => Ok(()),
        }
    }

    /// Returns `true` when this constraint consumes a fixed amount of space
    /// (i.e. is not flex-based).
    pub fn is_fixed_size(&self) -> bool {
        !matches!(self, Self::Flex(_))
    }

    /// Returns the explicit size hint, if any.
    pub fn size_hint(&self) -> Option<u16> {
        match *self {
            Self::Fixed(v) | Self::Min(v) | Self::Max(v) => Some(v),
            Self::Percentage(p) => Some(p),
            Self::Flex(_) => None,
        }
    }
}

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fixed(v) => write!(f, "fixed({v})"),
            Self::Percentage(p) => write!(f, "{p}%"),
            Self::Min(v) => write!(f, "min({v})"),
            Self::Max(v) => write!(f, "max({v})"),
            Self::Flex(g) => write!(f, "flex({g})"),
        }
    }
}

// ---------------------------------------------------------------------------
// LayoutNode
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct LayoutNode {
    pub direction: Direction,
    pub constraints: Vec<Constraint>,
}

impl fmt::Display for LayoutNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LayoutNode({}, {} constraints)", self.direction, self.constraints.len())
    }
}

impl LayoutNode {
    pub fn horizontal(constraints: Vec<Constraint>) -> Self {
        Self {
            direction: Direction::Horizontal,
            constraints,
        }
    }

    pub fn vertical(constraints: Vec<Constraint>) -> Self {
        Self {
            direction: Direction::Vertical,
            constraints,
        }
    }

    /// Validate every constraint in this node.
    pub fn validate(&self) -> Result<(), LayoutError> {
        if self.constraints.is_empty() {
            return Err(LayoutError::EmptyConstraints);
        }
        for c in &self.constraints {
            c.validate()?;
        }
        let total_fixed: u32 = self
            .constraints
            .iter()
            .filter_map(|c| match c {
                Constraint::Fixed(v) => Some(*v as u32),
                _ => None,
            })
            .sum();
        // We report overflow but don't prevent split — it clamps gracefully.
        if total_fixed > u16::MAX as u32 {
            return Err(LayoutError::OverflowFixed {
                total_fixed: u16::MAX,
                available: u16::MAX,
            });
        }
        Ok(())
    }

    /// Return the number of constraints.
    pub fn len(&self) -> usize {
        self.constraints.len()
    }

    /// Returns `true` when there are no constraints.
    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }

    /// Compute the total area consumed by the split rectangles.
    pub fn total_consumed(&self, area: Rect) -> u16 {
        self.split(area)
            .iter()
            .map(|r| match self.direction {
                Direction::Horizontal => r.width,
                Direction::Vertical => r.height,
            })
            .sum()
    }

    /// Split `area` according to the configured constraints.
    pub fn split(&self, area: Rect) -> Vec<Rect> {
        let n = self.constraints.len();
        if n == 0 {
            return vec![];
        }

        let total = match self.direction {
            Direction::Horizontal => area.width,
            Direction::Vertical => area.height,
        };

        let mut sizes: Vec<u16> = vec![0; n];
        let mut remaining = total;
        let mut flex_total: u16 = 0;

        // First pass: allocate Fixed, Percentage, Min, Max; accumulate Flex.
        for (i, c) in self.constraints.iter().enumerate() {
            match *c {
                Constraint::Fixed(v) => {
                    let v = v.min(remaining);
                    sizes[i] = v;
                    remaining = remaining.saturating_sub(v);
                }
                Constraint::Percentage(p) => {
                    let v = ((total as u32 * p as u32) / 100) as u16;
                    let v = v.min(remaining);
                    sizes[i] = v;
                    remaining = remaining.saturating_sub(v);
                }
                Constraint::Min(v) => {
                    let v = v.min(remaining);
                    sizes[i] = v;
                    remaining = remaining.saturating_sub(v);
                }
                Constraint::Max(v) => {
                    let v = v.min(remaining);
                    sizes[i] = v;
                    remaining = remaining.saturating_sub(v);
                }
                Constraint::Flex(f) => {
                    flex_total = flex_total.saturating_add(f);
                }
            }
        }

        // Second pass: distribute remaining space to Flex constraints.
        if flex_total > 0 {
            let flex_remaining = remaining;
            for (i, c) in self.constraints.iter().enumerate() {
                if let Constraint::Flex(f) = *c {
                    let v = ((flex_remaining as u32 * f as u32) / flex_total as u32) as u16;
                    sizes[i] = v;
                }
            }
        }

        // Clamp Min/Max after flex distribution.
        for (i, c) in self.constraints.iter().enumerate() {
            match *c {
                Constraint::Min(min_val) => {
                    if sizes[i] < min_val {
                        sizes[i] = min_val.min(total);
                    }
                }
                Constraint::Max(max_val) => {
                    if sizes[i] > max_val {
                        sizes[i] = max_val;
                    }
                }
                _ => {}
            }
        }

        // Build result rects.
        let mut rects = Vec::with_capacity(n);
        let mut offset: u16 = 0;

        for size in &sizes {
            let rect = match self.direction {
                Direction::Horizontal => Rect::new(area.x + offset, area.y, *size, area.height),
                Direction::Vertical => Rect::new(area.x, area.y + offset, area.width, *size),
            };
            rects.push(rect);
            offset = offset.saturating_add(*size);
        }

        rects
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Ergonomic builder for constructing a [`LayoutNode`].
#[derive(Debug, Clone)]
pub struct LayoutBuilder {
    direction: Direction,
    constraints: Vec<Constraint>,
}

impl LayoutBuilder {
    /// Start building a horizontal layout.
    pub fn horizontal() -> Self {
        Self {
            direction: Direction::Horizontal,
            constraints: Vec::new(),
        }
    }

    /// Start building a vertical layout.
    pub fn vertical() -> Self {
        Self {
            direction: Direction::Vertical,
            constraints: Vec::new(),
        }
    }

    /// Add a constraint to the builder.
    pub fn constraint(mut self, c: Constraint) -> Self {
        self.constraints.push(c);
        self
    }

    /// Add a fixed-size constraint.
    pub fn fixed(self, size: u16) -> Self {
        self.constraint(Constraint::Fixed(size))
    }

    /// Add a percentage constraint.
    pub fn percentage(self, pct: u16) -> Self {
        self.constraint(Constraint::Percentage(pct))
    }

    /// Add a flex constraint.
    pub fn flex(self, factor: u16) -> Self {
        self.constraint(Constraint::Flex(factor))
    }

    /// Add a min constraint.
    pub fn min(self, value: u16) -> Self {
        self.constraint(Constraint::Min(value))
    }

    /// Add a max constraint.
    pub fn max(self, value: u16) -> Self {
        self.constraint(Constraint::Max(value))
    }

    /// Validate and build the [`LayoutNode`].
    pub fn build(self) -> Result<LayoutNode, LayoutError> {
        let node = LayoutNode {
            direction: self.direction,
            constraints: self.constraints,
        };
        node.validate()?;
        Ok(node)
    }

    /// Build without validation.
    pub fn build_unchecked(self) -> LayoutNode {
        LayoutNode {
            direction: self.direction,
            constraints: self.constraints,
        }
    }
}

// ---------------------------------------------------------------------------
// Margin helper
// ---------------------------------------------------------------------------

/// Inset an area by the given margin on all sides.
pub fn inset(area: Rect, margin: u16) -> Rect {
    let double = margin.saturating_mul(2);
    if area.width <= double || area.height <= double {
        return Rect::new(area.x, area.y, 0, 0);
    }
    Rect::new(
        area.x.saturating_add(margin),
        area.y.saturating_add(margin),
        area.width - double,
        area.height - double,
    )
}

/// Compute the center point of a [`Rect`].
pub fn center(area: Rect) -> (u16, u16) {
    (
        area.x.saturating_add(area.width / 2),
        area.y.saturating_add(area.height / 2),
    )
}

/// Return `true` when `inner` is fully contained within `outer`.
pub fn contains(outer: Rect, inner: Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x.saturating_add(inner.width) <= outer.x.saturating_add(outer.width)
        && inner.y.saturating_add(inner.height) <= outer.y.saturating_add(outer.height)
}

/// Accumulated statistics for layout operations.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl LayoutStats {
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
    pub fn merge(&mut self, other: &LayoutStats) {
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

impl Default for LayoutStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LayoutStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LayoutStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for layout.
#[derive(Debug, Clone)]
pub struct LayoutValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl LayoutValidator {
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

impl Default for LayoutValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// LayoutConstraint – min/max/preferred sizing
// ---------------------------------------------------------------------------

/// Describes minimum, maximum, and preferred sizing constraints for a region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutConstraint {
    pub min_width: u16,
    pub min_height: u16,
    pub max_width: u16,
    pub max_height: u16,
    pub preferred_width: Option<u16>,
    pub preferred_height: Option<u16>,
}

impl LayoutConstraint {
    /// Create a new unconstrained `LayoutConstraint`.
    pub fn new() -> Self {
        Self {
            min_width: 0,
            min_height: 0,
            max_width: u16::MAX,
            max_height: u16::MAX,
            preferred_width: None,
            preferred_height: None,
        }
    }

    /// Set the minimum width and height.
    pub fn with_min(mut self, w: u16, h: u16) -> Self {
        self.min_width = w;
        self.min_height = h;
        self
    }

    /// Set the maximum width and height.
    pub fn with_max(mut self, w: u16, h: u16) -> Self {
        self.max_width = w;
        self.max_height = h;
        self
    }

    /// Set the preferred width and height.
    pub fn with_preferred(mut self, w: u16, h: u16) -> Self {
        self.preferred_width = Some(w);
        self.preferred_height = Some(h);
        self
    }

    /// Clamp `w` to the min/max width range.
    pub fn clamp_width(&self, w: u16) -> u16 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp `h` to the min/max height range.
    pub fn clamp_height(&self, h: u16) -> u16 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Clamp both dimensions of a [`Rect`], preserving position.
    pub fn clamp_rect(&self, rect: Rect) -> Rect {
        Rect::new(
            rect.x,
            rect.y,
            self.clamp_width(rect.width),
            self.clamp_height(rect.height),
        )
    }

    /// Return `true` when `rect` satisfies the min/max constraints.
    pub fn is_satisfied_by(&self, rect: Rect) -> bool {
        rect.width >= self.min_width
            && rect.width <= self.max_width
            && rect.height >= self.min_height
            && rect.height <= self.max_height
    }

    /// Validate that min values do not exceed max values.
    pub fn validate(&self) -> Result<(), LayoutError> {
        if self.min_width > self.max_width {
            return Err(LayoutError::OverflowFixed {
                total_fixed: self.min_width,
                available: self.max_width,
            });
        }
        if self.min_height > self.max_height {
            return Err(LayoutError::OverflowFixed {
                total_fixed: self.min_height,
                available: self.max_height,
            });
        }
        Ok(())
    }
}

impl Default for LayoutConstraint {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SplitView – adjustable split panels
// ---------------------------------------------------------------------------

/// A two-panel split with an adjustable ratio.
#[derive(Debug, Clone)]
pub struct SplitView {
    direction: Direction,
    ratio: f64,
    min_first: u16,
    min_second: u16,
}

impl SplitView {
    /// Create a horizontal split (left | right).
    pub fn horizontal(ratio: f64) -> Self {
        Self {
            direction: Direction::Horizontal,
            ratio: ratio.clamp(0.0, 1.0),
            min_first: 0,
            min_second: 0,
        }
    }

    /// Create a vertical split (top / bottom).
    pub fn vertical(ratio: f64) -> Self {
        Self {
            direction: Direction::Vertical,
            ratio: ratio.clamp(0.0, 1.0),
            min_first: 0,
            min_second: 0,
        }
    }

    /// Set the minimum size of the first panel.
    pub fn with_min_first(mut self, min: u16) -> Self {
        self.min_first = min;
        self
    }

    /// Set the minimum size of the second panel.
    pub fn with_min_second(mut self, min: u16) -> Self {
        self.min_second = min;
        self
    }

    /// Compute the two panel rectangles from `area`.
    pub fn split(&self, area: Rect) -> (Rect, Rect) {
        match self.direction {
            Direction::Horizontal => {
                let total = area.width;
                let first_w = (total as f64 * self.ratio) as u16;
                let first_w = first_w.max(self.min_first);
                let second_w = total.saturating_sub(first_w);
                let second_w = second_w.max(self.min_second);
                let first_w = total.saturating_sub(second_w);
                (
                    Rect::new(area.x, area.y, first_w, area.height),
                    Rect::new(area.x.saturating_add(first_w), area.y, second_w, area.height),
                )
            }
            Direction::Vertical => {
                let total = area.height;
                let first_h = (total as f64 * self.ratio) as u16;
                let first_h = first_h.max(self.min_first);
                let second_h = total.saturating_sub(first_h);
                let second_h = second_h.max(self.min_second);
                let first_h = total.saturating_sub(second_h);
                (
                    Rect::new(area.x, area.y, area.width, first_h),
                    Rect::new(area.x, area.y.saturating_add(first_h), area.width, second_h),
                )
            }
        }
    }

    /// Update the split ratio, clamped to `0.0..=1.0`.
    pub fn set_ratio(&mut self, ratio: f64) {
        self.ratio = ratio.clamp(0.0, 1.0);
    }

    /// Return the current ratio.
    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    /// Adjust the ratio by a pixel delta relative to `total` size.
    pub fn resize_by(&mut self, delta: i16, total: u16) {
        if total == 0 {
            return;
        }
        let shift = delta as f64 / total as f64;
        self.ratio = (self.ratio + shift).clamp(0.0, 1.0);
    }
}

// ---------------------------------------------------------------------------
// layout_reflow – recalculate layouts on resize
// ---------------------------------------------------------------------------

/// Reflow a slice of [`LayoutNode`]s into a new `area`.
///
/// Returns one `Vec<Rect>` per node, each being the result of
/// [`LayoutNode::split`] applied to `area`.
pub fn layout_reflow(nodes: &[LayoutNode], area: Rect) -> Vec<Vec<Rect>> {
    nodes.iter().map(|node| node.split(area)).collect()
}

// ---------------------------------------------------------------------------
// LayoutGrid – grid-based layouts
// ---------------------------------------------------------------------------

/// A simple uniform grid layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutGrid {
    cols: u16,
    rows: u16,
}

impl LayoutGrid {
    /// Create a new grid with the given number of columns and rows.
    pub fn new(cols: u16, rows: u16) -> Self {
        Self { cols, rows }
    }

    /// Width of a single column within `area`.
    pub fn col_width(&self, area: Rect) -> u16 {
        if self.cols == 0 {
            return 0;
        }
        area.width / self.cols
    }

    /// Height of a single row within `area`.
    pub fn row_height(&self, area: Rect) -> u16 {
        if self.rows == 0 {
            return 0;
        }
        area.height / self.rows
    }

    /// Return the [`Rect`] for the cell at (`col`, `row`), or `None` if out of bounds.
    pub fn cell(&self, area: Rect, col: u16, row: u16) -> Option<Rect> {
        if col >= self.cols || row >= self.rows {
            return None;
        }
        let cw = self.col_width(area);
        let rh = self.row_height(area);
        Some(Rect::new(
            area.x.saturating_add(col.saturating_mul(cw)),
            area.y.saturating_add(row.saturating_mul(rh)),
            cw,
            rh,
        ))
    }

    /// Number of columns.
    pub fn cols(&self) -> u16 {
        self.cols
    }

    /// Number of rows.
    pub fn rows(&self) -> u16 {
        self.rows
    }

    /// Total number of cells.
    pub fn total_cells(&self) -> u16 {
        self.cols.saturating_mul(self.rows)
    }
}

// ---------------------------------------------------------------------------
// Padding
// ---------------------------------------------------------------------------

/// Padding values for all four sides of a rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Padding {
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
    pub left: u16,
}

impl Padding {
    /// Create uniform padding on all sides.
    pub fn uniform(v: u16) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }

    /// Create symmetric padding: `h` for left/right, `v` for top/bottom.
    pub fn symmetric(h: u16, v: u16) -> Self {
        Self {
            top: v,
            right: h,
            bottom: v,
            left: h,
        }
    }

    /// Total horizontal padding (`left + right`).
    pub fn horizontal(&self) -> u16 {
        self.left.saturating_add(self.right)
    }

    /// Total vertical padding (`top + bottom`).
    pub fn vertical(&self) -> u16 {
        self.top.saturating_add(self.bottom)
    }

    /// Shrink `area` by this padding. Returns a zero-size rect when padding
    /// exceeds the area dimensions.
    pub fn apply(&self, area: Rect) -> Rect {
        let h = self.horizontal();
        let v = self.vertical();
        if area.width <= h || area.height <= v {
            return Rect::new(area.x, area.y, 0, 0);
        }
        Rect::new(
            area.x.saturating_add(self.left),
            area.y.saturating_add(self.top),
            area.width - h,
            area.height - v,
        )
    }
}


// ---------------------------------------------------------------------------
// LayoutTreeNode - tree-based layout representation
// ---------------------------------------------------------------------------

/// Split direction for a layout tree node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// A node in a layout tree that can be either a leaf or an interior split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutTreeNode {
    pub split: Option<SplitDirection>,
    pub children: Vec<LayoutTreeNode>,
    pub area: Rect,
}

impl LayoutTreeNode {
    pub fn leaf(area: Rect) -> Self {
        Self { split: None, children: Vec::new(), area }
    }

    pub fn split_node(direction: SplitDirection, children: Vec<LayoutTreeNode>, area: Rect) -> Self {
        Self { split: Some(direction), children, area }
    }

    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    pub fn leaf_count(&self) -> usize {
        if self.is_leaf() {
            1
        } else {
            self.children.iter().map(|c| c.leaf_count()).sum()
        }
    }

    pub fn depth(&self) -> usize {
        if self.is_leaf() {
            0
        } else {
            1 + self.children.iter().map(|c| c.depth()).max().unwrap_or(0)
        }
    }
}

// ---------------------------------------------------------------------------
// LayoutSerializer
// ---------------------------------------------------------------------------

pub struct LayoutSerializer;

impl LayoutSerializer {
    pub fn serialize(node: &LayoutTreeNode) -> String {
        let mut out = String::new();
        Self::serialize_inner(node, 0, &mut out);
        out
    }

    fn serialize_inner(node: &LayoutTreeNode, indent: usize, out: &mut String) {
        let pad: String = "  ".repeat(indent);
        if node.is_leaf() {
            out.push_str(&format!(
                "{pad}leaf({},{},{},{})\n",
                node.area.x, node.area.y, node.area.width, node.area.height
            ));
        } else {
            let dir = match node.split {
                Some(SplitDirection::Horizontal) => "H",
                Some(SplitDirection::Vertical) => "V",
                None => "?",
            };
            out.push_str(&format!(
                "{pad}split-{dir}({},{},{},{}) {{\n",
                node.area.x, node.area.y, node.area.width, node.area.height
            ));
            for child in &node.children {
                Self::serialize_inner(child, indent + 1, out);
            }
            out.push_str(&format!("{pad}}}\n"));
        }
    }

    pub fn deserialize(input: &str) -> Option<LayoutTreeNode> {
        let lines: Vec<&str> = input.lines().collect();
        if lines.is_empty() {
            return None;
        }
        let (node, _) = Self::parse_node(&lines, 0)?;
        Some(node)
    }

    fn parse_node(lines: &[&str], idx: usize) -> Option<(LayoutTreeNode, usize)> {
        if idx >= lines.len() {
            return None;
        }
        let trimmed = lines[idx].trim();
        if trimmed.starts_with("leaf(") {
            let inner = trimmed.strip_prefix("leaf(")?.strip_suffix(')')?;
            let nums: Vec<u16> = inner.split(',').filter_map(|s| s.trim().parse().ok()).collect();
            if nums.len() != 4 { return None; }
            Some((LayoutTreeNode::leaf(Rect::new(nums[0], nums[1], nums[2], nums[3])), idx + 1))
        } else if trimmed.starts_with("split-") {
            let dir = if trimmed.starts_with("split-H") {
                SplitDirection::Horizontal
            } else {
                SplitDirection::Vertical
            };
            let paren_start = trimmed.find('(')?;
            let paren_end = trimmed.find(')')?;
            let inner = &trimmed[paren_start + 1..paren_end];
            let nums: Vec<u16> = inner.split(',').filter_map(|s| s.trim().parse().ok()).collect();
            if nums.len() != 4 { return None; }
            let area = Rect::new(nums[0], nums[1], nums[2], nums[3]);
            let mut children = Vec::new();
            let mut cur = idx + 1;
            while cur < lines.len() && !lines[cur].trim().starts_with('}') {
                let (child, next) = Self::parse_node(lines, cur)?;
                children.push(child);
                cur = next;
            }
            Some((LayoutTreeNode::split_node(dir, children, area), cur + 1))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// LayoutAnimation
// ---------------------------------------------------------------------------

/// Describes an animated transition between two rectangles.
#[derive(Debug, Clone)]
pub struct LayoutAnimation {
    pub from_rect: Rect,
    pub to_rect: Rect,
    pub progress: f64,
}

impl LayoutAnimation {
    pub fn new(from: Rect, to: Rect) -> Self {
        Self { from_rect: from, to_rect: to, progress: 0.0 }
    }

    pub fn set_progress(&mut self, p: f64) {
        self.progress = p.clamp(0.0, 1.0);
    }

    fn lerp(a: u16, b: u16, t: f64) -> u16 {
        (a as f64 + (b as f64 - a as f64) * t).round() as u16
    }

    pub fn interpolated_rect(&self) -> Rect {
        Rect::new(
            Self::lerp(self.from_rect.x, self.to_rect.x, self.progress),
            Self::lerp(self.from_rect.y, self.to_rect.y, self.progress),
            Self::lerp(self.from_rect.width, self.to_rect.width, self.progress),
            Self::lerp(self.from_rect.height, self.to_rect.height, self.progress),
        )
    }

    pub fn is_done(&self) -> bool {
        (self.progress - 1.0).abs() < f64::EPSILON
    }
}

// ---------------------------------------------------------------------------
// LayoutPreset
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutPreset {
    Default,
    SidebarLeft,
    SidebarRight,
    CenteredEditor,
    TwoColumn,
}

impl LayoutPreset {
    pub fn build(self, area: Rect) -> LayoutTreeNode {
        match self {
            LayoutPreset::Default => LayoutTreeNode::leaf(area),
            LayoutPreset::SidebarLeft => {
                let sidebar_w = area.width / 4;
                let main_w = area.width.saturating_sub(sidebar_w);
                LayoutTreeNode::split_node(
                    SplitDirection::Horizontal,
                    vec![
                        LayoutTreeNode::leaf(Rect::new(area.x, area.y, sidebar_w, area.height)),
                        LayoutTreeNode::leaf(Rect::new(area.x + sidebar_w, area.y, main_w, area.height)),
                    ],
                    area,
                )
            }
            LayoutPreset::SidebarRight => {
                let main_w = area.width * 3 / 4;
                let sidebar_w = area.width.saturating_sub(main_w);
                LayoutTreeNode::split_node(
                    SplitDirection::Horizontal,
                    vec![
                        LayoutTreeNode::leaf(Rect::new(area.x, area.y, main_w, area.height)),
                        LayoutTreeNode::leaf(Rect::new(area.x + main_w, area.y, sidebar_w, area.height)),
                    ],
                    area,
                )
            }
            LayoutPreset::CenteredEditor => {
                let margin = area.width / 6;
                let inner_w = area.width.saturating_sub(margin * 2);
                LayoutTreeNode::split_node(
                    SplitDirection::Horizontal,
                    vec![
                        LayoutTreeNode::leaf(Rect::new(area.x, area.y, margin, area.height)),
                        LayoutTreeNode::leaf(Rect::new(area.x + margin, area.y, inner_w, area.height)),
                        LayoutTreeNode::leaf(Rect::new(area.x + margin + inner_w, area.y, margin, area.height)),
                    ],
                    area,
                )
            }
            LayoutPreset::TwoColumn => {
                let half = area.width / 2;
                let rest = area.width.saturating_sub(half);
                LayoutTreeNode::split_node(
                    SplitDirection::Horizontal,
                    vec![
                        LayoutTreeNode::leaf(Rect::new(area.x, area.y, half, area.height)),
                        LayoutTreeNode::leaf(Rect::new(area.x + half, area.y, rest, area.height)),
                    ],
                    area,
                )
            }
        }
    }
}

impl fmt::Display for LayoutPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => write!(f, "Default"),
            Self::SidebarLeft => write!(f, "Sidebar Left"),
            Self::SidebarRight => write!(f, "Sidebar Right"),
            Self::CenteredEditor => write!(f, "Centered Editor"),
            Self::TwoColumn => write!(f, "Two Column"),
        }
    }
}

// ---------------------------------------------------------------------------
// Layout analysis utilities
// ---------------------------------------------------------------------------

/// Compute the total area (in cells) consumed by a `Rect`.
pub fn rect_area(r: &Rect) -> u32 {
    r.width as u32 * r.height as u32
}

/// Check if `inner` is fully contained within `outer`.
pub fn rect_contains(outer: &Rect, inner: &Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
}

/// Compute the intersection of two rectangles, returning `None` if they
/// don't overlap.
pub fn rect_intersection(a: &Rect, b: &Rect) -> Option<Rect> {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = (a.x + a.width).min(b.x + b.width);
    let y2 = (a.y + a.height).min(b.y + b.height);
    if x1 < x2 && y1 < y2 {
        Some(Rect::new(x1, y1, x2 - x1, y2 - y1))
    } else {
        None
    }
}

/// Split a rectangle into two parts along the given direction at the
/// specified `offset` from the start.
pub fn rect_split(r: &Rect, direction: Direction, offset: u16) -> (Rect, Rect) {
    match direction {
        Direction::Horizontal => {
            let w1 = offset.min(r.width);
            let w2 = r.width.saturating_sub(w1);
            (
                Rect::new(r.x, r.y, w1, r.height),
                Rect::new(r.x + w1, r.y, w2, r.height),
            )
        }
        Direction::Vertical => {
            let h1 = offset.min(r.height);
            let h2 = r.height.saturating_sub(h1);
            (
                Rect::new(r.x, r.y, r.width, h1),
                Rect::new(r.x, r.y + h1, r.width, h2),
            )
        }
    }
}

/// Apply `Padding` to a `Rect`, shrinking it inward. Returns a zero-sized
/// rect if padding exceeds the available space.
pub fn apply_padding(r: &Rect, p: &Padding) -> Rect {
    let x = r.x + p.left;
    let y = r.y + p.top;
    let w = r.width.saturating_sub(p.left + p.right);
    let h = r.height.saturating_sub(p.top + p.bottom);
    Rect::new(x, y, w, h)
}

/// Distribute `total` space among `count` items as evenly as possible,
/// returning the sizes. Leftover is given to the first items.
pub fn distribute_evenly(total: u16, count: u16) -> Vec<u16> {
    if count == 0 {
        return vec![];
    }
    let base = total / count;
    let extra = total % count;
    (0..count)
        .map(|i| if i < extra { base + 1 } else { base })
        .collect()
}

// ---------------------------------------------------------------------------
// Constraint solver
// ---------------------------------------------------------------------------

/// Enforces minimum and maximum size constraints on rectangles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutConstraintSolver {
    pub min_width: u16,
    pub max_width: u16,
    pub min_height: u16,
    pub max_height: u16,
}

impl LayoutConstraintSolver {
    pub fn new(min_width: u16, max_width: u16, min_height: u16, max_height: u16) -> Self {
        Self {
            min_width,
            max_width,
            min_height,
            max_height,
        }
    }

    /// Clamp a width value to the configured bounds.
    pub fn clamp_width(&self, w: u16) -> u16 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to the configured bounds.
    pub fn clamp_height(&self, h: u16) -> u16 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Return a new `Rect` whose width and height are clamped to the bounds.
    pub fn clamp_rect(&self, r: &Rect) -> Rect {
        Rect::new(
            r.x,
            r.y,
            self.clamp_width(r.width),
            self.clamp_height(r.height),
        )
    }

    /// Check whether a rectangle already satisfies all constraints.
    pub fn is_satisfied(&self, r: &Rect) -> bool {
        r.width >= self.min_width
            && r.width <= self.max_width
            && r.height >= self.min_height
            && r.height <= self.max_height
    }

    /// Return a list of human-readable constraint violations for `r`.
    pub fn violations(&self, r: &Rect) -> Vec<String> {
        let mut out = Vec::new();
        if r.width < self.min_width {
            out.push(format!(
                "width {} below minimum {}",
                r.width, self.min_width
            ));
        }
        if r.width > self.max_width {
            out.push(format!(
                "width {} above maximum {}",
                r.width, self.max_width
            ));
        }
        if r.height < self.min_height {
            out.push(format!(
                "height {} below minimum {}",
                r.height, self.min_height
            ));
        }
        if r.height > self.max_height {
            out.push(format!(
                "height {} above maximum {}",
                r.height, self.max_height
            ));
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Zone management
// ---------------------------------------------------------------------------

/// A named rectangular region in the layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutZone {
    pub name: String,
    pub rect: Rect,
    pub draggable: bool,
    pub visible: bool,
}

/// Manages a collection of named rectangular zones.
#[derive(Debug, Clone)]
pub struct LayoutZoneManager {
    zones: HashMap<String, LayoutZone>,
}

impl LayoutZoneManager {
    pub fn new() -> Self {
        Self {
            zones: HashMap::new(),
        }
    }

    /// Add a zone. New zones default to visible.
    pub fn add_zone(&mut self, name: &str, rect: Rect, draggable: bool) -> &mut Self {
        self.zones.insert(
            name.to_string(),
            LayoutZone {
                name: name.to_string(),
                rect,
                draggable,
                visible: true,
            },
        );
        self
    }

    /// Remove a zone by name, returning it if it existed.
    pub fn remove_zone(&mut self, name: &str) -> Option<LayoutZone> {
        self.zones.remove(name)
    }

    /// Find the first zone whose rectangle contains the point `(x, y)`.
    pub fn zone_at_point(&self, x: u16, y: u16) -> Option<&LayoutZone> {
        self.zones.values().find(|z| {
            z.visible
                && x >= z.rect.x
                && x < z.rect.x.saturating_add(z.rect.width)
                && y >= z.rect.y
                && y < z.rect.y.saturating_add(z.rect.height)
        })
    }

    /// Return all zones that are draggable.
    pub fn draggable_zones(&self) -> Vec<&LayoutZone> {
        self.zones
            .values()
            .filter(|z| z.draggable)
            .collect()
    }

    /// Return all zones that are visible.
    pub fn visible_zones(&self) -> Vec<&LayoutZone> {
        self.zones
            .values()
            .filter(|z| z.visible)
            .collect()
    }

    /// Total number of zones (visible or not).
    pub fn zone_count(&self) -> usize {
        self.zones.len()
    }
}

impl Default for LayoutZoneManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Resize handles
// ---------------------------------------------------------------------------

/// One of eight positions around a rectangle where a resize handle can sit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlePosition {
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// A resize handle attached to a zone edge or corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutResizeHandle {
    pub position: HandlePosition,
    pub size: u16,
}

impl LayoutResizeHandle {
    pub fn new(position: HandlePosition, size: u16) -> Self {
        Self { position, size }
    }

    /// Test whether the point `(x, y)` falls within this handle's hit area
    /// relative to `zone_rect`.
    pub fn hit_test(&self, zone_rect: &Rect, x: u16, y: u16) -> bool {
        let s = self.size;
        let zx = zone_rect.x;
        let zy = zone_rect.y;
        let zw = zone_rect.width;
        let zh = zone_rect.height;

        let (hx, hy, hw, hh) = match self.position {
            HandlePosition::Top => (zx, zy, zw, s),
            HandlePosition::Bottom => (zx, zy.saturating_add(zh).saturating_sub(s), zw, s),
            HandlePosition::Left => (zx, zy, s, zh),
            HandlePosition::Right => (zx.saturating_add(zw).saturating_sub(s), zy, s, zh),
            HandlePosition::TopLeft => (zx, zy, s, s),
            HandlePosition::TopRight => (zx.saturating_add(zw).saturating_sub(s), zy, s, s),
            HandlePosition::BottomLeft => (zx, zy.saturating_add(zh).saturating_sub(s), s, s),
            HandlePosition::BottomRight => (
                zx.saturating_add(zw).saturating_sub(s),
                zy.saturating_add(zh).saturating_sub(s),
                s,
                s,
            ),
        };

        x >= hx && x < hx.saturating_add(hw) && y >= hy && y < hy.saturating_add(hh)
    }
}

// ---------------------------------------------------------------------------
// Grid snapping
// ---------------------------------------------------------------------------

/// Snaps coordinate values to the nearest grid line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutSnapper {
    pub grid_size: u16,
}

impl LayoutSnapper {
    pub fn new(grid_size: u16) -> Self {
        assert!(grid_size > 0, "grid_size must be > 0");
        Self { grid_size }
    }

    /// Snap a single value to the nearest multiple of `grid_size`.
    pub fn snap(&self, value: u16) -> u16 {
        let g = self.grid_size;
        let remainder = value % g;
        if remainder >= g / 2 + g % 2 {
            value.saturating_add(g - remainder)
        } else {
            value - remainder
        }
    }

    /// Snap all four fields of a `Rect` to the grid.
    pub fn snap_rect(&self, r: &Rect) -> Rect {
        Rect::new(
            self.snap(r.x),
            self.snap(r.y),
            self.snap(r.width),
            self.snap(r.height),
        )
    }
}

// ---------------------------------------------------------------------------
// LayoutGridCalculator – calculates grid positions from constraints
// ---------------------------------------------------------------------------

/// A cell position within a calculated grid layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridCellPosition {
    pub row: u16,
    pub col: u16,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

/// Specification for a grid layout.
#[derive(Debug, Clone)]
pub struct GridSpec {
    pub rows: u16,
    pub cols: u16,
    pub row_gap: u16,
    pub col_gap: u16,
}

impl GridSpec {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self { rows, cols, row_gap: 0, col_gap: 0 }
    }

    pub fn with_gaps(mut self, row_gap: u16, col_gap: u16) -> Self {
        self.row_gap = row_gap;
        self.col_gap = col_gap;
        self
    }

    pub fn cell_count(&self) -> u16 {
        self.rows * self.cols
    }
}

/// Calculates grid cell positions from a specification and a bounding area.
#[derive(Debug)]
pub struct LayoutGridCalculator {
    spec: GridSpec,
}

impl LayoutGridCalculator {
    pub fn new(spec: GridSpec) -> Self {
        Self { spec }
    }

    /// Calculate positions for all grid cells within the given bounding rect.
    pub fn calculate(&self, area: &Rect) -> Vec<GridCellPosition> {
        if self.spec.rows == 0 || self.spec.cols == 0 {
            return Vec::new();
        }
        let total_row_gap = self.spec.row_gap.saturating_mul(self.spec.rows.saturating_sub(1));
        let total_col_gap = self.spec.col_gap.saturating_mul(self.spec.cols.saturating_sub(1));
        let usable_w = area.width.saturating_sub(total_col_gap);
        let usable_h = area.height.saturating_sub(total_row_gap);
        let cell_w = usable_w / self.spec.cols;
        let cell_h = usable_h / self.spec.rows;

        let mut cells = Vec::with_capacity(self.spec.cell_count() as usize);
        for r in 0..self.spec.rows {
            for c in 0..self.spec.cols {
                let x = area.x + c * (cell_w + self.spec.col_gap);
                let y = area.y + r * (cell_h + self.spec.row_gap);
                cells.push(GridCellPosition { row: r, col: c, x, y, width: cell_w, height: cell_h });
            }
        }
        cells
    }

    /// Get the cell at a specific row/col (0-indexed), or None if out of bounds.
    pub fn cell_at(&self, area: &Rect, row: u16, col: u16) -> Option<GridCellPosition> {
        if row >= self.spec.rows || col >= self.spec.cols {
            return None;
        }
        let cells = self.calculate(area);
        let idx = (row * self.spec.cols + col) as usize;
        cells.into_iter().nth(idx)
    }

    /// Total number of cells.
    pub fn cell_count(&self) -> u16 {
        self.spec.cell_count()
    }

    /// Get the spec.
    pub fn spec(&self) -> &GridSpec {
        &self.spec
    }
}

// ---------------------------------------------------------------------------
// LayoutSnapToEdgeHandler – snaps elements to edges/grid lines
// ---------------------------------------------------------------------------

/// Which edge an element was snapped to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapEdge {
    Top,
    Bottom,
    Left,
    Right,
}

/// Result of a snap operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapResult {
    pub snapped_x: u16,
    pub snapped_y: u16,
    pub edges: Vec<SnapEdge>,
}

/// Handles snapping elements to edges and grid lines.
#[derive(Debug)]
pub struct LayoutSnapToEdgeHandler {
    threshold: u16,
    grid_size: u16,
    snap_to_grid: bool,
}

impl LayoutSnapToEdgeHandler {
    pub fn new(threshold: u16) -> Self {
        Self { threshold, grid_size: 1, snap_to_grid: false }
    }

    pub fn with_grid(mut self, grid_size: u16) -> Self {
        self.grid_size = if grid_size == 0 { 1 } else { grid_size };
        self.snap_to_grid = true;
        self
    }

    /// Snap a point to the nearest grid line.
    pub fn snap_to_nearest_grid(&self, value: u16) -> u16 {
        if self.grid_size <= 1 {
            return value;
        }
        let remainder = value % self.grid_size;
        if remainder <= self.grid_size / 2 {
            value - remainder
        } else {
            value - remainder + self.grid_size
        }
    }

    /// Snap a position within a bounding area, considering edges and grid.
    pub fn snap(&self, x: u16, y: u16, area: &Rect) -> SnapResult {
        let mut snapped_x = x;
        let mut snapped_y = y;
        let mut edges = Vec::new();

        // Snap to left edge
        if x.saturating_sub(area.x) <= self.threshold {
            snapped_x = area.x;
            edges.push(SnapEdge::Left);
        }
        // Snap to right edge
        let right = area.x.saturating_add(area.width);
        if right.saturating_sub(x) <= self.threshold {
            snapped_x = right;
            edges.push(SnapEdge::Right);
        }
        // Snap to top edge
        if y.saturating_sub(area.y) <= self.threshold {
            snapped_y = area.y;
            edges.push(SnapEdge::Top);
        }
        // Snap to bottom edge
        let bottom = area.y.saturating_add(area.height);
        if bottom.saturating_sub(y) <= self.threshold {
            snapped_y = bottom;
            edges.push(SnapEdge::Bottom);
        }

        // Apply grid snapping if enabled and no edge snap happened
        if self.snap_to_grid {
            if !edges.iter().any(|e| matches!(e, SnapEdge::Left | SnapEdge::Right)) {
                snapped_x = self.snap_to_nearest_grid(snapped_x);
            }
            if !edges.iter().any(|e| matches!(e, SnapEdge::Top | SnapEdge::Bottom)) {
                snapped_y = self.snap_to_nearest_grid(snapped_y);
            }
        }

        SnapResult { snapped_x, snapped_y, edges }
    }

    pub fn threshold(&self) -> u16 {
        self.threshold
    }

    pub fn grid_size(&self) -> u16 {
        self.grid_size
    }
}

// --- LayoutGridV2: 2D grid layout ---

/// A 2D grid layout that assigns cells across rows and columns.
pub struct LayoutGridV2 {
    rows: usize,
    cols: usize,
    /// (row, col) -> (row_span, col_span)
    spans: HashMap<(usize, usize), (usize, usize)>,
}

impl LayoutGridV2 {
    pub fn new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, spans: HashMap::new() }
    }

    pub fn set_cell_span(&mut self, row: usize, col: usize, row_span: usize, col_span: usize) {
        if row + row_span <= self.rows && col + col_span <= self.cols {
            self.spans.insert((row, col), (row_span, col_span));
        }
    }

    pub fn total_cells(&self) -> usize {
        self.rows * self.cols
    }

    pub fn compute_cell_bounds(&self, area: Rect, row: usize, col: usize) -> Option<Rect> {
        if row >= self.rows || col >= self.cols {
            return None;
        }
        let cell_w = area.width / self.cols as u16;
        let cell_h = area.height / self.rows as u16;
        let (rs, cs) = self.spans.get(&(row, col)).copied().unwrap_or((1, 1));
        Some(Rect::new(
            area.x + col as u16 * cell_w,
            area.y + row as u16 * cell_h,
            cell_w * cs as u16,
            cell_h * rs as u16,
        ))
    }

    pub fn merge_cells(&mut self, row: usize, col: usize, row_span: usize, col_span: usize) -> bool {
        if row + row_span <= self.rows && col + col_span <= self.cols {
            self.spans.insert((row, col), (row_span, col_span));
            true
        } else {
            false
        }
    }

    pub fn rows(&self) -> usize { self.rows }
    pub fn cols(&self) -> usize { self.cols }
}

// --- LayoutBreakpointSystem: responsive breakpoints ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Breakpoint {
    Compact,
    Normal,
    Wide,
    UltraWide,
}

pub struct LayoutBreakpointSystem {
    compact_threshold: u16,
    normal_threshold: u16,
    wide_threshold: u16,
    current: Breakpoint,
}

impl LayoutBreakpointSystem {
    pub fn new(compact: u16, normal: u16, wide: u16) -> Self {
        Self {
            compact_threshold: compact,
            normal_threshold: normal,
            wide_threshold: wide,
            current: Breakpoint::Normal,
        }
    }

    pub fn current_breakpoint(&self) -> Breakpoint { self.current }

    pub fn on_resize(&mut self, width: u16) {
        self.current = if width < self.compact_threshold {
            Breakpoint::Compact
        } else if width < self.normal_threshold {
            Breakpoint::Normal
        } else if width < self.wide_threshold {
            Breakpoint::Wide
        } else {
            Breakpoint::UltraWide
        };
    }

    pub fn thresholds(&self) -> (u16, u16, u16) {
        (self.compact_threshold, self.normal_threshold, self.wide_threshold)
    }
}

// --- LayoutOverflowHandler ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowStrategy {
    Clip,
    Scroll,
    Wrap,
    Ellipsis,
}

pub struct LayoutOverflowHandler {
    strategy: OverflowStrategy,
    content_length: usize,
    visible_length: usize,
    scroll_offset: usize,
}

impl LayoutOverflowHandler {
    pub fn new(strategy: OverflowStrategy, content_length: usize, visible_length: usize) -> Self {
        Self { strategy, content_length, visible_length, scroll_offset: 0 }
    }

    pub fn strategy(&self) -> OverflowStrategy { self.strategy }

    pub fn compute_visible_content_range(&self) -> (usize, usize) {
        let start = self.scroll_offset.min(self.content_length);
        let end = (start + self.visible_length).min(self.content_length);
        (start, end)
    }

    pub fn needs_scrollbar(&self) -> bool {
        self.content_length > self.visible_length
    }

    pub fn set_scroll_offset(&mut self, offset: usize) {
        self.scroll_offset = offset.min(self.content_length.saturating_sub(self.visible_length));
    }

    pub fn content_length(&self) -> usize { self.content_length }
    pub fn visible_length(&self) -> usize { self.visible_length }
}



/// Configuration manager for layout functionality.
pub struct LayoutConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl LayoutConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &LayoutConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for layout operations.
pub struct LayoutRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl LayoutRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for layout.
pub struct LayoutValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl LayoutValidationCollector {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &LayoutValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Editor layout grid and split management — extended utilities (zv)
// ---------------------------------------------------------------------------

/// Metric accumulator for layout operations.
#[derive(Debug, Clone)]
pub struct ZvMetrics {
    samples: Vec<f64>,
    label: String,
}

impl ZvMetrics {
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

/// Sliding-window rate counter for layout.
#[derive(Debug, Clone)]
pub struct ZvRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl ZvRateWindow {
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

/// A small LRU-style cache for layout lookups.
#[derive(Debug, Clone)]
pub struct ZvLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZvLruCache {
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
// xa_ extended helpers for layout
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaLayoutRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaLayoutRingBuf {
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
pub struct XaLayoutCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaLayoutCounter {
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

impl Default for XaLayoutCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 109
// ---------------------------------------------------------------------------

/// Generic object pool `Xc109Pool<T>`.
pub struct Xc109Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc109Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc109PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc109Pool<T> {
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
    pub fn stats(&self) -> Xc109PoolStats {
        Xc109PoolStats {
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

impl<T> Default for Xc109Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc109Scheduler`.
pub struct Xc109Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc109Scheduler {
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

impl Default for Xc109Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_109 hash for the given byte slice.
pub fn xc_109_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_109 convention.
pub fn xc_109_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_34 deepening: state machine + event bus ---

/// States for the Xd34 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd34State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd34State {
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
pub struct Xd34Transition {
    pub from: Xd34State,
    pub to: Xd34State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd34StateMachine {
    current: Xd34State,
    history: Vec<Xd34Transition>,
    step_counter: usize,
}

impl Xd34StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd34State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd34State {
        self.current
    }

    pub fn history(&self) -> &[Xd34Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd34State) -> Result<Xd34State, String> {
        let allowed = match (self.current, target) {
            (Xd34State::Idle, Xd34State::Running) => true,
            (Xd34State::Running, Xd34State::Paused) => true,
            (Xd34State::Running, Xd34State::Done) => true,
            (Xd34State::Paused, Xd34State::Running) => true,
            (Xd34State::Paused, Xd34State::Done) => true,
            (Xd34State::Done, Xd34State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_34: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd34Transition {
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
            "Xd34SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd34State> {
        let prefix = "Xd34SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd34State::Idle),
            "Running" => Some(Xd34State::Running),
            "Paused" => Some(Xd34State::Paused),
            "Done" => Some(Xd34State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd34State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd34 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd34Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd34Event {
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

type Xd34HandlerFn = Box<dyn Fn(&Xd34Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd34EventBus {
    handlers: Vec<(usize, Option<String>, Xd34HandlerFn)>,
    next_id: usize,
    published: Vec<Xd34Event>,
}

impl Xd34EventBus {
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
        F: Fn(&Xd34Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd34Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd34Event) {
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

    pub fn published_events(&self) -> &[Xd34Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: u16, y: u16, w: u16, h: u16) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn fixed_horizontal() {
        let node = LayoutNode::horizontal(vec![Constraint::Fixed(10), Constraint::Fixed(20)]);
        let rects = node.split(rect(0, 0, 100, 50));
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0], rect(0, 0, 10, 50));
        assert_eq!(rects[1], rect(10, 0, 20, 50));
    }

    #[test]
    fn fixed_vertical() {
        let node = LayoutNode::vertical(vec![Constraint::Fixed(5), Constraint::Fixed(10)]);
        let rects = node.split(rect(0, 0, 80, 24));
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0], rect(0, 0, 80, 5));
        assert_eq!(rects[1], rect(0, 5, 80, 10));
    }

    #[test]
    fn percentage() {
        let node = LayoutNode::horizontal(vec![Constraint::Percentage(25), Constraint::Percentage(75)]);
        let rects = node.split(rect(0, 0, 100, 50));
        assert_eq!(rects[0].width, 25);
        assert_eq!(rects[1].width, 75);
    }

    #[test]
    fn flex_even_split() {
        let node = LayoutNode::horizontal(vec![Constraint::Flex(1), Constraint::Flex(1)]);
        let rects = node.split(rect(0, 0, 100, 50));
        assert_eq!(rects[0].width, 50);
        assert_eq!(rects[1].width, 50);
    }

    #[test]
    fn flex_weighted() {
        let node = LayoutNode::horizontal(vec![Constraint::Flex(1), Constraint::Flex(3)]);
        let rects = node.split(rect(0, 0, 100, 50));
        assert_eq!(rects[0].width, 25);
        assert_eq!(rects[1].width, 75);
    }

    #[test]
    fn mixed_fixed_and_flex() {
        let node = LayoutNode::horizontal(vec![
            Constraint::Fixed(20),
            Constraint::Flex(1),
            Constraint::Flex(1),
        ]);
        let rects = node.split(rect(0, 0, 100, 50));
        assert_eq!(rects[0].width, 20);
        assert_eq!(rects[1].width, 40);
        assert_eq!(rects[2].width, 40);
    }

    #[test]
    fn min_constraint() {
        let node = LayoutNode::horizontal(vec![Constraint::Min(30)]);
        let rects = node.split(rect(0, 0, 100, 50));
        assert!(rects[0].width >= 30);
    }

    #[test]
    fn max_constraint() {
        let node = LayoutNode::horizontal(vec![Constraint::Max(50)]);
        let rects = node.split(rect(0, 0, 100, 50));
        assert!(rects[0].width <= 50);
    }

    #[test]
    fn empty_constraints() {
        let node = LayoutNode::horizontal(vec![]);
        let rects = node.split(rect(0, 0, 100, 50));
        assert!(rects.is_empty());
    }

    #[test]
    fn offset_area() {
        let node = LayoutNode::horizontal(vec![Constraint::Fixed(10), Constraint::Fixed(10)]);
        let rects = node.split(rect(5, 3, 100, 50));
        assert_eq!(rects[0], rect(5, 3, 10, 50));
        assert_eq!(rects[1], rect(15, 3, 10, 50));
    }

    // ---- new tests ----

    #[test]
    fn direction_perpendicular() {
        assert_eq!(Direction::Horizontal.perpendicular(), Direction::Vertical);
        assert_eq!(Direction::Vertical.perpendicular(), Direction::Horizontal);
    }

    #[test]
    fn direction_display() {
        assert_eq!(Direction::Horizontal.to_string(), "horizontal");
        assert_eq!(Direction::Vertical.to_string(), "vertical");
    }

    #[test]
    fn constraint_display() {
        assert_eq!(Constraint::Fixed(10).to_string(), "fixed(10)");
        assert_eq!(Constraint::Percentage(50).to_string(), "50%");
        assert_eq!(Constraint::Min(5).to_string(), "min(5)");
        assert_eq!(Constraint::Max(80).to_string(), "max(80)");
        assert_eq!(Constraint::Flex(2).to_string(), "flex(2)");
    }

    #[test]
    fn constraint_validate_percentage_over_100() {
        assert_eq!(
            Constraint::Percentage(101).validate(),
            Err(LayoutError::InvalidPercentage(101))
        );
        assert!(Constraint::Percentage(100).validate().is_ok());
    }

    #[test]
    fn constraint_validate_zero_flex() {
        assert_eq!(
            Constraint::Flex(0).validate(),
            Err(LayoutError::ZeroFlexFactor)
        );
        assert!(Constraint::Flex(1).validate().is_ok());
    }

    #[test]
    fn constraint_is_fixed_size() {
        assert!(Constraint::Fixed(10).is_fixed_size());
        assert!(Constraint::Percentage(50).is_fixed_size());
        assert!(Constraint::Min(5).is_fixed_size());
        assert!(!Constraint::Flex(1).is_fixed_size());
    }

    #[test]
    fn constraint_size_hint() {
        assert_eq!(Constraint::Fixed(42).size_hint(), Some(42));
        assert_eq!(Constraint::Flex(3).size_hint(), None);
    }

    #[test]
    fn layout_node_len_and_display() {
        let node = LayoutNode::horizontal(vec![Constraint::Fixed(10), Constraint::Flex(1)]);
        assert_eq!(node.len(), 2);
        assert!(!node.is_empty());
        assert_eq!(node.to_string(), "LayoutNode(horizontal, 2 constraints)");
    }

    #[test]
    fn layout_node_validate_empty() {
        let node = LayoutNode::horizontal(vec![]);
        assert_eq!(node.validate(), Err(LayoutError::EmptyConstraints));
    }

    #[test]
    fn layout_node_total_consumed() {
        let node = LayoutNode::horizontal(vec![Constraint::Fixed(10), Constraint::Fixed(20)]);
        assert_eq!(node.total_consumed(rect(0, 0, 100, 50)), 30);
    }

    #[test]
    fn builder_horizontal() {
        let node = LayoutBuilder::horizontal()
            .fixed(20)
            .flex(1)
            .flex(1)
            .build()
            .unwrap();
        let rects = node.split(rect(0, 0, 100, 50));
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0].width, 20);
        assert_eq!(rects[1].width, 40);
        assert_eq!(rects[2].width, 40);
    }

    #[test]
    fn builder_validation_rejects_bad_percentage() {
        let result = LayoutBuilder::horizontal().percentage(120).build();
        assert!(result.is_err());
    }

    #[test]
    fn inset_normal() {
        let area = rect(10, 10, 100, 50);
        let inner = inset(area, 5);
        assert_eq!(inner, rect(15, 15, 90, 40));
    }

    #[test]
    fn inset_too_large() {
        let area = rect(0, 0, 10, 10);
        let inner = inset(area, 6);
        assert_eq!(inner.width, 0);
        assert_eq!(inner.height, 0);
    }

    #[test]
    fn center_calculation() {
        assert_eq!(center(rect(0, 0, 100, 50)), (50, 25));
        assert_eq!(center(rect(10, 20, 40, 60)), (30, 50));
    }

    #[test]
    fn contains_check() {
        let outer = rect(0, 0, 100, 100);
        let inner = rect(10, 10, 20, 20);
        assert!(contains(outer, inner));
        assert!(!contains(inner, outer));
    }

    #[test]
    fn layout_error_display() {
        let err = LayoutError::InvalidPercentage(150);
        assert_eq!(err.to_string(), "percentage 150 exceeds 100");
        let err2 = LayoutError::ZeroFlexFactor;
        assert_eq!(err2.to_string(), "flex factor must be non-zero");
    }

    #[test]
    fn layout_node_clone_and_eq() {
        let a = LayoutNode::horizontal(vec![Constraint::Fixed(10)]);
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn three_way_vertical_flex() {
        let node = LayoutNode::vertical(vec![
            Constraint::Flex(1),
            Constraint::Flex(2),
            Constraint::Flex(1),
        ]);
        let rects = node.split(rect(0, 0, 80, 40));
        assert_eq!(rects[0].height, 10);
        assert_eq!(rects[1].height, 20);
        assert_eq!(rects[2].height, 10);
    }

    #[test]
    fn layout_stats_new_defaults() {
        let stats = LayoutStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn layout_stats_record_success() {
        let mut stats = LayoutStats::new();
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
    fn layout_stats_record_failure() {
        let mut stats = LayoutStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn layout_stats_reset() {
        let mut stats = LayoutStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn layout_stats_merge() {
        let mut a = LayoutStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = LayoutStats::new();
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
    fn layout_stats_display() {
        let mut stats = LayoutStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn layout_stats_default() {
        let stats = LayoutStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn layout_validator_accepts_and_rejects() {
        let mut v = LayoutValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn layout_validator_warnings() {
        let mut v = LayoutValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn layout_validator_clear_and_merge() {
        let mut v = LayoutValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = LayoutValidationCollector::new();
        a.add_error("a_err");
        let mut b = LayoutValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
    }

    // -----------------------------------------------------------------------
    // LayoutConstraint tests
    // -----------------------------------------------------------------------

    #[test]
    fn layout_constraint_clamp_width() {
        let c = LayoutConstraint::new().with_min(10, 0).with_max(50, 100);
        assert_eq!(c.clamp_width(5), 10);
        assert_eq!(c.clamp_width(30), 30);
        assert_eq!(c.clamp_width(80), 50);
    }

    #[test]
    fn layout_constraint_is_satisfied_by() {
        let c = LayoutConstraint::new().with_min(10, 5).with_max(100, 50);
        assert!(c.is_satisfied_by(rect(0, 0, 50, 25)));
        assert!(!c.is_satisfied_by(rect(0, 0, 5, 25)));
        assert!(!c.is_satisfied_by(rect(0, 0, 50, 60)));
    }

    #[test]
    fn layout_constraint_validate_min_gt_max_fails() {
        let c = LayoutConstraint::new().with_min(100, 0).with_max(50, 100);
        assert!(c.validate().is_err());
    }

    #[test]
    fn layout_constraint_clamp_rect() {
        let c = LayoutConstraint::new().with_min(10, 10).with_max(80, 40);
        let r = c.clamp_rect(rect(5, 5, 200, 3));
        assert_eq!(r, rect(5, 5, 80, 10));
    }

    // -----------------------------------------------------------------------
    // SplitView tests
    // -----------------------------------------------------------------------

    #[test]
    fn split_view_horizontal() {
        let sv = SplitView::horizontal(0.5);
        let (a, b) = sv.split(rect(0, 0, 100, 50));
        assert_eq!(a, rect(0, 0, 50, 50));
        assert_eq!(b, rect(50, 0, 50, 50));
    }

    #[test]
    fn split_view_vertical() {
        let sv = SplitView::vertical(0.25);
        let (a, b) = sv.split(rect(0, 0, 80, 40));
        assert_eq!(a, rect(0, 0, 80, 10));
        assert_eq!(b, rect(0, 10, 80, 30));
    }

    #[test]
    fn split_view_resize_by_adjusts_ratio() {
        let mut sv = SplitView::horizontal(0.5);
        sv.resize_by(10, 100);
        assert!((sv.ratio() - 0.6).abs() < 1e-9);
        sv.resize_by(-20, 100);
        assert!((sv.ratio() - 0.4).abs() < 1e-9);
    }

    // -----------------------------------------------------------------------
    // layout_reflow test
    // -----------------------------------------------------------------------

    #[test]
    fn layout_reflow_returns_correct_structure() {
        let nodes = vec![
            LayoutNode::horizontal(vec![Constraint::Fixed(20), Constraint::Fixed(30)]),
            LayoutNode::vertical(vec![Constraint::Fixed(5)]),
        ];
        let area = rect(0, 0, 100, 50);
        let result = layout_reflow(&nodes, area);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].len(), 2);
        assert_eq!(result[1].len(), 1);
    }

    // -----------------------------------------------------------------------
    // LayoutGrid tests
    // -----------------------------------------------------------------------

    #[test]
    fn layout_grid_cell_calculation() {
        let grid = LayoutGrid::new(4, 2);
        assert_eq!(grid.total_cells(), 8);
        let area = rect(0, 0, 80, 40);
        let cell = grid.cell(area, 1, 0).unwrap();
        assert_eq!(cell, rect(20, 0, 20, 20));
        assert!(grid.cell(area, 5, 0).is_none());
    }

    // -----------------------------------------------------------------------
    // Padding tests
    // -----------------------------------------------------------------------

    #[test]
    fn padding_apply_shrinks_area() {
        let p = Padding::uniform(5);
        let r = p.apply(rect(0, 0, 100, 50));
        assert_eq!(r, rect(5, 5, 90, 40));
    }

    #[test]
    fn padding_apply_zero_when_too_large() {
        let p = Padding::uniform(30);
        let r = p.apply(rect(0, 0, 50, 50));
        assert_eq!(r, rect(0, 0, 0, 0));
    }

    // --- new tests ---

    #[test]
    fn layout_tree_node_leaf_count() {
        let root = LayoutTreeNode::split_node(
            SplitDirection::Horizontal,
            vec![
                LayoutTreeNode::leaf(rect(0, 0, 50, 100)),
                LayoutTreeNode::split_node(
                    SplitDirection::Vertical,
                    vec![
                        LayoutTreeNode::leaf(rect(50, 0, 50, 50)),
                        LayoutTreeNode::leaf(rect(50, 50, 50, 50)),
                    ],
                    rect(50, 0, 50, 100),
                ),
            ],
            rect(0, 0, 100, 100),
        );
        assert_eq!(root.leaf_count(), 3);
        assert_eq!(root.depth(), 2);
        assert!(!root.is_leaf());
    }

    #[test]
    fn layout_serialize_roundtrip() {
        let node = LayoutTreeNode::split_node(
            SplitDirection::Horizontal,
            vec![
                LayoutTreeNode::leaf(rect(0, 0, 40, 100)),
                LayoutTreeNode::leaf(rect(40, 0, 60, 100)),
            ],
            rect(0, 0, 100, 100),
        );
        let text = LayoutSerializer::serialize(&node);
        let parsed = LayoutSerializer::deserialize(&text).expect("should parse");
        assert_eq!(parsed.leaf_count(), 2);
        assert_eq!(parsed.area, rect(0, 0, 100, 100));
    }

    #[test]
    fn layout_animation_interpolation() {
        let mut anim = LayoutAnimation::new(rect(0, 0, 100, 100), rect(10, 10, 80, 80));
        anim.set_progress(0.5);
        let r = anim.interpolated_rect();
        assert_eq!(r.x, 5);
        assert_eq!(r.y, 5);
        assert_eq!(r.width, 90);
        assert!(!anim.is_done());
        anim.set_progress(1.0);
        assert!(anim.is_done());
    }

    #[test]
    fn layout_preset_sidebar_left() {
        let area = rect(0, 0, 120, 60);
        let tree = LayoutPreset::SidebarLeft.build(area);
        assert_eq!(tree.leaf_count(), 2);
        assert_eq!(tree.area, area);
        assert_eq!(tree.children[0].area.width, 30);
    }

    #[test]
    fn layout_preset_two_column() {
        let area = rect(0, 0, 100, 50);
        let tree = LayoutPreset::TwoColumn.build(area);
        assert_eq!(tree.leaf_count(), 2);
        assert_eq!(tree.children[0].area.width, 50);
    }

    #[test]
    fn layout_preset_display() {
        assert_eq!(format!("{}", LayoutPreset::CenteredEditor), "Centered Editor");
    }

    // -- rect_area -------------------------------------------------------------

    #[test]
    fn rect_area_computed() {
        assert_eq!(rect_area(&rect(0, 0, 10, 5)), 50);
    }

    // -- rect_contains ---------------------------------------------------------

    #[test]
    fn rect_contains_true() {
        let outer = rect(0, 0, 100, 100);
        let inner = rect(10, 10, 20, 20);
        assert!(rect_contains(&outer, &inner));
    }

    #[test]
    fn rect_contains_false() {
        let outer = rect(0, 0, 10, 10);
        let inner = rect(5, 5, 20, 20);
        assert!(!rect_contains(&outer, &inner));
    }

    // -- rect_intersection -----------------------------------------------------

    #[test]
    fn rect_intersection_overlap() {
        let a = rect(0, 0, 10, 10);
        let b = rect(5, 5, 10, 10);
        let i = rect_intersection(&a, &b).unwrap();
        assert_eq!(i, rect(5, 5, 5, 5));
    }

    #[test]
    fn rect_intersection_none() {
        let a = rect(0, 0, 5, 5);
        let b = rect(10, 10, 5, 5);
        assert!(rect_intersection(&a, &b).is_none());
    }

    // -- rect_split ------------------------------------------------------------

    #[test]
    fn rect_split_horizontal() {
        let r = rect(0, 0, 100, 50);
        let (left, right) = rect_split(&r, Direction::Horizontal, 30);
        assert_eq!(left, rect(0, 0, 30, 50));
        assert_eq!(right, rect(30, 0, 70, 50));
    }

    // -- apply_padding ---------------------------------------------------------

    #[test]
    fn apply_padding_shrinks() {
        let r = rect(0, 0, 100, 100);
        let p = Padding::uniform(10);
        let result = apply_padding(&r, &p);
        assert_eq!(result, rect(10, 10, 80, 80));
    }

    // -- distribute_evenly -----------------------------------------------------

    #[test]
    fn distribute_evenly_exact() {
        assert_eq!(distribute_evenly(12, 3), vec![4, 4, 4]);
    }

    #[test]
    fn distribute_evenly_remainder() {
        let sizes = distribute_evenly(10, 3);
        assert_eq!(sizes, vec![4, 3, 3]);
        assert_eq!(sizes.iter().sum::<u16>(), 10);
    }

    // -- LayoutConstraintSolver tests --

    #[test]
    fn test_constraint_solver_clamp_width() {
        let solver = LayoutConstraintSolver::new(10, 100, 5, 50);
        assert_eq!(solver.clamp_width(5), 10);
        assert_eq!(solver.clamp_width(50), 50);
        assert_eq!(solver.clamp_width(200), 100);
    }

    #[test]
    fn test_constraint_solver_clamp_height() {
        let solver = LayoutConstraintSolver::new(10, 100, 5, 50);
        assert_eq!(solver.clamp_height(2), 5);
        assert_eq!(solver.clamp_height(30), 30);
        assert_eq!(solver.clamp_height(80), 50);
    }

    #[test]
    fn test_constraint_solver_clamp_rect() {
        let solver = LayoutConstraintSolver::new(10, 100, 5, 50);
        let r = rect(1, 2, 200, 1);
        let clamped = solver.clamp_rect(&r);
        assert_eq!(clamped.x, 1);
        assert_eq!(clamped.y, 2);
        assert_eq!(clamped.width, 100);
        assert_eq!(clamped.height, 5);
    }

    #[test]
    fn test_constraint_solver_is_satisfied() {
        let solver = LayoutConstraintSolver::new(10, 100, 5, 50);
        assert!(solver.is_satisfied(&rect(0, 0, 50, 25)));
        assert!(!solver.is_satisfied(&rect(0, 0, 5, 25)));
        assert!(!solver.is_satisfied(&rect(0, 0, 50, 60)));
    }

    #[test]
    fn test_constraint_solver_violations() {
        let solver = LayoutConstraintSolver::new(10, 100, 5, 50);
        let v = solver.violations(&rect(0, 0, 50, 25));
        assert!(v.is_empty());
        let v = solver.violations(&rect(0, 0, 3, 80));
        assert_eq!(v.len(), 2);
        assert!(v[0].contains("width"));
        assert!(v[1].contains("height"));
    }

    // -- LayoutZoneManager tests --

    #[test]
    fn test_zone_manager_add_remove() {
        let mut mgr = LayoutZoneManager::new();
        mgr.add_zone("a", rect(0, 0, 10, 10), false);
        mgr.add_zone("b", rect(20, 0, 10, 10), true);
        assert_eq!(mgr.zone_count(), 2);
        let removed = mgr.remove_zone("a");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name, "a");
        assert_eq!(mgr.zone_count(), 1);
        assert!(mgr.remove_zone("nonexistent").is_none());
    }

    #[test]
    fn test_zone_manager_zone_at_point() {
        let mut mgr = LayoutZoneManager::new();
        mgr.add_zone("panel", rect(5, 5, 20, 10), false);
        assert!(mgr.zone_at_point(10, 8).is_some());
        assert_eq!(mgr.zone_at_point(10, 8).unwrap().name, "panel");
        assert!(mgr.zone_at_point(0, 0).is_none());
        assert!(mgr.zone_at_point(25, 15).is_none());
    }

    #[test]
    fn test_zone_manager_draggable_zones() {
        let mut mgr = LayoutZoneManager::new();
        mgr.add_zone("fixed", rect(0, 0, 10, 10), false);
        mgr.add_zone("drag1", rect(10, 0, 10, 10), true);
        mgr.add_zone("drag2", rect(20, 0, 10, 10), true);
        let draggable = mgr.draggable_zones();
        assert_eq!(draggable.len(), 2);
        assert!(draggable.iter().all(|z| z.draggable));
    }

    #[test]
    fn test_zone_manager_visible_zones() {
        let mut mgr = LayoutZoneManager::new();
        mgr.add_zone("vis", rect(0, 0, 10, 10), false);
        mgr.add_zone("also_vis", rect(10, 0, 10, 10), false);
        // All zones start visible
        assert_eq!(mgr.visible_zones().len(), 2);
    }

    // -- LayoutResizeHandle tests --

    #[test]
    fn test_resize_handle_hit_test() {
        let zone = rect(10, 10, 40, 30);
        let top = LayoutResizeHandle::new(HandlePosition::Top, 3);
        assert!(top.hit_test(&zone, 20, 10));
        assert!(top.hit_test(&zone, 20, 12));
        assert!(!top.hit_test(&zone, 20, 13));

        let br = LayoutResizeHandle::new(HandlePosition::BottomRight, 4);
        assert!(br.hit_test(&zone, 49, 39));
        assert!(!br.hit_test(&zone, 10, 10));

        let left = LayoutResizeHandle::new(HandlePosition::Left, 2);
        assert!(left.hit_test(&zone, 10, 20));
        assert!(!left.hit_test(&zone, 12, 20));
    }

    // -- LayoutSnapper tests --

    #[test]
    fn test_snapper_snap_value() {
        let snapper = LayoutSnapper::new(8);
        assert_eq!(snapper.snap(0), 0);
        assert_eq!(snapper.snap(3), 0);
        assert_eq!(snapper.snap(4), 8);
        assert_eq!(snapper.snap(7), 8);
        assert_eq!(snapper.snap(8), 8);
        assert_eq!(snapper.snap(12), 16);
    }

    #[test]
    fn test_snapper_snap_rect() {
        let snapper = LayoutSnapper::new(10);
        let r = rect(3, 7, 14, 26);
        let snapped = snapper.snap_rect(&r);
        assert_eq!(snapped.x, 0);
        assert_eq!(snapped.y, 10);
        assert_eq!(snapped.width, 10);
        assert_eq!(snapped.height, 30);
    }


    #[test]
    fn grid_spec_cell_count() {
        let spec = GridSpec::new(3, 4);
        assert_eq!(spec.cell_count(), 12);
    }

    #[test]
    fn grid_calculator_basic() {
        let calc = LayoutGridCalculator::new(GridSpec::new(2, 2));
        let area = rect(0, 0, 100, 100);
        let cells = calc.calculate(&area);
        assert_eq!(cells.len(), 4);
        assert_eq!(cells[0], GridCellPosition { row: 0, col: 0, x: 0, y: 0, width: 50, height: 50 });
        assert_eq!(cells[3], GridCellPosition { row: 1, col: 1, x: 50, y: 50, width: 50, height: 50 });
    }

    #[test]
    fn grid_calculator_with_gaps() {
        let spec = GridSpec::new(2, 2).with_gaps(2, 4);
        let calc = LayoutGridCalculator::new(spec);
        let area = rect(0, 0, 104, 52);
        let cells = calc.calculate(&area);
        assert_eq!(cells.len(), 4);
        assert_eq!(cells[0].width, 50);
        assert_eq!(cells[0].height, 25);
    }

    #[test]
    fn grid_calculator_empty() {
        let calc = LayoutGridCalculator::new(GridSpec::new(0, 5));
        let area = rect(0, 0, 100, 100);
        assert!(calc.calculate(&area).is_empty());
    }

    #[test]
    fn grid_calculator_cell_at() {
        let calc = LayoutGridCalculator::new(GridSpec::new(3, 3));
        let area = rect(0, 0, 90, 90);
        let cell = calc.cell_at(&area, 1, 2).unwrap();
        assert_eq!(cell.row, 1);
        assert_eq!(cell.col, 2);
        assert_eq!(cell.x, 60);
        assert_eq!(cell.y, 30);
        assert!(calc.cell_at(&area, 5, 0).is_none());
    }

    #[test]
    fn grid_calculator_cell_count() {
        let calc = LayoutGridCalculator::new(GridSpec::new(4, 3));
        assert_eq!(calc.cell_count(), 12);
    }

    #[test]
    fn snap_to_edge_left() {
        let handler = LayoutSnapToEdgeHandler::new(5);
        let area = rect(10, 10, 100, 100);
        let result = handler.snap(12, 50, &area);
        assert_eq!(result.snapped_x, 10);
        assert!(result.edges.contains(&SnapEdge::Left));
    }

    #[test]
    fn snap_to_edge_bottom() {
        let handler = LayoutSnapToEdgeHandler::new(5);
        let area = rect(0, 0, 100, 100);
        let result = handler.snap(50, 98, &area);
        assert_eq!(result.snapped_y, 100);
        assert!(result.edges.contains(&SnapEdge::Bottom));
    }

    #[test]
    fn snap_no_edge() {
        let handler = LayoutSnapToEdgeHandler::new(5);
        let area = rect(0, 0, 100, 100);
        let result = handler.snap(50, 50, &area);
        assert_eq!(result.snapped_x, 50);
        assert_eq!(result.snapped_y, 50);
        assert!(result.edges.is_empty());
    }

    #[test]
    fn snap_to_grid_lines() {
        let handler = LayoutSnapToEdgeHandler::new(2).with_grid(10);
        let area = rect(0, 0, 200, 200);
        let result = handler.snap(53, 47, &area);
        assert_eq!(result.snapped_x, 50);
        assert_eq!(result.snapped_y, 50);
    }

    #[test]
    fn snap_grid_nearest() {
        let handler = LayoutSnapToEdgeHandler::new(0).with_grid(10);
        assert_eq!(handler.snap_to_nearest_grid(14), 10);
        assert_eq!(handler.snap_to_nearest_grid(16), 20);
        assert_eq!(handler.snap_to_nearest_grid(15), 10);
    }

    #[test]
    fn snap_grid_size_zero_handled() {
        let handler = LayoutSnapToEdgeHandler::new(0).with_grid(0);
        assert_eq!(handler.grid_size(), 1);
    }

    #[test]
    fn snap_threshold_accessor() {
        let handler = LayoutSnapToEdgeHandler::new(7);
        assert_eq!(handler.threshold(), 7);
    }

    #[test]
    fn grid_v2_total_cells() {
        let g = LayoutGridV2::new(3, 4);
        assert_eq!(g.total_cells(), 12);
    }

    #[test]
    fn grid_v2_cell_bounds_basic() {
        let g = LayoutGridV2::new(2, 2);
        let bounds = g.compute_cell_bounds(rect(0, 0, 100, 100), 0, 0).unwrap();
        assert_eq!(bounds.x, 0);
        assert_eq!(bounds.y, 0);
        assert_eq!(bounds.width, 50);
        assert_eq!(bounds.height, 50);
    }

    #[test]
    fn grid_v2_cell_bounds_out_of_range() {
        let g = LayoutGridV2::new(2, 2);
        assert!(g.compute_cell_bounds(rect(0, 0, 100, 100), 5, 0).is_none());
    }

    #[test]
    fn grid_v2_merge_cells() {
        let mut g = LayoutGridV2::new(3, 3);
        assert!(g.merge_cells(0, 0, 2, 2));
        let bounds = g.compute_cell_bounds(rect(0, 0, 90, 90), 0, 0).unwrap();
        assert_eq!(bounds.width, 60);
        assert_eq!(bounds.height, 60);
    }

    #[test]
    fn grid_v2_merge_cells_out_of_bounds() {
        let mut g = LayoutGridV2::new(2, 2);
        assert!(!g.merge_cells(1, 1, 3, 3));
    }

    #[test]
    fn breakpoint_system_compact() {
        let mut bs = LayoutBreakpointSystem::new(40, 80, 120);
        bs.on_resize(30);
        assert_eq!(bs.current_breakpoint(), Breakpoint::Compact);
    }

    #[test]
    fn breakpoint_system_normal() {
        let mut bs = LayoutBreakpointSystem::new(40, 80, 120);
        bs.on_resize(50);
        assert_eq!(bs.current_breakpoint(), Breakpoint::Normal);
    }

    #[test]
    fn breakpoint_system_wide() {
        let mut bs = LayoutBreakpointSystem::new(40, 80, 120);
        bs.on_resize(100);
        assert_eq!(bs.current_breakpoint(), Breakpoint::Wide);
    }

    #[test]
    fn breakpoint_system_ultrawide() {
        let mut bs = LayoutBreakpointSystem::new(40, 80, 120);
        bs.on_resize(200);
        assert_eq!(bs.current_breakpoint(), Breakpoint::UltraWide);
    }

    #[test]
    fn overflow_handler_needs_scrollbar() {
        let h = LayoutOverflowHandler::new(OverflowStrategy::Scroll, 100, 50);
        assert!(h.needs_scrollbar());
    }

    #[test]
    fn overflow_handler_no_scrollbar() {
        let h = LayoutOverflowHandler::new(OverflowStrategy::Clip, 30, 50);
        assert!(!h.needs_scrollbar());
    }

    #[test]
    fn overflow_handler_visible_range() {
        let mut h = LayoutOverflowHandler::new(OverflowStrategy::Scroll, 100, 20);
        h.set_scroll_offset(10);
        assert_eq!(h.compute_visible_content_range(), (10, 30));
    }

    #[test]
    fn overflow_handler_clamp_offset() {
        let mut h = LayoutOverflowHandler::new(OverflowStrategy::Scroll, 50, 20);
        h.set_scroll_offset(999);
        assert_eq!(h.compute_visible_content_range(), (30, 50));
    }

    #[test]
    fn layout_config_new() {
        let cfg = LayoutConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn layout_config_set_get() {
        let mut cfg = LayoutConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn layout_config_remove() {
        let mut cfg = LayoutConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn layout_config_keys_sorted() {
        let mut cfg = LayoutConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn layout_config_bump_version() {
        let mut cfg = LayoutConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn layout_config_clear() {
        let mut cfg = LayoutConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn layout_config_merge() {
        let mut cfg1 = LayoutConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = LayoutConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn layout_config_disable() {
        let mut cfg = LayoutConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn layout_rate_tracker_empty() {
        let rt = LayoutRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn layout_rate_tracker_record() {
        let mut rt = LayoutRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn layout_rate_tracker_prune() {
        let mut rt = LayoutRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn layout_validator_valid() {
        let v = LayoutValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn layout_validator_errors() {
        let mut v = LayoutValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn layout_validator_clear() {
        let mut v = LayoutValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn layout_validator_merge() {
        let mut v1 = LayoutValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = LayoutValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn layout_rate_tracker_clear() {
        let mut rt = LayoutRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn zv_metrics_empty() {
        let m = ZvMetrics::new("layout");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zv_metrics_record_and_mean() {
        let mut m = ZvMetrics::new("layout");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zv_metrics_min_max() {
        let mut m = ZvMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zv_metrics_variance_and_std() {
        let mut m = ZvMetrics::new("v");
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
    fn zv_metrics_percentile() {
        let mut m = ZvMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn zv_metrics_merge() {
        let mut a = ZvMetrics::new("a");
        a.record(1.0);
        let mut b = ZvMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn zv_metrics_reset() {
        let mut m = ZvMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn zv_rate_window_empty() {
        let rw = ZvRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn zv_rate_window_tick_and_rate() {
        let mut rw = ZvRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn zv_lru_cache_basic() {
        let mut c = ZvLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn zv_lru_cache_contains_and_keys() {
        let mut c = ZvLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn zv_lru_cache_remove() {
        let mut c = ZvLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn zv_metrics_sum() {
        let mut m = ZvMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zv_metrics_label() {
        let m = ZvMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn zv_lru_cache_clear() {
        let mut c = ZvLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for layout
    #[test]
    fn xa_layout_ring_new() {
        let rb = super::XaLayoutRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_layout_ring_push_len() {
        let mut rb = super::XaLayoutRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_layout_ring_wrap() {
        let mut rb = super::XaLayoutRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_layout_ring_mean_empty() {
        let rb = super::XaLayoutRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_layout_ring_mean_values() {
        let mut rb = super::XaLayoutRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_layout_ring_min_max() {
        let mut rb = super::XaLayoutRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_layout_ring_iter() {
        let mut rb = super::XaLayoutRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_layout_counter_new() {
        let c = super::XaLayoutCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_layout_counter_inc() {
        let mut c = super::XaLayoutCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_layout_counter_inc_by() {
        let mut c = super::XaLayoutCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_layout_counter_reset() {
        let mut c = super::XaLayoutCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_layout_counter_clear() {
        let mut c = super::XaLayoutCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_layout_counter_default() {
        let c = super::XaLayoutCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 109 ----

    #[test]
    fn xc_109_pool_new_empty() {
        let pool: super::Xc109Pool<i32> = super::Xc109Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_109_pool_release_acquire() {
        let mut pool = super::Xc109Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_109_pool_acquire_empty() {
        let mut pool: super::Xc109Pool<i32> = super::Xc109Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_109_pool_full() {
        let mut pool = super::Xc109Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_109_pool_drain() {
        let mut pool = super::Xc109Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_109_pool_stats() {
        let mut pool = super::Xc109Pool::new(8);
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
    fn xc_109_pool_clear() {
        let mut pool = super::Xc109Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_109_pool_shrink() {
        let mut pool = super::Xc109Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_109_pool_default() {
        let pool: super::Xc109Pool<String> = super::Xc109Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_109_pool_extend() {
        let mut pool = super::Xc109Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_109_pool_retain() {
        let mut pool = super::Xc109Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_109_scheduler_round_robin() {
        let mut sched = super::Xc109Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_109_scheduler_empty() {
        let mut sched = super::Xc109Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_109_scheduler_reset() {
        let mut sched = super::Xc109Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_109_scheduler_add_remove() {
        let mut sched = super::Xc109Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_109_scheduler_targets() {
        let sched = super::Xc109Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_109_hash_empty() {
        assert_eq!(super::xc_109_hash(b""), 5381);
    }

    #[test]
    fn xc_109_hash_data() {
        let h = super::xc_109_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_109_hash(b"hello"), h);
    }

    #[test]
    fn xc_109_reverse_str() {
        assert_eq!(super::xc_109_reverse("abc"), "cba");
        assert_eq!(super::xc_109_reverse(""), "");
    }


    // --- xd_34 deepening tests ---

    #[test]
    fn xd_34_sm_initial_state() {
        let sm = Xd34StateMachine::new();
        assert_eq!(sm.current_state(), Xd34State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_34_sm_valid_idle_to_running() {
        let mut sm = Xd34StateMachine::new();
        assert!(sm.transition(Xd34State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd34State::Running);
    }

    #[test]
    fn xd_34_sm_valid_running_to_paused() {
        let mut sm = Xd34StateMachine::new();
        sm.transition(Xd34State::Running).unwrap();
        assert!(sm.transition(Xd34State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd34State::Paused);
    }

    #[test]
    fn xd_34_sm_valid_running_to_done() {
        let mut sm = Xd34StateMachine::new();
        sm.transition(Xd34State::Running).unwrap();
        assert!(sm.transition(Xd34State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd34State::Done);
    }

    #[test]
    fn xd_34_sm_valid_paused_to_running() {
        let mut sm = Xd34StateMachine::new();
        sm.transition(Xd34State::Running).unwrap();
        sm.transition(Xd34State::Paused).unwrap();
        assert!(sm.transition(Xd34State::Running).is_ok());
    }

    #[test]
    fn xd_34_sm_valid_done_to_idle() {
        let mut sm = Xd34StateMachine::new();
        sm.transition(Xd34State::Running).unwrap();
        sm.transition(Xd34State::Done).unwrap();
        assert!(sm.transition(Xd34State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd34State::Idle);
    }

    #[test]
    fn xd_34_sm_invalid_idle_to_done() {
        let mut sm = Xd34StateMachine::new();
        assert!(sm.transition(Xd34State::Done).is_err());
    }

    #[test]
    fn xd_34_sm_invalid_idle_to_paused() {
        let mut sm = Xd34StateMachine::new();
        assert!(sm.transition(Xd34State::Paused).is_err());
    }

    #[test]
    fn xd_34_sm_history_tracking() {
        let mut sm = Xd34StateMachine::new();
        sm.transition(Xd34State::Running).unwrap();
        sm.transition(Xd34State::Paused).unwrap();
        sm.transition(Xd34State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd34State::Idle);
        assert_eq!(sm.history()[0].to, Xd34State::Running);
        assert_eq!(sm.history()[1].from, Xd34State::Running);
        assert_eq!(sm.history()[2].to, Xd34State::Done);
    }

    #[test]
    fn xd_34_sm_serialize_deserialize() {
        let mut sm = Xd34StateMachine::new();
        sm.transition(Xd34State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd34StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd34State::Running));
    }

    #[test]
    fn xd_34_sm_deserialize_invalid() {
        assert_eq!(Xd34StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_34_sm_reset() {
        let mut sm = Xd34StateMachine::new();
        sm.transition(Xd34State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd34State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_34_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd34EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd34Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_34_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd34EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd34Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd34Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_34_bus_unsubscribe() {
        let mut bus = Xd34EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_34_event_kind_and_payload() {
        let e = Xd34Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd34Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_34_bus_clear_history() {
        let mut bus = Xd34EventBus::new();
        bus.publish(Xd34Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_34_sm_step_counter_increments() {
        let mut sm = Xd34StateMachine::new();
        sm.transition(Xd34State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd34State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }

}
