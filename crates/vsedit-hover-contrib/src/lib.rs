//! Hover tooltip contribution.

use std::collections::HashMap;
use std::fmt;
/// Markdown-formatted string for hover content.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkdownString {
    pub value: String,
    pub is_trusted: bool,
}

impl MarkdownString {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            is_trusted: false,
        }
    }

    pub fn trusted(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            is_trusted: true,
        }
    }

    /// Create a markdown string containing only a fenced code block.
    pub fn code(code: &str, language: &str) -> Self {
        Self {
            value: format!("```{language}\n{code}\n```"),
            is_trusted: false,
        }
    }

    /// Append raw text to the markdown value.
    pub fn append(&mut self, text: &str) {
        self.value.push_str(text);
    }

    /// Append a fenced code block to the markdown value.
    pub fn append_codeblock(&mut self, code: &str, language: &str) {
        self.value.push_str(&format!("\n```{language}\n{code}\n```"));
    }

    /// Returns `true` if the markdown value is empty.
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
}

impl std::fmt::Display for MarkdownString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

/// A range in a document where a hover applies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoverRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl HoverRange {
    /// Create a range spanning a single word on one line.
    pub fn from_word(line: u32, start_col: u32, end_col: u32) -> Self {
        Self {
            start_line: line,
            start_col,
            end_line: line,
            end_col,
        }
    }

    /// Returns `true` if the given position is inside this range (inclusive).
    pub fn contains_position(&self, line: u32, col: u32) -> bool {
        if line < self.start_line || line > self.end_line {
            return false;
        }
        if line == self.start_line && col < self.start_col {
            return false;
        }
        if line == self.end_line && col > self.end_col {
            return false;
        }
        true
    }

    /// Returns `true` if the range spans only a single line.
    pub fn is_single_line(&self) -> bool {
        self.start_line == self.end_line
    }
}

/// A hover tooltip with markdown contents and an optional range.
#[derive(Debug, Clone, PartialEq)]
pub struct Hover {
    pub contents: Vec<MarkdownString>,
    pub range: Option<HoverRange>,
}

impl Hover {
    pub fn new(contents: Vec<MarkdownString>) -> Self {
        Self {
            contents,
            range: None,
        }
    }

    pub fn with_range(mut self, start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        self.range = Some(HoverRange {
            start_line,
            start_col,
            end_line,
            end_col,
        });
        self
    }

    /// Builder method to add a content item to the hover.
    pub fn with_contents(mut self, content: MarkdownString) -> Self {
        self.contents.push(content);
        self
    }

    /// Returns `true` if the hover has no contents.
    pub fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }
}

/// Trait for types that can provide hover information.
pub trait HoverProvider {
    fn provide_hover(&self, uri: &str, line: u32, col: u32) -> Option<Hover>;
}

/// Merge multiple hovers into a single hover, concatenating contents.
/// Uses the range from the first hover that has one.
pub fn merge_hovers(hovers: Vec<Hover>) -> Hover {
    let mut contents = Vec::new();
    let mut range = None;
    for hover in hovers {
        if range.is_none() {
            range = hover.range;
        }
        contents.extend(hover.contents);
    }
    Hover { contents, range }
}

/// Filter hovers to only those whose range contains the given position.
/// Hovers with no range are always included.
pub fn filter_hovers(hovers: Vec<Hover>, line: u32, col: u32) -> Vec<Hover> {
    hovers
        .into_iter()
        .filter(|h| match &h.range {
            Some(r) => r.contains_position(line, col),
            None => true,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur when building or validating hover data.
#[derive(Debug, Clone, PartialEq)]
pub enum HoverError {
    /// The hover range is invalid (end precedes start).
    InvalidRange {
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    },
    /// The hover has no content.
    EmptyContent,
    /// A URI was expected but was empty or invalid.
    InvalidUri(String),
}

impl std::fmt::Display for HoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HoverError::InvalidRange { start_line, start_col, end_line, end_col } => {
                write!(
                    f,
                    "invalid hover range: ({start_line}:{start_col}) to ({end_line}:{end_col})"
                )
            }
            HoverError::EmptyContent => write!(f, "hover has no content"),
            HoverError::InvalidUri(uri) => write!(f, "invalid URI: {uri}"),
        }
    }
}

impl std::error::Error for HoverError {}

// ---------------------------------------------------------------------------
// Display for HoverRange
// ---------------------------------------------------------------------------

impl std::fmt::Display for HoverRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}-{}:{}",
            self.start_line, self.start_col, self.end_line, self.end_col
        )
    }
}

// ---------------------------------------------------------------------------
// HoverRange helpers
// ---------------------------------------------------------------------------

impl HoverRange {
    /// Validate that the range is well-formed (start ≤ end).
    pub fn validate(&self) -> Result<(), HoverError> {
        let valid = self.start_line < self.end_line
            || (self.start_line == self.end_line && self.start_col <= self.end_col);
        if valid {
            Ok(())
        } else {
            Err(HoverError::InvalidRange {
                start_line: self.start_line,
                start_col: self.start_col,
                end_line: self.end_line,
                end_col: self.end_col,
            })
        }
    }

    /// Number of lines this range spans (always ≥ 1).
    pub fn line_span(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

    /// Extend this range to also cover `other`, producing the smallest
    /// enclosing range.
    pub fn union(&self, other: &HoverRange) -> HoverRange {
        let (sl, sc) = if self.start_line < other.start_line
            || (self.start_line == other.start_line && self.start_col <= other.start_col)
        {
            (self.start_line, self.start_col)
        } else {
            (other.start_line, other.start_col)
        };
        let (el, ec) = if self.end_line > other.end_line
            || (self.end_line == other.end_line && self.end_col >= other.end_col)
        {
            (self.end_line, self.end_col)
        } else {
            (other.end_line, other.end_col)
        };
        HoverRange {
            start_line: sl,
            start_col: sc,
            end_line: el,
            end_col: ec,
        }
    }
}

// ---------------------------------------------------------------------------
// MarkdownString helpers
// ---------------------------------------------------------------------------

impl MarkdownString {
    /// Render a section heading followed by body text.
    pub fn section(heading: &str, body: &str) -> Self {
        Self {
            value: format!("### {heading}\n\n{body}"),
            is_trusted: false,
        }
    }

    /// Approximate word count of the underlying markdown source.
    pub fn word_count(&self) -> usize {
        self.value.split_whitespace().count()
    }

    /// Strip all markdown formatting and return plain text using `vsedit_markdown`.
    pub fn to_plain_text(&self) -> String {
        vsedit_markdown::strip_markdown(&self.value)
    }
}

// ---------------------------------------------------------------------------
// HoverBuilder – validated construction
// ---------------------------------------------------------------------------

/// Builder for constructing a [`Hover`] with validation.
#[derive(Debug, Clone)]
pub struct HoverBuilder {
    contents: Vec<MarkdownString>,
    range: Option<HoverRange>,
}

impl Default for HoverBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HoverBuilder {
    pub fn new() -> Self {
        Self {
            contents: Vec::new(),
            range: None,
        }
    }

    pub fn content(mut self, md: MarkdownString) -> Self {
        self.contents.push(md);
        self
    }

    pub fn code(mut self, code: &str, language: &str) -> Self {
        self.contents.push(MarkdownString::code(code, language));
        self
    }

    pub fn range(mut self, range: HoverRange) -> Self {
        self.range = Some(range);
        self
    }

    /// Build the hover, returning an error if the range is invalid or
    /// there is no content.
    pub fn build(self) -> Result<Hover, HoverError> {
        if self.contents.is_empty() {
            return Err(HoverError::EmptyContent);
        }
        if let Some(ref r) = self.range {
            r.validate()?;
        }
        Ok(Hover {
            contents: self.contents,
            range: self.range,
        })
    }
}

// ---------------------------------------------------------------------------
// HoverRegistry – manage multiple providers
// ---------------------------------------------------------------------------

/// Collects [`HoverProvider`] implementations and queries them all.
pub struct HoverRegistry {
    providers: Vec<Box<dyn HoverProvider>>,
}

impl HoverRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register(&mut self, provider: Box<dyn HoverProvider>) {
        self.providers.push(provider);
    }

    /// Query every registered provider and merge results.
    pub fn hover_at(&self, uri: &str, line: u32, col: u32) -> Option<Hover> {
        let hovers: Vec<Hover> = self
            .providers
            .iter()
            .filter_map(|p| p.provide_hover(uri, line, col))
            .collect();
        if hovers.is_empty() {
            None
        } else {
            Some(merge_hovers(hovers))
        }
    }

    /// Number of registered providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

impl Default for HoverRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate a URI string (non-empty and contains a colon scheme separator).
pub fn validate_uri(uri: &str) -> Result<(), HoverError> {
    if uri.is_empty() || !uri.contains(':') {
        Err(HoverError::InvalidUri(uri.to_string()))
    } else {
        Ok(())
    }
}

/// Accumulated statistics for hover-contrib operations.
#[derive(Debug, Clone, PartialEq)]
pub struct HoverContribStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl HoverContribStats {
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
    pub fn merge(&mut self, other: &HoverContribStats) {
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

impl Default for HoverContribStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for HoverContribStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HoverContribStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for hover-contrib.
#[derive(Debug, Clone)]
pub struct HoverContribValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl HoverContribValidator {
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

impl Default for HoverContribValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Hover priority
// ---------------------------------------------------------------------------

/// Priority level for hover providers, controlling display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HoverPriority {
    /// Lowest – shown last.
    Fallback = 0,
    /// Normal extension hover.
    Normal = 1,
    /// Language service hover (type info, docs).
    LanguageService = 2,
    /// Built-in hover (e.g. color preview, regex).
    Builtin = 3,
}

impl fmt::Display for HoverPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HoverPriority::Fallback => write!(f, "fallback"),
            HoverPriority::Normal => write!(f, "normal"),
            HoverPriority::LanguageService => write!(f, "language-service"),
            HoverPriority::Builtin => write!(f, "builtin"),
        }
    }
}

/// A hover result tagged with its provider's priority.
#[derive(Debug, Clone)]
pub struct PrioritizedHover {
    pub hover: Hover,
    pub priority: HoverPriority,
    pub provider_id: String,
}

impl PrioritizedHover {
    pub fn new(hover: Hover, priority: HoverPriority, provider_id: impl Into<String>) -> Self {
        Self {
            hover,
            priority,
            provider_id: provider_id.into(),
        }
    }
}

/// Sort and deduplicate hover results by priority (highest first).
pub fn sort_hovers_by_priority(mut hovers: Vec<PrioritizedHover>) -> Vec<PrioritizedHover> {
    hovers.sort_by(|a, b| b.priority.cmp(&a.priority));
    hovers
}

/// Merge prioritized hovers into a single Hover, ordered by priority.
pub fn merge_prioritized(hovers: Vec<PrioritizedHover>) -> Hover {
    let sorted = sort_hovers_by_priority(hovers);
    let mut contents = Vec::new();
    let mut range: Option<HoverRange> = None;
    for ph in sorted {
        contents.extend(ph.hover.contents);
        if range.is_none() {
            range = ph.hover.range;
        }
    }
    Hover { contents, range }
}

/// Filter prioritized hovers to keep only those at or above a minimum priority.
pub fn filter_by_min_priority(hovers: Vec<PrioritizedHover>, min: HoverPriority) -> Vec<PrioritizedHover> {
    hovers.into_iter().filter(|h| h.priority >= min).collect()
}

// ---------------------------------------------------------------------------
// MarkdownString extensions
// ---------------------------------------------------------------------------

impl MarkdownString {
    pub fn truncate(&self, max_len: usize) -> MarkdownString {
        if self.value.len() <= max_len {
            return self.clone();
        }
        let truncated: String = self.value.chars().take(max_len.saturating_sub(1)).collect();
        MarkdownString {
            value: format!("{truncated}…"),
            is_trusted: self.is_trusted,
        }
    }

    pub fn line_count(&self) -> usize {
        if self.value.is_empty() {
            return 0;
        }
        self.value.lines().count()
    }

    pub fn contains_code_block(&self) -> bool {
        self.value.contains("```")
    }
}

// ---------------------------------------------------------------------------
// Hover extensions
// ---------------------------------------------------------------------------

impl Hover {
    pub fn total_content_length(&self) -> usize {
        self.contents.iter().map(|c| c.value.len()).sum()
    }

    pub fn section_count(&self) -> usize {
        self.contents.len()
    }

    pub fn has_range(&self) -> bool {
        self.range.is_some()
    }
}

// ---------------------------------------------------------------------------
// HoverPriority extensions
// ---------------------------------------------------------------------------

impl HoverPriority {
    pub fn is_high(&self) -> bool {
        matches!(self, HoverPriority::Builtin | HoverPriority::LanguageService)
    }

    pub fn is_low(&self) -> bool {
        matches!(self, HoverPriority::Fallback)
    }

    pub fn numeric_value(&self) -> u8 {
        match self {
            HoverPriority::Fallback => 0,
            HoverPriority::Normal => 1,
            HoverPriority::LanguageService => 2,
            HoverPriority::Builtin => 3,
        }
    }
}

// ---------------------------------------------------------------------------
// PrioritizedHover extensions
// ---------------------------------------------------------------------------

impl PrioritizedHover {
    pub fn matches_position(&self, line: u32, col: u32) -> bool {
        match &self.hover.range {
            Some(r) => r.contains_position(line, col),
            None => true,
        }
    }
}

// ---------------------------------------------------------------------------
// HoverCollection – manage multiple hovers with merge/dedup
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HoverCollection {
    hovers: Vec<Hover>,
}

impl HoverCollection {
    pub fn new() -> Self {
        Self { hovers: Vec::new() }
    }

    pub fn push(&mut self, hover: Hover) {
        self.hovers.push(hover);
    }

    pub fn len(&self) -> usize {
        self.hovers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hovers.is_empty()
    }

    pub fn merge_all(self) -> Hover {
        merge_hovers(self.hovers)
    }

    pub fn filter_by_position(self, line: u32, col: u32) -> HoverCollection {
        HoverCollection {
            hovers: filter_hovers(self.hovers, line, col),
        }
    }

    pub fn dedup_by_content(&mut self) {
        let mut seen: Vec<Vec<String>> = Vec::new();
        self.hovers.retain(|h| {
            let key: Vec<String> = h.contents.iter().map(|c| c.value.clone()).collect();
            if seen.contains(&key) {
                false
            } else {
                seen.push(key);
                true
            }
        });
    }

    pub fn total_content_length(&self) -> usize {
        self.hovers.iter().map(|h| h.total_content_length()).sum()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Hover> {
        self.hovers.iter()
    }
}

impl Default for HoverCollection {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for HoverCollection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HoverCollection({} hovers)", self.hovers.len())
    }
}

// ---------------------------------------------------------------------------
// HoverContribStats summary
// ---------------------------------------------------------------------------

impl HoverContribStats {
    pub fn summary(&self) -> String {
        format!(
            "{} ops ({} ok, {} err), avg {}ns",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }

    pub fn has_failures(&self) -> bool {
        self.failed_operations > 0
    }
}

// ---------------------------------------------------------------------------
// HoverRegistry extensions
// ---------------------------------------------------------------------------

impl HoverRegistry {
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    pub fn clear(&mut self) {
        self.providers.clear();
    }
}

// ---------------------------------------------------------------------------
// Display for Hover
// ---------------------------------------------------------------------------

impl fmt::Display for Hover {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sections: Vec<&str> = self.contents.iter().map(|c| c.value.as_str()).collect();
        write!(f, "{}", sections.join("\n---\n"))
    }
}

// ---------------------------------------------------------------------------
// HoverContentFormatter – render hover cards with separators and structure
// ---------------------------------------------------------------------------

/// Formats multiple hover content sections into a single rendered markdown
/// string with configurable separators and optional headers.
#[derive(Debug, Clone)]
pub struct HoverContentFormatter {
    separator: String,
    show_provider_headers: bool,
    max_line_width: Option<usize>,
}

impl HoverContentFormatter {
    pub fn new() -> Self {
        Self {
            separator: "\n---\n".to_string(),
            show_provider_headers: false,
            max_line_width: None,
        }
    }

    /// Set the separator placed between content sections.
    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    /// When enabled, each section is prefixed with the provider id as a header.
    pub fn show_headers(mut self, show: bool) -> Self {
        self.show_provider_headers = show;
        self
    }

    /// Set maximum line width; lines longer than this are soft-wrapped.
    pub fn max_line_width(mut self, width: usize) -> Self {
        self.max_line_width = Some(width);
        self
    }

    /// Format a single `Hover` into a rendered markdown string.
    pub fn format_hover(&self, hover: &Hover) -> String {
        let sections: Vec<&str> = hover.contents.iter().map(|c| c.value.as_str()).collect();
        let joined = sections.join(&self.separator);
        self.apply_line_width(&joined)
    }

    /// Format a list of `PrioritizedHover` entries into a single string,
    /// sorted by priority (highest first).
    pub fn format_prioritized(&self, hovers: &[PrioritizedHover]) -> String {
        let mut sorted: Vec<&PrioritizedHover> = hovers.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));

        let sections: Vec<String> = sorted
            .iter()
            .flat_map(|ph| {
                ph.hover.contents.iter().map(move |c| {
                    if self.show_provider_headers {
                        format!("**{}**\n\n{}", ph.provider_id, c.value)
                    } else {
                        c.value.clone()
                    }
                })
            })
            .collect();

        let joined = sections.join(&self.separator);
        self.apply_line_width(&joined)
    }

    fn apply_line_width(&self, text: &str) -> String {
        let Some(width) = self.max_line_width else {
            return text.to_string();
        };
        text.lines()
            .map(|line| Self::soft_wrap(line, width))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn soft_wrap(line: &str, width: usize) -> String {
        if line.len() <= width {
            return line.to_string();
        }
        let mut result = String::with_capacity(line.len() + line.len() / width);
        let mut col = 0;
        for word in line.split_inclusive(' ') {
            if col > 0 && col + word.len() > width {
                result.push('\n');
                col = 0;
            }
            result.push_str(word);
            col += word.len();
        }
        result
    }
}

impl Default for HoverContentFormatter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// HoverRangeCalculator – compute ranges from text content
// ---------------------------------------------------------------------------

/// Utility for computing [`HoverRange`] values from source text and offsets.
pub struct HoverRangeCalculator;

impl HoverRangeCalculator {
    /// Convert a byte offset in `text` to a `(line, col)` pair (both 0-based).
    /// Returns `None` if the offset is out of bounds.
    pub fn offset_to_position(text: &str, offset: usize) -> Option<(u32, u32)> {
        if offset > text.len() {
            return None;
        }
        let mut line: u32 = 0;
        let mut col: u32 = 0;
        for (i, ch) in text.char_indices() {
            if i == offset {
                return Some((line, col));
            }
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        // offset == text.len() (end of string)
        if offset == text.len() {
            Some((line, col))
        } else {
            None
        }
    }

    /// Build a `HoverRange` from a byte-offset span in `text`.
    pub fn range_from_offsets(text: &str, start: usize, end: usize) -> Option<HoverRange> {
        let (sl, sc) = Self::offset_to_position(text, start)?;
        let (el, ec) = Self::offset_to_position(text, end)?;
        Some(HoverRange {
            start_line: sl,
            start_col: sc,
            end_line: el,
            end_col: ec,
        })
    }

    /// Find the word boundaries around `offset` (treating `[a-zA-Z0-9_]` as
    /// word characters) and return the corresponding `HoverRange`.
    pub fn word_range_at(text: &str, offset: usize) -> Option<HoverRange> {
        let bytes = text.as_bytes();
        if offset >= bytes.len() {
            return None;
        }
        let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        if !is_word(bytes[offset]) {
            return None;
        }
        let mut start = offset;
        while start > 0 && is_word(bytes[start - 1]) {
            start -= 1;
        }
        let mut end = offset;
        while end < bytes.len() && is_word(bytes[end]) {
            end += 1;
        }
        Self::range_from_offsets(text, start, end.saturating_sub(1))
    }
}

// ---------------------------------------------------------------------------
// HoverContentTruncator – limit hover size
// ---------------------------------------------------------------------------

/// Truncates hover content so it fits within configurable limits.
#[derive(Debug, Clone)]
pub struct HoverContentTruncator {
    /// Maximum number of content sections to keep.
    pub max_sections: usize,
    /// Maximum character length per section.
    pub max_section_chars: usize,
    /// Maximum total character length across all sections.
    pub max_total_chars: usize,
}

impl HoverContentTruncator {
    pub fn new() -> Self {
        Self {
            max_sections: 10,
            max_section_chars: 5000,
            max_total_chars: 20_000,
        }
    }

    pub fn max_sections(mut self, n: usize) -> Self {
        self.max_sections = n;
        self
    }

    pub fn max_section_chars(mut self, n: usize) -> Self {
        self.max_section_chars = n;
        self
    }

    pub fn max_total_chars(mut self, n: usize) -> Self {
        self.max_total_chars = n;
        self
    }

    /// Truncate a `Hover` in place, returning the modified hover.
    pub fn truncate(&self, mut hover: Hover) -> Hover {
        // Limit section count.
        if hover.contents.len() > self.max_sections {
            hover.contents.truncate(self.max_sections);
        }

        // Truncate individual sections.
        for section in &mut hover.contents {
            if section.value.chars().count() > self.max_section_chars {
                *section = section.truncate(self.max_section_chars);
            }
        }

        // Enforce total limit by dropping trailing sections.
        let mut total = 0usize;
        let mut keep = hover.contents.len();
        for (i, section) in hover.contents.iter().enumerate() {
            total += section.value.chars().count();
            if total > self.max_total_chars {
                keep = i;
                break;
            }
        }
        hover.contents.truncate(keep);

        hover
    }
}

impl Default for HoverContentTruncator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// HoverProviderChain – chain providers with fallback semantics
// ---------------------------------------------------------------------------

/// Chains multiple hover providers, querying them in priority order and
/// optionally short-circuiting when the first result is found.
pub struct HoverProviderChain {
    providers: Vec<(Box<dyn HoverProvider>, HoverPriority)>,
    short_circuit: bool,
}

impl HoverProviderChain {
    /// Create a new chain. When `short_circuit` is `true`, iteration stops
    /// after the first provider that returns a hover.
    pub fn new(short_circuit: bool) -> Self {
        Self {
            providers: Vec::new(),
            short_circuit,
        }
    }

    /// Add a provider with the given priority.
    pub fn add(&mut self, provider: Box<dyn HoverProvider>, priority: HoverPriority) {
        self.providers.push((provider, priority));
        // Keep sorted highest priority first.
        self.providers.sort_by(|a, b| b.1.cmp(&a.1));
    }

    /// Query all providers (respecting short-circuit) and return collected
    /// prioritized hovers.
    pub fn query(&self, uri: &str, line: u32, col: u32) -> Vec<PrioritizedHover> {
        let mut results = Vec::new();
        for (i, (provider, priority)) in self.providers.iter().enumerate() {
            if let Some(hover) = provider.provide_hover(uri, line, col) {
                results.push(PrioritizedHover::new(
                    hover,
                    *priority,
                    format!("chain-{i}"),
                ));
                if self.short_circuit {
                    break;
                }
            }
        }
        results
    }

    /// Query and merge into a single hover, or `None` if nothing matched.
    pub fn hover_at(&self, uri: &str, line: u32, col: u32) -> Option<Hover> {
        let results = self.query(uri, line, col);
        if results.is_empty() {
            None
        } else {
            Some(merge_prioritized(results))
        }
    }

    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

// ---------------------------------------------------------------------------
// HoverMultiSource – merging hover info from multiple providers
// ---------------------------------------------------------------------------

/// A collected hover result from multiple sources, with source attribution.
#[derive(Debug, Clone)]
pub struct HoverMultiSourceResult {
    pub source_id: String,
    pub hover: Hover,
    pub priority: u32,
}

/// Collects hover results from multiple providers and merges them.
pub struct HoverMultiSource {
    results: Vec<HoverMultiSourceResult>,
}

impl HoverMultiSource {
    pub fn new() -> Self {
        Self { results: Vec::new() }
    }

    /// Add a hover result from a named source.
    pub fn add(&mut self, source_id: impl Into<String>, hover: Hover, priority: u32) {
        self.results.push(HoverMultiSourceResult {
            source_id: source_id.into(),
            hover,
            priority,
        });
    }

    /// Merge all results into a single hover, ordered by priority (highest first).
    pub fn merge(&self) -> Option<Hover> {
        if self.results.is_empty() {
            return None;
        }
        let mut sorted: Vec<&HoverMultiSourceResult> = self.results.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        let mut contents = Vec::new();
        let mut range = None;
        for r in &sorted {
            contents.extend(r.hover.contents.iter().cloned());
            if range.is_none() {
                range = r.hover.range;
            }
        }
        Some(Hover { contents, range })
    }

    /// Number of sources contributing.
    pub fn source_count(&self) -> usize {
        self.results.len()
    }

    /// Check if any results were collected.
    pub fn has_results(&self) -> bool {
        !self.results.is_empty()
    }
}

impl Default for HoverMultiSource {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// HoverStickyDelay – persistent hover timing
// ---------------------------------------------------------------------------

/// Configuration for how long a hover tooltip stays visible.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoverStickyDelay {
    pub show_delay_ms: u32,
    pub hide_delay_ms: u32,
    pub sticky: bool,
}

impl HoverStickyDelay {
    pub fn default_config() -> Self {
        Self {
            show_delay_ms: 300,
            hide_delay_ms: 200,
            sticky: false,
        }
    }

    pub fn sticky_config() -> Self {
        Self {
            show_delay_ms: 300,
            hide_delay_ms: 500,
            sticky: true,
        }
    }

    /// Whether the hover should remain visible (sticky mode or mouse over hover).
    pub fn should_persist(&self, mouse_over_hover: bool) -> bool {
        self.sticky || mouse_over_hover
    }
}

// ---------------------------------------------------------------------------
// HoverAction – handler for buttons in hover
// ---------------------------------------------------------------------------

/// An action button embedded in a hover tooltip.
#[derive(Debug, Clone, PartialEq)]
pub struct HoverAction {
    pub label: String,
    pub command: String,
    pub tooltip: Option<String>,
}

impl HoverAction {
    pub fn new(label: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            command: command.into(),
            tooltip: None,
        }
    }

    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }
}

impl std::fmt::Display for HoverAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}](command:{})", self.label, self.command)
    }
}

/// A hover widget with optional action buttons.
#[derive(Debug, Clone)]
pub struct HoverWithActions {
    pub hover: Hover,
    pub actions: Vec<HoverAction>,
}

impl HoverWithActions {
    pub fn new(hover: Hover) -> Self {
        Self { hover, actions: Vec::new() }
    }

    pub fn add_action(&mut self, action: HoverAction) {
        self.actions.push(action);
    }

    pub fn has_actions(&self) -> bool {
        !self.actions.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Hover positioning logic near cursor
// ---------------------------------------------------------------------------

/// Preferred position of the hover tooltip relative to the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverPosition {
    Above,
    Below,
}

/// Calculate hover positioning given available space.
pub struct HoverPositionCalculator;

impl HoverPositionCalculator {
    /// Determine if the hover should appear above or below the cursor.
    pub fn position(
        cursor_line: u32,
        total_lines: u32,
        hover_height_lines: u32,
    ) -> HoverPosition {
        let space_above = cursor_line;
        let space_below = total_lines.saturating_sub(cursor_line + 1);
        if space_below >= hover_height_lines || space_below >= space_above {
            HoverPosition::Below
        } else {
            HoverPosition::Above
        }
    }

    /// Calculate the y-offset for the hover in character lines.
    pub fn y_offset(
        cursor_line: u32,
        hover_height: u32,
        position: HoverPosition,
    ) -> u32 {
        match position {
            HoverPosition::Below => cursor_line + 1,
            HoverPosition::Above => cursor_line.saturating_sub(hover_height),
        }
    }
}


// ---------------------------------------------------------------------------
// HoverDiagnosticsDisplay
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HoverDiagnosticsDisplay {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl HoverDiagnosticsDisplay {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for HoverDiagnosticsDisplay {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for HoverDiagnosticsDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "HoverDiagnosticsDisplay({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// HoverDefinitionPreview
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HoverDefinitionPreview {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl HoverDefinitionPreview {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for HoverDefinitionPreview {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for HoverDefinitionPreview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "HoverDefinitionPreview({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// HoverDiagnosticsDisplaySnapshot — point-in-time snapshot of HoverDiagnosticsDisplay state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HoverDiagnosticsDisplaySnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl HoverDiagnosticsDisplaySnapshot {
    pub fn capture(source: &HoverDiagnosticsDisplay, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for HoverDiagnosticsDisplaySnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// HoverDefinitionPreviewStats — aggregate statistics for HoverDefinitionPreview
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct HoverDefinitionPreviewStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl HoverDefinitionPreviewStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for HoverDefinitionPreviewStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// HoverDiagnosticsDisplayConfig — configuration for HoverDiagnosticsDisplay
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HoverDiagnosticsDisplayConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl HoverDiagnosticsDisplayConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for HoverDiagnosticsDisplayConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for HoverDiagnosticsDisplayConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// HoverContentBuilder
// ---------------------------------------------------------------------------

/// Builds multi-part hover content.
pub struct HoverContentBuilder {
    parts: Vec<String>,
}

impl HoverContentBuilder {
    pub fn new() -> Self {
        Self { parts: Vec::new() }
    }

    pub fn add_code_block(&mut self, code: &str, language: &str) -> &mut Self {
        self.parts.push(format!("```{}\n{}\n```", language, code));
        self
    }

    pub fn add_markdown(&mut self, md: &str) -> &mut Self {
        self.parts.push(md.to_string());
        self
    }

    pub fn add_separator(&mut self) -> &mut Self {
        self.parts.push("---".to_string());
        self
    }

    pub fn add_signature(&mut self, sig: &str) -> &mut Self {
        self.parts.push(format!("`{}`", sig));
        self
    }

    pub fn build(&self) -> String {
        self.parts.join("\n\n")
    }

    pub fn part_count(&self) -> usize {
        self.parts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }
}

// ---------------------------------------------------------------------------
// HoverPositionCalculator
// ---------------------------------------------------------------------------

/// Computes tooltip position relative to cursor, avoiding overflow.
pub struct HoverTooltipPositioner {
    pub viewport_width: u32,
    pub viewport_height: u32,
}

impl HoverTooltipPositioner {
    pub fn new(viewport_width: u32, viewport_height: u32) -> Self {
        Self { viewport_width, viewport_height }
    }

    /// Returns (x, y) position for the tooltip. Prefers placing above the cursor.
    pub fn compute_position(
        &self,
        cursor_x: u32,
        cursor_y: u32,
        tooltip_width: u32,
        tooltip_height: u32,
    ) -> (u32, u32) {
        let x = if cursor_x + tooltip_width > self.viewport_width {
            self.viewport_width.saturating_sub(tooltip_width)
        } else {
            cursor_x
        };
        let y = if cursor_y >= tooltip_height + 2 {
            cursor_y - tooltip_height - 2
        } else {
            cursor_y + 2
        };
        (x, y)
    }

    pub fn prefer_above(&self, cursor_y: u32, tooltip_height: u32) -> bool {
        cursor_y >= tooltip_height + 2
    }
}

// ---------------------------------------------------------------------------
// HoverDelayManager
// ---------------------------------------------------------------------------

/// Manages delay before showing hover tooltip.
pub struct HoverDelayManager {
    delay_ms: u64,
    started_at: Option<std::time::Instant>,
}

impl HoverDelayManager {
    pub fn new(delay_ms: u64) -> Self {
        Self { delay_ms, started_at: None }
    }

    pub fn start(&mut self) {
        self.started_at = Some(std::time::Instant::now());
    }

    pub fn cancel(&mut self) {
        self.started_at = None;
    }

    pub fn reset(&mut self) {
        self.started_at = None;
    }

    pub fn is_ready(&self) -> bool {
        match self.started_at {
            Some(t) => t.elapsed().as_millis() as u64 >= self.delay_ms,
            None => false,
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        match self.started_at {
            Some(t) => t.elapsed().as_millis() as u64,
            None => 0,
        }
    }

    pub fn delay(&self) -> u64 {
        self.delay_ms
    }
}


/// Hover contribution configuration manager.
#[derive(Debug, Clone)]
pub struct HoverContribConfig {
    entries: Vec<HoverContribEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single hover contribution entry.
#[derive(Debug, Clone, PartialEq)]
pub struct HoverContribEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl HoverContribEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl HoverContribConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: HoverContribEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&HoverContribEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut HoverContribEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&HoverContribEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&HoverContribEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&HoverContribEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
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

    pub fn drain_inactive(&mut self) -> Vec<HoverContribEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_new_and_with_range() {
        let hover = Hover::new(vec![MarkdownString::new("hello")])
            .with_range(1, 0, 1, 5);
        assert_eq!(hover.contents.len(), 1);
        assert_eq!(hover.contents[0].value, "hello");
        let r = hover.range.unwrap();
        assert_eq!((r.start_line, r.start_col, r.end_line, r.end_col), (1, 0, 1, 5));
    }

    #[test]
    fn merge_hovers_combines_contents_and_takes_first_range() {
        let h1 = Hover::new(vec![MarkdownString::new("a")]).with_range(0, 0, 0, 1);
        let h2 = Hover::new(vec![MarkdownString::new("b"), MarkdownString::new("c")])
            .with_range(2, 0, 2, 5);
        let merged = merge_hovers(vec![h1, h2]);
        assert_eq!(merged.contents.len(), 3);
        assert_eq!(merged.range.unwrap().start_line, 0);
    }

    #[test]
    fn merge_hovers_empty() {
        let merged = merge_hovers(vec![]);
        assert!(merged.contents.is_empty());
        assert!(merged.range.is_none());
    }

    #[test]
    fn markdown_string_trusted() {
        let ms = MarkdownString::trusted("cmd");
        assert!(ms.is_trusted);
        assert_eq!(ms.value, "cmd");
    }

    #[test]
    fn markdown_string_append_and_is_empty() {
        let mut ms = MarkdownString::new("");
        assert!(ms.is_empty());
        ms.append("hello");
        assert!(!ms.is_empty());
        assert_eq!(ms.value, "hello");
    }

    #[test]
    fn markdown_string_append_codeblock() {
        let mut ms = MarkdownString::new("docs");
        ms.append_codeblock("let x = 1;", "rust");
        assert!(ms.value.contains("```rust\nlet x = 1;\n```"));
    }

    #[test]
    fn markdown_string_code_constructor() {
        let ms = MarkdownString::code("fn main() {}", "rust");
        assert_eq!(ms.value, "```rust\nfn main() {}\n```");
        assert!(!ms.is_trusted);
    }

    #[test]
    fn markdown_string_display() {
        let ms = MarkdownString::new("**bold**");
        assert_eq!(format!("{ms}"), "**bold**");
    }

    #[test]
    fn hover_range_contains_position() {
        let r = HoverRange::from_word(5, 2, 8);
        assert!(r.contains_position(5, 2));
        assert!(r.contains_position(5, 8));
        assert!(r.contains_position(5, 5));
        assert!(!r.contains_position(5, 1));
        assert!(!r.contains_position(5, 9));
        assert!(!r.contains_position(4, 5));
        assert!(!r.contains_position(6, 5));
    }

    #[test]
    fn hover_range_multiline_contains() {
        let r = HoverRange { start_line: 2, start_col: 5, end_line: 4, end_col: 3 };
        assert!(r.contains_position(3, 0));
        assert!(r.contains_position(2, 5));
        assert!(r.contains_position(4, 3));
        assert!(!r.contains_position(2, 4));
        assert!(!r.contains_position(4, 4));
    }

    #[test]
    fn hover_range_is_single_line() {
        assert!(HoverRange::from_word(1, 0, 5).is_single_line());
        let multi = HoverRange { start_line: 0, start_col: 0, end_line: 1, end_col: 0 };
        assert!(!multi.is_single_line());
    }

    #[test]
    fn hover_with_contents_builder() {
        let hover = Hover::new(vec![])
            .with_contents(MarkdownString::new("a"))
            .with_contents(MarkdownString::new("b"));
        assert_eq!(hover.contents.len(), 2);
        assert_eq!(hover.contents[0].value, "a");
    }

    #[test]
    fn hover_is_empty() {
        assert!(Hover::new(vec![]).is_empty());
        assert!(!Hover::new(vec![MarkdownString::new("x")]).is_empty());
    }

    #[test]
    fn filter_hovers_keeps_matching_and_rangeless() {
        let h1 = Hover::new(vec![MarkdownString::new("a")]).with_range(1, 0, 1, 5);
        let h2 = Hover::new(vec![MarkdownString::new("b")]).with_range(3, 0, 3, 5);
        let h3 = Hover::new(vec![MarkdownString::new("c")]); // no range
        let result = filter_hovers(vec![h1, h2, h3], 1, 3);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].contents[0].value, "a");
        assert_eq!(result[1].contents[0].value, "c");
    }

    // --- new tests ---

    #[test]
    fn hover_error_display_invalid_range() {
        let err = HoverError::InvalidRange {
            start_line: 5,
            start_col: 10,
            end_line: 3,
            end_col: 2,
        };
        let msg = format!("{err}");
        assert!(msg.contains("(5:10)"));
        assert!(msg.contains("(3:2)"));
    }

    #[test]
    fn hover_error_display_empty_content() {
        assert_eq!(format!("{}", HoverError::EmptyContent), "hover has no content");
    }

    #[test]
    fn hover_error_display_invalid_uri() {
        let err = HoverError::InvalidUri("bad".into());
        assert!(format!("{err}").contains("bad"));
    }

    #[test]
    fn hover_range_display() {
        let r = HoverRange::from_word(3, 5, 12);
        assert_eq!(format!("{r}"), "3:5-3:12");
    }

    #[test]
    fn hover_range_validate_ok() {
        assert!(HoverRange::from_word(1, 0, 5).validate().is_ok());
        // zero-width range is valid
        assert!(HoverRange::from_word(1, 3, 3).validate().is_ok());
    }

    #[test]
    fn hover_range_validate_err() {
        let r = HoverRange {
            start_line: 5,
            start_col: 10,
            end_line: 5,
            end_col: 2,
        };
        assert!(r.validate().is_err());
    }

    #[test]
    fn hover_range_line_span() {
        assert_eq!(HoverRange::from_word(0, 0, 5).line_span(), 1);
        let multi = HoverRange { start_line: 2, start_col: 0, end_line: 7, end_col: 0 };
        assert_eq!(multi.line_span(), 6);
    }

    #[test]
    fn hover_range_union() {
        let a = HoverRange::from_word(1, 5, 10);
        let b = HoverRange { start_line: 0, start_col: 3, end_line: 2, end_col: 8 };
        let u = a.union(&b);
        assert_eq!(u.start_line, 0);
        assert_eq!(u.start_col, 3);
        assert_eq!(u.end_line, 2);
        assert_eq!(u.end_col, 8);
    }

    #[test]
    fn markdown_string_section() {
        let ms = MarkdownString::section("Title", "Some body text.");
        assert!(ms.value.starts_with("### Title"));
        assert!(ms.value.contains("Some body text."));
    }

    #[test]
    fn markdown_string_word_count() {
        let ms = MarkdownString::new("hello world foo");
        assert_eq!(ms.word_count(), 3);
        assert_eq!(MarkdownString::new("").word_count(), 0);
    }

    #[test]
    fn markdown_string_to_plain_text() {
        let ms = MarkdownString::new("**bold** and *italic*");
        let plain = ms.to_plain_text();
        assert!(plain.contains("bold"));
        assert!(!plain.contains("**"));
    }

    #[test]
    fn hover_builder_success() {
        let hover = HoverBuilder::new()
            .content(MarkdownString::new("info"))
            .code("let x = 1;", "rust")
            .range(HoverRange::from_word(0, 0, 5))
            .build()
            .unwrap();
        assert_eq!(hover.contents.len(), 2);
        assert!(hover.range.is_some());
    }

    #[test]
    fn hover_builder_empty_content_error() {
        let result = HoverBuilder::new().build();
        assert_eq!(result, Err(HoverError::EmptyContent));
    }

    #[test]
    fn hover_builder_invalid_range_error() {
        let bad_range = HoverRange {
            start_line: 10,
            start_col: 0,
            end_line: 5,
            end_col: 0,
        };
        let result = HoverBuilder::new()
            .content(MarkdownString::new("x"))
            .range(bad_range)
            .build();
        assert!(matches!(result, Err(HoverError::InvalidRange { .. })));
    }

    #[test]
    fn validate_uri_ok() {
        assert!(validate_uri("file:///foo.rs").is_ok());
        assert!(validate_uri("https://example.com").is_ok());
    }

    #[test]
    fn validate_uri_err() {
        assert!(validate_uri("").is_err());
        assert!(validate_uri("no_scheme").is_err());
    }

    #[test]
    fn hover_registry_collects_providers() {
        struct DummyProvider;
        impl HoverProvider for DummyProvider {
            fn provide_hover(&self, _uri: &str, _line: u32, _col: u32) -> Option<Hover> {
                Some(Hover::new(vec![MarkdownString::new("dummy")]))
            }
        }
        let mut reg = HoverRegistry::new();
        assert!(reg.is_empty());
        reg.register(Box::new(DummyProvider));
        assert_eq!(reg.len(), 1);
        let hover = reg.hover_at("file:///x.rs", 0, 0).unwrap();
        assert_eq!(hover.contents[0].value, "dummy");
    }

    #[test]
    fn hover_registry_returns_none_when_no_providers_match() {
        struct NoneProvider;
        impl HoverProvider for NoneProvider {
            fn provide_hover(&self, _uri: &str, _line: u32, _col: u32) -> Option<Hover> {
                None
            }
        }
        let mut reg = HoverRegistry::new();
        reg.register(Box::new(NoneProvider));
        assert!(reg.hover_at("file:///x.rs", 0, 0).is_none());
    }

    #[test]
    fn hover_contrib_stats_new_defaults() {
        let stats = HoverContribStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn hover_contrib_stats_record_success() {
        let mut stats = HoverContribStats::new();
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
    fn hover_contrib_stats_record_failure() {
        let mut stats = HoverContribStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn hover_contrib_stats_reset() {
        let mut stats = HoverContribStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn hover_contrib_stats_merge() {
        let mut a = HoverContribStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = HoverContribStats::new();
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
    fn hover_contrib_stats_display() {
        let mut stats = HoverContribStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn hover_contrib_stats_default() {
        let stats = HoverContribStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn hover_contrib_validator_accepts_valid_name() {
        let v = HoverContribValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn hover_contrib_validator_rejects_empty() {
        let v = HoverContribValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn hover_contrib_validator_rejects_too_long() {
        let v = HoverContribValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn hover_contrib_validator_forbidden_prefix() {
        let v = HoverContribValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn hover_contrib_validator_allowed_chars() {
        let v = HoverContribValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn hover_contrib_validator_range() {
        let v = HoverContribValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn hover_contrib_sanitize_removes_control() {
        let result = HoverContribValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn hover_contrib_truncate_short_string() {
        assert_eq!(HoverContribValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn hover_contrib_truncate_long_string() {
        let result = HoverContribValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn hover_contrib_is_ascii_printable() {
        assert!(HoverContribValidator::is_ascii_printable("Hello World 123"));
        assert!(!HoverContribValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- HoverPriority --

    #[test]
    fn hover_priority_ordering() {
        assert!(HoverPriority::Builtin > HoverPriority::LanguageService);
        assert!(HoverPriority::LanguageService > HoverPriority::Normal);
        assert!(HoverPriority::Normal > HoverPriority::Fallback);
    }

    #[test]
    fn hover_priority_display() {
        assert_eq!(format!("{}", HoverPriority::Fallback), "fallback");
        assert_eq!(format!("{}", HoverPriority::Builtin), "builtin");
        assert_eq!(format!("{}", HoverPriority::LanguageService), "language-service");
    }

    #[test]
    fn sort_hovers_by_priority_order() {
        let hovers = vec![
            PrioritizedHover::new(
                Hover { contents: vec![MarkdownString::new("low")], range: None },
                HoverPriority::Fallback, "ext1",
            ),
            PrioritizedHover::new(
                Hover { contents: vec![MarkdownString::new("high")], range: None },
                HoverPriority::Builtin, "builtin",
            ),
        ];
        let sorted = sort_hovers_by_priority(hovers);
        assert_eq!(sorted[0].priority, HoverPriority::Builtin);
        assert_eq!(sorted[1].priority, HoverPriority::Fallback);
    }

    #[test]
    fn merge_prioritized_combines_contents() {
        let hovers = vec![
            PrioritizedHover::new(
                Hover { contents: vec![MarkdownString::new("A")], range: None },
                HoverPriority::Normal, "p1",
            ),
            PrioritizedHover::new(
                Hover { contents: vec![MarkdownString::new("B")], range: None },
                HoverPriority::Builtin, "p2",
            ),
        ];
        let merged = merge_prioritized(hovers);
        assert_eq!(merged.contents.len(), 2);
        assert_eq!(merged.contents[0].value, "B");
    }

    #[test]
    fn filter_by_min_priority_excludes_low() {
        let hovers = vec![
            PrioritizedHover::new(
                Hover { contents: vec![], range: None },
                HoverPriority::Fallback, "a",
            ),
            PrioritizedHover::new(
                Hover { contents: vec![], range: None },
                HoverPriority::LanguageService, "b",
            ),
        ];
        let filtered = filter_by_min_priority(hovers, HoverPriority::Normal);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].provider_id, "b");
    }

    // -- new tests --

    #[test]
    fn markdown_string_truncate_short() {
        let ms = MarkdownString::new("hi");
        let t = ms.truncate(10);
        assert_eq!(t.value, "hi");
        assert_eq!(t.is_trusted, ms.is_trusted);
    }

    #[test]
    fn markdown_string_truncate_long() {
        let ms = MarkdownString::trusted("hello world");
        let t = ms.truncate(6);
        assert!(t.value.ends_with('…'));
        assert_eq!(t.value.chars().count(), 6);
        assert!(t.is_trusted);
    }

    #[test]
    fn markdown_string_line_count() {
        assert_eq!(MarkdownString::new("").line_count(), 0);
        assert_eq!(MarkdownString::new("one").line_count(), 1);
        assert_eq!(MarkdownString::new("one\ntwo\nthree").line_count(), 3);
    }

    #[test]
    fn markdown_string_contains_code_block() {
        assert!(MarkdownString::code("x", "rs").contains_code_block());
        assert!(!MarkdownString::new("plain text").contains_code_block());
    }

    #[test]
    fn hover_total_content_length_and_section_count() {
        let h = Hover::new(vec![
            MarkdownString::new("abc"),
            MarkdownString::new("de"),
        ]);
        assert_eq!(h.total_content_length(), 5);
        assert_eq!(h.section_count(), 2);
    }

    #[test]
    fn hover_has_range() {
        assert!(!Hover::new(vec![]).has_range());
        assert!(Hover::new(vec![]).with_range(0, 0, 0, 1).has_range());
    }

    #[test]
    fn hover_display() {
        let h = Hover::new(vec![
            MarkdownString::new("first"),
            MarkdownString::new("second"),
        ]);
        let s = format!("{h}");
        assert!(s.contains("first"));
        assert!(s.contains("---"));
        assert!(s.contains("second"));
    }

    #[test]
    fn hover_priority_is_high_and_is_low() {
        assert!(HoverPriority::Builtin.is_high());
        assert!(HoverPriority::LanguageService.is_high());
        assert!(!HoverPriority::Normal.is_high());
        assert!(!HoverPriority::Fallback.is_high());
        assert!(HoverPriority::Fallback.is_low());
        assert!(!HoverPriority::Normal.is_low());
    }

    #[test]
    fn hover_priority_numeric_value() {
        assert_eq!(HoverPriority::Fallback.numeric_value(), 0);
        assert_eq!(HoverPriority::Normal.numeric_value(), 1);
        assert_eq!(HoverPriority::LanguageService.numeric_value(), 2);
        assert_eq!(HoverPriority::Builtin.numeric_value(), 3);
    }

    #[test]
    fn prioritized_hover_matches_position() {
        let ph = PrioritizedHover::new(
            Hover::new(vec![MarkdownString::new("x")]).with_range(1, 0, 1, 5),
            HoverPriority::Normal,
            "test",
        );
        assert!(ph.matches_position(1, 3));
        assert!(!ph.matches_position(2, 0));

        let no_range = PrioritizedHover::new(
            Hover::new(vec![]),
            HoverPriority::Normal,
            "test2",
        );
        assert!(no_range.matches_position(99, 99));
    }

    #[test]
    fn hover_collection_push_merge_dedup() {
        let mut coll = HoverCollection::new();
        assert!(coll.is_empty());
        coll.push(Hover::new(vec![MarkdownString::new("a")]));
        coll.push(Hover::new(vec![MarkdownString::new("b")]));
        coll.push(Hover::new(vec![MarkdownString::new("a")]));
        assert_eq!(coll.len(), 3);
        coll.dedup_by_content();
        assert_eq!(coll.len(), 2);
        let merged = coll.merge_all();
        assert_eq!(merged.contents.len(), 2);
    }

    #[test]
    fn hover_collection_filter_and_display() {
        let mut coll = HoverCollection::new();
        coll.push(Hover::new(vec![MarkdownString::new("in")]).with_range(1, 0, 1, 5));
        coll.push(Hover::new(vec![MarkdownString::new("out")]).with_range(5, 0, 5, 5));
        let s = format!("{coll}");
        assert!(s.contains("2 hovers"));
        let filtered = coll.filter_by_position(1, 3);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn hover_collection_total_content_length() {
        let mut coll = HoverCollection::new();
        coll.push(Hover::new(vec![MarkdownString::new("abc")]));
        coll.push(Hover::new(vec![MarkdownString::new("de")]));
        assert_eq!(coll.total_content_length(), 5);
    }

    #[test]
    fn hover_contrib_stats_summary_and_has_failures() {
        let mut stats = HoverContribStats::new();
        assert!(!stats.has_failures());
        stats.record_success(100);
        stats.record_failure(200);
        assert!(stats.has_failures());
        let s = stats.summary();
        assert!(s.contains("2 ops"));
        assert!(s.contains("1 ok"));
        assert!(s.contains("1 err"));
    }

    #[test]
    fn hover_registry_clear() {
        struct Dummy;
        impl HoverProvider for Dummy {
            fn provide_hover(&self, _: &str, _: u32, _: u32) -> Option<Hover> { None }
        }
        let mut reg = HoverRegistry::new();
        reg.register(Box::new(Dummy));
        assert_eq!(reg.provider_count(), 1);
        reg.clear();
        assert_eq!(reg.provider_count(), 0);
        assert!(reg.is_empty());
    }

    // -- HoverContentFormatter tests --

    #[test]
    fn content_formatter_basic() {
        let hover = Hover::new(vec![
            MarkdownString::new("Section A"),
            MarkdownString::new("Section B"),
        ]);
        let fmt = HoverContentFormatter::new().separator("\n\n");
        let result = fmt.format_hover(&hover);
        assert!(result.contains("Section A"));
        assert!(result.contains("Section B"));
        assert!(!result.contains("---"));
    }

    #[test]
    fn content_formatter_with_headers() {
        let hovers = vec![
            PrioritizedHover::new(
                Hover::new(vec![MarkdownString::new("type info")]),
                HoverPriority::LanguageService,
                "typescript",
            ),
            PrioritizedHover::new(
                Hover::new(vec![MarkdownString::new("doc")]),
                HoverPriority::Fallback,
                "docs-ext",
            ),
        ];
        let fmt = HoverContentFormatter::new().show_headers(true);
        let result = fmt.format_prioritized(&hovers);
        assert!(result.contains("**typescript**"));
        assert!(result.contains("**docs-ext**"));
        // Highest priority first
        let ts_pos = result.find("**typescript**").unwrap();
        let doc_pos = result.find("**docs-ext**").unwrap();
        assert!(ts_pos < doc_pos);
    }

    #[test]
    fn content_formatter_line_width_wraps() {
        let hover = Hover::new(vec![MarkdownString::new(
            "this is a fairly long line that should be wrapped at a reasonable width",
        )]);
        let fmt = HoverContentFormatter::new().max_line_width(20);
        let result = fmt.format_hover(&hover);
        assert!(result.contains('\n'));
        for line in result.lines() {
            // Soft-wrap can sometimes exceed width by one word, but should be
            // close.
            assert!(line.len() < 80, "line too long: {line}");
        }
    }

    // -- HoverRangeCalculator tests --

    #[test]
    fn range_calculator_offset_to_position() {
        let text = "hello\nworld\nfoo";
        assert_eq!(HoverRangeCalculator::offset_to_position(text, 0), Some((0, 0)));
        assert_eq!(HoverRangeCalculator::offset_to_position(text, 4), Some((0, 4)));
        // offset 5 is the '\n'
        assert_eq!(HoverRangeCalculator::offset_to_position(text, 6), Some((1, 0)));
        assert_eq!(HoverRangeCalculator::offset_to_position(text, text.len()), Some((2, 3)));
        assert_eq!(HoverRangeCalculator::offset_to_position(text, text.len() + 1), None);
    }

    #[test]
    fn range_calculator_word_range_at() {
        let text = "let foo_bar = 42;";
        //          0123456789...
        let r = HoverRangeCalculator::word_range_at(text, 5).unwrap();
        // "foo_bar" starts at col 4, ends at col 10
        assert_eq!(r.start_line, 0);
        assert_eq!(r.start_col, 4);
        assert_eq!(r.end_col, 10);
        // Space is not a word char
        assert!(HoverRangeCalculator::word_range_at(text, 3).is_none());
    }

    // -- HoverContentTruncator tests --

    #[test]
    fn content_truncator_limits_sections_and_chars() {
        let hover = Hover::new(vec![
            MarkdownString::new("aaa"),
            MarkdownString::new("bbb"),
            MarkdownString::new("ccc"),
            MarkdownString::new("ddd"),
        ]);
        let truncator = HoverContentTruncator::new()
            .max_sections(2)
            .max_section_chars(100)
            .max_total_chars(10_000);
        let result = truncator.truncate(hover);
        assert_eq!(result.contents.len(), 2);
        assert_eq!(result.contents[0].value, "aaa");
        assert_eq!(result.contents[1].value, "bbb");
    }

    // -- HoverProviderChain tests --

    #[test]
    fn provider_chain_short_circuit() {
        struct AlwaysHover(&'static str);
        impl HoverProvider for AlwaysHover {
            fn provide_hover(&self, _: &str, _: u32, _: u32) -> Option<Hover> {
                Some(Hover::new(vec![MarkdownString::new(self.0)]))
            }
        }

        let mut chain = HoverProviderChain::new(true);
        chain.add(Box::new(AlwaysHover("high")), HoverPriority::Builtin);
        chain.add(Box::new(AlwaysHover("low")), HoverPriority::Fallback);
        assert_eq!(chain.len(), 2);

        let results = chain.query("file:///x.rs", 0, 0);
        // Short-circuit: only one result from the highest-priority provider.
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].hover.contents[0].value, "high");

        let merged = chain.hover_at("file:///x.rs", 0, 0).unwrap();
        assert_eq!(merged.contents.len(), 1);
    }

    // -- HoverMultiSource tests --

    #[test]
    fn multi_source_merge() {
        let mut ms = HoverMultiSource::new();
        ms.add("lsp", Hover::new(vec![MarkdownString::new("Type: i32")]), 10);
        ms.add("docs", Hover::new(vec![MarkdownString::new("Docs: ...")]), 5);
        let merged = ms.merge().unwrap();
        assert_eq!(merged.contents.len(), 2);
        // Higher priority first
        assert_eq!(merged.contents[0].value, "Type: i32");
        assert_eq!(ms.source_count(), 2);
    }

    #[test]
    fn multi_source_empty() {
        let ms = HoverMultiSource::default();
        assert!(ms.merge().is_none());
        assert!(!ms.has_results());
    }

    // -- HoverStickyDelay tests --

    #[test]
    fn sticky_delay_default() {
        let d = HoverStickyDelay::default_config();
        assert!(!d.sticky);
        assert_eq!(d.show_delay_ms, 300);
        assert!(!d.should_persist(false));
        assert!(d.should_persist(true));
    }

    #[test]
    fn sticky_delay_sticky() {
        let d = HoverStickyDelay::sticky_config();
        assert!(d.sticky);
        assert!(d.should_persist(false));
    }

    // -- HoverAction tests --

    #[test]
    fn hover_action_display() {
        let action = HoverAction::new("Go to Definition", "editor.goToDefinition");
        assert_eq!(format!("{}", action), "[Go to Definition](command:editor.goToDefinition)");
    }

    #[test]
    fn hover_action_with_tooltip() {
        let action = HoverAction::new("Fix", "fix.apply").with_tooltip("Apply quick fix");
        assert_eq!(action.tooltip.as_deref(), Some("Apply quick fix"));
    }

    #[test]
    fn hover_with_actions() {
        let hover = Hover::new(vec![MarkdownString::new("info")]);
        let mut hwa = HoverWithActions::new(hover);
        assert!(!hwa.has_actions());
        hwa.add_action(HoverAction::new("Fix", "fix"));
        assert!(hwa.has_actions());
    }

    // -- HoverPosition tests --

    #[test]
    fn position_below_when_space() {
        let pos = HoverPositionCalculator::position(5, 100, 10);
        assert_eq!(pos, HoverPosition::Below);
    }

    #[test]
    fn position_above_when_near_bottom() {
        let pos = HoverPositionCalculator::position(95, 100, 10);
        assert_eq!(pos, HoverPosition::Above);
    }

    #[test]
    fn y_offset_below() {
        let y = HoverPositionCalculator::y_offset(10, 5, HoverPosition::Below);
        assert_eq!(y, 11);
    }

    #[test]
    fn y_offset_above() {
        let y = HoverPositionCalculator::y_offset(10, 5, HoverPosition::Above);
        assert_eq!(y, 5);
    }

    #[test] fn hoverDiagnosticsDisplay_new() { let s = HoverDiagnosticsDisplay::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn hoverDiagnosticsDisplay_add() { let mut s = HoverDiagnosticsDisplay::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn hoverDiagnosticsDisplay_remove() { let mut s = HoverDiagnosticsDisplay::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn hoverDiagnosticsDisplay_config() { let mut s = HoverDiagnosticsDisplay::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn hoverDiagnosticsDisplay_nav() { let mut s = HoverDiagnosticsDisplay::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn hoverDiagnosticsDisplay_filter() { let mut s = HoverDiagnosticsDisplay::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn hoverDiagnosticsDisplay_display() { assert!(format!("{}", HoverDiagnosticsDisplay::new()).contains("HoverDiagnosticsDisplay")); }
    #[test] fn hoverDefinitionPreview_new() { let s = HoverDefinitionPreview::new(); assert!(s.is_empty()); }
    #[test] fn hoverDefinitionPreview_add() { let mut s = HoverDefinitionPreview::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn hoverDefinitionPreview_active() { let mut s = HoverDefinitionPreview::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn hoverDefinitionPreview_error() { let mut s = HoverDefinitionPreview::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn hoverDefinitionPreview_rm_group() { let mut s = HoverDefinitionPreview::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn hoverDefinitionPreview_display() { assert!(format!("{}", HoverDefinitionPreview::new()).contains("HoverDefinitionPreview")); }


    #[test] fn hoverDiagnosticsDisplay_snap_capture() {
        let s = HoverDiagnosticsDisplay::new();
        let snap = HoverDiagnosticsDisplaySnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn hoverDiagnosticsDisplay_snap_stale() {
        let s = HoverDiagnosticsDisplay::new();
        let snap = HoverDiagnosticsDisplaySnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn hoverDiagnosticsDisplay_snap_diff() {
        let s = HoverDiagnosticsDisplay::new();
        let s1v = HoverDiagnosticsDisplaySnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn hoverDiagnosticsDisplay_snap_display() {
        let s = HoverDiagnosticsDisplay::new();
        let snap = HoverDiagnosticsDisplaySnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn hoverDefinitionPreview_stats_record() {
        let mut st = HoverDefinitionPreviewStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn hoverDefinitionPreview_stats_hit_ratio() {
        let mut st = HoverDefinitionPreviewStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn hoverDefinitionPreview_stats_merge() {
        let mut a = HoverDefinitionPreviewStats::new();
        a.total_adds = 5;
        let mut b = HoverDefinitionPreviewStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn hoverDefinitionPreview_stats_display() {
        let st = HoverDefinitionPreviewStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn hoverDiagnosticsDisplay_config_default() {
        let c = HoverDiagnosticsDisplayConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn hoverDiagnosticsDisplay_config_builder() {
        let c = HoverDiagnosticsDisplayConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn hoverDiagnosticsDisplay_config_labels() {
        let mut c = HoverDiagnosticsDisplayConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn hoverDiagnosticsDisplay_config_cleanup_threshold() {
        let c = HoverDiagnosticsDisplayConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn hoverDiagnosticsDisplay_config_display() {
        assert!(format!("{}", HoverDiagnosticsDisplayConfig::new()).contains("Config"));
    }
    #[test] fn hoverDefinitionPreview_stats_peaks() {
        let mut st = HoverDefinitionPreviewStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- HoverContentBuilder tests --

    #[test]
    fn builder_empty() {
        let b = HoverContentBuilder::new();
        assert!(b.is_empty());
        assert_eq!(b.build(), "");
    }

    #[test]
    fn builder_add_code_block() {
        let mut b = HoverContentBuilder::new();
        b.add_code_block("let x = 1;", "rust");
        assert!(b.build().contains("```rust"));
        assert_eq!(b.part_count(), 1);
    }

    #[test]
    fn builder_add_markdown() {
        let mut b = HoverContentBuilder::new();
        b.add_markdown("**bold**");
        assert!(b.build().contains("**bold**"));
    }

    #[test]
    fn builder_add_separator() {
        let mut b = HoverContentBuilder::new();
        b.add_markdown("a");
        b.add_separator();
        b.add_markdown("b");
        assert!(b.build().contains("---"));
        assert_eq!(b.part_count(), 3);
    }

    #[test]
    fn builder_add_signature() {
        let mut b = HoverContentBuilder::new();
        b.add_signature("fn foo()");
        assert!(b.build().contains("`fn foo()`"));
    }

    #[test]
    fn builder_multi_part() {
        let mut b = HoverContentBuilder::new();
        b.add_code_block("x", "rs").add_markdown("doc").add_separator();
        assert_eq!(b.part_count(), 3);
    }

    // -- HoverTooltipPositioner tests --

    #[test]
    fn position_no_overflow() {
        let calc = HoverTooltipPositioner::new(800, 600);
        let (x, y) = calc.compute_position(100, 200, 150, 50);
        assert_eq!(x, 100);
        assert_eq!(y, 148);
    }

    #[test]
    fn position_overflow_x() {
        let calc = HoverTooltipPositioner::new(800, 600);
        let (x, _) = calc.compute_position(750, 200, 150, 50);
        assert_eq!(x, 650);
    }

    #[test]
    fn position_below_when_no_space_above() {
        let calc = HoverTooltipPositioner::new(800, 600);
        let (_, y) = calc.compute_position(100, 10, 150, 50);
        assert_eq!(y, 12);
    }

    #[test]
    fn prefer_above_check() {
        let calc = HoverTooltipPositioner::new(800, 600);
        assert!(calc.prefer_above(100, 50));
        assert!(!calc.prefer_above(10, 50));
    }

    // -- HoverDelayManager tests --

    #[test]
    fn delay_manager_not_ready_without_start() {
        let dm = HoverDelayManager::new(500);
        assert!(!dm.is_ready());
        assert_eq!(dm.elapsed_ms(), 0);
    }

    #[test]
    fn delay_manager_cancel() {
        let mut dm = HoverDelayManager::new(0);
        dm.start();
        dm.cancel();
        assert!(!dm.is_ready());
    }

    #[test]
    fn delay_manager_delay_value() {
        let dm = HoverDelayManager::new(300);
        assert_eq!(dm.delay(), 300);
    }


    #[test]
    fn hover_contrib_entry_creation() {
        let e = HoverContribEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn hover_contrib_entry_with_priority() {
        let e = HoverContribEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn hover_contrib_entry_metadata() {
        let e = HoverContribEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn hover_contrib_entry_remove_meta() {
        let mut e = HoverContribEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn hover_contrib_entry_activate_deactivate() {
        let mut e = HoverContribEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn hover_contrib_config_add_sorted() {
        let mut c = HoverContribConfig::new(10);
        c.add(HoverContribEntry::new("lo", "Lo").with_priority(1));
        c.add(HoverContribEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn hover_contrib_config_capacity() {
        let mut c = HoverContribConfig::new(1);
        assert!(c.add(HoverContribEntry::new("a", "A")));
        assert!(!c.add(HoverContribEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn hover_contrib_config_remove() {
        let mut c = HoverContribConfig::new(10);
        c.add(HoverContribEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn hover_contrib_config_get() {
        let mut c = HoverContribConfig::new(10);
        c.add(HoverContribEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn hover_contrib_config_active_entries() {
        let mut c = HoverContribConfig::new(10);
        c.add(HoverContribEntry::new("a", "A"));
        c.add(HoverContribEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn hover_contrib_config_enable_disable() {
        let mut c = HoverContribConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn hover_contrib_config_clear() {
        let mut c = HoverContribConfig::new(10);
        c.add(HoverContribEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn hover_contrib_config_find_by_label() {
        let mut c = HoverContribConfig::new(10);
        c.add(HoverContribEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn hover_contrib_config_top_n() {
        let mut c = HoverContribConfig::new(10);
        c.add(HoverContribEntry::new("a", "A").with_priority(1));
        c.add(HoverContribEntry::new("b", "B").with_priority(2));
        c.add(HoverContribEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn hover_contrib_config_deactivate_activate_all() {
        let mut c = HoverContribConfig::new(10);
        c.add(HoverContribEntry::new("a", "A"));
        c.add(HoverContribEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn hover_contrib_config_highest_priority() {
        let mut c = HoverContribConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(HoverContribEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn hover_contrib_config_contains() {
        let mut c = HoverContribConfig::new(10);
        c.add(HoverContribEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn hover_contrib_config_labels() {
        let mut c = HoverContribConfig::new(10);
        c.add(HoverContribEntry::new("a", "Alpha"));
        c.add(HoverContribEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn hover_contrib_config_drain_inactive() {
        let mut c = HoverContribConfig::new(10);
        c.add(HoverContribEntry::new("a", "A"));
        c.add(HoverContribEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }

}
