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
// xa_ extended helpers for hover_contrib
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaHoverContribRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaHoverContribRingBuf {
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
pub struct XaHoverContribCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaHoverContribCounter {
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

impl Default for XaHoverContribCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 89
// ---------------------------------------------------------------------------

/// Generic object pool `Xc89Pool<T>`.
pub struct Xc89Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc89Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc89PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc89Pool<T> {
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
    pub fn stats(&self) -> Xc89PoolStats {
        Xc89PoolStats {
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

impl<T> Default for Xc89Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc89Scheduler`.
pub struct Xc89Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc89Scheduler {
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

impl Default for Xc89Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_89 hash for the given byte slice.
pub fn xc_89_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_89 convention.
pub fn xc_89_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_43 deepening: state machine + event bus ---

/// States for the Xd43 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd43State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd43State {
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
pub struct Xd43Transition {
    pub from: Xd43State,
    pub to: Xd43State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd43StateMachine {
    current: Xd43State,
    history: Vec<Xd43Transition>,
    step_counter: usize,
}

impl Xd43StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd43State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd43State {
        self.current
    }

    pub fn history(&self) -> &[Xd43Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd43State) -> Result<Xd43State, String> {
        let allowed = match (self.current, target) {
            (Xd43State::Idle, Xd43State::Running) => true,
            (Xd43State::Running, Xd43State::Paused) => true,
            (Xd43State::Running, Xd43State::Done) => true,
            (Xd43State::Paused, Xd43State::Running) => true,
            (Xd43State::Paused, Xd43State::Done) => true,
            (Xd43State::Done, Xd43State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_43: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd43Transition {
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
            "Xd43SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd43State> {
        let prefix = "Xd43SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd43State::Idle),
            "Running" => Some(Xd43State::Running),
            "Paused" => Some(Xd43State::Paused),
            "Done" => Some(Xd43State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd43State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd43 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd43Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd43Event {
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

type Xd43HandlerFn = Box<dyn Fn(&Xd43Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd43EventBus {
    handlers: Vec<(usize, Option<String>, Xd43HandlerFn)>,
    next_id: usize,
    published: Vec<Xd43Event>,
}

impl Xd43EventBus {
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
        F: Fn(&Xd43Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd43Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd43Event) {
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

    pub fn published_events(&self) -> &[Xd43Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #41
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf41Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf41TrieNode {
    children: std::collections::HashMap<char, Xf41TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf41Trie {
    root: Xf41TrieNode,
    count: usize,
}

impl Xf41Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf41TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf41TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf41TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf41BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf41BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 88).
pub struct Xh88SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh88SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 130 as u64,
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

/// A compact bit set supporting boolean operations (variant 88).
pub struct Xh88BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh88BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 88).
pub struct Xi88Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi88Deque<T> {
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
pub struct Xi88Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi88Interval {
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

/// A simple interval tree (variant 88).
pub struct Xi88IntervalTree {
    xi_intervals: Vec<Xi88Interval>,
}

impl Xi88IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi88Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi88Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi88Interval) -> Vec<&Xi88Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi88Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi88Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi88Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi88Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi88Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi88Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 88) ---

/// Disjoint set / union-find for crate 88.
pub struct Xj88UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj88UnionFind {
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

const XJ88_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 88.
pub struct Xj88BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj88BTreeNode<K, V>>>,
    len: usize,
}

struct Xj88BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj88BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj88BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ88_BTREE_ORDER - 1
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
        let mid = XJ88_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj88BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj88BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj88BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj88BTreeNode::xj_new_leaf();
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


    // xa_ extended tests for hover_contrib
    #[test]
    fn xa_hover_contrib_ring_new() {
        let rb = super::XaHoverContribRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_hover_contrib_ring_push_len() {
        let mut rb = super::XaHoverContribRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_hover_contrib_ring_wrap() {
        let mut rb = super::XaHoverContribRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_hover_contrib_ring_mean_empty() {
        let rb = super::XaHoverContribRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_hover_contrib_ring_mean_values() {
        let mut rb = super::XaHoverContribRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_hover_contrib_ring_min_max() {
        let mut rb = super::XaHoverContribRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_hover_contrib_ring_iter() {
        let mut rb = super::XaHoverContribRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_hover_contrib_counter_new() {
        let c = super::XaHoverContribCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_hover_contrib_counter_inc() {
        let mut c = super::XaHoverContribCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_hover_contrib_counter_inc_by() {
        let mut c = super::XaHoverContribCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_hover_contrib_counter_reset() {
        let mut c = super::XaHoverContribCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_hover_contrib_counter_clear() {
        let mut c = super::XaHoverContribCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_hover_contrib_counter_default() {
        let c = super::XaHoverContribCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 89 ----

    #[test]
    fn xc_89_pool_new_empty() {
        let pool: super::Xc89Pool<i32> = super::Xc89Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_89_pool_release_acquire() {
        let mut pool = super::Xc89Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_89_pool_acquire_empty() {
        let mut pool: super::Xc89Pool<i32> = super::Xc89Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_89_pool_full() {
        let mut pool = super::Xc89Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_89_pool_drain() {
        let mut pool = super::Xc89Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_89_pool_stats() {
        let mut pool = super::Xc89Pool::new(8);
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
    fn xc_89_pool_clear() {
        let mut pool = super::Xc89Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_89_pool_shrink() {
        let mut pool = super::Xc89Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_89_pool_default() {
        let pool: super::Xc89Pool<String> = super::Xc89Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_89_pool_extend() {
        let mut pool = super::Xc89Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_89_pool_retain() {
        let mut pool = super::Xc89Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_89_scheduler_round_robin() {
        let mut sched = super::Xc89Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_89_scheduler_empty() {
        let mut sched = super::Xc89Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_89_scheduler_reset() {
        let mut sched = super::Xc89Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_89_scheduler_add_remove() {
        let mut sched = super::Xc89Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_89_scheduler_targets() {
        let sched = super::Xc89Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_89_hash_empty() {
        assert_eq!(super::xc_89_hash(b""), 5381);
    }

    #[test]
    fn xc_89_hash_data() {
        let h = super::xc_89_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_89_hash(b"hello"), h);
    }

    #[test]
    fn xc_89_reverse_str() {
        assert_eq!(super::xc_89_reverse("abc"), "cba");
        assert_eq!(super::xc_89_reverse(""), "");
    }


    // --- xd_43 deepening tests ---

    #[test]
    fn xd_43_sm_initial_state() {
        let sm = Xd43StateMachine::new();
        assert_eq!(sm.current_state(), Xd43State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_43_sm_valid_idle_to_running() {
        let mut sm = Xd43StateMachine::new();
        assert!(sm.transition(Xd43State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd43State::Running);
    }

    #[test]
    fn xd_43_sm_valid_running_to_paused() {
        let mut sm = Xd43StateMachine::new();
        sm.transition(Xd43State::Running).unwrap();
        assert!(sm.transition(Xd43State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd43State::Paused);
    }

    #[test]
    fn xd_43_sm_valid_running_to_done() {
        let mut sm = Xd43StateMachine::new();
        sm.transition(Xd43State::Running).unwrap();
        assert!(sm.transition(Xd43State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd43State::Done);
    }

    #[test]
    fn xd_43_sm_valid_paused_to_running() {
        let mut sm = Xd43StateMachine::new();
        sm.transition(Xd43State::Running).unwrap();
        sm.transition(Xd43State::Paused).unwrap();
        assert!(sm.transition(Xd43State::Running).is_ok());
    }

    #[test]
    fn xd_43_sm_valid_done_to_idle() {
        let mut sm = Xd43StateMachine::new();
        sm.transition(Xd43State::Running).unwrap();
        sm.transition(Xd43State::Done).unwrap();
        assert!(sm.transition(Xd43State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd43State::Idle);
    }

    #[test]
    fn xd_43_sm_invalid_idle_to_done() {
        let mut sm = Xd43StateMachine::new();
        assert!(sm.transition(Xd43State::Done).is_err());
    }

    #[test]
    fn xd_43_sm_invalid_idle_to_paused() {
        let mut sm = Xd43StateMachine::new();
        assert!(sm.transition(Xd43State::Paused).is_err());
    }

    #[test]
    fn xd_43_sm_history_tracking() {
        let mut sm = Xd43StateMachine::new();
        sm.transition(Xd43State::Running).unwrap();
        sm.transition(Xd43State::Paused).unwrap();
        sm.transition(Xd43State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd43State::Idle);
        assert_eq!(sm.history()[0].to, Xd43State::Running);
        assert_eq!(sm.history()[1].from, Xd43State::Running);
        assert_eq!(sm.history()[2].to, Xd43State::Done);
    }

    #[test]
    fn xd_43_sm_serialize_deserialize() {
        let mut sm = Xd43StateMachine::new();
        sm.transition(Xd43State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd43StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd43State::Running));
    }

    #[test]
    fn xd_43_sm_deserialize_invalid() {
        assert_eq!(Xd43StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_43_sm_reset() {
        let mut sm = Xd43StateMachine::new();
        sm.transition(Xd43State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd43State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_43_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd43EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd43Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_43_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd43EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd43Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd43Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_43_bus_unsubscribe() {
        let mut bus = Xd43EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_43_event_kind_and_payload() {
        let e = Xd43Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd43Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_43_bus_clear_history() {
        let mut bus = Xd43EventBus::new();
        bus.publish(Xd43Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_43_sm_step_counter_increments() {
        let mut sm = Xd43StateMachine::new();
        sm.transition(Xd43State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd43State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #41 --

    #[test]
    fn xf41_trie_insert_search() {
        let mut t = Xf41Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf41_trie_starts_with() {
        let mut t = Xf41Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf41_trie_remove() {
        let mut t = Xf41Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf41_trie_word_count() {
        let mut t = Xf41Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf41_trie_longest_prefix() {
        let mut t = Xf41Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf41_trie_all_words() {
        let mut t = Xf41Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf41_trie_autocomplete() {
        let mut t = Xf41Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf41_trie_empty_search() {
        let t = Xf41Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf41_bloom_add_contains() {
        let mut bf = Xf41BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf41_bloom_probably_absent() {
        let bf = Xf41BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf41_bloom_false_positive_rate() {
        let mut bf = Xf41BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf41_bloom_clear() {
        let mut bf = Xf41BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf41_bloom_union() {
        let mut a = Xf41BloomFilter::xf_new(512, 2);
        let mut b = Xf41BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf41_bloom_intersection_estimate() {
        let mut a = Xf41BloomFilter::xf_new(512, 2);
        let mut b = Xf41BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf41_bloom_union_size_mismatch() {
        let a = Xf41BloomFilter::xf_new(256, 2);
        let b = Xf41BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh88_skip_insert_contains() {
        let mut sl = super::Xh88SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh88_skip_remove() {
        let mut sl = super::Xh88SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh88_skip_len() {
        let mut sl = super::Xh88SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh88_skip_range_query() {
        let mut sl = super::Xh88SkipList::xh_new(4);
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
    fn xh88_skip_floor_ceiling() {
        let mut sl = super::Xh88SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh88_skip_rank() {
        let mut sl = super::Xh88SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh88_skip_empty() {
        let sl = super::Xh88SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh88_skip_duplicates() {
        let mut sl = super::Xh88SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh88_bitset_set_test() {
        let mut bs = super::Xh88BitSet::xh_new(256);
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
    fn xh88_bitset_clear_count() {
        let mut bs = super::Xh88BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh88_bitset_and_or_xor() {
        let mut a = super::Xh88BitSet::xh_new(128);
        let mut b = super::Xh88BitSet::xh_new(128);
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
    fn xh88_bitset_iter_ones() {
        let mut bs = super::Xh88BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh88_bitset_first_last() {
        let mut bs = super::Xh88BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh88_bitset_empty() {
        let bs = super::Xh88BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi88_deque_push_pop_back() {
        let mut dq = super::Xi88Deque::xi_new(4);
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
    fn xi88_deque_push_pop_front() {
        let mut dq = super::Xi88Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi88_deque_mixed_ops() {
        let mut dq = super::Xi88Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi88_deque_get_and_split() {
        let mut dq = super::Xi88Deque::xi_new(8);
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
    fn xi88_deque_rotate_left() {
        let mut dq = super::Xi88Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi88_deque_rotate_right() {
        let mut dq = super::Xi88Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi88_deque_grow() {
        let mut dq = super::Xi88Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi88_deque_empty() {
        let dq = super::Xi88Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi88_interval_tree_insert_query() {
        let mut tree = super::Xi88IntervalTree::xi_new();
        tree.xi_insert(super::Xi88Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi88Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi88Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi88_interval_tree_overlap() {
        let mut tree = super::Xi88IntervalTree::xi_new();
        tree.xi_insert(super::Xi88Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi88Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi88Interval::xi_new(12, 20));
        let q = super::Xi88Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi88_interval_tree_remove() {
        let mut tree = super::Xi88IntervalTree::xi_new();
        tree.xi_insert(super::Xi88Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi88Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi88_interval_tree_gaps() {
        let mut tree = super::Xi88IntervalTree::xi_new();
        tree.xi_insert(super::Xi88Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi88Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi88Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi88Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi88Interval::xi_new(8, 10));
    }

    #[test]
    fn xi88_interval_tree_merge() {
        let mut tree = super::Xi88IntervalTree::xi_new();
        tree.xi_insert(super::Xi88Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi88Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi88Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi88Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi88Interval::xi_new(10, 15));
    }

    #[test]
    fn xi88_interval_tree_all() {
        let mut tree = super::Xi88IntervalTree::xi_new();
        tree.xi_insert(super::Xi88Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi88Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi88_interval_tree_empty() {
        let tree = super::Xi88IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi88_interval_tree_contains_point() {
        let iv = super::Xi88Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 88) ---

    #[test]
    fn xj_88_uf_make_and_find() {
        let mut uf = super::Xj88UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_88_uf_union_connected() {
        let mut uf = super::Xj88UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_88_uf_component_count() {
        let mut uf = super::Xj88UnionFind::xj_new();
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
    fn xj_88_uf_component_size() {
        let mut uf = super::Xj88UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_88_uf_largest_component() {
        let mut uf = super::Xj88UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_88_uf_many_elements() {
        let mut uf = super::Xj88UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_88_uf_separate_components() {
        let mut uf = super::Xj88UnionFind::xj_new();
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
    fn xj_88_uf_path_compression() {
        let mut uf = super::Xj88UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_88_bt_insert_get() {
        let mut bt = super::Xj88BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_88_bt_contains_len() {
        let mut bt = super::Xj88BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_88_bt_replace() {
        let mut bt = super::Xj88BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_88_bt_remove() {
        let mut bt = super::Xj88BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_88_bt_keys_values() {
        let mut bt = super::Xj88BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_88_bt_range() {
        let mut bt = super::Xj88BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_88_bt_min_max() {
        let mut bt = super::Xj88BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_88_bt_many_inserts() {
        let mut bt = super::Xj88BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }

}
