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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_trailing() {
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
}
