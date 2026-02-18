//! Column ruler lines.

use std::fmt;

/// Maximum column value allowed for a ruler.
pub const MAX_COLUMN: u32 = 10_000;

/// Errors that can occur when configuring rulers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulerError {
    /// Column exceeds the maximum allowed value.
    ColumnOutOfRange(u32),
    /// A ruler already exists at this column.
    DuplicateColumn(u32),
    /// The provided color string is invalid.
    InvalidColor(String),
    /// Character width must be positive.
    InvalidCharWidth(String),
    /// Viewport dimensions must be positive.
    InvalidViewport(String),
}

impl fmt::Display for RulerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RulerError::ColumnOutOfRange(col) => {
                write!(f, "column {col} exceeds maximum {MAX_COLUMN}")
            }
            RulerError::DuplicateColumn(col) => {
                write!(f, "ruler already exists at column {col}")
            }
            RulerError::InvalidColor(c) => write!(f, "invalid color: {c}"),
            RulerError::InvalidCharWidth(msg) => write!(f, "invalid char width: {msg}"),
            RulerError::InvalidViewport(msg) => write!(f, "invalid viewport: {msg}"),
        }
    }
}

impl std::error::Error for RulerError {}

/// Validate that a color string looks like a hex color (e.g. `#abc` or `#aabbcc`).
fn is_valid_hex_color(s: &str) -> bool {
    if !s.starts_with('#') {
        return false;
    }
    let hex = &s[1..];
    (hex.len() == 3 || hex.len() == 6) && hex.chars().all(|c| c.is_ascii_hexdigit())
}

/// A single ruler at a specific column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulerConfig {
    pub column: u32,
    pub color: Option<String>,
}

impl RulerConfig {
    /// Create a new ruler config, validating the column and optional color.
    pub fn new(column: u32, color: Option<String>) -> Result<Self, RulerError> {
        if column > MAX_COLUMN {
            return Err(RulerError::ColumnOutOfRange(column));
        }
        if let Some(ref c) = color {
            if !is_valid_hex_color(c) {
                return Err(RulerError::InvalidColor(c.clone()));
            }
        }
        Ok(Self { column, color })
    }

    /// Return the effective color, falling back to a provided default.
    pub fn effective_color<'a>(&'a self, default: &'a str) -> &'a str {
        self.color.as_deref().unwrap_or(default)
    }
}

impl fmt::Display for RulerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.color {
            Some(c) => write!(f, "ruler@{} ({})", self.column, c),
            None => write!(f, "ruler@{}", self.column),
        }
    }
}

/// Configuration for all editor rulers.
#[derive(Debug, Clone, PartialEq)]
pub struct RulersConfig {
    pub rulers: Vec<RulerConfig>,
    pub default_color: String,
}

impl Default for RulersConfig {
    fn default() -> Self {
        Self {
            rulers: Vec::new(),
            default_color: "#d3d3d3".to_string(),
        }
    }
}

impl RulersConfig {
    /// Add a ruler at the given column with an optional color override.
    pub fn add_ruler(&mut self, column: u32, color: Option<String>) {
        self.rulers.push(RulerConfig { column, color });
    }

    /// Remove all rulers at the given column. Returns the number removed.
    pub fn remove_ruler(&mut self, column: u32) -> usize {
        let before = self.rulers.len();
        self.rulers.retain(|r| r.column != column);
        before - self.rulers.len()
    }

    /// Return a slice of all configured rulers.
    pub fn get_rulers(&self) -> &[RulerConfig] {
        &self.rulers
    }

    /// Check whether any ruler exists at the given column.
    pub fn has_ruler_at(&self, column: u32) -> bool {
        self.rulers.iter().any(|r| r.column == column)
    }

    /// Return rulers sorted by column position.
    pub fn sorted_rulers(&self) -> Vec<&RulerConfig> {
        let mut sorted: Vec<&RulerConfig> = self.rulers.iter().collect();
        sorted.sort_by_key(|r| r.column);
        sorted
    }

    /// Add a ruler with full validation. Rejects out-of-range columns,
    /// invalid colors, and duplicate columns.
    pub fn add_ruler_validated(
        &mut self,
        column: u32,
        color: Option<String>,
    ) -> Result<(), RulerError> {
        if self.has_ruler_at(column) {
            return Err(RulerError::DuplicateColumn(column));
        }
        let rc = RulerConfig::new(column, color)?;
        self.rulers.push(rc);
        Ok(())
    }

    /// Return the number of configured rulers.
    pub fn len(&self) -> usize {
        self.rulers.len()
    }

    /// Return `true` if no rulers are configured.
    pub fn is_empty(&self) -> bool {
        self.rulers.is_empty()
    }

    /// Clear all rulers.
    pub fn clear(&mut self) {
        self.rulers.clear();
    }

    /// Return the column range `(min, max)` if rulers exist.
    pub fn column_range(&self) -> Option<(u32, u32)> {
        if self.rulers.is_empty() {
            return None;
        }
        let min = self.rulers.iter().map(|r| r.column).min().unwrap();
        let max = self.rulers.iter().map(|r| r.column).max().unwrap();
        Some((min, max))
    }

    /// Return all unique columns in ascending order.
    pub fn columns(&self) -> Vec<u32> {
        let mut cols: Vec<u32> = self.rulers.iter().map(|r| r.column).collect();
        cols.sort_unstable();
        cols.dedup();
        cols
    }
}

impl fmt::Display for RulersConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RulersConfig({} rulers, default={})",
            self.rulers.len(),
            self.default_color
        )
    }
}

/// Builder for constructing a `RulersConfig` with validation.
#[derive(Debug, Clone)]
pub struct RulersConfigBuilder {
    rulers: Vec<RulerConfig>,
    default_color: String,
}

impl RulersConfigBuilder {
    pub fn new() -> Self {
        Self {
            rulers: Vec::new(),
            default_color: "#d3d3d3".to_string(),
        }
    }

    pub fn default_color(mut self, color: impl Into<String>) -> Result<Self, RulerError> {
        let c = color.into();
        if !is_valid_hex_color(&c) {
            return Err(RulerError::InvalidColor(c));
        }
        self.default_color = c;
        Ok(self)
    }

    pub fn ruler(mut self, column: u32, color: Option<String>) -> Result<Self, RulerError> {
        if self.rulers.iter().any(|r| r.column == column) {
            return Err(RulerError::DuplicateColumn(column));
        }
        let rc = RulerConfig::new(column, color)?;
        self.rulers.push(rc);
        Ok(self)
    }

    pub fn build(self) -> RulersConfig {
        RulersConfig {
            rulers: self.rulers,
            default_color: self.default_color,
        }
    }
}

impl Default for RulersConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Service that computes ruler line positions from a `RulersConfig`.
#[derive(Debug, Clone, PartialEq)]
pub struct RulerService {
    config: RulersConfig,
}

impl RulerService {
    pub fn new(config: RulersConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &RulersConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut RulersConfig {
        &mut self.config
    }

    /// Compute pixel x-positions for ruler lines given a character width.
    pub fn compute_positions(&self, char_width: f64) -> Vec<RulerPosition> {
        self.config
            .rulers
            .iter()
            .map(|r| {
                let color = r
                    .color
                    .clone()
                    .unwrap_or_else(|| self.config.default_color.clone());
                RulerPosition {
                    column: r.column,
                    x: r.column as f64 * char_width,
                    color,
                }
            })
            .collect()
    }

    /// Return only rulers whose column falls within `[0, visible_columns)`.
    pub fn visible_rulers(&self, visible_columns: u32) -> Vec<&RulerConfig> {
        self.config
            .rulers
            .iter()
            .filter(|r| r.column < visible_columns)
            .collect()
    }

    /// Compute positions with validation on `char_width`.
    pub fn compute_positions_checked(&self, char_width: f64) -> Result<Vec<RulerPosition>, RulerError> {
        if char_width <= 0.0 || char_width.is_nan() || char_width.is_infinite() {
            return Err(RulerError::InvalidCharWidth(format!("{char_width}")));
        }
        Ok(self.compute_positions(char_width))
    }

    /// Compute positions only for rulers visible within the given column count.
    pub fn compute_visible_positions(
        &self,
        char_width: f64,
        visible_columns: u32,
    ) -> Vec<RulerPosition> {
        self.config
            .rulers
            .iter()
            .filter(|r| r.column < visible_columns)
            .map(|r| {
                let color = r
                    .color
                    .clone()
                    .unwrap_or_else(|| self.config.default_color.clone());
                RulerPosition {
                    column: r.column,
                    x: r.column as f64 * char_width,
                    color,
                }
            })
            .collect()
    }

    /// Total number of configured rulers.
    pub fn ruler_count(&self) -> usize {
        self.config.rulers.len()
    }
}

impl fmt::Display for RulerService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RulerService({})", self.config)
    }
}

/// A computed ruler position ready for rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct RulerPosition {
    pub column: u32,
    pub x: f64,
    pub color: String,
}

impl fmt::Display for RulerPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "col {} @ x={:.1} ({})", self.column, self.x, self.color)
    }
}

/// An overlay decoration representing a single ruler line.
#[derive(Debug, Clone, PartialEq)]
pub struct RulerDecoration {
    pub x: f64,
    pub height: f64,
    pub color: String,
    pub width: f64,
}

impl fmt::Display for RulerDecoration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "decoration x={:.1} h={:.1} w={:.1} ({})",
            self.x, self.height, self.width, self.color
        )
    }
}

/// Generate overlay decorations from ruler positions.
pub fn render_rulers(
    positions: &[RulerPosition],
    viewport_height: f64,
    line_width: f64,
) -> Vec<RulerDecoration> {
    positions
        .iter()
        .map(|pos| RulerDecoration {
            x: pos.x,
            height: viewport_height,
            color: pos.color.clone(),
            width: line_width,
        })
        .collect()
}

/// Validated version of `render_rulers`.
pub fn render_rulers_checked(
    positions: &[RulerPosition],
    viewport_height: f64,
    line_width: f64,
) -> Result<Vec<RulerDecoration>, RulerError> {
    if viewport_height <= 0.0 || viewport_height.is_nan() {
        return Err(RulerError::InvalidViewport(format!(
            "height must be positive, got {viewport_height}"
        )));
    }
    if line_width <= 0.0 || line_width.is_nan() {
        return Err(RulerError::InvalidViewport(format!(
            "line_width must be positive, got {line_width}"
        )));
    }
    Ok(render_rulers(positions, viewport_height, line_width))
}

/// Accumulated statistics for rulers operations.
#[derive(Debug, Clone, PartialEq)]
pub struct RulersStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl RulersStats {
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
    pub fn merge(&mut self, other: &RulersStats) {
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

impl Default for RulersStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RulersStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RulersStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for rulers.
#[derive(Debug, Clone)]
pub struct RulersValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl RulersValidator {
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

impl Default for RulersValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Public wrapper for hex color validation (delegates to private helper).
pub fn is_valid_hex_color_pub(s: &str) -> bool {
    is_valid_hex_color(s)
}

/// Higher-level ruler configuration with named ruler sets.
#[derive(Debug, Clone)]
pub struct RulerConfiguration {
    pub name: String,
    pub positions: Vec<u32>,
    pub color_map: Vec<(u32, String)>,
    pub default_color: String,
}

impl RulerConfiguration {
    /// Create a new named ruler configuration.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            positions: Vec::new(),
            color_map: Vec::new(),
            default_color: "#d3d3d3".to_string(),
        }
    }

    /// Add a ruler position.
    pub fn add_position(&mut self, col: u32) -> Result<(), RulerError> {
        if col > MAX_COLUMN {
            return Err(RulerError::ColumnOutOfRange(col));
        }
        if !self.positions.contains(&col) {
            self.positions.push(col);
            self.positions.sort_unstable();
        }
        Ok(())
    }

    /// Set color for a specific column position.
    pub fn set_color(&mut self, col: u32, color: impl Into<String>) -> Result<(), RulerError> {
        let c = color.into();
        if !is_valid_hex_color_pub(&c) {
            return Err(RulerError::InvalidColor(c));
        }
        self.color_map.retain(|(pos, _)| *pos != col);
        self.color_map.push((col, c));
        Ok(())
    }

    /// Get the number of ruler positions.
    pub fn position_count(&self) -> usize {
        self.positions.len()
    }

    /// Convert to a RulersConfig.
    pub fn to_rulers_config(&self) -> RulersConfig {
        let rulers = self
            .positions
            .iter()
            .map(|&col| {
                let color = self
                    .color_map
                    .iter()
                    .find(|(c, _)| *c == col)
                    .map(|(_, clr)| clr.clone());
                RulerConfig { column: col, color }
            })
            .collect();
        RulersConfig {
            rulers,
            default_color: self.default_color.clone(),
        }
    }
}

/// A computed render position for a ruler line.
#[derive(Debug, Clone, PartialEq)]
pub struct RulerRenderPosition {
    pub column: u32,
    pub pixel_x: f64,
    pub color: String,
}

/// Compute pixel render positions for all rulers in a configuration.
/// `char_width` is the width of a single character in pixels.
/// `scroll_offset` is the horizontal scroll offset in pixels.
pub fn ruler_render_positions(
    config: &RulersConfig,
    char_width: f64,
    scroll_offset: f64,
) -> Result<Vec<RulerRenderPosition>, RulerError> {
    if char_width <= 0.0 {
        return Err(RulerError::InvalidCharWidth(format!(
            "char_width must be positive, got {}",
            char_width
        )));
    }
    Ok(config
        .rulers
        .iter()
        .map(|r| {
            let pixel_x = (r.column as f64 * char_width) - scroll_offset;
            let color = r.effective_color(&config.default_color).to_string();
            RulerRenderPosition {
                column: r.column,
                pixel_x,
                color,
            }
        })
        .collect())
}

/// Resolve the color for a ruler at a specific column.
/// Checks the configuration's color map, then falls back to default.
pub fn ruler_color_by_position(config: &RulersConfig, column: u32) -> String {
    config
        .rulers
        .iter()
        .find(|r| r.column == column)
        .and_then(|r| r.color.clone())
        .unwrap_or_else(|| config.default_color.clone())
}

/// Check if any ruler is within a visible column range.
pub fn rulers_in_range(config: &RulersConfig, start_col: u32, end_col: u32) -> Vec<u32> {
    config
        .rulers
        .iter()
        .filter(|r| r.column >= start_col && r.column <= end_col)
        .map(|r| r.column)
        .collect()
}

// ---------------------------------------------------------------------------
// Iterator over ruler columns
// ---------------------------------------------------------------------------

/// Iterator that yields ruler columns in sorted order.
pub struct RulerColumnIter {
    columns: Vec<u32>,
    index: usize,
}

impl RulerColumnIter {
    /// Create a new iterator from a slice of columns.
    pub fn new(columns: &[u32]) -> Self {
        let mut sorted = columns.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        Self { columns: sorted, index: 0 }
    }

    /// Returns the number of remaining columns.
    pub fn remaining(&self) -> usize {
        self.columns.len() - self.index
    }
}

impl Iterator for RulerColumnIter {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.columns.len() {
            let val = self.columns[self.index];
            self.index += 1;
            Some(val)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let rem = self.remaining();
        (rem, Some(rem))
    }
}

impl ExactSizeIterator for RulerColumnIter {}

// ---------------------------------------------------------------------------
// RulerSpan: range between consecutive rulers
// ---------------------------------------------------------------------------

/// Represents a span between two consecutive ruler columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RulerSpan {
    /// Start column (inclusive).
    pub start: u32,
    /// End column (exclusive).
    pub end: u32,
}

impl RulerSpan {
    /// Width of the span.
    pub fn width(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    /// Returns true if `col` falls within this span.
    pub fn contains(&self, col: u32) -> bool {
        col >= self.start && col < self.end
    }
}

impl fmt::Display for RulerSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}..{})", self.start, self.end)
    }
}

/// Compute spans between consecutive rulers.
pub fn ruler_spans(columns: &[u32]) -> Vec<RulerSpan> {
    let mut sorted: Vec<u32> = columns.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    if sorted.is_empty() {
        return Vec::new();
    }
    let mut spans = Vec::new();
    // Span from 0 to first ruler
    if sorted[0] > 0 {
        spans.push(RulerSpan { start: 0, end: sorted[0] });
    }
    for w in sorted.windows(2) {
        spans.push(RulerSpan { start: w[0], end: w[1] });
    }
    spans
}

// ---------------------------------------------------------------------------
// From impls
// ---------------------------------------------------------------------------

impl From<u32> for RulerSpan {
    fn from(col: u32) -> Self {
        Self { start: 0, end: col }
    }
}

impl From<(u32, u32)> for RulerSpan {
    fn from((start, end): (u32, u32)) -> Self {
        Self { start, end }
    }
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Validates that a list of ruler columns are all within bounds and unique.
pub fn validate_ruler_columns(columns: &[u32]) -> Result<Vec<u32>, RulerError> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::with_capacity(columns.len());
    for &col in columns {
        if col > MAX_COLUMN {
            return Err(RulerError::ColumnOutOfRange(col));
        }
        if !seen.insert(col) {
            return Err(RulerError::DuplicateColumn(col));
        }
        result.push(col);
    }
    result.sort_unstable();
    Ok(result)
}

/// Returns the nearest ruler column to the given position.
pub fn nearest_ruler(columns: &[u32], position: u32) -> Option<u32> {
    if columns.is_empty() {
        return None;
    }
    columns.iter().copied().min_by_key(|&c| {
        if c > position { c - position } else { position - c }
    })
}

/// Summarizes ruler distribution as (min, max, mean).
pub fn ruler_distribution(columns: &[u32]) -> Option<(u32, u32, f64)> {
    if columns.is_empty() {
        return None;
    }
    let min = *columns.iter().min().unwrap();
    let max = *columns.iter().max().unwrap();
    let mean = columns.iter().map(|&c| c as f64).sum::<f64>() / columns.len() as f64;
    Some((min, max, mean))
}

/// Merges two sets of ruler columns, removing duplicates and sorting.
pub fn merge_ruler_columns(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut merged: Vec<u32> = a.iter().chain(b.iter()).copied().collect();
    merged.sort_unstable();
    merged.dedup();
    merged
}


// ---------------------------------------------------------------------------
// RulerSet
// ---------------------------------------------------------------------------

/// A sorted, deduplicated collection of `RulerConfig` entries.
#[derive(Debug, Clone, Default)]
pub struct RulerSet {
    rulers: Vec<RulerConfig>,
}

impl RulerSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a ruler. Returns an error on duplicates or invalid values.
    pub fn add(&mut self, cfg: RulerConfig) -> Result<(), RulerError> {
        if self.rulers.iter().any(|r| r.column == cfg.column) {
            return Err(RulerError::DuplicateColumn(cfg.column));
        }
        self.rulers.push(cfg);
        self.rulers.sort_by_key(|r| r.column);
        Ok(())
    }

    /// Remove a ruler by column. Returns `true` if found and removed.
    pub fn remove(&mut self, column: u32) -> bool {
        let before = self.rulers.len();
        self.rulers.retain(|r| r.column != column);
        self.rulers.len() < before
    }

    pub fn contains(&self, column: u32) -> bool {
        self.rulers.iter().any(|r| r.column == column)
    }

    pub fn iter(&self) -> impl Iterator<Item = &RulerConfig> {
        self.rulers.iter()
    }

    pub fn len(&self) -> usize {
        self.rulers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rulers.is_empty()
    }

    pub fn columns(&self) -> Vec<u32> {
        self.rulers.iter().map(|r| r.column).collect()
    }
}

impl fmt::Display for RulerSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RulerSet[")?;
        for (i, r) in self.rulers.iter().enumerate() {
            if i > 0 { write!(f, ", ")?; }
            write!(f, "{}", r.column)?;
        }
        write!(f, "]")
    }
}

// ---------------------------------------------------------------------------
// RulerPreset
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulerPreset {
    PEP8,
    StandardRust,
    CustomWidth(u32),
}

impl RulerPreset {
    pub fn to_columns(&self) -> Vec<u32> {
        match self {
            RulerPreset::PEP8 => vec![79, 120],
            RulerPreset::StandardRust => vec![100],
            RulerPreset::CustomWidth(w) => vec![*w],
        }
    }
}

impl fmt::Display for RulerPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PEP8 => write!(f, "PEP8 (79, 120)"),
            Self::StandardRust => write!(f, "Rust (100)"),
            Self::CustomWidth(w) => write!(f, "Custom ({w})"),
        }
    }
}

// ---------------------------------------------------------------------------
// RulerOverlapChecker
// ---------------------------------------------------------------------------

/// Checks whether any ruler columns coincide with tab stops.
pub struct RulerOverlapChecker;

impl RulerOverlapChecker {
    pub fn overlapping(ruler_columns: &[u32], tab_width: u32) -> Vec<u32> {
        if tab_width == 0 {
            return Vec::new();
        }
        ruler_columns
            .iter()
            .copied()
            .filter(|&c| c > 0 && c % tab_width == 0)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// RulerVisibility
// ---------------------------------------------------------------------------

/// Computes which rulers are visible within a horizontal viewport range.
#[derive(Debug)]
pub struct RulerVisibility {
    pub visible: Vec<u32>,
}

impl RulerVisibility {
    pub fn compute(columns: &[u32], start_col: u32, end_col: u32) -> Self {
        let visible = columns
            .iter()
            .copied()
            .filter(|&c| c >= start_col && c < end_col)
            .collect();
        Self { visible }
    }

    pub fn count(&self) -> usize {
        self.visible.len()
    }

    pub fn is_empty(&self) -> bool {
        self.visible.is_empty()
    }
}

// ---------------------------------------------------------------------------
// RulerGuide: descriptive guide lines for code style enforcement
// ---------------------------------------------------------------------------

/// A named guide that pairs a ruler column with a human-readable purpose,
/// making it easy to display tooltips or status-bar hints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulerGuide {
    pub column: u32,
    pub label: String,
    pub severity: GuideSeverity,
}

/// How strictly the ruler guide should be treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideSeverity {
    /// Informational – shown as a faint line.
    Info,
    /// Soft limit – highlight but don't warn.
    Soft,
    /// Hard limit – lines exceeding this should produce a warning.
    Hard,
}

impl fmt::Display for GuideSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GuideSeverity::Info => write!(f, "info"),
            GuideSeverity::Soft => write!(f, "soft"),
            GuideSeverity::Hard => write!(f, "hard"),
        }
    }
}

impl RulerGuide {
    /// Create a new guide with validation.
    pub fn new(column: u32, label: impl Into<String>, severity: GuideSeverity) -> Result<Self, RulerError> {
        if column > MAX_COLUMN {
            return Err(RulerError::ColumnOutOfRange(column));
        }
        Ok(Self {
            column,
            label: label.into(),
            severity,
        })
    }

    /// Returns true if this guide represents a hard limit.
    pub fn is_hard_limit(&self) -> bool {
        self.severity == GuideSeverity::Hard
    }

    /// Check whether a line length exceeds this guide's column.
    pub fn exceeds(&self, line_length: u32) -> bool {
        line_length > self.column
    }
}

impl fmt::Display for RulerGuide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} @{} [{}]", self.label, self.column, self.severity)
    }
}

/// A collection of ruler guides with lookup and filtering capabilities.
#[derive(Debug, Clone, Default)]
pub struct RulerGuideSet {
    guides: Vec<RulerGuide>,
}

impl RulerGuideSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a guide to the set. Rejects duplicates at the same column.
    pub fn add(&mut self, guide: RulerGuide) -> Result<(), RulerError> {
        if self.guides.iter().any(|g| g.column == guide.column) {
            return Err(RulerError::DuplicateColumn(guide.column));
        }
        self.guides.push(guide);
        self.guides.sort_by_key(|g| g.column);
        Ok(())
    }

    /// Return all hard-limit guides.
    pub fn hard_limits(&self) -> Vec<&RulerGuide> {
        self.guides.iter().filter(|g| g.is_hard_limit()).collect()
    }

    /// Return all guides that a given line length exceeds.
    pub fn exceeded_by(&self, line_length: u32) -> Vec<&RulerGuide> {
        self.guides.iter().filter(|g| g.exceeds(line_length)).collect()
    }

    /// Convert the guide set into a `RulersConfig`.
    pub fn to_rulers_config(&self, default_color: &str) -> RulersConfig {
        let rulers = self
            .guides
            .iter()
            .map(|g| RulerConfig { column: g.column, color: None })
            .collect();
        RulersConfig {
            rulers,
            default_color: default_color.to_string(),
        }
    }

    pub fn len(&self) -> usize {
        self.guides.len()
    }

    pub fn is_empty(&self) -> bool {
        self.guides.is_empty()
    }

    /// Return a summary string listing each guide.
    pub fn summary(&self) -> String {
        self.guides
            .iter()
            .map(|g| format!("{}", g))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl fmt::Display for RulerGuideSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RulerGuideSet({} guides)", self.guides.len())
    }
}

// ---------------------------------------------------------------------------
// RulerConfigParser – parse column-based ruler config strings
// ---------------------------------------------------------------------------

/// Parses ruler configuration strings like `"80,120:red,160"`.
///
/// Each entry is a column number optionally followed by `:color`.
#[derive(Debug, Clone)]
pub struct RulerConfigParser;

impl RulerConfigParser {
    /// Parse a comma-separated ruler specification string.
    ///
    /// Format: `"col[:color],col[:color],..."`.
    /// Returns a [`RulersConfig`] with the parsed rulers.
    pub fn parse(input: &str, default_color: &str) -> Result<RulersConfig, RulerError> {
        let mut config = RulersConfig::default();
        config.default_color = default_color.to_string();
        for segment in input.split(',') {
            let segment = segment.trim();
            if segment.is_empty() {
                continue;
            }
            let (col_str, color) = match segment.split_once(':') {
                Some((c, clr)) => (c.trim(), Some(clr.trim().to_string())),
                None => (segment, None),
            };
            let column: u32 = col_str
                .parse()
                .map_err(|_| RulerError::ColumnOutOfRange(0))?;
            config.add_ruler_validated(column, color)?;
        }
        Ok(config)
    }

    /// Parse a JSON-style array of ruler objects.
    ///
    /// Accepts entries like `[{"column":80},{"column":120,"color":"#ff0000"}]`.
    pub fn parse_json_array(entries: &[(u32, Option<&str>)]) -> Result<RulersConfig, RulerError> {
        let mut config = RulersConfig::default();
        for &(column, color) in entries {
            config.add_ruler_validated(column, color.map(|s| s.to_string()))?;
        }
        Ok(config)
    }
}

// ---------------------------------------------------------------------------
// MultiRulerRenderer – render multiple rulers with styling
// ---------------------------------------------------------------------------

/// Renders multiple rulers into a line buffer.
///
/// Each ruler is represented by a vertical bar character at the configured
/// column position.
#[derive(Debug, Clone)]
pub struct MultiRulerRenderer {
    /// The character used for ruler lines.
    pub ruler_char: char,
    /// Whether to show rulers beyond the visible area.
    pub clip_to_viewport: bool,
    /// Viewport width in columns.
    pub viewport_width: u32,
}

impl Default for MultiRulerRenderer {
    fn default() -> Self {
        Self {
            ruler_char: '│',
            clip_to_viewport: true,
            viewport_width: 120,
        }
    }
}

impl MultiRulerRenderer {
    /// Create a new renderer for the given viewport width.
    pub fn new(viewport_width: u32) -> Self {
        Self {
            viewport_width,
            ..Default::default()
        }
    }

    /// Render rulers into a string buffer of the given width.
    ///
    /// Returns a string where ruler positions contain [`ruler_char`] and all
    /// other positions contain spaces.
    pub fn render_line(&self, config: &RulersConfig) -> String {
        let width = self.viewport_width as usize;
        let mut buf = vec![' '; width];
        for ruler in &config.rulers {
            let col = ruler.column as usize;
            if col < width {
                buf[col] = self.ruler_char;
            } else if !self.clip_to_viewport {
                // Extend buffer to fit
                buf.resize(col + 1, ' ');
                buf[col] = self.ruler_char;
            }
        }
        buf.into_iter().collect()
    }

    /// Return the set of column positions that have rulers within the viewport.
    pub fn visible_columns(&self, config: &RulersConfig) -> Vec<u32> {
        config
            .rulers
            .iter()
            .filter(|r| !self.clip_to_viewport || r.column < self.viewport_width)
            .map(|r| r.column)
            .collect()
    }
}

impl fmt::Display for MultiRulerRenderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MultiRulerRenderer(char='{}', vp={})",
            self.ruler_char, self.viewport_width
        )
    }
}

// ---------------------------------------------------------------------------
// RulerColor – color customization for rulers
// ---------------------------------------------------------------------------

/// Represents a ruler color with optional opacity.
#[derive(Debug, Clone, PartialEq)]
pub struct RulerColor {
    /// Hex color string (e.g. `"#ff0000"`).
    pub hex: String,
    /// Opacity from 0.0 (transparent) to 1.0 (opaque).
    pub opacity: f64,
}

impl RulerColor {
    /// Create a new ruler color from a hex string.
    pub fn new(hex: impl Into<String>) -> Self {
        Self {
            hex: hex.into(),
            opacity: 1.0,
        }
    }

    /// Create a ruler color with a specific opacity.
    pub fn with_opacity(hex: impl Into<String>, opacity: f64) -> Self {
        Self {
            hex: hex.into(),
            opacity: opacity.clamp(0.0, 1.0),
        }
    }

    /// Validate the hex color string format.
    pub fn is_valid_hex(&self) -> bool {
        let h = self.hex.trim_start_matches('#');
        (h.len() == 3 || h.len() == 6) && h.chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Parse RGB components from the hex string.
    pub fn to_rgb(&self) -> Option<(u8, u8, u8)> {
        if !self.is_valid_hex() {
            return None;
        }
        let h = self.hex.trim_start_matches('#');
        let expanded = if h.len() == 3 {
            let chars: Vec<char> = h.chars().collect();
            format!(
                "{}{}{}{}{}{}",
                chars[0], chars[0], chars[1], chars[1], chars[2], chars[2]
            )
        } else {
            h.to_string()
        };
        let r = u8::from_str_radix(&expanded[0..2], 16).ok()?;
        let g = u8::from_str_radix(&expanded[2..4], 16).ok()?;
        let b = u8::from_str_radix(&expanded[4..6], 16).ok()?;
        Some((r, g, b))
    }

    /// Blend this color with another at the given factor (0.0 = self, 1.0 = other).
    pub fn blend(&self, other: &RulerColor, factor: f64) -> Option<RulerColor> {
        let (r1, g1, b1) = self.to_rgb()?;
        let (r2, g2, b2) = other.to_rgb()?;
        let f = factor.clamp(0.0, 1.0);
        let r = (r1 as f64 * (1.0 - f) + r2 as f64 * f) as u8;
        let g = (g1 as f64 * (1.0 - f) + g2 as f64 * f) as u8;
        let b = (b1 as f64 * (1.0 - f) + b2 as f64 * f) as u8;
        Some(RulerColor::new(format!("#{:02x}{:02x}{:02x}", r, g, b)))
    }
}

impl Default for RulerColor {
    fn default() -> Self {
        Self::new("#d3d3d3")
    }
}

impl fmt::Display for RulerColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if (self.opacity - 1.0).abs() < f64::EPSILON {
            write!(f, "{}", self.hex)
        } else {
            write!(f, "{}@{:.0}%", self.hex, self.opacity * 100.0)
        }
    }
}

// ---------------------------------------------------------------------------
// WordWrapGuideRuler – word wrap guide
// ---------------------------------------------------------------------------

/// A ruler that indicates the preferred word wrap column.
///
/// Unlike regular rulers which are decorative, the word wrap guide ruler
/// has semantic meaning—it shows where text will wrap.
#[derive(Debug, Clone)]
pub struct WordWrapGuideRuler {
    /// The word wrap column.
    pub wrap_column: u32,
    /// Color for the wrap guide.
    pub color: RulerColor,
    /// Whether the guide is currently active.
    pub active: bool,
    /// Style of the guide line.
    pub style: WrapGuideStyle,
}

/// Visual style for the word wrap guide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapGuideStyle {
    /// Solid vertical line.
    Solid,
    /// Dashed vertical line.
    Dashed,
    /// Dotted vertical line.
    Dotted,
}

impl fmt::Display for WrapGuideStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WrapGuideStyle::Solid => write!(f, "solid"),
            WrapGuideStyle::Dashed => write!(f, "dashed"),
            WrapGuideStyle::Dotted => write!(f, "dotted"),
        }
    }
}

impl Default for WordWrapGuideRuler {
    fn default() -> Self {
        Self {
            wrap_column: 80,
            color: RulerColor::with_opacity("#808080", 0.5),
            active: true,
            style: WrapGuideStyle::Solid,
        }
    }
}

impl WordWrapGuideRuler {
    /// Create a word wrap guide at the given column.
    pub fn new(wrap_column: u32) -> Self {
        Self {
            wrap_column,
            ..Default::default()
        }
    }

    /// Check whether a line exceeds the wrap column.
    pub fn exceeds_wrap(&self, line_length: u32) -> bool {
        line_length > self.wrap_column
    }

    /// Calculate how many characters overflow past the wrap guide.
    pub fn overflow_amount(&self, line_length: u32) -> u32 {
        line_length.saturating_sub(self.wrap_column)
    }

    /// Return the character used to render this guide style.
    pub fn guide_char(&self) -> char {
        match self.style {
            WrapGuideStyle::Solid => '│',
            WrapGuideStyle::Dashed => '┆',
            WrapGuideStyle::Dotted => '┊',
        }
    }

    /// Convert this guide into a [`RulerConfig`] for unified rendering.
    pub fn to_ruler_config(&self) -> Result<RulerConfig, RulerError> {
        RulerConfig::new(self.wrap_column, Some(self.color.hex.clone()))
    }
}

impl fmt::Display for WordWrapGuideRuler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WordWrapGuide(col={}, style={}, active={})",
            self.wrap_column, self.style, self.active
        )
    }
}

// ---------------------------------------------------------------------------
// RulerInteractiveEditor
// ---------------------------------------------------------------------------

/// An edit action on a ruler position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulerEditAction {
    Add(u32),
    Remove(u32),
    Move { from: u32, to: u32 },
}

impl std::fmt::Display for RulerEditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RulerEditAction::Add(col) => write!(f, "+{col}"),
            RulerEditAction::Remove(col) => write!(f, "-{col}"),
            RulerEditAction::Move { from, to } => write!(f, "{from}->{to}"),
        }
    }
}

/// Allows interactive editing of ruler positions with undo/redo support.
pub struct RulerInteractiveEditor {
    positions: Vec<u32>,
    undo_stack: Vec<RulerEditAction>,
    redo_stack: Vec<RulerEditAction>,
    max_rulers: usize,
}

impl RulerInteractiveEditor {
    pub fn new(max_rulers: usize) -> Self {
        Self {
            positions: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_rulers,
        }
    }

    pub fn positions(&self) -> &[u32] {
        &self.positions
    }

    pub fn count(&self) -> usize {
        self.positions.len()
    }

    pub fn add(&mut self, column: u32) -> Result<(), String> {
        if self.positions.len() >= self.max_rulers {
            return Err(format!("max rulers ({}) reached", self.max_rulers));
        }
        if self.positions.contains(&column) {
            return Err(format!("ruler at column {column} already exists"));
        }
        self.positions.push(column);
        self.positions.sort();
        self.undo_stack.push(RulerEditAction::Add(column));
        self.redo_stack.clear();
        Ok(())
    }

    pub fn remove(&mut self, column: u32) -> Result<(), String> {
        if let Some(pos) = self.positions.iter().position(|&c| c == column) {
            self.positions.remove(pos);
            self.undo_stack.push(RulerEditAction::Remove(column));
            self.redo_stack.clear();
            Ok(())
        } else {
            Err(format!("no ruler at column {column}"))
        }
    }

    pub fn move_ruler(&mut self, from: u32, to: u32) -> Result<(), String> {
        if !self.positions.contains(&from) {
            return Err(format!("no ruler at column {from}"));
        }
        if self.positions.contains(&to) {
            return Err(format!("ruler at column {to} already exists"));
        }
        self.positions.retain(|&c| c != from);
        self.positions.push(to);
        self.positions.sort();
        self.undo_stack.push(RulerEditAction::Move { from, to });
        self.redo_stack.clear();
        Ok(())
    }

    pub fn undo(&mut self) -> Option<RulerEditAction> {
        let action = self.undo_stack.pop()?;
        match &action {
            RulerEditAction::Add(col) => {
                self.positions.retain(|c| c != col);
            }
            RulerEditAction::Remove(col) => {
                self.positions.push(*col);
                self.positions.sort();
            }
            RulerEditAction::Move { from, to } => {
                self.positions.retain(|c| c != to);
                self.positions.push(*from);
                self.positions.sort();
            }
        }
        self.redo_stack.push(action.clone());
        Some(action)
    }

    pub fn redo(&mut self) -> Option<RulerEditAction> {
        let action = self.redo_stack.pop()?;
        match &action {
            RulerEditAction::Add(col) => {
                self.positions.push(*col);
                self.positions.sort();
            }
            RulerEditAction::Remove(col) => {
                self.positions.retain(|c| c != col);
            }
            RulerEditAction::Move { from, to } => {
                self.positions.retain(|c| c != from);
                self.positions.push(*to);
                self.positions.sort();
            }
        }
        self.undo_stack.push(action.clone());
        Some(action)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn clear(&mut self) {
        self.positions.clear();
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Generate a summary of the current state.
    pub fn summary(&self) -> String {
        let cols: Vec<String> = self.positions.iter().map(|c| c.to_string()).collect();
        format!("Rulers at columns: [{}]", cols.join(", "))
    }
}

impl std::fmt::Display for RulerInteractiveEditor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RulerInteractiveEditor({} rulers, max={})", self.positions.len(), self.max_rulers)
    }
}

// ---------------------------------------------------------------------------
// RulerPositionValidator
// ---------------------------------------------------------------------------

/// Constraint for validating ruler positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulerConstraint {
    pub min_column: u32,
    pub max_column: u32,
    pub min_spacing: u32,
    pub max_rulers: usize,
}

impl RulerConstraint {
    pub fn new(min_column: u32, max_column: u32, min_spacing: u32, max_rulers: usize) -> Self {
        Self { min_column, max_column, min_spacing, max_rulers }
    }
}

impl Default for RulerConstraint {
    fn default() -> Self {
        Self { min_column: 1, max_column: 320, min_spacing: 1, max_rulers: 20 }
    }
}

impl std::fmt::Display for RulerConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RulerConstraint(col {}-{}, spacing>={}, max={})",
            self.min_column, self.max_column, self.min_spacing, self.max_rulers)
    }
}

/// Validation error for ruler positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulerValidationError {
    OutOfRange { column: u32, min: u32, max: u32 },
    TooClose { col_a: u32, col_b: u32, min_spacing: u32 },
    TooManyRulers { count: usize, max: usize },
    DuplicateColumn(u32),
}

impl std::fmt::Display for RulerValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RulerValidationError::OutOfRange { column, min, max } =>
                write!(f, "column {column} out of range [{min}, {max}]"),
            RulerValidationError::TooClose { col_a, col_b, min_spacing } =>
                write!(f, "columns {col_a} and {col_b} too close (min spacing {min_spacing})"),
            RulerValidationError::TooManyRulers { count, max } =>
                write!(f, "too many rulers: {count} > {max}"),
            RulerValidationError::DuplicateColumn(col) =>
                write!(f, "duplicate column: {col}"),
        }
    }
}

/// Validates ruler positions against document constraints.
pub struct RulerPositionValidator {
    constraint: RulerConstraint,
}

impl RulerPositionValidator {
    pub fn new(constraint: RulerConstraint) -> Self {
        Self { constraint }
    }

    pub fn with_defaults() -> Self {
        Self { constraint: RulerConstraint::default() }
    }

    pub fn constraint(&self) -> &RulerConstraint {
        &self.constraint
    }

    /// Validate a set of ruler positions against the constraints.
    pub fn validate(&self, positions: &[u32]) -> Vec<RulerValidationError> {
        let mut errors = Vec::new();

        if positions.len() > self.constraint.max_rulers {
            errors.push(RulerValidationError::TooManyRulers {
                count: positions.len(),
                max: self.constraint.max_rulers,
            });
        }

        let mut seen = std::collections::HashSet::new();
        for &col in positions {
            if !seen.insert(col) {
                errors.push(RulerValidationError::DuplicateColumn(col));
            }
            if col < self.constraint.min_column || col > self.constraint.max_column {
                errors.push(RulerValidationError::OutOfRange {
                    column: col,
                    min: self.constraint.min_column,
                    max: self.constraint.max_column,
                });
            }
        }

        let mut sorted = positions.to_vec();
        sorted.sort();
        for w in sorted.windows(2) {
            if w[1] - w[0] < self.constraint.min_spacing {
                errors.push(RulerValidationError::TooClose {
                    col_a: w[0],
                    col_b: w[1],
                    min_spacing: self.constraint.min_spacing,
                });
            }
        }

        errors
    }

    /// Check if positions are valid (no errors).
    pub fn is_valid(&self, positions: &[u32]) -> bool {
        self.validate(positions).is_empty()
    }

    /// Snap a column to the nearest valid position.
    pub fn snap_to_valid(&self, column: u32) -> u32 {
        column.clamp(self.constraint.min_column, self.constraint.max_column)
    }

    /// Filter out invalid positions and return only valid ones.
    pub fn filter_valid(&self, positions: &[u32]) -> Vec<u32> {
        let mut result: Vec<u32> = positions
            .iter()
            .copied()
            .filter(|&c| c >= self.constraint.min_column && c <= self.constraint.max_column)
            .collect();
        result.sort();
        result.dedup();
        // Enforce spacing
        let mut filtered = Vec::new();
        for col in result {
            if filtered.last().map_or(true, |&last: &u32| col - last >= self.constraint.min_spacing) {
                filtered.push(col);
            }
        }
        filtered.truncate(self.constraint.max_rulers);
        filtered
    }
}

impl std::fmt::Display for RulerPositionValidator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RulerPositionValidator({})", self.constraint)
    }
}


/// Ruler annotation overlay manager.
#[derive(Debug, Clone)]
pub struct RulerAnnotationManager {
    entries: Vec<RulerAnnotation>,
    enabled: bool,
    max_entries: usize,
}

/// A single ruler annotation.
#[derive(Debug, Clone, PartialEq)]
pub struct RulerAnnotation {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl RulerAnnotation {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self { self.priority = p; self }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string())); self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) { self.active = false; }
    pub fn activate(&mut self) { self.active = true; }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize { self.metadata.len() }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl RulerAnnotationManager {
    pub fn new(max_entries: usize) -> Self {
        Self { entries: Vec::new(), enabled: true, max_entries }
    }

    pub fn add(&mut self, entry: RulerAnnotation) -> bool {
        if self.entries.len() >= self.max_entries { return false; }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&RulerAnnotation> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut RulerAnnotation> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&RulerAnnotation> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
    pub fn is_full(&self) -> bool { self.entries.len() >= self.max_entries }
    pub fn enable(&mut self) { self.enabled = true; }
    pub fn disable(&mut self) { self.enabled = false; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&RulerAnnotation> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&RulerAnnotation> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries { e.active = false; }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries { e.active = true; }
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

    pub fn drain_inactive(&mut self) -> Vec<RulerAnnotation> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}



// ---------------------------------------------------------------------------
// rulers – Extended ruler overlay helpers
// ---------------------------------------------------------------------------

/// Priority levels for ruler overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZRulersPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZRulersPriority {
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
    pub fn all_asc() -> [ZRulersPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZRulersPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks ruler overlay data.
#[derive(Debug, Clone)]
pub struct ZRulersRulerOverlay {
    pub positions: Vec<(u32, String)>,
    pub opacity: f32,
    pub visible: bool,
}

impl ZRulersRulerOverlay {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            opacity: 0.0,
            visible: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.positions.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZRulersRulerOverlay[opacity={:?}, visible={:?}]", self.opacity, self.visible)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.visible = !c.visible;
        c
    }
}

/// Compute a simple rolling hash for ruler overlay.
pub fn z_rulers_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_rulers_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_rulers_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_rulers_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_rulers_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_rulers_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_rulers_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 83
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer83 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer83 {
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
pub fn xb_fnv1a_83(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_83<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_83<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_83(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_83(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 152
// ---------------------------------------------------------------------------

/// Generic object pool `Xc152Pool<T>`.
pub struct Xc152Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc152Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc152PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc152Pool<T> {
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
    pub fn stats(&self) -> Xc152PoolStats {
        Xc152PoolStats {
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

impl<T> Default for Xc152Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc152Scheduler`.
pub struct Xc152Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc152Scheduler {
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

impl Default for Xc152Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_152 hash for the given byte slice.
pub fn xc_152_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_152 convention.
pub fn xc_152_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe96 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe96Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe96PipelineError {
    pub stage: Xe96Stage,
    pub message: String,
}

impl std::fmt::Display for Xe96PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe96Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe96Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe96PipelineError>>>,
    stage_names: Vec<Xe96Stage>,
}

impl Xe96Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe96PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe96Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe96PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe96Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe96PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe96Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe96PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe96Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe96PipelineError> {
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

    pub fn compose(mut self, other: Xe96Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe96CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe96CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe96Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe96CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe96CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe96Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe96CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_96_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe96CacheEntry {
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

    fn xe_96_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe96CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_96_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe96PipelineError> {
    Ok(data)
}

pub fn xe_96_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe96PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_96_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe96PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_96_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe96PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_96_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe96PipelineError> {
    Err(Xe96PipelineError {
        stage: Xe96Stage::Parse,
        message: "intentional failure".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = RulersConfig::default();
        assert!(cfg.rulers.is_empty());
        assert_eq!(cfg.default_color, "#d3d3d3");
    }

    #[test]
    fn add_and_query_rulers() {
        let mut cfg = RulersConfig::default();
        cfg.add_ruler(80, None);
        cfg.add_ruler(120, Some("#ff0000".to_string()));
        assert_eq!(cfg.get_rulers().len(), 2);
        assert!(cfg.has_ruler_at(80));
        assert!(cfg.has_ruler_at(120));
        assert!(!cfg.has_ruler_at(100));
    }

    #[test]
    fn remove_ruler() {
        let mut cfg = RulersConfig::default();
        cfg.add_ruler(80, None);
        cfg.add_ruler(120, None);
        assert_eq!(cfg.remove_ruler(80), 1);
        assert!(!cfg.has_ruler_at(80));
        assert!(cfg.has_ruler_at(120));
        assert_eq!(cfg.remove_ruler(999), 0);
    }

    #[test]
    fn sorted_rulers_returns_ordered() {
        let mut cfg = RulersConfig::default();
        cfg.add_ruler(120, None);
        cfg.add_ruler(40, None);
        cfg.add_ruler(80, None);
        let sorted = cfg.sorted_rulers();
        assert_eq!(sorted[0].column, 40);
        assert_eq!(sorted[1].column, 80);
        assert_eq!(sorted[2].column, 120);
    }

    #[test]
    fn compute_positions_with_defaults() {
        let mut cfg = RulersConfig::default();
        cfg.add_ruler(80, None);
        cfg.add_ruler(120, Some("#ff0000".into()));
        let svc = RulerService::new(cfg);
        let positions = svc.compute_positions(8.0);
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0].column, 80);
        assert!((positions[0].x - 640.0).abs() < f64::EPSILON);
        assert_eq!(positions[0].color, "#d3d3d3");
        assert_eq!(positions[1].color, "#ff0000");
    }

    #[test]
    fn visible_rulers_filters_by_viewport() {
        let mut cfg = RulersConfig::default();
        cfg.add_ruler(40, None);
        cfg.add_ruler(80, None);
        cfg.add_ruler(120, None);
        let svc = RulerService::new(cfg);
        let visible = svc.visible_rulers(100);
        assert_eq!(visible.len(), 2);
        assert!(visible.iter().all(|r| r.column < 100));
    }

    #[test]
    fn render_rulers_produces_decorations() {
        let positions = vec![
            RulerPosition { column: 80, x: 640.0, color: "#aaa".into() },
            RulerPosition { column: 120, x: 960.0, color: "#bbb".into() },
        ];
        let decorations = render_rulers(&positions, 500.0, 1.0);
        assert_eq!(decorations.len(), 2);
        assert!((decorations[0].x - 640.0).abs() < f64::EPSILON);
        assert!((decorations[0].height - 500.0).abs() < f64::EPSILON);
        assert!((decorations[0].width - 1.0).abs() < f64::EPSILON);
        assert_eq!(decorations[1].color, "#bbb");
    }

    #[test]
    fn render_rulers_empty() {
        let decorations = render_rulers(&[], 500.0, 1.0);
        assert!(decorations.is_empty());
    }

    #[test]
    fn ruler_config_new_validates_column() {
        assert!(RulerConfig::new(80, None).is_ok());
        assert!(RulerConfig::new(MAX_COLUMN, None).is_ok());
        let err = RulerConfig::new(MAX_COLUMN + 1, None).unwrap_err();
        assert_eq!(err, RulerError::ColumnOutOfRange(MAX_COLUMN + 1));
    }

    #[test]
    fn ruler_config_new_validates_color() {
        assert!(RulerConfig::new(80, Some("#abc".into())).is_ok());
        assert!(RulerConfig::new(80, Some("#aabbcc".into())).is_ok());
        let err = RulerConfig::new(80, Some("red".into())).unwrap_err();
        assert_eq!(err, RulerError::InvalidColor("red".into()));
        let err = RulerConfig::new(80, Some("#zzzzzz".into())).unwrap_err();
        assert_eq!(err, RulerError::InvalidColor("#zzzzzz".into()));
    }

    #[test]
    fn effective_color_uses_override_or_default() {
        let with_color = RulerConfig { column: 80, color: Some("#ff0000".into()) };
        assert_eq!(with_color.effective_color("#d3d3d3"), "#ff0000");

        let without_color = RulerConfig { column: 80, color: None };
        assert_eq!(without_color.effective_color("#d3d3d3"), "#d3d3d3");
    }

    #[test]
    fn add_ruler_validated_rejects_duplicates() {
        let mut cfg = RulersConfig::default();
        assert!(cfg.add_ruler_validated(80, None).is_ok());
        let err = cfg.add_ruler_validated(80, None).unwrap_err();
        assert_eq!(err, RulerError::DuplicateColumn(80));
    }

    #[test]
    fn config_len_is_empty_clear() {
        let mut cfg = RulersConfig::default();
        assert!(cfg.is_empty());
        assert_eq!(cfg.len(), 0);
        cfg.add_ruler(80, None);
        cfg.add_ruler(120, None);
        assert!(!cfg.is_empty());
        assert_eq!(cfg.len(), 2);
        cfg.clear();
        assert!(cfg.is_empty());
    }

    #[test]
    fn column_range_and_columns() {
        let mut cfg = RulersConfig::default();
        assert_eq!(cfg.column_range(), None);
        cfg.add_ruler(120, None);
        cfg.add_ruler(40, None);
        cfg.add_ruler(80, None);
        assert_eq!(cfg.column_range(), Some((40, 120)));
        assert_eq!(cfg.columns(), vec![40, 80, 120]);
    }

    #[test]
    fn builder_pattern() {
        let cfg = RulersConfigBuilder::new()
            .default_color("#aabbcc")
            .unwrap()
            .ruler(80, None)
            .unwrap()
            .ruler(120, Some("#ff0000".into()))
            .unwrap()
            .build();
        assert_eq!(cfg.len(), 2);
        assert_eq!(cfg.default_color, "#aabbcc");
    }

    #[test]
    fn builder_rejects_invalid_default_color() {
        let err = RulersConfigBuilder::new().default_color("bad").unwrap_err();
        assert_eq!(err, RulerError::InvalidColor("bad".into()));
    }

    #[test]
    fn builder_rejects_duplicate_ruler() {
        let err = RulersConfigBuilder::new()
            .ruler(80, None)
            .unwrap()
            .ruler(80, None)
            .unwrap_err();
        assert_eq!(err, RulerError::DuplicateColumn(80));
    }

    #[test]
    fn compute_positions_checked_validates() {
        let svc = RulerService::new(RulersConfig::default());
        assert!(svc.compute_positions_checked(8.0).is_ok());
        assert!(svc.compute_positions_checked(0.0).is_err());
        assert!(svc.compute_positions_checked(-1.0).is_err());
        assert!(svc.compute_positions_checked(f64::NAN).is_err());
    }

    #[test]
    fn compute_visible_positions_filters() {
        let mut cfg = RulersConfig::default();
        cfg.add_ruler(40, None);
        cfg.add_ruler(80, None);
        cfg.add_ruler(120, None);
        let svc = RulerService::new(cfg);
        let positions = svc.compute_visible_positions(8.0, 100);
        assert_eq!(positions.len(), 2);
        assert!(positions.iter().all(|p| p.column < 100));
    }

    #[test]
    fn render_rulers_checked_validates() {
        let positions = vec![RulerPosition {
            column: 80,
            x: 640.0,
            color: "#aaa".into(),
        }];
        assert!(render_rulers_checked(&positions, 500.0, 1.0).is_ok());
        assert!(render_rulers_checked(&positions, 0.0, 1.0).is_err());
        assert!(render_rulers_checked(&positions, 500.0, -1.0).is_err());
    }

    #[test]
    fn display_impls() {
        let rc = RulerConfig { column: 80, color: Some("#ff0000".into()) };
        assert_eq!(format!("{rc}"), "ruler@80 (#ff0000)");

        let rc_no_color = RulerConfig { column: 120, color: None };
        assert_eq!(format!("{rc_no_color}"), "ruler@120");

        let cfg = RulersConfig::default();
        assert_eq!(format!("{cfg}"), "RulersConfig(0 rulers, default=#d3d3d3)");

        let svc = RulerService::new(RulersConfig::default());
        assert!(format!("{svc}").contains("RulerService"));

        let pos = RulerPosition { column: 80, x: 640.0, color: "#aaa".into() };
        assert!(format!("{pos}").contains("col 80"));

        let dec = RulerDecoration { x: 640.0, height: 500.0, color: "#aaa".into(), width: 1.0 };
        assert!(format!("{dec}").contains("decoration"));
    }

    #[test]
    fn error_display() {
        let e = RulerError::ColumnOutOfRange(20000);
        assert!(format!("{e}").contains("20000"));

        let e = RulerError::DuplicateColumn(80);
        assert!(format!("{e}").contains("80"));

        let e = RulerError::InvalidColor("bad".into());
        assert!(format!("{e}").contains("bad"));
    }

    #[test]
    fn ruler_service_ruler_count() {
        let mut cfg = RulersConfig::default();
        cfg.add_ruler(80, None);
        cfg.add_ruler(120, None);
        let svc = RulerService::new(cfg);
        assert_eq!(svc.ruler_count(), 2);
    }

    #[test]
    fn rulers_stats_new_defaults() {
        let stats = RulersStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn rulers_stats_record_success() {
        let mut stats = RulersStats::new();
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
    fn rulers_stats_record_failure() {
        let mut stats = RulersStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn rulers_stats_reset() {
        let mut stats = RulersStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn rulers_stats_merge() {
        let mut a = RulersStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = RulersStats::new();
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
    fn rulers_stats_display() {
        let mut stats = RulersStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn rulers_stats_default() {
        let stats = RulersStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn rulers_validator_accepts_valid_name() {
        let v = RulersValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn rulers_validator_rejects_empty() {
        let v = RulersValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn rulers_validator_rejects_too_long() {
        let v = RulersValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn rulers_validator_forbidden_prefix() {
        let v = RulersValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn rulers_validator_allowed_chars() {
        let v = RulersValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn rulers_validator_range() {
        let v = RulersValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn rulers_sanitize_removes_control() {
        let result = RulersValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn rulers_truncate_short_string() {
        assert_eq!(RulersValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn rulers_truncate_long_string() {
        let result = RulersValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn rulers_is_ascii_printable() {
        assert!(RulersValidator::is_ascii_printable("Hello World 123"));
        assert!(!RulersValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn ruler_configuration_add_positions() {
        let mut rc = RulerConfiguration::new("default");
        rc.add_position(80).unwrap();
        rc.add_position(120).unwrap();
        assert_eq!(rc.position_count(), 2);
        assert_eq!(rc.positions, vec![80, 120]);
    }

    #[test]
    fn ruler_configuration_dedup() {
        let mut rc = RulerConfiguration::new("test");
        rc.add_position(80).unwrap();
        rc.add_position(80).unwrap();
        assert_eq!(rc.position_count(), 1);
    }

    #[test]
    fn ruler_configuration_out_of_range() {
        let mut rc = RulerConfiguration::new("test");
        assert!(rc.add_position(MAX_COLUMN + 1).is_err());
    }

    #[test]
    fn ruler_configuration_set_color() {
        let mut rc = RulerConfiguration::new("test");
        rc.add_position(80).unwrap();
        rc.set_color(80, "#ff0000").unwrap();
        let config = rc.to_rulers_config();
        assert_eq!(config.rulers[0].color, Some("#ff0000".to_string()));
    }

    #[test]
    fn ruler_render_positions_basic() {
        let mut config = RulersConfig::default();
        config.add_ruler(80, Some("#ff0000".into()));
        config.add_ruler(120, None);
        let positions = ruler_render_positions(&config, 8.0, 0.0).unwrap();
        assert_eq!(positions.len(), 2);
        assert!((positions[0].pixel_x - 640.0).abs() < 0.001);
        assert_eq!(positions[0].color, "#ff0000");
        assert!((positions[1].pixel_x - 960.0).abs() < 0.001);
    }

    #[test]
    fn ruler_render_positions_invalid_char_width() {
        let config = RulersConfig::default();
        assert!(ruler_render_positions(&config, 0.0, 0.0).is_err());
        assert!(ruler_render_positions(&config, -1.0, 0.0).is_err());
    }

    #[test]
    fn ruler_color_by_position_with_override() {
        let mut config = RulersConfig::default();
        config.add_ruler(80, Some("#ff0000".into()));
        config.add_ruler(120, None);
        assert_eq!(ruler_color_by_position(&config, 80), "#ff0000");
        assert_eq!(ruler_color_by_position(&config, 120), "#d3d3d3");
    }

    #[test]
    fn rulers_in_range_basic() {
        let mut config = RulersConfig::default();
        config.add_ruler(40, None);
        config.add_ruler(80, None);
        config.add_ruler(120, None);
        let visible = rulers_in_range(&config, 50, 100);
        assert_eq!(visible, vec![80]);
    }

    #[test]
    fn test_ruler_column_iter_sorted_dedup() {
        let iter = RulerColumnIter::new(&[80, 40, 80, 120, 40]);
        let cols: Vec<u32> = iter.collect();
        assert_eq!(cols, vec![40, 80, 120]);
    }

    #[test]
    fn test_ruler_column_iter_empty() {
        let iter = RulerColumnIter::new(&[]);
        assert_eq!(iter.remaining(), 0);
        assert_eq!(iter.collect::<Vec<u32>>(), Vec::<u32>::new());
    }

    #[test]
    fn test_ruler_column_iter_exact_size() {
        let iter = RulerColumnIter::new(&[10, 20, 30]);
        assert_eq!(iter.len(), 3);
    }

    #[test]
    fn test_ruler_span_basics() {
        let span = RulerSpan { start: 10, end: 80 };
        assert_eq!(span.width(), 70);
        assert!(span.contains(10));
        assert!(span.contains(50));
        assert!(!span.contains(80));
        assert_eq!(format!("{span}"), "[10..80)");
    }

    #[test]
    fn test_ruler_spans_computation() {
        let spans = ruler_spans(&[80, 40, 120]);
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0], RulerSpan { start: 0, end: 40 });
        assert_eq!(spans[1], RulerSpan { start: 40, end: 80 });
        assert_eq!(spans[2], RulerSpan { start: 80, end: 120 });
    }

    #[test]
    fn test_ruler_span_from_impls() {
        let span: RulerSpan = 80u32.into();
        assert_eq!(span, RulerSpan { start: 0, end: 80 });
        let span2: RulerSpan = (10u32, 50u32).into();
        assert_eq!(span2, RulerSpan { start: 10, end: 50 });
    }

    #[test]
    fn test_validate_ruler_columns_ok() {
        let result = validate_ruler_columns(&[80, 40, 120]);
        assert_eq!(result.unwrap(), vec![40, 80, 120]);
    }

    #[test]
    fn test_validate_ruler_columns_out_of_range() {
        let result = validate_ruler_columns(&[80, 20_000]);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_ruler_columns_duplicate() {
        let result = validate_ruler_columns(&[80, 80]);
        assert!(result.is_err());
    }

    #[test]
    fn test_nearest_ruler() {
        assert_eq!(nearest_ruler(&[40, 80, 120], 50), Some(40));
        assert_eq!(nearest_ruler(&[40, 80, 120], 75), Some(80));
        assert_eq!(nearest_ruler(&[], 50), None);
    }

    #[test]
    fn test_ruler_distribution() {
        let dist = ruler_distribution(&[40, 80, 120]).unwrap();
        assert_eq!(dist.0, 40);
        assert_eq!(dist.1, 120);
        assert!((dist.2 - 80.0).abs() < f64::EPSILON);
        assert!(ruler_distribution(&[]).is_none());
    }

    #[test]
    fn test_merge_ruler_columns() {
        let merged = merge_ruler_columns(&[80, 40], &[120, 40]);
        assert_eq!(merged, vec![40, 80, 120]);
    }

    // --- new tests ---

    #[test]
    fn ruler_set_add_remove_contains() {
        let mut set = RulerSet::new();
        set.add(RulerConfig::new(80, None).unwrap()).unwrap();
        set.add(RulerConfig::new(120, None).unwrap()).unwrap();
        assert_eq!(set.len(), 2);
        assert!(set.contains(80));
        assert!(!set.contains(100));
        assert!(set.add(RulerConfig::new(80, None).unwrap()).is_err());
        assert_eq!(set.columns(), vec![80, 120]);
        assert!(set.remove(80));
        assert!(!set.contains(80));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn ruler_set_display() {
        let mut set = RulerSet::new();
        set.add(RulerConfig::new(40, None).unwrap()).unwrap();
        set.add(RulerConfig::new(80, None).unwrap()).unwrap();
        let s = format!("{set}");
        assert!(s.contains("40"));
        assert!(s.contains("80"));
    }

    #[test]
    fn ruler_preset_columns() {
        assert_eq!(RulerPreset::PEP8.to_columns(), vec![79, 120]);
        assert_eq!(RulerPreset::StandardRust.to_columns(), vec![100]);
        assert_eq!(RulerPreset::CustomWidth(72).to_columns(), vec![72]);
    }

    #[test]
    fn ruler_overlap_checker() {
        let overlaps = RulerOverlapChecker::overlapping(&[40, 79, 80, 120], 8);
        assert!(overlaps.contains(&40));
        assert!(overlaps.contains(&80));
        assert!(overlaps.contains(&120));
        assert!(!overlaps.contains(&79));
        assert!(RulerOverlapChecker::overlapping(&[10], 0).is_empty());
    }

    #[test]
    fn ruler_visibility_compute() {
        let vis = RulerVisibility::compute(&[40, 80, 120, 200], 50, 130);
        assert_eq!(vis.visible, vec![80, 120]);
        assert_eq!(vis.count(), 2);
    }

    #[test]
    fn ruler_preset_display() {
        assert!(format!("{}", RulerPreset::PEP8).contains("79"));
        assert!(format!("{}", RulerPreset::StandardRust).contains("100"));
    }

    // --- new tests ---

    #[test]
    fn ruler_guide_creation_and_display() {
        let guide = RulerGuide::new(80, "PEP8 line limit", GuideSeverity::Hard).unwrap();
        assert_eq!(guide.column, 80);
        assert!(guide.is_hard_limit());
        assert!(guide.exceeds(81));
        assert!(!guide.exceeds(80));
        let s = format!("{}", guide);
        assert!(s.contains("PEP8 line limit"));
        assert!(s.contains("hard"));
    }

    #[test]
    fn ruler_guide_out_of_range() {
        let err = RulerGuide::new(MAX_COLUMN + 1, "too far", GuideSeverity::Info);
        assert!(err.is_err());
    }

    #[test]
    fn ruler_guide_set_operations() {
        let mut set = RulerGuideSet::new();
        set.add(RulerGuide::new(80, "soft", GuideSeverity::Soft).unwrap()).unwrap();
        set.add(RulerGuide::new(120, "hard", GuideSeverity::Hard).unwrap()).unwrap();
        set.add(RulerGuide::new(40, "info", GuideSeverity::Info).unwrap()).unwrap();
        assert_eq!(set.len(), 3);
        assert_eq!(set.hard_limits().len(), 1);
        assert_eq!(set.hard_limits()[0].column, 120);
        // Duplicate rejection
        assert!(set.add(RulerGuide::new(80, "dup", GuideSeverity::Info).unwrap()).is_err());
    }

    #[test]
    fn ruler_guide_exceeded_by() {
        let mut set = RulerGuideSet::new();
        set.add(RulerGuide::new(80, "soft", GuideSeverity::Soft).unwrap()).unwrap();
        set.add(RulerGuide::new(120, "hard", GuideSeverity::Hard).unwrap()).unwrap();
        let exceeded = set.exceeded_by(100);
        assert_eq!(exceeded.len(), 1);
        assert_eq!(exceeded[0].column, 80);
        let exceeded_all = set.exceeded_by(200);
        assert_eq!(exceeded_all.len(), 2);
    }

    #[test]
    fn ruler_guide_set_to_rulers_config() {
        let mut set = RulerGuideSet::new();
        set.add(RulerGuide::new(79, "pep8", GuideSeverity::Hard).unwrap()).unwrap();
        set.add(RulerGuide::new(100, "rust", GuideSeverity::Soft).unwrap()).unwrap();
        let cfg = set.to_rulers_config("#aaa");
        assert_eq!(cfg.rulers.len(), 2);
        assert_eq!(cfg.default_color, "#aaa");
        assert_eq!(cfg.rulers[0].column, 79);
        assert_eq!(cfg.rulers[1].column, 100);
    }

    #[test]
    fn guide_severity_display() {
        assert_eq!(format!("{}", GuideSeverity::Info), "info");
        assert_eq!(format!("{}", GuideSeverity::Soft), "soft");
        assert_eq!(format!("{}", GuideSeverity::Hard), "hard");
    }

    // -- RulerConfigParser tests --

    #[test]
    fn parser_basic() {
        let cfg = RulerConfigParser::parse("80,120", "#aaa").unwrap();
        assert_eq!(cfg.len(), 2);
        assert!(cfg.has_ruler_at(80));
        assert!(cfg.has_ruler_at(120));
    }

    #[test]
    fn parser_with_colors() {
        let cfg = RulerConfigParser::parse("80:#ff0000,120:#00ff00", "#aaa").unwrap();
        assert_eq!(cfg.rulers[0].color.as_deref(), Some("#ff0000"));
        assert_eq!(cfg.rulers[1].color.as_deref(), Some("#00ff00"));
    }

    #[test]
    fn parser_empty_input() {
        let cfg = RulerConfigParser::parse("", "#aaa").unwrap();
        assert!(cfg.is_empty());
    }

    #[test]
    fn parser_duplicate_column_error() {
        let result = RulerConfigParser::parse("80,80", "#aaa");
        assert!(result.is_err());
    }

    #[test]
    fn parser_json_array() {
        let entries = vec![(80, None), (120, Some("#ff0000"))];
        let cfg = RulerConfigParser::parse_json_array(&entries).unwrap();
        assert_eq!(cfg.len(), 2);
    }

    // -- MultiRulerRenderer tests --

    #[test]
    fn renderer_default() {
        let r = MultiRulerRenderer::default();
        assert_eq!(r.ruler_char, '│');
        assert!(r.clip_to_viewport);
        assert_eq!(r.viewport_width, 120);
    }

    #[test]
    fn renderer_render_line() {
        let mut cfg = RulersConfig::default();
        cfg.add_ruler(5, None);
        cfg.add_ruler(10, None);
        let r = MultiRulerRenderer::new(20);
        let line = r.render_line(&cfg);
        assert_eq!(line.chars().nth(5), Some('│'));
        assert_eq!(line.chars().nth(10), Some('│'));
        assert_eq!(line.chars().nth(0), Some(' '));
    }

    #[test]
    fn renderer_visible_columns() {
        let mut cfg = RulersConfig::default();
        cfg.add_ruler(5, None);
        cfg.add_ruler(200, None);
        let r = MultiRulerRenderer::new(100);
        let cols = r.visible_columns(&cfg);
        assert_eq!(cols, vec![5]);
    }

    // -- RulerColor tests --

    #[test]
    fn ruler_color_valid_hex() {
        assert!(RulerColor::new("#ff0000").is_valid_hex());
        assert!(RulerColor::new("#abc").is_valid_hex());
        assert!(!RulerColor::new("not-a-color").is_valid_hex());
    }

    #[test]
    fn ruler_color_to_rgb() {
        let c = RulerColor::new("#ff8000");
        assert_eq!(c.to_rgb(), Some((255, 128, 0)));
    }

    #[test]
    fn ruler_color_short_hex() {
        let c = RulerColor::new("#f00");
        assert_eq!(c.to_rgb(), Some((255, 0, 0)));
    }

    #[test]
    fn ruler_color_opacity() {
        let c = RulerColor::with_opacity("#fff", 0.5);
        assert!((c.opacity - 0.5).abs() < f64::EPSILON);
        assert_eq!(format!("{}", c), "#fff@50%");
    }

    #[test]
    fn ruler_color_blend() {
        let c1 = RulerColor::new("#000000");
        let c2 = RulerColor::new("#ffffff");
        let blended = c1.blend(&c2, 0.5).unwrap();
        let (r, g, b) = blended.to_rgb().unwrap();
        // Should be roughly middle gray
        assert!((r as i16 - 127).abs() <= 1);
        assert!((g as i16 - 127).abs() <= 1);
        assert!((b as i16 - 127).abs() <= 1);
    }

    // -- WordWrapGuideRuler tests --

    #[test]
    fn wrap_guide_default() {
        let g = WordWrapGuideRuler::default();
        assert_eq!(g.wrap_column, 80);
        assert!(g.active);
        assert_eq!(g.style, WrapGuideStyle::Solid);
    }

    #[test]
    fn wrap_guide_exceeds() {
        let g = WordWrapGuideRuler::new(80);
        assert!(!g.exceeds_wrap(80));
        assert!(g.exceeds_wrap(81));
        assert_eq!(g.overflow_amount(90), 10);
        assert_eq!(g.overflow_amount(50), 0);
    }

    #[test]
    fn wrap_guide_char() {
        assert_eq!(WordWrapGuideRuler { style: WrapGuideStyle::Dashed, ..Default::default() }.guide_char(), '┆');
        assert_eq!(WordWrapGuideRuler { style: WrapGuideStyle::Dotted, ..Default::default() }.guide_char(), '┊');
    }

    #[test]
    fn wrap_guide_to_ruler_config() {
        let g = WordWrapGuideRuler::new(100);
        let rc = g.to_ruler_config().unwrap();
        assert_eq!(rc.column, 100);
    }

    #[test]
    fn wrap_guide_style_display() {
        assert_eq!(format!("{}", WrapGuideStyle::Solid), "solid");
        assert_eq!(format!("{}", WrapGuideStyle::Dashed), "dashed");
        assert_eq!(format!("{}", WrapGuideStyle::Dotted), "dotted");
    }

    #[test]
    fn editor_add_ruler() {
        let mut editor = RulerInteractiveEditor::new(10);
        editor.add(80).unwrap();
        assert_eq!(editor.positions(), &[80]);
        assert_eq!(editor.count(), 1);
    }

    #[test]
    fn editor_add_duplicate_error() {
        let mut editor = RulerInteractiveEditor::new(10);
        editor.add(80).unwrap();
        assert!(editor.add(80).is_err());
    }

    #[test]
    fn editor_add_max_reached() {
        let mut editor = RulerInteractiveEditor::new(1);
        editor.add(80).unwrap();
        assert!(editor.add(120).is_err());
    }

    #[test]
    fn editor_remove_ruler() {
        let mut editor = RulerInteractiveEditor::new(10);
        editor.add(80).unwrap();
        editor.remove(80).unwrap();
        assert_eq!(editor.count(), 0);
    }

    #[test]
    fn editor_remove_nonexistent() {
        let mut editor = RulerInteractiveEditor::new(10);
        assert!(editor.remove(999).is_err());
    }

    #[test]
    fn editor_move_ruler() {
        let mut editor = RulerInteractiveEditor::new(10);
        editor.add(80).unwrap();
        editor.move_ruler(80, 100).unwrap();
        assert_eq!(editor.positions(), &[100]);
    }

    #[test]
    fn editor_undo_add() {
        let mut editor = RulerInteractiveEditor::new(10);
        editor.add(80).unwrap();
        let action = editor.undo().unwrap();
        assert_eq!(action, RulerEditAction::Add(80));
        assert_eq!(editor.count(), 0);
    }

    #[test]
    fn editor_undo_remove() {
        let mut editor = RulerInteractiveEditor::new(10);
        editor.add(80).unwrap();
        editor.remove(80).unwrap();
        editor.undo();
        assert_eq!(editor.positions(), &[80]);
    }

    #[test]
    fn editor_redo() {
        let mut editor = RulerInteractiveEditor::new(10);
        editor.add(80).unwrap();
        editor.undo();
        assert!(editor.can_redo());
        editor.redo();
        assert_eq!(editor.positions(), &[80]);
    }

    #[test]
    fn editor_summary_and_display() {
        let mut editor = RulerInteractiveEditor::new(10);
        editor.add(80).unwrap();
        editor.add(120).unwrap();
        let s = editor.summary();
        assert!(s.contains("80"));
        assert!(s.contains("120"));
        assert!(format!("{editor}").contains("2 rulers"));
    }

    #[test]
    fn editor_clear() {
        let mut editor = RulerInteractiveEditor::new(10);
        editor.add(80).unwrap();
        editor.clear();
        assert_eq!(editor.count(), 0);
        assert!(!editor.can_undo());
    }

    #[test]
    fn editor_positions_sorted() {
        let mut editor = RulerInteractiveEditor::new(10);
        editor.add(120).unwrap();
        editor.add(80).unwrap();
        editor.add(40).unwrap();
        assert_eq!(editor.positions(), &[40, 80, 120]);
    }

    #[test]
    fn edit_action_display() {
        assert_eq!(format!("{}", RulerEditAction::Add(80)), "+80");
        assert_eq!(format!("{}", RulerEditAction::Remove(80)), "-80");
        assert_eq!(format!("{}", RulerEditAction::Move { from: 80, to: 100 }), "80->100");
    }

    #[test]
    fn validator_valid_positions() {
        let v = RulerPositionValidator::with_defaults();
        assert!(v.is_valid(&[40, 80, 120]));
    }

    #[test]
    fn validator_out_of_range() {
        let v = RulerPositionValidator::new(RulerConstraint::new(1, 100, 1, 10));
        let errors = v.validate(&[200]);
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], RulerValidationError::OutOfRange { .. }));
    }

    #[test]
    fn validator_too_many() {
        let v = RulerPositionValidator::new(RulerConstraint::new(1, 320, 1, 2));
        let errors = v.validate(&[10, 20, 30]);
        assert!(errors.iter().any(|e| matches!(e, RulerValidationError::TooManyRulers { .. })));
    }

    #[test]
    fn validator_too_close() {
        let v = RulerPositionValidator::new(RulerConstraint::new(1, 320, 10, 20));
        let errors = v.validate(&[80, 85]);
        assert!(errors.iter().any(|e| matches!(e, RulerValidationError::TooClose { .. })));
    }

    #[test]
    fn validator_duplicate() {
        let v = RulerPositionValidator::with_defaults();
        let errors = v.validate(&[80, 80]);
        assert!(errors.iter().any(|e| matches!(e, RulerValidationError::DuplicateColumn(80))));
    }

    #[test]
    fn validator_snap_to_valid() {
        let v = RulerPositionValidator::new(RulerConstraint::new(10, 200, 1, 10));
        assert_eq!(v.snap_to_valid(5), 10);
        assert_eq!(v.snap_to_valid(300), 200);
        assert_eq!(v.snap_to_valid(100), 100);
    }

    #[test]
    fn validator_filter_valid() {
        let v = RulerPositionValidator::new(RulerConstraint::new(10, 200, 10, 5));
        let result = v.filter_valid(&[5, 10, 15, 20, 200, 300]);
        assert!(result.iter().all(|&c| c >= 10 && c <= 200));
        for w in result.windows(2) {
            assert!(w[1] - w[0] >= 10);
        }
    }

    #[test]
    fn validator_display() {
        let v = RulerPositionValidator::with_defaults();
        let s = format!("{v}");
        assert!(s.contains("RulerPositionValidator"));
    }

    #[test]
    fn validation_error_display() {
        let e = RulerValidationError::OutOfRange { column: 500, min: 1, max: 320 };
        assert!(format!("{e}").contains("500"));
        let e2 = RulerValidationError::TooClose { col_a: 10, col_b: 12, min_spacing: 5 };
        assert!(format!("{e2}").contains("too close"));
    }

    #[test]
    fn constraint_display() {
        let c = RulerConstraint::default();
        let s = format!("{c}");
        assert!(s.contains("1-320"));
    }

    #[test]
    fn ruler_annotation_creation() {
        let e = RulerAnnotation::new("r1", "80-col");
        assert_eq!(e.id, "r1");
        assert!(e.active);
    }

    #[test]
    fn ruler_annotation_priority() {
        let e = RulerAnnotation::new("r1", "R").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn ruler_annotation_metadata() {
        let e = RulerAnnotation::new("r1", "R").with_meta("col", "80");
        assert_eq!(e.get_meta("col"), Some("80"));
        assert!(e.has_meta("col"));
    }

    #[test]
    fn ruler_annotation_remove_meta() {
        let mut e = RulerAnnotation::new("r1", "R").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn ruler_annotation_activate_deactivate() {
        let mut e = RulerAnnotation::new("r1", "R");
        e.deactivate(); assert!(!e.active);
        e.activate(); assert!(e.active);
    }

    #[test]
    fn ruler_annotation_mgr_add_sorted() {
        let mut m = RulerAnnotationManager::new(10);
        m.add(RulerAnnotation::new("lo", "Lo").with_priority(1));
        m.add(RulerAnnotation::new("hi", "Hi").with_priority(10));
        assert_eq!(m.ids()[0], "hi");
    }

    #[test]
    fn ruler_annotation_mgr_capacity() {
        let mut m = RulerAnnotationManager::new(1);
        assert!(m.add(RulerAnnotation::new("a", "A")));
        assert!(!m.add(RulerAnnotation::new("b", "B")));
    }

    #[test]
    fn ruler_annotation_mgr_remove() {
        let mut m = RulerAnnotationManager::new(10);
        m.add(RulerAnnotation::new("a", "A"));
        assert!(m.remove("a"));
        assert!(m.is_empty());
    }

    #[test]
    fn ruler_annotation_mgr_active() {
        let mut m = RulerAnnotationManager::new(10);
        m.add(RulerAnnotation::new("a", "A"));
        m.add(RulerAnnotation::new("b", "B"));
        m.get_mut("a").unwrap().deactivate();
        assert_eq!(m.count_active(), 1);
    }

    #[test]
    fn ruler_annotation_mgr_enable_disable() {
        let mut m = RulerAnnotationManager::new(10);
        m.disable(); assert!(!m.is_enabled());
        m.enable(); assert!(m.is_enabled());
    }

    #[test]
    fn ruler_annotation_mgr_find_label() {
        let mut m = RulerAnnotationManager::new(10);
        m.add(RulerAnnotation::new("a", "Alpha"));
        assert_eq!(m.find_by_label("Alpha").unwrap().id, "a");
    }

    #[test]
    fn ruler_annotation_mgr_drain_inactive() {
        let mut m = RulerAnnotationManager::new(10);
        m.add(RulerAnnotation::new("a", "A"));
        m.add(RulerAnnotation::new("b", "B"));
        m.get_mut("a").unwrap().deactivate();
        let d = m.drain_inactive();
        assert_eq!(d.len(), 1);
        assert_eq!(m.len(), 1);
    }


    // -- rulers Z-extended tests -----------------------------------------------

    #[test]
    fn z_rulers_priority_weight() {
        assert_eq!(ZRulersPriority::Idle.weight(), 0);
        assert_eq!(ZRulersPriority::Normal.weight(), 2);
        assert_eq!(ZRulersPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_rulers_priority_label() {
        assert_eq!(ZRulersPriority::Low.label(), "low");
        assert_eq!(ZRulersPriority::High.label(), "high");
    }

    #[test]
    fn z_rulers_priority_is_elevated() {
        assert!(!ZRulersPriority::Normal.is_elevated());
        assert!(ZRulersPriority::High.is_elevated());
        assert!(ZRulersPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_rulers_priority_display() {
        assert_eq!(format!("{}", ZRulersPriority::Idle), "idle");
    }

    #[test]
    fn z_rulers_priority_all_asc() {
        let all = ZRulersPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZRulersPriority::Idle);
        assert_eq!(all[4], ZRulersPriority::Realtime);
    }

    #[test]
    fn z_rulers_struct_new() {
        let s = ZRulersRulerOverlay::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_rulers_struct_toggled_clone() {
        let s = ZRulersRulerOverlay::new();
        let t = s.toggled_clone();
        assert_ne!(s.visible, t.visible);
    }

    #[test]
    fn z_rulers_rolling_hash_deterministic() {
        let h1 = z_rulers_rolling_hash(b"test");
        let h2 = z_rulers_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_rulers_rolling_hash(b"a"), z_rulers_rolling_hash(b"b"));
    }

    #[test]
    fn z_rulers_pad_to_basic() {
        assert_eq!(z_rulers_pad_to("hi", 5), "hi   ");
        assert_eq!(z_rulers_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_rulers_is_identifier_basic() {
        assert!(z_rulers_is_identifier("foo_bar"));
        assert!(z_rulers_is_identifier("abc123"));
        assert!(!z_rulers_is_identifier(""));
        assert!(!z_rulers_is_identifier("has space"));
    }

    #[test]
    fn z_rulers_levenshtein_basic() {
        assert_eq!(z_rulers_levenshtein("", ""), 0);
        assert_eq!(z_rulers_levenshtein("abc", "abc"), 0);
        assert_eq!(z_rulers_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_rulers_unique_words_basic() {
        let w = z_rulers_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_rulers_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_rulers_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_rulers_common_prefix_basic() {
        assert_eq!(z_rulers_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_rulers_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_rulers_struct_clear() {
        let mut s = ZRulersRulerOverlay::new();
        s.positions.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_rulers_rolling_hash_empty() {
        let h = z_rulers_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    #[test]
    fn xb_ring_buffer_83_push_and_len() {
        let mut rb = super::XbRingBuffer83::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_83_overwrite() {
        let mut rb = super::XbRingBuffer83::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_83_get_out_of_bounds() {
        let rb = super::XbRingBuffer83::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_83_drain_all() {
        let mut rb = super::XbRingBuffer83::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_83_peek_front_back() {
        let mut rb = super::XbRingBuffer83::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_83_clear() {
        let mut rb = super::XbRingBuffer83::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_83_capacity() {
        let rb = super::XbRingBuffer83::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_83_basic() {
        let h = super::xb_fnv1a_83(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_83(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_83_different_inputs() {
        let h1 = super::xb_fnv1a_83(b"abc");
        let h2 = super::xb_fnv1a_83(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_83_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_83(&data);
        let dec = super::xb_rle_decode_83(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_83_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_83(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_83(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_83_values() {
        assert!((super::xb_clamp_83(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_83(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_83(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_83_values() {
        assert!((super::xb_lerp_83(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_83(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_83(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_83_wrap_around_twice() {
        let mut rb = super::XbRingBuffer83::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 152 ----

    #[test]
    fn xc_152_pool_new_empty() {
        let pool: super::Xc152Pool<i32> = super::Xc152Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_152_pool_release_acquire() {
        let mut pool = super::Xc152Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_152_pool_acquire_empty() {
        let mut pool: super::Xc152Pool<i32> = super::Xc152Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_152_pool_full() {
        let mut pool = super::Xc152Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_152_pool_drain() {
        let mut pool = super::Xc152Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_152_pool_stats() {
        let mut pool = super::Xc152Pool::new(8);
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
    fn xc_152_pool_clear() {
        let mut pool = super::Xc152Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_152_pool_shrink() {
        let mut pool = super::Xc152Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_152_pool_default() {
        let pool: super::Xc152Pool<String> = super::Xc152Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_152_pool_extend() {
        let mut pool = super::Xc152Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_152_pool_retain() {
        let mut pool = super::Xc152Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_152_scheduler_round_robin() {
        let mut sched = super::Xc152Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_152_scheduler_empty() {
        let mut sched = super::Xc152Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_152_scheduler_reset() {
        let mut sched = super::Xc152Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_152_scheduler_add_remove() {
        let mut sched = super::Xc152Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_152_scheduler_targets() {
        let sched = super::Xc152Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_152_hash_empty() {
        assert_eq!(super::xc_152_hash(b""), 5381);
    }

    #[test]
    fn xc_152_hash_data() {
        let h = super::xc_152_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_152_hash(b"hello"), h);
    }

    #[test]
    fn xc_152_reverse_str() {
        assert_eq!(super::xc_152_reverse("abc"), "cba");
        assert_eq!(super::xc_152_reverse(""), "");
    }


    #[test]
    fn xe_96_pipeline_empty() {
        let p = super::Xe96Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_96_pipeline_parse_stage() {
        let p = super::Xe96Pipeline::new()
            .add_parse(super::xe_96_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_96_pipeline_transform_double() {
        let p = super::Xe96Pipeline::new()
            .add_transform(super::xe_96_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_96_pipeline_validate_reverse() {
        let p = super::Xe96Pipeline::new()
            .add_validate(super::xe_96_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_96_pipeline_emit_filter() {
        let p = super::Xe96Pipeline::new()
            .add_emit(super::xe_96_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_96_pipeline_multi_stage() {
        let p = super::Xe96Pipeline::new()
            .add_parse(super::xe_96_pipeline_identity)
            .add_transform(super::xe_96_pipeline_double)
            .add_validate(super::xe_96_pipeline_reverse)
            .add_emit(super::xe_96_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_96_pipeline_error_propagation() {
        let p = super::Xe96Pipeline::new()
            .add_parse(super::xe_96_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe96Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_96_pipeline_compose() {
        let p1 = super::Xe96Pipeline::new()
            .add_parse(super::xe_96_pipeline_identity);
        let p2 = super::Xe96Pipeline::new()
            .add_transform(super::xe_96_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_96_pipeline_error_display() {
        let e = super::Xe96PipelineError {
            stage: super::Xe96Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_96_cache_put_get() {
        let mut c = super::Xe96Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_96_cache_miss() {
        let mut c: super::Xe96Cache<&str, i32> = super::Xe96Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_96_cache_ttl_expiry() {
        let mut c = super::Xe96Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_96_cache_evict() {
        let mut c = super::Xe96Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_96_cache_capacity() {
        let mut c = super::Xe96Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_96_cache_stats() {
        let mut c = super::Xe96Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_96_cache_clear() {
        let mut c = super::Xe96Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

}
