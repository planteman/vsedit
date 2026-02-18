//! Sticky scroll widget that pins nesting-context lines at the top of the editor viewport.

use std::collections::HashMap;
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


// ---------------------------------------------------------------------------
// StickyScrollLine helpers
// ---------------------------------------------------------------------------

impl StickyScrollLine {
    /// Create a new sticky scroll line.
    pub fn new(line_number: u32, text: impl Into<String>, nesting_level: u32) -> Self {
        Self {
            line_number,
            text: text.into(),
            nesting_level,
            collapsed: false,
        }
    }

    /// Mark this line as collapsed.
    pub fn collapsed(mut self) -> Self {
        self.collapsed = true;
        self
    }

    /// Returns the trimmed text (no leading whitespace).
    pub fn trimmed_text(&self) -> &str {
        self.text.trim_start()
    }

    /// Returns the visual indent width (4 spaces per level).
    pub fn indent_width(&self) -> usize {
        self.nesting_level as usize * 4
    }

    /// Returns true if this line is at the top level.
    pub fn is_top_level(&self) -> bool {
        self.nesting_level == 0
    }
}

impl Default for StickyScrollLine {
    fn default() -> Self {
        Self::new(0, "", 0)
    }
}

// ---------------------------------------------------------------------------
// StickyScrollConfig helpers
// ---------------------------------------------------------------------------

impl StickyScrollConfig {
    /// Create a config with default values.
    pub fn standard() -> Self {
        Self::default()
    }

    /// Create a minimal config (1 line max).
    pub fn minimal() -> Self {
        Self {
            enabled: true,
            max_line_count: 1,
            default_model: "outlineModel".to_string(),
        }
    }

    /// Create a disabled config.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            max_line_count: 0,
            default_model: "outlineModel".to_string(),
        }
    }

    /// Validate the config.
    pub fn validate(&self) -> Result<(), StickyScrollError> {
        if self.enabled && self.max_line_count == 0 {
            return Err(StickyScrollError::NoLinesAvailable);
        }
        Ok(())
    }
}

impl fmt::Display for StickyScrollConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.enabled {
            write!(f, "enabled (max {} lines)", self.max_line_count)
        } else {
            write!(f, "disabled")
        }
    }
}

// ---------------------------------------------------------------------------
// StickyScrollStrategy helpers
// ---------------------------------------------------------------------------

impl StickyScrollStrategy {
    /// Returns all strategy variants.
    pub fn all() -> &'static [StickyScrollStrategy] {
        &[StickyScrollStrategy::Indentation, StickyScrollStrategy::Scope]
    }

    /// Parse from a string.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "indentation" | "indent" => Some(Self::Indentation),
            "scope" | "ast" => Some(Self::Scope),
            _ => None,
        }
    }
}

impl Default for StickyScrollStrategy {
    fn default() -> Self {
        StickyScrollStrategy::Indentation
    }
}

// ---------------------------------------------------------------------------
// Sticky scroll analysis helpers
// ---------------------------------------------------------------------------

/// Summary of sticky scroll state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StickyScrollSummary {
    pub visible_lines: usize,
    pub max_nesting: u32,
    pub collapsed_count: usize,
    pub widget_enabled: bool,
}

impl StickyScrollSummary {
    /// Generate summary from a widget.
    pub fn from_widget(widget: &StickyScrollWidget) -> Self {
        let lines = widget.get_visible_sticky_lines();
        Self {
            visible_lines: lines.len(),
            max_nesting: lines.iter().map(|l| l.nesting_level).max().unwrap_or(0),
            collapsed_count: lines.iter().filter(|l| l.collapsed).count(),
            widget_enabled: widget.is_enabled(),
        }
    }
}

impl fmt::Display for StickyScrollSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} visible lines (max nesting {}, {} collapsed)",
            self.visible_lines, self.max_nesting, self.collapsed_count
        )
    }
}

/// Compute the nesting depth of a line based on indentation.
pub fn nesting_depth_from_indent(line: &str, indent_size: usize) -> u32 {
    if indent_size == 0 {
        return 0;
    }
    let leading_spaces = line.len() - line.trim_start().len();
    (leading_spaces / indent_size) as u32
}

/// Filter sticky scroll lines to only show up to max_depth nesting.
pub fn filter_by_max_depth(lines: &[StickyScrollLine], max_depth: u32) -> Vec<StickyScrollLine> {
    lines.iter()
        .filter(|l| l.nesting_level <= max_depth)
        .cloned()
        .collect()
}


// ---------------------------------------------------------------------------
// StickyScrollContext - nesting context at a cursor position
// ---------------------------------------------------------------------------

/// Represents the nesting context at a given cursor position.
#[derive(Debug, Clone, Default)]
pub struct StickyScrollContext {
    lines: Vec<StickyScrollLine>,
}

impl StickyScrollContext {
    /// Create a context from the given lines (assumed to be ordered from outermost to innermost).
    pub fn from_lines(lines: Vec<StickyScrollLine>) -> Self {
        Self { lines }
    }

    /// Create an empty context.
    pub fn empty() -> Self {
        Self { lines: Vec::new() }
    }

    /// The nesting depth (number of context lines).
    pub fn depth(&self) -> usize {
        self.lines.len()
    }

    /// The innermost (closest) parent context line.
    pub fn parent(&self) -> Option<&StickyScrollLine> {
        self.lines.last()
    }

    /// All ancestor context lines from outermost to innermost.
    pub fn ancestors(&self) -> &[StickyScrollLine] {
        &self.lines
    }

    /// Returns true if the context is empty (at the top level).
    pub fn is_top_level(&self) -> bool {
        self.lines.is_empty()
    }

    /// Get the context line at a specific depth.
    pub fn at_depth(&self, depth: usize) -> Option<&StickyScrollLine> {
        self.lines.get(depth)
    }

    /// Get the text of the outermost ancestor.
    pub fn outermost_text(&self) -> Option<&str> {
        self.lines.first().map(|l| l.text.as_str())
    }
}

impl fmt::Display for StickyScrollContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Context(depth={})", self.depth())
    }
}

// ---------------------------------------------------------------------------
// StickyScrollAnimator - smooth scrolling transitions
// ---------------------------------------------------------------------------

/// Animates smooth scrolling transitions for the sticky scroll widget.
#[derive(Debug, Clone)]
pub struct StickyScrollAnimator {
    start_offset: f64,
    target_offset: f64,
    current_offset: f64,
    duration_ms: u64,
    elapsed_ms: u64,
    complete: bool,
}

impl StickyScrollAnimator {
    /// Create a new animator that transitions from `start` to `target` over `duration_ms`.
    pub fn new(start_offset: f64, target_offset: f64, duration_ms: u64) -> Self {
        let dur = duration_ms.max(1);
        Self {
            start_offset,
            target_offset,
            current_offset: start_offset,
            duration_ms: dur,
            elapsed_ms: 0,
            complete: false,
        }
    }

    /// Start/reset the animation.
    pub fn start(&mut self) {
        self.elapsed_ms = 0;
        self.current_offset = self.start_offset;
        self.complete = false;
    }

    /// Advance the animation by `delta_ms` milliseconds.
    pub fn tick(&mut self, delta_ms: u64) {
        if self.complete {
            return;
        }
        self.elapsed_ms += delta_ms;
        if self.elapsed_ms >= self.duration_ms {
            self.current_offset = self.target_offset;
            self.complete = true;
        } else {
            let t = self.elapsed_ms as f64 / self.duration_ms as f64;
            // ease-out quadratic
            let eased = t * (2.0 - t);
            self.current_offset = self.start_offset + (self.target_offset - self.start_offset) * eased;
        }
    }

    /// Returns true if the animation is complete.
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Current animated offset.
    pub fn current_offset(&self) -> f64 {
        self.current_offset
    }

    /// Progress as a fraction from 0.0 to 1.0.
    pub fn progress(&self) -> f64 {
        if self.complete {
            1.0
        } else {
            (self.elapsed_ms as f64 / self.duration_ms as f64).min(1.0)
        }
    }
}

impl fmt::Display for StickyScrollAnimator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Animator({:.1} -> {:.1}, {:.0}%)",
            self.start_offset,
            self.target_offset,
            self.progress() * 100.0
        )
    }
}

// ---------------------------------------------------------------------------
// StickyScrollCache - caches computed sticky lines per scroll position
// ---------------------------------------------------------------------------

/// Caches computed sticky scroll lines for given scroll positions.
#[derive(Debug, Clone)]
pub struct StickyScrollCache {
    entries: HashMap<u32, Vec<StickyScrollLine>>,
    hits: u64,
    misses: u64,
    capacity: usize,
}

impl StickyScrollCache {
    /// Create a new cache with the given max capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            hits: 0,
            misses: 0,
            capacity: capacity.max(1),
        }
    }

    /// Get cached lines for a scroll position.
    pub fn get(&mut self, scroll_top: u32) -> Option<&[StickyScrollLine]> {
        if self.entries.contains_key(&scroll_top) {
            self.hits += 1;
            self.entries.get(&scroll_top).map(|v| v.as_slice())
        } else {
            self.misses += 1;
            None
        }
    }

    /// Store lines for a scroll position.
    pub fn put(&mut self, scroll_top: u32, lines: Vec<StickyScrollLine>) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&scroll_top) {
            // Evict the first key we find (simple eviction)
            if let Some(key) = self.entries.keys().next().copied() {
                self.entries.remove(&key);
            }
        }
        self.entries.insert(scroll_top, lines);
    }

    /// Invalidate all cached entries.
    pub fn invalidate(&mut self) {
        self.entries.clear();
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Cache hit rate as a fraction.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

impl fmt::Display for StickyScrollCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Cache({} entries, {:.1}% hit rate)",
            self.len(),
            self.hit_rate() * 100.0
        )
    }
}

// ---------------------------------------------------------------------------
// Sticky-scroll analysis utilities
// ---------------------------------------------------------------------------

/// Compute the total number of visible (non-collapsed) lines across all
/// sticky scroll lines.
pub fn visible_count(lines: &[StickyScrollLine]) -> usize {
    lines.iter().filter(|l| !l.collapsed).count()
}

/// Return the maximum nesting level present in the given lines.
pub fn max_nesting(lines: &[StickyScrollLine]) -> u32 {
    lines.iter().map(|l| l.nesting_level).max().unwrap_or(0)
}

/// Flatten a nested set of sticky scroll lines into a single string,
/// indenting each line by its nesting level (2 spaces per level).
pub fn flatten_to_string(lines: &[StickyScrollLine]) -> String {
    let mut buf = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            buf.push('\n');
        }
        for _ in 0..line.nesting_level {
            buf.push_str("  ");
        }
        buf.push_str(&line.text);
    }
    buf
}

/// Build a breadcrumb-style path from the current sticky scroll context.
/// E.g. `"class Foo > fn bar > if cond"`.
pub fn breadcrumb_path(lines: &[StickyScrollLine], separator: &str) -> String {
    lines
        .iter()
        .filter(|l| !l.collapsed)
        .map(|l| l.text.trim().to_string())
        .collect::<Vec<_>>()
        .join(separator)
}

/// Group sticky scroll lines by nesting level.
pub fn group_by_nesting(lines: &[StickyScrollLine]) -> std::collections::HashMap<u32, Vec<&StickyScrollLine>> {
    let mut map: std::collections::HashMap<u32, Vec<&StickyScrollLine>> = std::collections::HashMap::new();
    for line in lines {
        map.entry(line.nesting_level).or_default().push(line);
    }
    map
}

/// Create a StickyScrollLine from raw parts (convenience constructor).
pub fn make_line(line_number: u32, text: &str, nesting_level: u32) -> StickyScrollLine {
    StickyScrollLine {
        line_number,
        text: text.to_string(),
        nesting_level,
        collapsed: false,
    }
}

/// Filter out lines whose nesting level exceeds a maximum.
pub fn filter_by_max_nesting(lines: &[StickyScrollLine], max: u32) -> Vec<&StickyScrollLine> {
    lines.iter().filter(|l| l.nesting_level <= max).collect()
}

// ---------------------------------------------------------------------------
// StickyScrollNesting – tracks indentation levels for scope detection
// ---------------------------------------------------------------------------

/// Tracks indentation-based nesting levels for sticky scroll scope detection.
#[derive(Debug, Clone)]
pub struct StickyScrollNesting {
    /// Stack of (line_number, indentation_level) pairs representing open scopes.
    scope_stack: Vec<(u32, u32)>,
    /// Number of spaces that constitute one indentation level.
    indent_size: u32,
}

impl StickyScrollNesting {
    pub fn new(indent_size: u32) -> Self {
        Self {
            scope_stack: Vec::new(),
            indent_size: if indent_size == 0 { 4 } else { indent_size },
        }
    }

    /// Compute the indentation level of a line based on leading whitespace.
    pub fn indentation_level(&self, line: &str) -> u32 {
        let spaces: u32 = line
            .chars()
            .take_while(|c| *c == ' ')
            .count() as u32;
        spaces / self.indent_size
    }

    /// Push a scope-opening line onto the stack.
    pub fn push_scope(&mut self, line_number: u32, indent_level: u32) {
        self.scope_stack.push((line_number, indent_level));
    }

    /// Pop scopes that are at the same or deeper indentation than `current_level`.
    pub fn pop_to_level(&mut self, current_level: u32) -> Vec<(u32, u32)> {
        let mut popped = Vec::new();
        while let Some(&(_, lvl)) = self.scope_stack.last() {
            if lvl >= current_level {
                popped.push(self.scope_stack.pop().unwrap());
            } else {
                break;
            }
        }
        popped
    }

    /// Return the current nesting depth.
    pub fn depth(&self) -> usize {
        self.scope_stack.len()
    }

    /// Return the active scope lines (line numbers of all open scopes).
    pub fn active_scopes(&self) -> Vec<u32> {
        self.scope_stack.iter().map(|(ln, _)| *ln).collect()
    }

    /// Clear all tracked scopes.
    pub fn clear(&mut self) {
        self.scope_stack.clear();
    }

    /// Return scope stack entries.
    pub fn scope_entries(&self) -> &[(u32, u32)] {
        &self.scope_stack
    }
}

impl Default for StickyScrollNesting {
    fn default() -> Self {
        Self::new(4)
    }
}

impl fmt::Display for StickyScrollNesting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StickyScrollNesting(depth={}, indent={})", self.depth(), self.indent_size)
    }
}

// ---------------------------------------------------------------------------
// StickyScrollAnimation – smooth transitions for sticky scroll area
// ---------------------------------------------------------------------------

/// Represents the animation state for sticky scroll transitions.
#[derive(Debug, Clone)]
pub struct StickyScrollAnimation {
    /// Current scroll offset in pixels (fractional for smooth scrolling).
    pub current_offset: f64,
    /// Target scroll offset.
    pub target_offset: f64,
    /// Animation duration in milliseconds.
    pub duration_ms: u32,
    /// Elapsed time in milliseconds.
    pub elapsed_ms: u32,
    /// Whether the animation is currently running.
    pub running: bool,
}

impl StickyScrollAnimation {
    pub fn new(duration_ms: u32) -> Self {
        Self {
            current_offset: 0.0,
            target_offset: 0.0,
            duration_ms,
            elapsed_ms: 0,
            running: false,
        }
    }

    /// Start an animation to the given target offset.
    pub fn animate_to(&mut self, target: f64) {
        if (self.current_offset - target).abs() < 0.001 {
            return;
        }
        self.target_offset = target;
        self.elapsed_ms = 0;
        self.running = true;
    }

    /// Advance the animation by `delta_ms` milliseconds. Returns the new offset.
    pub fn tick(&mut self, delta_ms: u32) -> f64 {
        if !self.running {
            return self.current_offset;
        }
        self.elapsed_ms += delta_ms;
        if self.elapsed_ms >= self.duration_ms {
            self.current_offset = self.target_offset;
            self.running = false;
        } else {
            let t = self.elapsed_ms as f64 / self.duration_ms as f64;
            // ease-out cubic: 1 - (1-t)^3
            let eased = 1.0 - (1.0 - t).powi(3);
            let start = self.current_offset;
            self.current_offset = start + (self.target_offset - start) * eased;
        }
        self.current_offset
    }

    /// Whether the animation has completed.
    pub fn is_complete(&self) -> bool {
        !self.running
    }

    /// Reset animation state to zero.
    pub fn reset(&mut self) {
        self.current_offset = 0.0;
        self.target_offset = 0.0;
        self.elapsed_ms = 0;
        self.running = false;
    }

    /// Progress as a fraction 0.0..=1.0.
    pub fn progress(&self) -> f64 {
        if !self.running {
            return 1.0;
        }
        if self.duration_ms == 0 {
            return 1.0;
        }
        (self.elapsed_ms as f64 / self.duration_ms as f64).min(1.0)
    }
}

impl Default for StickyScrollAnimation {
    fn default() -> Self {
        Self::new(150)
    }
}

impl fmt::Display for StickyScrollAnimation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Animation(offset={:.1}, target={:.1}, {})",
            self.current_offset,
            self.target_offset,
            if self.running { "running" } else { "idle" }
        )
    }
}

// ---------------------------------------------------------------------------
// Partial line display – shows a clipped portion of a sticky line
// ---------------------------------------------------------------------------

/// A partial view of a sticky scroll line for rendering.
#[derive(Debug, Clone, PartialEq)]
pub struct StickyScrollPartialLine {
    pub line_number: u32,
    pub visible_text: String,
    pub is_truncated: bool,
    pub visible_fraction: f64,
}

impl StickyScrollPartialLine {
    /// Create a partial line by clipping text to `max_chars`.
    pub fn from_line(line: &StickyScrollLine, max_chars: usize) -> Self {
        let text = &line.text;
        if text.len() <= max_chars {
            Self {
                line_number: line.line_number,
                visible_text: text.clone(),
                is_truncated: false,
                visible_fraction: 1.0,
            }
        } else {
            Self {
                line_number: line.line_number,
                visible_text: format!("{}…", &text[..max_chars]),
                is_truncated: true,
                visible_fraction: max_chars as f64 / text.len() as f64,
            }
        }
    }
}

impl fmt::Display for StickyScrollPartialLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "L{}: {}", self.line_number, self.visible_text)
    }
}

// ---------------------------------------------------------------------------
// Context-based scope detection
// ---------------------------------------------------------------------------

/// Detects scope boundaries in source code based on simple heuristics.
#[derive(Debug, Clone)]
pub struct ScopeDetector {
    /// Keywords that open a new scope (e.g., "fn", "class", "if").
    pub scope_keywords: Vec<String>,
}

impl ScopeDetector {
    pub fn new(keywords: Vec<String>) -> Self {
        Self { scope_keywords: keywords }
    }

    /// Default scope detector for Rust-like languages.
    pub fn rust_default() -> Self {
        Self::new(vec![
            "fn".into(), "struct".into(), "enum".into(),
            "impl".into(), "mod".into(), "trait".into(),
            "if".into(), "for".into(), "while".into(), "loop".into(),
        ])
    }

    /// Check if a line opens a new scope by containing a keyword followed by content.
    pub fn is_scope_opener(&self, line: &str) -> bool {
        let trimmed = line.trim();
        self.scope_keywords.iter().any(|kw| {
            trimmed.starts_with(kw.as_str())
                && trimmed.len() > kw.len()
                && trimmed.as_bytes().get(kw.len()).map_or(false, |b| *b == b' ' || *b == b'(')
        })
    }

    /// Detect all scope-opening lines in a document and return their line numbers (1-based).
    pub fn detect_scopes(&self, lines: &[&str]) -> Vec<u32> {
        lines.iter().enumerate()
            .filter(|(_, l)| self.is_scope_opener(l))
            .map(|(i, _)| (i + 1) as u32)
            .collect()
    }

    /// Build sticky scroll lines from detected scopes.
    pub fn build_sticky_lines(&self, lines: &[&str], nesting: &StickyScrollNesting) -> Vec<StickyScrollLine> {
        let mut result = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if self.is_scope_opener(line) {
                let level = nesting.indentation_level(line);
                result.push(StickyScrollLine {
                    line_number: (i + 1) as u32,
                    text: line.trim().to_string(),
                    nesting_level: level,
                    collapsed: false,
                });
            }
        }
        result
    }
}

impl Default for ScopeDetector {
    fn default() -> Self {
        Self::rust_default()
    }
}

impl fmt::Display for ScopeDetector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ScopeDetector({} keywords)", self.scope_keywords.len())
    }
}


// ---------------------------------------------------------------------------
// StickyScrollScopeDetectorEngine
// ---------------------------------------------------------------------------

/// A detected scope in source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedScope {
    pub start_line: u32,
    pub end_line: u32,
    pub nesting: u32,
    pub kind: ScopeKind,
    pub header_text: String,
}

/// Kind of detected scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Function,
    Block,
    Class,
    Module,
    Loop,
    Conditional,
    Unknown,
}

impl fmt::Display for ScopeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Function => write!(f, "function"),
            Self::Block => write!(f, "block"),
            Self::Class => write!(f, "class"),
            Self::Module => write!(f, "module"),
            Self::Loop => write!(f, "loop"),
            Self::Conditional => write!(f, "conditional"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

impl DetectedScope {
    /// Number of lines in this scope.
    pub fn line_count(&self) -> u32 {
        self.end_line.saturating_sub(self.start_line) + 1
    }

    /// Whether a given line is inside this scope (inclusive).
    pub fn contains_line(&self, line: u32) -> bool {
        line >= self.start_line && line <= self.end_line
    }
}

/// Engine that detects code scopes for sticky scroll display.
///
/// Uses indentation-based heuristics (suitable for Python-like languages)
/// and brace-based heuristics (suitable for C-like languages).
#[derive(Debug)]
pub struct StickyScrollScopeDetectorEngine {
    scopes: Vec<DetectedScope>,
    indent_width: u32,
    max_nesting: u32,
}

impl StickyScrollScopeDetectorEngine {
    pub fn new(indent_width: u32, max_nesting: u32) -> Self {
        Self {
            scopes: Vec::new(),
            indent_width,
            max_nesting,
        }
    }

    /// Count leading spaces of a line.
    fn leading_spaces(line: &str) -> u32 {
        line.chars().take_while(|c| *c == ' ').count() as u32
    }

    /// Detect scopes using indentation heuristics.
    pub fn detect_by_indentation(&mut self, source: &str) {
        let lines: Vec<&str> = source.lines().collect();
        let mut scope_stack: Vec<(u32, u32, String)> = Vec::new(); // (indent, start_line, header)

        for (i, &line) in lines.iter().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let indent = Self::leading_spaces(line);
            let nesting = if self.indent_width > 0 { indent / self.indent_width } else { 0 };

            // Close scopes that have ended
            while let Some(&(scope_indent, start, _)) = scope_stack.last() {
                if indent <= scope_indent && i as u32 > start {
                    let (si, sl, header) = scope_stack.pop().unwrap();
                    let scope_nesting = if self.indent_width > 0 { si / self.indent_width } else { 0 };
                    if scope_nesting <= self.max_nesting {
                        self.scopes.push(DetectedScope {
                            start_line: sl,
                            end_line: i as u32 - 1,
                            nesting: scope_nesting,
                            kind: Self::guess_kind(&header),
                            header_text: header,
                        });
                    }
                } else {
                    break;
                }
            }

            // If next line has more indent, this starts a scope
            if i + 1 < lines.len() {
                let next = lines[i + 1];
                if !next.trim().is_empty() && Self::leading_spaces(next) > indent {
                    scope_stack.push((indent, i as u32, line.trim().to_string()));
                }
            }
        }
        // Close remaining scopes
        let total_lines = lines.len() as u32;
        while let Some((si, sl, header)) = scope_stack.pop() {
            let scope_nesting = if self.indent_width > 0 { si / self.indent_width } else { 0 };
            if scope_nesting <= self.max_nesting {
                self.scopes.push(DetectedScope {
                    start_line: sl,
                    end_line: total_lines.saturating_sub(1),
                    nesting: scope_nesting,
                    kind: Self::guess_kind(&header),
                    header_text: header,
                });
            }
        }
    }

    /// Guess the scope kind from the header line.
    fn guess_kind(header: &str) -> ScopeKind {
        let trimmed = header.trim();
        if trimmed.starts_with("fn ") || trimmed.starts_with("def ") || trimmed.starts_with("func ") {
            ScopeKind::Function
        } else if trimmed.starts_with("class ") || trimmed.starts_with("struct ") {
            ScopeKind::Class
        } else if trimmed.starts_with("mod ") {
            ScopeKind::Module
        } else if trimmed.starts_with("for ") || trimmed.starts_with("while ") || trimmed.starts_with("loop") {
            ScopeKind::Loop
        } else if trimmed.starts_with("if ") || trimmed.starts_with("else") || trimmed.starts_with("match ") {
            ScopeKind::Conditional
        } else {
            ScopeKind::Unknown
        }
    }

    /// Get all detected scopes.
    pub fn scopes(&self) -> &[DetectedScope] {
        &self.scopes
    }

    /// Get scopes active at a particular line.
    pub fn scopes_at_line(&self, line: u32) -> Vec<&DetectedScope> {
        self.scopes.iter().filter(|s| s.contains_line(line)).collect()
    }

    /// Clear all detected scopes.
    pub fn clear(&mut self) {
        self.scopes.clear();
    }
}

// ---------------------------------------------------------------------------
// StickyScrollAnimationController
// ---------------------------------------------------------------------------

/// Easing function used for scroll animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EasingFunction {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl EasingFunction {
    /// Compute the eased value for `t` in `[0.0, 1.0]`.
    pub fn apply(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => t * (2.0 - t),
            Self::EaseInOut => {
                if t < 0.5 { 2.0 * t * t } else { -1.0 + (4.0 - 2.0 * t) * t }
            }
        }
    }
}

/// Controls scroll animation timing and easing for sticky scroll transitions.
#[derive(Debug)]
pub struct StickyScrollAnimationController {
    easing: EasingFunction,
    duration_ms: u64,
    current_progress: f64,
    start_offset: f64,
    target_offset: f64,
    is_running: bool,
    elapsed_ms: u64,
}

impl StickyScrollAnimationController {
    pub fn new(easing: EasingFunction, duration_ms: u64) -> Self {
        Self {
            easing,
            duration_ms,
            current_progress: 0.0,
            start_offset: 0.0,
            target_offset: 0.0,
            is_running: false,
            elapsed_ms: 0,
        }
    }

    /// Start an animation from `start` to `target`.
    pub fn start(&mut self, start: f64, target: f64) {
        self.start_offset = start;
        self.target_offset = target;
        self.current_progress = 0.0;
        self.elapsed_ms = 0;
        self.is_running = true;
    }

    /// Advance the animation by `delta_ms` milliseconds. Returns the current offset.
    pub fn tick(&mut self, delta_ms: u64) -> f64 {
        if !self.is_running {
            return self.target_offset;
        }
        self.elapsed_ms += delta_ms;
        if self.elapsed_ms >= self.duration_ms {
            self.is_running = false;
            self.current_progress = 1.0;
            return self.target_offset;
        }
        let t = self.elapsed_ms as f64 / self.duration_ms as f64;
        self.current_progress = self.easing.apply(t);
        self.start_offset + (self.target_offset - self.start_offset) * self.current_progress
    }

    /// Current interpolated offset.
    pub fn current_offset(&self) -> f64 {
        self.start_offset + (self.target_offset - self.start_offset) * self.current_progress
    }

    /// Whether the animation is currently running.
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// Cancel the animation, snapping to the target.
    pub fn cancel(&mut self) {
        self.is_running = false;
        self.current_progress = 1.0;
    }

    /// Reset to idle state.
    pub fn reset(&mut self) {
        self.is_running = false;
        self.current_progress = 0.0;
        self.elapsed_ms = 0;
        self.start_offset = 0.0;
        self.target_offset = 0.0;
    }
}



// ---------------------------------------------------------------------------
// stickyscroll – Editor text helpers
// ---------------------------------------------------------------------------

/// A half-open range within a document `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XStickyscrollTextSpan {
    pub start: usize,
    pub end: usize,
}

impl XStickyscrollTextSpan {
    pub fn new(start: usize, end: usize) -> Self {
        let (s, e) = if start <= end { (start, end) } else { (end, start) };
        Self { start: s, end: e }
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Extract the spanned slice from `text`.
    pub fn extract<'a>(&self, text: &'a str) -> &'a str {
        &text[self.start..self.end]
    }

    /// Returns true if `pos` is contained within this span.
    pub fn contains(&self, pos: usize) -> bool {
        pos >= self.start && pos < self.end
    }

    /// Returns the overlap with `other`, if any.
    pub fn intersect(&self, other: &Self) -> Option<Self> {
        let s = self.start.max(other.start);
        let e = self.end.min(other.end);
        if s < e { Some(Self { start: s, end: e }) } else { None }
    }

    /// Merge two spans into the smallest enclosing span.
    pub fn union(&self, other: &Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    /// Shift the span by `delta` positions to the right.
    pub fn shift(&self, delta: usize) -> Self {
        Self { start: self.start + delta, end: self.end + delta }
    }
}

/// Count the number of lines in `text`.
pub fn x_stickyscroll_count_lines(text: &str) -> usize {
    if text.is_empty() { return 0; }
    text.lines().count()
}

/// Return the byte offset of the start of line `n` (0-based).
pub fn x_stickyscroll_line_start_offset(text: &str, line: usize) -> Option<usize> {
    let mut current = 0usize;
    for (i, l) in text.split('\n').enumerate() {
        if i == line { return Some(current); }
        current += l.len() + 1;
    }
    None
}

/// Compute the indentation level (number of leading spaces) of a line.
pub fn x_stickyscroll_indent_level(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

/// Trim trailing whitespace from every line in `text`.
pub fn x_stickyscroll_trim_trailing(text: &str) -> String {
    text.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Detect the dominant line ending in `text` (`"\n"` or `"\r\n"`).
pub fn x_stickyscroll_detect_eol(text: &str) -> &'static str {
    let crlf = text.matches("\r\n").count();
    let lf = text.matches('\n').count().saturating_sub(crlf);
    if crlf > lf { "\r\n" } else { "\n" }
}

/// Simple word-boundary based tokenizer: split on whitespace and punctuation.
pub fn x_stickyscroll_tokenize(text: &str) -> Vec<&str> {
    text.split(|c: char| c.is_whitespace() || ".,;:!?()[]{}".contains(c))
        .filter(|s| !s.is_empty())
        .collect()
}



// ---------------------------------------------------------------------------
// stickyscroll – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for sticky scroll context lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YStickyscrollStickyScrollAnimation {
    None,
    FadeIn,
    SlideDown,
    Instant,
}

impl YStickyscrollStickyScrollAnimation {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::None => 0,
            Self::FadeIn => 1,
            Self::SlideDown => 2,
            Self::Instant => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::FadeIn => "FadeIn",
            Self::SlideDown => "SlideDown",
            Self::Instant => "Instant",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YStickyscrollStickyScrollAnimation] {
        &[
            YStickyscrollStickyScrollAnimation::None,
            YStickyscrollStickyScrollAnimation::FadeIn,
            YStickyscrollStickyScrollAnimation::SlideDown,
            YStickyscrollStickyScrollAnimation::Instant,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YStickyscrollStickyScrollAnimation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks scroll state data.
#[derive(Debug, Clone)]
pub struct YStickyscrollStickyScrollState {
    pub visible_count: usize,
    pub max_depth: u32,
    pub frozen: bool,
}

impl YStickyscrollStickyScrollState {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            visible_count: 0,
            max_depth: 0,
            frozen: false,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YStickyscrollStickyScrollState({}: {:?})", "visible_count", self.visible_count)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_stickyscroll_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_stickyscroll_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_stickyscroll_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_stickyscroll_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_stickyscroll_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_stickyscroll_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_stickyscroll_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_stickyscroll_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// stickyscroll – Extended sticky scroll metrics helpers
// ---------------------------------------------------------------------------

/// Priority levels for sticky scroll metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZStickyscrollPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZStickyscrollPriority {
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
    pub fn all_asc() -> [ZStickyscrollPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZStickyscrollPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks sticky scroll metrics data.
#[derive(Debug, Clone)]
pub struct ZStickyscrollStickyScrollMetrics {
    pub render_times_us: Vec<u64>,
    pub frame_count: u64,
    pub jank_detected: bool,
}

impl ZStickyscrollStickyScrollMetrics {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            render_times_us: Vec::new(),
            frame_count: 0,
            jank_detected: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.render_times_us.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.render_times_us.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.render_times_us.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZStickyscrollStickyScrollMetrics[frame_count={:?}, jank_detected={:?}]", self.frame_count, self.jank_detected)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.jank_detected = !c.jank_detected;
        c
    }
}

/// Compute a simple rolling hash for sticky scroll metrics.
pub fn z_stickyscroll_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_stickyscroll_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_stickyscroll_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_stickyscroll_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_stickyscroll_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_stickyscroll_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_stickyscroll_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 38
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer38 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer38 {
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
pub fn xb_fnv1a_38(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_38<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_38<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_38(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_38(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 165
// ---------------------------------------------------------------------------

/// Generic object pool `Xc165Pool<T>`.
pub struct Xc165Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc165Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc165PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc165Pool<T> {
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
    pub fn stats(&self) -> Xc165PoolStats {
        Xc165PoolStats {
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

impl<T> Default for Xc165Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc165Scheduler`.
pub struct Xc165Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc165Scheduler {
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

impl Default for Xc165Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_165 hash for the given byte slice.
pub fn xc_165_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_165 convention.
pub fn xc_165_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe51 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe51Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe51PipelineError {
    pub stage: Xe51Stage,
    pub message: String,
}

impl std::fmt::Display for Xe51PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe51Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe51Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe51PipelineError>>>,
    stage_names: Vec<Xe51Stage>,
}

impl Xe51Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe51PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe51Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe51PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe51Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe51PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe51Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe51PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe51Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe51PipelineError> {
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

    pub fn compose(mut self, other: Xe51Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe51CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe51CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe51Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe51CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe51CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe51Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe51CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_51_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe51CacheEntry {
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

    fn xe_51_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe51CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_51_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe51PipelineError> {
    Ok(data)
}

pub fn xe_51_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe51PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_51_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe51PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_51_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe51PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_51_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe51PipelineError> {
    Err(Xe51PipelineError {
        stage: Xe51Stage::Parse,
        message: "intentional failure".to_string(),
    })
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

    #[test]
    fn test_sticky_line_new() {
        let line = StickyScrollLine::new(10, "fn main() {", 0);
        assert_eq!(line.line_number, 10);
        assert!(line.is_top_level());
        assert!(!line.collapsed);
        assert_eq!(line.indent_width(), 0);
    }

    #[test]
    fn test_sticky_line_collapsed() {
        let line = StickyScrollLine::new(5, "if cond {", 1).collapsed();
        assert!(line.collapsed);
        assert_eq!(line.nesting_level, 1);
    }

    #[test]
    fn test_sticky_line_trimmed_text() {
        let line = StickyScrollLine::new(1, "    fn foo()", 1);
        assert_eq!(line.trimmed_text(), "fn foo()");
    }

    #[test]
    fn test_sticky_line_default() {
        let line = StickyScrollLine::default();
        assert_eq!(line.line_number, 0);
        assert!(line.text.is_empty());
    }

    #[test]
    fn test_sticky_config_presets() {
        let std = StickyScrollConfig::standard();
        assert!(std.enabled);
        let min = StickyScrollConfig::minimal();
        assert_eq!(min.max_line_count, 1);
        let dis = StickyScrollConfig::disabled();
        assert!(!dis.enabled);
    }

    #[test]
    fn test_sticky_config_display() {
        let c = StickyScrollConfig::standard();
        let s = format!("{c}");
        assert!(s.contains("enabled"));
    }

    #[test]
    fn test_sticky_config_validate() {
        let bad = StickyScrollConfig { enabled: true, max_line_count: 0, default_model: "outlineModel".to_string() };
        assert!(bad.validate().is_err());
        let good = StickyScrollConfig::standard();
        assert!(good.validate().is_ok());
    }

    #[test]
    fn test_sticky_strategy_all_and_from_name() {
        assert_eq!(StickyScrollStrategy::all().len(), 2);
        assert_eq!(StickyScrollStrategy::from_name("indent"), Some(StickyScrollStrategy::Indentation));
        assert_eq!(StickyScrollStrategy::from_name("scope"), Some(StickyScrollStrategy::Scope));
        assert_eq!(StickyScrollStrategy::from_name("bogus"), None);
        assert_eq!(StickyScrollStrategy::default(), StickyScrollStrategy::Indentation);
    }

    #[test]
    fn test_nesting_depth_from_indent() {
        assert_eq!(nesting_depth_from_indent("        x", 4), 2);
        assert_eq!(nesting_depth_from_indent("x", 4), 0);
    }

    #[test]
    fn test_filter_by_max_depth() {
        let lines = vec![
            StickyScrollLine::new(1, "fn main() {", 0),
            StickyScrollLine::new(2, "if cond {", 1),
            StickyScrollLine::new(3, "nested", 2),
        ];
        let filtered = filter_by_max_depth(&lines, 1);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn sticky_scroll_context_from_lines() {
        let lines = vec![
            StickyScrollLine::new(1, "fn main() {", 0),
            StickyScrollLine::new(5, "  if true {", 1),
            StickyScrollLine::new(10, "    for x in items {", 2),
        ];
        let ctx = StickyScrollContext::from_lines(lines);
        assert_eq!(ctx.depth(), 3);
        assert!(!ctx.is_top_level());
        assert_eq!(ctx.parent().unwrap().nesting_level, 2);
        assert_eq!(ctx.outermost_text(), Some("fn main() {"));
        assert_eq!(ctx.at_depth(1).unwrap().line_number, 5);
    }

    #[test]
    fn sticky_scroll_context_empty() {
        let ctx = StickyScrollContext::empty();
        assert!(ctx.is_top_level());
        assert_eq!(ctx.depth(), 0);
        assert!(ctx.parent().is_none());
        assert!(ctx.outermost_text().is_none());
    }

    #[test]
    fn sticky_scroll_animator_transitions() {
        let mut anim = StickyScrollAnimator::new(0.0, 100.0, 200);
        assert!(!anim.is_complete());
        assert_eq!(anim.current_offset(), 0.0);
        anim.tick(100);
        assert!(!anim.is_complete());
        assert!(anim.current_offset() > 0.0);
        assert!(anim.current_offset() < 100.0);
        anim.tick(200);
        assert!(anim.is_complete());
        assert!((anim.current_offset() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn sticky_scroll_animator_restart() {
        let mut anim = StickyScrollAnimator::new(0.0, 50.0, 100);
        anim.tick(100);
        assert!(anim.is_complete());
        anim.start();
        assert!(!anim.is_complete());
        assert_eq!(anim.current_offset(), 0.0);
    }

    #[test]
    fn sticky_scroll_cache_hit_miss() {
        let mut cache = StickyScrollCache::new(10);
        assert!(cache.is_empty());
        cache.put(0, vec![StickyScrollLine::new(1, "fn main() {", 0)]);
        assert_eq!(cache.len(), 1);
        assert!(cache.get(0).is_some());
        assert!(cache.get(99).is_none());
        assert!(cache.hit_rate() > 0.0);
        cache.invalidate();
        assert!(cache.is_empty());
    }

    #[test]
    fn sticky_scroll_cache_eviction() {
        let mut cache = StickyScrollCache::new(2);
        cache.put(0, vec![]);
        cache.put(1, vec![]);
        assert_eq!(cache.len(), 2);
        cache.put(2, vec![]);
        assert_eq!(cache.len(), 2); // one was evicted
    }

    // -- visible_count ---------------------------------------------------------

    #[test]
    fn visible_count_all() {
        let lines = vec![make_line(1, "fn main", 0), make_line(2, "if x", 1)];
        assert_eq!(visible_count(&lines), 2);
    }

    #[test]
    fn visible_count_with_collapsed() {
        let mut lines = vec![make_line(1, "fn main", 0), make_line(2, "if x", 1)];
        lines[1].collapsed = true;
        assert_eq!(visible_count(&lines), 1);
    }

    // -- max_nesting -----------------------------------------------------------

    #[test]
    fn max_nesting_basic() {
        let lines = vec![make_line(1, "a", 0), make_line(2, "b", 3), make_line(3, "c", 2)];
        assert_eq!(max_nesting(&lines), 3);
    }

    #[test]
    fn max_nesting_empty() {
        let lines: Vec<StickyScrollLine> = vec![];
        assert_eq!(max_nesting(&lines), 0);
    }

    // -- flatten_to_string -----------------------------------------------------

    #[test]
    fn flatten_to_string_indents() {
        let lines = vec![make_line(1, "fn main", 0), make_line(2, "if x", 1)];
        let flat = flatten_to_string(&lines);
        assert_eq!(flat, "fn main\n  if x");
    }

    // -- breadcrumb_path -------------------------------------------------------

    #[test]
    fn breadcrumb_path_basic() {
        let lines = vec![make_line(1, "class Foo", 0), make_line(2, "fn bar", 1)];
        assert_eq!(breadcrumb_path(&lines, " > "), "class Foo > fn bar");
    }

    // -- group_by_nesting ------------------------------------------------------

    #[test]
    fn group_by_nesting_groups() {
        let lines = vec![make_line(1, "a", 0), make_line(2, "b", 1), make_line(3, "c", 0)];
        let groups = group_by_nesting(&lines);
        assert_eq!(groups[&0].len(), 2);
        assert_eq!(groups[&1].len(), 1);
    }

    // -- filter_by_max_nesting -------------------------------------------------

    #[test]
    fn filter_by_max_nesting_filters() {
        let lines = vec![make_line(1, "a", 0), make_line(2, "b", 2), make_line(3, "c", 1)];
        let filtered = filter_by_max_nesting(&lines, 1);
        assert_eq!(filtered.len(), 2);
    }

    // -- StickyScrollNesting -----------------------------------------------

    #[test]
    fn nesting_indentation_level() {
        let n = StickyScrollNesting::new(4);
        assert_eq!(n.indentation_level("hello"), 0);
        assert_eq!(n.indentation_level("    hello"), 1);
        assert_eq!(n.indentation_level("        hello"), 2);
        assert_eq!(n.indentation_level("  hello"), 0); // 2 spaces < 4
    }

    #[test]
    fn nesting_push_and_pop_scopes() {
        let mut n = StickyScrollNesting::new(4);
        n.push_scope(1, 0);
        n.push_scope(5, 1);
        n.push_scope(10, 2);
        assert_eq!(n.depth(), 3);

        let popped = n.pop_to_level(1);
        assert_eq!(popped.len(), 2);
        assert_eq!(n.depth(), 1);
    }

    #[test]
    fn nesting_active_scopes() {
        let mut n = StickyScrollNesting::new(4);
        n.push_scope(1, 0);
        n.push_scope(5, 1);
        assert_eq!(n.active_scopes(), vec![1, 5]);
    }

    #[test]
    fn nesting_clear() {
        let mut n = StickyScrollNesting::new(4);
        n.push_scope(1, 0);
        n.clear();
        assert_eq!(n.depth(), 0);
    }

    #[test]
    fn nesting_default_indent_size() {
        let n = StickyScrollNesting::default();
        assert_eq!(n.indentation_level("        x"), 2);
        assert_eq!(format!("{n}"), "StickyScrollNesting(depth=0, indent=4)");
    }

    #[test]
    fn nesting_zero_indent_uses_default() {
        let n = StickyScrollNesting::new(0);
        assert_eq!(n.indentation_level("    x"), 1);
    }

    // -- StickyScrollAnimation ---------------------------------------------

    #[test]
    fn animation_idle_by_default() {
        let a = StickyScrollAnimation::default();
        assert!(a.is_complete());
        assert_eq!(a.progress(), 1.0);
    }

    #[test]
    fn animation_animate_and_tick() {
        let mut a = StickyScrollAnimation::new(100);
        a.animate_to(50.0);
        assert!(!a.is_complete());
        a.tick(50);
        assert!(a.current_offset > 0.0);
        assert!(a.current_offset < 50.0);
        a.tick(100);
        assert!(a.is_complete());
        assert!((a.current_offset - 50.0).abs() < 0.001);
    }

    #[test]
    fn animation_no_op_when_already_at_target() {
        let mut a = StickyScrollAnimation::new(100);
        a.animate_to(0.0);
        assert!(a.is_complete());
    }

    #[test]
    fn animation_reset() {
        let mut a = StickyScrollAnimation::new(100);
        a.animate_to(50.0);
        a.tick(30);
        a.reset();
        assert!(a.is_complete());
        assert_eq!(a.current_offset, 0.0);
    }

    #[test]
    fn animation_display() {
        let a = StickyScrollAnimation::new(100);
        let s = format!("{a}");
        assert!(s.contains("idle"));
    }

    // -- StickyScrollPartialLine -------------------------------------------

    #[test]
    fn partial_line_no_truncation() {
        let line = make_line(1, "short", 0);
        let partial = StickyScrollPartialLine::from_line(&line, 100);
        assert!(!partial.is_truncated);
        assert!((partial.visible_fraction - 1.0).abs() < 0.001);
    }

    #[test]
    fn partial_line_truncated() {
        let line = make_line(1, "a very long line of text here", 0);
        let partial = StickyScrollPartialLine::from_line(&line, 10);
        assert!(partial.is_truncated);
        assert!(partial.visible_text.ends_with('…'));
        assert!(partial.visible_fraction < 1.0);
    }

    // -- ScopeDetector -----------------------------------------------------

    #[test]
    fn scope_detector_rust_default() {
        let det = ScopeDetector::rust_default();
        assert!(det.is_scope_opener("fn main() {"));
        assert!(det.is_scope_opener("struct Foo {"));
        assert!(!det.is_scope_opener("let x = 5;"));
        assert!(!det.is_scope_opener("// fn comment"));
    }

    #[test]
    fn scope_detector_detect_scopes() {
        let det = ScopeDetector::rust_default();
        let lines = vec![
            "fn main() {",
            "    let x = 1;",
            "    if x > 0 {",
            "    }",
            "}",
        ];
        let scopes = det.detect_scopes(&lines);
        assert_eq!(scopes, vec![1, 3]);
    }

    #[test]
    fn scope_detector_build_sticky_lines() {
        let det = ScopeDetector::rust_default();
        let n = StickyScrollNesting::new(4);
        let lines = vec![
            "fn outer() {",
            "    fn inner() {",
            "    }",
            "}",
        ];
        let sticky = det.build_sticky_lines(&lines, &n);
        assert_eq!(sticky.len(), 2);
        assert_eq!(sticky[0].nesting_level, 0);
        assert_eq!(sticky[1].nesting_level, 1);
    }

    #[test]
    fn scope_detector_display() {
        let det = ScopeDetector::default();
        let s = format!("{det}");
        assert!(s.contains("keywords"));
    }

    // -- StickyScrollScopeDetectorEngine tests --------------------------------

    #[test]
    fn scope_detect_simple_indentation() {
        let mut engine = StickyScrollScopeDetectorEngine::new(4, 10);
        let source = "def foo():\n    body\n    more\nend";
        engine.detect_by_indentation(source);
        assert!(!engine.scopes().is_empty());
        assert_eq!(engine.scopes()[0].kind, ScopeKind::Function);
    }

    #[test]
    fn scope_contains_line() {
        let s = DetectedScope {
            start_line: 5,
            end_line: 10,
            nesting: 0,
            kind: ScopeKind::Block,
            header_text: "block".into(),
        };
        assert!(s.contains_line(5));
        assert!(s.contains_line(10));
        assert!(!s.contains_line(11));
    }

    #[test]
    fn scope_line_count() {
        let s = DetectedScope {
            start_line: 3,
            end_line: 7,
            nesting: 0,
            kind: ScopeKind::Function,
            header_text: "fn foo".into(),
        };
        assert_eq!(s.line_count(), 5);
    }

    #[test]
    fn scope_kind_display() {
        assert_eq!(ScopeKind::Function.to_string(), "function");
        assert_eq!(ScopeKind::Class.to_string(), "class");
        assert_eq!(ScopeKind::Unknown.to_string(), "unknown");
    }

    #[test]
    fn scope_scopes_at_line() {
        let mut engine = StickyScrollScopeDetectorEngine::new(4, 10);
        let source = "if cond:\n    inner\n    more\noutside";
        engine.detect_by_indentation(source);
        let at_1 = engine.scopes_at_line(1);
        assert!(!at_1.is_empty());
    }

    #[test]
    fn scope_clear() {
        let mut engine = StickyScrollScopeDetectorEngine::new(4, 10);
        engine.detect_by_indentation("def foo():\n    body");
        assert!(!engine.scopes().is_empty());
        engine.clear();
        assert!(engine.scopes().is_empty());
    }

    // -- StickyScrollAnimationController tests --------------------------------

    #[test]
    fn animation_linear_halfway() {
        let mut c = StickyScrollAnimationController::new(EasingFunction::Linear, 100);
        c.start(0.0, 100.0);
        let val = c.tick(50);
        assert!((val - 50.0).abs() < 0.01);
    }

    #[test]
    fn animation_completes() {
        let mut c = StickyScrollAnimationController::new(EasingFunction::Linear, 100);
        c.start(0.0, 200.0);
        let val = c.tick(150);
        assert!((val - 200.0).abs() < 0.01);
        assert!(!c.is_running());
    }

    #[test]
    fn animation_cancel() {
        let mut c = StickyScrollAnimationController::new(EasingFunction::EaseIn, 100);
        c.start(0.0, 50.0);
        c.tick(10);
        c.cancel();
        assert!(!c.is_running());
        assert!((c.current_offset() - 50.0).abs() < 0.01);
    }

    #[test]
    fn animation_controller_reset() {
        let mut c = StickyScrollAnimationController::new(EasingFunction::Linear, 100);
        c.start(10.0, 90.0);
        c.tick(50);
        c.reset();
        assert!(!c.is_running());
        assert!((c.current_offset() - 0.0).abs() < 0.01);
    }

    #[test]
    fn easing_ease_in_starts_slow() {
        let val = EasingFunction::EaseIn.apply(0.5);
        assert!(val < 0.5); // quadratic: 0.25
    }

    #[test]
    fn easing_ease_out_starts_fast() {
        let val = EasingFunction::EaseOut.apply(0.5);
        assert!(val > 0.5); // 0.75
    }

    #[test]
    fn easing_clamps() {
        assert!((EasingFunction::Linear.apply(-0.5) - 0.0).abs() < 0.001);
        assert!((EasingFunction::Linear.apply(1.5) - 1.0).abs() < 0.001);
    }

    #[test]
    fn animation_not_running_returns_target() {
        let mut c = StickyScrollAnimationController::new(EasingFunction::Linear, 100);
        c.start(0.0, 42.0);
        c.tick(200); // finish
        let val = c.tick(10); // already done
        assert!((val - 42.0).abs() < 0.01);
    }



    // -- stickyscroll additional tests -------------------------------------------

    #[test]
    fn x_stickyscroll_text_span_new_ordered() {
        let s = XStickyscrollTextSpan::new(5, 10);
        assert_eq!(s.start, 5);
        assert_eq!(s.end, 10);
    }

    #[test]
    fn x_stickyscroll_text_span_new_reversed() {
        let s = XStickyscrollTextSpan::new(10, 5);
        assert_eq!(s.start, 5);
        assert_eq!(s.end, 10);
    }

    #[test]
    fn x_stickyscroll_text_span_len() {
        assert_eq!(XStickyscrollTextSpan::new(3, 7).len(), 4);
        assert_eq!(XStickyscrollTextSpan::new(0, 0).len(), 0);
    }

    #[test]
    fn x_stickyscroll_text_span_extract() {
        let s = XStickyscrollTextSpan::new(0, 5);
        assert_eq!(s.extract("hello world"), "hello");
    }

    #[test]
    fn x_stickyscroll_text_span_contains() {
        let s = XStickyscrollTextSpan::new(2, 8);
        assert!(s.contains(2));
        assert!(s.contains(7));
        assert!(!s.contains(8));
    }

    #[test]
    fn x_stickyscroll_text_span_intersect() {
        let a = XStickyscrollTextSpan::new(0, 10);
        let b = XStickyscrollTextSpan::new(5, 15);
        let inter = a.intersect(&b).unwrap();
        assert_eq!(inter.start, 5);
        assert_eq!(inter.end, 10);
    }

    #[test]
    fn x_stickyscroll_text_span_intersect_none() {
        let a = XStickyscrollTextSpan::new(0, 5);
        let b = XStickyscrollTextSpan::new(5, 10);
        assert!(a.intersect(&b).is_none());
    }

    #[test]
    fn x_stickyscroll_text_span_union() {
        let a = XStickyscrollTextSpan::new(3, 7);
        let b = XStickyscrollTextSpan::new(5, 12);
        let u = a.union(&b);
        assert_eq!(u.start, 3);
        assert_eq!(u.end, 12);
    }

    #[test]
    fn x_stickyscroll_count_lines_basic() {
        assert_eq!(x_stickyscroll_count_lines("a\nb\nc"), 3);
        assert_eq!(x_stickyscroll_count_lines(""), 0);
        assert_eq!(x_stickyscroll_count_lines("single"), 1);
    }

    #[test]
    fn x_stickyscroll_line_start_offset_basic() {
        assert_eq!(x_stickyscroll_line_start_offset("abc\ndef\nghi", 0), Some(0));
        assert_eq!(x_stickyscroll_line_start_offset("abc\ndef\nghi", 1), Some(4));
        assert_eq!(x_stickyscroll_line_start_offset("abc\ndef\nghi", 2), Some(8));
        assert_eq!(x_stickyscroll_line_start_offset("abc\ndef\nghi", 3), None);
    }

    #[test]
    fn x_stickyscroll_indent_level_basic() {
        assert_eq!(x_stickyscroll_indent_level("    hello"), 4);
        assert_eq!(x_stickyscroll_indent_level("hello"), 0);
        assert_eq!(x_stickyscroll_indent_level("  "), 2);
    }

    #[test]
    fn x_stickyscroll_trim_trailing_basic() {
        let input = "hello   \nworld  \n  foo  ";
        let result = x_stickyscroll_trim_trailing(input);
        assert_eq!(result, "hello\nworld\n  foo");
    }

    #[test]
    fn x_stickyscroll_detect_eol_lf() {
        assert_eq!(x_stickyscroll_detect_eol("a\nb\nc"), "\n");
    }

    #[test]
    fn x_stickyscroll_detect_eol_crlf() {
        assert_eq!(x_stickyscroll_detect_eol("a\r\nb\r\nc"), "\r\n");
    }

    #[test]
    fn x_stickyscroll_tokenize_basic() {
        let tokens = x_stickyscroll_tokenize("hello, world! foo");
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn x_stickyscroll_text_span_shift() {
        let s = XStickyscrollTextSpan::new(2, 5).shift(10);
        assert_eq!(s.start, 12);
        assert_eq!(s.end, 15);
    }


    // -- stickyscroll extended domain tests ----------------------------------------

    #[test]
    fn y_stickyscroll_enum_index() {
        assert_eq!(YStickyscrollStickyScrollAnimation::None.index(), 0);
        assert_eq!(YStickyscrollStickyScrollAnimation::FadeIn.index(), 1);
        assert_eq!(YStickyscrollStickyScrollAnimation::SlideDown.index(), 2);
        assert_eq!(YStickyscrollStickyScrollAnimation::Instant.index(), 3);
    }

    #[test]
    fn y_stickyscroll_enum_label() {
        assert_eq!(YStickyscrollStickyScrollAnimation::None.label(), "None");
        assert_eq!(YStickyscrollStickyScrollAnimation::FadeIn.label(), "FadeIn");
        assert_eq!(YStickyscrollStickyScrollAnimation::SlideDown.label(), "SlideDown");
        assert_eq!(YStickyscrollStickyScrollAnimation::Instant.label(), "Instant");
    }

    #[test]
    fn y_stickyscroll_enum_all() {
        let all = YStickyscrollStickyScrollAnimation::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_stickyscroll_enum_is_default() {
        assert!(YStickyscrollStickyScrollAnimation::None.is_default());
        assert!(!YStickyscrollStickyScrollAnimation::Instant.is_default());
    }

    #[test]
    fn y_stickyscroll_enum_display() {
        assert_eq!(format!("{}", YStickyscrollStickyScrollAnimation::None), "None");
    }

    #[test]
    fn y_stickyscroll_struct_new() {
        let s = YStickyscrollStickyScrollState::new();
        let _ = s.summary();
    }

    #[test]
    fn y_stickyscroll_fingerprint_deterministic() {
        let h1 = y_stickyscroll_fingerprint("hello");
        let h2 = y_stickyscroll_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_stickyscroll_fingerprint("a"), y_stickyscroll_fingerprint("b"));
    }

    #[test]
    fn y_stickyscroll_truncate_short() {
        assert_eq!(y_stickyscroll_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_stickyscroll_truncate_long() {
        let r = y_stickyscroll_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_stickyscroll_normalize_key_basic() {
        assert_eq!(y_stickyscroll_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_stickyscroll_split_path_basic() {
        let parts = y_stickyscroll_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_stickyscroll_count_occurrences_basic() {
        assert_eq!(y_stickyscroll_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_stickyscroll_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_stickyscroll_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_stickyscroll_in_range_basic() {
        assert!(y_stickyscroll_in_range(5, 1, 10));
        assert!(y_stickyscroll_in_range(1, 1, 10));
        assert!(y_stickyscroll_in_range(10, 1, 10));
        assert!(!y_stickyscroll_in_range(0, 1, 10));
        assert!(!y_stickyscroll_in_range(11, 1, 10));
    }

    #[test]
    fn y_stickyscroll_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_stickyscroll_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_stickyscroll_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_stickyscroll_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- stickyscroll Z-extended tests -----------------------------------------------

    #[test]
    fn z_stickyscroll_priority_weight() {
        assert_eq!(ZStickyscrollPriority::Idle.weight(), 0);
        assert_eq!(ZStickyscrollPriority::Normal.weight(), 2);
        assert_eq!(ZStickyscrollPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_stickyscroll_priority_label() {
        assert_eq!(ZStickyscrollPriority::Low.label(), "low");
        assert_eq!(ZStickyscrollPriority::High.label(), "high");
    }

    #[test]
    fn z_stickyscroll_priority_is_elevated() {
        assert!(!ZStickyscrollPriority::Normal.is_elevated());
        assert!(ZStickyscrollPriority::High.is_elevated());
        assert!(ZStickyscrollPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_stickyscroll_priority_display() {
        assert_eq!(format!("{}", ZStickyscrollPriority::Idle), "idle");
    }

    #[test]
    fn z_stickyscroll_priority_all_asc() {
        let all = ZStickyscrollPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZStickyscrollPriority::Idle);
        assert_eq!(all[4], ZStickyscrollPriority::Realtime);
    }

    #[test]
    fn z_stickyscroll_struct_new() {
        let s = ZStickyscrollStickyScrollMetrics::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_stickyscroll_struct_toggled_clone() {
        let s = ZStickyscrollStickyScrollMetrics::new();
        let t = s.toggled_clone();
        assert_ne!(s.jank_detected, t.jank_detected);
    }

    #[test]
    fn z_stickyscroll_rolling_hash_deterministic() {
        let h1 = z_stickyscroll_rolling_hash(b"test");
        let h2 = z_stickyscroll_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_stickyscroll_rolling_hash(b"a"), z_stickyscroll_rolling_hash(b"b"));
    }

    #[test]
    fn z_stickyscroll_pad_to_basic() {
        assert_eq!(z_stickyscroll_pad_to("hi", 5), "hi   ");
        assert_eq!(z_stickyscroll_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_stickyscroll_is_identifier_basic() {
        assert!(z_stickyscroll_is_identifier("foo_bar"));
        assert!(z_stickyscroll_is_identifier("abc123"));
        assert!(!z_stickyscroll_is_identifier(""));
        assert!(!z_stickyscroll_is_identifier("has space"));
    }

    #[test]
    fn z_stickyscroll_levenshtein_basic() {
        assert_eq!(z_stickyscroll_levenshtein("", ""), 0);
        assert_eq!(z_stickyscroll_levenshtein("abc", "abc"), 0);
        assert_eq!(z_stickyscroll_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_stickyscroll_unique_words_basic() {
        let w = z_stickyscroll_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_stickyscroll_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_stickyscroll_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_stickyscroll_common_prefix_basic() {
        assert_eq!(z_stickyscroll_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_stickyscroll_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_stickyscroll_struct_clear() {
        let mut s = ZStickyscrollStickyScrollMetrics::new();
        s.render_times_us.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_stickyscroll_rolling_hash_empty() {
        let h = z_stickyscroll_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_38_push_and_len() {
        let mut rb = super::XbRingBuffer38::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_38_overwrite() {
        let mut rb = super::XbRingBuffer38::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_38_get_out_of_bounds() {
        let rb = super::XbRingBuffer38::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_38_drain_all() {
        let mut rb = super::XbRingBuffer38::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_38_peek_front_back() {
        let mut rb = super::XbRingBuffer38::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_38_clear() {
        let mut rb = super::XbRingBuffer38::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_38_capacity() {
        let rb = super::XbRingBuffer38::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_38_basic() {
        let h = super::xb_fnv1a_38(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_38(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_38_different_inputs() {
        let h1 = super::xb_fnv1a_38(b"abc");
        let h2 = super::xb_fnv1a_38(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_38_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_38(&data);
        let dec = super::xb_rle_decode_38(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_38_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_38(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_38(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_38_values() {
        assert!((super::xb_clamp_38(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_38(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_38(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_38_values() {
        assert!((super::xb_lerp_38(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_38(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_38(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_38_wrap_around_twice() {
        let mut rb = super::XbRingBuffer38::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 165 ----

    #[test]
    fn xc_165_pool_new_empty() {
        let pool: super::Xc165Pool<i32> = super::Xc165Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_165_pool_release_acquire() {
        let mut pool = super::Xc165Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_165_pool_acquire_empty() {
        let mut pool: super::Xc165Pool<i32> = super::Xc165Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_165_pool_full() {
        let mut pool = super::Xc165Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_165_pool_drain() {
        let mut pool = super::Xc165Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_165_pool_stats() {
        let mut pool = super::Xc165Pool::new(8);
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
    fn xc_165_pool_clear() {
        let mut pool = super::Xc165Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_165_pool_shrink() {
        let mut pool = super::Xc165Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_165_pool_default() {
        let pool: super::Xc165Pool<String> = super::Xc165Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_165_pool_extend() {
        let mut pool = super::Xc165Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_165_pool_retain() {
        let mut pool = super::Xc165Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_165_scheduler_round_robin() {
        let mut sched = super::Xc165Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_165_scheduler_empty() {
        let mut sched = super::Xc165Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_165_scheduler_reset() {
        let mut sched = super::Xc165Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_165_scheduler_add_remove() {
        let mut sched = super::Xc165Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_165_scheduler_targets() {
        let sched = super::Xc165Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_165_hash_empty() {
        assert_eq!(super::xc_165_hash(b""), 5381);
    }

    #[test]
    fn xc_165_hash_data() {
        let h = super::xc_165_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_165_hash(b"hello"), h);
    }

    #[test]
    fn xc_165_reverse_str() {
        assert_eq!(super::xc_165_reverse("abc"), "cba");
        assert_eq!(super::xc_165_reverse(""), "");
    }


    #[test]
    fn xe_51_pipeline_empty() {
        let p = super::Xe51Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_51_pipeline_parse_stage() {
        let p = super::Xe51Pipeline::new()
            .add_parse(super::xe_51_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_51_pipeline_transform_double() {
        let p = super::Xe51Pipeline::new()
            .add_transform(super::xe_51_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_51_pipeline_validate_reverse() {
        let p = super::Xe51Pipeline::new()
            .add_validate(super::xe_51_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_51_pipeline_emit_filter() {
        let p = super::Xe51Pipeline::new()
            .add_emit(super::xe_51_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_51_pipeline_multi_stage() {
        let p = super::Xe51Pipeline::new()
            .add_parse(super::xe_51_pipeline_identity)
            .add_transform(super::xe_51_pipeline_double)
            .add_validate(super::xe_51_pipeline_reverse)
            .add_emit(super::xe_51_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_51_pipeline_error_propagation() {
        let p = super::Xe51Pipeline::new()
            .add_parse(super::xe_51_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe51Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_51_pipeline_compose() {
        let p1 = super::Xe51Pipeline::new()
            .add_parse(super::xe_51_pipeline_identity);
        let p2 = super::Xe51Pipeline::new()
            .add_transform(super::xe_51_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_51_pipeline_error_display() {
        let e = super::Xe51PipelineError {
            stage: super::Xe51Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_51_cache_put_get() {
        let mut c = super::Xe51Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_51_cache_miss() {
        let mut c: super::Xe51Cache<&str, i32> = super::Xe51Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_51_cache_ttl_expiry() {
        let mut c = super::Xe51Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_51_cache_evict() {
        let mut c = super::Xe51Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_51_cache_capacity() {
        let mut c = super::Xe51Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_51_cache_stats() {
        let mut c = super::Xe51Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_51_cache_clear() {
        let mut c = super::Xe51Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

}
