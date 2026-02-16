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
}
