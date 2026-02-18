//! TextMate grammar loading and syntax highlighting via syntect.

use std::fmt;
use std::path::Path;

use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SyntectStyle, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors returned by the TextMate service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextMateError {
    /// The requested theme was not found.
    ThemeNotFound(String),
    /// No syntax matched the given query.
    SyntaxNotFound(String),
    /// Highlighting failed for the given line.
    HighlightError(String),
}

impl fmt::Display for TextMateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ThemeNotFound(name) => write!(f, "theme not found: {name}"),
            Self::SyntaxNotFound(query) => write!(f, "syntax not found: {query}"),
            Self::HighlightError(msg) => write!(f, "highlight error: {msg}"),
        }
    }
}

impl std::error::Error for TextMateError {}

// ---------------------------------------------------------------------------
// HighlightedSegment – a single styled piece of text
// ---------------------------------------------------------------------------

/// A segment of highlighted text with its foreground colour.
#[derive(Debug, Clone, PartialEq)]
pub struct HighlightedSegment {
    /// Foreground colour as (r, g, b).
    pub fg: (u8, u8, u8),
    /// The text content.
    pub text: String,
}

impl HighlightedSegment {
    /// Create a new segment.
    pub fn new(fg: (u8, u8, u8), text: impl Into<String>) -> Self {
        Self {
            fg,
            text: text.into(),
        }
    }

    /// Return `true` if the segment contains only whitespace.
    pub fn is_whitespace(&self) -> bool {
        self.text.chars().all(char::is_whitespace)
    }

    /// Byte length of the text content.
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Return `true` when the text is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Convert to a ratatui `Span`.
    pub fn to_ratatui_span(&self) -> ratatui::text::Span<'_> {
        ratatui::text::Span::styled(
            self.text.as_str(),
            ratatui::style::Style::default()
                .fg(ratatui::style::Color::Rgb(self.fg.0, self.fg.1, self.fg.2)),
        )
    }
}

impl fmt::Display for HighlightedSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.text)
    }
}

// ---------------------------------------------------------------------------
// HighlightedLine – a full highlighted source line
// ---------------------------------------------------------------------------

/// A complete highlighted line composed of segments.
#[derive(Debug, Clone, PartialEq)]
pub struct HighlightedLine {
    segments: Vec<HighlightedSegment>,
}

impl HighlightedLine {
    /// Build from a list of syntect (Style, String) pairs.
    pub fn from_syntect_ranges(ranges: &[(SyntectStyle, String)]) -> Self {
        Self {
            segments: ranges
                .iter()
                .map(|(s, t)| HighlightedSegment::new(syntect_to_rgb(*s), t.clone()))
                .collect(),
        }
    }

    /// Number of segments.
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Total byte length of the line text.
    pub fn text_len(&self) -> usize {
        self.segments.iter().map(|s| s.len()).sum()
    }

    /// Concatenate all segment texts.
    pub fn plain_text(&self) -> String {
        self.segments.iter().map(|s| s.text.as_str()).collect()
    }

    /// Iterate over the segments.
    pub fn segments(&self) -> &[HighlightedSegment] {
        &self.segments
    }

    /// Convert the whole line to ratatui `Spans`.
    pub fn to_ratatui_spans(&self) -> Vec<ratatui::text::Span<'_>> {
        self.segments.iter().map(|s| s.to_ratatui_span()).collect()
    }
}

impl fmt::Display for HighlightedLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for seg in &self.segments {
            write!(f, "{seg}")?;
        }
        Ok(())
    }
}

/// Manages loaded TextMate grammars and themes.
pub struct TextMateService {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    active_theme: String,
}

impl TextMateService {
    /// Create with syntect's default bundled grammars and themes.
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            active_theme: "base16-ocean.dark".to_string(),
        }
    }

    /// Find syntax definition for a file path.
    pub fn find_syntax_for_file(&self, path: &Path) -> Option<&SyntaxReference> {
        self.syntax_set.find_syntax_for_file(path).ok().flatten()
    }

    /// Find syntax definition by language name.
    pub fn find_syntax_by_name(&self, name: &str) -> Option<&SyntaxReference> {
        self.syntax_set.find_syntax_by_name(name)
    }

    /// Find syntax by extension.
    pub fn find_syntax_by_extension(&self, ext: &str) -> Option<&SyntaxReference> {
        self.syntax_set.find_syntax_by_extension(ext)
    }

    /// Get the active theme.
    pub fn get_active_theme(&self) -> &Theme {
        self.theme_set
            .themes
            .get(&self.active_theme)
            .unwrap_or_else(|| self.theme_set.themes.values().next().unwrap())
    }

    /// Set the active theme by name.
    pub fn set_theme(&mut self, name: &str) {
        if self.theme_set.themes.contains_key(name) {
            self.active_theme = name.to_string();
        }
    }

    /// List available theme names.
    pub fn available_themes(&self) -> Vec<&str> {
        self.theme_set.themes.keys().map(|s| s.as_str()).collect()
    }

    /// List available syntax names.
    pub fn available_syntaxes(&self) -> Vec<&str> {
        self.syntax_set
            .syntaxes()
            .iter()
            .map(|s| s.name.as_str())
            .collect()
    }

    /// Highlight a single line, returning styled segments.
    pub fn highlight_line<'a>(
        &self,
        highlighter: &mut HighlightLines<'a>,
        line: &str,
    ) -> Vec<(SyntectStyle, String)> {
        match highlighter.highlight_line(line, &self.syntax_set) {
            Ok(ranges) => ranges.into_iter().map(|(s, t)| (s, t.to_string())).collect(),
            Err(_) => vec![(SyntectStyle::default(), line.to_string())],
        }
    }

    /// Create a highlighter for a specific syntax.
    pub fn create_highlighter<'a>(&'a self, syntax: &'a SyntaxReference) -> HighlightLines<'a> {
        HighlightLines::new(syntax, self.get_active_theme())
    }

    /// Get a reference to the syntax set.
    pub fn syntax_set(&self) -> &SyntaxSet {
        &self.syntax_set
    }

    /// Return the name of the currently active theme.
    pub fn active_theme_name(&self) -> &str {
        &self.active_theme
    }

    /// Try to set the theme, returning an error when the name is unknown.
    pub fn try_set_theme(&mut self, name: &str) -> Result<(), TextMateError> {
        if self.theme_set.themes.contains_key(name) {
            self.active_theme = name.to_string();
            Ok(())
        } else {
            Err(TextMateError::ThemeNotFound(name.to_string()))
        }
    }

    /// Find a syntax definition by extension, returning a `TextMateError` on miss.
    pub fn require_syntax_by_extension<'a>(
        &'a self,
        ext: &str,
    ) -> Result<&'a SyntaxReference, TextMateError> {
        self.find_syntax_by_extension(ext)
            .ok_or_else(|| TextMateError::SyntaxNotFound(ext.to_string()))
    }

    /// Find a syntax definition by name, returning a `TextMateError` on miss.
    pub fn require_syntax_by_name<'a>(
        &'a self,
        name: &str,
    ) -> Result<&'a SyntaxReference, TextMateError> {
        self.find_syntax_by_name(name)
            .ok_or_else(|| TextMateError::SyntaxNotFound(name.to_string()))
    }

    /// Highlight a single line and return a `HighlightedLine`.
    pub fn highlight_line_structured<'a>(
        &self,
        highlighter: &mut HighlightLines<'a>,
        line: &str,
    ) -> HighlightedLine {
        let raw = self.highlight_line(highlighter, line);
        HighlightedLine::from_syntect_ranges(&raw)
    }

    /// Highlight multiple lines at once, returning a `Vec<HighlightedLine>`.
    pub fn highlight_lines<'a>(
        &self,
        highlighter: &mut HighlightLines<'a>,
        lines: &[&str],
    ) -> Vec<HighlightedLine> {
        lines
            .iter()
            .map(|line| self.highlight_line_structured(highlighter, line))
            .collect()
    }

    /// Return the number of loaded syntax definitions.
    pub fn syntax_count(&self) -> usize {
        self.syntax_set.syntaxes().len()
    }

    /// Return the number of loaded themes.
    pub fn theme_count(&self) -> usize {
        self.theme_set.themes.len()
    }
}

impl fmt::Debug for TextMateService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextMateService")
            .field("active_theme", &self.active_theme)
            .field("syntax_count", &self.syntax_count())
            .field("theme_count", &self.theme_count())
            .finish()
    }
}

impl fmt::Display for TextMateService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TextMateService(theme={}, syntaxes={}, themes={})",
            self.active_theme,
            self.syntax_count(),
            self.theme_count(),
        )
    }
}

impl Default for TextMateService {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a syntect style to RGB (r, g, b) tuple.
pub fn syntect_to_rgb(style: SyntectStyle) -> (u8, u8, u8) {
    (style.foreground.r, style.foreground.g, style.foreground.b)
}

/// Convert a syntect style to a ratatui Color.
pub fn syntect_to_ratatui_color(style: SyntectStyle) -> ratatui::style::Color {
    ratatui::style::Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b)
}

/// Accumulated statistics for wb-textmate operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbTextmateStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbTextmateStats {
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
    pub fn merge(&mut self, other: &WbTextmateStats) {
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

impl Default for WbTextmateStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbTextmateStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbTextmateStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-textmate.
#[derive(Debug, Clone)]
pub struct WbTextmateValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbTextmateValidator {
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

impl Default for WbTextmateValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Scope selector & theme scope matching
// ---------------------------------------------------------------------------

/// A TextMate scope selector for matching against scope stacks.
/// Supports dotted scope names like "source.rust keyword.control".
#[derive(Debug, Clone, PartialEq)]
pub struct ScopeSelector {
    segments: Vec<String>,
}

impl ScopeSelector {
    /// Parse a scope selector string (space-separated scope segments).
    pub fn parse(selector: &str) -> Self {
        Self {
            segments: selector.split_whitespace().map(String::from).collect(),
        }
    }

    /// Check whether this selector matches a given scope stack.
    /// Each segment in the selector must be a prefix of some scope in the stack.
    pub fn matches(&self, scope_stack: &[&str]) -> bool {
        if self.segments.is_empty() {
            return true;
        }
        let mut stack_idx = 0;
        for segment in &self.segments {
            let mut found = false;
            while stack_idx < scope_stack.len() {
                if scope_starts_with(scope_stack[stack_idx], segment) {
                    found = true;
                    stack_idx += 1;
                    break;
                }
                stack_idx += 1;
            }
            if !found {
                return false;
            }
        }
        true
    }

    /// Number of segments in this selector.
    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    /// Return the raw segments.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Check if the selector is empty (matches everything).
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

impl fmt::Display for ScopeSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.segments.join(" "))
    }
}

/// Check if a scope name starts with a given prefix, respecting dot boundaries.
fn scope_starts_with(scope: &str, prefix: &str) -> bool {
    if scope == prefix {
        return true;
    }
    scope.starts_with(prefix) && scope.as_bytes().get(prefix.len()) == Some(&b'.')
}

/// Compare two scope selectors by specificity. Returns an Ordering.
/// A selector with more segments is more specific.
/// Ties are broken by total dotted-name depth.
pub fn scope_specificity(a: &ScopeSelector, b: &ScopeSelector) -> std::cmp::Ordering {
    let seg_cmp = a.depth().cmp(&b.depth());
    if seg_cmp != std::cmp::Ordering::Equal {
        return seg_cmp;
    }
    let a_dots: usize = a.segments().iter().map(|s| s.matches('.').count()).sum();
    let b_dots: usize = b.segments().iter().map(|s| s.matches('.').count()).sum();
    a_dots.cmp(&b_dots)
}

/// A theme rule mapping a scope selector to a foreground color.
#[derive(Debug, Clone)]
pub struct ThemeScopeRule {
    pub selector: ScopeSelector,
    pub foreground: (u8, u8, u8),
    pub font_style: Option<String>,
}

impl ThemeScopeRule {
    pub fn new(selector: &str, fg: (u8, u8, u8)) -> Self {
        Self {
            selector: ScopeSelector::parse(selector),
            foreground: fg,
            font_style: None,
        }
    }

    pub fn with_font_style(mut self, style: impl Into<String>) -> Self {
        self.font_style = Some(style.into());
        self
    }
}

/// Find the most specific matching theme rule for a given scope stack.
/// Returns `None` if no rule matches.
pub fn theme_scope_lookup<'a>(
    rules: &'a [ThemeScopeRule],
    scope_stack: &[&str],
) -> Option<&'a ThemeScopeRule> {
    let mut best: Option<&ThemeScopeRule> = None;
    for rule in rules {
        if rule.selector.matches(scope_stack) {
            match best {
                None => best = Some(rule),
                Some(current_best) => {
                    if scope_specificity(&rule.selector, &current_best.selector)
                        == std::cmp::Ordering::Greater
                    {
                        best = Some(rule);
                    }
                }
            }
        }
    }
    best
}

// ---------------------------------------------------------------------------
// ScopePath – manipulating dotted TextMate scope names
// ---------------------------------------------------------------------------

/// A parsed dotted scope name (e.g. `source.rust.macro`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScopePath {
    components: Vec<String>,
}

impl ScopePath {
    /// Parse a dotted scope string into its components.
    pub fn parse(scope: &str) -> Self {
        Self {
            components: scope.split('.').map(String::from).collect(),
        }
    }

    /// Number of dotted components.
    pub fn depth(&self) -> usize {
        self.components.len()
    }

    /// Return the top-level component (e.g. `"source"` for `"source.rust"`).
    pub fn root(&self) -> Option<&str> {
        self.components.first().map(|s| s.as_str())
    }

    /// Return the leaf component (e.g. `"rust"` for `"source.rust"`).
    pub fn leaf(&self) -> Option<&str> {
        self.components.last().map(|s| s.as_str())
    }

    /// Check whether `self` is a prefix of `other`.
    pub fn is_prefix_of(&self, other: &ScopePath) -> bool {
        if self.components.len() > other.components.len() {
            return false;
        }
        self.components
            .iter()
            .zip(other.components.iter())
            .all(|(a, b)| a == b)
    }

    /// Return a new `ScopePath` with an extra component appended.
    pub fn push(&self, component: &str) -> Self {
        let mut components = self.components.clone();
        components.push(component.to_string());
        Self { components }
    }

    /// Return a parent path (all but the last component), or `None` if at root.
    pub fn parent(&self) -> Option<Self> {
        if self.components.len() <= 1 {
            return None;
        }
        Some(Self {
            components: self.components[..self.components.len() - 1].to_vec(),
        })
    }

    /// Reconstruct the dotted string.
    pub fn as_dotted(&self) -> String {
        self.components.join(".")
    }
}

impl fmt::Display for ScopePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_dotted())
    }
}

// ---------------------------------------------------------------------------
// TokenLineCache – cache highlighted lines by content hash
// ---------------------------------------------------------------------------

use std::collections::HashMap;

/// A cache mapping (line content, syntax name) pairs to highlighted output.
/// Avoids re-highlighting unchanged lines.
#[derive(Debug, Clone)]
pub struct TokenLineCache {
    entries: HashMap<(u64, String), HighlightedLine>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl TokenLineCache {
    /// Create a new cache with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            capacity,
            hits: 0,
            misses: 0,
        }
    }

    /// Simple hash of a string for cache keying.
    fn hash_line(line: &str) -> u64 {
        let mut h: u64 = 5381;
        for byte in line.bytes() {
            h = h.wrapping_mul(33).wrapping_add(u64::from(byte));
        }
        h
    }

    /// Look up a cached highlighted line.
    pub fn get(&mut self, line: &str, syntax_name: &str) -> Option<&HighlightedLine> {
        let key = (Self::hash_line(line), syntax_name.to_string());
        if self.entries.contains_key(&key) {
            self.hits += 1;
            self.entries.get(&key)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Insert a highlighted line into the cache. Evicts entries when over capacity.
    pub fn insert(&mut self, line: &str, syntax_name: &str, highlighted: HighlightedLine) {
        if self.entries.len() >= self.capacity {
            // Evict an arbitrary entry to make room.
            if let Some(key) = self.entries.keys().next().cloned() {
                self.entries.remove(&key);
            }
        }
        let key = (Self::hash_line(line), syntax_name.to_string());
        self.entries.insert(key, highlighted);
    }

    /// Number of entries currently in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all cached entries and reset counters.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    /// Return cache hit count.
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Return cache miss count.
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Return the hit rate as a fraction in [0.0, 1.0].
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        self.hits as f64 / total as f64
    }
}

impl Default for TokenLineCache {
    fn default() -> Self {
        Self::new(1024)
    }
}

// ---------------------------------------------------------------------------
// GrammarRegistry – track syntax usage statistics
// ---------------------------------------------------------------------------

/// Tracks which grammars/syntaxes have been used and how often.
#[derive(Debug, Clone, Default)]
pub struct GrammarRegistry {
    usage: HashMap<String, u64>,
}

impl GrammarRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a use of the given syntax name.
    pub fn record_use(&mut self, syntax_name: &str) {
        *self.usage.entry(syntax_name.to_string()).or_insert(0) += 1;
    }

    /// Return the usage count for a syntax, or 0 if never used.
    pub fn usage_count(&self, syntax_name: &str) -> u64 {
        self.usage.get(syntax_name).copied().unwrap_or(0)
    }

    /// Return all syntax names that have been used, sorted by usage descending.
    pub fn most_used(&self) -> Vec<(&str, u64)> {
        let mut entries: Vec<(&str, u64)> = self
            .usage
            .iter()
            .map(|(k, &v)| (k.as_str(), v))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries
    }

    /// Number of distinct syntaxes recorded.
    pub fn distinct_count(&self) -> usize {
        self.usage.len()
    }

    /// Total number of uses across all syntaxes.
    pub fn total_uses(&self) -> u64 {
        self.usage.values().sum()
    }

    /// Clear all recorded usage.
    pub fn clear(&mut self) {
        self.usage.clear();
    }
}

/// Merge two `HighlightedLine` values by concatenating their segments.
pub fn merge_highlighted_lines(a: &HighlightedLine, b: &HighlightedLine) -> HighlightedLine {
    let mut segs = a.segments.clone();
    segs.extend_from_slice(&b.segments);
    HighlightedLine { segments: segs }
}

/// Return segments from a `HighlightedLine` that are not whitespace-only.
pub fn non_whitespace_segments(line: &HighlightedLine) -> Vec<&HighlightedSegment> {
    line.segments.iter().filter(|s| !s.is_whitespace()).collect()
}

/// Total character count across all segments in a line.
pub fn total_char_count(line: &HighlightedLine) -> usize {
    line.segments.iter().map(|s| s.len()).sum()
}

/// Extract distinct foreground colors used in a highlighted line.
pub fn distinct_colors(line: &HighlightedLine) -> Vec<(u8, u8, u8)> {
    let mut colors = Vec::new();
    for seg in &line.segments {
        if !colors.contains(&seg.fg) {
            colors.push(seg.fg);
        }
    }
    colors
}

/// Check if a scope path string matches a simple glob pattern
/// where `*` matches any single component.
pub fn scope_glob_match(pattern: &str, scope: &str) -> bool {
    let pat_parts: Vec<&str> = pattern.split('.').collect();
    let scope_parts: Vec<&str> = scope.split('.').collect();
    if pat_parts.len() != scope_parts.len() {
        return false;
    }
    pat_parts
        .iter()
        .zip(scope_parts.iter())
        .all(|(p, s)| *p == "*" || p == s)
}

/// Count how many segments in a line have a specific foreground color.
pub fn count_segments_by_color(line: &HighlightedLine, fg: (u8, u8, u8)) -> usize {
    line.segments.iter().filter(|s| s.fg == fg).count()
}

/// Split a highlighted line at a given character offset, returning two new lines.
pub fn split_highlighted_line(line: &HighlightedLine, at_char: usize) -> (HighlightedLine, HighlightedLine) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut consumed = 0usize;
    for seg in &line.segments {
        if consumed >= at_char {
            right.push(seg.clone());
        } else if consumed + seg.len() <= at_char {
            left.push(seg.clone());
        } else {
            let split_pos = at_char - consumed;
            let (l, r) = seg.text.split_at(split_pos);
            left.push(HighlightedSegment { fg: seg.fg, text: l.to_string() });
            right.push(HighlightedSegment { fg: seg.fg, text: r.to_string() });
        }
        consumed += seg.len();
    }
    (HighlightedLine { segments: left }, HighlightedLine { segments: right })
}

// ---------------------------------------------------------------------------
// ColorStats – aggregate colour statistics for highlighted output
// ---------------------------------------------------------------------------

/// Statistics about colour usage across a set of highlighted lines.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorStats {
    /// Total number of segments analysed.
    pub total_segments: usize,
    /// Total character count across all segments.
    pub total_chars: usize,
    /// Number of unique foreground colours.
    pub unique_colors: usize,
    /// The (r,g,b) that covers the most characters.
    pub dominant_color: Option<(u8, u8, u8)>,
    /// Characters covered by the dominant colour.
    pub dominant_chars: usize,
}

/// Compute [`ColorStats`] for a slice of highlighted lines.
pub fn compute_color_stats(lines: &[HighlightedLine]) -> ColorStats {
    let mut freq: std::collections::HashMap<(u8, u8, u8), usize> =
        std::collections::HashMap::new();
    let mut total_segments = 0usize;
    let mut total_chars = 0usize;
    for line in lines {
        for seg in line.segments() {
            total_segments += 1;
            let chars = seg.text.chars().count();
            total_chars += chars;
            *freq.entry(seg.fg).or_insert(0) += chars;
        }
    }
    let (dominant_color, dominant_chars) = freq
        .iter()
        .max_by_key(|&(_, &count)| count)
        .map(|(&c, &n)| (Some(c), n))
        .unwrap_or((None, 0));
    ColorStats {
        total_segments,
        total_chars,
        unique_colors: freq.len(),
        dominant_color,
        dominant_chars,
    }
}

/// Return only lines that contain at least one non-whitespace segment.
pub fn non_blank_lines(lines: &[HighlightedLine]) -> Vec<&HighlightedLine> {
    lines
        .iter()
        .filter(|l| l.segments().iter().any(|s| !s.is_whitespace()))
        .collect()
}

/// Concatenate the plain text of all lines separated by newlines.
pub fn lines_to_plain_text(lines: &[HighlightedLine]) -> String {
    lines
        .iter()
        .map(|l| l.plain_text())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Search highlighted lines for segments whose text contains `needle` (case-sensitive).
/// Returns `(line_index, segment_index)` pairs.
pub fn search_segments(lines: &[HighlightedLine], needle: &str) -> Vec<(usize, usize)> {
    let mut hits = Vec::new();
    for (li, line) in lines.iter().enumerate() {
        for (si, seg) in line.segments().iter().enumerate() {
            if seg.text.contains(needle) {
                hits.push((li, si));
            }
        }
    }
    hits
}

/// Re-colour every segment in a line to the given foreground colour.
pub fn recolor_line(line: &HighlightedLine, fg: (u8, u8, u8)) -> HighlightedLine {
    HighlightedLine {
        segments: line
            .segments()
            .iter()
            .map(|s| HighlightedSegment::new(fg, s.text.clone()))
            .collect(),
    }
}

/// Trim leading whitespace-only segments from a highlighted line.
pub fn trim_leading_whitespace(line: &HighlightedLine) -> HighlightedLine {
    let segs = line.segments();
    let skip = segs.iter().take_while(|s| s.is_whitespace()).count();
    HighlightedLine {
        segments: segs[skip..].to_vec(),
    }
}

/// Trim trailing whitespace-only segments from a highlighted line.
pub fn trim_trailing_whitespace(line: &HighlightedLine) -> HighlightedLine {
    let segs = line.segments();
    let mut end = segs.len();
    while end > 0 && segs[end - 1].is_whitespace() {
        end -= 1;
    }
    HighlightedLine {
        segments: segs[..end].to_vec(),
    }
}

/// Map a function over every segment text, preserving colours.
pub fn map_segment_text<F>(line: &HighlightedLine, f: F) -> HighlightedLine
where
    F: Fn(&str) -> String,
{
    HighlightedLine {
        segments: line
            .segments()
            .iter()
            .map(|s| HighlightedSegment::new(s.fg, f(&s.text)))
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// TextMateThemeConverter – colour space helpers
// ---------------------------------------------------------------------------

/// Utilities for converting `syntect::highlighting::Color` values to various
/// representations and computing basic colour metrics.
pub struct TextMateThemeConverter;

impl TextMateThemeConverter {
    /// Create a new converter (stateless).
    pub fn new() -> Self {
        Self
    }

    /// Approximate an RGB colour to the xterm-256 palette index using the
    /// standard 6×6×6 colour cube (indices 16..=231).
    pub fn color_to_ansi256(color: syntect::highlighting::Color) -> u8 {
        let r_idx = Self::rgb_component_to_cube(color.r);
        let g_idx = Self::rgb_component_to_cube(color.g);
        let b_idx = Self::rgb_component_to_cube(color.b);
        16 + 36 * r_idx + 6 * g_idx + b_idx
    }

    /// Format an RGB colour as a CSS hex string (e.g. `#ff8000`).
    pub fn color_to_hex(color: syntect::highlighting::Color) -> String {
        format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
    }

    /// Return `true` when the perceived luminance of the colour is below 128,
    /// i.e. the colour is "dark".
    pub fn is_dark_color(color: syntect::highlighting::Color) -> bool {
        Self::luminance(color) < 128.0
    }

    /// Compute a simple contrast ratio between two colours based on the
    /// difference of their perceived luminances.  Returns a value in
    /// `[1.0, 21.0]` (following the simplified W3C formula).
    pub fn contrast_ratio(
        a: syntect::highlighting::Color,
        b: syntect::highlighting::Color,
    ) -> f64 {
        let la = Self::relative_luminance(a);
        let lb = Self::relative_luminance(b);
        let (lighter, darker) = if la > lb { (la, lb) } else { (lb, la) };
        (lighter + 0.05) / (darker + 0.05)
    }

    // -- private helpers --

    fn rgb_component_to_cube(v: u8) -> u8 {
        if v < 48 {
            0
        } else if v < 115 {
            1
        } else {
            (((v as u16) - 35) / 40).min(5) as u8
        }
    }

    fn luminance(c: syntect::highlighting::Color) -> f64 {
        0.299 * c.r as f64 + 0.587 * c.g as f64 + 0.114 * c.b as f64
    }

    fn relative_luminance(c: syntect::highlighting::Color) -> f64 {
        fn linearize(v: u8) -> f64 {
            let s = v as f64 / 255.0;
            if s <= 0.03928 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * linearize(c.r) + 0.7152 * linearize(c.g) + 0.0722 * linearize(c.b)
    }
}

// ---------------------------------------------------------------------------
// TextMateScopeInspector – debug helper for scope ↔ colour mappings
// ---------------------------------------------------------------------------

/// A single entry mapping a scope name to optional foreground / background
/// colour hex strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeEntry {
    /// The scope string (e.g. `"source.rust"`).
    pub scope: String,
    /// Optional foreground colour as hex.
    pub fg: Option<String>,
    /// Optional background colour as hex.
    pub bg: Option<String>,
}

/// Collects `ScopeEntry` items for inspection and debugging of theme-to-scope
/// assignments.
#[derive(Debug, Clone, Default)]
pub struct TextMateScopeInspector {
    entries: Vec<ScopeEntry>,
}

impl TextMateScopeInspector {
    /// Create an empty inspector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a scope → colour mapping.
    pub fn add_entry(&mut self, scope: &str, fg: Option<&str>, bg: Option<&str>) {
        self.entries.push(ScopeEntry {
            scope: scope.to_string(),
            fg: fg.map(String::from),
            bg: bg.map(String::from),
        });
    }

    /// Return all entries whose scope starts with `prefix`.
    pub fn find_by_scope(&self, prefix: &str) -> Vec<&ScopeEntry> {
        self.entries
            .iter()
            .filter(|e| e.scope.starts_with(prefix))
            .collect()
    }

    /// Produce a human-readable dump of all entries.
    pub fn dump(&self) -> String {
        let mut buf = String::new();
        for e in &self.entries {
            buf.push_str(&e.scope);
            if let Some(ref fg) = e.fg {
                buf.push_str("  fg=");
                buf.push_str(fg);
            }
            if let Some(ref bg) = e.bg {
                buf.push_str("  bg=");
                buf.push_str(bg);
            }
            buf.push('\n');
        }
        buf
    }

    /// Number of recorded entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return `true` when no entries have been recorded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// TextMateGrammarCache – track which grammars have been loaded
// ---------------------------------------------------------------------------

/// Lightweight bookkeeping for lazily loaded grammars.
#[derive(Debug, Clone, Default)]
pub struct TextMateGrammarCache {
    loaded: std::collections::HashMap<String, bool>,
    load_order: Vec<String>,
}

impl TextMateGrammarCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark a grammar as loaded, recording its load order.
    pub fn mark_loaded(&mut self, grammar_id: &str) {
        if self.loaded.insert(grammar_id.to_string(), true).is_none() {
            self.load_order.push(grammar_id.to_string());
        }
    }

    /// Check whether a grammar has been loaded.
    pub fn is_loaded(&self, grammar_id: &str) -> bool {
        self.loaded.get(grammar_id).copied().unwrap_or(false)
    }

    /// Number of currently loaded grammars.
    pub fn loaded_count(&self) -> usize {
        self.loaded.len()
    }

    /// The order in which grammars were first loaded.
    pub fn load_order(&self) -> &[String] {
        &self.load_order
    }

    /// Evict a grammar from the cache. Returns `true` if it was present.
    pub fn evict(&mut self, grammar_id: &str) -> bool {
        let removed = self.loaded.remove(grammar_id).is_some();
        if removed {
            self.load_order.retain(|id| id != grammar_id);
        }
        removed
    }
}

// ---------------------------------------------------------------------------
// ScopePriorityResolver – compare scope specificity
// ---------------------------------------------------------------------------

/// Determines which of several scope strings is most specific by counting
/// dot-separated components.
pub struct ScopePriorityResolver;

impl ScopePriorityResolver {
    /// Create a new resolver (stateless).
    pub fn new() -> Self {
        Self
    }

    /// Priority of a scope is the number of dot-separated components.
    /// More components ⇒ higher priority.
    pub fn priority(scope: &str) -> u32 {
        if scope.is_empty() {
            return 0;
        }
        scope.chars().filter(|&c| c == '.').count() as u32 + 1
    }

    /// Compare two scopes by specificity (dot count).
    pub fn compare_specificity(a: &str, b: &str) -> std::cmp::Ordering {
        Self::priority(a).cmp(&Self::priority(b))
    }

    /// Return the most specific scope from a slice, or `None` if the slice
    /// is empty.
    pub fn most_specific(scopes: &[&str]) -> Option<String> {
        scopes
            .iter()
            .max_by(|a, b| Self::compare_specificity(a, b))
            .map(|s| s.to_string())
    }

    /// Return `true` if `child` is a sub-scope of `parent`, i.e. `parent` is
    /// a dot-aligned prefix of `child`.
    pub fn is_subscope(parent: &str, child: &str) -> bool {
        if parent == child {
            return false;
        }
        child == parent
            || (child.starts_with(parent)
                && child.as_bytes().get(parent.len()) == Some(&b'.'))
    }
}


// === Textmate Injection Grammar Resolver ===

/// Textmate Injection Grammar Resolver implementation.
#[derive(Debug, Clone)]
pub struct TextmateInjectionGrammarResolver {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: TextmateInjectionGrammarResolverStats,
}

/// Statistics for TextmateInjectionGrammarResolver.
#[derive(Debug, Clone, Default)]
pub struct TextmateInjectionGrammarResolverStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl TextmateInjectionGrammarResolverStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl TextmateInjectionGrammarResolver {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: TextmateInjectionGrammarResolverStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &TextmateInjectionGrammarResolverStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for TextmateInjectionGrammarResolver {
    fn default() -> Self {
        Self::new()
    }
}

// === Textmate Scope Debugger ===

/// Priority level for TextmateScopeDebugger items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TextmateScopeDebuggerPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl TextmateScopeDebuggerPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for TextmateScopeDebuggerPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Textmate Scope Debugger implementation.
#[derive(Debug, Clone)]
pub struct TextmateScopeDebugger {
    items: Vec<TextmateScopeDebuggerItem>,
    max_items: usize,
    default_priority: TextmateScopeDebuggerPriority,
}

/// A single item in TextmateScopeDebugger.
#[derive(Debug, Clone)]
pub struct TextmateScopeDebuggerItem {
    pub id: String,
    pub label: String,
    pub priority: TextmateScopeDebuggerPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl TextmateScopeDebuggerItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: TextmateScopeDebuggerPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: TextmateScopeDebuggerPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl TextmateScopeDebugger {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: TextmateScopeDebuggerPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: TextmateScopeDebuggerItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<TextmateScopeDebuggerItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&TextmateScopeDebuggerItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: TextmateScopeDebuggerPriority) -> Vec<&TextmateScopeDebuggerItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&TextmateScopeDebuggerItem> {
        let mut sorted: Vec<&TextmateScopeDebuggerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&TextmateScopeDebuggerItem> {
        let mut sorted: Vec<&TextmateScopeDebuggerItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&TextmateScopeDebuggerItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: TextmateScopeDebuggerPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> TextmateScopeDebuggerPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &TextmateScopeDebuggerItem> {
        self.items.iter()
    }
}

impl Default for TextmateScopeDebugger {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// vsedit-wb-textmate: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WbTextmateXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl WbTextmateXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for WbTextmateXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct WbTextmateXRegistry {
    entries: Vec<WbTextmateXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl WbTextmateXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: WbTextmateXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&WbTextmateXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut WbTextmateXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<WbTextmateXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&WbTextmateXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&WbTextmateXConfig> {
        let mut sorted: Vec<&WbTextmateXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&WbTextmateXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> WbTextmateXIterator<'_> {
        WbTextmateXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct WbTextmateXIterator<'a> {
    inner: std::slice::Iter<'a, WbTextmateXConfig>,
}

impl<'a> Iterator for WbTextmateXIterator<'a> {
    type Item = &'a WbTextmateXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct WbTextmateXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl WbTextmateXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct WbTextmateXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl WbTextmateXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &WbTextmateXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &WbTextmateXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &WbTextmateXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for WbTextmateXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct WbTextmateXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl WbTextmateXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &WbTextmateXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &WbTextmateXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for WbTextmateXValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for wb_textmate
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWbTextmateRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWbTextmateRingBuf {
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
pub struct XaWbTextmateCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWbTextmateCounter {
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

impl Default for XaWbTextmateCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 228
// ---------------------------------------------------------------------------

/// Generic object pool `Xc228Pool<T>`.
pub struct Xc228Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc228Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc228PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc228Pool<T> {
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
    pub fn stats(&self) -> Xc228PoolStats {
        Xc228PoolStats {
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

impl<T> Default for Xc228Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc228Scheduler`.
pub struct Xc228Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc228Scheduler {
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

impl Default for Xc228Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_228 hash for the given byte slice.
pub fn xc_228_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_228 convention.
pub fn xc_228_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_30 deepening: state machine + event bus ---

/// States for the Xd30 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd30State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd30State {
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
pub struct Xd30Transition {
    pub from: Xd30State,
    pub to: Xd30State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd30StateMachine {
    current: Xd30State,
    history: Vec<Xd30Transition>,
    step_counter: usize,
}

impl Xd30StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd30State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd30State {
        self.current
    }

    pub fn history(&self) -> &[Xd30Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd30State) -> Result<Xd30State, String> {
        let allowed = match (self.current, target) {
            (Xd30State::Idle, Xd30State::Running) => true,
            (Xd30State::Running, Xd30State::Paused) => true,
            (Xd30State::Running, Xd30State::Done) => true,
            (Xd30State::Paused, Xd30State::Running) => true,
            (Xd30State::Paused, Xd30State::Done) => true,
            (Xd30State::Done, Xd30State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_30: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd30Transition {
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
            "Xd30SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd30State> {
        let prefix = "Xd30SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd30State::Idle),
            "Running" => Some(Xd30State::Running),
            "Paused" => Some(Xd30State::Paused),
            "Done" => Some(Xd30State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd30State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd30 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd30Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd30Event {
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

type Xd30HandlerFn = Box<dyn Fn(&Xd30Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd30EventBus {
    handlers: Vec<(usize, Option<String>, Xd30HandlerFn)>,
    next_id: usize,
    published: Vec<Xd30Event>,
}

impl Xd30EventBus {
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
        F: Fn(&Xd30Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd30Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd30Event) {
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

    pub fn published_events(&self) -> &[Xd30Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #28
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf28Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf28TrieNode {
    children: std::collections::HashMap<char, Xf28TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf28Trie {
    root: Xf28TrieNode,
    count: usize,
}

impl Xf28Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf28TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf28TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf28TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf28BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf28BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 227).
pub struct Xh227SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh227SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 269 as u64,
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

/// A compact bit set supporting boolean operations (variant 227).
pub struct Xh227BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh227BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 227).
pub struct Xi227Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi227Deque<T> {
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
pub struct Xi227Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi227Interval {
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

/// A simple interval tree (variant 227).
pub struct Xi227IntervalTree {
    xi_intervals: Vec<Xi227Interval>,
}

impl Xi227IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi227Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi227Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi227Interval) -> Vec<&Xi227Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi227Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi227Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi227Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi227Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi227Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi227Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 227) ---

/// Disjoint set / union-find for crate 227.
pub struct Xj227UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj227UnionFind {
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

const XJ227_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 227.
pub struct Xj227BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj227BTreeNode<K, V>>>,
    len: usize,
}

struct Xj227BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj227BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj227BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ227_BTREE_ORDER - 1
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
        let mid = XJ227_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj227BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj227BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj227BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj227BTreeNode::xj_new_leaf();
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


// --- xk_227 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk227SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk227SegmentTree {
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
pub struct Xk227DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk227DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_227).
#[derive(Debug, Clone)]
pub struct Xl227Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl227Rope {
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

/// Suffix array for efficient string searching (xl_227).
#[derive(Debug, Clone)]
pub struct Xl227SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl227SuffixArray {
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
    use std::path::PathBuf;
    use syntect::highlighting::Color;

    #[test]
    fn service_creation() {
        let svc = TextMateService::new();
        assert!(!svc.available_syntaxes().is_empty());
        assert!(!svc.available_themes().is_empty());
    }

    #[test]
    fn default_trait() {
        let svc = TextMateService::default();
        assert!(!svc.available_syntaxes().is_empty());
    }

    #[test]
    fn find_rust_by_extension() {
        let svc = TextMateService::new();
        let syntax = svc.find_syntax_by_extension("rs");
        assert!(syntax.is_some());
        assert_eq!(syntax.unwrap().name, "Rust");
    }

    #[test]
    fn find_python_by_name() {
        let svc = TextMateService::new();
        let syntax = svc.find_syntax_by_name("Python");
        assert!(syntax.is_some());
        assert_eq!(syntax.unwrap().name, "Python");
    }

    #[test]
    fn find_syntax_by_file_path() {
        let svc = TextMateService::new();
        let syntax = svc.find_syntax_for_file(Path::new("main.rs"));
        assert!(syntax.is_some());
        assert_eq!(syntax.unwrap().name, "Rust");
    }

    #[test]
    fn find_syntax_by_file_path_python() {
        let svc = TextMateService::new();
        let syntax = svc.find_syntax_for_file(&PathBuf::from("script.py"));
        assert!(syntax.is_some());
        assert_eq!(syntax.unwrap().name, "Python");
    }

    #[test]
    fn unknown_syntax_returns_none() {
        let svc = TextMateService::new();
        assert!(svc.find_syntax_by_extension("zzzzz").is_none());
        assert!(svc.find_syntax_by_name("NoSuchLanguage").is_none());
    }

    #[test]
    fn list_available_themes() {
        let svc = TextMateService::new();
        let themes = svc.available_themes();
        assert!(themes.contains(&"base16-ocean.dark"));
    }

    #[test]
    fn list_available_syntaxes() {
        let svc = TextMateService::new();
        let syntaxes = svc.available_syntaxes();
        assert!(syntaxes.contains(&"Rust"));
        assert!(syntaxes.contains(&"Python"));
        assert!(syntaxes.contains(&"JavaScript"));
    }

    #[test]
    fn set_theme() {
        let mut svc = TextMateService::new();
        let other: String = svc
            .available_themes()
            .into_iter()
            .find(|t| *t != "base16-ocean.dark")
            .unwrap()
            .to_string();
        svc.set_theme(&other);
        assert_eq!(svc.active_theme, other);
    }

    #[test]
    fn set_invalid_theme_is_noop() {
        let mut svc = TextMateService::new();
        svc.set_theme("nonexistent-theme");
        assert_eq!(svc.active_theme, "base16-ocean.dark");
    }

    #[test]
    fn highlight_rust_line() {
        let svc = TextMateService::new();
        let syntax = svc.find_syntax_by_extension("rs").unwrap();
        let mut hl = svc.create_highlighter(syntax);
        let result = svc.highlight_line(&mut hl, "fn main() {\n");
        assert!(!result.is_empty());
        let combined: String = result.iter().map(|(_, t)| t.as_str()).collect();
        assert!(combined.contains("fn"));
    }

    #[test]
    fn highlight_python_line() {
        let svc = TextMateService::new();
        let syntax = svc.find_syntax_by_name("Python").unwrap();
        let mut hl = svc.create_highlighter(syntax);
        let result = svc.highlight_line(&mut hl, "def hello():\n");
        assert!(!result.is_empty());
        let combined: String = result.iter().map(|(_, t)| t.as_str()).collect();
        assert!(combined.contains("def"));
    }

    #[test]
    fn syntect_to_rgb_conversion() {
        let style = SyntectStyle {
            foreground: Color { r: 255, g: 128, b: 0, a: 255 },
            background: Color { r: 0, g: 0, b: 0, a: 255 },
            font_style: Default::default(),
        };
        assert_eq!(syntect_to_rgb(style), (255, 128, 0));
    }

    #[test]
    fn syntect_to_ratatui_color_conversion() {
        let style = SyntectStyle {
            foreground: Color { r: 10, g: 20, b: 30, a: 255 },
            background: Color { r: 0, g: 0, b: 0, a: 255 },
            font_style: Default::default(),
        };
        let color = syntect_to_ratatui_color(style);
        assert_eq!(color, ratatui::style::Color::Rgb(10, 20, 30));
    }

    #[test]
    fn syntax_set_accessor() {
        let svc = TextMateService::new();
        let ss = svc.syntax_set();
        assert!(!ss.syntaxes().is_empty());
    }

    // ---- new tests ----

    #[test]
    fn textmate_error_display() {
        let e = TextMateError::ThemeNotFound("bad".into());
        assert_eq!(e.to_string(), "theme not found: bad");

        let e = TextMateError::SyntaxNotFound("xyz".into());
        assert_eq!(e.to_string(), "syntax not found: xyz");

        let e = TextMateError::HighlightError("oops".into());
        assert_eq!(e.to_string(), "highlight error: oops");
    }

    #[test]
    fn textmate_error_is_std_error() {
        let e: Box<dyn std::error::Error> =
            Box::new(TextMateError::ThemeNotFound("x".into()));
        assert!(e.to_string().contains("theme not found"));
    }

    #[test]
    fn try_set_theme_ok() {
        let mut svc = TextMateService::new();
        let name = svc.available_themes()[0].to_string();
        assert!(svc.try_set_theme(&name).is_ok());
        assert_eq!(svc.active_theme_name(), name);
    }

    #[test]
    fn try_set_theme_err() {
        let mut svc = TextMateService::new();
        let result = svc.try_set_theme("no-such-theme");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            TextMateError::ThemeNotFound("no-such-theme".into())
        );
    }

    #[test]
    fn require_syntax_by_extension_ok() {
        let svc = TextMateService::new();
        let syn = svc.require_syntax_by_extension("rs");
        assert!(syn.is_ok());
        assert_eq!(syn.unwrap().name, "Rust");
    }

    #[test]
    fn require_syntax_by_extension_err() {
        let svc = TextMateService::new();
        let syn = svc.require_syntax_by_extension("zzzzz");
        assert_eq!(
            syn.unwrap_err(),
            TextMateError::SyntaxNotFound("zzzzz".into())
        );
    }

    #[test]
    fn require_syntax_by_name_ok_and_err() {
        let svc = TextMateService::new();
        assert!(svc.require_syntax_by_name("Rust").is_ok());
        assert!(svc.require_syntax_by_name("NoLang").is_err());
    }

    #[test]
    fn syntax_and_theme_counts() {
        let svc = TextMateService::new();
        assert!(svc.syntax_count() > 0);
        assert!(svc.theme_count() > 0);
    }

    #[test]
    fn debug_and_display_impls() {
        let svc = TextMateService::new();
        let dbg = format!("{:?}", svc);
        assert!(dbg.contains("TextMateService"));
        assert!(dbg.contains("active_theme"));

        let disp = format!("{}", svc);
        assert!(disp.contains("base16-ocean.dark"));
    }

    #[test]
    fn highlighted_segment_basics() {
        let seg = HighlightedSegment::new((255, 0, 0), "hello");
        assert_eq!(seg.len(), 5);
        assert!(!seg.is_empty());
        assert!(!seg.is_whitespace());
        assert_eq!(seg.to_string(), "hello");

        let ws = HighlightedSegment::new((0, 0, 0), "  ");
        assert!(ws.is_whitespace());
    }

    #[test]
    fn highlighted_line_from_ranges() {
        let style = SyntectStyle {
            foreground: Color { r: 100, g: 200, b: 50, a: 255 },
            background: Color { r: 0, g: 0, b: 0, a: 255 },
            font_style: Default::default(),
        };
        let ranges = vec![
            (style, "fn ".to_string()),
            (style, "main".to_string()),
        ];
        let line = HighlightedLine::from_syntect_ranges(&ranges);
        assert_eq!(line.segment_count(), 2);
        assert_eq!(line.text_len(), 7);
        assert_eq!(line.plain_text(), "fn main");
        assert_eq!(line.to_string(), "fn main");
    }

    #[test]
    fn highlight_lines_structured() {
        let svc = TextMateService::new();
        let syn = svc.find_syntax_by_extension("rs").unwrap();
        let mut hl = svc.create_highlighter(syn);
        let lines = svc.highlight_lines(&mut hl, &["fn main() {\n", "}\n"]);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].plain_text().contains("fn"));
    }

    #[test]
    fn highlighted_line_to_ratatui_spans() {
        let style = SyntectStyle {
            foreground: Color { r: 10, g: 20, b: 30, a: 255 },
            background: Color { r: 0, g: 0, b: 0, a: 255 },
            font_style: Default::default(),
        };
        let ranges = vec![(style, "code".to_string())];
        let line = HighlightedLine::from_syntect_ranges(&ranges);
        let spans = line.to_ratatui_spans();
        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn wb_textmate_stats_new_defaults() {
        let stats = WbTextmateStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_textmate_stats_record_success() {
        let mut stats = WbTextmateStats::new();
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
    fn wb_textmate_stats_record_failure() {
        let mut stats = WbTextmateStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_textmate_stats_reset() {
        let mut stats = WbTextmateStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_textmate_stats_merge() {
        let mut a = WbTextmateStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbTextmateStats::new();
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
    fn wb_textmate_stats_display() {
        let mut stats = WbTextmateStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_textmate_stats_default() {
        let stats = WbTextmateStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_textmate_validator_accepts_valid_name() {
        let v = WbTextmateValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_textmate_validator_rejects_empty() {
        let v = WbTextmateValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_textmate_validator_rejects_too_long() {
        let v = WbTextmateValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_textmate_validator_forbidden_prefix() {
        let v = WbTextmateValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_textmate_validator_allowed_chars() {
        let v = WbTextmateValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_textmate_validator_range() {
        let v = WbTextmateValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_textmate_sanitize_removes_control() {
        let result = WbTextmateValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_textmate_truncate_short_string() {
        assert_eq!(WbTextmateValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_textmate_truncate_long_string() {
        let result = WbTextmateValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_textmate_is_ascii_printable() {
        assert!(WbTextmateValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbTextmateValidator::is_ascii_printable("Hello\x00World"));
    }

    // ---- scope selector & theme scope tests ----

    #[test]
    fn scope_selector_parse_simple() {
        let sel = ScopeSelector::parse("source.rust");
        assert_eq!(sel.depth(), 1);
        assert_eq!(sel.segments()[0], "source.rust");
    }

    #[test]
    fn scope_selector_parse_multi() {
        let sel = ScopeSelector::parse("source.rust keyword.control");
        assert_eq!(sel.depth(), 2);
        assert_eq!(sel.segments()[0], "source.rust");
        assert_eq!(sel.segments()[1], "keyword.control");
    }

    #[test]
    fn scope_selector_empty_matches_all() {
        let sel = ScopeSelector::parse("");
        assert!(sel.is_empty());
        assert!(sel.matches(&["source.rust", "keyword.control"]));
    }

    #[test]
    fn scope_selector_matches_exact() {
        let sel = ScopeSelector::parse("source.rust");
        assert!(sel.matches(&["source.rust"]));
        assert!(!sel.matches(&["source.python"]));
    }

    #[test]
    fn scope_selector_matches_prefix() {
        let sel = ScopeSelector::parse("source");
        assert!(sel.matches(&["source.rust"]));
        assert!(sel.matches(&["source.python"]));
    }

    #[test]
    fn scope_selector_no_partial_dot_match() {
        let sel = ScopeSelector::parse("source.r");
        assert!(!sel.matches(&["source.rust"]));
    }

    #[test]
    fn scope_selector_multi_segment_match() {
        let sel = ScopeSelector::parse("source.rust keyword.control");
        assert!(sel.matches(&["source.rust", "keyword.control.if"]));
        assert!(!sel.matches(&["source.rust", "string.quoted"]));
    }

    #[test]
    fn scope_selector_display() {
        let sel = ScopeSelector::parse("source.rust keyword.control");
        assert_eq!(format!("{sel}"), "source.rust keyword.control");
    }

    #[test]
    fn scope_specificity_more_segments_wins() {
        let a = ScopeSelector::parse("source.rust keyword.control");
        let b = ScopeSelector::parse("source.rust");
        assert_eq!(scope_specificity(&a, &b), std::cmp::Ordering::Greater);
    }

    #[test]
    fn scope_specificity_equal_segments_more_dots_wins() {
        let a = ScopeSelector::parse("source.rust.macro");
        let b = ScopeSelector::parse("source.rust");
        assert_eq!(scope_specificity(&a, &b), std::cmp::Ordering::Greater);
    }

    #[test]
    fn scope_specificity_equal() {
        let a = ScopeSelector::parse("source.rust");
        let b = ScopeSelector::parse("source.python");
        assert_eq!(scope_specificity(&a, &b), std::cmp::Ordering::Equal);
    }

    #[test]
    fn theme_scope_rule_new() {
        let rule = ThemeScopeRule::new("source.rust", (255, 0, 0));
        assert_eq!(rule.foreground, (255, 0, 0));
        assert!(rule.font_style.is_none());
    }

    #[test]
    fn theme_scope_rule_with_font_style() {
        let rule = ThemeScopeRule::new("keyword", (0, 255, 0)).with_font_style("bold");
        assert_eq!(rule.font_style.as_deref(), Some("bold"));
    }

    #[test]
    fn theme_scope_lookup_finds_best_match() {
        let rules = vec![
            ThemeScopeRule::new("source", (100, 100, 100)),
            ThemeScopeRule::new("source.rust", (200, 0, 0)),
            ThemeScopeRule::new("source.rust keyword.control", (0, 200, 0)),
        ];
        let stack = &["source.rust", "keyword.control.if"];
        let best = theme_scope_lookup(&rules, stack).unwrap();
        assert_eq!(best.foreground, (0, 200, 0));
    }

    #[test]
    fn theme_scope_lookup_fallback_to_less_specific() {
        let rules = vec![
            ThemeScopeRule::new("source", (100, 100, 100)),
            ThemeScopeRule::new("source.rust keyword.control", (0, 200, 0)),
        ];
        let stack = &["source.rust", "string.quoted"];
        let best = theme_scope_lookup(&rules, stack).unwrap();
        assert_eq!(best.foreground, (100, 100, 100));
    }

    #[test]
    fn theme_scope_lookup_no_match() {
        let rules = vec![
            ThemeScopeRule::new("source.python", (200, 0, 0)),
        ];
        let stack = &["source.rust"];
        assert!(theme_scope_lookup(&rules, stack).is_none());
    }

    #[test]
    fn scope_starts_with_respects_dots() {
        assert!(scope_starts_with("source.rust", "source"));
        assert!(scope_starts_with("source.rust", "source.rust"));
        assert!(!scope_starts_with("source.rust", "source.r"));
        assert!(!scope_starts_with("source.rust", "sourc"));
    }

    #[test]
    fn scope_selector_order_matters() {
        let sel = ScopeSelector::parse("keyword.control source.rust");
        // keyword.control must come before source.rust in the stack
        assert!(!sel.matches(&["source.rust", "keyword.control"]));
        assert!(sel.matches(&["keyword.control", "source.rust"]));
    }

    // ---- ScopePath tests ----

    #[test]
    fn scope_path_parse_and_components() {
        let p = ScopePath::parse("source.rust.macro");
        assert_eq!(p.depth(), 3);
        assert_eq!(p.root(), Some("source"));
        assert_eq!(p.leaf(), Some("macro"));
        assert_eq!(p.as_dotted(), "source.rust.macro");
        assert_eq!(format!("{p}"), "source.rust.macro");
    }

    #[test]
    fn scope_path_parent_and_push() {
        let p = ScopePath::parse("source.rust.macro");
        let parent = p.parent().unwrap();
        assert_eq!(parent.as_dotted(), "source.rust");
        let root = parent.parent().unwrap();
        assert_eq!(root.as_dotted(), "source");
        assert!(root.parent().is_none());

        let extended = root.push("python");
        assert_eq!(extended.as_dotted(), "source.python");
    }

    #[test]
    fn scope_path_is_prefix_of() {
        let short = ScopePath::parse("source.rust");
        let long = ScopePath::parse("source.rust.macro");
        assert!(short.is_prefix_of(&long));
        assert!(!long.is_prefix_of(&short));
        assert!(short.is_prefix_of(&short));
    }

    // ---- TokenLineCache tests ----

    #[test]
    fn token_line_cache_insert_and_get() {
        let mut cache = TokenLineCache::new(10);
        assert!(cache.is_empty());

        let line = HighlightedLine::from_syntect_ranges(&[]);
        cache.insert("fn main() {", "Rust", line.clone());
        assert_eq!(cache.len(), 1);

        let cached = cache.get("fn main() {", "Rust");
        assert!(cached.is_some());
        assert_eq!(cache.hits(), 1);
        assert_eq!(cache.misses(), 0);

        assert!(cache.get("other line", "Rust").is_none());
        assert_eq!(cache.misses(), 1);
    }

    #[test]
    fn token_line_cache_eviction() {
        let mut cache = TokenLineCache::new(2);
        let line = HighlightedLine::from_syntect_ranges(&[]);
        cache.insert("line1", "Rust", line.clone());
        cache.insert("line2", "Rust", line.clone());
        assert_eq!(cache.len(), 2);
        // Third insert should evict one entry to stay at capacity.
        cache.insert("line3", "Rust", line);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn token_line_cache_hit_rate() {
        let mut cache = TokenLineCache::new(10);
        let line = HighlightedLine::from_syntect_ranges(&[]);
        cache.insert("x", "Rust", line);
        cache.get("x", "Rust"); // hit
        cache.get("y", "Rust"); // miss
        assert!((cache.hit_rate() - 0.5).abs() < f64::EPSILON);

        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.hits(), 0);
        assert_eq!(cache.misses(), 0);
        assert!((cache.hit_rate() - 0.0).abs() < f64::EPSILON);
    }

    // ---- GrammarRegistry tests ----

    #[test]
    fn grammar_registry_tracks_usage() {
        let mut reg = GrammarRegistry::new();
        assert_eq!(reg.distinct_count(), 0);
        assert_eq!(reg.total_uses(), 0);

        reg.record_use("Rust");
        reg.record_use("Rust");
        reg.record_use("Python");
        assert_eq!(reg.usage_count("Rust"), 2);
        assert_eq!(reg.usage_count("Python"), 1);
        assert_eq!(reg.usage_count("Go"), 0);
        assert_eq!(reg.distinct_count(), 2);
        assert_eq!(reg.total_uses(), 3);
    }

    #[test]
    fn grammar_registry_most_used_order() {
        let mut reg = GrammarRegistry::new();
        reg.record_use("Go");
        reg.record_use("Rust");
        reg.record_use("Rust");
        reg.record_use("Rust");
        reg.record_use("Python");
        reg.record_use("Python");
        let top = reg.most_used();
        assert_eq!(top[0], ("Rust", 3));
        assert_eq!(top[1], ("Python", 2));
        assert_eq!(top[2], ("Go", 1));

        reg.clear();
        assert_eq!(reg.distinct_count(), 0);
    }

    #[test]
    fn merge_highlighted_lines_concatenates() {
        let a = HighlightedLine {
            segments: vec![HighlightedSegment::new((255, 0, 0), "hello")],
        };
        let b = HighlightedLine {
            segments: vec![HighlightedSegment::new((0, 255, 0), " world")],
        };
        let merged = merge_highlighted_lines(&a, &b);
        assert_eq!(merged.segment_count(), 2);
        assert_eq!(merged.plain_text(), "hello world");
    }

    #[test]
    fn non_whitespace_segments_filters() {
        let line = HighlightedLine {
            segments: vec![
                HighlightedSegment::new((255, 255, 255), "  "),
                HighlightedSegment::new((200, 200, 200), "code"),
                HighlightedSegment::new((255, 255, 255), "\t"),
            ],
        };
        let nws = non_whitespace_segments(&line);
        assert_eq!(nws.len(), 1);
        assert_eq!(nws[0].text, "code");
    }

    #[test]
    fn total_char_count_sums_segments() {
        let line = HighlightedLine {
            segments: vec![
                HighlightedSegment::new((0, 0, 0), "abc"),
                HighlightedSegment::new((0, 0, 0), "de"),
            ],
        };
        assert_eq!(total_char_count(&line), 5);
        assert_eq!(total_char_count(&HighlightedLine { segments: vec![] }), 0);
    }

    #[test]
    fn distinct_colors_unique() {
        let line = HighlightedLine {
            segments: vec![
                HighlightedSegment::new((255, 0, 0), "a"),
                HighlightedSegment::new((255, 0, 0), "b"),
                HighlightedSegment::new((0, 0, 255), "c"),
            ],
        };
        let colors = distinct_colors(&line);
        assert_eq!(colors.len(), 2);
    }

    #[test]
    fn scope_glob_match_works() {
        assert!(scope_glob_match("source.*", "source.rust"));
        assert!(scope_glob_match("source.rust", "source.rust"));
        assert!(!scope_glob_match("source.*", "source.rust.macro"));
        assert!(!scope_glob_match("source.python", "source.rust"));
        assert!(scope_glob_match("*.*", "source.rust"));
    }

    #[test]
    fn count_segments_by_color_counts() {
        let line = HighlightedLine {
            segments: vec![
                HighlightedSegment::new((1, 2, 3), "a"),
                HighlightedSegment::new((4, 5, 6), "b"),
                HighlightedSegment::new((1, 2, 3), "c"),
            ],
        };
        assert_eq!(count_segments_by_color(&line, (1, 2, 3)), 2);
        assert_eq!(count_segments_by_color(&line, (4, 5, 6)), 1);
        assert_eq!(count_segments_by_color(&line, (0, 0, 0)), 0);
    }

    #[test]
    fn split_highlighted_line_at_boundary() {
        let line = HighlightedLine {
            segments: vec![
                HighlightedSegment::new((255, 0, 0), "abc"),
                HighlightedSegment::new((0, 255, 0), "def"),
            ],
        };
        let (left, right) = split_highlighted_line(&line, 3);
        assert_eq!(left.plain_text(), "abc");
        assert_eq!(right.plain_text(), "def");
    }

    #[test]
    fn split_highlighted_line_mid_segment() {
        let line = HighlightedLine {
            segments: vec![HighlightedSegment::new((100, 100, 100), "abcdef")],
        };
        let (left, right) = split_highlighted_line(&line, 2);
        assert_eq!(left.plain_text(), "ab");
        assert_eq!(right.plain_text(), "cdef");
    }

    #[test]
    fn color_stats_empty() {
        let stats = compute_color_stats(&[]);
        assert_eq!(stats.total_segments, 0);
        assert_eq!(stats.total_chars, 0);
        assert_eq!(stats.unique_colors, 0);
        assert!(stats.dominant_color.is_none());
    }

    #[test]
    fn color_stats_single_line() {
        let line = HighlightedLine {
            segments: vec![
                HighlightedSegment::new((255, 0, 0), "red"),
                HighlightedSegment::new((0, 255, 0), "green!"),
            ],
        };
        let stats = compute_color_stats(&[line]);
        assert_eq!(stats.total_segments, 2);
        assert_eq!(stats.total_chars, 9);
        assert_eq!(stats.unique_colors, 2);
        assert_eq!(stats.dominant_color, Some((0, 255, 0)));
        assert_eq!(stats.dominant_chars, 6);
    }

    #[test]
    fn non_blank_lines_filters() {
        let blank = HighlightedLine { segments: vec![HighlightedSegment::new((0, 0, 0), "   ")] };
        let code = HighlightedLine {
            segments: vec![HighlightedSegment::new((0, 0, 0), "fn main()")],
        };
        let lines = [blank, code];
        let result = non_blank_lines(&lines);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].plain_text(), "fn main()");
    }

    #[test]
    fn lines_to_plain_text_joins() {
        let a = HighlightedLine { segments: vec![HighlightedSegment::new((0, 0, 0), "hello")] };
        let b = HighlightedLine { segments: vec![HighlightedSegment::new((0, 0, 0), "world")] };
        assert_eq!(lines_to_plain_text(&[a, b]), "hello\nworld");
    }

    #[test]
    fn search_segments_finds_matches() {
        let line = HighlightedLine {
            segments: vec![
                HighlightedSegment::new((0, 0, 0), "fn main()"),
                HighlightedSegment::new((0, 0, 0), " { }"),
            ],
        };
        let hits = search_segments(&[line], "main");
        assert_eq!(hits, vec![(0, 0)]);
    }

    #[test]
    fn recolor_line_changes_all_fg() {
        let line = HighlightedLine {
            segments: vec![
                HighlightedSegment::new((1, 2, 3), "a"),
                HighlightedSegment::new((4, 5, 6), "b"),
            ],
        };
        let recolored = recolor_line(&line, (255, 255, 255));
        for seg in recolored.segments() {
            assert_eq!(seg.fg, (255, 255, 255));
        }
    }

    #[test]
    fn trim_leading_whitespace_works() {
        let line = HighlightedLine {
            segments: vec![
                HighlightedSegment::new((0, 0, 0), "  "),
                HighlightedSegment::new((0, 0, 0), "\t"),
                HighlightedSegment::new((1, 1, 1), "code"),
            ],
        };
        let trimmed = trim_leading_whitespace(&line);
        assert_eq!(trimmed.segment_count(), 1);
        assert_eq!(trimmed.plain_text(), "code");
    }

    #[test]
    fn trim_trailing_whitespace_works() {
        let line = HighlightedLine {
            segments: vec![
                HighlightedSegment::new((1, 1, 1), "code"),
                HighlightedSegment::new((0, 0, 0), "  "),
            ],
        };
        let trimmed = trim_trailing_whitespace(&line);
        assert_eq!(trimmed.segment_count(), 1);
        assert_eq!(trimmed.plain_text(), "code");
    }

    #[test]
    fn map_segment_text_uppercases() {
        let line = HighlightedLine {
            segments: vec![HighlightedSegment::new((0, 0, 0), "hello")],
        };
        let mapped = map_segment_text(&line, |s| s.to_uppercase());
        assert_eq!(mapped.plain_text(), "HELLO");
    }

    // ---- TextMateThemeConverter tests ----

    #[test]
    fn theme_converter_color_to_hex() {
        let c = Color { r: 255, g: 128, b: 0, a: 255 };
        assert_eq!(TextMateThemeConverter::color_to_hex(c), "#ff8000");
        let black = Color { r: 0, g: 0, b: 0, a: 255 };
        assert_eq!(TextMateThemeConverter::color_to_hex(black), "#000000");
    }

    #[test]
    fn theme_converter_color_to_ansi256() {
        let white = Color { r: 255, g: 255, b: 255, a: 255 };
        let idx = TextMateThemeConverter::color_to_ansi256(white);
        // White maps to cube(5,5,5) = 16 + 36*5 + 6*5 + 5 = 231
        assert_eq!(idx, 231);
        let black = Color { r: 0, g: 0, b: 0, a: 255 };
        assert_eq!(TextMateThemeConverter::color_to_ansi256(black), 16);
    }

    #[test]
    fn theme_converter_is_dark_color() {
        let dark = Color { r: 10, g: 10, b: 10, a: 255 };
        assert!(TextMateThemeConverter::is_dark_color(dark));
        let bright = Color { r: 255, g: 255, b: 255, a: 255 };
        assert!(!TextMateThemeConverter::is_dark_color(bright));
    }

    #[test]
    fn theme_converter_contrast_ratio() {
        let black = Color { r: 0, g: 0, b: 0, a: 255 };
        let white = Color { r: 255, g: 255, b: 255, a: 255 };
        let ratio = TextMateThemeConverter::contrast_ratio(black, white);
        assert!(ratio > 15.0, "black/white contrast should be high, got {ratio}");
        let self_ratio = TextMateThemeConverter::contrast_ratio(black, black);
        assert!((self_ratio - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn theme_converter_new() {
        let _conv = TextMateThemeConverter::new();
    }

    // ---- TextMateScopeInspector tests ----

    #[test]
    fn scope_inspector_add_and_find() {
        let mut insp = TextMateScopeInspector::new();
        assert!(insp.is_empty());
        insp.add_entry("source.rust", Some("#ff0000"), None);
        insp.add_entry("source.python", None, Some("#000000"));
        insp.add_entry("keyword.control", Some("#00ff00"), Some("#111111"));
        assert_eq!(insp.len(), 3);

        let found = insp.find_by_scope("source");
        assert_eq!(found.len(), 2);
        let kw = insp.find_by_scope("keyword");
        assert_eq!(kw.len(), 1);
        assert_eq!(kw[0].fg.as_deref(), Some("#00ff00"));
    }

    #[test]
    fn scope_inspector_dump_format() {
        let mut insp = TextMateScopeInspector::new();
        insp.add_entry("source.rust", Some("#ff0000"), None);
        let dump = insp.dump();
        assert!(dump.contains("source.rust"));
        assert!(dump.contains("fg=#ff0000"));
        assert!(!dump.contains("bg="));
    }

    // ---- TextMateGrammarCache tests ----

    #[test]
    fn grammar_cache_load_and_evict() {
        let mut cache = TextMateGrammarCache::new();
        assert_eq!(cache.loaded_count(), 0);
        assert!(!cache.is_loaded("rust"));

        cache.mark_loaded("rust");
        cache.mark_loaded("python");
        cache.mark_loaded("rust"); // duplicate – no-op
        assert_eq!(cache.loaded_count(), 2);
        assert!(cache.is_loaded("rust"));
        assert_eq!(cache.load_order(), &["rust", "python"]);

        assert!(cache.evict("rust"));
        assert!(!cache.is_loaded("rust"));
        assert_eq!(cache.loaded_count(), 1);
        assert!(!cache.evict("rust")); // already gone
    }

    // ---- ScopePriorityResolver tests ----

    #[test]
    fn scope_priority_dot_count() {
        assert_eq!(ScopePriorityResolver::priority("source"), 1);
        assert_eq!(ScopePriorityResolver::priority("source.rust"), 2);
        assert_eq!(ScopePriorityResolver::priority("source.rust.macro"), 3);
        assert_eq!(ScopePriorityResolver::priority(""), 0);
    }

    #[test]
    fn scope_priority_compare_and_most_specific() {
        use std::cmp::Ordering;
        assert_eq!(
            ScopePriorityResolver::compare_specificity("source", "source.rust"),
            Ordering::Less
        );
        let scopes: Vec<&str> = vec!["source", "source.rust", "source.rust.macro"];
        let best = ScopePriorityResolver::most_specific(&scopes);
        assert_eq!(best.as_deref(), Some("source.rust.macro"));
        assert_eq!(ScopePriorityResolver::most_specific(&[]), None);
    }

    #[test]
    fn scope_priority_is_subscope() {
        assert!(ScopePriorityResolver::is_subscope("source", "source.rust"));
        assert!(ScopePriorityResolver::is_subscope("source.rust", "source.rust.macro"));
        assert!(!ScopePriorityResolver::is_subscope("source.rust", "source.rust"));
        assert!(!ScopePriorityResolver::is_subscope("source.r", "source.rust"));
    }

    #[test]
    fn textmateInjectionGrammarResolver_new() {
        let s = TextmateInjectionGrammarResolver::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn textmateInjectionGrammarResolver_add_contains() {
        let mut s = TextmateInjectionGrammarResolver::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn textmateInjectionGrammarResolver_add_duplicate() {
        let mut s = TextmateInjectionGrammarResolver::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn textmateInjectionGrammarResolver_remove() {
        let mut s = TextmateInjectionGrammarResolver::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn textmateInjectionGrammarResolver_capacity() {
        let s = TextmateInjectionGrammarResolver::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn textmateInjectionGrammarResolver_search() {
        let mut s = TextmateInjectionGrammarResolver::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn textmateInjectionGrammarResolver_stats() {
        let mut s = TextmateInjectionGrammarResolver::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn textmateScopeDebugger_new() {
        let m = TextmateScopeDebugger::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn textmateScopeDebugger_add_find() {
        let mut m = TextmateScopeDebugger::new();
        m.add(TextmateScopeDebuggerItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn textmateScopeDebugger_priority_filter() {
        let mut m = TextmateScopeDebugger::new();
        m.add(TextmateScopeDebuggerItem::new("a", "A").with_priority(TextmateScopeDebuggerPriority::High));
        m.add(TextmateScopeDebuggerItem::new("b", "B").with_priority(TextmateScopeDebuggerPriority::Low));
        m.add(TextmateScopeDebuggerItem::new("c", "C").with_priority(TextmateScopeDebuggerPriority::High));
        assert_eq!(m.by_priority(TextmateScopeDebuggerPriority::High).len(), 2);
    }

    #[test]
    fn textmateScopeDebugger_remove() {
        let mut m = TextmateScopeDebugger::new();
        m.add(TextmateScopeDebuggerItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn textmateScopeDebugger_search() {
        let mut m = TextmateScopeDebugger::new();
        m.add(TextmateScopeDebuggerItem::new("id1", "Hello World"));
        m.add(TextmateScopeDebuggerItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn textmateScopeDebugger_total_weight() {
        let mut m = TextmateScopeDebugger::new();
        m.add(TextmateScopeDebuggerItem::new("a", "A").with_priority(TextmateScopeDebuggerPriority::Critical));
        m.add(TextmateScopeDebuggerItem::new("b", "B").with_priority(TextmateScopeDebuggerPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn textmateScopeDebugger_capacity_limit() {
        let mut m = TextmateScopeDebugger::new().with_max_items(2);
        m.add(TextmateScopeDebuggerItem::new("1", "one"));
        m.add(TextmateScopeDebuggerItem::new("2", "two"));
        assert!(!m.add(TextmateScopeDebuggerItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn textmateScopeDebugger_sorted_by_priority() {
        let mut m = TextmateScopeDebugger::new();
        m.add(TextmateScopeDebuggerItem::new("lo", "Low").with_priority(TextmateScopeDebuggerPriority::Low));
        m.add(TextmateScopeDebuggerItem::new("hi", "High").with_priority(TextmateScopeDebuggerPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn textmateScopeDebugger_item_metadata() {
        let mut item = TextmateScopeDebuggerItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn textmateInjectionGrammarResolver_enabled_toggle() {
        let mut s = TextmateInjectionGrammarResolver::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn textmateScopeDebugger_priority_display() {
        assert_eq!(format!("{}", TextmateScopeDebuggerPriority::High), "high");
        assert_eq!(format!("{}", TextmateScopeDebuggerPriority::Low), "low");
    }


    #[test]
    fn wbTextmate_x_config_new() {
        let c = WbTextmateXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn wbTextmate_x_config_builder() {
        let c = WbTextmateXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn wbTextmate_x_config_display() {
        let c = WbTextmateXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn wbTextmate_x_registry_insert_get() {
        let mut reg = WbTextmateXRegistry::new();
        reg.insert(WbTextmateXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn wbTextmate_x_registry_duplicate() {
        let mut reg = WbTextmateXRegistry::new();
        reg.insert(WbTextmateXConfig::new("a")).unwrap();
        assert!(reg.insert(WbTextmateXConfig::new("a")).is_err());
    }

    #[test]
    fn wbTextmate_x_registry_remove() {
        let mut reg = WbTextmateXRegistry::new();
        reg.insert(WbTextmateXConfig::new("a")).unwrap();
        reg.insert(WbTextmateXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn wbTextmate_x_registry_active_entries() {
        let mut reg = WbTextmateXRegistry::new();
        reg.insert(WbTextmateXConfig::new("a")).unwrap();
        reg.insert(WbTextmateXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn wbTextmate_x_registry_by_weight() {
        let mut reg = WbTextmateXRegistry::new();
        reg.insert(WbTextmateXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(WbTextmateXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn wbTextmate_x_registry_tags() {
        let mut reg = WbTextmateXRegistry::new();
        reg.insert(WbTextmateXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(WbTextmateXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn wbTextmate_x_registry_total_weight() {
        let mut reg = WbTextmateXRegistry::new();
        reg.insert(WbTextmateXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(WbTextmateXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn wbTextmate_x_registry_iterator() {
        let mut reg = WbTextmateXRegistry::new();
        reg.insert(WbTextmateXConfig::new("a")).unwrap();
        reg.insert(WbTextmateXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn wbTextmate_x_cache_put_get() {
        let mut cache = WbTextmateXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn wbTextmate_x_cache_eviction() {
        let mut cache = WbTextmateXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn wbTextmate_x_cache_lru_order() {
        let mut cache = WbTextmateXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn wbTextmate_x_cache_most_least_recent() {
        let mut cache = WbTextmateXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn wbTextmate_x_formatter_entry() {
        let e = WbTextmateXConfig::new("k").with_value("v");
        let fmt = WbTextmateXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn wbTextmate_x_formatter_summary() {
        let mut reg = WbTextmateXRegistry::new();
        reg.insert(WbTextmateXConfig::new("a").with_weight(5)).unwrap();
        let fmt = WbTextmateXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn wbTextmate_x_validator_valid() {
        let v = WbTextmateXValidator::new();
        let c = WbTextmateXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn wbTextmate_x_validator_empty_key() {
        let v = WbTextmateXValidator::new();
        let c = WbTextmateXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn wbTextmate_x_validator_require_value() {
        let v = WbTextmateXValidator::new().require_value(true);
        let c = WbTextmateXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn wbTextmate_x_validator_allowed_tags() {
        let v = WbTextmateXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = WbTextmateXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn wbTextmate_x_validator_validate_all() {
        let v = WbTextmateXValidator::new();
        let mut reg = WbTextmateXRegistry::new();
        reg.insert(WbTextmateXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    // xa_ extended tests for wb_textmate
    #[test]
    fn xa_wb_textmate_ring_new() {
        let rb = super::XaWbTextmateRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_wb_textmate_ring_push_len() {
        let mut rb = super::XaWbTextmateRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_wb_textmate_ring_wrap() {
        let mut rb = super::XaWbTextmateRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_wb_textmate_ring_mean_empty() {
        let rb = super::XaWbTextmateRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_wb_textmate_ring_mean_values() {
        let mut rb = super::XaWbTextmateRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_wb_textmate_ring_min_max() {
        let mut rb = super::XaWbTextmateRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_wb_textmate_ring_iter() {
        let mut rb = super::XaWbTextmateRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_wb_textmate_counter_new() {
        let c = super::XaWbTextmateCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_textmate_counter_inc() {
        let mut c = super::XaWbTextmateCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_wb_textmate_counter_inc_by() {
        let mut c = super::XaWbTextmateCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_wb_textmate_counter_reset() {
        let mut c = super::XaWbTextmateCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_wb_textmate_counter_clear() {
        let mut c = super::XaWbTextmateCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_textmate_counter_default() {
        let c = super::XaWbTextmateCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 228 ----

    #[test]
    fn xc_228_pool_new_empty() {
        let pool: super::Xc228Pool<i32> = super::Xc228Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_228_pool_release_acquire() {
        let mut pool = super::Xc228Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_228_pool_acquire_empty() {
        let mut pool: super::Xc228Pool<i32> = super::Xc228Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_228_pool_full() {
        let mut pool = super::Xc228Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_228_pool_drain() {
        let mut pool = super::Xc228Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_228_pool_stats() {
        let mut pool = super::Xc228Pool::new(8);
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
    fn xc_228_pool_clear() {
        let mut pool = super::Xc228Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_228_pool_shrink() {
        let mut pool = super::Xc228Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_228_pool_default() {
        let pool: super::Xc228Pool<String> = super::Xc228Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_228_pool_extend() {
        let mut pool = super::Xc228Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_228_pool_retain() {
        let mut pool = super::Xc228Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_228_scheduler_round_robin() {
        let mut sched = super::Xc228Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_228_scheduler_empty() {
        let mut sched = super::Xc228Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_228_scheduler_reset() {
        let mut sched = super::Xc228Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_228_scheduler_add_remove() {
        let mut sched = super::Xc228Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_228_scheduler_targets() {
        let sched = super::Xc228Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_228_hash_empty() {
        assert_eq!(super::xc_228_hash(b""), 5381);
    }

    #[test]
    fn xc_228_hash_data() {
        let h = super::xc_228_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_228_hash(b"hello"), h);
    }

    #[test]
    fn xc_228_reverse_str() {
        assert_eq!(super::xc_228_reverse("abc"), "cba");
        assert_eq!(super::xc_228_reverse(""), "");
    }


    // --- xd_30 deepening tests ---

    #[test]
    fn xd_30_sm_initial_state() {
        let sm = Xd30StateMachine::new();
        assert_eq!(sm.current_state(), Xd30State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_30_sm_valid_idle_to_running() {
        let mut sm = Xd30StateMachine::new();
        assert!(sm.transition(Xd30State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd30State::Running);
    }

    #[test]
    fn xd_30_sm_valid_running_to_paused() {
        let mut sm = Xd30StateMachine::new();
        sm.transition(Xd30State::Running).unwrap();
        assert!(sm.transition(Xd30State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd30State::Paused);
    }

    #[test]
    fn xd_30_sm_valid_running_to_done() {
        let mut sm = Xd30StateMachine::new();
        sm.transition(Xd30State::Running).unwrap();
        assert!(sm.transition(Xd30State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd30State::Done);
    }

    #[test]
    fn xd_30_sm_valid_paused_to_running() {
        let mut sm = Xd30StateMachine::new();
        sm.transition(Xd30State::Running).unwrap();
        sm.transition(Xd30State::Paused).unwrap();
        assert!(sm.transition(Xd30State::Running).is_ok());
    }

    #[test]
    fn xd_30_sm_valid_done_to_idle() {
        let mut sm = Xd30StateMachine::new();
        sm.transition(Xd30State::Running).unwrap();
        sm.transition(Xd30State::Done).unwrap();
        assert!(sm.transition(Xd30State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd30State::Idle);
    }

    #[test]
    fn xd_30_sm_invalid_idle_to_done() {
        let mut sm = Xd30StateMachine::new();
        assert!(sm.transition(Xd30State::Done).is_err());
    }

    #[test]
    fn xd_30_sm_invalid_idle_to_paused() {
        let mut sm = Xd30StateMachine::new();
        assert!(sm.transition(Xd30State::Paused).is_err());
    }

    #[test]
    fn xd_30_sm_history_tracking() {
        let mut sm = Xd30StateMachine::new();
        sm.transition(Xd30State::Running).unwrap();
        sm.transition(Xd30State::Paused).unwrap();
        sm.transition(Xd30State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd30State::Idle);
        assert_eq!(sm.history()[0].to, Xd30State::Running);
        assert_eq!(sm.history()[1].from, Xd30State::Running);
        assert_eq!(sm.history()[2].to, Xd30State::Done);
    }

    #[test]
    fn xd_30_sm_serialize_deserialize() {
        let mut sm = Xd30StateMachine::new();
        sm.transition(Xd30State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd30StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd30State::Running));
    }

    #[test]
    fn xd_30_sm_deserialize_invalid() {
        assert_eq!(Xd30StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_30_sm_reset() {
        let mut sm = Xd30StateMachine::new();
        sm.transition(Xd30State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd30State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_30_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd30EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd30Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_30_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd30EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd30Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd30Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_30_bus_unsubscribe() {
        let mut bus = Xd30EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_30_event_kind_and_payload() {
        let e = Xd30Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd30Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_30_bus_clear_history() {
        let mut bus = Xd30EventBus::new();
        bus.publish(Xd30Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_30_sm_step_counter_increments() {
        let mut sm = Xd30StateMachine::new();
        sm.transition(Xd30State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd30State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #28 --

    #[test]
    fn xf28_trie_insert_search() {
        let mut t = Xf28Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf28_trie_starts_with() {
        let mut t = Xf28Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf28_trie_remove() {
        let mut t = Xf28Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf28_trie_word_count() {
        let mut t = Xf28Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf28_trie_longest_prefix() {
        let mut t = Xf28Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf28_trie_all_words() {
        let mut t = Xf28Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf28_trie_autocomplete() {
        let mut t = Xf28Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf28_trie_empty_search() {
        let t = Xf28Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf28_bloom_add_contains() {
        let mut bf = Xf28BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf28_bloom_probably_absent() {
        let bf = Xf28BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf28_bloom_false_positive_rate() {
        let mut bf = Xf28BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf28_bloom_clear() {
        let mut bf = Xf28BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf28_bloom_union() {
        let mut a = Xf28BloomFilter::xf_new(512, 2);
        let mut b = Xf28BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf28_bloom_intersection_estimate() {
        let mut a = Xf28BloomFilter::xf_new(512, 2);
        let mut b = Xf28BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf28_bloom_union_size_mismatch() {
        let a = Xf28BloomFilter::xf_new(256, 2);
        let b = Xf28BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh227_skip_insert_contains() {
        let mut sl = super::Xh227SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh227_skip_remove() {
        let mut sl = super::Xh227SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh227_skip_len() {
        let mut sl = super::Xh227SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh227_skip_range_query() {
        let mut sl = super::Xh227SkipList::xh_new(4);
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
    fn xh227_skip_floor_ceiling() {
        let mut sl = super::Xh227SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh227_skip_rank() {
        let mut sl = super::Xh227SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh227_skip_empty() {
        let sl = super::Xh227SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh227_skip_duplicates() {
        let mut sl = super::Xh227SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh227_bitset_set_test() {
        let mut bs = super::Xh227BitSet::xh_new(256);
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
    fn xh227_bitset_clear_count() {
        let mut bs = super::Xh227BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh227_bitset_and_or_xor() {
        let mut a = super::Xh227BitSet::xh_new(128);
        let mut b = super::Xh227BitSet::xh_new(128);
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
    fn xh227_bitset_iter_ones() {
        let mut bs = super::Xh227BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh227_bitset_first_last() {
        let mut bs = super::Xh227BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh227_bitset_empty() {
        let bs = super::Xh227BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi227_deque_push_pop_back() {
        let mut dq = super::Xi227Deque::xi_new(4);
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
    fn xi227_deque_push_pop_front() {
        let mut dq = super::Xi227Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi227_deque_mixed_ops() {
        let mut dq = super::Xi227Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi227_deque_get_and_split() {
        let mut dq = super::Xi227Deque::xi_new(8);
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
    fn xi227_deque_rotate_left() {
        let mut dq = super::Xi227Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi227_deque_rotate_right() {
        let mut dq = super::Xi227Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi227_deque_grow() {
        let mut dq = super::Xi227Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi227_deque_empty() {
        let dq = super::Xi227Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi227_interval_tree_insert_query() {
        let mut tree = super::Xi227IntervalTree::xi_new();
        tree.xi_insert(super::Xi227Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi227Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi227Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi227_interval_tree_overlap() {
        let mut tree = super::Xi227IntervalTree::xi_new();
        tree.xi_insert(super::Xi227Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi227Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi227Interval::xi_new(12, 20));
        let q = super::Xi227Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi227_interval_tree_remove() {
        let mut tree = super::Xi227IntervalTree::xi_new();
        tree.xi_insert(super::Xi227Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi227Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi227_interval_tree_gaps() {
        let mut tree = super::Xi227IntervalTree::xi_new();
        tree.xi_insert(super::Xi227Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi227Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi227Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi227Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi227Interval::xi_new(8, 10));
    }

    #[test]
    fn xi227_interval_tree_merge() {
        let mut tree = super::Xi227IntervalTree::xi_new();
        tree.xi_insert(super::Xi227Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi227Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi227Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi227Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi227Interval::xi_new(10, 15));
    }

    #[test]
    fn xi227_interval_tree_all() {
        let mut tree = super::Xi227IntervalTree::xi_new();
        tree.xi_insert(super::Xi227Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi227Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi227_interval_tree_empty() {
        let tree = super::Xi227IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi227_interval_tree_contains_point() {
        let iv = super::Xi227Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 227) ---

    #[test]
    fn xj_227_uf_make_and_find() {
        let mut uf = super::Xj227UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_227_uf_union_connected() {
        let mut uf = super::Xj227UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_227_uf_component_count() {
        let mut uf = super::Xj227UnionFind::xj_new();
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
    fn xj_227_uf_component_size() {
        let mut uf = super::Xj227UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_227_uf_largest_component() {
        let mut uf = super::Xj227UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_227_uf_many_elements() {
        let mut uf = super::Xj227UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_227_uf_separate_components() {
        let mut uf = super::Xj227UnionFind::xj_new();
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
    fn xj_227_uf_path_compression() {
        let mut uf = super::Xj227UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_227_bt_insert_get() {
        let mut bt = super::Xj227BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_227_bt_contains_len() {
        let mut bt = super::Xj227BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_227_bt_replace() {
        let mut bt = super::Xj227BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_227_bt_remove() {
        let mut bt = super::Xj227BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_227_bt_keys_values() {
        let mut bt = super::Xj227BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_227_bt_range() {
        let mut bt = super::Xj227BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_227_bt_min_max() {
        let mut bt = super::Xj227BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_227_bt_many_inserts() {
        let mut bt = super::Xj227BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_227 segment tree tests ---

    #[test]
    fn xk_227_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk227SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_227_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk227SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_227_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk227SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_227_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk227SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_227_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk227SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_227_st_single_element() {
        let data = vec![42];
        let st = super::Xk227SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_227_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk227SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_227_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk227SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_227 disjoint intervals tests ---

    #[test]
    fn xk_227_di_add_and_count() {
        let mut di = super::Xk227DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_227_di_merge_overlap() {
        let mut di = super::Xk227DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_227_di_contains() {
        let mut di = super::Xk227DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_227_di_remove() {
        let mut di = super::Xk227DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_227_di_covered_length() {
        let mut di = super::Xk227DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_227_di_gaps() {
        let mut di = super::Xk227DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_227_di_merge_adjacent() {
        let mut di = super::Xk227DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_227_di_empty() {
        let di = super::Xk227DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_227_rope_new_empty() {
        let rope = super::Xl227Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_227_rope_from_str() {
        let rope = super::Xl227Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_227_rope_insert_at() {
        let mut rope = super::Xl227Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_227_rope_delete_range() {
        let mut rope = super::Xl227Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_227_rope_char_at() {
        let rope = super::Xl227Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_227_rope_split_concat() {
        let rope = super::Xl227Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_227_rope_line_count() {
        let rope = super::Xl227Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_227_rope_line_at() {
        let rope = super::Xl227Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_227_sa_build_and_search() {
        let sa = super::Xl227SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_227_sa_count() {
        let sa = super::Xl227SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_227_sa_longest_repeated() {
        let sa = super::Xl227SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_227_sa_all_positions() {
        let sa = super::Xl227SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_227_sa_len() {
        let sa = super::Xl227SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_227_sa_empty() {
        let sa = super::Xl227SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_227_rope_slice() {
        let rope = super::Xl227Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_227_sa_search_start() {
        let sa = super::Xl227SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }
}
