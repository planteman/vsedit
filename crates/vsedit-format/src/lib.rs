//! Document formatting support.

use std::fmt;

/// A text edit from formatting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub new_text: String,
}

/// Options for formatting.
#[derive(Debug, Clone)]
pub struct FormattingOptions {
    pub tab_size: u32,
    pub insert_spaces: bool,
    pub trim_trailing_whitespace: bool,
    pub insert_final_newline: bool,
    pub trim_final_newlines: bool,
}

impl Default for FormattingOptions {
    fn default() -> Self {
        Self {
            tab_size: 4,
            insert_spaces: true,
            trim_trailing_whitespace: true,
            insert_final_newline: true,
            trim_final_newlines: true,
        }
    }
}

/// Provider for document formatting.
pub trait DocumentFormattingProvider: Send + Sync {
    fn format_document(&self, uri: &str, options: &FormattingOptions) -> Vec<TextEdit>;
}

/// Provider for range formatting.
pub trait DocumentRangeFormattingProvider: Send + Sync {
    fn format_range(&self, uri: &str, start_line: u32, end_line: u32, options: &FormattingOptions) -> Vec<TextEdit>;
}

/// Provider for on-type formatting.
pub trait OnTypeFormattingProvider: Send + Sync {
    fn trigger_characters(&self) -> Vec<char>;
    fn format_on_type(&self, uri: &str, line: u32, column: u32, ch: char, options: &FormattingOptions) -> Vec<TextEdit>;
}

/// Apply basic whitespace formatting.
pub fn format_whitespace(text: &str, options: &FormattingOptions) -> String {
    let mut lines: Vec<String> = text.lines().map(|l| {
        let mut line = l.to_string();
        if options.trim_trailing_whitespace {
            line = line.trim_end().to_string();
        }
        line
    }).collect();

    if options.trim_final_newlines {
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
    }

    let mut result = lines.join("\n");
    if options.insert_final_newline && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// The kind of edit operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    /// Insert new text at a position (start == end).
    Insert,
    /// Delete text in a range (new_text is empty).
    Delete,
    /// Replace text in a range with new text.
    Replace,
}

impl TextEdit {
    /// Determine the kind of this edit.
    pub fn kind(&self) -> EditKind {
        let same_pos = self.start_line == self.end_line && self.start_column == self.end_column;
        if same_pos {
            EditKind::Insert
        } else if self.new_text.is_empty() {
            EditKind::Delete
        } else {
            EditKind::Replace
        }
    }

    /// Apply this single edit to the given text, returning the result.
    pub fn apply(&self, text: &str) -> String {
        let lines: Vec<&str> = text.lines().collect();
        let mut result = String::new();

        for (i, line) in lines.iter().enumerate() {
            let li = i as u32;
            if li < self.start_line || li > self.end_line {
                result.push_str(line);
                result.push('\n');
            } else if li == self.start_line && li == self.end_line {
                let col_s = self.start_column as usize;
                let col_e = self.end_column as usize;
                result.push_str(&line[..col_s.min(line.len())]);
                result.push_str(&self.new_text);
                result.push_str(&line[col_e.min(line.len())..]);
                result.push('\n');
            } else if li == self.start_line {
                let col_s = self.start_column as usize;
                result.push_str(&line[..col_s.min(line.len())]);
                result.push_str(&self.new_text);
            } else if li == self.end_line {
                let col_e = self.end_column as usize;
                result.push_str(&line[col_e.min(line.len())..]);
                result.push('\n');
            }
            // lines strictly between start and end are dropped (replaced)
        }

        // Preserve trailing content if text had no trailing newline
        if !text.ends_with('\n') && result.ends_with('\n') {
            result.pop();
        }
        result
    }
}

impl fmt::Display for TextEdit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}:{}-{}:{}] {:?} \"{}\"",
            self.start_line,
            self.start_column,
            self.end_line,
            self.end_column,
            self.kind(),
            self.new_text,
        )
    }
}

/// Apply multiple edits to text. Edits are sorted by position in reverse
/// order so that earlier edits don't shift the positions of later ones.
pub fn apply_edits(text: &str, edits: &[TextEdit]) -> String {
    let mut sorted: Vec<&TextEdit> = edits.iter().collect();
    sorted.sort_by(|a, b| {
        b.start_line
            .cmp(&a.start_line)
            .then(b.start_column.cmp(&a.start_column))
    });

    let mut result = text.to_string();
    for edit in sorted {
        result = edit.apply(&result);
    }
    result
}

/// Builder for `FormattingOptions`.
#[derive(Debug, Clone)]
pub struct FormattingOptionsBuilder {
    options: FormattingOptions,
}

impl FormattingOptionsBuilder {
    pub fn new() -> Self {
        Self {
            options: FormattingOptions::default(),
        }
    }

    pub fn tab_size(mut self, size: u32) -> Self {
        self.options.tab_size = size;
        self
    }

    pub fn insert_spaces(mut self, yes: bool) -> Self {
        self.options.insert_spaces = yes;
        self
    }

    pub fn trim_trailing_whitespace(mut self, yes: bool) -> Self {
        self.options.trim_trailing_whitespace = yes;
        self
    }

    pub fn insert_final_newline(mut self, yes: bool) -> Self {
        self.options.insert_final_newline = yes;
        self
    }

    pub fn trim_final_newlines(mut self, yes: bool) -> Self {
        self.options.trim_final_newlines = yes;
        self
    }

    pub fn build(self) -> FormattingOptions {
        self.options
    }
}

impl Default for FormattingOptionsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert all leading tabs to spaces using the given tab size.
pub fn convert_tabs_to_spaces(text: &str, tab_size: u32) -> String {
    let spaces: String = " ".repeat(tab_size as usize);
    text.lines()
        .map(|line| {
            let leading_tabs = line.len() - line.trim_start_matches('\t').len();
            if leading_tabs > 0 {
                format!("{}{}", spaces.repeat(leading_tabs), &line[leading_tabs..])
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Normalize line endings to LF.
pub fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Count the indentation level of a line (number of leading whitespace groups of tab_size).
pub fn indentation_level(line: &str, tab_size: u32) -> u32 {
    let ts = tab_size as usize;
    if ts == 0 { return 0; }
    let leading_spaces = line.len() - line.trim_start_matches(' ').len();
    let leading_tabs = line.len() - line.trim_start_matches('\t').len();
    if leading_tabs > 0 {
        leading_tabs as u32
    } else {
        (leading_spaces / ts) as u32
    }
}

/// Reindent the given text to the specified level using the formatting options.
pub fn set_indentation(text: &str, level: u32, options: &FormattingOptions) -> String {
    let indent_unit = if options.insert_spaces {
        " ".repeat(options.tab_size as usize)
    } else {
        "\t".to_string()
    };
    let prefix = indent_unit.repeat(level as usize);
    text.lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.is_empty() {
                String::new()
            } else {
                format!("{prefix}{trimmed}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Detect the dominant indentation style in a document.
/// Returns (insert_spaces, tab_size).
pub fn detect_indentation(text: &str) -> (bool, u32) {
    let mut space_counts: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut tab_lines = 0usize;
    let mut space_lines = 0usize;

    for line in text.lines() {
        if line.starts_with('\t') {
            tab_lines += 1;
        } else {
            let leading = line.len() - line.trim_start_matches(' ').len();
            if leading > 0 {
                space_lines += 1;
                *space_counts.entry(leading).or_insert(0) += 1;
            }
        }
    }

    if tab_lines > space_lines {
        return (false, 4);
    }

    // Find most common space indent that divides evenly
    let mut best_size = 4u32;
    let mut best_count = 0usize;
    for size in [2, 4, 8] {
        let count: usize = space_counts
            .iter()
            .filter(|(spaces, _)| **spaces % size == 0)
            .map(|(_, c)| *c)
            .sum();
        if count > best_count {
            best_count = count;
            best_size = size as u32;
        }
    }

    (true, best_size)
}

/// Count the number of non-empty lines in text.
pub fn count_non_empty_lines(text: &str) -> usize {
    text.lines().filter(|l| !l.trim().is_empty()).count()
}

/// Ensure consistent indentation by normalizing mixed tabs/spaces to the configured style.
pub fn normalize_indentation(text: &str, options: &FormattingOptions) -> String {
    let tab_equivalent = options.tab_size as usize;
    text.lines()
        .map(|line| {
            let content_start = line.len() - line.trim_start().len();
            let leading = &line[..content_start];
            let rest = &line[content_start..];

            // Count total equivalent spaces
            let mut total_spaces = 0usize;
            for ch in leading.chars() {
                match ch {
                    '\t' => total_spaces += tab_equivalent,
                    ' ' => total_spaces += 1,
                    _ => break,
                }
            }

            let new_leading = if options.insert_spaces {
                " ".repeat(total_spaces)
            } else {
                let tabs = total_spaces / tab_equivalent;
                let spaces = total_spaces % tab_equivalent;
                format!("{}{}", "\t".repeat(tabs), " ".repeat(spaces))
            };

            format!("{new_leading}{rest}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl TextEdit {
    /// Whether this edit is a no-op (same position, empty new_text).
    pub fn is_noop(&self) -> bool {
        self.start_line == self.end_line
            && self.start_column == self.end_column
            && self.new_text.is_empty()
    }

    /// Number of lines in the new text.
    pub fn new_text_line_count(&self) -> usize {
        if self.new_text.is_empty() {
            0
        } else {
            self.new_text.lines().count().max(1)
        }
    }
}

/// Convert leading spaces (in multiples of tab_size) to tabs.
pub fn convert_spaces_to_tabs(text: &str, tab_size: u32) -> String {
    let ts = tab_size as usize;
    text.lines()
        .map(|line| {
            let leading_spaces = line.len() - line.trim_start_matches(' ').len();
            let tab_count = leading_spaces / ts;
            let remaining_spaces = leading_spaces % ts;
            if tab_count > 0 {
                format!(
                    "{}{}{}",
                    "\t".repeat(tab_count),
                    " ".repeat(remaining_spaces),
                    &line[leading_spaces..]
                )
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Accumulated statistics for format operations.
#[derive(Debug, Clone, PartialEq)]
pub struct FormatStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl FormatStats {
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
    pub fn merge(&mut self, other: &FormatStats) {
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

impl Default for FormatStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for FormatStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FormatStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for format.
#[derive(Debug, Clone)]
pub struct FormatValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl FormatValidator {
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

impl Default for FormatValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_trims_trailing() {
        let result = format_whitespace("hello   \nworld  \n", &FormattingOptions::default());
        assert_eq!(result, "hello\nworld\n");
    }

    #[test]
    fn format_inserts_final_newline() {
        let result = format_whitespace("hello", &FormattingOptions::default());
        assert!(result.ends_with('\n'));
    }

    #[test]
    fn format_trims_final_newlines() {
        let result = format_whitespace("hello\n\n\n", &FormattingOptions::default());
        assert_eq!(result, "hello\n");
    }

    #[test]
    fn formatting_options_default() {
        let opts = FormattingOptions::default();
        assert_eq!(opts.tab_size, 4);
        assert!(opts.insert_spaces);
    }

    #[test]
    fn edit_kind_insert() {
        let edit = TextEdit {
            start_line: 0, start_column: 5, end_line: 0, end_column: 5,
            new_text: "world".into(),
        };
        assert_eq!(edit.kind(), EditKind::Insert);
    }

    #[test]
    fn edit_kind_delete() {
        let edit = TextEdit {
            start_line: 0, start_column: 0, end_line: 0, end_column: 5,
            new_text: String::new(),
        };
        assert_eq!(edit.kind(), EditKind::Delete);
    }

    #[test]
    fn edit_kind_replace() {
        let edit = TextEdit {
            start_line: 0, start_column: 0, end_line: 0, end_column: 5,
            new_text: "hi".into(),
        };
        assert_eq!(edit.kind(), EditKind::Replace);
    }

    #[test]
    fn apply_single_edit() {
        let edit = TextEdit {
            start_line: 0, start_column: 5, end_line: 0, end_column: 5,
            new_text: " world".into(),
        };
        assert_eq!(edit.apply("hello"), "hello world");
    }

    #[test]
    fn apply_multiple_edits() {
        let text = "aaa bbb ccc";
        let edits = vec![
            TextEdit {
                start_line: 0, start_column: 0, end_line: 0, end_column: 3,
                new_text: "AAA".into(),
            },
            TextEdit {
                start_line: 0, start_column: 8, end_line: 0, end_column: 11,
                new_text: "CCC".into(),
            },
        ];
        assert_eq!(apply_edits(text, &edits), "AAA bbb CCC");
    }

    #[test]
    fn text_edit_display() {
        let edit = TextEdit {
            start_line: 1, start_column: 0, end_line: 1, end_column: 3,
            new_text: "foo".into(),
        };
        let s = format!("{}", edit);
        assert!(s.contains("1:0-1:3"));
        assert!(s.contains("Replace"));
    }

    #[test]
    fn builder_pattern() {
        let opts = FormattingOptionsBuilder::new()
            .tab_size(2)
            .insert_spaces(false)
            .trim_trailing_whitespace(false)
            .build();
        assert_eq!(opts.tab_size, 2);
        assert!(!opts.insert_spaces);
        assert!(!opts.trim_trailing_whitespace);
    }

    #[test]
    fn tabs_to_spaces() {
        let input = "\t\thello";
        let result = convert_tabs_to_spaces(input, 4);
        assert_eq!(result, "        hello");
    }

    #[test]
    fn spaces_to_tabs() {
        let input = "        hello";
        let result = convert_spaces_to_tabs(input, 4);
        assert_eq!(result, "\t\thello");
    }

    #[test]
    fn normalize_line_endings_crlf() {
        assert_eq!(normalize_line_endings("a\r\nb\r\n"), "a\nb\n");
    }

    #[test]
    fn normalize_line_endings_cr() {
        assert_eq!(normalize_line_endings("a\rb\r"), "a\nb\n");
    }

    #[test]
    fn normalize_line_endings_lf_unchanged() {
        assert_eq!(normalize_line_endings("a\nb\n"), "a\nb\n");
    }

    #[test]
    fn indentation_level_spaces() {
        assert_eq!(indentation_level("        hello", 4), 2);
        assert_eq!(indentation_level("    hello", 4), 1);
        assert_eq!(indentation_level("hello", 4), 0);
    }

    #[test]
    fn indentation_level_tabs() {
        assert_eq!(indentation_level("\t\thello", 4), 2);
        assert_eq!(indentation_level("\thello", 4), 1);
    }

    #[test]
    fn set_indentation_spaces() {
        let opts = FormattingOptions::default();
        let result = set_indentation("hello\nworld", 2, &opts);
        assert!(result.starts_with("        hello"));
    }

    #[test]
    fn set_indentation_tabs() {
        let mut opts = FormattingOptions::default();
        opts.insert_spaces = false;
        let result = set_indentation("hello", 1, &opts);
        assert_eq!(result, "\thello");
    }

    #[test]
    fn detect_indentation_spaces() {
        let text = "    a\n    b\n        c\n";
        let (spaces, size) = detect_indentation(text);
        assert!(spaces);
        // 4 and 8 are both divisible by 2, so the algorithm picks the smallest fitting size
        assert!(size == 2 || size == 4);
    }

    #[test]
    fn detect_indentation_tabs() {
        let text = "\ta\n\t\tb\n\tc\n";
        let (spaces, _) = detect_indentation(text);
        assert!(!spaces);
    }

    #[test]
    fn count_non_empty_lines_basic() {
        assert_eq!(count_non_empty_lines("a\n\nb\n  \nc\n"), 3);
        assert_eq!(count_non_empty_lines(""), 0);
    }

    #[test]
    fn normalize_indentation_mixed() {
        let opts = FormattingOptions::default(); // spaces, tab_size=4
        let input = "\t hello"; // tab + space
        let result = normalize_indentation(input, &opts);
        assert_eq!(result, "     hello"); // 5 spaces
    }

    #[test]
    fn text_edit_is_noop() {
        let noop = TextEdit {
            start_line: 0, start_column: 0, end_line: 0, end_column: 0,
            new_text: String::new(),
        };
        assert!(noop.is_noop());
        let not_noop = TextEdit {
            start_line: 0, start_column: 0, end_line: 0, end_column: 0,
            new_text: "x".into(),
        };
        assert!(!not_noop.is_noop());
    }

    #[test]
    fn text_edit_new_text_line_count() {
        let edit = TextEdit {
            start_line: 0, start_column: 0, end_line: 0, end_column: 0,
            new_text: "a\nb\nc".into(),
        };
        assert_eq!(edit.new_text_line_count(), 3);
    }

    #[test]
    fn indentation_level_zero_tab_size() {
        assert_eq!(indentation_level("    hello", 0), 0);
    }

    #[test]
    fn format_stats_new_defaults() {
        let stats = FormatStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn format_stats_record_success() {
        let mut stats = FormatStats::new();
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
    fn format_stats_record_failure() {
        let mut stats = FormatStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn format_stats_reset() {
        let mut stats = FormatStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn format_stats_merge() {
        let mut a = FormatStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = FormatStats::new();
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
    fn format_stats_display() {
        let mut stats = FormatStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn format_stats_default() {
        let stats = FormatStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn format_validator_accepts_valid_name() {
        let v = FormatValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn format_validator_rejects_empty() {
        let v = FormatValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn format_validator_rejects_too_long() {
        let v = FormatValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn format_validator_forbidden_prefix() {
        let v = FormatValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn format_validator_allowed_chars() {
        let v = FormatValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn format_validator_range() {
        let v = FormatValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn format_sanitize_removes_control() {
        let result = FormatValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn format_truncate_short_string() {
        assert_eq!(FormatValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn format_truncate_long_string() {
        let result = FormatValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn format_is_ascii_printable() {
        assert!(FormatValidator::is_ascii_printable("Hello World 123"));
        assert!(!FormatValidator::is_ascii_printable("Hello\x00World"));
    }
}
