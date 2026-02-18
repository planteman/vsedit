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


// ---------------------------------------------------------------------------
// xg_94: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg94Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg94Graph {
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

impl Default for Xg94Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_94: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg94Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg94Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg94Heap<T>) {
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

impl<T: Ord> Default for Xg94Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 151).
pub struct Xh151SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh151SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 193 as u64,
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

/// A compact bit set supporting boolean operations (variant 151).
pub struct Xh151BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh151BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 151).
pub struct Xi151Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi151Deque<T> {
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
pub struct Xi151Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi151Interval {
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

/// A simple interval tree (variant 151).
pub struct Xi151IntervalTree {
    xi_intervals: Vec<Xi151Interval>,
}

impl Xi151IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi151Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi151Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi151Interval) -> Vec<&Xi151Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi151Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi151Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi151Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi151Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi151Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi151Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 151) ---

/// Disjoint set / union-find for crate 151.
pub struct Xj151UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj151UnionFind {
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

const XJ151_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 151.
pub struct Xj151BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj151BTreeNode<K, V>>>,
    len: usize,
}

struct Xj151BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj151BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj151BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ151_BTREE_ORDER - 1
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
        let mid = XJ151_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj151BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj151BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj151BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj151BTreeNode::xj_new_leaf();
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


// --- xk_151 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk151SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk151SegmentTree {
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
pub struct Xk151DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk151DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_151).
#[derive(Debug, Clone)]
pub struct Xl151Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl151Rope {
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

/// Suffix array for efficient string searching (xl_151).
#[derive(Debug, Clone)]
pub struct Xl151SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl151SuffixArray {
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
pub struct Xm151MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm151MatrixSparse {
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
pub struct Xm151Tokenizer {
    text: String,
}

impl Xm151Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 151.
pub struct Xn151Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn151Fenwick {
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

// ----- AVL tree map — crate 151 -----

#[derive(Debug, Clone)]
struct Xn151AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn151AvlNode<K, V>>>,
    right: Option<Box<Xn151AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 151.
#[derive(Debug, Clone)]
pub struct Xn151AVL<K, V> {
    root: Option<Box<Xn151AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn151AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn151AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn151AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn151AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn151AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn151AvlNode<K, V>>) -> Box<Xn151AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn151AvlNode<K, V>>) -> Box<Xn151AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn151AvlNode<K, V>>) -> Box<Xn151AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn151AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn151AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn151AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn151AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn151AvlNode<K, V>>) -> &Xn151AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn151AvlNode<K, V>>) -> (Box<Xn151AvlNode<K, V>>, Option<Box<Xn151AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn151AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn151AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn151AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn151AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn151AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn151AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn151AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo151RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo151Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo151RBNode<K, V> {
    key: K,
    value: V,
    color: Xo151Color,
    left: Option<Box<Xo151RBNode<K, V>>>,
    right: Option<Box<Xo151RBNode<K, V>>>,
}

/// A red-black tree map for crate 151.
#[derive(Debug, Clone)]
pub struct Xo151RedBlack<K, V> {
    root: Option<Box<Xo151RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo151RedBlack<K, V> {
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
            r.color = Xo151Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo151RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo151RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo151RBNode {
                    key, value, color: Xo151Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo151RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo151Color::Red)
    }

    fn xo_balance(mut h: Box<Xo151RBNode<K, V>>) -> Box<Xo151RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo151Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo151RBNode<K, V>>) -> Box<Xo151RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo151Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo151RBNode<K, V>>) -> Box<Xo151RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo151Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo151RBNode<K, V>>) {
        h.color = Xo151Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo151Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo151Color::Black; }
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
            r.color = Xo151Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo151RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo151RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo151RBNode<K, V>) -> (K, V, Option<Box<Xo151RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo151RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo151Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo151RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo151ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 151.
#[derive(Debug, Clone)]
pub struct Xo151ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo151ConsistentHash {
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
            let vkey = format!("{}#xo151#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo151#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 151).
#[derive(Debug)]
pub struct Xp151SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp151Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp151Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp151Node<K, V>>>,
    xp_right: Option<Box<Xp151Node<K, V>>>,
}

impl<K: Ord, V> Xp151Node<K, V> {
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

impl<K: Ord, V> Default for Xp151SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp151SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp151Node<K, V>>>, key: &K) -> Option<Box<Xp151Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp151Node<K, V>>) -> Box<Xp151Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp151Node<K, V>>) -> Box<Xp151Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp151Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp151Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp151Node::xp_new(key, val));
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


// --------------- Xq151Treap ---------------

use std::cmp::Ordering as Xq151Ord;

struct Xq151TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq151TreapNode<K, V>>>,
    right: Option<Box<Xq151TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq151Treap<K, V> {
    root: Option<Box<Xq151TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq151TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_151_size<K, V>(node: &Option<Box<Xq151TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_151_update_size<K, V>(node: &mut Xq151TreapNode<K, V>) {
    node.size = 1 + xq_151_size(&node.left) + xq_151_size(&node.right);
}

fn xq_151_rotate_right<K, V>(mut node: Box<Xq151TreapNode<K, V>>) -> Box<Xq151TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_151_update_size(&mut node);
    left.right = Some(node);
    xq_151_update_size(&mut left);
    left
}

fn xq_151_rotate_left<K, V>(mut node: Box<Xq151TreapNode<K, V>>) -> Box<Xq151TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_151_update_size(&mut node);
    right.left = Some(node);
    xq_151_update_size(&mut right);
    right
}

fn xq_151_insert_node<K: Ord, V>(
    node: Option<Box<Xq151TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq151TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq151TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq151Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq151Ord::Less => {
                let (new_left, old) = xq_151_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_151_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_151_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq151Ord::Greater => {
                let (new_right, old) = xq_151_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_151_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_151_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_151_remove_node<K: Ord, V>(
    node: Option<Box<Xq151TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq151TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq151Ord::Less => {
                let (new_left, old) = xq_151_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_151_update_size(&mut n);
                (Some(n), old)
            }
            Xq151Ord::Greater => {
                let (new_right, old) = xq_151_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_151_update_size(&mut n);
                (Some(n), old)
            }
            Xq151Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_151_rotate_right(n);
                    let (new_right, old) = xq_151_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_151_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_151_rotate_left(n);
                    let (new_left, old) = xq_151_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_151_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_151_find_min<K, V>(node: &Option<Box<Xq151TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_151_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_151_find_max<K, V>(node: &Option<Box<Xq151TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_151_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_151_rank<K: Ord, V>(node: &Option<Box<Xq151TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq151Ord::Less => xq_151_rank(&n.left, key),
            Xq151Ord::Equal => xq_151_size(&n.left),
            Xq151Ord::Greater => 1 + xq_151_size(&n.left) + xq_151_rank(&n.right, key),
        },
    }
}

fn xq_151_kth<K, V>(node: &Option<Box<Xq151TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_151_size(&n.left);
        if k < left_size {
            xq_151_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_151_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_151_in_order<K: Clone, V>(node: &Option<Box<Xq151TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_151_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_151_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq151Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 151 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_151_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq151Ord::Equal => return Some(&n.value),
                Xq151Ord::Less => cur = &n.left,
                Xq151Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_151_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_151_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_151_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_151_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_151_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_151_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_151_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq151VEBTree ---------------

pub struct Xq151VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq151VEBTree>>,
    clusters: Vec<Option<Box<Xq151VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq151VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq151VEBTree::xq_new(sqrt_hi))) };
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
                    self.clusters[hi] = Some(Box::new(Xq151VEBTree::xq_new(self.sqrt_lo)));
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
pub struct Xr151KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr151KDPoint {
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
pub struct Xr151BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr151KDNode {
    xr_point: Xr151KDPoint,
    xr_left: Option<Box<Xr151KDNode>>,
    xr_right: Option<Box<Xr151KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr151KDTree {
    xr_root: Option<Box<Xr151KDNode>>,
    xr_size: usize,
}

impl Xr151KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr151KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr151KDNode>>,
        point: Xr151KDPoint,
        depth: usize,
    ) -> Box<Xr151KDNode> {
        match node {
            None => Box::new(Xr151KDNode {
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
    pub fn xr_nearest_neighbor(&self, query: &Xr151KDPoint) -> Option<Xr151KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr151KDNode>,
        query: &Xr151KDPoint,
        depth: usize,
        best: &mut Xr151KDPoint,
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
    ) -> Vec<Xr151KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr151KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr151KDPoint>,
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
    pub fn xr_all_points(&self) -> Vec<Xr151KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr151KDNode>>, pts: &mut Vec<Xr151KDPoint>) {
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

    fn xr_depth_rec(node: &Option<Box<Xr151KDNode>>) -> usize {
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
    pub fn xr_bounding_box(&self) -> Option<Xr151BoundingBox> {
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
        Some(Xr151BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
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


    // -- xg_94 graph tests ------------------------------------------------

    #[test]
    fn xg_94_graph_empty() {
        let g = super::Xg94Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_94_graph_add_node() {
        let mut g = super::Xg94Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_94_graph_add_edge() {
        let mut g = super::Xg94Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_94_graph_neighbors() {
        let mut g = super::Xg94Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_94_graph_has_path() {
        let mut g = super::Xg94Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_94_graph_self_path() {
        let g = super::Xg94Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_94_graph_topo_sort() {
        let mut g = super::Xg94Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_94_graph_cycle_detect_false() {
        let mut g = super::Xg94Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_94_graph_cycle_detect_true() {
        let mut g = super::Xg94Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_94 heap tests -------------------------------------------------

    #[test]
    fn xg_94_heap_empty() {
        let h: super::Xg94Heap<i32> = super::Xg94Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_94_heap_push_pop() {
        let mut h = super::Xg94Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_94_heap_peek() {
        let mut h = super::Xg94Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_94_heap_drain_sorted() {
        let mut h = super::Xg94Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_94_heap_merge() {
        let mut a = super::Xg94Heap::new();
        let mut b = super::Xg94Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_94_heap_default() {
        let h: super::Xg94Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_94_graph_default() {
        let g: super::Xg94Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh151_skip_insert_contains() {
        let mut sl = super::Xh151SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh151_skip_remove() {
        let mut sl = super::Xh151SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh151_skip_len() {
        let mut sl = super::Xh151SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh151_skip_range_query() {
        let mut sl = super::Xh151SkipList::xh_new(4);
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
    fn xh151_skip_floor_ceiling() {
        let mut sl = super::Xh151SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh151_skip_rank() {
        let mut sl = super::Xh151SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh151_skip_empty() {
        let sl = super::Xh151SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh151_skip_duplicates() {
        let mut sl = super::Xh151SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh151_bitset_set_test() {
        let mut bs = super::Xh151BitSet::xh_new(256);
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
    fn xh151_bitset_clear_count() {
        let mut bs = super::Xh151BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh151_bitset_and_or_xor() {
        let mut a = super::Xh151BitSet::xh_new(128);
        let mut b = super::Xh151BitSet::xh_new(128);
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
    fn xh151_bitset_iter_ones() {
        let mut bs = super::Xh151BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh151_bitset_first_last() {
        let mut bs = super::Xh151BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh151_bitset_empty() {
        let bs = super::Xh151BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi151_deque_push_pop_back() {
        let mut dq = super::Xi151Deque::xi_new(4);
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
    fn xi151_deque_push_pop_front() {
        let mut dq = super::Xi151Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi151_deque_mixed_ops() {
        let mut dq = super::Xi151Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi151_deque_get_and_split() {
        let mut dq = super::Xi151Deque::xi_new(8);
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
    fn xi151_deque_rotate_left() {
        let mut dq = super::Xi151Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi151_deque_rotate_right() {
        let mut dq = super::Xi151Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi151_deque_grow() {
        let mut dq = super::Xi151Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi151_deque_empty() {
        let dq = super::Xi151Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi151_interval_tree_insert_query() {
        let mut tree = super::Xi151IntervalTree::xi_new();
        tree.xi_insert(super::Xi151Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi151Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi151Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi151_interval_tree_overlap() {
        let mut tree = super::Xi151IntervalTree::xi_new();
        tree.xi_insert(super::Xi151Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi151Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi151Interval::xi_new(12, 20));
        let q = super::Xi151Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi151_interval_tree_remove() {
        let mut tree = super::Xi151IntervalTree::xi_new();
        tree.xi_insert(super::Xi151Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi151Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi151_interval_tree_gaps() {
        let mut tree = super::Xi151IntervalTree::xi_new();
        tree.xi_insert(super::Xi151Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi151Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi151Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi151Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi151Interval::xi_new(8, 10));
    }

    #[test]
    fn xi151_interval_tree_merge() {
        let mut tree = super::Xi151IntervalTree::xi_new();
        tree.xi_insert(super::Xi151Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi151Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi151Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi151Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi151Interval::xi_new(10, 15));
    }

    #[test]
    fn xi151_interval_tree_all() {
        let mut tree = super::Xi151IntervalTree::xi_new();
        tree.xi_insert(super::Xi151Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi151Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi151_interval_tree_empty() {
        let tree = super::Xi151IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi151_interval_tree_contains_point() {
        let iv = super::Xi151Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 151) ---

    #[test]
    fn xj_151_uf_make_and_find() {
        let mut uf = super::Xj151UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_151_uf_union_connected() {
        let mut uf = super::Xj151UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_151_uf_component_count() {
        let mut uf = super::Xj151UnionFind::xj_new();
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
    fn xj_151_uf_component_size() {
        let mut uf = super::Xj151UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_151_uf_largest_component() {
        let mut uf = super::Xj151UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_151_uf_many_elements() {
        let mut uf = super::Xj151UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_151_uf_separate_components() {
        let mut uf = super::Xj151UnionFind::xj_new();
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
    fn xj_151_uf_path_compression() {
        let mut uf = super::Xj151UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_151_bt_insert_get() {
        let mut bt = super::Xj151BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_151_bt_contains_len() {
        let mut bt = super::Xj151BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_151_bt_replace() {
        let mut bt = super::Xj151BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_151_bt_remove() {
        let mut bt = super::Xj151BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_151_bt_keys_values() {
        let mut bt = super::Xj151BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_151_bt_range() {
        let mut bt = super::Xj151BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_151_bt_min_max() {
        let mut bt = super::Xj151BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_151_bt_many_inserts() {
        let mut bt = super::Xj151BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_151 segment tree tests ---

    #[test]
    fn xk_151_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk151SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_151_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk151SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_151_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk151SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_151_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk151SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_151_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk151SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_151_st_single_element() {
        let data = vec![42];
        let st = super::Xk151SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_151_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk151SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_151_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk151SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_151 disjoint intervals tests ---

    #[test]
    fn xk_151_di_add_and_count() {
        let mut di = super::Xk151DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_151_di_merge_overlap() {
        let mut di = super::Xk151DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_151_di_contains() {
        let mut di = super::Xk151DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_151_di_remove() {
        let mut di = super::Xk151DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_151_di_covered_length() {
        let mut di = super::Xk151DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_151_di_gaps() {
        let mut di = super::Xk151DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_151_di_merge_adjacent() {
        let mut di = super::Xk151DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_151_di_empty() {
        let di = super::Xk151DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_151_rope_new_empty() {
        let rope = super::Xl151Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_151_rope_from_str() {
        let rope = super::Xl151Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_151_rope_insert_at() {
        let mut rope = super::Xl151Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_151_rope_delete_range() {
        let mut rope = super::Xl151Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_151_rope_char_at() {
        let rope = super::Xl151Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_151_rope_split_concat() {
        let rope = super::Xl151Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_151_rope_line_count() {
        let rope = super::Xl151Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_151_rope_line_at() {
        let rope = super::Xl151Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_151_sa_build_and_search() {
        let sa = super::Xl151SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_151_sa_count() {
        let sa = super::Xl151SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_151_sa_longest_repeated() {
        let sa = super::Xl151SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_151_sa_all_positions() {
        let sa = super::Xl151SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_151_sa_len() {
        let sa = super::Xl151SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_151_sa_empty() {
        let sa = super::Xl151SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_151_rope_slice() {
        let rope = super::Xl151Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_151_sa_search_start() {
        let sa = super::Xl151SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_151_sparse_set_get() {
        let mut m = super::Xm151MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_151_sparse_row_col() {
        let mut m = super::Xm151MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_151_sparse_transpose() {
        let mut m = super::Xm151MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_151_sparse_multiply_vec() {
        let mut m = super::Xm151MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_151_sparse_nnz_density() {
        let mut m = super::Xm151MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_151_sparse_clear() {
        let mut m = super::Xm151MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_151_sparse_overwrite_zero() {
        let mut m = super::Xm151MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_151_tokenizer_basic() {
        let t = super::Xm151Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_151_tokenizer_count() {
        let t = super::Xm151Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_151_tokenizer_unique() {
        let t = super::Xm151Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_151_tokenizer_frequency() {
        let t = super::Xm151Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_151_tokenizer_delimiter() {
        let t = super::Xm151Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_151_tokenizer_whitespace() {
        let t = super::Xm151Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_151_tokenizer_empty() {
        let t = super::Xm151Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 151 ----

    #[test]
    fn xn_151_fenwick_prefix_sum() {
        let mut ft = super::Xn151Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_151_fenwick_range_sum() {
        let mut ft = super::Xn151Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_151_fenwick_point_query() {
        let mut ft = super::Xn151Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_151_fenwick_len() {
        let ft = super::Xn151Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_151_fenwick_multiple_updates() {
        let mut ft = super::Xn151Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_151_fenwick_single_element() {
        let mut ft = super::Xn151Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_151_fenwick_find_kth() {
        let mut ft = super::Xn151Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_151_fenwick_negative_delta() {
        let mut ft = super::Xn151Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 151 ----

    #[test]
    fn xn_151_avl_insert_get() {
        let mut m = super::Xn151AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_151_avl_remove() {
        let mut m = super::Xn151AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_151_avl_in_order() {
        let mut m = super::Xn151AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_151_avl_min_max() {
        let mut m = super::Xn151AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_151_avl_floor_ceiling() {
        let mut m = super::Xn151AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_151_avl_height_balanced() {
        let mut m = super::Xn151AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_151_avl_overwrite() {
        let mut m = super::Xn151AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_151_avl_empty() {
        let m: super::Xn151AVL<i32, i32> = super::Xn151AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo151RedBlack tests ---

    #[test]
    fn xo_151_rb_insert_and_get() {
        let mut tree = super::Xo151RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_151_rb_len_and_empty() {
        let mut tree = super::Xo151RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_151_rb_min_max() {
        let mut tree = super::Xo151RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_151_rb_contains() {
        let mut tree = super::Xo151RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_151_rb_remove() {
        let mut tree = super::Xo151RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_151_rb_in_order() {
        let mut tree = super::Xo151RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_151_rb_black_height() {
        let mut tree = super::Xo151RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_151_rb_overwrite() {
        let mut tree = super::Xo151RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo151ConsistentHash tests ---

    #[test]
    fn xo_151_ch_add_and_count() {
        let mut ring = super::Xo151ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_151_ch_remove_node() {
        let mut ring = super::Xo151ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_151_ch_get_node() {
        let mut ring = super::Xo151ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_151_ch_empty_ring() {
        let ring = super::Xo151ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_151_ch_distribution() {
        let mut ring = super::Xo151ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_151_ch_rebalance() {
        let mut ring = super::Xo151ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_151_ch_virtual_nodes() {
        let mut ring = super::Xo151ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_151_ch_consistent_lookup() {
        let mut ring = super::Xo151ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_151_splay_insert_get() {
        let mut t = super::Xp151SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_151_splay_remove() {
        let mut t = super::Xp151SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_151_splay_count_increases() {
        let mut t = super::Xp151SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_151_splay_depth() {
        let mut t = super::Xp151SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_151_splay_len_empty() {
        let t = super::Xp151SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_151_splay_min_max() {
        let mut t = super::Xp151SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_151_splay_overwrite() {
        let mut t = super::Xp151SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_151_splay_remove_missing() {
        let mut t = super::Xp151SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_151 treap tests ----
    #[test]
    fn xq_151_treap_empty() {
        let t = super::Xq151Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_151_treap_insert_get() {
        let mut t = super::Xq151Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_151_treap_overwrite() {
        let mut t = super::Xq151Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_151_treap_remove() {
        let mut t = super::Xq151Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_151_treap_min_max() {
        let mut t = super::Xq151Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_151_treap_rank() {
        let mut t = super::Xq151Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_151_treap_kth() {
        let mut t = super::Xq151Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_151_treap_in_order() {
        let mut t = super::Xq151Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_151 VEB tree tests ----
    #[test]
    fn xq_151_veb_empty() {
        let v = super::Xq151VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_151_veb_insert_contains() {
        let mut v = super::Xq151VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_151_veb_min_max() {
        let mut v = super::Xq151VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_151_veb_delete() {
        let mut v = super::Xq151VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_151_veb_successor() {
        let mut v = super::Xq151VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_151_veb_predecessor() {
        let mut v = super::Xq151VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_151_veb_count() {
        let mut v = super::Xq151VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_151_veb_duplicate_insert() {
        let mut v = super::Xq151VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_151_kdtree_empty() {
        let tree = super::Xr151KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_151_kdtree_insert_one() {
        let mut tree = super::Xr151KDTree::xr_new();
        tree.xr_insert(super::Xr151KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_151_kdtree_insert_multiple() {
        let mut tree = super::Xr151KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr151KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_151_kdtree_nearest_neighbor() {
        let mut tree = super::Xr151KDTree::xr_new();
        tree.xr_insert(super::Xr151KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr151KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr151KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_151_kdtree_nn_empty() {
        let tree = super::Xr151KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr151KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_151_kdtree_range_search() {
        let mut tree = super::Xr151KDTree::xr_new();
        tree.xr_insert(super::Xr151KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr151KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr151KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_151_kdtree_range_empty() {
        let mut tree = super::Xr151KDTree::xr_new();
        tree.xr_insert(super::Xr151KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_151_kdtree_all_points() {
        let mut tree = super::Xr151KDTree::xr_new();
        tree.xr_insert(super::Xr151KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr151KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_151_kdtree_depth() {
        let mut tree = super::Xr151KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr151KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_151_kdtree_bounding_box() {
        let mut tree = super::Xr151KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr151KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr151KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

}
