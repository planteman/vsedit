//! Whitespace visualization and trimming.

use std::fmt;

/// Errors that can occur during whitespace operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhitespaceError {
    /// Tab size must be at least 1.
    InvalidTabSize(usize),
    /// The indentation width is not a multiple of the configured unit.
    InconsistentIndentation { line: usize, found: usize, expected_multiple: usize },
    /// Mixed tabs and spaces in indentation.
    MixedIndentation { line: usize },
}

impl fmt::Display for WhitespaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WhitespaceError::InvalidTabSize(s) => {
                write!(f, "invalid tab size {s}: must be >= 1")
            }
            WhitespaceError::InconsistentIndentation { line, found, expected_multiple } => {
                write!(
                    f,
                    "line {line}: indentation width {found} is not a multiple of {expected_multiple}"
                )
            }
            WhitespaceError::MixedIndentation { line } => {
                write!(f, "line {line}: mixed tabs and spaces in indentation")
            }
        }
    }
}

impl std::error::Error for WhitespaceError {}

/// Whitespace rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhitespaceMode {
    /// Don't render whitespace characters.
    None,
    /// Render only at word boundaries.
    Boundary,
    /// Render only in selected text.
    Selection,
    /// Render only trailing whitespace.
    Trailing,
    /// Render all whitespace characters.
    All,
}

impl fmt::Display for WhitespaceMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            WhitespaceMode::None => "none",
            WhitespaceMode::Boundary => "boundary",
            WhitespaceMode::Selection => "selection",
            WhitespaceMode::Trailing => "trailing",
            WhitespaceMode::All => "all",
        };
        f.write_str(label)
    }
}

/// The kind of whitespace character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhitespaceKind {
    Space,
    Tab,
    /// Non-breaking space (U+00A0).
    Nbsp,
}

impl fmt::Display for WhitespaceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WhitespaceKind::Space => f.write_str("space"),
            WhitespaceKind::Tab => f.write_str("tab"),
            WhitespaceKind::Nbsp => f.write_str("nbsp"),
        }
    }
}

/// A single detected whitespace character with its position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhitespaceChar {
    /// Byte offset within the line.
    pub offset: usize,
    pub kind: WhitespaceKind,
    /// Display width in columns.
    pub width: usize,
}

/// Detect whitespace characters in a single line.
pub fn detect_whitespace(line: &str, tab_size: usize) -> Vec<WhitespaceChar> {
    let mut result = Vec::new();
    let mut col = 0;
    for (offset, ch) in line.char_indices() {
        match ch {
            ' ' => {
                result.push(WhitespaceChar {
                    offset,
                    kind: WhitespaceKind::Space,
                    width: 1,
                });
                col += 1;
            }
            '\t' => {
                let w = tab_size - (col % tab_size);
                result.push(WhitespaceChar {
                    offset,
                    kind: WhitespaceKind::Tab,
                    width: w,
                });
                col += w;
            }
            '\u{00A0}' => {
                result.push(WhitespaceChar {
                    offset,
                    kind: WhitespaceKind::Nbsp,
                    width: 1,
                });
                col += 1;
            }
            _ => {
                col += 1;
            }
        }
    }
    result
}

/// Convert whitespace characters to visible glyphs: `·` for space, `→` for tab, `°` for NBSP.
pub fn render_whitespace_chars(chars: &[WhitespaceChar]) -> Vec<(usize, String)> {
    chars
        .iter()
        .map(|wc| {
            let glyph = match wc.kind {
                WhitespaceKind::Space => "·".to_string(),
                WhitespaceKind::Tab => {
                    let mut s = "→".to_string();
                    for _ in 1..wc.width {
                        s.push(' ');
                    }
                    s
                }
                WhitespaceKind::Nbsp => "°".to_string(),
            };
            (wc.offset, glyph)
        })
        .collect()
}

/// Detect trailing whitespace on a single line. Returns `Some((start_offset, count))`.
pub fn detect_trailing_whitespace(line: &str) -> Option<(usize, usize)> {
    let trimmed = line.trim_end();
    let count = line.len() - trimmed.len();
    if count > 0 {
        Some((trimmed.len(), count))
    } else {
        None
    }
}

/// Trim trailing whitespace from all lines.
pub fn trim_trailing_whitespace(text: &str) -> String {
    text.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Ensure file ends with exactly one newline.
pub fn ensure_final_newline(text: &str) -> String {
    let trimmed = text.trim_end_matches('\n');
    format!("{}\n", trimmed)
}

/// Trim final empty lines.
pub fn trim_final_newlines(text: &str) -> String {
    let mut result = text.trim_end().to_string();
    if !result.is_empty() {
        result.push('\n');
    }
    result
}

/// Count trailing whitespace characters on a line.
pub fn trailing_whitespace_count(line: &str) -> usize {
    line.len() - line.trim_end().len()
}

/// Replace tabs with spaces.
pub fn tabs_to_spaces(text: &str, tab_size: usize) -> String {
    let mut result = String::with_capacity(text.len());
    let mut col = 0;
    for ch in text.chars() {
        if ch == '\t' {
            let spaces = tab_size - (col % tab_size);
            for _ in 0..spaces {
                result.push(' ');
            }
            col += spaces;
        } else if ch == '\n' {
            result.push(ch);
            col = 0;
        } else {
            result.push(ch);
            col += 1;
        }
    }
    result
}

/// Replace leading spaces with tabs where possible.
pub fn spaces_to_tabs(text: &str, tab_size: usize) -> String {
    text.lines()
        .map(|line| {
            let indent_len = line.len() - line.trim_start_matches(' ').len();
            let full_tabs = indent_len / tab_size;
            let remaining = indent_len % tab_size;
            let mut out = String::new();
            for _ in 0..full_tabs {
                out.push('\t');
            }
            for _ in 0..remaining {
                out.push(' ');
            }
            out.push_str(&line[indent_len..]);
            out
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Configuration builder for whitespace processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhitespaceConfig {
    pub tab_size: usize,
    pub trim_trailing: bool,
    pub ensure_final_newline: bool,
    pub trim_final_newlines: bool,
    pub insert_spaces: bool,
}

impl Default for WhitespaceConfig {
    fn default() -> Self {
        Self {
            tab_size: 4,
            trim_trailing: true,
            ensure_final_newline: true,
            trim_final_newlines: false,
            insert_spaces: true,
        }
    }
}

impl WhitespaceConfig {
    /// Create a new builder starting from defaults.
    pub fn builder() -> WhitespaceConfigBuilder {
        WhitespaceConfigBuilder(WhitespaceConfig::default())
    }

    /// Validate the configuration, returning an error if invalid.
    pub fn validate(&self) -> Result<(), WhitespaceError> {
        if self.tab_size == 0 {
            return Err(WhitespaceError::InvalidTabSize(0));
        }
        Ok(())
    }

    /// Apply all configured transformations to the input text.
    pub fn apply(&self, text: &str) -> Result<String, WhitespaceError> {
        self.validate()?;
        let mut result = text.to_string();

        if self.insert_spaces {
            result = tabs_to_spaces(&result, self.tab_size);
        } else {
            result = spaces_to_tabs(&result, self.tab_size);
        }

        if self.trim_trailing {
            result = trim_trailing_whitespace(&result);
        }

        if self.trim_final_newlines {
            result = crate::trim_final_newlines(&result);
        } else if self.ensure_final_newline {
            result = crate::ensure_final_newline(&result);
        }

        Ok(result)
    }
}

impl fmt::Display for WhitespaceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tab_size={}, spaces={}, trim_trail={}, final_nl={}",
            self.tab_size, self.insert_spaces, self.trim_trailing, self.ensure_final_newline
        )
    }
}

/// Builder for [`WhitespaceConfig`].
#[derive(Debug, Clone)]
pub struct WhitespaceConfigBuilder(WhitespaceConfig);

impl WhitespaceConfigBuilder {
    pub fn tab_size(mut self, size: usize) -> Self {
        self.0.tab_size = size;
        self
    }

    pub fn trim_trailing(mut self, enabled: bool) -> Self {
        self.0.trim_trailing = enabled;
        self
    }

    pub fn ensure_final_newline(mut self, enabled: bool) -> Self {
        self.0.ensure_final_newline = enabled;
        self
    }

    pub fn trim_final_newlines(mut self, enabled: bool) -> Self {
        self.0.trim_final_newlines = enabled;
        self
    }

    pub fn insert_spaces(mut self, enabled: bool) -> Self {
        self.0.insert_spaces = enabled;
        self
    }

    /// Build and validate the configuration.
    pub fn build(self) -> Result<WhitespaceConfig, WhitespaceError> {
        self.0.validate()?;
        Ok(self.0)
    }
}

/// Compute the indentation level (number of leading whitespace columns) of a line.
pub fn indentation_width(line: &str, tab_size: usize) -> usize {
    let mut col = 0;
    for ch in line.chars() {
        match ch {
            ' ' => col += 1,
            '\t' => col += tab_size - (col % tab_size),
            _ => break,
        }
    }
    col
}

/// Check that every line's indentation is a multiple of `indent_size` columns.
pub fn check_indentation_consistency(
    text: &str,
    tab_size: usize,
    indent_size: usize,
) -> Result<(), WhitespaceError> {
    if tab_size == 0 {
        return Err(WhitespaceError::InvalidTabSize(0));
    }
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let width = indentation_width(line, tab_size);
        if width % indent_size != 0 {
            return Err(WhitespaceError::InconsistentIndentation {
                line: i + 1,
                found: width,
                expected_multiple: indent_size,
            });
        }
    }
    Ok(())
}

/// Detect whether a line uses mixed tabs and spaces in its leading whitespace.
pub fn has_mixed_indentation(line: &str) -> bool {
    let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
    if indent.is_empty() {
        return false;
    }
    let has_tab = indent.contains('\t');
    let has_space = indent.contains(' ');
    has_tab && has_space
}

/// Scan all lines and return errors for any that have mixed indentation.
pub fn check_mixed_indentation(text: &str) -> Vec<WhitespaceError> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| has_mixed_indentation(line))
        .map(|(i, _)| WhitespaceError::MixedIndentation { line: i + 1 })
        .collect()
}

/// Summary statistics for whitespace in a document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WhitespaceStats {
    pub total_lines: usize,
    pub blank_lines: usize,
    pub lines_with_trailing: usize,
    pub total_trailing_chars: usize,
    pub tab_count: usize,
    pub space_count: usize,
    pub nbsp_count: usize,
}

impl fmt::Display for WhitespaceStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} lines ({} blank), {} trailing ws on {} lines, tabs={} spaces={} nbsp={}",
            self.total_lines,
            self.blank_lines,
            self.total_trailing_chars,
            self.lines_with_trailing,
            self.tab_count,
            self.space_count,
            self.nbsp_count,
        )
    }
}

/// Gather whitespace statistics for an entire document.
pub fn compute_stats(text: &str, tab_size: usize) -> WhitespaceStats {
    let mut stats = WhitespaceStats::default();
    for line in text.lines() {
        stats.total_lines += 1;
        if line.trim().is_empty() {
            stats.blank_lines += 1;
        }
        let trailing = trailing_whitespace_count(line);
        if trailing > 0 {
            stats.lines_with_trailing += 1;
            stats.total_trailing_chars += trailing;
        }
        for wc in detect_whitespace(line, tab_size) {
            match wc.kind {
                WhitespaceKind::Space => stats.space_count += 1,
                WhitespaceKind::Tab => stats.tab_count += 1,
                WhitespaceKind::Nbsp => stats.nbsp_count += 1,
            }
        }
    }
    stats
}

/// Accumulated statistics for whitespace operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WhitespaceStatsSummary {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WhitespaceStatsSummary {
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
    pub fn merge(&mut self, other: &WhitespaceStatsSummary) {
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

impl Default for WhitespaceStatsSummary {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WhitespaceStatsSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WhitespaceStatsSummary(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for whitespace.
#[derive(Debug, Clone)]
pub struct WhitespaceValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WhitespaceValidator {
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

impl Default for WhitespaceValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Whitespace normalization
// ---------------------------------------------------------------------------

/// Strategy for normalizing mixed whitespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizeTarget {
    /// Convert all indentation to spaces.
    Spaces(usize),
    /// Convert all indentation to tabs.
    Tabs,
}

/// Normalize mixed tabs and spaces in the indentation of each line.
///
/// Non-indentation whitespace (inside the line content) is left untouched.
pub fn whitespace_normalize(text: &str, target: NormalizeTarget) -> String {
    let mut result = String::with_capacity(text.len());
    for (i, line) in text.lines().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        let content = line.trim_start();
        if content.is_empty() {
            result.push_str(line);
            continue;
        }
        let indent_len = line.len() - content.len();
        let indent = &line[..indent_len];
        // Count the effective width of the existing indent
        let width = indent_width(indent);
        let new_indent = match target {
            NormalizeTarget::Spaces(size) => " ".repeat(width),
            NormalizeTarget::Tabs => {
                let tabs = width / 4; // default tab width
                let spaces = width % 4;
                "\t".repeat(tabs) + &" ".repeat(spaces)
            }
        };
        result.push_str(&new_indent);
        result.push_str(content);
    }
    if text.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Compute the visual width of an indentation string, where tabs count as 4 spaces.
fn indent_width(indent: &str) -> usize {
    let mut w = 0;
    for ch in indent.chars() {
        match ch {
            '\t' => w += 4,
            ' ' => w += 1,
            _ => break,
        }
    }
    w
}

/// Report summarizing normalization results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizeReport {
    pub lines_changed: usize,
    pub total_lines: usize,
}

impl NormalizeReport {
    /// Build a report by comparing original and normalized text.
    pub fn compare(original: &str, normalized: &str) -> Self {
        let orig_lines: Vec<&str> = original.lines().collect();
        let norm_lines: Vec<&str> = normalized.lines().collect();
        let total = orig_lines.len().max(norm_lines.len());
        let mut changed = 0;
        for i in 0..total {
            let a = orig_lines.get(i).unwrap_or(&"");
            let b = norm_lines.get(i).unwrap_or(&"");
            if a != b {
                changed += 1;
            }
        }
        Self { lines_changed: changed, total_lines: total }
    }
}

impl fmt::Display for NormalizeReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{} lines changed", self.lines_changed, self.total_lines)
    }
}

// ---------------------------------------------------------------------------
// WhitespaceChar extensions
// ---------------------------------------------------------------------------

impl WhitespaceChar {
    pub fn is_tab(&self) -> bool {
        self.kind == WhitespaceKind::Tab
    }

    pub fn is_space(&self) -> bool {
        self.kind == WhitespaceKind::Space
    }

    pub fn is_nbsp(&self) -> bool {
        self.kind == WhitespaceKind::Nbsp
    }

    pub fn visual_width(&self, tab_size: u32) -> u32 {
        match self.kind {
            WhitespaceKind::Tab => tab_size,
            WhitespaceKind::Space | WhitespaceKind::Nbsp => 1,
        }
    }
}

// ---------------------------------------------------------------------------
// WhitespaceConfig extensions
// ---------------------------------------------------------------------------

impl WhitespaceConfig {
    pub fn is_default(&self) -> bool {
        *self == WhitespaceConfig::default()
    }

    pub fn summary(&self) -> String {
        let indent = if self.insert_spaces {
            format!("{} spaces", self.tab_size)
        } else {
            "tabs".to_string()
        };
        let mut flags = Vec::new();
        if self.trim_trailing {
            flags.push("trim-trail");
        }
        if self.ensure_final_newline {
            flags.push("final-nl");
        }
        if self.trim_final_newlines {
            flags.push("trim-final-nl");
        }
        if flags.is_empty() {
            indent
        } else {
            format!("{indent} [{}]", flags.join(", "))
        }
    }
}

// ---------------------------------------------------------------------------
// WhitespaceStats extensions
// ---------------------------------------------------------------------------

impl WhitespaceStats {
    pub fn merge(&mut self, other: &WhitespaceStats) {
        self.total_lines += other.total_lines;
        self.blank_lines += other.blank_lines;
        self.lines_with_trailing += other.lines_with_trailing;
        self.total_trailing_chars += other.total_trailing_chars;
        self.tab_count += other.tab_count;
        self.space_count += other.space_count;
        self.nbsp_count += other.nbsp_count;
    }

    pub fn total_whitespace_chars(&self) -> usize {
        self.tab_count + self.space_count + self.nbsp_count
    }

    pub fn whitespace_ratio(&self, total_chars: usize) -> f64 {
        if total_chars == 0 {
            return 0.0;
        }
        self.total_whitespace_chars() as f64 / total_chars as f64
    }

    pub fn has_tabs(&self) -> bool {
        self.tab_count > 0
    }

    pub fn has_spaces(&self) -> bool {
        self.space_count > 0
    }

    pub fn has_mixed_whitespace(&self) -> bool {
        self.has_tabs() && self.has_spaces()
    }
}

// ---------------------------------------------------------------------------
// NormalizeReport extensions
// ---------------------------------------------------------------------------

impl NormalizeReport {
    pub fn had_changes(&self) -> bool {
        self.lines_changed > 0
    }

    pub fn change_count(&self) -> usize {
        self.lines_changed
    }

    pub fn unchanged_lines(&self) -> usize {
        self.total_lines.saturating_sub(self.lines_changed)
    }

    pub fn change_ratio(&self) -> f64 {
        if self.total_lines == 0 {
            return 0.0;
        }
        self.lines_changed as f64 / self.total_lines as f64
    }
}

// ---------------------------------------------------------------------------
// NormalizeTarget extensions
// ---------------------------------------------------------------------------

impl NormalizeTarget {
    pub fn label(&self) -> &'static str {
        match self {
            NormalizeTarget::Spaces(_) => "spaces",
            NormalizeTarget::Tabs => "tabs",
        }
    }

    pub fn indent_unit(&self) -> String {
        match self {
            NormalizeTarget::Spaces(n) => " ".repeat(*n),
            NormalizeTarget::Tabs => "\t".to_string(),
        }
    }
}

impl fmt::Display for NormalizeTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NormalizeTarget::Spaces(n) => write!(f, "{n} spaces"),
            NormalizeTarget::Tabs => f.write_str("tabs"),
        }
    }
}

// ---------------------------------------------------------------------------
// WhitespaceKind extensions
// ---------------------------------------------------------------------------

impl WhitespaceKind {
    pub fn all() -> Vec<WhitespaceKind> {
        vec![WhitespaceKind::Space, WhitespaceKind::Tab, WhitespaceKind::Nbsp]
    }

    pub fn as_char(&self) -> char {
        match self {
            WhitespaceKind::Space => ' ',
            WhitespaceKind::Tab => '\t',
            WhitespaceKind::Nbsp => '\u{00A0}',
        }
    }

    pub fn glyph(&self) -> char {
        match self {
            WhitespaceKind::Space => '·',
            WhitespaceKind::Tab => '→',
            WhitespaceKind::Nbsp => '°',
        }
    }
}

// ---------------------------------------------------------------------------
// Indentation detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndentationStyle {
    Tab,
    Spaces(u32),
    Mixed,
    Unknown,
}

impl fmt::Display for IndentationStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IndentationStyle::Tab => f.write_str("tabs"),
            IndentationStyle::Spaces(n) => write!(f, "{n} spaces"),
            IndentationStyle::Mixed => f.write_str("mixed"),
            IndentationStyle::Unknown => f.write_str("unknown"),
        }
    }
}

pub fn detect_indentation(lines: &[&str]) -> IndentationStyle {
    let mut tab_lines = 0u32;
    let mut space_lines = 0u32;
    let mut min_space_indent: Option<u32> = None;

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let first = match line.chars().next() {
            Some(c) => c,
            None => continue,
        };
        if first == '\t' {
            tab_lines += 1;
        } else if first == ' ' {
            space_lines += 1;
            let count = line.chars().take_while(|c| *c == ' ').count() as u32;
            if count > 0 {
                min_space_indent = Some(match min_space_indent {
                    Some(prev) => gcd(prev, count),
                    None => count,
                });
            }
        }
    }

    if tab_lines == 0 && space_lines == 0 {
        return IndentationStyle::Unknown;
    }
    if tab_lines > 0 && space_lines > 0 {
        return IndentationStyle::Mixed;
    }
    if tab_lines > 0 {
        return IndentationStyle::Tab;
    }
    IndentationStyle::Spaces(min_space_indent.unwrap_or(4))
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

// ---------------------------------------------------------------------------
// Whitespace line iterator
// ---------------------------------------------------------------------------

pub struct WhitespaceIter<'a> {
    chars: std::str::CharIndices<'a>,
    col: usize,
    tab_size: usize,
}

impl<'a> WhitespaceIter<'a> {
    pub fn new(line: &'a str, tab_size: usize) -> Self {
        Self {
            chars: line.char_indices(),
            col: 0,
            tab_size,
        }
    }
}

impl<'a> Iterator for WhitespaceIter<'a> {
    type Item = WhitespaceChar;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (offset, ch) = self.chars.next()?;
            match ch {
                ' ' => {
                    self.col += 1;
                    return Some(WhitespaceChar {
                        offset,
                        kind: WhitespaceKind::Space,
                        width: 1,
                    });
                }
                '\t' => {
                    let w = self.tab_size - (self.col % self.tab_size);
                    self.col += w;
                    return Some(WhitespaceChar {
                        offset,
                        kind: WhitespaceKind::Tab,
                        width: w,
                    });
                }
                '\u{00A0}' => {
                    self.col += 1;
                    return Some(WhitespaceChar {
                        offset,
                        kind: WhitespaceKind::Nbsp,
                        width: 1,
                    });
                }
                _ => {
                    self.col += 1;
                }
            }
        }
    }
}

pub fn whitespace_iter(line: &str, tab_size: usize) -> WhitespaceIter<'_> {
    WhitespaceIter::new(line, tab_size)
}

/// Count the number of leading whitespace characters (spaces and tabs) in a line.
pub fn leading_whitespace_count(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ' || *c == '\t').count()
}

/// Replace all non-breaking spaces (U+00A0) with regular spaces.
pub fn normalize_nbsp(text: &str) -> String {
    text.replace('\u{00A0}', " ")
}

/// Check if a line consists entirely of whitespace.
pub fn is_blank_line(line: &str) -> bool {
    line.chars().all(|c| c.is_whitespace())
}

/// Count blank lines in a multi-line text.
pub fn count_blank_lines(text: &str) -> usize {
    text.lines().filter(|line| is_blank_line(line)).count()
}

/// Extract the indentation prefix (leading whitespace) from a line.
pub fn extract_indentation(line: &str) -> &str {
    let end = line.len() - line.trim_start().len();
    &line[..end]
}

/// Compute the visual column width of a string, expanding tabs.
pub fn visual_width(text: &str, tab_size: usize) -> usize {
    let mut col = 0;
    for ch in text.chars() {
        match ch {
            '\t' => {
                col += tab_size - (col % tab_size);
            }
            _ => {
                col += 1;
            }
        }
    }
    col
}

/// Return the line with the deepest indentation in the text.
pub fn max_indentation_line(text: &str, tab_size: usize) -> Option<usize> {
    text.lines()
        .enumerate()
        .max_by_key(|(_, line)| indentation_width(line, tab_size))
        .map(|(i, _)| i)
}

/// Detect if a text uses predominantly tabs or spaces for indentation.
pub fn detect_dominant_indentation(text: &str) -> WhitespaceKind {
    let mut tab_lines = 0usize;
    let mut space_lines = 0usize;
    for line in text.lines() {
        let first = line.chars().next();
        match first {
            Some('\t') => tab_lines += 1,
            Some(' ') => space_lines += 1,
            _ => {}
        }
    }
    if tab_lines >= space_lines {
        WhitespaceKind::Tab
    } else {
        WhitespaceKind::Space
    }
}

// ---------------------------------------------------------------------------
// Line-level whitespace analysis
// ---------------------------------------------------------------------------

/// Per-line whitespace summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineWhitespaceInfo {
    /// 0-based line index.
    pub line_index: usize,
    /// Number of leading whitespace columns.
    pub leading_columns: usize,
    /// Number of trailing whitespace characters.
    pub trailing_chars: usize,
    /// Whether the line is entirely blank.
    pub is_blank: bool,
    /// The indentation style detected on this line.
    pub indent_style: IndentationStyle,
}

/// Analyse every line of `text` and return per-line whitespace information.
pub fn analyse_lines(text: &str, tab_size: usize) -> Vec<LineWhitespaceInfo> {
    text.lines()
        .enumerate()
        .map(|(idx, line)| {
            let leading_columns = leading_width(line, tab_size);
            let trailing_chars = line.len() - line.trim_end().len();
            let is_blank = line.trim().is_empty();
            let indent_style = detect_line_indent_style(line);
            LineWhitespaceInfo {
                line_index: idx,
                leading_columns,
                trailing_chars,
                is_blank,
                indent_style,
            }
        })
        .collect()
}

/// Compute the visual column width of leading whitespace in a single line.
pub fn leading_width(line: &str, tab_size: usize) -> usize {
    let mut col = 0;
    for ch in line.chars() {
        match ch {
            ' ' => col += 1,
            '\t' => col += tab_size - (col % tab_size),
            _ => break,
        }
    }
    col
}

/// Detect the indentation style used on a single line.
fn detect_line_indent_style(line: &str) -> IndentationStyle {
    let mut has_tab = false;
    let mut space_count: u32 = 0;
    for ch in line.chars() {
        match ch {
            '\t' => has_tab = true,
            ' ' => space_count += 1,
            _ => break,
        }
    }
    if has_tab && space_count > 0 {
        IndentationStyle::Mixed
    } else if has_tab {
        IndentationStyle::Tab
    } else if space_count > 0 {
        IndentationStyle::Spaces(space_count)
    } else {
        IndentationStyle::Unknown
    }
}

/// Return the set of unique indentation widths present in `text`.
pub fn unique_indent_widths(text: &str, tab_size: usize) -> Vec<usize> {
    let mut widths: Vec<usize> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| leading_width(l, tab_size))
        .collect();
    widths.sort_unstable();
    widths.dedup();
    widths
}

/// Guess the indentation unit size (e.g. 2 or 4 spaces) from text.
///
/// Returns `None` if no indentation is found.
pub fn guess_indent_size(text: &str, tab_size: usize) -> Option<usize> {
    let widths = unique_indent_widths(text, tab_size);
    if widths.len() < 2 {
        return widths.first().copied().filter(|&w| w > 0);
    }
    // GCD of all non-zero widths is a good heuristic.
    let non_zero: Vec<usize> = widths.into_iter().filter(|&w| w > 0).collect();
    if non_zero.is_empty() {
        return None;
    }
    let mut g = non_zero[0];
    for &w in &non_zero[1..] {
        g = gcd_usize(g, w);
    }
    Some(g)
}

fn gcd_usize(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

// -- WhitespaceDetector for mixed indentation --------------------------------

/// Result of inspecting a line's indentation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentationType {
    Tabs,
    Spaces,
    Mixed,
    None,
}

impl fmt::Display for IndentationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IndentationType::Tabs => f.write_str("tabs"),
            IndentationType::Spaces => f.write_str("spaces"),
            IndentationType::Mixed => f.write_str("mixed"),
            IndentationType::None => f.write_str("none"),
        }
    }
}

/// Detect the indentation type of a single line.
pub fn detect_line_indentation(line: &str) -> IndentationType {
    let mut has_tabs = false;
    let mut has_spaces = false;
    for ch in line.chars() {
        match ch {
            '\t' => has_tabs = true,
            ' ' => has_spaces = true,
            _ => break,
        }
    }
    match (has_tabs, has_spaces) {
        (true, true) => IndentationType::Mixed,
        (true, false) => IndentationType::Tabs,
        (false, true) => IndentationType::Spaces,
        (false, false) => IndentationType::None,
    }
}

/// Detect mixed indentation across all lines. Returns line numbers (1-based) with mixed indent.
pub fn detect_mixed_indentation(text: &str) -> Vec<usize> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| detect_line_indentation(line) == IndentationType::Mixed)
        .map(|(i, _)| i + 1)
        .collect()
}

// -- WhitespaceNormalizer for trailing/leading cleanup -------------------------

/// Remove trailing blank lines, keeping at most one final newline.
pub fn trim_trailing_blank_lines(text: &str) -> String {
    let trimmed = text.trim_end();
    if text.ends_with('\n') && !trimmed.is_empty() {
        format!("{trimmed}\n")
    } else {
        trimmed.to_string()
    }
}

/// Count the number of lines that have trailing whitespace.
pub fn count_trailing_whitespace_lines(text: &str) -> usize {
    text.lines().filter(|line| {
        let trimmed = line.trim_end();
        trimmed.len() < line.len()
    }).count()
}

// -- Whitespace visualization toggle ------------------------------------------

/// Configuration for whitespace visualization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhitespaceVisualization {
    pub mode: WhitespaceMode,
    pub space_char: char,
    pub tab_char: char,
    pub newline_char: Option<char>,
}

impl Default for WhitespaceVisualization {
    fn default() -> Self {
        Self {
            mode: WhitespaceMode::None,
            space_char: '·',
            tab_char: '→',
            newline_char: None,
        }
    }
}

impl WhitespaceVisualization {
    /// Render a line with visible whitespace markers.
    pub fn render_line(&self, line: &str) -> String {
        if self.mode == WhitespaceMode::None {
            return line.to_string();
        }
        let mut result = String::with_capacity(line.len());
        let trimmed_end = line.trim_end().len();
        for (i, ch) in line.chars().enumerate() {
            match ch {
                ' ' => {
                    let show = match self.mode {
                        WhitespaceMode::All => true,
                        WhitespaceMode::Trailing => i >= trimmed_end,
                        WhitespaceMode::Boundary => {
                            i == 0 || i >= trimmed_end
                        }
                        _ => false,
                    };
                    result.push(if show { self.space_char } else { ' ' });
                }
                '\t' => {
                    let show = match self.mode {
                        WhitespaceMode::All | WhitespaceMode::Boundary => true,
                        WhitespaceMode::Trailing => i >= trimmed_end,
                        _ => false,
                    };
                    result.push(if show { self.tab_char } else { '\t' });
                }
                other => result.push(other),
            }
        }
        result
    }
}

impl fmt::Display for WhitespaceVisualization {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Visualization(mode={}, space='{}', tab='{}')", self.mode, self.space_char, self.tab_char)
    }
}


// ---------------------------------------------------------------------------
// WhitespaceGuideRenderer — render indentation guides
// ---------------------------------------------------------------------------

/// Style for rendering indentation guide lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideStyle {
    /// Thin vertical bar: │
    Thin,
    /// Dotted vertical line: ┆
    Dotted,
    /// No guide character (just spacing).
    None,
}

impl fmt::Display for GuideStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Thin => write!(f, "thin"),
            Self::Dotted => write!(f, "dotted"),
            Self::None => write!(f, "none"),
        }
    }
}

/// Renders indentation guide characters for a set of lines.
#[derive(Debug, Clone)]
pub struct WhitespaceGuideRenderer {
    pub tab_size: usize,
    pub style: GuideStyle,
    pub max_depth: usize,
}

impl WhitespaceGuideRenderer {
    pub fn new(tab_size: usize, style: GuideStyle) -> Self {
        Self {
            tab_size: if tab_size == 0 { 1 } else { tab_size },
            style,
            max_depth: 20,
        }
    }

    /// Return the guide character for the configured style.
    pub fn guide_char(&self) -> char {
        match self.style {
            GuideStyle::Thin => '│',
            GuideStyle::Dotted => '┆',
            GuideStyle::None => ' ',
        }
    }

    /// Compute the indentation depth (in units of `tab_size`) for a line.
    pub fn indent_depth(&self, line: &str) -> usize {
        let mut spaces = 0usize;
        for ch in line.chars() {
            match ch {
                ' ' => spaces += 1,
                '\t' => spaces += self.tab_size - (spaces % self.tab_size),
                _ => break,
            }
        }
        (spaces / self.tab_size).min(self.max_depth)
    }

    /// Render guide markers for a single line.
    /// Returns a string of guide characters at each indentation level.
    pub fn render_guides(&self, line: &str) -> String {
        let depth = self.indent_depth(line);
        if depth == 0 || self.style == GuideStyle::None {
            return String::new();
        }
        let guide = self.guide_char();
        let mut result = String::with_capacity(depth);
        for _ in 0..depth {
            result.push(guide);
        }
        result
    }

    /// Compute guide depths for multiple lines.
    pub fn compute_depths(&self, lines: &[&str]) -> Vec<usize> {
        lines.iter().map(|l| self.indent_depth(l)).collect()
    }

    /// Find the maximum indentation depth across all lines.
    pub fn max_indent_depth(&self, lines: &[&str]) -> usize {
        lines.iter().map(|l| self.indent_depth(l)).max().unwrap_or(0)
    }

    /// Render guides for all lines, returning (depth, guide_string) pairs.
    pub fn render_all(&self, lines: &[&str]) -> Vec<(usize, String)> {
        lines
            .iter()
            .map(|l| {
                let d = self.indent_depth(l);
                let g = self.render_guides(l);
                (d, g)
            })
            .collect()
    }
}

impl Default for WhitespaceGuideRenderer {
    fn default() -> Self {
        Self::new(4, GuideStyle::Thin)
    }
}

// ---------------------------------------------------------------------------
// WhitespaceReport — statistics about whitespace in a document
// ---------------------------------------------------------------------------

/// Aggregated statistics about whitespace in a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhitespaceReport {
    /// Total number of lines analysed.
    pub total_lines: usize,
    /// Lines that contain trailing whitespace.
    pub lines_with_trailing: usize,
    /// Total number of trailing whitespace characters.
    pub trailing_char_count: usize,
    /// Lines that are entirely blank (only whitespace).
    pub blank_lines: usize,
    /// Number of lines using tabs for indentation.
    pub tab_indented_lines: usize,
    /// Number of lines using spaces for indentation.
    pub space_indented_lines: usize,
    /// Number of lines with mixed indentation.
    pub mixed_indented_lines: usize,
}

impl WhitespaceReport {
    /// Analyse a document and produce a report.
    pub fn analyse(text: &str) -> Self {
        let mut report = Self {
            total_lines: 0,
            lines_with_trailing: 0,
            trailing_char_count: 0,
            blank_lines: 0,
            tab_indented_lines: 0,
            space_indented_lines: 0,
            mixed_indented_lines: 0,
        };

        for line in text.lines() {
            report.total_lines += 1;

            if line.trim().is_empty() {
                report.blank_lines += 1;
            }

            let trailing = trailing_whitespace_count(line);
            if trailing > 0 {
                report.lines_with_trailing += 1;
                report.trailing_char_count += trailing;
            }

            // Classify indentation
            let indent_chars: String = line.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
            if !indent_chars.is_empty() {
                let has_tabs = indent_chars.contains('\t');
                let has_spaces = indent_chars.contains(' ');
                if has_tabs && has_spaces {
                    report.mixed_indented_lines += 1;
                } else if has_tabs {
                    report.tab_indented_lines += 1;
                } else {
                    report.space_indented_lines += 1;
                }
            }
        }

        report
    }

    /// Percentage of lines with trailing whitespace.
    pub fn trailing_percentage(&self) -> f64 {
        if self.total_lines == 0 {
            return 0.0;
        }
        (self.lines_with_trailing as f64 / self.total_lines as f64) * 100.0
    }

    /// The dominant indentation style.
    pub fn dominant_indent(&self) -> &'static str {
        if self.tab_indented_lines > self.space_indented_lines {
            "tabs"
        } else if self.space_indented_lines > self.tab_indented_lines {
            "spaces"
        } else {
            "mixed"
        }
    }

    /// Whether the document has any whitespace issues.
    pub fn has_issues(&self) -> bool {
        self.lines_with_trailing > 0 || self.mixed_indented_lines > 0
    }

    /// A short summary string.
    pub fn summary(&self) -> String {
        format!(
            "{} lines, {} trailing, {} blank, indent={}",
            self.total_lines,
            self.lines_with_trailing,
            self.blank_lines,
            self.dominant_indent()
        )
    }
}

impl fmt::Display for WhitespaceReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

// ---------------------------------------------------------------------------
// WhitespaceAutoFix — auto-fix whitespace issues on save
// ---------------------------------------------------------------------------

/// Configuration for automatic whitespace fixing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoFixConfig {
    pub trim_trailing: bool,
    pub ensure_final_newline: bool,
    pub trim_final_newlines: bool,
    pub normalize_indent: bool,
    pub target_indent: IndentStyle,
    pub tab_size: usize,
}

/// The target indentation style for auto-fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
    Spaces,
    Tabs,
}

impl fmt::Display for IndentStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spaces => write!(f, "spaces"),
            Self::Tabs => write!(f, "tabs"),
        }
    }
}

impl Default for AutoFixConfig {
    fn default() -> Self {
        Self {
            trim_trailing: true,
            ensure_final_newline: true,
            trim_final_newlines: false,
            normalize_indent: false,
            target_indent: IndentStyle::Spaces,
            tab_size: 4,
        }
    }
}

/// Result of an auto-fix pass.
#[derive(Debug, Clone)]
pub struct AutoFixResult {
    pub original: String,
    pub fixed: String,
    pub changes_made: usize,
}

impl AutoFixResult {
    /// Whether any changes were made.
    pub fn was_modified(&self) -> bool {
        self.changes_made > 0
    }

    /// Character-level difference in length.
    pub fn length_delta(&self) -> isize {
        self.fixed.len() as isize - self.original.len() as isize
    }
}

/// Apply automatic whitespace fixes to a text buffer.
pub fn auto_fix(text: &str, config: &AutoFixConfig) -> AutoFixResult {
    let original = text.to_string();
    let mut result = text.to_string();
    let mut changes = 0usize;

    if config.trim_trailing {
        let trimmed = trim_trailing_whitespace(&result);
        if trimmed != result {
            changes += 1;
            result = trimmed;
        }
    }

    if config.normalize_indent {
        let converted = match config.target_indent {
            IndentStyle::Spaces => tabs_to_spaces(&result, config.tab_size),
            IndentStyle::Tabs => spaces_to_tabs(&result, config.tab_size),
        };
        if converted != result {
            changes += 1;
            result = converted;
        }
    }

    if config.trim_final_newlines {
        let trimmed = trim_final_newlines(&result);
        if trimmed != result {
            changes += 1;
            result = trimmed;
        }
    }

    if config.ensure_final_newline {
        let ensured = ensure_final_newline(&result);
        if ensured != result {
            changes += 1;
            result = ensured;
        }
    }

    AutoFixResult {
        original,
        fixed: result,
        changes_made: changes,
    }
}

// ---------------------------------------------------------------------------
// WhitespaceScopeDetector — detect indentation scope boundaries
// ---------------------------------------------------------------------------

/// A detected scope (block) based on indentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentScope {
    /// First line of the scope (0-based).
    pub start_line: usize,
    /// Last line of the scope (inclusive, 0-based).
    pub end_line: usize,
    /// Indentation depth of this scope.
    pub depth: usize,
}

impl IndentScope {
    /// Number of lines in this scope.
    pub fn line_count(&self) -> usize {
        self.end_line - self.start_line + 1
    }
}

impl fmt::Display for IndentScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "scope(lines {}-{}, depth {})", self.start_line, self.end_line, self.depth)
    }
}

/// Detect indentation-based scopes in a document.
pub fn detect_scopes(text: &str, tab_size: usize) -> Vec<IndentScope> {
    let renderer = WhitespaceGuideRenderer::new(tab_size, GuideStyle::None);
    let lines: Vec<&str> = text.lines().collect();
    let depths: Vec<usize> = lines.iter().map(|l| renderer.indent_depth(l)).collect();

    let mut scopes: Vec<IndentScope> = Vec::new();
    if depths.is_empty() {
        return scopes;
    }

    let mut i = 0;
    while i < depths.len() {
        let depth = depths[i];
        let start = i;
        // Extend while depth is >= this depth
        while i < depths.len() && depths[i] >= depth {
            i += 1;
        }
        if i > start {
            scopes.push(IndentScope {
                start_line: start,
                end_line: i - 1,
                depth,
            });
        }
    }
    scopes
}

/// Find the scope containing a given line number.
pub fn scope_at_line(scopes: &[IndentScope], line: usize) -> Option<&IndentScope> {
    scopes
        .iter()
        .filter(|s| s.start_line <= line && s.end_line >= line)
        .max_by_key(|s| s.depth)
}


// ---------------------------------------------------------------------------
// whitespace – Editor text helpers
// ---------------------------------------------------------------------------

/// A half-open range within a document `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XWhitespaceTextSpan {
    pub start: usize,
    pub end: usize,
}

impl XWhitespaceTextSpan {
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
pub fn x_whitespace_count_lines(text: &str) -> usize {
    if text.is_empty() { return 0; }
    text.lines().count()
}

/// Return the byte offset of the start of line `n` (0-based).
pub fn x_whitespace_line_start_offset(text: &str, line: usize) -> Option<usize> {
    let mut current = 0usize;
    for (i, l) in text.split('\n').enumerate() {
        if i == line { return Some(current); }
        current += l.len() + 1;
    }
    None
}

/// Compute the indentation level (number of leading spaces) of a line.
pub fn x_whitespace_indent_level(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

/// Trim trailing whitespace from every line in `text`.
pub fn x_whitespace_trim_trailing(text: &str) -> String {
    text.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Detect the dominant line ending in `text` (`"\n"` or `"\r\n"`).
pub fn x_whitespace_detect_eol(text: &str) -> &'static str {
    let crlf = text.matches("\r\n").count();
    let lf = text.matches('\n').count().saturating_sub(crlf);
    if crlf > lf { "\r\n" } else { "\n" }
}

/// Simple word-boundary based tokenizer: split on whitespace and punctuation.
pub fn x_whitespace_tokenize(text: &str) -> Vec<&str> {
    text.split(|c: char| c.is_whitespace() || ".,;:!?()[]{}".contains(c))
        .filter(|s| !s.is_empty())
        .collect()
}



// ---------------------------------------------------------------------------
// whitespace – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for whitespace visualization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YWhitespaceWhitespaceRenderStyle {
    Dot,
    Arrow,
    Bar,
    Invisible,
}

impl YWhitespaceWhitespaceRenderStyle {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Dot => 0,
            Self::Arrow => 1,
            Self::Bar => 2,
            Self::Invisible => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Dot => "Dot",
            Self::Arrow => "Arrow",
            Self::Bar => "Bar",
            Self::Invisible => "Invisible",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YWhitespaceWhitespaceRenderStyle] {
        &[
            YWhitespaceWhitespaceRenderStyle::Dot,
            YWhitespaceWhitespaceRenderStyle::Arrow,
            YWhitespaceWhitespaceRenderStyle::Bar,
            YWhitespaceWhitespaceRenderStyle::Invisible,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YWhitespaceWhitespaceRenderStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks whitespace analysis data.
#[derive(Debug, Clone)]
pub struct YWhitespaceWhitespaceAnalysis {
    pub tab_count: usize,
    pub space_count: usize,
    pub mixed_lines: usize,
}

impl YWhitespaceWhitespaceAnalysis {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            tab_count: 0,
            space_count: 0,
            mixed_lines: 0,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YWhitespaceWhitespaceAnalysis({}: {:?})", "tab_count", self.tab_count)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_whitespace_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_whitespace_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_whitespace_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_whitespace_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_whitespace_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_whitespace_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_whitespace_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_whitespace_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// whitespace – Extended whitespace report helpers
// ---------------------------------------------------------------------------

/// Priority levels for whitespace report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZWhitespacePriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZWhitespacePriority {
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
    pub fn all_asc() -> [ZWhitespacePriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZWhitespacePriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks whitespace report data.
#[derive(Debug, Clone)]
pub struct ZWhitespaceWhitespaceReport {
    pub lines_checked: Vec<usize>,
    pub tab_lines: usize,
    pub space_lines: usize,
}

impl ZWhitespaceWhitespaceReport {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            lines_checked: Vec::new(),
            tab_lines: 0,
            space_lines: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.lines_checked.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.lines_checked.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.lines_checked.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZWhitespaceWhitespaceReport[tab_lines={:?}, space_lines={:?}]", self.tab_lines, self.space_lines)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for whitespace report.
pub fn z_whitespace_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_whitespace_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_whitespace_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_whitespace_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_whitespace_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_whitespace_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_whitespace_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_trailing_works() {
        assert_eq!(trim_trailing_whitespace("hello   \nworld  "), "hello\nworld");
    }

    #[test]
    fn final_newline() {
        assert_eq!(ensure_final_newline("hello"), "hello\n");
        assert_eq!(ensure_final_newline("hello\n\n"), "hello\n");
    }

    #[test]
    fn trim_final() {
        assert_eq!(trim_final_newlines("hello\n\n\n"), "hello\n");
    }

    #[test]
    fn trailing_count() {
        assert_eq!(trailing_whitespace_count("hello   "), 3);
        assert_eq!(trailing_whitespace_count("hello"), 0);
    }

    #[test]
    fn tab_conversion() {
        assert_eq!(tabs_to_spaces("\thello", 4), "    hello");
        assert_eq!(tabs_to_spaces("ab\tcd", 4), "ab  cd");
    }

    #[test]
    fn detect_whitespace_spaces_and_tabs() {
        let chars = detect_whitespace("a b\tc", 4);
        assert_eq!(chars.len(), 2);
        assert_eq!(chars[0].kind, WhitespaceKind::Space);
        assert_eq!(chars[0].offset, 1);
        assert_eq!(chars[1].kind, WhitespaceKind::Tab);
    }

    #[test]
    fn detect_whitespace_nbsp() {
        let line = "a\u{00A0}b";
        let chars = detect_whitespace(line, 4);
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].kind, WhitespaceKind::Nbsp);
    }

    #[test]
    fn detect_whitespace_empty() {
        let chars = detect_whitespace("hello", 4);
        assert!(chars.is_empty());
    }

    #[test]
    fn render_whitespace_glyphs() {
        let chars = vec![
            WhitespaceChar { offset: 0, kind: WhitespaceKind::Space, width: 1 },
            WhitespaceChar { offset: 1, kind: WhitespaceKind::Tab, width: 4 },
            WhitespaceChar { offset: 5, kind: WhitespaceKind::Nbsp, width: 1 },
        ];
        let rendered = render_whitespace_chars(&chars);
        assert_eq!(rendered[0].1, "·");
        assert_eq!(rendered[1].1, "→   ");
        assert_eq!(rendered[2].1, "°");
    }

    #[test]
    fn detect_trailing_whitespace_found() {
        let result = detect_trailing_whitespace("hello   ");
        assert_eq!(result, Some((5, 3)));
    }

    #[test]
    fn detect_trailing_whitespace_none() {
        assert_eq!(detect_trailing_whitespace("hello"), None);
    }

    #[test]
    fn whitespace_mode_enum_exists() {
        // Ensure all modes are constructible
        let modes = [
            WhitespaceMode::None,
            WhitespaceMode::Boundary,
            WhitespaceMode::Selection,
            WhitespaceMode::Trailing,
            WhitespaceMode::All,
        ];
        assert_eq!(modes.len(), 5);
    }

    #[test]
    fn whitespace_mode_display() {
        assert_eq!(WhitespaceMode::None.to_string(), "none");
        assert_eq!(WhitespaceMode::All.to_string(), "all");
        assert_eq!(WhitespaceMode::Trailing.to_string(), "trailing");
    }

    #[test]
    fn whitespace_kind_display() {
        assert_eq!(WhitespaceKind::Space.to_string(), "space");
        assert_eq!(WhitespaceKind::Tab.to_string(), "tab");
        assert_eq!(WhitespaceKind::Nbsp.to_string(), "nbsp");
    }

    #[test]
    fn whitespace_error_display() {
        let e = WhitespaceError::InvalidTabSize(0);
        assert_eq!(e.to_string(), "invalid tab size 0: must be >= 1");

        let e2 = WhitespaceError::MixedIndentation { line: 3 };
        assert!(e2.to_string().contains("line 3"));
    }

    #[test]
    fn config_builder_defaults() {
        let cfg = WhitespaceConfig::builder().build().unwrap();
        assert_eq!(cfg.tab_size, 4);
        assert!(cfg.trim_trailing);
        assert!(cfg.ensure_final_newline);
        assert!(cfg.insert_spaces);
    }

    #[test]
    fn config_builder_custom() {
        let cfg = WhitespaceConfig::builder()
            .tab_size(2)
            .trim_trailing(false)
            .insert_spaces(false)
            .build()
            .unwrap();
        assert_eq!(cfg.tab_size, 2);
        assert!(!cfg.trim_trailing);
        assert!(!cfg.insert_spaces);
    }

    #[test]
    fn config_builder_invalid_tab_size() {
        let result = WhitespaceConfig::builder().tab_size(0).build();
        assert_eq!(result, Err(WhitespaceError::InvalidTabSize(0)));
    }

    #[test]
    fn config_apply_trims_and_converts() {
        let cfg = WhitespaceConfig::builder()
            .tab_size(4)
            .trim_trailing(true)
            .ensure_final_newline(true)
            .build()
            .unwrap();
        let out = cfg.apply("\thello   \nworld  ").unwrap();
        assert_eq!(out, "    hello\nworld\n");
    }

    #[test]
    fn spaces_to_tabs_conversion() {
        assert_eq!(spaces_to_tabs("        hello", 4), "\t\thello");
        assert_eq!(spaces_to_tabs("      hello", 4), "\t  hello");
    }

    #[test]
    fn indentation_width_spaces() {
        assert_eq!(indentation_width("    hello", 4), 4);
        assert_eq!(indentation_width("\thello", 4), 4);
        assert_eq!(indentation_width("  \thello", 4), 4);
    }

    #[test]
    fn check_indentation_consistency_ok() {
        let text = "    line1\n        line2\nline3\n";
        assert!(check_indentation_consistency(text, 4, 4).is_ok());
    }

    #[test]
    fn check_indentation_consistency_err() {
        let text = "   bad_indent\n";
        let err = check_indentation_consistency(text, 4, 4);
        assert!(matches!(err, Err(WhitespaceError::InconsistentIndentation { .. })));
    }

    #[test]
    fn mixed_indentation_detection() {
        assert!(has_mixed_indentation("\t hello"));
        assert!(!has_mixed_indentation("\thello"));
        assert!(!has_mixed_indentation("    hello"));
        assert!(!has_mixed_indentation("hello"));
    }

    #[test]
    fn check_mixed_indentation_report() {
        let text = "    good\n\t bad\n\talso_good\n";
        let errors = check_mixed_indentation(text);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0], WhitespaceError::MixedIndentation { line: 2 });
    }

    #[test]
    fn compute_stats_basic() {
        let text = "hello   \n\n\tworld\n";
        let stats = compute_stats(text, 4);
        assert_eq!(stats.total_lines, 3);
        assert_eq!(stats.blank_lines, 1);
        assert_eq!(stats.lines_with_trailing, 1);
        assert_eq!(stats.total_trailing_chars, 3);
        assert_eq!(stats.tab_count, 1);
        assert_eq!(stats.space_count, 3);
    }

    #[test]
    fn whitespace_stats_display() {
        let stats = WhitespaceStats {
            total_lines: 10,
            blank_lines: 2,
            lines_with_trailing: 1,
            total_trailing_chars: 3,
            tab_count: 0,
            space_count: 5,
            nbsp_count: 0,
        };
        let s = stats.to_string();
        assert!(s.contains("10 lines"));
        assert!(s.contains("2 blank"));
    }

    #[test]
    fn config_display() {
        let cfg = WhitespaceConfig::default();
        let s = cfg.to_string();
        assert!(s.contains("tab_size=4"));
        assert!(s.contains("spaces=true"));
    }

    #[test]
    fn whitespace_stats_new_defaults() {
        let stats = WhitespaceStatsSummary::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn whitespace_stats_record_success() {
        let mut stats = WhitespaceStatsSummary::new();
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
    fn whitespace_stats_record_failure() {
        let mut stats = WhitespaceStatsSummary::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn whitespace_stats_reset() {
        let mut stats = WhitespaceStatsSummary::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn whitespace_stats_merge() {
        let mut a = WhitespaceStatsSummary::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WhitespaceStatsSummary::new();
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
    fn whitespace_stats_summary_display() {
        let mut stats = WhitespaceStatsSummary::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn whitespace_stats_default() {
        let stats = WhitespaceStatsSummary::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn whitespace_validator_accepts_valid_name() {
        let v = WhitespaceValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn whitespace_validator_rejects_empty() {
        let v = WhitespaceValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn whitespace_validator_rejects_too_long() {
        let v = WhitespaceValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn whitespace_validator_forbidden_prefix() {
        let v = WhitespaceValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn whitespace_validator_allowed_chars() {
        let v = WhitespaceValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn whitespace_validator_range() {
        let v = WhitespaceValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn whitespace_sanitize_removes_control() {
        let result = WhitespaceValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn whitespace_truncate_short_string() {
        assert_eq!(WhitespaceValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn whitespace_truncate_long_string() {
        let result = WhitespaceValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn whitespace_is_ascii_printable() {
        assert!(WhitespaceValidator::is_ascii_printable("Hello World 123"));
        assert!(!WhitespaceValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- whitespace_normalize --

    #[test]
    fn normalize_tabs_to_spaces() {
        let input = "\tline1\n\t\tline2\nline3";
        let result = whitespace_normalize(input, NormalizeTarget::Spaces(4));
        assert_eq!(result, "    line1\n        line2\nline3");
    }

    #[test]
    fn normalize_spaces_to_tabs() {
        let input = "    line1\n        line2\nline3";
        let result = whitespace_normalize(input, NormalizeTarget::Tabs);
        assert_eq!(result, "\tline1\n\t\tline2\nline3");
    }

    #[test]
    fn normalize_mixed_to_spaces() {
        let input = "\t line1\n  \tline2";
        let result = whitespace_normalize(input, NormalizeTarget::Spaces(4));
        // Tab=4 + 1 space = 5 spaces
        assert!(result.starts_with("     line1"));
    }

    #[test]
    fn normalize_preserves_empty_lines() {
        let input = "line1\n\nline3";
        let result = whitespace_normalize(input, NormalizeTarget::Spaces(4));
        assert_eq!(result, "line1\n\nline3");
    }

    #[test]
    fn normalize_preserves_trailing_newline() {
        let input = "\tline1\n";
        let result = whitespace_normalize(input, NormalizeTarget::Spaces(4));
        assert!(result.ends_with('\n'));
    }

    #[test]
    fn normalize_report_counts_changes() {
        let original = "\tline1\nline2\n\tline3";
        let normalized = whitespace_normalize(original, NormalizeTarget::Spaces(4));
        let report = NormalizeReport::compare(original, &normalized);
        assert_eq!(report.lines_changed, 2);
        assert_eq!(report.total_lines, 3);
    }

    #[test]
    fn normalize_report_display() {
        let r = NormalizeReport { lines_changed: 5, total_lines: 10 };
        assert_eq!(format!("{r}"), "5/10 lines changed");
    }

    #[test]
    fn whitespace_char_is_helpers() {
        let tab = WhitespaceChar { offset: 0, kind: WhitespaceKind::Tab, width: 4 };
        let space = WhitespaceChar { offset: 0, kind: WhitespaceKind::Space, width: 1 };
        let nbsp = WhitespaceChar { offset: 0, kind: WhitespaceKind::Nbsp, width: 1 };
        assert!(tab.is_tab());
        assert!(!tab.is_space());
        assert!(space.is_space());
        assert!(!space.is_tab());
        assert!(nbsp.is_nbsp());
        assert_eq!(tab.visual_width(8), 8);
        assert_eq!(space.visual_width(8), 1);
    }

    #[test]
    fn config_is_default_and_summary() {
        let cfg = WhitespaceConfig::default();
        assert!(cfg.is_default());
        let custom = WhitespaceConfig { tab_size: 2, ..WhitespaceConfig::default() };
        assert!(!custom.is_default());
        let summary = cfg.summary();
        assert!(summary.contains("4 spaces"));
        assert!(summary.contains("trim-trail"));
    }

    #[test]
    fn whitespace_stats_merge_and_totals() {
        let mut a = WhitespaceStats {
            total_lines: 10,
            blank_lines: 2,
            lines_with_trailing: 1,
            total_trailing_chars: 3,
            tab_count: 5,
            space_count: 10,
            nbsp_count: 0,
        };
        let b = WhitespaceStats {
            total_lines: 5,
            blank_lines: 1,
            lines_with_trailing: 2,
            total_trailing_chars: 4,
            tab_count: 3,
            space_count: 7,
            nbsp_count: 1,
        };
        a.merge(&b);
        assert_eq!(a.total_lines, 15);
        assert_eq!(a.total_whitespace_chars(), 26);
        assert!(a.has_mixed_whitespace());
        let ratio = a.whitespace_ratio(100);
        assert!((ratio - 0.26).abs() < f64::EPSILON);
    }

    #[test]
    fn normalize_report_extensions() {
        let r = NormalizeReport { lines_changed: 3, total_lines: 10 };
        assert!(r.had_changes());
        assert_eq!(r.change_count(), 3);
        assert_eq!(r.unchanged_lines(), 7);
        assert!((r.change_ratio() - 0.3).abs() < f64::EPSILON);
        let no_change = NormalizeReport { lines_changed: 0, total_lines: 5 };
        assert!(!no_change.had_changes());
    }

    #[test]
    fn normalize_target_label_and_display() {
        let spaces = NormalizeTarget::Spaces(4);
        let tabs = NormalizeTarget::Tabs;
        assert_eq!(spaces.label(), "spaces");
        assert_eq!(tabs.label(), "tabs");
        assert_eq!(format!("{spaces}"), "4 spaces");
        assert_eq!(format!("{tabs}"), "tabs");
        assert_eq!(spaces.indent_unit(), "    ");
        assert_eq!(tabs.indent_unit(), "\t");
    }

    #[test]
    fn whitespace_kind_all_and_helpers() {
        let all = WhitespaceKind::all();
        assert_eq!(all.len(), 3);
        assert_eq!(WhitespaceKind::Space.as_char(), ' ');
        assert_eq!(WhitespaceKind::Tab.as_char(), '\t');
        assert_eq!(WhitespaceKind::Nbsp.as_char(), '\u{00A0}');
        assert_eq!(WhitespaceKind::Space.glyph(), '·');
        assert_eq!(WhitespaceKind::Tab.glyph(), '→');
    }

    #[test]
    fn detect_indentation_tabs_and_spaces() {
        let tab_lines = vec!["\tfoo", "\t\tbar", "baz"];
        assert_eq!(detect_indentation(&tab_lines), IndentationStyle::Tab);
        let space_lines = vec!["    foo", "        bar", "baz"];
        assert_eq!(detect_indentation(&space_lines), IndentationStyle::Spaces(4));
        let mixed = vec!["\tfoo", "    bar"];
        assert_eq!(detect_indentation(&mixed), IndentationStyle::Mixed);
        let empty: Vec<&str> = vec!["no_indent", "also_none"];
        assert_eq!(detect_indentation(&empty), IndentationStyle::Unknown);
    }

    #[test]
    fn whitespace_iter_matches_detect() {
        let line = "a b\tc";
        let from_iter: Vec<WhitespaceChar> = whitespace_iter(line, 4).collect();
        let from_detect = detect_whitespace(line, 4);
        assert_eq!(from_iter, from_detect);
    }

    #[test]
    fn indentation_style_display() {
        assert_eq!(IndentationStyle::Tab.to_string(), "tabs");
        assert_eq!(IndentationStyle::Spaces(2).to_string(), "2 spaces");
        assert_eq!(IndentationStyle::Mixed.to_string(), "mixed");
        assert_eq!(IndentationStyle::Unknown.to_string(), "unknown");
    }

    #[test]
    fn leading_whitespace_count_spaces() {
        assert_eq!(leading_whitespace_count("    hello"), 4);
    }

    #[test]
    fn leading_whitespace_count_tabs() {
        assert_eq!(leading_whitespace_count("\t\thello"), 2);
    }

    #[test]
    fn leading_whitespace_count_none() {
        assert_eq!(leading_whitespace_count("hello"), 0);
    }

    #[test]
    fn normalize_nbsp_replaces() {
        let input = "hello\u{00A0}world";
        assert_eq!(normalize_nbsp(input), "hello world");
    }

    #[test]
    fn normalize_nbsp_no_change() {
        assert_eq!(normalize_nbsp("hello world"), "hello world");
    }

    #[test]
    fn is_blank_line_true() {
        assert!(is_blank_line("   \t  "));
        assert!(is_blank_line(""));
    }

    #[test]
    fn is_blank_line_false() {
        assert!(!is_blank_line("  x "));
    }

    #[test]
    fn count_blank_lines_basic() {
        let text = "hello\n\n  \nworld\n";
        assert_eq!(count_blank_lines(text), 2);
    }

    #[test]
    fn extract_indentation_spaces() {
        assert_eq!(extract_indentation("    foo"), "    ");
    }

    #[test]
    fn extract_indentation_none() {
        assert_eq!(extract_indentation("foo"), "");
    }

    #[test]
    fn visual_width_no_tabs() {
        assert_eq!(visual_width("hello", 4), 5);
    }

    #[test]
    fn visual_width_with_tab() {
        assert_eq!(visual_width("\thello", 4), 9);
    }

    #[test]
    fn max_indentation_line_basic() {
        let text = "no indent\n  two\n    four\n  two";
        assert_eq!(max_indentation_line(text, 4), Some(2));
    }

    #[test]
    fn max_indentation_line_single() {
        assert_eq!(max_indentation_line("hello", 4), Some(0));
    }

    #[test]
    fn detect_dominant_indentation_tabs() {
        let text = "\tline1\n\tline2\n line3";
        assert_eq!(detect_dominant_indentation(text), WhitespaceKind::Tab);
    }

    #[test]
    fn detect_dominant_indentation_spaces() {
        let text = "  line1\n  line2\n\tline3";
        assert_eq!(detect_dominant_indentation(text), WhitespaceKind::Space);
    }

    // -- analyse_lines ---------------------------------------------------------

    #[test]
    fn analyse_lines_basic() {
        let info = analyse_lines("  hello  \n\tworld", 4);
        assert_eq!(info.len(), 2);
        assert_eq!(info[0].leading_columns, 2);
        assert_eq!(info[0].trailing_chars, 2);
        assert!(!info[0].is_blank);
    }

    #[test]
    fn analyse_lines_blank() {
        let info = analyse_lines("   \nhello", 4);
        assert!(info[0].is_blank);
        assert!(!info[1].is_blank);
    }

    #[test]
    fn analyse_lines_mixed_indent() {
        let info = analyse_lines("\t code", 4);
        assert_eq!(info[0].indent_style, IndentationStyle::Mixed);
    }

    // -- leading_width ---------------------------------------------------------

    #[test]
    fn leading_width_spaces() {
        assert_eq!(leading_width("    hello", 4), 4);
    }

    #[test]
    fn leading_width_tab() {
        assert_eq!(leading_width("\thello", 4), 4);
    }

    #[test]
    fn leading_width_mixed() {
        // tab at col 0 -> 4, then 2 spaces -> 6
        assert_eq!(leading_width("\t  hello", 4), 6);
    }

    // -- unique_indent_widths --------------------------------------------------

    #[test]
    fn unique_indent_widths_basic() {
        let widths = unique_indent_widths("  a\n    b\n  c\n      d", 4);
        assert_eq!(widths, vec![2, 4, 6]);
    }

    // -- guess_indent_size -----------------------------------------------------

    #[test]
    fn guess_indent_size_two() {
        let text = "  a\n    b\n      c";
        assert_eq!(guess_indent_size(text, 4), Some(2));
    }

    #[test]
    fn guess_indent_size_four() {
        let text = "    a\n        b";
        assert_eq!(guess_indent_size(text, 4), Some(4));
    }

    #[test]
    fn guess_indent_size_none() {
        assert_eq!(guess_indent_size("hello\nworld", 4), None);
    }

    // -- IndentationType tests ------------------------------------------------

    #[test]
    fn detect_line_indentation_tabs() {
        assert_eq!(detect_line_indentation("\thello"), IndentationType::Tabs);
    }

    #[test]
    fn detect_line_indentation_spaces() {
        assert_eq!(detect_line_indentation("    hello"), IndentationType::Spaces);
    }

    #[test]
    fn detect_line_indentation_mixed() {
        assert_eq!(detect_line_indentation("\t hello"), IndentationType::Mixed);
    }

    #[test]
    fn detect_line_indentation_none() {
        assert_eq!(detect_line_indentation("hello"), IndentationType::None);
    }

    #[test]
    fn detect_mixed_indentation_finds_lines() {
        let text = "    a\n\t b\n\tc\n";
        let mixed = detect_mixed_indentation(text);
        assert_eq!(mixed, vec![2]);
    }

    // -- WhitespaceConverter tests --------------------------------------------

    #[test]
    fn tabs_to_spaces_basic() {
        assert_eq!(tabs_to_spaces("\thello", 4), "    hello");
    }

    #[test]
    fn tabs_to_spaces_nested() {
        assert_eq!(tabs_to_spaces("\t\thello", 2), "    hello");
    }

    #[test]
    fn spaces_to_tabs_basic() {
        assert_eq!(spaces_to_tabs("    hello", 4), "\thello");
    }

    #[test]
    fn spaces_to_tabs_remainder() {
        let result = spaces_to_tabs("      hello", 4);
        assert_eq!(result, "\t  hello");
    }

    // -- WhitespaceNormalizer tests --------------------------------------------

    #[test]
    fn trim_trailing_whitespace_removes() {
        assert_eq!(trim_trailing_whitespace("hello   \nworld  "), "hello\nworld");
    }

    #[test]
    fn trim_trailing_blank_lines_keeps_one() {
        assert_eq!(trim_trailing_blank_lines("hello\n\n\n"), "hello\n");
    }

    #[test]
    fn ensure_final_newline_adds() {
        assert_eq!(ensure_final_newline("hello"), "hello\n");
        assert_eq!(ensure_final_newline("hello\n"), "hello\n");
    }

    #[test]
    fn count_trailing_whitespace_lines_counts() {
        assert_eq!(count_trailing_whitespace_lines("hello  \nworld\nfoo \n"), 2);
    }

    // -- WhitespaceVisualization tests -----------------------------------------

    #[test]
    fn visualization_none_mode() {
        let viz = WhitespaceVisualization::default();
        assert_eq!(viz.render_line("  hello  "), "  hello  ");
    }

    #[test]
    fn visualization_all_mode() {
        let viz = WhitespaceVisualization { mode: WhitespaceMode::All, ..Default::default() };
        assert_eq!(viz.render_line(" x "), "·x·");
    }

    #[test]
    fn visualization_trailing_mode() {
        let viz = WhitespaceVisualization { mode: WhitespaceMode::Trailing, ..Default::default() };
        assert_eq!(viz.render_line("hello  "), "hello··");
    }

    #[test]
    fn visualization_display() {
        let viz = WhitespaceVisualization::default();
        let s = viz.to_string();
        assert!(s.contains("none"));
    }

    #[test]
    fn indentation_type_display() {
        assert_eq!(IndentationType::Mixed.to_string(), "mixed");
        assert_eq!(IndentationType::Tabs.to_string(), "tabs");
    }

    // --- WhitespaceGuideRenderer tests --------------------------------------

    #[test]
    fn guide_renderer_depth() {
        let r = WhitespaceGuideRenderer::new(4, GuideStyle::Thin);
        assert_eq!(r.indent_depth("hello"), 0);
        assert_eq!(r.indent_depth("    hello"), 1);
        assert_eq!(r.indent_depth("        hello"), 2);
    }

    #[test]
    fn guide_renderer_tab_depth() {
        let r = WhitespaceGuideRenderer::new(4, GuideStyle::Thin);
        assert_eq!(r.indent_depth("\thello"), 1);
        assert_eq!(r.indent_depth("\t\thello"), 2);
    }

    #[test]
    fn guide_renderer_render() {
        let r = WhitespaceGuideRenderer::new(4, GuideStyle::Thin);
        assert_eq!(r.render_guides("        hello"), "││");
    }

    #[test]
    fn guide_renderer_none_style() {
        let r = WhitespaceGuideRenderer::new(4, GuideStyle::None);
        assert_eq!(r.render_guides("    hello"), "");
    }

    #[test]
    fn guide_renderer_max_depth() {
        let lines: Vec<&str> = vec!["    a", "        b", "c"];
        let r = WhitespaceGuideRenderer::default();
        assert_eq!(r.max_indent_depth(&lines), 2);
    }

    #[test]
    fn guide_renderer_render_all() {
        let r = WhitespaceGuideRenderer::new(4, GuideStyle::Dotted);
        let results = r.render_all(&["hello", "    world"]);
        assert_eq!(results[0].0, 0);
        assert_eq!(results[1].0, 1);
        assert_eq!(r.guide_char(), '┆');
    }

    #[test]
    fn guide_style_display() {
        assert_eq!(GuideStyle::Thin.to_string(), "thin");
        assert_eq!(GuideStyle::Dotted.to_string(), "dotted");
    }

    // --- WhitespaceReport tests ---------------------------------------------

    #[test]
    fn report_basic() {
        let text = "hello   \n    world\n\n";
        let report = WhitespaceReport::analyse(text);
        assert_eq!(report.total_lines, 3);
        assert!(report.lines_with_trailing > 0);
        assert_eq!(report.blank_lines, 1);
    }

    #[test]
    fn report_dominant_indent_spaces() {
        let text = "    a\n    b\n    c";
        let report = WhitespaceReport::analyse(text);
        assert_eq!(report.dominant_indent(), "spaces");
    }

    #[test]
    fn report_trailing_percentage() {
        let text = "ok\ntrailing  \nok2";
        let report = WhitespaceReport::analyse(text);
        assert!(report.trailing_percentage() > 0.0);
    }

    #[test]
    fn report_no_issues() {
        let text = "clean\nlines\nhere";
        let report = WhitespaceReport::analyse(text);
        assert!(!report.has_issues());
    }

    #[test]
    fn report_summary() {
        let text = "a\nb";
        let report = WhitespaceReport::analyse(text);
        let s = report.summary();
        assert!(s.contains("2 lines"));
    }

    #[test]
    fn report_empty() {
        let report = WhitespaceReport::analyse("");
        assert_eq!(report.total_lines, 0);
        assert!((report.trailing_percentage() - 0.0).abs() < f64::EPSILON);
    }

    // --- AutoFix tests ------------------------------------------------------

    #[test]
    fn autofix_trim_trailing() {
        let config = AutoFixConfig::default();
        let result = auto_fix("hello   \nworld  ", &config);
        assert!(result.was_modified());
        assert!(result.fixed.contains("hello"));
        assert!(!result.fixed.contains("hello   "));
    }

    #[test]
    fn autofix_ensure_final_newline() {
        let config = AutoFixConfig {
            trim_trailing: false,
            ensure_final_newline: true,
            trim_final_newlines: false,
            normalize_indent: false,
            target_indent: IndentStyle::Spaces,
            tab_size: 4,
        };
        let result = auto_fix("hello", &config);
        assert!(result.fixed.ends_with('\n'));
    }

    #[test]
    fn autofix_no_change() {
        let config = AutoFixConfig {
            trim_trailing: false,
            ensure_final_newline: false,
            trim_final_newlines: false,
            normalize_indent: false,
            target_indent: IndentStyle::Spaces,
            tab_size: 4,
        };
        let result = auto_fix("hello", &config);
        assert!(!result.was_modified());
    }

    #[test]
    fn autofix_length_delta() {
        let config = AutoFixConfig::default();
        let result = auto_fix("hello   ", &config);
        assert!(result.length_delta() != 0 || result.was_modified());
    }

    #[test]
    fn indent_style_display() {
        assert_eq!(IndentStyle::Spaces.to_string(), "spaces");
        assert_eq!(IndentStyle::Tabs.to_string(), "tabs");
    }

    // --- Scope detection tests ----------------------------------------------

    #[test]
    fn detect_scopes_basic() {
        let text = "top\n    inner\n        deep\ntop2";
        let scopes = detect_scopes(text, 4);
        assert!(!scopes.is_empty());
    }

    #[test]
    fn scope_at_line_basic() {
        let text = "a\n    b\n    c\nd";
        let scopes = detect_scopes(text, 4);
        let s = scope_at_line(&scopes, 1);
        assert!(s.is_some());
    }

    #[test]
    fn indent_scope_line_count() {
        let scope = IndentScope { start_line: 2, end_line: 5, depth: 1 };
        assert_eq!(scope.line_count(), 4);
    }

    #[test]
    fn indent_scope_display() {
        let scope = IndentScope { start_line: 0, end_line: 3, depth: 2 };
        let s = scope.to_string();
        assert!(s.contains("depth 2"));
    }

    #[test]
    fn detect_scopes_empty() {
        let scopes = detect_scopes("", 4);
        assert!(scopes.is_empty());
    }


    // -- whitespace additional tests -------------------------------------------

    #[test]
    fn x_whitespace_text_span_new_ordered() {
        let s = XWhitespaceTextSpan::new(5, 10);
        assert_eq!(s.start, 5);
        assert_eq!(s.end, 10);
    }

    #[test]
    fn x_whitespace_text_span_new_reversed() {
        let s = XWhitespaceTextSpan::new(10, 5);
        assert_eq!(s.start, 5);
        assert_eq!(s.end, 10);
    }

    #[test]
    fn x_whitespace_text_span_len() {
        assert_eq!(XWhitespaceTextSpan::new(3, 7).len(), 4);
        assert_eq!(XWhitespaceTextSpan::new(0, 0).len(), 0);
    }

    #[test]
    fn x_whitespace_text_span_extract() {
        let s = XWhitespaceTextSpan::new(0, 5);
        assert_eq!(s.extract("hello world"), "hello");
    }

    #[test]
    fn x_whitespace_text_span_contains() {
        let s = XWhitespaceTextSpan::new(2, 8);
        assert!(s.contains(2));
        assert!(s.contains(7));
        assert!(!s.contains(8));
    }

    #[test]
    fn x_whitespace_text_span_intersect() {
        let a = XWhitespaceTextSpan::new(0, 10);
        let b = XWhitespaceTextSpan::new(5, 15);
        let inter = a.intersect(&b).unwrap();
        assert_eq!(inter.start, 5);
        assert_eq!(inter.end, 10);
    }

    #[test]
    fn x_whitespace_text_span_intersect_none() {
        let a = XWhitespaceTextSpan::new(0, 5);
        let b = XWhitespaceTextSpan::new(5, 10);
        assert!(a.intersect(&b).is_none());
    }

    #[test]
    fn x_whitespace_text_span_union() {
        let a = XWhitespaceTextSpan::new(3, 7);
        let b = XWhitespaceTextSpan::new(5, 12);
        let u = a.union(&b);
        assert_eq!(u.start, 3);
        assert_eq!(u.end, 12);
    }

    #[test]
    fn x_whitespace_count_lines_basic() {
        assert_eq!(x_whitespace_count_lines("a\nb\nc"), 3);
        assert_eq!(x_whitespace_count_lines(""), 0);
        assert_eq!(x_whitespace_count_lines("single"), 1);
    }

    #[test]
    fn x_whitespace_line_start_offset_basic() {
        assert_eq!(x_whitespace_line_start_offset("abc\ndef\nghi", 0), Some(0));
        assert_eq!(x_whitespace_line_start_offset("abc\ndef\nghi", 1), Some(4));
        assert_eq!(x_whitespace_line_start_offset("abc\ndef\nghi", 2), Some(8));
        assert_eq!(x_whitespace_line_start_offset("abc\ndef\nghi", 3), None);
    }

    #[test]
    fn x_whitespace_indent_level_basic() {
        assert_eq!(x_whitespace_indent_level("    hello"), 4);
        assert_eq!(x_whitespace_indent_level("hello"), 0);
        assert_eq!(x_whitespace_indent_level("  "), 2);
    }

    #[test]
    fn x_whitespace_trim_trailing_basic() {
        let input = "hello   \nworld  \n  foo  ";
        let result = x_whitespace_trim_trailing(input);
        assert_eq!(result, "hello\nworld\n  foo");
    }

    #[test]
    fn x_whitespace_detect_eol_lf() {
        assert_eq!(x_whitespace_detect_eol("a\nb\nc"), "\n");
    }

    #[test]
    fn x_whitespace_detect_eol_crlf() {
        assert_eq!(x_whitespace_detect_eol("a\r\nb\r\nc"), "\r\n");
    }

    #[test]
    fn x_whitespace_tokenize_basic() {
        let tokens = x_whitespace_tokenize("hello, world! foo");
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn x_whitespace_text_span_shift() {
        let s = XWhitespaceTextSpan::new(2, 5).shift(10);
        assert_eq!(s.start, 12);
        assert_eq!(s.end, 15);
    }


    // -- whitespace extended domain tests ----------------------------------------

    #[test]
    fn y_whitespace_enum_index() {
        assert_eq!(YWhitespaceWhitespaceRenderStyle::Dot.index(), 0);
        assert_eq!(YWhitespaceWhitespaceRenderStyle::Arrow.index(), 1);
        assert_eq!(YWhitespaceWhitespaceRenderStyle::Bar.index(), 2);
        assert_eq!(YWhitespaceWhitespaceRenderStyle::Invisible.index(), 3);
    }

    #[test]
    fn y_whitespace_enum_label() {
        assert_eq!(YWhitespaceWhitespaceRenderStyle::Dot.label(), "Dot");
        assert_eq!(YWhitespaceWhitespaceRenderStyle::Arrow.label(), "Arrow");
        assert_eq!(YWhitespaceWhitespaceRenderStyle::Bar.label(), "Bar");
        assert_eq!(YWhitespaceWhitespaceRenderStyle::Invisible.label(), "Invisible");
    }

    #[test]
    fn y_whitespace_enum_all() {
        let all = YWhitespaceWhitespaceRenderStyle::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_whitespace_enum_is_default() {
        assert!(YWhitespaceWhitespaceRenderStyle::Dot.is_default());
        assert!(!YWhitespaceWhitespaceRenderStyle::Invisible.is_default());
    }

    #[test]
    fn y_whitespace_enum_display() {
        assert_eq!(format!("{}", YWhitespaceWhitespaceRenderStyle::Dot), "Dot");
    }

    #[test]
    fn y_whitespace_struct_new() {
        let s = YWhitespaceWhitespaceAnalysis::new();
        let _ = s.summary();
    }

    #[test]
    fn y_whitespace_fingerprint_deterministic() {
        let h1 = y_whitespace_fingerprint("hello");
        let h2 = y_whitespace_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_whitespace_fingerprint("a"), y_whitespace_fingerprint("b"));
    }

    #[test]
    fn y_whitespace_truncate_short() {
        assert_eq!(y_whitespace_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_whitespace_truncate_long() {
        let r = y_whitespace_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_whitespace_normalize_key_basic() {
        assert_eq!(y_whitespace_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_whitespace_split_path_basic() {
        let parts = y_whitespace_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_whitespace_count_occurrences_basic() {
        assert_eq!(y_whitespace_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_whitespace_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_whitespace_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_whitespace_in_range_basic() {
        assert!(y_whitespace_in_range(5, 1, 10));
        assert!(y_whitespace_in_range(1, 1, 10));
        assert!(y_whitespace_in_range(10, 1, 10));
        assert!(!y_whitespace_in_range(0, 1, 10));
        assert!(!y_whitespace_in_range(11, 1, 10));
    }

    #[test]
    fn y_whitespace_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_whitespace_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_whitespace_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_whitespace_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- whitespace Z-extended tests -----------------------------------------------

    #[test]
    fn z_whitespace_priority_weight() {
        assert_eq!(ZWhitespacePriority::Idle.weight(), 0);
        assert_eq!(ZWhitespacePriority::Normal.weight(), 2);
        assert_eq!(ZWhitespacePriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_whitespace_priority_label() {
        assert_eq!(ZWhitespacePriority::Low.label(), "low");
        assert_eq!(ZWhitespacePriority::High.label(), "high");
    }

    #[test]
    fn z_whitespace_priority_is_elevated() {
        assert!(!ZWhitespacePriority::Normal.is_elevated());
        assert!(ZWhitespacePriority::High.is_elevated());
        assert!(ZWhitespacePriority::Realtime.is_elevated());
    }

    #[test]
    fn z_whitespace_priority_display() {
        assert_eq!(format!("{}", ZWhitespacePriority::Idle), "idle");
    }

    #[test]
    fn z_whitespace_priority_all_asc() {
        let all = ZWhitespacePriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZWhitespacePriority::Idle);
        assert_eq!(all[4], ZWhitespacePriority::Realtime);
    }

    #[test]
    fn z_whitespace_struct_new() {
        let s = ZWhitespaceWhitespaceReport::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_whitespace_struct_toggled_clone() {
        let s = ZWhitespaceWhitespaceReport::new();
        let t = s.toggled_clone();
        let _ = t.space_lines;
    }

    #[test]
    fn z_whitespace_rolling_hash_deterministic() {
        let h1 = z_whitespace_rolling_hash(b"test");
        let h2 = z_whitespace_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_whitespace_rolling_hash(b"a"), z_whitespace_rolling_hash(b"b"));
    }

    #[test]
    fn z_whitespace_pad_to_basic() {
        assert_eq!(z_whitespace_pad_to("hi", 5), "hi   ");
        assert_eq!(z_whitespace_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_whitespace_is_identifier_basic() {
        assert!(z_whitespace_is_identifier("foo_bar"));
        assert!(z_whitespace_is_identifier("abc123"));
        assert!(!z_whitespace_is_identifier(""));
        assert!(!z_whitespace_is_identifier("has space"));
    }

    #[test]
    fn z_whitespace_levenshtein_basic() {
        assert_eq!(z_whitespace_levenshtein("", ""), 0);
        assert_eq!(z_whitespace_levenshtein("abc", "abc"), 0);
        assert_eq!(z_whitespace_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_whitespace_unique_words_basic() {
        let w = z_whitespace_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_whitespace_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_whitespace_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_whitespace_common_prefix_basic() {
        assert_eq!(z_whitespace_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_whitespace_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_whitespace_struct_clear() {
        let mut s = ZWhitespaceWhitespaceReport::new();
        s.lines_checked.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_whitespace_rolling_hash_empty() {
        let h = z_whitespace_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }
}
