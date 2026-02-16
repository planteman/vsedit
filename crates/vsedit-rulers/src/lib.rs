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
}
