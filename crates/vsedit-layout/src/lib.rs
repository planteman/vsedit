//! Flexbox-like terminal layout engine.
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
    fn layout_validator_accepts_valid_name() {
        let v = LayoutValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn layout_validator_rejects_empty() {
        let v = LayoutValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn layout_validator_rejects_too_long() {
        let v = LayoutValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn layout_validator_forbidden_prefix() {
        let v = LayoutValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn layout_validator_allowed_chars() {
        let v = LayoutValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn layout_validator_range() {
        let v = LayoutValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn layout_sanitize_removes_control() {
        let result = LayoutValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn layout_truncate_short_string() {
        assert_eq!(LayoutValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn layout_truncate_long_string() {
        let result = LayoutValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn layout_is_ascii_printable() {
        assert!(LayoutValidator::is_ascii_printable("Hello World 123"));
        assert!(!LayoutValidator::is_ascii_printable("Hello\x00World"));
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
}
