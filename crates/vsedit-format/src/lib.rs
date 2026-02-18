//! Document formatting support.

use std::collections::HashMap;
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

/// Service that manages formatting providers and dispatches requests.
pub struct FormattingService {
    document_providers: Vec<Box<dyn DocumentFormattingProvider>>,
    range_providers: Vec<Box<dyn DocumentRangeFormattingProvider>>,
    on_type_providers: Vec<Box<dyn OnTypeFormattingProvider>>,
    format_on_save: bool,
}

impl FormattingService {
    pub fn new() -> Self {
        Self {
            document_providers: Vec::new(),
            range_providers: Vec::new(),
            on_type_providers: Vec::new(),
            format_on_save: false,
        }
    }

    pub fn register_document_provider(&mut self, provider: Box<dyn DocumentFormattingProvider>) {
        self.document_providers.push(provider);
    }

    pub fn register_range_provider(&mut self, provider: Box<dyn DocumentRangeFormattingProvider>) {
        self.range_providers.push(provider);
    }

    pub fn register_on_type_provider(&mut self, provider: Box<dyn OnTypeFormattingProvider>) {
        self.on_type_providers.push(provider);
    }

    pub fn set_format_on_save(&mut self, enabled: bool) {
        self.format_on_save = enabled;
    }

    pub fn is_format_on_save(&self) -> bool {
        self.format_on_save
    }

    /// Format an entire document (Shift+Alt+F).
    pub fn format_document(&self, uri: &str, options: &FormattingOptions) -> Vec<TextEdit> {
        for provider in &self.document_providers {
            let edits = provider.format_document(uri, options);
            if !edits.is_empty() {
                return edits;
            }
        }
        Vec::new()
    }

    /// Format a selection/range.
    pub fn format_selection(
        &self,
        uri: &str,
        start_line: u32,
        end_line: u32,
        options: &FormattingOptions,
    ) -> Vec<TextEdit> {
        for provider in &self.range_providers {
            let edits = provider.format_range(uri, start_line, end_line, options);
            if !edits.is_empty() {
                return edits;
            }
        }
        Vec::new()
    }

    /// Format on type (triggered by a character).
    pub fn format_on_type(
        &self,
        uri: &str,
        line: u32,
        column: u32,
        ch: char,
        options: &FormattingOptions,
    ) -> Vec<TextEdit> {
        for provider in &self.on_type_providers {
            if provider.trigger_characters().contains(&ch) {
                let edits = provider.format_on_type(uri, line, column, ch, options);
                if !edits.is_empty() {
                    return edits;
                }
            }
        }
        Vec::new()
    }

    /// Format on save (if enabled).
    pub fn format_on_save(&self, uri: &str, options: &FormattingOptions) -> Vec<TextEdit> {
        if self.format_on_save {
            self.format_document(uri, options)
        } else {
            Vec::new()
        }
    }

    /// Whether any document formatting provider is registered.
    pub fn has_document_provider(&self) -> bool {
        !self.document_providers.is_empty()
    }

    /// Whether any range formatting provider is registered.
    pub fn has_range_provider(&self) -> bool {
        !self.range_providers.is_empty()
    }

    /// Collect all on-type trigger characters from registered providers.
    pub fn all_trigger_characters(&self) -> Vec<char> {
        let mut chars = Vec::new();
        for provider in &self.on_type_providers {
            for ch in provider.trigger_characters() {
                if !chars.contains(&ch) {
                    chars.push(ch);
                }
            }
        }
        chars
    }
}

impl Default for FormattingService {
    fn default() -> Self {
        Self::new()
    }
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

// ---------------------------------------------------------------------------
// TextEdit helpers
// ---------------------------------------------------------------------------

impl TextEdit {
    /// Number of lines this edit spans (inclusive).
    pub fn line_span(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

    /// Whether this edit affects only a single line.
    pub fn is_single_line(&self) -> bool {
        self.start_line == self.end_line
    }

    /// Whether this edit overlaps with another edit's range.
    pub fn overlaps(&self, other: &TextEdit) -> bool {
        if self.end_line < other.start_line || other.end_line < self.start_line {
            return false;
        }
        if self.end_line == other.start_line && self.end_column <= other.start_column {
            return false;
        }
        if other.end_line == self.start_line && other.end_column <= self.start_column {
            return false;
        }
        true
    }

    /// Character length of the replaced region on a single-line edit.
    /// Returns 0 for multi-line edits.
    pub fn replaced_length(&self) -> u32 {
        if self.is_single_line() {
            self.end_column.saturating_sub(self.start_column)
        } else {
            0
        }
    }
}

// ---------------------------------------------------------------------------
// FormattingOptions helpers
// ---------------------------------------------------------------------------

impl FormattingOptions {
    /// Human-readable summary of the formatting options.
    pub fn summary(&self) -> String {
        let indent = if self.insert_spaces {
            format!("{} spaces", self.tab_size)
        } else {
            "tabs".to_string()
        };
        format!(
            "indent={}, trim_trailing={}, final_newline={}, trim_final={}",
            indent,
            self.trim_trailing_whitespace,
            self.insert_final_newline,
            self.trim_final_newlines,
        )
    }

    /// Builder-style setter for tab_size.
    pub fn with_tab_size(mut self, size: u32) -> Self {
        self.tab_size = size;
        self
    }

    /// Builder-style setter for insert_spaces.
    pub fn with_insert_spaces(mut self, yes: bool) -> Self {
        self.insert_spaces = yes;
        self
    }
}

impl fmt::Display for FormattingOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

/// Sort edits by start position (line, then column).
pub fn sort_edits(edits: &mut [TextEdit]) {
    edits.sort_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then(a.start_column.cmp(&b.start_column))
    });
}

/// Merge adjacent or overlapping single-line edits on the same line into
/// combined edits. Non-single-line edits are passed through unchanged.
pub fn merge_adjacent_edits(edits: &[TextEdit]) -> Vec<TextEdit> {
    if edits.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<TextEdit> = edits.to_vec();
    sort_edits(&mut sorted);

    let mut merged: Vec<TextEdit> = vec![sorted[0].clone()];
    for edit in &sorted[1..] {
        let last = merged.last_mut().unwrap();
        if last.is_single_line()
            && edit.is_single_line()
            && last.end_line == edit.start_line
            && last.end_column >= edit.start_column
        {
            last.end_column = last.end_column.max(edit.end_column);
            last.new_text.push_str(&edit.new_text);
        } else {
            merged.push(edit.clone());
        }
    }
    merged
}

/// Returns true if any edits in the slice overlap each other.
pub fn has_overlapping_edits(edits: &[TextEdit]) -> bool {
    for i in 0..edits.len() {
        for j in (i + 1)..edits.len() {
            if edits[i].overlaps(&edits[j]) {
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// FormattingResult – outcome of a formatting pass
// ---------------------------------------------------------------------------

/// Captures the outcome of a formatting operation.
pub struct FormattingResult {
    pub edits: Vec<TextEdit>,
    pub elapsed_ms: u64,
    pub provider_name: String,
}

impl FormattingResult {
    pub fn new(edits: Vec<TextEdit>, elapsed_ms: u64, provider_name: impl Into<String>) -> Self {
        Self { edits, elapsed_ms, provider_name: provider_name.into() }
    }

    pub fn edit_count(&self) -> usize {
        self.edits.len()
    }

    pub fn is_noop(&self) -> bool {
        self.edits.is_empty()
    }
}

impl fmt::Display for FormattingResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} edit(s) in {}ms",
            self.provider_name,
            self.edits.len(),
            self.elapsed_ms,
        )
    }
}

// ---------------------------------------------------------------------------
// IndentationDetector – detect indent style from text
// ---------------------------------------------------------------------------

/// Analyzes text to detect whether it uses tabs or spaces and the indent size.
pub struct IndentationDetector;

/// The detected indentation style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndentStyle {
    Tabs,
    Spaces(u32),
    Mixed,
    Unknown,
}

impl IndentationDetector {
    /// Analyze the text and return the detected indentation style.
    pub fn detect(text: &str) -> IndentStyle {
        let mut tab_lines = 0u32;
        let mut space_lines = 0u32;
        let mut space_sizes: Vec<u32> = Vec::new();

        for line in text.lines() {
            if line.is_empty() {
                continue;
            }
            let leading: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            if leading.is_empty() {
                continue;
            }
            if leading.contains('\t') {
                tab_lines += 1;
            } else {
                space_lines += 1;
                let len = leading.len() as u32;
                if len > 0 {
                    space_sizes.push(len);
                }
            }
        }

        if tab_lines == 0 && space_lines == 0 {
            return IndentStyle::Unknown;
        }
        if tab_lines > 0 && space_lines > 0 {
            return IndentStyle::Mixed;
        }
        if tab_lines > 0 {
            return IndentStyle::Tabs;
        }

        let size = space_sizes.iter().copied().reduce(indent_gcd).unwrap_or(4);
        IndentStyle::Spaces(if size == 0 { 4 } else { size })
    }
}

fn indent_gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { indent_gcd(b, a % b) }
}

// ---------------------------------------------------------------------------
// WhitespaceNormalizer – fix line endings and trailing whitespace
// ---------------------------------------------------------------------------

/// Utilities for normalizing whitespace in text.
pub struct WhitespaceNormalizer;

impl WhitespaceNormalizer {
    /// Convert all line endings to `\n` (LF).
    pub fn normalize_line_endings(text: &str) -> String {
        text.replace("\r\n", "\n").replace('\r', "\n")
    }

    /// Remove trailing whitespace from each line.
    pub fn trim_trailing(text: &str) -> String {
        text.lines()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Ensure the text ends with exactly one newline.
    pub fn ensure_final_newline(text: &str) -> String {
        let trimmed = text.trim_end_matches('\n').trim_end_matches('\r');
        format!("{}\n", trimmed)
    }

    /// Apply all normalizations in order.
    pub fn normalize_all(text: &str) -> String {
        let text = Self::normalize_line_endings(text);
        let text = Self::trim_trailing(&text);
        Self::ensure_final_newline(&text)
    }
}

// ---------------------------------------------------------------------------
// FormattingConfig – builder for extended configuration
// ---------------------------------------------------------------------------

/// Extended formatting configuration with builder pattern.
pub struct FormattingConfig {
    pub options: FormattingOptions,
    pub format_on_save: bool,
    pub format_on_paste: bool,
    pub format_on_type: bool,
    pub default_provider: Option<String>,
}

impl FormattingConfig {
    pub fn builder() -> FormattingConfigBuilder {
        FormattingConfigBuilder::new()
    }
}

pub struct FormattingConfigBuilder {
    options: FormattingOptions,
    format_on_save: bool,
    format_on_paste: bool,
    format_on_type: bool,
    default_provider: Option<String>,
}

impl FormattingConfigBuilder {
    pub fn new() -> Self {
        Self {
            options: FormattingOptions {
                tab_size: 4,
                insert_spaces: true,
                trim_trailing_whitespace: true,
                insert_final_newline: true,
                trim_final_newlines: false,
            },
            format_on_save: false,
            format_on_paste: false,
            format_on_type: false,
            default_provider: None,
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

    pub fn format_on_save(mut self, yes: bool) -> Self {
        self.format_on_save = yes;
        self
    }

    pub fn format_on_paste(mut self, yes: bool) -> Self {
        self.format_on_paste = yes;
        self
    }

    pub fn format_on_type(mut self, yes: bool) -> Self {
        self.format_on_type = yes;
        self
    }

    pub fn default_provider(mut self, name: impl Into<String>) -> Self {
        self.default_provider = Some(name.into());
        self
    }

    pub fn build(self) -> FormattingConfig {
        FormattingConfig {
            options: self.options,
            format_on_save: self.format_on_save,
            format_on_paste: self.format_on_paste,
            format_on_type: self.format_on_type,
            default_provider: self.default_provider,
        }
    }
}

// ---------------------------------------------------------------------------
// TextEdit – construction helpers
// ---------------------------------------------------------------------------

impl TextEdit {
    /// Create an insertion edit at the given position.
    pub fn insert(line: u32, column: u32, text: impl Into<String>) -> Self {
        Self {
            start_line: line,
            start_column: column,
            end_line: line,
            end_column: column,
            new_text: text.into(),
        }
    }

    /// Create a deletion edit spanning the given range.
    pub fn delete(start_line: u32, start_column: u32, end_line: u32, end_column: u32) -> Self {
        Self {
            start_line,
            start_column,
            end_line,
            end_column,
            new_text: String::new(),
        }
    }

    /// Create a replacement edit spanning the given range.
    pub fn replace(
        start_line: u32,
        start_column: u32,
        end_line: u32,
        end_column: u32,
        text: impl Into<String>,
    ) -> Self {
        Self {
            start_line,
            start_column,
            end_line,
            end_column,
            new_text: text.into(),
        }
    }

    /// Return whether the edit range comes strictly before `other`.
    pub fn is_before(&self, other: &TextEdit) -> bool {
        self.end_line < other.start_line
            || (self.end_line == other.start_line && self.end_column <= other.start_column)
    }

    /// Return whether the edit range comes strictly after `other`.
    pub fn is_after(&self, other: &TextEdit) -> bool {
        other.is_before(self)
    }

    /// Shift this edit's position by the given line and column deltas.
    pub fn shifted(&self, line_delta: i64, col_delta: i64) -> Self {
        Self {
            start_line: (self.start_line as i64 + line_delta).max(0) as u32,
            start_column: (self.start_column as i64 + col_delta).max(0) as u32,
            end_line: (self.end_line as i64 + line_delta).max(0) as u32,
            end_column: (self.end_column as i64 + col_delta).max(0) as u32,
            new_text: self.new_text.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// FormattingOptions – indentation string helpers
// ---------------------------------------------------------------------------

impl FormattingOptions {
    /// Return the single-level indentation string according to current settings.
    pub fn indent_str(&self) -> String {
        if self.insert_spaces {
            " ".repeat(self.tab_size as usize)
        } else {
            "\t".to_string()
        }
    }

    /// Return an indentation string for the given nesting level.
    pub fn indent_at_level(&self, level: u32) -> String {
        self.indent_str().repeat(level as usize)
    }
}

// ---------------------------------------------------------------------------
// FormattingService – provider counts and clearing
// ---------------------------------------------------------------------------

impl FormattingService {
    /// Return the total number of registered providers across all categories.
    pub fn provider_count(&self) -> usize {
        self.document_providers.len()
            + self.range_providers.len()
            + self.on_type_providers.len()
    }

    /// Remove all registered providers.
    pub fn clear_providers(&mut self) {
        self.document_providers.clear();
        self.range_providers.clear();
        self.on_type_providers.clear();
    }
}

// ---------------------------------------------------------------------------
// FormatStats – percentage helpers
// ---------------------------------------------------------------------------

impl FormatStats {
    /// Return the number of successful operations.
    pub fn successes(&self) -> u64 {
        self.successful_operations
    }

    /// Return the number of failed operations.
    pub fn failures(&self) -> u64 {
        self.failed_operations
    }

    /// Return total elapsed time across all operations in milliseconds.
    pub fn total_time_ms(&self) -> u64 {
        self.total_time_ns / 1_000_000
    }

    /// Return the average operation time in milliseconds.
    pub fn average_time_ms(&self) -> u64 {
        self.average_time_ns() / 1_000_000
    }
}

// ---------------------------------------------------------------------------
// Diff-based edit generation
// ---------------------------------------------------------------------------

/// Generate the minimal set of line-level `TextEdit`s that transform
/// `original` into `formatted`.  Each changed line produces one replace edit;
/// inserted/deleted lines are handled similarly.
pub fn diff_edits(original: &str, formatted: &str) -> Vec<TextEdit> {
    let orig_lines: Vec<&str> = original.lines().collect();
    let fmt_lines: Vec<&str> = formatted.lines().collect();
    let mut edits = Vec::new();

    let max_len = orig_lines.len().max(fmt_lines.len());
    for i in 0..max_len {
        let orig_line = orig_lines.get(i).copied();
        let fmt_line = fmt_lines.get(i).copied();
        match (orig_line, fmt_line) {
            (Some(o), Some(f)) if o != f => {
                edits.push(TextEdit::replace(
                    i as u32,
                    0,
                    i as u32,
                    o.len() as u32,
                    f,
                ));
            }
            (Some(o), None) => {
                edits.push(TextEdit::delete(i as u32, 0, i as u32, o.len() as u32));
            }
            (None, Some(f)) => {
                edits.push(TextEdit::insert(i as u32, 0, f));
            }
            _ => {}
        }
    }
    edits
}

/// Remove consecutive blank lines, collapsing runs of 2+ empty lines into one.
pub fn collapse_blank_lines(text: &str) -> String {
    let mut result = Vec::new();
    let mut prev_blank = false;
    for line in text.lines() {
        let blank = line.trim().is_empty();
        if blank && prev_blank {
            continue;
        }
        result.push(line);
        prev_blank = blank;
    }
    result.join("\n")
}

/// Ensure there is exactly one blank line between top-level blocks.
/// A "block boundary" is defined as a non-empty line followed (or preceded)
/// by a line with zero indentation.
pub fn ensure_blank_line_between_blocks(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    let mut result = vec![lines[0].to_string()];
    for i in 1..lines.len() {
        let prev = lines[i - 1];
        let curr = lines[i];
        let prev_content = !prev.trim().is_empty();
        let curr_at_root = !curr.trim().is_empty()
            && curr.len() == curr.trim_start().len();
        if prev_content && curr_at_root {
            // Insert blank separator if missing
            if let Some(last) = result.last() {
                if !last.trim().is_empty() {
                    result.push(String::new());
                }
            }
        }
        result.push(curr.to_string());
    }
    result.join("\n")
}

// ---------------------------------------------------------------------------
// FormatEdit – a single formatting edit targeting a line range
// ---------------------------------------------------------------------------

/// Represents a single formatting edit applied to a contiguous range of lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatEdit {
    pub start_line: usize,
    pub end_line: usize,
    pub new_text: String,
}

impl std::fmt::Display for FormatEdit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Edit[{}..{}]: {}",
            self.start_line,
            self.end_line,
            if self.new_text.len() > 40 {
                format!("{}…", &self.new_text[..40])
            } else {
                self.new_text.clone()
            }
        )
    }
}

// ---------------------------------------------------------------------------
// FormatRangeOptimizer
// ---------------------------------------------------------------------------

/// Minimizes formatting edits by merging adjacent or overlapping ranges.
#[derive(Debug, Clone)]
pub struct FormatRangeOptimizer {
    edits: Vec<FormatEdit>,
}

impl FormatRangeOptimizer {
    pub fn new() -> Self {
        Self { edits: Vec::new() }
    }

    pub fn add_edit(&mut self, start_line: usize, end_line: usize, new_text: &str) {
        self.edits.push(FormatEdit {
            start_line,
            end_line,
            new_text: new_text.to_string(),
        });
    }

    /// Returns the number of raw (unoptimized) edits recorded so far.
    pub fn edit_count(&self) -> usize {
        self.edits.len()
    }

    /// Merges adjacent or overlapping edits into the smallest set of
    /// non-overlapping edits.  When two edits overlap, their texts are
    /// concatenated with a newline separator.
    pub fn optimize(&self) -> Vec<FormatEdit> {
        if self.edits.is_empty() {
            return Vec::new();
        }

        let mut sorted: Vec<FormatEdit> = self.edits.clone();
        sorted.sort_by_key(|e| (e.start_line, e.end_line));

        let mut merged: Vec<FormatEdit> = Vec::new();
        merged.push(sorted[0].clone());

        for edit in sorted.iter().skip(1) {
            let last = merged.last_mut().unwrap();
            // Adjacent means edit.start_line <= last.end_line + 1
            if edit.start_line <= last.end_line + 1 {
                if edit.end_line > last.end_line {
                    last.end_line = edit.end_line;
                }
                last.new_text.push('\n');
                last.new_text.push_str(&edit.new_text);
            } else {
                merged.push(edit.clone());
            }
        }
        merged
    }
}

impl Default for FormatRangeOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for FormatRangeOptimizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FormatRangeOptimizer({} edits)", self.edits.len())
    }
}

// ---------------------------------------------------------------------------
// FormatConflictResolver
// ---------------------------------------------------------------------------

/// Resolves conflicts between edits produced by multiple formatters.
///
/// When edits from different formatters overlap, the formatter with the
/// higher priority value wins.
#[derive(Debug, Clone)]
pub struct FormatConflictResolver {
    formatters: Vec<(String, u32)>,
    edits: Vec<(String, FormatEdit)>,
}

impl FormatConflictResolver {
    pub fn new() -> Self {
        Self {
            formatters: Vec::new(),
            edits: Vec::new(),
        }
    }

    pub fn add_formatter(&mut self, name: &str, priority: u32) {
        self.formatters.push((name.to_string(), priority));
    }

    pub fn add_edit(
        &mut self,
        formatter: &str,
        start_line: usize,
        end_line: usize,
        new_text: &str,
    ) {
        self.edits.push((
            formatter.to_string(),
            FormatEdit {
                start_line,
                end_line,
                new_text: new_text.to_string(),
            },
        ));
    }

    /// Returns `true` when two or more edits from *different* formatters
    /// cover overlapping line ranges.
    pub fn has_conflicts(&self) -> bool {
        self.conflict_count() > 0
    }

    /// Counts the number of edit pairs that conflict.
    pub fn conflict_count(&self) -> usize {
        let mut count = 0usize;
        for i in 0..self.edits.len() {
            for j in (i + 1)..self.edits.len() {
                let (ref name_a, ref edit_a) = self.edits[i];
                let (ref name_b, ref edit_b) = self.edits[j];
                if name_a != name_b && Self::overlaps(edit_a, edit_b) {
                    count += 1;
                }
            }
        }
        count
    }

    /// Resolves all edits: when edits overlap the higher-priority formatter
    /// wins and the lower-priority edit is discarded.
    pub fn resolve(&self) -> Vec<FormatEdit> {
        // Pair each edit with its formatter priority.
        let mut prioritized: Vec<(u32, &FormatEdit)> = self
            .edits
            .iter()
            .map(|(name, edit)| {
                let prio = self
                    .formatters
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, p)| *p)
                    .unwrap_or(0);
                (prio, edit)
            })
            .collect();

        // Sort by start_line, then by descending priority so the winner
        // appears first for each position.
        prioritized.sort_by(|a, b| {
            a.1.start_line
                .cmp(&b.1.start_line)
                .then(b.0.cmp(&a.0))
        });

        let mut result: Vec<FormatEdit> = Vec::new();
        for (_, edit) in &prioritized {
            let dominated = result.iter().any(|existing| {
                Self::overlaps(existing, edit)
            });
            if !dominated {
                result.push((*edit).clone());
            }
        }

        result.sort_by_key(|e| e.start_line);
        result
    }

    fn overlaps(a: &FormatEdit, b: &FormatEdit) -> bool {
        a.start_line <= b.end_line && b.start_line <= a.end_line
    }
}

impl Default for FormatConflictResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for FormatConflictResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FormatConflictResolver({} formatters, {} edits)",
            self.formatters.len(),
            self.edits.len()
        )
    }
}

// ---------------------------------------------------------------------------
// FormatOnPasteHandler
// ---------------------------------------------------------------------------

/// Handles formatting of pasted content: re-indentation, trailing-whitespace
/// removal, and line-ending normalisation.
#[derive(Debug, Clone)]
pub struct FormatOnPasteHandler {
    indent_size: usize,
}

impl FormatOnPasteHandler {
    pub fn new() -> Self {
        Self { indent_size: 4 }
    }

    pub fn set_indent_size(&mut self, size: usize) {
        self.indent_size = size;
    }

    /// Re-indents every line of `text` so that the *minimum* indentation in
    /// the input maps to `target_indent` levels (each level being
    /// `indent_size` spaces).  Relative indentation between lines is
    /// preserved.
    pub fn reindent(&self, text: &str, target_indent: usize) -> String {
        let lines: Vec<&str> = text.lines().collect();
        if lines.is_empty() {
            return String::new();
        }

        let min_indent = lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.len() - l.trim_start().len())
            .min()
            .unwrap_or(0);

        let target_spaces = target_indent * self.indent_size;

        lines
            .iter()
            .map(|line| {
                if line.trim().is_empty() {
                    String::new()
                } else {
                    let cur = line.len() - line.trim_start().len();
                    let relative = cur.saturating_sub(min_indent);
                    let new_indent = target_spaces + relative;
                    format!("{}{}", " ".repeat(new_indent), line.trim_start())
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Removes trailing whitespace from every line.
    pub fn trim_trailing(&self, text: &str) -> String {
        text.lines()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Normalises line endings to `\n`.
    pub fn normalize_line_endings(&self, text: &str) -> String {
        text.replace("\r\n", "\n").replace('\r', "\n")
    }
}

impl Default for FormatOnPasteHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for FormatOnPasteHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FormatOnPasteHandler(indent_size={})", self.indent_size)
    }
}

// ---------------------------------------------------------------------------
// FormatProgressIndicator
// ---------------------------------------------------------------------------

/// Tracks the progress of a multi-file formatting operation.
#[derive(Debug, Clone)]
pub struct FormatProgressIndicator {
    total_files: usize,
    current_file: Option<String>,
    completed: usize,
    skipped: usize,
}

impl FormatProgressIndicator {
    pub fn new(total_files: usize) -> Self {
        Self {
            total_files,
            current_file: None,
            completed: 0,
            skipped: 0,
        }
    }

    pub fn start_file(&mut self, path: &str) {
        self.current_file = Some(path.to_string());
    }

    pub fn complete_file(&mut self) {
        self.completed += 1;
        self.current_file = None;
    }

    pub fn skip_file(&mut self) {
        self.skipped += 1;
        self.current_file = None;
    }

    pub fn current_file(&self) -> Option<&str> {
        self.current_file.as_deref()
    }

    pub fn completed(&self) -> usize {
        self.completed
    }

    pub fn skipped(&self) -> usize {
        self.skipped
    }

    /// Returns the percentage of files processed (completed + skipped) out of
    /// the total.  Returns `100.0` when total is zero.
    pub fn percentage(&self) -> f64 {
        if self.total_files == 0 {
            return 100.0;
        }
        ((self.completed + self.skipped) as f64 / self.total_files as f64) * 100.0
    }

    pub fn is_complete(&self) -> bool {
        self.completed + self.skipped >= self.total_files
    }
}

impl std::fmt::Display for FormatProgressIndicator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Progress: {:.1}% ({}/{} done, {} skipped)",
            self.percentage(),
            self.completed,
            self.total_files,
            self.skipped,
        )
    }
}


// ─── FmtBld Builder & Validator ─────────────────────────────

/// Builder for constructing formatter configurations.
#[derive(Debug, Clone)]
pub struct FmtBldBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl FmtBldBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<FmtBldCfg, FmtBldBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(FmtBldBuildErr { errors }); }
        Ok(FmtBldCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated formatter configuration.
#[derive(Debug, Clone)]
pub struct FmtBldCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl FmtBldCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &FmtBldCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for FmtBldCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FmtBldCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct FmtBldBuildErr { pub errors: Vec<String> }

impl fmt::Display for FmtBldBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FmtBldBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for FmtBldBuildErr {}

// ─── FmtOut Formatter ───────────────────────────────────────

/// Formatting options for formatter output.
#[derive(Debug, Clone)]
pub struct FmtOutFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for FmtOutFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl FmtOutFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for formatter data.
pub struct FmtOutFmt {
    options: FmtOutFmtOpts,
}

impl FmtOutFmt {
    pub fn new(options: FmtOutFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: FmtOutFmtOpts::default() } }

    pub fn format_list(&self, items: &[&str]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut result = String::new();
        let mut line_len = 0usize;
        for (i, item) in items.iter().enumerate() {
            let formatted = if self.options.prefix_str.is_empty() {
                format!("{}{}", ind, item)
            } else {
                format!("{}{}{}", ind, self.options.prefix_str, item)
            };
            if i > 0 && line_len + formatted.len() > self.options.max_width {
                result.push('\n'); line_len = 0;
            } else if i > 0 {
                result.push_str(&self.options.separator);
                line_len += self.options.separator.len();
            }
            line_len += formatted.len();
            result.push_str(&formatted);
        }
        result
    }

    pub fn format_kv(&self, key: &str, value: &str) -> String {
        format!("{}{} = {}", " ".repeat(self.options.indent), key, value)
    }

    pub fn format_section(&self, heading: &str, lines: &[String]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut r = format!("[{}]\n", heading);
        for line in lines { r.push_str(&format!("{}{}\n", ind, line)); }
        r
    }

    pub fn truncate(&self, s: &str) -> String {
        if s.len() <= self.options.max_width { s.to_string() }
        else {
            let end = self.options.max_width.saturating_sub(3);
            format!("{}...", &s[..end])
        }
    }
}


/// Configuration manager for format functionality.
pub struct FormatConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl FormatConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &FormatConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for format operations.
pub struct FormatRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl FormatRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for format.
pub struct FormatValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl FormatValidationCollector {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &FormatValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Document formatting pipeline — extended utilities (qa)
// ---------------------------------------------------------------------------

/// Metric accumulator for format operations.
#[derive(Debug, Clone)]
pub struct QaMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QaMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for format.
#[derive(Debug, Clone)]
pub struct QaRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QaRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for format lookups.
#[derive(Debug, Clone)]
pub struct QaLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QaLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for format
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaFormatRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaFormatRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaFormatCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaFormatCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaFormatCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 84
// ---------------------------------------------------------------------------

/// Generic object pool `Xc84Pool<T>`.
pub struct Xc84Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc84Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc84PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc84Pool<T> {
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
    pub fn stats(&self) -> Xc84PoolStats {
        Xc84PoolStats {
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

impl<T> Default for Xc84Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc84Scheduler`.
pub struct Xc84Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc84Scheduler {
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

impl Default for Xc84Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_84 hash for the given byte slice.
pub fn xc_84_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_84 convention.
pub fn xc_84_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_115 deepening: state machine + event bus ---

/// States for the Xd115 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd115State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd115State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd115Transition {
    pub from: Xd115State,
    pub to: Xd115State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd115StateMachine {
    current: Xd115State,
    history: Vec<Xd115Transition>,
    step_counter: usize,
}

impl Xd115StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd115State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd115State {
        self.current
    }

    pub fn history(&self) -> &[Xd115Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd115State) -> Result<Xd115State, String> {
        let allowed = match (self.current, target) {
            (Xd115State::Idle, Xd115State::Running) => true,
            (Xd115State::Running, Xd115State::Paused) => true,
            (Xd115State::Running, Xd115State::Done) => true,
            (Xd115State::Paused, Xd115State::Running) => true,
            (Xd115State::Paused, Xd115State::Done) => true,
            (Xd115State::Done, Xd115State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_115: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd115Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd115SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd115State> {
        let prefix = "Xd115SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd115State::Idle),
            "Running" => Some(Xd115State::Running),
            "Paused" => Some(Xd115State::Paused),
            "Done" => Some(Xd115State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd115State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd115 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd115Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd115Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd115HandlerFn = Box<dyn Fn(&Xd115Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd115EventBus {
    handlers: Vec<(usize, Option<String>, Xd115HandlerFn)>,
    next_id: usize,
    published: Vec<Xd115Event>,
}

impl Xd115EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd115Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd115Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd115Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd115Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xg_41: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg41Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg41Graph {
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

impl Default for Xg41Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_41: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg41Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg41Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg41Heap<T>) {
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

impl<T: Ord> Default for Xg41Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 83).
pub struct Xh83SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh83SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 125 as u64,
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

/// A compact bit set supporting boolean operations (variant 83).
pub struct Xh83BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh83BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 83).
pub struct Xi83Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi83Deque<T> {
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
pub struct Xi83Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi83Interval {
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

/// A simple interval tree (variant 83).
pub struct Xi83IntervalTree {
    xi_intervals: Vec<Xi83Interval>,
}

impl Xi83IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi83Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi83Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi83Interval) -> Vec<&Xi83Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi83Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi83Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi83Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi83Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi83Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi83Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 83) ---

/// Disjoint set / union-find for crate 83.
pub struct Xj83UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj83UnionFind {
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

const XJ83_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 83.
pub struct Xj83BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj83BTreeNode<K, V>>>,
    len: usize,
}

struct Xj83BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj83BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj83BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ83_BTREE_ORDER - 1
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
        let mid = XJ83_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj83BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj83BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj83BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj83BTreeNode::xj_new_leaf();
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


// --- xk_83 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk83SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk83SegmentTree {
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
pub struct Xk83DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk83DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_83).
#[derive(Debug, Clone)]
pub struct Xl83Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl83Rope {
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

/// Suffix array for efficient string searching (xl_83).
#[derive(Debug, Clone)]
pub struct Xl83SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl83SuffixArray {
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
pub struct Xm83MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm83MatrixSparse {
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
pub struct Xm83Tokenizer {
    text: String,
}

impl Xm83Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 83.
pub struct Xn83Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn83Fenwick {
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

// ----- AVL tree map — crate 83 -----

#[derive(Debug, Clone)]
struct Xn83AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn83AvlNode<K, V>>>,
    right: Option<Box<Xn83AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 83.
#[derive(Debug, Clone)]
pub struct Xn83AVL<K, V> {
    root: Option<Box<Xn83AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn83AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn83AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn83AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn83AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn83AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn83AvlNode<K, V>>) -> Box<Xn83AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn83AvlNode<K, V>>) -> Box<Xn83AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn83AvlNode<K, V>>) -> Box<Xn83AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn83AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn83AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn83AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn83AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn83AvlNode<K, V>>) -> &Xn83AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn83AvlNode<K, V>>) -> (Box<Xn83AvlNode<K, V>>, Option<Box<Xn83AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn83AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn83AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn83AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn83AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn83AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn83AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn83AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo83RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo83Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo83RBNode<K, V> {
    key: K,
    value: V,
    color: Xo83Color,
    left: Option<Box<Xo83RBNode<K, V>>>,
    right: Option<Box<Xo83RBNode<K, V>>>,
}

/// A red-black tree map for crate 83.
#[derive(Debug, Clone)]
pub struct Xo83RedBlack<K, V> {
    root: Option<Box<Xo83RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo83RedBlack<K, V> {
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
            r.color = Xo83Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo83RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo83RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo83RBNode {
                    key, value, color: Xo83Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo83RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo83Color::Red)
    }

    fn xo_balance(mut h: Box<Xo83RBNode<K, V>>) -> Box<Xo83RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo83Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo83RBNode<K, V>>) -> Box<Xo83RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo83Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo83RBNode<K, V>>) -> Box<Xo83RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo83Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo83RBNode<K, V>>) {
        h.color = Xo83Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo83Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo83Color::Black; }
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
            r.color = Xo83Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo83RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo83RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo83RBNode<K, V>) -> (K, V, Option<Box<Xo83RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo83RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo83Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo83RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo83ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 83.
#[derive(Debug, Clone)]
pub struct Xo83ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo83ConsistentHash {
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
            let vkey = format!("{}#xo83#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo83#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 83).
#[derive(Debug)]
pub struct Xp83SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp83Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp83Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp83Node<K, V>>>,
    xp_right: Option<Box<Xp83Node<K, V>>>,
}

impl<K: Ord, V> Xp83Node<K, V> {
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

impl<K: Ord, V> Default for Xp83SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp83SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp83Node<K, V>>>, key: &K) -> Option<Box<Xp83Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp83Node<K, V>>) -> Box<Xp83Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp83Node<K, V>>) -> Box<Xp83Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp83Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp83Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp83Node::xp_new(key, val));
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


// --------------- Xq83Treap ---------------

use std::cmp::Ordering as Xq83Ord;

struct Xq83TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq83TreapNode<K, V>>>,
    right: Option<Box<Xq83TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq83Treap<K, V> {
    root: Option<Box<Xq83TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq83TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_83_size<K, V>(node: &Option<Box<Xq83TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_83_update_size<K, V>(node: &mut Xq83TreapNode<K, V>) {
    node.size = 1 + xq_83_size(&node.left) + xq_83_size(&node.right);
}

fn xq_83_rotate_right<K, V>(mut node: Box<Xq83TreapNode<K, V>>) -> Box<Xq83TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_83_update_size(&mut node);
    left.right = Some(node);
    xq_83_update_size(&mut left);
    left
}

fn xq_83_rotate_left<K, V>(mut node: Box<Xq83TreapNode<K, V>>) -> Box<Xq83TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_83_update_size(&mut node);
    right.left = Some(node);
    xq_83_update_size(&mut right);
    right
}

fn xq_83_insert_node<K: Ord, V>(
    node: Option<Box<Xq83TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq83TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq83TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq83Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq83Ord::Less => {
                let (new_left, old) = xq_83_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_83_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_83_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq83Ord::Greater => {
                let (new_right, old) = xq_83_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_83_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_83_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_83_remove_node<K: Ord, V>(
    node: Option<Box<Xq83TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq83TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq83Ord::Less => {
                let (new_left, old) = xq_83_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_83_update_size(&mut n);
                (Some(n), old)
            }
            Xq83Ord::Greater => {
                let (new_right, old) = xq_83_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_83_update_size(&mut n);
                (Some(n), old)
            }
            Xq83Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_83_rotate_right(n);
                    let (new_right, old) = xq_83_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_83_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_83_rotate_left(n);
                    let (new_left, old) = xq_83_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_83_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_83_find_min<K, V>(node: &Option<Box<Xq83TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_83_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_83_find_max<K, V>(node: &Option<Box<Xq83TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_83_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_83_rank<K: Ord, V>(node: &Option<Box<Xq83TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq83Ord::Less => xq_83_rank(&n.left, key),
            Xq83Ord::Equal => xq_83_size(&n.left),
            Xq83Ord::Greater => 1 + xq_83_size(&n.left) + xq_83_rank(&n.right, key),
        },
    }
}

fn xq_83_kth<K, V>(node: &Option<Box<Xq83TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_83_size(&n.left);
        if k < left_size {
            xq_83_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_83_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_83_in_order<K: Clone, V>(node: &Option<Box<Xq83TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_83_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_83_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq83Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 83 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_83_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq83Ord::Equal => return Some(&n.value),
                Xq83Ord::Less => cur = &n.left,
                Xq83Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_83_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_83_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_83_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_83_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_83_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_83_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_83_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq83VEBTree ---------------

pub struct Xq83VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq83VEBTree>>,
    clusters: Vec<Option<Box<Xq83VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq83VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq83VEBTree::xq_new(sqrt_hi))) };
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
                    self.clusters[hi] = Some(Box::new(Xq83VEBTree::xq_new(self.sqrt_lo)));
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
pub struct Xr83KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr83KDPoint {
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
pub struct Xr83BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr83KDNode {
    xr_point: Xr83KDPoint,
    xr_left: Option<Box<Xr83KDNode>>,
    xr_right: Option<Box<Xr83KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr83KDTree {
    xr_root: Option<Box<Xr83KDNode>>,
    xr_size: usize,
}

impl Xr83KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr83KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr83KDNode>>,
        point: Xr83KDPoint,
        depth: usize,
    ) -> Box<Xr83KDNode> {
        match node {
            None => Box::new(Xr83KDNode {
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
    pub fn xr_nearest_neighbor(&self, query: &Xr83KDPoint) -> Option<Xr83KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr83KDNode>,
        query: &Xr83KDPoint,
        depth: usize,
        best: &mut Xr83KDPoint,
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
    ) -> Vec<Xr83KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr83KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr83KDPoint>,
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
    pub fn xr_all_points(&self) -> Vec<Xr83KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr83KDNode>>, pts: &mut Vec<Xr83KDPoint>) {
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

    fn xr_depth_rec(node: &Option<Box<Xr83KDNode>>) -> usize {
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
    pub fn xr_bounding_box(&self) -> Option<Xr83BoundingBox> {
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
        Some(Xr83BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs83PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs83PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs83PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs83PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs83ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs83ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs83ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs83RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs83RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs83RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs83CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs83CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs83CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
    }
}

/// Auxiliary statistics tracker for xs_83 data structures.
#[derive(Debug, Clone)]
pub struct Xs83StatsTracker {
    xs_samples: Vec<f64>,
    xs_sorted: bool,
}

impl Xs83StatsTracker {
    /// Create a new stats tracker.
    pub fn xs_new() -> Self {
        Xs83StatsTracker {
            xs_samples: Vec::new(),
            xs_sorted: true,
        }
    }

    /// Add a sample value.
    pub fn xs_add(&mut self, value: f64) {
        self.xs_samples.push(value);
        self.xs_sorted = false;
    }

    /// Return the number of samples.
    pub fn xs_count(&self) -> usize {
        self.xs_samples.len()
    }

    /// Return the mean of all samples.
    pub fn xs_mean(&self) -> f64 {
        if self.xs_samples.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.xs_samples.iter().sum();
        sum / self.xs_samples.len() as f64
    }

    /// Return the minimum value.
    pub fn xs_min(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::min)
    }

    /// Return the maximum value.
    pub fn xs_max(&self) -> Option<f64> {
        self.xs_samples.iter().cloned().reduce(f64::max)
    }

    /// Return the variance of all samples.
    pub fn xs_variance(&self) -> f64 {
        if self.xs_samples.len() < 2 {
            return 0.0;
        }
        let mean = self.xs_mean();
        let sum_sq: f64 = self.xs_samples.iter()
            .map(|x| (x - mean) * (x - mean))
            .sum();
        sum_sq / (self.xs_samples.len() - 1) as f64
    }

    /// Return the standard deviation.
    pub fn xs_std_dev(&self) -> f64 {
        self.xs_variance().sqrt()
    }

    /// Return the median value.
    pub fn xs_median(&mut self) -> Option<f64> {
        if self.xs_samples.is_empty() {
            return None;
        }
        if !self.xs_sorted {
            self.xs_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            self.xs_sorted = true;
        }
        let mid = self.xs_samples.len() / 2;
        if self.xs_samples.len() % 2 == 0 {
            Some((self.xs_samples[mid - 1] + self.xs_samples[mid]) / 2.0)
        } else {
            Some(self.xs_samples[mid])
        }
    }

    /// Check if the tracker is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_samples.is_empty()
    }

    /// Clear all samples.
    pub fn xs_clear(&mut self) {
        self.xs_samples.clear();
        self.xs_sorted = true;
    }

    /// Return the range (max - min).
    pub fn xs_range(&self) -> f64 {
        match (self.xs_min(), self.xs_max()) {
            (Some(min), Some(max)) => max - min,
            _ => 0.0,
        }
    }

    /// Return the sum of all samples.
    pub fn xs_sum(&self) -> f64 {
        self.xs_samples.iter().sum()
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
    fn format_validator_accepts_and_rejects() {
        let mut v = FormatValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn format_validator_warnings() {
        let mut v = FormatValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn format_validator_clear_and_merge() {
        let mut v = FormatValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = FormatValidationCollector::new();
        a.add_error("a_err");
        let mut b = FormatValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
    }

    #[test]
    fn formatting_service_empty() {
        let svc = FormattingService::new();
        assert!(!svc.has_document_provider());
        assert!(!svc.has_range_provider());
        assert!(!svc.is_format_on_save());
        assert!(svc.all_trigger_characters().is_empty());
    }

    #[test]
    fn formatting_service_format_document() {
        struct TestDocProvider;
        impl DocumentFormattingProvider for TestDocProvider {
            fn format_document(&self, _uri: &str, _opts: &FormattingOptions) -> Vec<TextEdit> {
                vec![TextEdit {
                    start_line: 0, start_column: 0, end_line: 0, end_column: 3,
                    new_text: "bar".into(),
                }]
            }
        }
        let mut svc = FormattingService::new();
        svc.register_document_provider(Box::new(TestDocProvider));
        assert!(svc.has_document_provider());
        let edits = svc.format_document("f.rs", &FormattingOptions::default());
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "bar");
    }

    #[test]
    fn formatting_service_format_selection() {
        struct TestRangeProvider;
        impl DocumentRangeFormattingProvider for TestRangeProvider {
            fn format_range(&self, _: &str, start: u32, _end: u32, _opts: &FormattingOptions) -> Vec<TextEdit> {
                vec![TextEdit {
                    start_line: start, start_column: 0, end_line: start, end_column: 5,
                    new_text: "fixed".into(),
                }]
            }
        }
        let mut svc = FormattingService::new();
        svc.register_range_provider(Box::new(TestRangeProvider));
        assert!(svc.has_range_provider());
        let edits = svc.format_selection("f.rs", 2, 5, &FormattingOptions::default());
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].start_line, 2);
    }

    #[test]
    fn formatting_service_format_on_type() {
        struct TestOnTypeProvider;
        impl OnTypeFormattingProvider for TestOnTypeProvider {
            fn trigger_characters(&self) -> Vec<char> { vec![';'] }
            fn format_on_type(&self, _: &str, line: u32, _col: u32, _ch: char, _opts: &FormattingOptions) -> Vec<TextEdit> {
                vec![TextEdit {
                    start_line: line, start_column: 0, end_line: line, end_column: 0,
                    new_text: "    ".into(),
                }]
            }
        }
        let mut svc = FormattingService::new();
        svc.register_on_type_provider(Box::new(TestOnTypeProvider));
        assert_eq!(svc.all_trigger_characters(), vec![';']);

        let edits = svc.format_on_type("f.rs", 3, 10, ';', &FormattingOptions::default());
        assert_eq!(edits.len(), 1);

        // Wrong trigger char returns empty
        let empty = svc.format_on_type("f.rs", 3, 10, '}', &FormattingOptions::default());
        assert!(empty.is_empty());
    }

    #[test]
    fn formatting_service_format_on_save() {
        struct TestDocProvider;
        impl DocumentFormattingProvider for TestDocProvider {
            fn format_document(&self, _: &str, _: &FormattingOptions) -> Vec<TextEdit> {
                vec![TextEdit {
                    start_line: 0, start_column: 0, end_line: 0, end_column: 1,
                    new_text: "X".into(),
                }]
            }
        }
        let mut svc = FormattingService::new();
        svc.register_document_provider(Box::new(TestDocProvider));

        // Disabled by default
        let edits = svc.format_on_save("f.rs", &FormattingOptions::default());
        assert!(edits.is_empty());

        // Enable and try again
        svc.set_format_on_save(true);
        let edits = svc.format_on_save("f.rs", &FormattingOptions::default());
        assert_eq!(edits.len(), 1);
    }

    #[test]
    fn formatting_service_no_provider_returns_empty() {
        let svc = FormattingService::new();
        assert!(svc.format_document("f.rs", &FormattingOptions::default()).is_empty());
        assert!(svc.format_selection("f.rs", 0, 5, &FormattingOptions::default()).is_empty());
        assert!(svc.format_on_type("f.rs", 0, 0, ';', &FormattingOptions::default()).is_empty());
    }

    #[test]
    fn formatting_service_default() {
        let svc = FormattingService::default();
        assert!(!svc.is_format_on_save());
    }

    // -- TextEdit helpers ---------------------------------------------------

    #[test]
    fn text_edit_line_span_single() {
        let edit = TextEdit {
            start_line: 3, start_column: 0, end_line: 3, end_column: 5,
            new_text: "hi".into(),
        };
        assert_eq!(edit.line_span(), 1);
        assert!(edit.is_single_line());
    }

    #[test]
    fn text_edit_line_span_multi() {
        let edit = TextEdit {
            start_line: 1, start_column: 0, end_line: 4, end_column: 0,
            new_text: String::new(),
        };
        assert_eq!(edit.line_span(), 4);
        assert!(!edit.is_single_line());
    }

    #[test]
    fn text_edit_overlaps_true() {
        let a = TextEdit {
            start_line: 1, start_column: 0, end_line: 1, end_column: 10,
            new_text: String::new(),
        };
        let b = TextEdit {
            start_line: 1, start_column: 5, end_line: 1, end_column: 15,
            new_text: String::new(),
        };
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
    }

    #[test]
    fn text_edit_overlaps_false() {
        let a = TextEdit {
            start_line: 1, start_column: 0, end_line: 1, end_column: 5,
            new_text: String::new(),
        };
        let b = TextEdit {
            start_line: 1, start_column: 5, end_line: 1, end_column: 10,
            new_text: String::new(),
        };
        assert!(!a.overlaps(&b));
    }

    #[test]
    fn text_edit_replaced_length() {
        let edit = TextEdit {
            start_line: 0, start_column: 2, end_line: 0, end_column: 7,
            new_text: "x".into(),
        };
        assert_eq!(edit.replaced_length(), 5);
    }

    #[test]
    fn formatting_options_summary_and_display() {
        let opts = FormattingOptions::default();
        let s = opts.summary();
        assert!(s.contains("4 spaces"));
        assert!(s.contains("trim_trailing=true"));
        let display = format!("{}", opts);
        assert_eq!(display, s);
    }

    #[test]
    fn formatting_options_with_tab_size() {
        let opts = FormattingOptions::default().with_tab_size(2).with_insert_spaces(false);
        assert_eq!(opts.tab_size, 2);
        assert!(!opts.insert_spaces);
    }

    #[test]
    fn sort_edits_orders_by_position() {
        let mut edits = vec![
            TextEdit { start_line: 5, start_column: 0, end_line: 5, end_column: 1, new_text: String::new() },
            TextEdit { start_line: 1, start_column: 3, end_line: 1, end_column: 4, new_text: String::new() },
            TextEdit { start_line: 1, start_column: 0, end_line: 1, end_column: 1, new_text: String::new() },
        ];
        sort_edits(&mut edits);
        assert_eq!(edits[0].start_line, 1);
        assert_eq!(edits[0].start_column, 0);
        assert_eq!(edits[1].start_column, 3);
        assert_eq!(edits[2].start_line, 5);
    }

    #[test]
    fn merge_adjacent_edits_combines() {
        let edits = vec![
            TextEdit { start_line: 0, start_column: 0, end_line: 0, end_column: 3, new_text: "a".into() },
            TextEdit { start_line: 0, start_column: 3, end_line: 0, end_column: 6, new_text: "b".into() },
        ];
        let merged = merge_adjacent_edits(&edits);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].new_text, "ab");
        assert_eq!(merged[0].end_column, 6);
    }

    #[test]
    fn has_overlapping_edits_detects() {
        let edits = vec![
            TextEdit { start_line: 0, start_column: 0, end_line: 0, end_column: 5, new_text: String::new() },
            TextEdit { start_line: 0, start_column: 3, end_line: 0, end_column: 8, new_text: String::new() },
        ];
        assert!(has_overlapping_edits(&edits));

        let no_overlap = vec![
            TextEdit { start_line: 0, start_column: 0, end_line: 0, end_column: 3, new_text: String::new() },
            TextEdit { start_line: 1, start_column: 0, end_line: 1, end_column: 3, new_text: String::new() },
        ];
        assert!(!has_overlapping_edits(&no_overlap));
    }

    #[test]
    fn test_formatting_result_display() {
        let result = FormattingResult::new(
            vec![TextEdit { start_line: 0, start_column: 0, end_line: 0, end_column: 1, new_text: "x".into() }],
            42,
            "rustfmt",
        );
        assert_eq!(format!("{}", result), "rustfmt: 1 edit(s) in 42ms");
        assert!(!result.is_noop());
    }

    #[test]
    fn test_formatting_result_noop() {
        let result = FormattingResult::new(vec![], 0, "none");
        assert!(result.is_noop());
        assert_eq!(result.edit_count(), 0);
    }

    #[test]
    fn test_indentation_detector_spaces() {
        let text = "fn main() {\n    let x = 1;\n    let y = 2;\n}\n";
        assert_eq!(IndentationDetector::detect(text), IndentStyle::Spaces(4));
    }

    #[test]
    fn test_indentation_detector_tabs() {
        let text = "fn main() {\n\tlet x = 1;\n\tlet y = 2;\n}\n";
        assert_eq!(IndentationDetector::detect(text), IndentStyle::Tabs);
    }

    #[test]
    fn test_whitespace_normalizer() {
        assert_eq!(
            WhitespaceNormalizer::normalize_line_endings("a\r\nb\rc"),
            "a\nb\nc"
        );
        assert_eq!(
            WhitespaceNormalizer::trim_trailing("hello   \nworld  "),
            "hello\nworld"
        );
        assert_eq!(
            WhitespaceNormalizer::ensure_final_newline("hello\n\n"),
            "hello\n"
        );
    }

    #[test]
    fn test_formatting_config_builder() {
        let cfg = FormattingConfig::builder()
            .tab_size(2)
            .insert_spaces(true)
            .format_on_save(true)
            .format_on_paste(false)
            .default_provider("rustfmt")
            .build();
        assert_eq!(cfg.options.tab_size, 2);
        assert!(cfg.format_on_save);
        assert!(!cfg.format_on_paste);
        assert_eq!(cfg.default_provider.as_deref(), Some("rustfmt"));
    }

    // -- TextEdit construction helpers -------------------------------------

    #[test]
    fn text_edit_insert_constructor() {
        let edit = TextEdit::insert(3, 5, "hello");
        assert_eq!(edit.start_line, 3);
        assert_eq!(edit.start_column, 5);
        assert_eq!(edit.end_line, 3);
        assert_eq!(edit.end_column, 5);
        assert_eq!(edit.new_text, "hello");
        assert_eq!(edit.kind(), EditKind::Insert);
    }

    #[test]
    fn text_edit_delete_constructor() {
        let edit = TextEdit::delete(1, 0, 1, 10);
        assert_eq!(edit.new_text, "");
        assert_eq!(edit.kind(), EditKind::Delete);
        assert_eq!(edit.replaced_length(), 10);
    }

    #[test]
    fn text_edit_replace_constructor() {
        let edit = TextEdit::replace(0, 0, 0, 5, "world");
        assert_eq!(edit.kind(), EditKind::Replace);
        assert_eq!(edit.new_text, "world");
    }

    #[test]
    fn text_edit_is_before_and_after() {
        let a = TextEdit::insert(1, 5, "x");
        let b = TextEdit::insert(2, 0, "y");
        assert!(a.is_before(&b));
        assert!(!b.is_before(&a));
        assert!(b.is_after(&a));
        assert!(!a.is_after(&b));
    }

    #[test]
    fn text_edit_is_before_same_line() {
        let a = TextEdit::replace(1, 0, 1, 3, "aa");
        let b = TextEdit::replace(1, 3, 1, 6, "bb");
        assert!(a.is_before(&b));
        assert!(!b.is_before(&a));
    }

    #[test]
    fn text_edit_shifted() {
        let edit = TextEdit::replace(2, 4, 3, 0, "new");
        let shifted = edit.shifted(1, -2);
        assert_eq!(shifted.start_line, 3);
        assert_eq!(shifted.start_column, 2);
        assert_eq!(shifted.end_line, 4);
        assert_eq!(shifted.end_column, 0); // max(0-2, 0) = 0
        assert_eq!(shifted.new_text, "new");
    }

    #[test]
    fn text_edit_shifted_clamps_to_zero() {
        let edit = TextEdit::insert(0, 1, "x");
        let shifted = edit.shifted(-5, -5);
        assert_eq!(shifted.start_line, 0);
        assert_eq!(shifted.start_column, 0);
    }

    // -- FormattingOptions indent helpers -----------------------------------

    #[test]
    fn formatting_options_indent_str_spaces() {
        let opts = FormattingOptions::default();
        assert_eq!(opts.indent_str(), "    ");
    }

    #[test]
    fn formatting_options_indent_str_tabs() {
        let opts = FormattingOptions::default().with_insert_spaces(false);
        assert_eq!(opts.indent_str(), "\t");
    }

    #[test]
    fn formatting_options_indent_at_level() {
        let opts = FormattingOptions::default().with_tab_size(2);
        assert_eq!(opts.indent_at_level(0), "");
        assert_eq!(opts.indent_at_level(1), "  ");
        assert_eq!(opts.indent_at_level(3), "      ");
    }

    // -- FormattingService provider management ------------------------------

    #[test]
    fn formatting_service_provider_count_and_clear() {
        struct Dp;
        impl DocumentFormattingProvider for Dp {
            fn format_document(&self, _: &str, _: &FormattingOptions) -> Vec<TextEdit> {
                vec![]
            }
        }
        struct Rp;
        impl DocumentRangeFormattingProvider for Rp {
            fn format_range(&self, _: &str, _: u32, _: u32, _: &FormattingOptions) -> Vec<TextEdit> {
                vec![]
            }
        }

        let mut svc = FormattingService::new();
        assert_eq!(svc.provider_count(), 0);
        svc.register_document_provider(Box::new(Dp));
        svc.register_range_provider(Box::new(Rp));
        assert_eq!(svc.provider_count(), 2);
        svc.clear_providers();
        assert_eq!(svc.provider_count(), 0);
    }

    // -- FormatStats millisecond helpers ------------------------------------

    #[test]
    fn format_stats_millisecond_accessors() {
        let mut stats = FormatStats::new();
        stats.record_success(5_000_000); // 5ms
        stats.record_success(3_000_000); // 3ms
        assert_eq!(stats.total_time_ms(), 8);
        assert_eq!(stats.average_time_ms(), 4);
        assert_eq!(stats.successes(), 2);
        assert_eq!(stats.failures(), 0);
    }

    #[test]
    fn format_stats_failures_accessor() {
        let mut stats = FormatStats::new();
        stats.record_failure(1_000_000);
        stats.record_failure(2_000_000);
        assert_eq!(stats.failures(), 2);
        assert_eq!(stats.successes(), 0);
        assert_eq!(stats.total_time_ms(), 3);
    }

    // -- diff_edits --------------------------------------------------------

    #[test]
    fn diff_edits_identical_text() {
        let text = "line 1\nline 2\nline 3";
        assert!(diff_edits(text, text).is_empty());
    }

    #[test]
    fn diff_edits_single_line_change() {
        let edits = diff_edits("hello\nworld", "hello\nearth");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].start_line, 1);
        assert_eq!(edits[0].new_text, "earth");
    }

    #[test]
    fn diff_edits_line_added() {
        let edits = diff_edits("a", "a\nb");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].kind(), EditKind::Insert);
        assert_eq!(edits[0].new_text, "b");
    }

    #[test]
    fn diff_edits_line_removed() {
        let edits = diff_edits("a\nb", "a");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].kind(), EditKind::Delete);
    }

    // -- collapse_blank_lines ----------------------------------------------

    #[test]
    fn collapse_blank_lines_removes_runs() {
        let input = "a\n\n\n\nb\n\nc";
        let result = collapse_blank_lines(input);
        assert_eq!(result, "a\n\nb\n\nc");
    }

    #[test]
    fn collapse_blank_lines_preserves_single() {
        let input = "a\n\nb";
        assert_eq!(collapse_blank_lines(input), "a\n\nb");
    }

    #[test]
    fn collapse_blank_lines_empty_input() {
        assert_eq!(collapse_blank_lines(""), "");
    }

    // -- ensure_blank_line_between_blocks ----------------------------------

    #[test]
    fn ensure_blank_line_between_blocks_inserts() {
        let input = "fn a() {}\nfn b() {}";
        let result = ensure_blank_line_between_blocks(input);
        assert_eq!(result, "fn a() {}\n\nfn b() {}");
    }

    #[test]
    fn ensure_blank_line_between_blocks_already_present() {
        let input = "fn a() {}\n\nfn b() {}";
        let result = ensure_blank_line_between_blocks(input);
        assert_eq!(result, "fn a() {}\n\nfn b() {}");
    }

    // -- FormatRangeOptimizer ------------------------------------------------

    #[test]
    fn range_optimizer_empty() {
        let opt = FormatRangeOptimizer::new();
        assert_eq!(opt.edit_count(), 0);
        assert!(opt.optimize().is_empty());
    }

    #[test]
    fn range_optimizer_no_merge() {
        let mut opt = FormatRangeOptimizer::new();
        opt.add_edit(1, 3, "aaa");
        opt.add_edit(10, 12, "bbb");
        assert_eq!(opt.edit_count(), 2);
        let merged = opt.optimize();
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn range_optimizer_merges_adjacent() {
        let mut opt = FormatRangeOptimizer::new();
        opt.add_edit(1, 3, "aaa");
        opt.add_edit(4, 6, "bbb");
        let merged = opt.optimize();
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].start_line, 1);
        assert_eq!(merged[0].end_line, 6);
        assert!(merged[0].new_text.contains("aaa"));
        assert!(merged[0].new_text.contains("bbb"));
    }

    #[test]
    fn range_optimizer_merges_overlapping() {
        let mut opt = FormatRangeOptimizer::new();
        opt.add_edit(1, 5, "alpha");
        opt.add_edit(3, 7, "beta");
        let merged = opt.optimize();
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].end_line, 7);
    }

    #[test]
    fn range_optimizer_display() {
        let opt = FormatRangeOptimizer::new();
        let display = format!("{opt}");
        assert!(display.contains("0 edits"));
    }

    // -- FormatConflictResolver ----------------------------------------------

    #[test]
    fn conflict_resolver_no_conflicts() {
        let mut resolver = FormatConflictResolver::new();
        resolver.add_formatter("rustfmt", 10);
        resolver.add_formatter("prettier", 5);
        resolver.add_edit("rustfmt", 1, 3, "aaa");
        resolver.add_edit("prettier", 10, 12, "bbb");
        assert!(!resolver.has_conflicts());
        assert_eq!(resolver.conflict_count(), 0);
        assert_eq!(resolver.resolve().len(), 2);
    }

    #[test]
    fn conflict_resolver_higher_priority_wins() {
        let mut resolver = FormatConflictResolver::new();
        resolver.add_formatter("rustfmt", 10);
        resolver.add_formatter("prettier", 5);
        resolver.add_edit("rustfmt", 1, 5, "rust-output");
        resolver.add_edit("prettier", 3, 7, "prettier-output");
        assert!(resolver.has_conflicts());
        assert_eq!(resolver.conflict_count(), 1);
        let resolved = resolver.resolve();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].new_text, "rust-output");
    }

    #[test]
    fn conflict_resolver_display() {
        let mut resolver = FormatConflictResolver::new();
        resolver.add_formatter("a", 1);
        let s = format!("{resolver}");
        assert!(s.contains("1 formatters"));
    }

    // -- FormatOnPasteHandler ------------------------------------------------

    #[test]
    fn paste_handler_reindent() {
        let handler = FormatOnPasteHandler::new();
        let input = "  line1\n    line2\n  line3";
        let result = handler.reindent(input, 2);
        // target_indent=2 => 8 base spaces; line2 had 2 extra relative
        assert!(result.starts_with("        line1"));
        assert!(result.contains("          line2"));
    }

    #[test]
    fn paste_handler_trim_trailing() {
        let handler = FormatOnPasteHandler::new();
        let result = handler.trim_trailing("hello   \nworld  ");
        assert_eq!(result, "hello\nworld");
    }

    #[test]
    fn paste_handler_normalize_line_endings() {
        let handler = FormatOnPasteHandler::new();
        let result = handler.normalize_line_endings("a\r\nb\rc\n");
        assert_eq!(result, "a\nb\nc\n");
    }

    // -- FormatProgressIndicator ---------------------------------------------

    #[test]
    fn progress_indicator_lifecycle() {
        let mut progress = FormatProgressIndicator::new(3);
        assert_eq!(progress.percentage(), 0.0);
        assert!(!progress.is_complete());

        progress.start_file("a.rs");
        assert_eq!(progress.current_file(), Some("a.rs"));
        progress.complete_file();
        assert_eq!(progress.completed(), 1);
        assert!(progress.current_file().is_none());

        progress.start_file("b.rs");
        progress.skip_file();
        assert_eq!(progress.skipped(), 1);

        progress.start_file("c.rs");
        progress.complete_file();
        assert!(progress.is_complete());

        let pct = progress.percentage();
        assert!((pct - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn progress_indicator_display() {
        let progress = FormatProgressIndicator::new(10);
        let s = format!("{progress}");
        assert!(s.contains("0.0%"));
    }

    #[test]
    fn fmtbld_builder_valid() {
        let cfg = FmtBldBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn fmtbld_builder_empty_name() {
        let r = FmtBldBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn fmtbld_builder_bad_priority() {
        assert!(FmtBldBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn fmtbld_builder_zero_max() {
        assert!(FmtBldBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn fmtbld_cfg_merge() {
        let mut a = FmtBldBuilder::new("a").property("x", "1").build().unwrap();
        let b = FmtBldBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn fmtbld_cfg_display() {
        let cfg = FmtBldBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }

    #[test]
    fn fmtout_fmt_list() {
        let f = FmtOutFmt::new(FmtOutFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn fmtout_fmt_kv() {
        let f = FmtOutFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn fmtout_fmt_section() {
        let f = FmtOutFmt::new(FmtOutFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn fmtout_fmt_truncate() {
        let f = FmtOutFmt::new(FmtOutFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn fmtout_fmt_opts_defaults() {
        let o = FmtOutFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
    }


    #[test]
    fn format_config_new() {
        let cfg = FormatConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn format_config_set_get() {
        let mut cfg = FormatConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn format_config_remove() {
        let mut cfg = FormatConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn format_config_keys_sorted() {
        let mut cfg = FormatConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn format_config_bump_version() {
        let mut cfg = FormatConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn format_config_clear() {
        let mut cfg = FormatConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn format_config_merge() {
        let mut cfg1 = FormatConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = FormatConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn format_config_disable() {
        let mut cfg = FormatConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn format_rate_tracker_empty() {
        let rt = FormatRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn format_rate_tracker_record() {
        let mut rt = FormatRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn format_rate_tracker_prune() {
        let mut rt = FormatRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn format_validator_valid() {
        let v = FormatValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn format_validator_errors() {
        let mut v = FormatValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn format_validator_clear() {
        let mut v = FormatValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn format_validator_merge() {
        let mut v1 = FormatValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = FormatValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn format_rate_tracker_clear() {
        let mut rt = FormatRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn qa_metrics_empty() {
        let m = QaMetrics::new("format");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qa_metrics_record_and_mean() {
        let mut m = QaMetrics::new("format");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qa_metrics_min_max() {
        let mut m = QaMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qa_metrics_variance_and_std() {
        let mut m = QaMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn qa_metrics_percentile() {
        let mut m = QaMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qa_metrics_merge() {
        let mut a = QaMetrics::new("a");
        a.record(1.0);
        let mut b = QaMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qa_metrics_reset() {
        let mut m = QaMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qa_rate_window_empty() {
        let rw = QaRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qa_rate_window_tick_and_rate() {
        let mut rw = QaRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qa_lru_cache_basic() {
        let mut c = QaLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qa_lru_cache_contains_and_keys() {
        let mut c = QaLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qa_lru_cache_remove() {
        let mut c = QaLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qa_metrics_sum() {
        let mut m = QaMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qa_metrics_label() {
        let m = QaMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qa_lru_cache_clear() {
        let mut c = QaLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for format
    #[test]
    fn xa_format_ring_new() {
        let rb = super::XaFormatRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_format_ring_push_len() {
        let mut rb = super::XaFormatRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_format_ring_wrap() {
        let mut rb = super::XaFormatRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_format_ring_mean_empty() {
        let rb = super::XaFormatRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_format_ring_mean_values() {
        let mut rb = super::XaFormatRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_format_ring_min_max() {
        let mut rb = super::XaFormatRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_format_ring_iter() {
        let mut rb = super::XaFormatRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_format_counter_new() {
        let c = super::XaFormatCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_format_counter_inc() {
        let mut c = super::XaFormatCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_format_counter_inc_by() {
        let mut c = super::XaFormatCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_format_counter_reset() {
        let mut c = super::XaFormatCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_format_counter_clear() {
        let mut c = super::XaFormatCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_format_counter_default() {
        let c = super::XaFormatCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 84 ----

    #[test]
    fn xc_84_pool_new_empty() {
        let pool: super::Xc84Pool<i32> = super::Xc84Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_84_pool_release_acquire() {
        let mut pool = super::Xc84Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_84_pool_acquire_empty() {
        let mut pool: super::Xc84Pool<i32> = super::Xc84Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_84_pool_full() {
        let mut pool = super::Xc84Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_84_pool_drain() {
        let mut pool = super::Xc84Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_84_pool_stats() {
        let mut pool = super::Xc84Pool::new(8);
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
    fn xc_84_pool_clear() {
        let mut pool = super::Xc84Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_84_pool_shrink() {
        let mut pool = super::Xc84Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_84_pool_default() {
        let pool: super::Xc84Pool<String> = super::Xc84Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_84_pool_extend() {
        let mut pool = super::Xc84Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_84_pool_retain() {
        let mut pool = super::Xc84Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_84_scheduler_round_robin() {
        let mut sched = super::Xc84Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_84_scheduler_empty() {
        let mut sched = super::Xc84Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_84_scheduler_reset() {
        let mut sched = super::Xc84Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_84_scheduler_add_remove() {
        let mut sched = super::Xc84Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_84_scheduler_targets() {
        let sched = super::Xc84Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_84_hash_empty() {
        assert_eq!(super::xc_84_hash(b""), 5381);
    }

    #[test]
    fn xc_84_hash_data() {
        let h = super::xc_84_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_84_hash(b"hello"), h);
    }

    #[test]
    fn xc_84_reverse_str() {
        assert_eq!(super::xc_84_reverse("abc"), "cba");
        assert_eq!(super::xc_84_reverse(""), "");
    }


    // --- xd_115 deepening tests ---

    #[test]
    fn xd_115_sm_initial_state() {
        let sm = Xd115StateMachine::new();
        assert_eq!(sm.current_state(), Xd115State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_115_sm_valid_idle_to_running() {
        let mut sm = Xd115StateMachine::new();
        assert!(sm.transition(Xd115State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd115State::Running);
    }

    #[test]
    fn xd_115_sm_valid_running_to_paused() {
        let mut sm = Xd115StateMachine::new();
        sm.transition(Xd115State::Running).unwrap();
        assert!(sm.transition(Xd115State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd115State::Paused);
    }

    #[test]
    fn xd_115_sm_valid_running_to_done() {
        let mut sm = Xd115StateMachine::new();
        sm.transition(Xd115State::Running).unwrap();
        assert!(sm.transition(Xd115State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd115State::Done);
    }

    #[test]
    fn xd_115_sm_valid_paused_to_running() {
        let mut sm = Xd115StateMachine::new();
        sm.transition(Xd115State::Running).unwrap();
        sm.transition(Xd115State::Paused).unwrap();
        assert!(sm.transition(Xd115State::Running).is_ok());
    }

    #[test]
    fn xd_115_sm_valid_done_to_idle() {
        let mut sm = Xd115StateMachine::new();
        sm.transition(Xd115State::Running).unwrap();
        sm.transition(Xd115State::Done).unwrap();
        assert!(sm.transition(Xd115State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd115State::Idle);
    }

    #[test]
    fn xd_115_sm_invalid_idle_to_done() {
        let mut sm = Xd115StateMachine::new();
        assert!(sm.transition(Xd115State::Done).is_err());
    }

    #[test]
    fn xd_115_sm_invalid_idle_to_paused() {
        let mut sm = Xd115StateMachine::new();
        assert!(sm.transition(Xd115State::Paused).is_err());
    }

    #[test]
    fn xd_115_sm_history_tracking() {
        let mut sm = Xd115StateMachine::new();
        sm.transition(Xd115State::Running).unwrap();
        sm.transition(Xd115State::Paused).unwrap();
        sm.transition(Xd115State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd115State::Idle);
        assert_eq!(sm.history()[0].to, Xd115State::Running);
        assert_eq!(sm.history()[1].from, Xd115State::Running);
        assert_eq!(sm.history()[2].to, Xd115State::Done);
    }

    #[test]
    fn xd_115_sm_serialize_deserialize() {
        let mut sm = Xd115StateMachine::new();
        sm.transition(Xd115State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd115StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd115State::Running));
    }

    #[test]
    fn xd_115_sm_deserialize_invalid() {
        assert_eq!(Xd115StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_115_sm_reset() {
        let mut sm = Xd115StateMachine::new();
        sm.transition(Xd115State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd115State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_115_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd115EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd115Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_115_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd115EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd115Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd115Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_115_bus_unsubscribe() {
        let mut bus = Xd115EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_115_event_kind_and_payload() {
        let e = Xd115Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd115Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_115_bus_clear_history() {
        let mut bus = Xd115EventBus::new();
        bus.publish(Xd115Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_115_sm_step_counter_increments() {
        let mut sm = Xd115StateMachine::new();
        sm.transition(Xd115State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd115State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_41 graph tests ------------------------------------------------

    #[test]
    fn xg_41_graph_empty() {
        let g = super::Xg41Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_41_graph_add_node() {
        let mut g = super::Xg41Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_41_graph_add_edge() {
        let mut g = super::Xg41Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_41_graph_neighbors() {
        let mut g = super::Xg41Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_41_graph_has_path() {
        let mut g = super::Xg41Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_41_graph_self_path() {
        let g = super::Xg41Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_41_graph_topo_sort() {
        let mut g = super::Xg41Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_41_graph_cycle_detect_false() {
        let mut g = super::Xg41Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_41_graph_cycle_detect_true() {
        let mut g = super::Xg41Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_41 heap tests -------------------------------------------------

    #[test]
    fn xg_41_heap_empty() {
        let h: super::Xg41Heap<i32> = super::Xg41Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_41_heap_push_pop() {
        let mut h = super::Xg41Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_41_heap_peek() {
        let mut h = super::Xg41Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_41_heap_drain_sorted() {
        let mut h = super::Xg41Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_41_heap_merge() {
        let mut a = super::Xg41Heap::new();
        let mut b = super::Xg41Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_41_heap_default() {
        let h: super::Xg41Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_41_graph_default() {
        let g: super::Xg41Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh83_skip_insert_contains() {
        let mut sl = super::Xh83SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh83_skip_remove() {
        let mut sl = super::Xh83SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh83_skip_len() {
        let mut sl = super::Xh83SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh83_skip_range_query() {
        let mut sl = super::Xh83SkipList::xh_new(4);
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
    fn xh83_skip_floor_ceiling() {
        let mut sl = super::Xh83SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh83_skip_rank() {
        let mut sl = super::Xh83SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh83_skip_empty() {
        let sl = super::Xh83SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh83_skip_duplicates() {
        let mut sl = super::Xh83SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh83_bitset_set_test() {
        let mut bs = super::Xh83BitSet::xh_new(256);
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
    fn xh83_bitset_clear_count() {
        let mut bs = super::Xh83BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh83_bitset_and_or_xor() {
        let mut a = super::Xh83BitSet::xh_new(128);
        let mut b = super::Xh83BitSet::xh_new(128);
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
    fn xh83_bitset_iter_ones() {
        let mut bs = super::Xh83BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh83_bitset_first_last() {
        let mut bs = super::Xh83BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh83_bitset_empty() {
        let bs = super::Xh83BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi83_deque_push_pop_back() {
        let mut dq = super::Xi83Deque::xi_new(4);
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
    fn xi83_deque_push_pop_front() {
        let mut dq = super::Xi83Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi83_deque_mixed_ops() {
        let mut dq = super::Xi83Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi83_deque_get_and_split() {
        let mut dq = super::Xi83Deque::xi_new(8);
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
    fn xi83_deque_rotate_left() {
        let mut dq = super::Xi83Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi83_deque_rotate_right() {
        let mut dq = super::Xi83Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi83_deque_grow() {
        let mut dq = super::Xi83Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi83_deque_empty() {
        let dq = super::Xi83Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi83_interval_tree_insert_query() {
        let mut tree = super::Xi83IntervalTree::xi_new();
        tree.xi_insert(super::Xi83Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi83Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi83Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi83_interval_tree_overlap() {
        let mut tree = super::Xi83IntervalTree::xi_new();
        tree.xi_insert(super::Xi83Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi83Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi83Interval::xi_new(12, 20));
        let q = super::Xi83Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi83_interval_tree_remove() {
        let mut tree = super::Xi83IntervalTree::xi_new();
        tree.xi_insert(super::Xi83Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi83Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi83_interval_tree_gaps() {
        let mut tree = super::Xi83IntervalTree::xi_new();
        tree.xi_insert(super::Xi83Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi83Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi83Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi83Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi83Interval::xi_new(8, 10));
    }

    #[test]
    fn xi83_interval_tree_merge() {
        let mut tree = super::Xi83IntervalTree::xi_new();
        tree.xi_insert(super::Xi83Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi83Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi83Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi83Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi83Interval::xi_new(10, 15));
    }

    #[test]
    fn xi83_interval_tree_all() {
        let mut tree = super::Xi83IntervalTree::xi_new();
        tree.xi_insert(super::Xi83Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi83Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi83_interval_tree_empty() {
        let tree = super::Xi83IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi83_interval_tree_contains_point() {
        let iv = super::Xi83Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 83) ---

    #[test]
    fn xj_83_uf_make_and_find() {
        let mut uf = super::Xj83UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_83_uf_union_connected() {
        let mut uf = super::Xj83UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_83_uf_component_count() {
        let mut uf = super::Xj83UnionFind::xj_new();
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
    fn xj_83_uf_component_size() {
        let mut uf = super::Xj83UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_83_uf_largest_component() {
        let mut uf = super::Xj83UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_83_uf_many_elements() {
        let mut uf = super::Xj83UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_83_uf_separate_components() {
        let mut uf = super::Xj83UnionFind::xj_new();
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
    fn xj_83_uf_path_compression() {
        let mut uf = super::Xj83UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_83_bt_insert_get() {
        let mut bt = super::Xj83BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_83_bt_contains_len() {
        let mut bt = super::Xj83BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_83_bt_replace() {
        let mut bt = super::Xj83BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_83_bt_remove() {
        let mut bt = super::Xj83BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_83_bt_keys_values() {
        let mut bt = super::Xj83BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_83_bt_range() {
        let mut bt = super::Xj83BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_83_bt_min_max() {
        let mut bt = super::Xj83BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_83_bt_many_inserts() {
        let mut bt = super::Xj83BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_83 segment tree tests ---

    #[test]
    fn xk_83_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk83SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_83_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk83SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_83_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk83SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_83_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk83SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_83_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk83SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_83_st_single_element() {
        let data = vec![42];
        let st = super::Xk83SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_83_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk83SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_83_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk83SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_83 disjoint intervals tests ---

    #[test]
    fn xk_83_di_add_and_count() {
        let mut di = super::Xk83DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_83_di_merge_overlap() {
        let mut di = super::Xk83DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_83_di_contains() {
        let mut di = super::Xk83DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_83_di_remove() {
        let mut di = super::Xk83DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_83_di_covered_length() {
        let mut di = super::Xk83DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_83_di_gaps() {
        let mut di = super::Xk83DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_83_di_merge_adjacent() {
        let mut di = super::Xk83DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_83_di_empty() {
        let di = super::Xk83DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_83_rope_new_empty() {
        let rope = super::Xl83Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_83_rope_from_str() {
        let rope = super::Xl83Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_83_rope_insert_at() {
        let mut rope = super::Xl83Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_83_rope_delete_range() {
        let mut rope = super::Xl83Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_83_rope_char_at() {
        let rope = super::Xl83Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_83_rope_split_concat() {
        let rope = super::Xl83Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_83_rope_line_count() {
        let rope = super::Xl83Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_83_rope_line_at() {
        let rope = super::Xl83Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_83_sa_build_and_search() {
        let sa = super::Xl83SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_83_sa_count() {
        let sa = super::Xl83SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_83_sa_longest_repeated() {
        let sa = super::Xl83SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_83_sa_all_positions() {
        let sa = super::Xl83SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_83_sa_len() {
        let sa = super::Xl83SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_83_sa_empty() {
        let sa = super::Xl83SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_83_rope_slice() {
        let rope = super::Xl83Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_83_sa_search_start() {
        let sa = super::Xl83SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_83_sparse_set_get() {
        let mut m = super::Xm83MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_83_sparse_row_col() {
        let mut m = super::Xm83MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_83_sparse_transpose() {
        let mut m = super::Xm83MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_83_sparse_multiply_vec() {
        let mut m = super::Xm83MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_83_sparse_nnz_density() {
        let mut m = super::Xm83MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_83_sparse_clear() {
        let mut m = super::Xm83MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_83_sparse_overwrite_zero() {
        let mut m = super::Xm83MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_83_tokenizer_basic() {
        let t = super::Xm83Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_83_tokenizer_count() {
        let t = super::Xm83Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_83_tokenizer_unique() {
        let t = super::Xm83Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_83_tokenizer_frequency() {
        let t = super::Xm83Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_83_tokenizer_delimiter() {
        let t = super::Xm83Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_83_tokenizer_whitespace() {
        let t = super::Xm83Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_83_tokenizer_empty() {
        let t = super::Xm83Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 83 ----

    #[test]
    fn xn_83_fenwick_prefix_sum() {
        let mut ft = super::Xn83Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_83_fenwick_range_sum() {
        let mut ft = super::Xn83Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_83_fenwick_point_query() {
        let mut ft = super::Xn83Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_83_fenwick_len() {
        let ft = super::Xn83Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_83_fenwick_multiple_updates() {
        let mut ft = super::Xn83Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_83_fenwick_single_element() {
        let mut ft = super::Xn83Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_83_fenwick_find_kth() {
        let mut ft = super::Xn83Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_83_fenwick_negative_delta() {
        let mut ft = super::Xn83Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 83 ----

    #[test]
    fn xn_83_avl_insert_get() {
        let mut m = super::Xn83AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_83_avl_remove() {
        let mut m = super::Xn83AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_83_avl_in_order() {
        let mut m = super::Xn83AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_83_avl_min_max() {
        let mut m = super::Xn83AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_83_avl_floor_ceiling() {
        let mut m = super::Xn83AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_83_avl_height_balanced() {
        let mut m = super::Xn83AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_83_avl_overwrite() {
        let mut m = super::Xn83AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_83_avl_empty() {
        let m: super::Xn83AVL<i32, i32> = super::Xn83AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo83RedBlack tests ---

    #[test]
    fn xo_83_rb_insert_and_get() {
        let mut tree = super::Xo83RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_83_rb_len_and_empty() {
        let mut tree = super::Xo83RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_83_rb_min_max() {
        let mut tree = super::Xo83RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_83_rb_contains() {
        let mut tree = super::Xo83RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_83_rb_remove() {
        let mut tree = super::Xo83RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_83_rb_in_order() {
        let mut tree = super::Xo83RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_83_rb_black_height() {
        let mut tree = super::Xo83RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_83_rb_overwrite() {
        let mut tree = super::Xo83RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo83ConsistentHash tests ---

    #[test]
    fn xo_83_ch_add_and_count() {
        let mut ring = super::Xo83ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_83_ch_remove_node() {
        let mut ring = super::Xo83ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_83_ch_get_node() {
        let mut ring = super::Xo83ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_83_ch_empty_ring() {
        let ring = super::Xo83ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_83_ch_distribution() {
        let mut ring = super::Xo83ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_83_ch_rebalance() {
        let mut ring = super::Xo83ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_83_ch_virtual_nodes() {
        let mut ring = super::Xo83ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_83_ch_consistent_lookup() {
        let mut ring = super::Xo83ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_83_splay_insert_get() {
        let mut t = super::Xp83SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_83_splay_remove() {
        let mut t = super::Xp83SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_83_splay_count_increases() {
        let mut t = super::Xp83SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_83_splay_depth() {
        let mut t = super::Xp83SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_83_splay_len_empty() {
        let t = super::Xp83SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_83_splay_min_max() {
        let mut t = super::Xp83SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_83_splay_overwrite() {
        let mut t = super::Xp83SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_83_splay_remove_missing() {
        let mut t = super::Xp83SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_83 treap tests ----
    #[test]
    fn xq_83_treap_empty() {
        let t = super::Xq83Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_83_treap_insert_get() {
        let mut t = super::Xq83Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_83_treap_overwrite() {
        let mut t = super::Xq83Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_83_treap_remove() {
        let mut t = super::Xq83Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_83_treap_min_max() {
        let mut t = super::Xq83Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_83_treap_rank() {
        let mut t = super::Xq83Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_83_treap_kth() {
        let mut t = super::Xq83Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_83_treap_in_order() {
        let mut t = super::Xq83Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_83 VEB tree tests ----
    #[test]
    fn xq_83_veb_empty() {
        let v = super::Xq83VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_83_veb_insert_contains() {
        let mut v = super::Xq83VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_83_veb_min_max() {
        let mut v = super::Xq83VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_83_veb_delete() {
        let mut v = super::Xq83VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_83_veb_successor() {
        let mut v = super::Xq83VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_83_veb_predecessor() {
        let mut v = super::Xq83VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_83_veb_count() {
        let mut v = super::Xq83VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_83_veb_duplicate_insert() {
        let mut v = super::Xq83VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_83_kdtree_empty() {
        let tree = super::Xr83KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_83_kdtree_insert_one() {
        let mut tree = super::Xr83KDTree::xr_new();
        tree.xr_insert(super::Xr83KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_83_kdtree_insert_multiple() {
        let mut tree = super::Xr83KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr83KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_83_kdtree_nearest_neighbor() {
        let mut tree = super::Xr83KDTree::xr_new();
        tree.xr_insert(super::Xr83KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr83KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr83KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_83_kdtree_nn_empty() {
        let tree = super::Xr83KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr83KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_83_kdtree_range_search() {
        let mut tree = super::Xr83KDTree::xr_new();
        tree.xr_insert(super::Xr83KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr83KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr83KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_83_kdtree_range_empty() {
        let mut tree = super::Xr83KDTree::xr_new();
        tree.xr_insert(super::Xr83KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_83_kdtree_all_points() {
        let mut tree = super::Xr83KDTree::xr_new();
        tree.xr_insert(super::Xr83KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr83KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_83_kdtree_depth() {
        let mut tree = super::Xr83KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr83KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_83_kdtree_bounding_box() {
        let mut tree = super::Xr83KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr83KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr83KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_83_persistent_array_new() {
        let arr = super::Xs83PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_83_persistent_array_push() {
        let mut arr = super::Xs83PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_83_persistent_array_set() {
        let mut arr = super::Xs83PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_83_persistent_array_diff() {
        let mut arr = super::Xs83PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_83_persistent_array_rollback() {
        let mut arr = super::Xs83PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_83_persistent_array_history() {
        let mut arr = super::Xs83PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_83_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs83PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_83_persistent_array_from_vec() {
        let arr = super::Xs83PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_83_concurrent_queue_new() {
        let q = super::Xs83ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_83_concurrent_queue_push_pop() {
        let mut q = super::Xs83ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_83_concurrent_queue_full() {
        let mut q = super::Xs83ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_83_concurrent_queue_drain() {
        let mut q = super::Xs83ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_83_concurrent_queue_try_pop() {
        let mut q = super::Xs83ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_83_concurrent_queue_clear() {
        let mut q = super::Xs83ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_83_range_map_new() {
        let rm = super::Xs83RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_83_range_map_insert_get() {
        let mut rm = super::Xs83RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_83_range_map_overlap() {
        let mut rm = super::Xs83RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_83_range_map_remove() {
        let mut rm = super::Xs83RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_83_range_map_gaps() {
        let mut rm = super::Xs83RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_83_range_map_coverage() {
        let mut rm = super::Xs83RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_83_range_map_contains() {
        let mut rm = super::Xs83RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_83_range_map_clear() {
        let mut rm = super::Xs83RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_83_circular_buffer_new() {
        let buf = super::Xs83CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_83_circular_buffer_push_pop() {
        let mut buf = super::Xs83CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_83_circular_buffer_overwrite() {
        let mut buf = super::Xs83CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_83_circular_buffer_peek() {
        let mut buf = super::Xs83CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_83_circular_buffer_is_full() {
        let mut buf = super::Xs83CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_83_circular_buffer_iter() {
        let mut buf = super::Xs83CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_83_circular_buffer_clear() {
        let mut buf = super::Xs83CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_83_circular_buffer_to_vec() {
        let mut buf = super::Xs83CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }

    #[test]
    fn xs_83_stats_tracker_new() {
        let tracker = super::Xs83StatsTracker::xs_new();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_83_stats_tracker_mean() {
        let mut tracker = super::Xs83StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        tracker.xs_add(30.0);
        assert!((tracker.xs_mean() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn xs_83_stats_tracker_min_max() {
        let mut tracker = super::Xs83StatsTracker::xs_new();
        tracker.xs_add(5.0);
        tracker.xs_add(15.0);
        tracker.xs_add(10.0);
        assert_eq!(tracker.xs_min(), Some(5.0));
        assert_eq!(tracker.xs_max(), Some(15.0));
    }

    #[test]
    fn xs_83_stats_tracker_median() {
        let mut tracker = super::Xs83StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(3.0);
        tracker.xs_add(2.0);
        assert_eq!(tracker.xs_median(), Some(2.0));
    }

    #[test]
    fn xs_83_stats_tracker_variance() {
        let mut tracker = super::Xs83StatsTracker::xs_new();
        tracker.xs_add(2.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(4.0);
        tracker.xs_add(5.0);
        tracker.xs_add(5.0);
        tracker.xs_add(7.0);
        tracker.xs_add(9.0);
        let var = tracker.xs_variance();
        assert!(var > 0.0);
    }

    #[test]
    fn xs_83_stats_tracker_range() {
        let mut tracker = super::Xs83StatsTracker::xs_new();
        tracker.xs_add(3.0);
        tracker.xs_add(7.0);
        tracker.xs_add(1.0);
        assert!((tracker.xs_range() - 6.0).abs() < 1e-9);
    }

    #[test]
    fn xs_83_stats_tracker_clear() {
        let mut tracker = super::Xs83StatsTracker::xs_new();
        tracker.xs_add(1.0);
        tracker.xs_add(2.0);
        tracker.xs_clear();
        assert!(tracker.xs_is_empty());
        assert_eq!(tracker.xs_count(), 0);
    }

    #[test]
    fn xs_83_stats_tracker_sum() {
        let mut tracker = super::Xs83StatsTracker::xs_new();
        tracker.xs_add(10.0);
        tracker.xs_add(20.0);
        assert!((tracker.xs_sum() - 30.0).abs() < 1e-9);
    }

}