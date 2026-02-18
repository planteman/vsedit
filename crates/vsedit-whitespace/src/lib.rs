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


// ---------------------------------------------------------------------------
// xb_ utilities – batch 41
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer41 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer41 {
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
pub fn xb_fnv1a_41(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_41<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_41<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_41(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_41(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 236
// ---------------------------------------------------------------------------

/// Generic object pool `Xc236Pool<T>`.
pub struct Xc236Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc236Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc236PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc236Pool<T> {
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
    pub fn stats(&self) -> Xc236PoolStats {
        Xc236PoolStats {
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

impl<T> Default for Xc236Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc236Scheduler`.
pub struct Xc236Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc236Scheduler {
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

impl Default for Xc236Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_236 hash for the given byte slice.
pub fn xc_236_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_236 convention.
pub fn xc_236_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe54 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe54Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe54PipelineError {
    pub stage: Xe54Stage,
    pub message: String,
}

impl std::fmt::Display for Xe54PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe54Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe54Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe54PipelineError>>>,
    stage_names: Vec<Xe54Stage>,
}

impl Xe54Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe54PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe54Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe54PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe54Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe54PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe54Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe54PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe54Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe54PipelineError> {
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

    pub fn compose(mut self, other: Xe54Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe54CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe54CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe54Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe54CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe54CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe54Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe54CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_54_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe54CacheEntry {
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

    fn xe_54_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe54CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_54_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe54PipelineError> {
    Ok(data)
}

pub fn xe_54_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe54PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_54_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe54PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_54_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe54PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_54_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe54PipelineError> {
    Err(Xe54PipelineError {
        stage: Xe54Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_52: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg52Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg52Graph {
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

impl Default for Xg52Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_52: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg52Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg52Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg52Heap<T>) {
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

impl<T: Ord> Default for Xg52Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 235).
pub struct Xh235SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh235SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 277 as u64,
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

/// A compact bit set supporting boolean operations (variant 235).
pub struct Xh235BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh235BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 235).
pub struct Xi235Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi235Deque<T> {
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
pub struct Xi235Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi235Interval {
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

/// A simple interval tree (variant 235).
pub struct Xi235IntervalTree {
    xi_intervals: Vec<Xi235Interval>,
}

impl Xi235IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi235Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi235Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi235Interval) -> Vec<&Xi235Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi235Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi235Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi235Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi235Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi235Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi235Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 235) ---

/// Disjoint set / union-find for crate 235.
pub struct Xj235UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj235UnionFind {
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

const XJ235_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 235.
pub struct Xj235BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj235BTreeNode<K, V>>>,
    len: usize,
}

struct Xj235BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj235BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj235BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ235_BTREE_ORDER - 1
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
        let mid = XJ235_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj235BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj235BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj235BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj235BTreeNode::xj_new_leaf();
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


// --- xk_235 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk235SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk235SegmentTree {
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
pub struct Xk235DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk235DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_235).
#[derive(Debug, Clone)]
pub struct Xl235Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl235Rope {
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

/// Suffix array for efficient string searching (xl_235).
#[derive(Debug, Clone)]
pub struct Xl235SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl235SuffixArray {
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

    #[test]
    fn xb_ring_buffer_41_push_and_len() {
        let mut rb = super::XbRingBuffer41::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_41_overwrite() {
        let mut rb = super::XbRingBuffer41::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_41_get_out_of_bounds() {
        let rb = super::XbRingBuffer41::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_41_drain_all() {
        let mut rb = super::XbRingBuffer41::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_41_peek_front_back() {
        let mut rb = super::XbRingBuffer41::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_41_clear() {
        let mut rb = super::XbRingBuffer41::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_41_capacity() {
        let rb = super::XbRingBuffer41::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_41_basic() {
        let h = super::xb_fnv1a_41(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_41(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_41_different_inputs() {
        let h1 = super::xb_fnv1a_41(b"abc");
        let h2 = super::xb_fnv1a_41(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_41_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_41(&data);
        let dec = super::xb_rle_decode_41(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_41_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_41(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_41(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_41_values() {
        assert!((super::xb_clamp_41(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_41(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_41(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_41_values() {
        assert!((super::xb_lerp_41(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_41(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_41(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_41_wrap_around_twice() {
        let mut rb = super::XbRingBuffer41::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 236 ----

    #[test]
    fn xc_236_pool_new_empty() {
        let pool: super::Xc236Pool<i32> = super::Xc236Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_236_pool_release_acquire() {
        let mut pool = super::Xc236Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_236_pool_acquire_empty() {
        let mut pool: super::Xc236Pool<i32> = super::Xc236Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_236_pool_full() {
        let mut pool = super::Xc236Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_236_pool_drain() {
        let mut pool = super::Xc236Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_236_pool_stats() {
        let mut pool = super::Xc236Pool::new(8);
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
    fn xc_236_pool_clear() {
        let mut pool = super::Xc236Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_236_pool_shrink() {
        let mut pool = super::Xc236Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_236_pool_default() {
        let pool: super::Xc236Pool<String> = super::Xc236Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_236_pool_extend() {
        let mut pool = super::Xc236Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_236_pool_retain() {
        let mut pool = super::Xc236Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_236_scheduler_round_robin() {
        let mut sched = super::Xc236Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_236_scheduler_empty() {
        let mut sched = super::Xc236Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_236_scheduler_reset() {
        let mut sched = super::Xc236Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_236_scheduler_add_remove() {
        let mut sched = super::Xc236Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_236_scheduler_targets() {
        let sched = super::Xc236Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_236_hash_empty() {
        assert_eq!(super::xc_236_hash(b""), 5381);
    }

    #[test]
    fn xc_236_hash_data() {
        let h = super::xc_236_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_236_hash(b"hello"), h);
    }

    #[test]
    fn xc_236_reverse_str() {
        assert_eq!(super::xc_236_reverse("abc"), "cba");
        assert_eq!(super::xc_236_reverse(""), "");
    }


    #[test]
    fn xe_54_pipeline_empty() {
        let p = super::Xe54Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_54_pipeline_parse_stage() {
        let p = super::Xe54Pipeline::new()
            .add_parse(super::xe_54_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_54_pipeline_transform_double() {
        let p = super::Xe54Pipeline::new()
            .add_transform(super::xe_54_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_54_pipeline_validate_reverse() {
        let p = super::Xe54Pipeline::new()
            .add_validate(super::xe_54_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_54_pipeline_emit_filter() {
        let p = super::Xe54Pipeline::new()
            .add_emit(super::xe_54_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_54_pipeline_multi_stage() {
        let p = super::Xe54Pipeline::new()
            .add_parse(super::xe_54_pipeline_identity)
            .add_transform(super::xe_54_pipeline_double)
            .add_validate(super::xe_54_pipeline_reverse)
            .add_emit(super::xe_54_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_54_pipeline_error_propagation() {
        let p = super::Xe54Pipeline::new()
            .add_parse(super::xe_54_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe54Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_54_pipeline_compose() {
        let p1 = super::Xe54Pipeline::new()
            .add_parse(super::xe_54_pipeline_identity);
        let p2 = super::Xe54Pipeline::new()
            .add_transform(super::xe_54_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_54_pipeline_error_display() {
        let e = super::Xe54PipelineError {
            stage: super::Xe54Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_54_cache_put_get() {
        let mut c = super::Xe54Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_54_cache_miss() {
        let mut c: super::Xe54Cache<&str, i32> = super::Xe54Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_54_cache_ttl_expiry() {
        let mut c = super::Xe54Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_54_cache_evict() {
        let mut c = super::Xe54Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_54_cache_capacity() {
        let mut c = super::Xe54Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_54_cache_stats() {
        let mut c = super::Xe54Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_54_cache_clear() {
        let mut c = super::Xe54Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_52 graph tests ------------------------------------------------

    #[test]
    fn xg_52_graph_empty() {
        let g = super::Xg52Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_52_graph_add_node() {
        let mut g = super::Xg52Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_52_graph_add_edge() {
        let mut g = super::Xg52Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_52_graph_neighbors() {
        let mut g = super::Xg52Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_52_graph_has_path() {
        let mut g = super::Xg52Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_52_graph_self_path() {
        let g = super::Xg52Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_52_graph_topo_sort() {
        let mut g = super::Xg52Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_52_graph_cycle_detect_false() {
        let mut g = super::Xg52Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_52_graph_cycle_detect_true() {
        let mut g = super::Xg52Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_52 heap tests -------------------------------------------------

    #[test]
    fn xg_52_heap_empty() {
        let h: super::Xg52Heap<i32> = super::Xg52Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_52_heap_push_pop() {
        let mut h = super::Xg52Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_52_heap_peek() {
        let mut h = super::Xg52Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_52_heap_drain_sorted() {
        let mut h = super::Xg52Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_52_heap_merge() {
        let mut a = super::Xg52Heap::new();
        let mut b = super::Xg52Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_52_heap_default() {
        let h: super::Xg52Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_52_graph_default() {
        let g: super::Xg52Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh235_skip_insert_contains() {
        let mut sl = super::Xh235SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh235_skip_remove() {
        let mut sl = super::Xh235SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh235_skip_len() {
        let mut sl = super::Xh235SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh235_skip_range_query() {
        let mut sl = super::Xh235SkipList::xh_new(4);
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
    fn xh235_skip_floor_ceiling() {
        let mut sl = super::Xh235SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh235_skip_rank() {
        let mut sl = super::Xh235SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh235_skip_empty() {
        let sl = super::Xh235SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh235_skip_duplicates() {
        let mut sl = super::Xh235SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh235_bitset_set_test() {
        let mut bs = super::Xh235BitSet::xh_new(256);
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
    fn xh235_bitset_clear_count() {
        let mut bs = super::Xh235BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh235_bitset_and_or_xor() {
        let mut a = super::Xh235BitSet::xh_new(128);
        let mut b = super::Xh235BitSet::xh_new(128);
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
    fn xh235_bitset_iter_ones() {
        let mut bs = super::Xh235BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh235_bitset_first_last() {
        let mut bs = super::Xh235BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh235_bitset_empty() {
        let bs = super::Xh235BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi235_deque_push_pop_back() {
        let mut dq = super::Xi235Deque::xi_new(4);
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
    fn xi235_deque_push_pop_front() {
        let mut dq = super::Xi235Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi235_deque_mixed_ops() {
        let mut dq = super::Xi235Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi235_deque_get_and_split() {
        let mut dq = super::Xi235Deque::xi_new(8);
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
    fn xi235_deque_rotate_left() {
        let mut dq = super::Xi235Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi235_deque_rotate_right() {
        let mut dq = super::Xi235Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi235_deque_grow() {
        let mut dq = super::Xi235Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi235_deque_empty() {
        let dq = super::Xi235Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi235_interval_tree_insert_query() {
        let mut tree = super::Xi235IntervalTree::xi_new();
        tree.xi_insert(super::Xi235Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi235Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi235Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi235_interval_tree_overlap() {
        let mut tree = super::Xi235IntervalTree::xi_new();
        tree.xi_insert(super::Xi235Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi235Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi235Interval::xi_new(12, 20));
        let q = super::Xi235Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi235_interval_tree_remove() {
        let mut tree = super::Xi235IntervalTree::xi_new();
        tree.xi_insert(super::Xi235Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi235Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi235_interval_tree_gaps() {
        let mut tree = super::Xi235IntervalTree::xi_new();
        tree.xi_insert(super::Xi235Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi235Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi235Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi235Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi235Interval::xi_new(8, 10));
    }

    #[test]
    fn xi235_interval_tree_merge() {
        let mut tree = super::Xi235IntervalTree::xi_new();
        tree.xi_insert(super::Xi235Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi235Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi235Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi235Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi235Interval::xi_new(10, 15));
    }

    #[test]
    fn xi235_interval_tree_all() {
        let mut tree = super::Xi235IntervalTree::xi_new();
        tree.xi_insert(super::Xi235Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi235Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi235_interval_tree_empty() {
        let tree = super::Xi235IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi235_interval_tree_contains_point() {
        let iv = super::Xi235Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 235) ---

    #[test]
    fn xj_235_uf_make_and_find() {
        let mut uf = super::Xj235UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_235_uf_union_connected() {
        let mut uf = super::Xj235UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_235_uf_component_count() {
        let mut uf = super::Xj235UnionFind::xj_new();
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
    fn xj_235_uf_component_size() {
        let mut uf = super::Xj235UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_235_uf_largest_component() {
        let mut uf = super::Xj235UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_235_uf_many_elements() {
        let mut uf = super::Xj235UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_235_uf_separate_components() {
        let mut uf = super::Xj235UnionFind::xj_new();
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
    fn xj_235_uf_path_compression() {
        let mut uf = super::Xj235UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_235_bt_insert_get() {
        let mut bt = super::Xj235BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_235_bt_contains_len() {
        let mut bt = super::Xj235BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_235_bt_replace() {
        let mut bt = super::Xj235BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_235_bt_remove() {
        let mut bt = super::Xj235BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_235_bt_keys_values() {
        let mut bt = super::Xj235BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_235_bt_range() {
        let mut bt = super::Xj235BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_235_bt_min_max() {
        let mut bt = super::Xj235BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_235_bt_many_inserts() {
        let mut bt = super::Xj235BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_235 segment tree tests ---

    #[test]
    fn xk_235_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk235SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_235_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk235SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_235_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk235SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_235_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk235SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_235_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk235SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_235_st_single_element() {
        let data = vec![42];
        let st = super::Xk235SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_235_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk235SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_235_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk235SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_235 disjoint intervals tests ---

    #[test]
    fn xk_235_di_add_and_count() {
        let mut di = super::Xk235DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_235_di_merge_overlap() {
        let mut di = super::Xk235DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_235_di_contains() {
        let mut di = super::Xk235DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_235_di_remove() {
        let mut di = super::Xk235DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_235_di_covered_length() {
        let mut di = super::Xk235DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_235_di_gaps() {
        let mut di = super::Xk235DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_235_di_merge_adjacent() {
        let mut di = super::Xk235DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_235_di_empty() {
        let di = super::Xk235DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_235_rope_new_empty() {
        let rope = super::Xl235Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_235_rope_from_str() {
        let rope = super::Xl235Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_235_rope_insert_at() {
        let mut rope = super::Xl235Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_235_rope_delete_range() {
        let mut rope = super::Xl235Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_235_rope_char_at() {
        let rope = super::Xl235Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_235_rope_split_concat() {
        let rope = super::Xl235Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_235_rope_line_count() {
        let rope = super::Xl235Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_235_rope_line_at() {
        let rope = super::Xl235Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_235_sa_build_and_search() {
        let sa = super::Xl235SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_235_sa_count() {
        let sa = super::Xl235SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_235_sa_longest_repeated() {
        let sa = super::Xl235SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_235_sa_all_positions() {
        let sa = super::Xl235SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_235_sa_len() {
        let sa = super::Xl235SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_235_sa_empty() {
        let sa = super::Xl235SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_235_rope_slice() {
        let rope = super::Xl235Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_235_sa_search_start() {
        let sa = super::Xl235SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }
}
