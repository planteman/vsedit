//! Sticky scroll widget that pins nesting-context lines at the top of the editor viewport.

use std::fmt;

/// Errors produced by [`StickyScrollWidget`] operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StickyScrollError {
    WidgetDisabled,
    NoLinesAvailable,
    InvalidNestingLevel(u32),
}

impl fmt::Display for StickyScrollError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WidgetDisabled => write!(f, "sticky scroll widget is disabled"),
            Self::NoLinesAvailable => write!(f, "no sticky scroll lines available"),
            Self::InvalidNestingLevel(n) => write!(f, "invalid nesting level: {n}"),
        }
    }
}

/// A single line pinned in the sticky scroll area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StickyScrollLine {
    pub line_number: u32,
    pub text: String,
    pub nesting_level: u32,
    pub collapsed: bool,
}

impl StickyScrollLine {
    /// Return a string of spaces proportional to `nesting_level` (4 spaces per level).
    pub fn indentation(&self) -> String {
        " ".repeat(self.nesting_level as usize * 4)
    }
}

impl fmt::Display for StickyScrollLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "L{}: {} (level {})",
            self.line_number, self.text, self.nesting_level
        )
    }
}

/// Widget that manages the set of sticky scroll lines.
#[derive(Debug, Clone)]
pub struct StickyScrollWidget {
    lines: Vec<StickyScrollLine>,
    max_lines: usize,
    enabled: bool,
}

impl StickyScrollWidget {
    /// Create a new widget that displays at most `max_lines` sticky lines.
    pub fn new(max_lines: usize) -> Self {
        Self {
            lines: Vec::new(),
            max_lines,
            enabled: true,
        }
    }

    /// Replace the current sticky lines based on visible ranges and nesting data.
    ///
    /// `nesting_data` entries are `(line_number, text, nesting_level)` tuples that
    /// describe scope-opening lines (e.g. function / class headers). Only lines
    /// whose nesting level fits within `max_lines` are retained.
    pub fn update_lines(
        &mut self,
        _visible_range_start: u32,
        _visible_range_end: u32,
        nesting_data: &[(u32, &str, u32)],
    ) {
        if !self.enabled {
            return;
        }
        self.lines = nesting_data
            .iter()
            .take(self.max_lines)
            .map(|(line, text, level)| StickyScrollLine {
                line_number: *line,
                text: (*text).to_string(),
                nesting_level: *level,
                collapsed: false,
            })
            .collect();
    }

    /// Enable or disable the widget.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.lines.clear();
        }
    }

    /// Return the currently visible sticky scroll lines.
    pub fn get_visible_sticky_lines(&self) -> &[StickyScrollLine] {
        &self.lines
    }

    /// Remove all sticky lines.
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Toggle the collapsed state of the sticky line at the given line number.
    pub fn toggle_collapse(&mut self, line_number: u32) -> Result<(), StickyScrollError> {
        if !self.enabled {
            return Err(StickyScrollError::WidgetDisabled);
        }
        let line = self
            .lines
            .iter_mut()
            .find(|l| l.line_number == line_number)
            .ok_or(StickyScrollError::NoLinesAvailable)?;
        line.collapsed = !line.collapsed;
        Ok(())
    }

    /// Return the number of currently collapsed sticky lines.
    pub fn collapsed_count(&self) -> usize {
        self.lines.iter().filter(|l| l.collapsed).count()
    }

    /// Return the highest nesting level among current sticky lines, or `None` if empty.
    pub fn max_nesting_level(&self) -> Option<u32> {
        self.lines.iter().map(|l| l.nesting_level).max()
    }

    /// Find a sticky line by its line number.
    pub fn find_by_line(&self, line_number: u32) -> Option<&StickyScrollLine> {
        self.lines.iter().find(|l| l.line_number == line_number)
    }

    /// Dynamically change the maximum number of displayed sticky lines.
    pub fn set_max_lines(&mut self, max_lines: usize) {
        self.max_lines = max_lines;
        self.lines.truncate(max_lines);
    }

    /// Return only the non-collapsed sticky lines.
    pub fn get_uncollapsed_lines(&self) -> Vec<&StickyScrollLine> {
        self.lines.iter().filter(|l| !l.collapsed).collect()
    }

    /// Returns true if lines is empty.
    pub fn is_lines_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Get the first line, if any.
    pub fn first_line(&self) -> Option<&StickyScrollLine> {
        self.lines.first()
    }

    /// Get the last line, if any.
    pub fn last_line(&self) -> Option<&StickyScrollLine> {
        self.lines.last()
    }

    /// Retain only lines matching the predicate.
    pub fn retain_lines(&mut self, f: impl Fn(&StickyScrollLine) -> bool) {
        self.lines.retain(|item| f(item));
    }

    /// Return the number of currently visible sticky lines.
    pub fn visible_count(&self) -> usize {
        self.lines.len()
    }

    /// Check if the widget is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the max_lines setting.
    pub fn max_lines(&self) -> usize {
        self.max_lines
    }

    /// Remove a specific sticky line by its line number.
    pub fn remove_line(&mut self, line_number: u32) -> Result<(), StickyScrollError> {
        if !self.enabled {
            return Err(StickyScrollError::WidgetDisabled);
        }
        let idx = self.lines.iter().position(|l| l.line_number == line_number)
            .ok_or(StickyScrollError::NoLinesAvailable)?;
        self.lines.remove(idx);
        Ok(())
    }

    /// Collapse all sticky lines.
    pub fn collapse_all(&mut self) {
        for line in &mut self.lines {
            line.collapsed = true;
        }
    }

    /// Expand all sticky lines.
    pub fn expand_all(&mut self) {
        for line in &mut self.lines {
            line.collapsed = false;
        }
    }

    /// Get the total text length of all sticky lines.
    pub fn total_text_length(&self) -> usize {
        self.lines.iter().map(|l| l.text.len()).sum()
    }

    /// Get lines at a specific nesting level.
    pub fn lines_at_level(&self, level: u32) -> Vec<&StickyScrollLine> {
        self.lines.iter().filter(|l| l.nesting_level == level).collect()
    }

    /// Get the minimum nesting level, or None if empty.
    pub fn min_nesting_level(&self) -> Option<u32> {
        self.lines.iter().map(|l| l.nesting_level).min()
    }

    /// Check if a specific line number is displayed.
    pub fn contains_line(&self, line_number: u32) -> bool {
        self.lines.iter().any(|l| l.line_number == line_number)
    }

    /// Sort sticky lines by nesting level.
    pub fn sort_by_nesting(&mut self) {
        self.lines.sort_by_key(|l| l.nesting_level);
    }

    /// Get the deepest sticky line (highest nesting level).
    pub fn deepest_line(&self) -> Option<&StickyScrollLine> {
        self.lines.iter().max_by_key(|l| l.nesting_level)
    }

    /// Get all line numbers currently displayed.
    pub fn line_numbers(&self) -> Vec<u32> {
        self.lines.iter().map(|l| l.line_number).collect()
    }

    /// Compute effective height excluding collapsed lines.
    pub fn effective_height(&self) -> usize {
        self.lines.iter().filter(|l| !l.collapsed).count()
    }

    /// Toggle the enabled flag.
    pub fn toggle_enabled(&mut self) {
        self.enabled = !self.enabled;
        if !self.enabled {
            self.lines.clear();
        }
    }
}

/// Configuration for the sticky scroll feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StickyScrollConfig {
    pub enabled: bool,
    pub max_line_count: usize,
    pub default_model: String,
}

impl Default for StickyScrollConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_line_count: 5,
            default_model: "outlineModel".to_string(),
        }
    }
}

/// Builder for [`StickyScrollConfig`].
#[derive(Debug, Clone)]
pub struct StickyScrollConfigBuilder {
    enabled: bool,
    max_line_count: usize,
    default_model: String,
}

impl StickyScrollConfigBuilder {
    pub fn new() -> Self {
        let defaults = StickyScrollConfig::default();
        Self {
            enabled: defaults.enabled,
            max_line_count: defaults.max_line_count,
            default_model: defaults.default_model,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn max_line_count(mut self, count: usize) -> Self {
        self.max_line_count = count;
        self
    }

    pub fn default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    pub fn build(self) -> StickyScrollConfig {
        StickyScrollConfig {
            enabled: self.enabled,
            max_line_count: self.max_line_count,
            default_model: self.default_model,
        }
    }
}

impl Default for StickyScrollConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for stickyscroll operations.
#[derive(Debug, Clone, PartialEq)]
pub struct StickyscrollStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl StickyscrollStats {
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
    pub fn merge(&mut self, other: &StickyscrollStats) {
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

impl Default for StickyscrollStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for StickyscrollStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StickyscrollStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for stickyscroll.
#[derive(Debug, Clone)]
pub struct StickyscrollValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl StickyscrollValidator {
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

impl Default for StickyscrollValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// A display profile for the sticky scroll feature, combining visual options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StickyScrollProfile {
    /// Maximum number of sticky lines to show.
    pub max_lines: usize,
    /// Whether sticky scroll is enabled.
    pub enabled: bool,
    /// Strategy used to compute sticky lines.
    pub strategy: StickyScrollStrategy,
}

/// Determines how sticky lines are computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StickyScrollStrategy {
    /// Compute from indentation levels.
    Indentation,
    /// Compute from language-specific scopes (e.g., function/class).
    Scope,
}

impl Default for StickyScrollProfile {
    fn default() -> Self {
        Self {
            max_lines: 5,
            enabled: true,
            strategy: StickyScrollStrategy::Indentation,
        }
    }
}

impl StickyScrollProfile {
    pub fn new(max_lines: usize, enabled: bool, strategy: StickyScrollStrategy) -> Self {
        Self { max_lines, enabled, strategy }
    }

    /// Apply this profile to a StickyScrollWidget.
    pub fn apply(&self, widget: &mut StickyScrollWidget) {
        widget.set_enabled(self.enabled);
        widget.set_max_lines(self.max_lines);
    }
}

impl fmt::Display for StickyScrollProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StickyScrollProfile(max={}, enabled={}, strategy={:?})",
            self.max_lines, self.enabled, self.strategy
        )
    }
}

impl fmt::Display for StickyScrollStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StickyScrollStrategy::Indentation => write!(f, "Indentation"),
            StickyScrollStrategy::Scope => write!(f, "Scope"),
        }
    }
}

/// Compute sticky scroll lines from source text based on indentation.
/// Each line that starts a new indentation scope (has greater indentation
/// than the previous significant line) is a potential sticky header.
/// The function finds ancestors of `cursor_line` in the indentation tree.
pub fn sticky_scroll_compute(lines: &[&str], cursor_line: usize, max_depth: usize) -> Vec<StickyScrollLine> {
    if lines.is_empty() || cursor_line >= lines.len() {
        return Vec::new();
    }

    let indents: Vec<u32> = lines.iter().map(|l| {
        let spaces = l.chars().take_while(|c| *c == ' ').count();
        (spaces / 4) as u32
    }).collect();

    let mut headers: Vec<StickyScrollLine> = Vec::new();
    let cursor_indent = indents[cursor_line];
    let mut target_indent = cursor_indent;

    let mut i = cursor_line;
    loop {
        if indents[i] < target_indent {
            let text = lines[i].trim().to_string();
            if !text.is_empty() {
                headers.push(StickyScrollLine {
                    line_number: i as u32 + 1,
                    text,
                    nesting_level: indents[i],
                    collapsed: false,
                });
                target_indent = indents[i];
            }
        }
        if i == 0 || headers.len() >= max_depth {
            break;
        }
        i -= 1;
    }

    headers.reverse();
    headers
}

/// Compute sticky lines from scope information (e.g., from a language server).
/// `scopes` is a list of `(start_line, end_line, text, nesting_level)`.
pub fn sticky_scroll_from_scopes(
    scopes: &[(u32, u32, &str, u32)],
    cursor_line: u32,
    max_depth: usize,
) -> Vec<StickyScrollLine> {
    let mut active: Vec<&(u32, u32, &str, u32)> = scopes.iter()
        .filter(|(start, end, _, _)| *start <= cursor_line && *end >= cursor_line)
        .collect();
    active.sort_by_key(|(_, _, _, level)| *level);
    active.truncate(max_depth);
    active.iter().map(|(start, _, text, level)| {
        StickyScrollLine {
            line_number: *start,
            text: text.to_string(),
            nesting_level: *level,
            collapsed: false,
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_widget_is_empty_and_enabled() {
        let w = StickyScrollWidget::new(5);
        assert!(w.enabled);
        assert!(w.get_visible_sticky_lines().is_empty());
    }

    #[test]
    fn update_lines_respects_max() {
        let mut w = StickyScrollWidget::new(2);
        let data = vec![
            (1, "fn main() {", 0),
            (5, "if cond {", 1),
            (10, "for x in xs {", 2),
        ];
        w.update_lines(0, 20, &data);
        assert_eq!(w.get_visible_sticky_lines().len(), 2);
        assert_eq!(w.get_visible_sticky_lines()[0].line_number, 1);
        assert_eq!(w.get_visible_sticky_lines()[1].line_number, 5);
    }

    #[test]
    fn disable_clears_lines() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 10, &[(1, "fn foo() {", 0)]);
        assert_eq!(w.get_visible_sticky_lines().len(), 1);

        w.set_enabled(false);
        assert!(w.get_visible_sticky_lines().is_empty());

        // update_lines is a no-op when disabled
        w.update_lines(0, 10, &[(2, "fn bar() {", 0)]);
        assert!(w.get_visible_sticky_lines().is_empty());
    }

    #[test]
    fn default_config() {
        let cfg = StickyScrollConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.max_line_count, 5);
        assert_eq!(cfg.default_model, "outlineModel");
    }

    #[test]
    fn toggle_collapse_works() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 20, &[(1, "fn main() {", 0), (5, "if cond {", 1)]);
        assert!(!w.find_by_line(1).unwrap().collapsed);
        w.toggle_collapse(1).unwrap();
        assert!(w.find_by_line(1).unwrap().collapsed);
        w.toggle_collapse(1).unwrap();
        assert!(!w.find_by_line(1).unwrap().collapsed);
    }

    #[test]
    fn toggle_collapse_disabled_widget() {
        let mut w = StickyScrollWidget::new(5);
        w.set_enabled(false);
        assert_eq!(w.toggle_collapse(1), Err(StickyScrollError::WidgetDisabled));
    }

    #[test]
    fn toggle_collapse_missing_line() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 10, &[(1, "fn foo() {", 0)]);
        assert_eq!(
            w.toggle_collapse(99),
            Err(StickyScrollError::NoLinesAvailable)
        );
    }

    #[test]
    fn collapsed_count_tracks_collapsed() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 20, &[(1, "a", 0), (2, "b", 1), (3, "c", 2)]);
        assert_eq!(w.collapsed_count(), 0);
        w.toggle_collapse(1).unwrap();
        w.toggle_collapse(3).unwrap();
        assert_eq!(w.collapsed_count(), 2);
    }

    #[test]
    fn max_nesting_level_works() {
        let mut w = StickyScrollWidget::new(10);
        assert_eq!(w.max_nesting_level(), None);
        w.update_lines(0, 30, &[(1, "a", 0), (5, "b", 3), (10, "c", 2)]);
        assert_eq!(w.max_nesting_level(), Some(3));
    }

    #[test]
    fn find_by_line_returns_correct() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 20, &[(1, "fn main() {", 0), (5, "if cond {", 1)]);
        let found = w.find_by_line(5).unwrap();
        assert_eq!(found.text, "if cond {");
        assert!(w.find_by_line(99).is_none());
    }

    #[test]
    fn set_max_lines_truncates() {
        let mut w = StickyScrollWidget::new(10);
        w.update_lines(0, 30, &[(1, "a", 0), (2, "b", 1), (3, "c", 2), (4, "d", 3)]);
        assert_eq!(w.get_visible_sticky_lines().len(), 4);
        w.set_max_lines(2);
        assert_eq!(w.get_visible_sticky_lines().len(), 2);
    }

    #[test]
    fn display_impl_for_line() {
        let line = StickyScrollLine {
            line_number: 42,
            text: "fn hello()".to_string(),
            nesting_level: 2,
            collapsed: false,
        };
        assert_eq!(format!("{line}"), "L42: fn hello() (level 2)");
    }

    #[test]
    fn indentation_proportional_to_level() {
        let line = StickyScrollLine {
            line_number: 1,
            text: "x".to_string(),
            nesting_level: 3,
            collapsed: false,
        };
        assert_eq!(line.indentation(), "            "); // 12 spaces
        let root = StickyScrollLine {
            line_number: 1,
            text: "y".to_string(),
            nesting_level: 0,
            collapsed: false,
        };
        assert_eq!(root.indentation(), "");
    }

    #[test]
    fn config_builder_defaults() {
        let cfg = StickyScrollConfigBuilder::new().build();
        assert_eq!(cfg, StickyScrollConfig::default());
    }

    #[test]
    fn config_builder_custom() {
        let cfg = StickyScrollConfigBuilder::new()
            .enabled(false)
            .max_line_count(10)
            .default_model("foldingModel")
            .build();
        assert!(!cfg.enabled);
        assert_eq!(cfg.max_line_count, 10);
        assert_eq!(cfg.default_model, "foldingModel");
    }

    #[test]
    fn get_uncollapsed_lines_filters() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 20, &[(1, "a", 0), (2, "b", 1), (3, "c", 2)]);
        w.toggle_collapse(2).unwrap();
        let uncollapsed = w.get_uncollapsed_lines();
        assert_eq!(uncollapsed.len(), 2);
        assert_eq!(uncollapsed[0].line_number, 1);
        assert_eq!(uncollapsed[1].line_number, 3);
    }

    #[test]
    fn error_display_messages() {
        assert_eq!(
            format!("{}", StickyScrollError::WidgetDisabled),
            "sticky scroll widget is disabled"
        );
        assert_eq!(
            format!("{}", StickyScrollError::NoLinesAvailable),
            "no sticky scroll lines available"
        );
        assert_eq!(
            format!("{}", StickyScrollError::InvalidNestingLevel(7)),
            "invalid nesting level: 7"
        );
    }

    #[test]
    fn visible_count_matches() {
        let mut w = StickyScrollWidget::new(5);
        assert_eq!(w.visible_count(), 0);
        w.update_lines(0, 20, &[(1, "fn main() {", 0), (5, "if cond {", 1)]);
        assert_eq!(w.visible_count(), 2);
    }

    #[test]
    fn is_enabled_and_max_lines() {
        let w = StickyScrollWidget::new(7);
        assert!(w.is_enabled());
        assert_eq!(w.max_lines(), 7);
    }

    #[test]
    fn remove_line_works() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 20, &[(1, "a", 0), (5, "b", 1), (10, "c", 2)]);
        w.remove_line(5).unwrap();
        assert_eq!(w.visible_count(), 2);
        assert!(w.find_by_line(5).is_none());
    }

    #[test]
    fn remove_line_not_found() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 20, &[(1, "a", 0)]);
        assert_eq!(w.remove_line(99), Err(StickyScrollError::NoLinesAvailable));
    }

    #[test]
    fn remove_line_disabled() {
        let mut w = StickyScrollWidget::new(5);
        w.set_enabled(false);
        assert_eq!(w.remove_line(1), Err(StickyScrollError::WidgetDisabled));
    }

    #[test]
    fn collapse_and_expand_all() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 20, &[(1, "a", 0), (2, "b", 1), (3, "c", 2)]);
        w.collapse_all();
        assert_eq!(w.collapsed_count(), 3);
        w.expand_all();
        assert_eq!(w.collapsed_count(), 0);
    }

    #[test]
    fn total_text_length_computed() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 20, &[(1, "abc", 0), (2, "de", 1)]);
        assert_eq!(w.total_text_length(), 5);
    }

    #[test]
    fn lines_at_level_filters() {
        let mut w = StickyScrollWidget::new(10);
        w.update_lines(0, 30, &[(1, "a", 0), (5, "b", 1), (10, "c", 0)]);
        assert_eq!(w.lines_at_level(0).len(), 2);
        assert_eq!(w.lines_at_level(1).len(), 1);
        assert_eq!(w.lines_at_level(5).len(), 0);
    }

    #[test]
    fn min_nesting_level_works() {
        let mut w = StickyScrollWidget::new(10);
        assert_eq!(w.min_nesting_level(), None);
        w.update_lines(0, 30, &[(1, "a", 2), (5, "b", 1), (10, "c", 3)]);
        assert_eq!(w.min_nesting_level(), Some(1));
    }

    #[test]
    fn contains_line_check() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 20, &[(1, "a", 0), (5, "b", 1)]);
        assert!(w.contains_line(1));
        assert!(!w.contains_line(99));
    }

    #[test]
    fn sort_by_nesting_reorders() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 20, &[(10, "c", 2), (1, "a", 0), (5, "b", 1)]);
        w.sort_by_nesting();
        let lines = w.get_visible_sticky_lines();
        assert_eq!(lines[0].nesting_level, 0);
        assert_eq!(lines[2].nesting_level, 2);
    }

    #[test]
    fn deepest_line_found() {
        let mut w = StickyScrollWidget::new(10);
        w.update_lines(0, 30, &[(1, "a", 0), (5, "b", 3), (10, "c", 1)]);
        assert_eq!(w.deepest_line().unwrap().nesting_level, 3);
    }

    #[test]
    fn deepest_line_empty() {
        let w = StickyScrollWidget::new(5);
        assert!(w.deepest_line().is_none());
    }

    #[test]
    fn line_numbers_returns_all() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 20, &[(3, "a", 0), (7, "b", 1)]);
        assert_eq!(w.line_numbers(), vec![3, 7]);
    }

    #[test]
    fn effective_height_excludes_collapsed() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 20, &[(1, "a", 0), (2, "b", 1), (3, "c", 2)]);
        assert_eq!(w.effective_height(), 3);
        w.toggle_collapse(2).unwrap();
        assert_eq!(w.effective_height(), 2);
    }

    #[test]
    fn first_and_last_line() {
        let mut w = StickyScrollWidget::new(5);
        assert!(w.first_line().is_none());
        w.update_lines(0, 20, &[(1, "first", 0), (5, "last", 1)]);
        assert_eq!(w.first_line().unwrap().text, "first");
        assert_eq!(w.last_line().unwrap().text, "last");
    }

    #[test]
    fn retain_lines_filters() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 20, &[(1, "a", 0), (2, "b", 1), (3, "c", 2)]);
        w.retain_lines(|l| l.nesting_level > 0);
        assert_eq!(w.visible_count(), 2);
    }

    #[test]
    fn toggle_enabled_clears_when_disabling() {
        let mut w = StickyScrollWidget::new(5);
        w.update_lines(0, 20, &[(1, "a", 0)]);
        assert!(w.is_enabled());
        w.toggle_enabled();
        assert!(!w.is_enabled());
        assert_eq!(w.visible_count(), 0);
    }

    #[test]
    fn is_lines_empty_initially() {
        let w = StickyScrollWidget::new(5);
        assert!(w.is_lines_empty());
    }

    #[test]
    fn stickyscroll_stats_new_defaults() {
        let stats = StickyscrollStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn stickyscroll_stats_record_success() {
        let mut stats = StickyscrollStats::new();
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
    fn stickyscroll_stats_record_failure() {
        let mut stats = StickyscrollStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn stickyscroll_stats_reset() {
        let mut stats = StickyscrollStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn stickyscroll_stats_merge() {
        let mut a = StickyscrollStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = StickyscrollStats::new();
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
    fn stickyscroll_stats_display() {
        let mut stats = StickyscrollStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn stickyscroll_stats_default() {
        let stats = StickyscrollStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn stickyscroll_validator_accepts_valid_name() {
        let v = StickyscrollValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn stickyscroll_validator_rejects_empty() {
        let v = StickyscrollValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn stickyscroll_validator_rejects_too_long() {
        let v = StickyscrollValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn stickyscroll_validator_forbidden_prefix() {
        let v = StickyscrollValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn stickyscroll_validator_allowed_chars() {
        let v = StickyscrollValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn stickyscroll_validator_range() {
        let v = StickyscrollValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn stickyscroll_sanitize_removes_control() {
        let result = StickyscrollValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn stickyscroll_truncate_short_string() {
        assert_eq!(StickyscrollValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn stickyscroll_truncate_long_string() {
        let result = StickyscrollValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn stickyscroll_is_ascii_printable() {
        assert!(StickyscrollValidator::is_ascii_printable("Hello World 123"));
        assert!(!StickyscrollValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn sticky_scroll_profile_default() {
        let profile = StickyScrollProfile::default();
        assert_eq!(profile.max_lines, 5);
        assert!(profile.enabled);
        assert_eq!(profile.strategy, StickyScrollStrategy::Indentation);
    }

    #[test]
    fn sticky_scroll_profile_apply() {
        let profile = StickyScrollProfile::new(3, false, StickyScrollStrategy::Scope);
        let mut widget = StickyScrollWidget::new(10);
        profile.apply(&mut widget);
        assert!(!widget.is_enabled());
        assert_eq!(widget.max_lines(), 3);
    }

    #[test]
    fn sticky_scroll_profile_display() {
        let profile = StickyScrollProfile::default();
        let s = profile.to_string();
        assert!(s.contains("max=5"));
        assert!(s.contains("enabled=true"));
    }

    #[test]
    fn sticky_scroll_compute_nested() {
        let lines = vec![
            "fn main() {",
            "    let x = 1;",
            "    if true {",
            "        println!();",
            "    }",
            "}",
        ];
        let refs: Vec<&str> = lines.iter().map(|s| *s).collect();
        let result = sticky_scroll_compute(&refs, 3, 5);
        assert!(!result.is_empty());
        assert!(result.iter().any(|l| l.text.contains("fn main")));
    }

    #[test]
    fn sticky_scroll_compute_empty() {
        let result = sticky_scroll_compute(&[], 0, 5);
        assert!(result.is_empty());
    }

    #[test]
    fn sticky_scroll_compute_max_depth() {
        let lines = vec![
            "level0",
            "    level1",
            "        level2",
            "            level3",
        ];
        let refs: Vec<&str> = lines.iter().map(|s| *s).collect();
        let result = sticky_scroll_compute(&refs, 3, 2);
        assert!(result.len() <= 2);
    }

    #[test]
    fn sticky_scroll_from_scopes_basic() {
        let scopes = vec![
            (1, 10, "fn main() {", 0),
            (3, 8, "if condition {", 1),
            (5, 7, "for x in items {", 2),
        ];
        let result = sticky_scroll_from_scopes(&scopes, 6, 5);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].nesting_level, 0);
        assert_eq!(result[2].nesting_level, 2);
    }

    #[test]
    fn sticky_scroll_from_scopes_limited() {
        let scopes = vec![
            (1, 10, "outer", 0),
            (2, 9, "middle", 1),
            (3, 8, "inner", 2),
        ];
        let result = sticky_scroll_from_scopes(&scopes, 5, 2);
        assert_eq!(result.len(), 2);
    }
}
