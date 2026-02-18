//! Find and replace functionality.
//!
//! Equivalent to VS Code's `vs/editor/contrib/find`.
//! Provides text search with regex, case sensitivity, whole word, and replace.

use std::collections::HashMap;
use std::fmt;

use regex::Regex;

/// Errors that can occur during find/replace operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindError {
    /// The regex pattern is invalid.
    InvalidRegex(String),
    /// The search string is empty when it shouldn't be.
    EmptySearch,
    /// The match index is out of bounds.
    MatchOutOfBounds { index: usize, total: usize },
}

impl fmt::Display for FindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FindError::InvalidRegex(msg) => write!(f, "invalid regex pattern: {}", msg),
            FindError::EmptySearch => write!(f, "search string is empty"),
            FindError::MatchOutOfBounds { index, total } => {
                write!(f, "match index {} out of bounds (total: {})", index, total)
            }
        }
    }
}

impl std::error::Error for FindError {}

/// Options for a find operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindOptions {
    pub search_string: String,
    pub is_regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub preserve_case: bool,
}

impl FindOptions {
    pub fn new(search_string: impl Into<String>) -> Self {
        Self {
            search_string: search_string.into(),
            is_regex: false,
            case_sensitive: false,
            whole_word: false,
            preserve_case: false,
        }
    }

    pub fn with_case_sensitive(mut self, v: bool) -> Self {
        self.case_sensitive = v;
        self
    }

    pub fn with_regex(mut self, v: bool) -> Self {
        self.is_regex = v;
        self
    }

    pub fn with_whole_word(mut self, v: bool) -> Self {
        self.whole_word = v;
        self
    }
}

impl fmt::Display for FindOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"{}\"", self.search_string)?;
        let mut flags = Vec::new();
        if self.is_regex {
            flags.push("regex");
        }
        if self.case_sensitive {
            flags.push("case-sensitive");
        }
        if self.whole_word {
            flags.push("whole-word");
        }
        if self.preserve_case {
            flags.push("preserve-case");
        }
        if !flags.is_empty() {
            write!(f, " [{}]", flags.join(", "))?;
        }
        Ok(())
    }
}

impl FindOptions {
    /// Validate that the search options are well-formed.
    /// Returns `Err` if the search string is empty or an invalid regex.
    pub fn validate(&self) -> Result<(), FindError> {
        if self.search_string.is_empty() {
            return Err(FindError::EmptySearch);
        }
        if self.is_regex {
            Regex::new(&self.search_string).map_err(|e| FindError::InvalidRegex(e.to_string()))?;
        }
        Ok(())
    }

    /// Builder method to set preserve_case.
    pub fn with_preserve_case(mut self, v: bool) -> Self {
        self.preserve_case = v;
        self
    }

    /// Build the compiled regex pattern for this search configuration.
    pub fn compile_pattern(&self) -> Result<Regex, FindError> {
        if self.search_string.is_empty() {
            return Err(FindError::EmptySearch);
        }

        let pattern = if self.is_regex {
            self.search_string.clone()
        } else {
            regex::escape(&self.search_string)
        };

        let pattern = if self.whole_word {
            format!(r"\b{}\b", pattern)
        } else {
            pattern
        };

        let pattern = if self.case_sensitive {
            pattern
        } else {
            format!("(?i){}", pattern)
        };

        Regex::new(&pattern).map_err(|e| FindError::InvalidRegex(e.to_string()))
    }
}

/// A match found in the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindMatch {
    pub line: u32,     // 1-based
    pub start_col: u32, // 1-based
    pub end_col: u32,   // 1-based, exclusive
    pub text: String,
}

impl fmt::Display for FindMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\"{}\" at line {}:{}-{}",
            self.text, self.line, self.start_col, self.end_col
        )
    }
}

impl FindMatch {
    /// Length of the matched text in columns.
    pub fn match_len(&self) -> u32 {
        self.end_col - self.start_col
    }

    /// Returns true if this match is on the given line.
    pub fn is_on_line(&self, line: u32) -> bool {
        self.line == line
    }
}

/// Find all matches, returning an error for invalid patterns.
pub fn find_matches_checked(text: &str, options: &FindOptions) -> Result<Vec<FindMatch>, FindError> {
    if options.search_string.is_empty() {
        return Err(FindError::EmptySearch);
    }

    let re = options.compile_pattern()?;
    let mut matches = Vec::new();
    for (line_idx, line) in text.lines().enumerate() {
        for m in re.find_iter(line) {
            matches.push(FindMatch {
                line: (line_idx + 1) as u32,
                start_col: (m.start() + 1) as u32,
                end_col: (m.end() + 1) as u32,
                text: m.as_str().to_string(),
            });
        }
    }
    Ok(matches)
}

/// Replace only the nth occurrence (0-based) in the text.
pub fn replace_nth(text: &str, options: &FindOptions, replacement: &str, n: usize) -> Result<String, FindError> {
    let re = options.compile_pattern()?;
    let mut count = 0usize;
    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;

    for m in re.find_iter(text) {
        if count == n {
            result.push_str(&text[last_end..m.start()]);
            result.push_str(replacement);
            last_end = m.end();
            // Append the rest and return
            result.push_str(&text[last_end..]);
            return Ok(result);
        }
        count += 1;
    }

    Err(FindError::MatchOutOfBounds {
        index: n,
        total: count,
    })
}

/// Count all occurrences of the search pattern in text.
pub fn count_matches(text: &str, options: &FindOptions) -> usize {
    find_matches(text, options).len()
}

/// Find all matches in text.
pub fn find_matches(text: &str, options: &FindOptions) -> Vec<FindMatch> {
    if options.search_string.is_empty() {
        return Vec::new();
    }

    let pattern = if options.is_regex {
        options.search_string.clone()
    } else {
        regex::escape(&options.search_string)
    };

    let pattern = if options.whole_word {
        format!(r"\b{}\b", pattern)
    } else {
        pattern
    };

    let re = if options.case_sensitive {
        Regex::new(&pattern)
    } else {
        Regex::new(&format!("(?i){}", pattern))
    };

    let re = match re {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut matches = Vec::new();
    for (line_idx, line) in text.lines().enumerate() {
        for m in re.find_iter(line) {
            matches.push(FindMatch {
                line: (line_idx + 1) as u32,
                start_col: (m.start() + 1) as u32,
                end_col: (m.end() + 1) as u32,
                text: m.as_str().to_string(),
            });
        }
    }

    matches
}

/// Replace all matches in text.
pub fn replace_all(text: &str, options: &FindOptions, replacement: &str) -> String {
    if options.search_string.is_empty() {
        return text.to_string();
    }

    let pattern = if options.is_regex {
        options.search_string.clone()
    } else {
        regex::escape(&options.search_string)
    };

    let pattern = if options.whole_word {
        format!(r"\b{}\b", pattern)
    } else {
        pattern
    };

    let re = if options.case_sensitive {
        Regex::new(&pattern)
    } else {
        Regex::new(&format!("(?i){}", pattern))
    };

    match re {
        Ok(r) => r.replace_all(text, replacement).to_string(),
        Err(_) => text.to_string(),
    }
}

/// Find state for incremental search.
#[derive(Debug, Clone, PartialEq)]
pub struct FindState {
    pub options: FindOptions,
    pub matches: Vec<FindMatch>,
    pub current_match: Option<usize>,
    pub replace_string: String,
}

impl FindState {
    pub fn new() -> Self {
        Self {
            options: FindOptions::new(""),
            matches: Vec::new(),
            current_match: None,
            replace_string: String::new(),
        }
    }

    /// Update the search and recompute matches.
    pub fn search(&mut self, text: &str) {
        self.matches = find_matches(text, &self.options);
        if self.matches.is_empty() {
            self.current_match = None;
        } else {
            self.current_match = Some(0);
        }
    }

    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    pub fn next_match(&mut self) {
        if let Some(idx) = self.current_match {
            if !self.matches.is_empty() {
                self.current_match = Some((idx + 1) % self.matches.len());
            }
        }
    }

    pub fn previous_match(&mut self) {
        if let Some(idx) = self.current_match {
            if !self.matches.is_empty() {
                self.current_match = Some(if idx == 0 {
                    self.matches.len() - 1
                } else {
                    idx - 1
                });
            }
        }
    }

    pub fn current(&self) -> Option<&FindMatch> {
        self.current_match.and_then(|i| self.matches.get(i))
    }

    /// Jump to a specific match by index.
    pub fn goto_match(&mut self, index: usize) -> Result<(), FindError> {
        if index >= self.matches.len() {
            return Err(FindError::MatchOutOfBounds {
                index,
                total: self.matches.len(),
            });
        }
        self.current_match = Some(index);
        Ok(())
    }

    /// Returns true if the state has an active search with results.
    pub fn has_matches(&self) -> bool {
        !self.matches.is_empty()
    }

    /// Return all matches on a given line (1-based).
    pub fn matches_on_line(&self, line: u32) -> Vec<&FindMatch> {
        self.matches.iter().filter(|m| m.is_on_line(line)).collect()
    }

    /// Replace the current match in the provided text, returning the new text.
    pub fn replace_current(&self, text: &str) -> Result<String, FindError> {
        let idx = self.current_match.ok_or(FindError::MatchOutOfBounds {
            index: 0,
            total: self.matches.len(),
        })?;
        replace_nth(text, &self.options, &self.replace_string, idx)
    }
}

impl fmt::Display for FindState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let current = self
            .current_match
            .map(|i| i + 1) // display as 1-based
            .unwrap_or(0);
        write!(
            f,
            "Find {}: {}/{} matches",
            self.options,
            current,
            self.matches.len()
        )
    }
}

impl Default for FindState {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for find operations.
#[derive(Debug, Clone, PartialEq)]
pub struct FindStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl FindStats {
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
    pub fn merge(&mut self, other: &FindStats) {
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

impl Default for FindStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for FindStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FindStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for find.
#[derive(Debug, Clone)]
pub struct FindValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl FindValidator {
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

impl Default for FindValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// FindReplaceState
// ---------------------------------------------------------------------------

/// Tracks the complete state of the find/replace UI widget.
pub struct FindReplaceState {
    pub is_visible: bool,
    pub is_replace_visible: bool,
    pub search_string: String,
    pub replace_string: String,
    pub use_regex: bool,
    pub match_case: bool,
    pub match_whole_word: bool,
    pub preserve_case: bool,
    pub is_in_selection: bool,
    history: Vec<String>,
    max_history: usize,
}

impl FindReplaceState {
    /// Create a new default find/replace state.
    pub fn new() -> Self {
        Self {
            is_visible: false,
            is_replace_visible: false,
            search_string: String::new(),
            replace_string: String::new(),
            use_regex: false,
            match_case: false,
            match_whole_word: false,
            preserve_case: false,
            is_in_selection: false,
            history: Vec::new(),
            max_history: 100,
        }
    }

    /// Toggle the visibility of the find widget.
    pub fn toggle_visibility(&mut self) {
        self.is_visible = !self.is_visible;
    }

    /// Toggle the visibility of the replace input.
    pub fn toggle_replace(&mut self) {
        self.is_replace_visible = !self.is_replace_visible;
    }

    /// Set the search string, pushing it to history if it differs from the last entry.
    pub fn set_search(&mut self, s: impl Into<String>) {
        let s = s.into();
        if self.history.last().map_or(true, |last| *last != s) && !s.is_empty() {
            self.history.push(s.clone());
            if self.history.len() > self.max_history {
                self.history.remove(0);
            }
        }
        self.search_string = s;
    }

    /// Set the replace string.
    pub fn set_replace(&mut self, s: impl Into<String>) {
        self.replace_string = s.into();
    }

    /// Toggle the regex option.
    pub fn toggle_regex(&mut self) {
        self.use_regex = !self.use_regex;
    }

    /// Toggle the case-sensitive option.
    pub fn toggle_case(&mut self) {
        self.match_case = !self.match_case;
    }

    /// Toggle the whole-word option.
    pub fn toggle_whole_word(&mut self) {
        self.match_whole_word = !self.match_whole_word;
    }

    /// Convert the current state to `FindOptions`.
    pub fn to_find_options(&self) -> FindOptions {
        FindOptions {
            search_string: self.search_string.clone(),
            is_regex: self.use_regex,
            case_sensitive: self.match_case,
            whole_word: self.match_whole_word,
            preserve_case: self.preserve_case,
        }
    }

    /// Return the search history.
    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// Clear the search history.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Return the previous search string (the item before the current search in history).
    pub fn previous_search(&self) -> Option<&str> {
        if self.history.len() >= 2 {
            Some(&self.history[self.history.len() - 2])
        } else {
            None
        }
    }
}

impl Default for FindReplaceState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// find_all_matches (with context)
// ---------------------------------------------------------------------------

/// A match with surrounding context lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindMatchWithContext {
    pub find_match: FindMatch,
    pub context_before: String,
    pub context_after: String,
}

/// Find all matches and include context from surrounding text.
///
/// For each match, extracts up to `context_chars` characters before and after the
/// match text on the same line.
pub fn find_all_matches(
    text: &str,
    options: &FindOptions,
    context_chars: usize,
) -> Vec<FindMatchWithContext> {
    let matches = find_matches(text, options);
    let lines: Vec<&str> = text.lines().collect();

    matches
        .into_iter()
        .map(|m| {
            let line_idx = (m.line - 1) as usize;
            let line = lines.get(line_idx).copied().unwrap_or("");

            // start_col and end_col are 1-based; convert to 0-based byte offsets
            let start = (m.start_col - 1) as usize;
            let end = (m.end_col - 1) as usize;

            let before_start = start.saturating_sub(context_chars);
            let after_end = (end + context_chars).min(line.len());

            let context_before = line[before_start..start].to_string();
            let context_after = line[end..after_end].to_string();

            FindMatchWithContext {
                find_match: m,
                context_before,
                context_after,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// FindHighlightDecoration
// ---------------------------------------------------------------------------

/// The kind of highlight decoration applied to a find match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindHighlightKind {
    /// A regular (non-current) match highlight.
    Match,
    /// The currently-focused match.
    CurrentMatch,
    /// A preview of what the replacement text would look like.
    ReplacePreview,
}

/// A decoration range for highlighting a find match in the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindHighlightDecoration {
    pub line: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub is_current: bool,
    pub kind: FindHighlightKind,
}

impl FindHighlightDecoration {
    /// Create a new highlight decoration.
    pub fn new(line: u32, start_col: u32, end_col: u32, kind: FindHighlightKind) -> Self {
        let is_current = kind == FindHighlightKind::CurrentMatch;
        Self {
            line,
            start_col,
            end_col,
            is_current,
            kind,
        }
    }

    /// Returns true if the given position is within this decoration.
    pub fn contains_position(&self, line: u32, col: u32) -> bool {
        self.line == line && col >= self.start_col && col < self.end_col
    }

    /// Returns the width (number of columns) of this decoration.
    pub fn width(&self) -> u32 {
        self.end_col - self.start_col
    }
}

/// Generate highlight decorations for all matches, marking the current one.
pub fn generate_highlight_decorations(
    matches: &[FindMatch],
    current_index: Option<usize>,
) -> Vec<FindHighlightDecoration> {
    matches
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let is_current = current_index == Some(i);
            let kind = if is_current {
                FindHighlightKind::CurrentMatch
            } else {
                FindHighlightKind::Match
            };
            FindHighlightDecoration::new(m.line, m.start_col, m.end_col, kind)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Context extraction with surrounding lines
// ---------------------------------------------------------------------------

/// A match with surrounding lines of context (like grep -C).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchLineContext {
    pub find_match: FindMatch,
    /// Lines before the match line (may be fewer than requested at start of file).
    pub lines_before: Vec<String>,
    /// The full line containing the match.
    pub match_line: String,
    /// Lines after the match line (may be fewer than requested at end of file).
    pub lines_after: Vec<String>,
}

/// Find all matches and return each with `context_lines` surrounding lines.
pub fn find_with_line_context(
    text: &str,
    options: &FindOptions,
    context_lines: usize,
) -> Vec<MatchLineContext> {
    let all_lines: Vec<&str> = text.lines().collect();
    let matches = find_matches(text, options);

    matches
        .into_iter()
        .map(|m| {
            let line_idx = (m.line - 1) as usize;
            let start = line_idx.saturating_sub(context_lines);
            let end = (line_idx + context_lines + 1).min(all_lines.len());

            let lines_before = all_lines[start..line_idx]
                .iter()
                .map(|s| s.to_string())
                .collect();
            let match_line = all_lines
                .get(line_idx)
                .map(|s| s.to_string())
                .unwrap_or_default();
            let lines_after = if line_idx + 1 < all_lines.len() {
                all_lines[(line_idx + 1)..end]
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            } else {
                Vec::new()
            };

            MatchLineContext {
                find_match: m,
                lines_before,
                match_line,
                lines_after,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Replace preview generation
// ---------------------------------------------------------------------------

/// A before/after pair showing what a single replacement would look like.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacePreview {
    pub line: u32,
    pub before: String,
    pub after: String,
}

/// Generate a preview for every match showing the original and replaced line.
pub fn generate_replace_previews(
    text: &str,
    options: &FindOptions,
    replacement: &str,
) -> Vec<ReplacePreview> {
    let re = match options.compile_pattern() {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let lines: Vec<&str> = text.lines().collect();
    let matches = find_matches(text, options);

    matches
        .iter()
        .map(|m| {
            let line_idx = (m.line - 1) as usize;
            let original = lines.get(line_idx).copied().unwrap_or("");
            // Replace only the specific occurrence within this line by position.
            let start = (m.start_col - 1) as usize;
            let end = (m.end_col - 1) as usize;
            let after = format!("{}{}{}", &original[..start], replacement, &original[end..]);
            ReplacePreview {
                line: m.line,
                before: original.to_string(),
                after,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Case-preserving replace
// ---------------------------------------------------------------------------

/// Apply the case pattern of `original` to `replacement`.
///
/// Rules:
/// - If `original` is all uppercase, return `replacement` uppercased.
/// - If `original` is all lowercase, return `replacement` lowercased.
/// - If `original` starts with an uppercase letter and the rest is lowercase
///   (title case), return `replacement` title-cased.
/// - Otherwise return `replacement` unchanged.
fn apply_case_pattern(original: &str, replacement: &str) -> String {
    if original.is_empty() || replacement.is_empty() {
        return replacement.to_string();
    }
    let all_upper = original.chars().all(|c| !c.is_alphabetic() || c.is_uppercase());
    let all_lower = original.chars().all(|c| !c.is_alphabetic() || c.is_lowercase());

    if all_upper && original.chars().any(|c| c.is_alphabetic()) {
        return replacement.to_uppercase();
    }
    if all_lower {
        return replacement.to_lowercase();
    }

    // Title case: first char uppercase, rest lowercase.
    let mut chars = original.chars();
    let first_upper = chars.next().map_or(false, |c| c.is_uppercase());
    let rest_lower = chars.all(|c| !c.is_alphabetic() || c.is_lowercase());
    if first_upper && rest_lower {
        let mut result = String::with_capacity(replacement.len());
        for (i, ch) in replacement.chars().enumerate() {
            if i == 0 {
                for u in ch.to_uppercase() {
                    result.push(u);
                }
            } else {
                for l in ch.to_lowercase() {
                    result.push(l);
                }
            }
        }
        return result;
    }

    replacement.to_string()
}

/// Replace all matches using case-preserving logic: the replacement adopts the
/// case pattern of each individual match.
pub fn replace_all_preserve_case(
    text: &str,
    options: &FindOptions,
    replacement: &str,
) -> String {
    let re = match options.compile_pattern() {
        Ok(r) => r,
        Err(_) => return text.to_string(),
    };

    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;
    for m in re.find_iter(text) {
        result.push_str(&text[last_end..m.start()]);
        result.push_str(&apply_case_pattern(m.as_str(), replacement));
        last_end = m.end();
    }
    result.push_str(&text[last_end..]);
    result
}

// ---------------------------------------------------------------------------
// Multi-file find result aggregation
// ---------------------------------------------------------------------------

/// A single match within a named file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMatch {
    pub file: String,
    pub find_match: FindMatch,
}

/// Results from searching across multiple files, grouped by file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiFileResults {
    pub file_matches: Vec<FileMatch>,
}

impl MultiFileResults {
    /// Build multi-file results by searching each `(filename, content)` pair.
    pub fn search(files: &[(&str, &str)], options: &FindOptions) -> Self {
        let mut file_matches = Vec::new();
        for &(name, content) in files {
            for m in find_matches(content, options) {
                file_matches.push(FileMatch {
                    file: name.to_string(),
                    find_match: m,
                });
            }
        }
        Self { file_matches }
    }

    /// Total number of matches across all files.
    pub fn total_matches(&self) -> usize {
        self.file_matches.len()
    }

    /// Number of files that contain at least one match.
    pub fn file_count(&self) -> usize {
        let mut seen = std::collections::HashSet::new();
        for fm in &self.file_matches {
            seen.insert(&fm.file);
        }
        seen.len()
    }

    /// Return matches grouped by file, preserving encounter order.
    pub fn grouped_by_file(&self) -> Vec<(&str, Vec<&FindMatch>)> {
        let mut groups: Vec<(&str, Vec<&FindMatch>)> = Vec::new();
        for fm in &self.file_matches {
            if let Some(group) = groups.iter_mut().find(|(f, _)| *f == fm.file.as_str()) {
                group.1.push(&fm.find_match);
            } else {
                groups.push((fm.file.as_str(), vec![&fm.find_match]));
            }
        }
        groups
    }
}

// ---------------------------------------------------------------------------
// Find history
// ---------------------------------------------------------------------------

/// A bounded, deduplicated search history.
#[derive(Debug, Clone)]
pub struct FindHistory {
    entries: Vec<String>,
    capacity: usize,
}

impl FindHistory {
    /// Create a new history with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity: capacity.max(1),
        }
    }

    /// Push a search term. If already present it is moved to the front.
    /// Empty strings are ignored.
    pub fn push(&mut self, term: impl Into<String>) {
        let term = term.into();
        if term.is_empty() {
            return;
        }
        // Remove existing duplicate.
        self.entries.retain(|e| *e != term);
        self.entries.insert(0, term);
        if self.entries.len() > self.capacity {
            self.entries.truncate(self.capacity);
        }
    }

    /// Most recent entry.
    pub fn most_recent(&self) -> Option<&str> {
        self.entries.first().map(|s| s.as_str())
    }

    /// Return entries that contain `substring` (case-insensitive).
    pub fn search(&self, substring: &str) -> Vec<&str> {
        let lower = substring.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.to_lowercase().contains(&lower))
            .map(|s| s.as_str())
            .collect()
    }

    /// All entries from most recent to oldest.
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// Number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// -- FindInSelection for scoped search ---------------------------------------

/// A text range for selection-scoped search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRange {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

impl TextRange {
    pub fn new(start_line: usize, start_col: usize, end_line: usize, end_col: usize) -> Self {
        Self { start_line, start_col, end_line, end_col }
    }

    pub fn contains_line(&self, line: usize) -> bool {
        line >= self.start_line && line <= self.end_line
    }

    pub fn line_count(&self) -> usize {
        if self.end_line >= self.start_line {
            self.end_line - self.start_line + 1
        } else {
            0
        }
    }
}

impl fmt::Display for TextRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}:{}-{}:{}]", self.start_line, self.start_col, self.end_line, self.end_col)
    }
}

/// Find matches only within a selection range.
pub fn find_in_selection(text: &str, pattern: &str, selection: &TextRange) -> Vec<(usize, usize)> {
    let mut results = Vec::new();
    for (line_idx, line) in text.lines().enumerate() {
        if !selection.contains_line(line_idx) {
            continue;
        }
        let mut start = 0;
        while let Some(pos) = line[start..].find(pattern) {
            let abs_pos = start + pos;
            // Check column bounds for first/last lines
            if line_idx == selection.start_line && abs_pos < selection.start_col {
                start = abs_pos + 1;
                continue;
            }
            if line_idx == selection.end_line && abs_pos + pattern.len() > selection.end_col {
                break;
            }
            results.push((line_idx, abs_pos));
            start = abs_pos + pattern.len().max(1);
        }
    }
    results
}

// -- FindPreserveCase for smart replacement ----------------------------------

/// Perform a case-preserving replacement.
pub fn preserve_case_replace(original: &str, replacement: &str) -> String {
    if original.is_empty() || replacement.is_empty() {
        return replacement.to_string();
    }

    // All upper
    if original.chars().all(|c| c.is_uppercase() || !c.is_alphabetic()) {
        return replacement.to_uppercase();
    }

    // All lower
    if original.chars().all(|c| c.is_lowercase() || !c.is_alphabetic()) {
        return replacement.to_lowercase();
    }

    // Title case (first char upper, rest lower)
    if original.chars().next().is_some_and(|c| c.is_uppercase())
        && original.chars().skip(1).all(|c| c.is_lowercase() || !c.is_alphabetic())
    {
        let mut chars = replacement.chars();
        if let Some(first) = chars.next() {
            let rest: String = chars.collect();
            return format!("{}{}", first.to_uppercase(), rest.to_lowercase());
        }
    }

    replacement.to_string()
}

// -- FindRegexGroupReplace with capture references ---------------------------

/// Replace using regex with capture group references ($1, $2, etc.).
pub fn regex_group_replace(text: &str, pattern: &str, replacement: &str) -> Result<String, FindError> {
    let re = Regex::new(pattern).map_err(|e| FindError::InvalidRegex(e.to_string()))?;
    Ok(re.replace_all(text, replacement).into_owned())
}

/// Count regex matches in text.
pub fn regex_match_count(text: &str, pattern: &str) -> Result<usize, FindError> {
    let re = Regex::new(pattern).map_err(|e| FindError::InvalidRegex(e.to_string()))?;
    Ok(re.find_iter(text).count())
}

// -- Find result count badge -------------------------------------------------

/// Format a match count as a badge string.
pub fn format_match_badge(count: usize, has_more: bool) -> String {
    if count == 0 {
        return "No results".to_string();
    }
    if has_more {
        format!("{count}+ results")
    } else if count == 1 {
        "1 result".to_string()
    } else {
        format!("{count} results")
    }
}

/// Match position with line and column information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchPosition {
    pub line: usize,
    pub column: usize,
    pub length: usize,
}

impl fmt::Display for MatchPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{} (len {})", self.line + 1, self.column + 1, self.length)
    }
}

/// Find all match positions in text.
pub fn find_all_positions(text: &str, pattern: &str, case_sensitive: bool) -> Vec<MatchPosition> {
    let mut positions = Vec::new();
    let search_text;
    let search_pattern;

    if case_sensitive {
        search_text = text.to_string();
        search_pattern = pattern.to_string();
    } else {
        search_text = text.to_lowercase();
        search_pattern = pattern.to_lowercase();
    }

    for (line_idx, line) in search_text.lines().enumerate() {
        let mut start = 0;
        while let Some(pos) = line[start..].find(&search_pattern) {
            positions.push(MatchPosition {
                line: line_idx,
                column: start + pos,
                length: pattern.len(),
            });
            start += pos + pattern.len().max(1);
        }
    }
    positions
}

/// Navigate to the Nth match (0-based), wrapping around.
pub fn navigate_match(positions: &[MatchPosition], current_index: usize) -> Option<&MatchPosition> {
    if positions.is_empty() {
        return None;
    }
    Some(&positions[current_index % positions.len()])
}


// ---------------------------------------------------------------------------
// FindHighlightAll
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FindHighlightAll {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl FindHighlightAll {
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

impl Default for FindHighlightAll {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for FindHighlightAll {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "FindHighlightAll({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// FindHistoryPersistence
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FindHistoryPersistence {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl FindHistoryPersistence {
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

impl Default for FindHistoryPersistence {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for FindHistoryPersistence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "FindHistoryPersistence({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// FindHighlightAllSnapshot — point-in-time snapshot of FindHighlightAll state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FindHighlightAllSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl FindHighlightAllSnapshot {
    pub fn capture(source: &FindHighlightAll, timestamp: u64) -> Self {
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

impl fmt::Display for FindHighlightAllSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// FindHistoryPersistenceStats — aggregate statistics for FindHistoryPersistence
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct FindHistoryPersistenceStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl FindHistoryPersistenceStats {
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

impl fmt::Display for FindHistoryPersistenceStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// FindHighlightAllConfig — configuration for FindHighlightAll
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FindHighlightAllConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl FindHighlightAllConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for FindHighlightAllConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for FindHighlightAllConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ── FindHighlighter ─────────────────────────────────────────────────────

/// Computes and manages highlight ranges for find matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightRange {
    pub start: usize,
    pub end: usize,
}

impl HighlightRange {
    pub fn new(start: usize, end: usize) -> Self { Self { start, end } }
    pub fn len(&self) -> usize { self.end.saturating_sub(self.start) }
    pub fn is_empty(&self) -> bool { self.start >= self.end }
    pub fn overlaps(&self, other: &HighlightRange) -> bool {
        self.start < other.end && other.start < self.end
    }
}

pub struct FindHighlighter;

impl FindHighlighter {
    /// Compute highlight ranges from a list of (start, length) match positions.
    pub fn compute_ranges(matches: &[(usize, usize)]) -> Vec<HighlightRange> {
        matches.iter().map(|&(s, l)| HighlightRange::new(s, s + l)).collect()
    }

    /// Merge overlapping ranges into non-overlapping sorted ranges.
    pub fn merge_overlapping_ranges(ranges: &mut Vec<HighlightRange>) {
        if ranges.len() <= 1 { return; }
        ranges.sort_by_key(|r| r.start);
        let mut merged: Vec<HighlightRange> = Vec::new();
        for r in ranges.drain(..) {
            if let Some(last) = merged.last_mut() {
                if r.start <= last.end {
                    last.end = last.end.max(r.end);
                    continue;
                }
            }
            merged.push(r);
        }
        *ranges = merged;
    }

    /// Filter ranges to only those visible within [view_start, view_end).
    pub fn visible_range_filter(ranges: &[HighlightRange], view_start: usize, view_end: usize) -> Vec<&HighlightRange> {
        ranges.iter().filter(|r| r.end > view_start && r.start < view_end).collect()
    }
}

// ── SearchHistory ───────────────────────────────────────────────────────

/// Maintains a history of search terms with deduplication and size limits.
#[derive(Debug, Clone)]
pub struct SearchHistory {
    terms: Vec<String>,
    max_size: usize,
}

impl SearchHistory {
    pub fn new(max_size: usize) -> Self { Self { terms: Vec::new(), max_size: max_size.max(1) } }

    pub fn push_term(&mut self, term: &str) {
        // Remove existing duplicate first
        self.terms.retain(|t| t != term);
        self.terms.insert(0, term.to_string());
        if self.terms.len() > self.max_size { self.terms.truncate(self.max_size); }
    }

    pub fn recent_terms(&self, n: usize) -> &[String] {
        let end = n.min(self.terms.len());
        &self.terms[..end]
    }

    pub fn remove_term(&mut self, term: &str) -> bool {
        if let Some(pos) = self.terms.iter().position(|t| t == term) {
            self.terms.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) { self.terms.clear(); }

    pub fn contains(&self, term: &str) -> bool { self.terms.iter().any(|t| t == term) }
    pub fn len(&self) -> usize { self.terms.len() }
    pub fn is_empty(&self) -> bool { self.terms.is_empty() }
    pub fn max_history_size(&self) -> usize { self.max_size }

    /// Remove duplicates (should already be deduped, but defensive).
    pub fn deduplicate(&mut self) {
        let mut seen = Vec::new();
        self.terms.retain(|t| {
            if seen.contains(t) { false } else { seen.push(t.clone()); true }
        });
    }
}

// ── ReplacePreview ──────────────────────────────────────────────────────

/// Computes a preview of replacements without modifying the text.
#[derive(Debug, Clone)]
pub struct ReplacePreviewLine {
    pub line_number: usize,
    pub original_line: String,
    pub replaced_line: String,
    pub change_count: usize,
}

impl ReplacePreviewLine {
    pub fn has_changes(&self) -> bool { self.change_count > 0 }
}

pub struct ReplacePreviewBuilder;

impl ReplacePreviewBuilder {
    /// Compute preview of replacing `search` with `replacement` in each line.
    pub fn compute(text: &str, search: &str, replacement: &str) -> Vec<ReplacePreviewLine> {
        if search.is_empty() { return Vec::new(); }
        text.lines().enumerate().filter_map(|(i, line)| {
            let count = line.matches(search).count();
            if count > 0 {
                Some(ReplacePreviewLine {
                    line_number: i + 1,
                    original_line: line.to_string(),
                    replaced_line: line.replace(search, replacement),
                    change_count: count,
                })
            } else {
                None
            }
        }).collect()
    }

    /// Total number of replacements across all preview lines.
    pub fn total_changes(previews: &[ReplacePreviewLine]) -> usize {
        previews.iter().map(|p| p.change_count).sum()
    }

    /// Check if any lines have changes.
    pub fn has_any_changes(previews: &[ReplacePreviewLine]) -> bool {
        previews.iter().any(|p| p.has_changes())
    }
}


/// Find and replace configuration manager.
#[derive(Debug, Clone)]
pub struct FindConfig {
    entries: Vec<FindEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single find and replace entry.
#[derive(Debug, Clone, PartialEq)]
pub struct FindEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl FindEntry {
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

impl FindConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: FindEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&FindEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut FindEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&FindEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&FindEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&FindEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<FindEntry> {
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
// xa_ extended helpers for find
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaFindRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaFindRingBuf {
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
pub struct XaFindCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaFindCounter {
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

impl Default for XaFindCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 82
// ---------------------------------------------------------------------------

/// Generic object pool `Xc82Pool<T>`.
pub struct Xc82Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc82Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc82PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc82Pool<T> {
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
    pub fn stats(&self) -> Xc82PoolStats {
        Xc82PoolStats {
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

impl<T> Default for Xc82Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc82Scheduler`.
pub struct Xc82Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc82Scheduler {
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

impl Default for Xc82Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_82 hash for the given byte slice.
pub fn xc_82_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_82 convention.
pub fn xc_82_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_67 deepening: state machine + event bus ---

/// States for the Xd67 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd67State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd67State {
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
pub struct Xd67Transition {
    pub from: Xd67State,
    pub to: Xd67State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd67StateMachine {
    current: Xd67State,
    history: Vec<Xd67Transition>,
    step_counter: usize,
}

impl Xd67StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd67State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd67State {
        self.current
    }

    pub fn history(&self) -> &[Xd67Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd67State) -> Result<Xd67State, String> {
        let allowed = match (self.current, target) {
            (Xd67State::Idle, Xd67State::Running) => true,
            (Xd67State::Running, Xd67State::Paused) => true,
            (Xd67State::Running, Xd67State::Done) => true,
            (Xd67State::Paused, Xd67State::Running) => true,
            (Xd67State::Paused, Xd67State::Done) => true,
            (Xd67State::Done, Xd67State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_67: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd67Transition {
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
            "Xd67SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd67State> {
        let prefix = "Xd67SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd67State::Idle),
            "Running" => Some(Xd67State::Running),
            "Paused" => Some(Xd67State::Paused),
            "Done" => Some(Xd67State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd67State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd67 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd67Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd67Event {
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

type Xd67HandlerFn = Box<dyn Fn(&Xd67Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd67EventBus {
    handlers: Vec<(usize, Option<String>, Xd67HandlerFn)>,
    next_id: usize,
    published: Vec<Xd67Event>,
}

impl Xd67EventBus {
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
        F: Fn(&Xd67Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd67Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd67Event) {
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

    pub fn published_events(&self) -> &[Xd67Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #76
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf76Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf76TrieNode {
    children: std::collections::HashMap<char, Xf76TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf76Trie {
    root: Xf76TrieNode,
    count: usize,
}

impl Xf76Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf76TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf76TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf76TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf76BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf76BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 81).
pub struct Xh81SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh81SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 123 as u64,
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

/// A compact bit set supporting boolean operations (variant 81).
pub struct Xh81BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh81BitSet {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_simple() {
        let matches = find_matches("hello world\nhello rust", &FindOptions::new("hello"));
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line, 1);
        assert_eq!(matches[0].start_col, 1);
        assert_eq!(matches[1].line, 2);
    }

    #[test]
    fn find_case_insensitive() {
        let opts = FindOptions::new("hello").with_case_sensitive(false);
        let matches = find_matches("Hello HELLO hello", &opts);
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn find_case_sensitive() {
        let opts = FindOptions::new("Hello").with_case_sensitive(true);
        let matches = find_matches("Hello hello HELLO", &opts);
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn find_regex() {
        let opts = FindOptions::new(r"\d+").with_regex(true);
        let matches = find_matches("abc 123 def 456", &opts);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].text, "123");
    }

    #[test]
    fn find_whole_word() {
        let opts = FindOptions::new("he").with_whole_word(true);
        let matches = find_matches("he hello the he", &opts);
        assert_eq!(matches.len(), 2); // "he" at start and end
    }

    #[test]
    fn find_no_match() {
        let matches = find_matches("hello world", &FindOptions::new("xyz"));
        assert!(matches.is_empty());
    }

    #[test]
    fn replace() {
        let result = replace_all("hello world", &FindOptions::new("world"), "rust");
        assert_eq!(result, "hello rust");
    }

    #[test]
    fn find_state_navigation() {
        let mut state = FindState::new();
        state.options = FindOptions::new("a");
        state.search("a b a c a");
        assert_eq!(state.match_count(), 3);
        assert_eq!(state.current(), Some(&state.matches[0]));

        state.next_match();
        assert_eq!(state.current(), Some(&state.matches[1]));

        state.next_match();
        state.next_match(); // wraps around
        assert_eq!(state.current(), Some(&state.matches[0]));

        state.previous_match(); // wraps back
        assert_eq!(state.current(), Some(&state.matches[2]));
    }

    #[test]
    fn empty_search() {
        let matches = find_matches("hello", &FindOptions::new(""));
        assert!(matches.is_empty());
    }

    #[test]
    fn find_options_validate_ok() {
        let opts = FindOptions::new("hello");
        assert!(opts.validate().is_ok());
    }

    #[test]
    fn find_options_validate_empty() {
        let opts = FindOptions::new("");
        assert_eq!(opts.validate(), Err(FindError::EmptySearch));
    }

    #[test]
    fn find_options_validate_bad_regex() {
        let opts = FindOptions::new("[invalid").with_regex(true);
        assert!(matches!(opts.validate(), Err(FindError::InvalidRegex(_))));
    }

    #[test]
    fn find_options_display() {
        let opts = FindOptions::new("foo").with_regex(true).with_case_sensitive(true);
        let s = format!("{}", opts);
        assert!(s.contains("foo"));
        assert!(s.contains("regex"));
        assert!(s.contains("case-sensitive"));
    }

    #[test]
    fn find_match_display_and_len() {
        let m = FindMatch {
            line: 3,
            start_col: 5,
            end_col: 10,
            text: "world".to_string(),
        };
        assert_eq!(m.match_len(), 5);
        assert!(m.is_on_line(3));
        assert!(!m.is_on_line(1));
        let s = format!("{}", m);
        assert!(s.contains("world"));
        assert!(s.contains("line 3"));
    }

    #[test]
    fn find_matches_checked_error() {
        let opts = FindOptions::new("[bad").with_regex(true);
        let result = find_matches_checked("some text", &opts);
        assert!(result.is_err());
    }

    #[test]
    fn find_matches_checked_ok() {
        let opts = FindOptions::new("ok");
        let result = find_matches_checked("ok then ok", &opts);
        assert_eq!(result.unwrap().len(), 2);
    }

    #[test]
    fn replace_nth_match() {
        let opts = FindOptions::new("a");
        let result = replace_nth("a b a c a", &opts, "X", 1).unwrap();
        assert_eq!(result, "a b X c a");
    }

    #[test]
    fn replace_nth_out_of_bounds() {
        let opts = FindOptions::new("a");
        let result = replace_nth("a b a", &opts, "X", 5);
        assert!(matches!(result, Err(FindError::MatchOutOfBounds { .. })));
    }

    #[test]
    fn count_matches_basic() {
        let opts = FindOptions::new("ab");
        assert_eq!(count_matches("ab cd ab ef ab", &opts), 3);
    }

    #[test]
    fn find_state_goto_and_display() {
        let mut state = FindState::new();
        state.options = FindOptions::new("x");
        state.search("x y x z x");
        assert_eq!(state.match_count(), 3);

        assert!(state.goto_match(2).is_ok());
        assert_eq!(state.current().unwrap().start_col, 9);

        assert!(state.goto_match(5).is_err());

        let s = format!("{}", state);
        assert!(s.contains("3/3 matches"));
    }

    #[test]
    fn find_state_matches_on_line() {
        let mut state = FindState::new();
        state.options = FindOptions::new("a");
        state.search("a b a\nc d\na");
        assert_eq!(state.matches_on_line(1).len(), 2);
        assert_eq!(state.matches_on_line(2).len(), 0);
        assert_eq!(state.matches_on_line(3).len(), 1);
    }

    #[test]
    fn find_state_replace_current() {
        let mut state = FindState::new();
        state.options = FindOptions::new("cat");
        state.replace_string = "dog".to_string();
        let text = "the cat sat on the cat mat";
        state.search(text);
        state.next_match(); // move to second "cat"
        let result = state.replace_current(text).unwrap();
        assert_eq!(result, "the cat sat on the dog mat");
    }

    #[test]
    fn find_state_has_matches() {
        let mut state = FindState::new();
        state.options = FindOptions::new("z");
        state.search("abc");
        assert!(!state.has_matches());
        state.search("xyz");
        assert!(state.has_matches());
    }

    #[test]
    fn find_error_display() {
        let e = FindError::EmptySearch;
        assert_eq!(format!("{}", e), "search string is empty");
        let e = FindError::MatchOutOfBounds { index: 3, total: 2 };
        assert!(format!("{}", e).contains("3"));
    }

    #[test]
    fn find_options_with_preserve_case() {
        let opts = FindOptions::new("test").with_preserve_case(true);
        assert!(opts.preserve_case);
        let s = format!("{}", opts);
        assert!(s.contains("preserve-case"));
    }

    #[test]
    fn replace_case_insensitive() {
        let opts = FindOptions::new("HELLO").with_case_sensitive(false);
        let result = replace_all("hello Hello HELLO", &opts, "hi");
        assert_eq!(result, "hi hi hi");
    }

    #[test]
    fn find_stats_new_defaults() {
        let stats = FindStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn find_stats_record_success() {
        let mut stats = FindStats::new();
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
    fn find_stats_record_failure() {
        let mut stats = FindStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn find_stats_reset() {
        let mut stats = FindStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn find_stats_merge() {
        let mut a = FindStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = FindStats::new();
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
    fn find_stats_display() {
        let mut stats = FindStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn find_stats_default() {
        let stats = FindStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn find_validator_accepts_valid_name() {
        let v = FindValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn find_validator_rejects_empty() {
        let v = FindValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn find_validator_rejects_too_long() {
        let v = FindValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn find_validator_forbidden_prefix() {
        let v = FindValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn find_validator_allowed_chars() {
        let v = FindValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn find_validator_range() {
        let v = FindValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn find_sanitize_removes_control() {
        let result = FindValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn find_truncate_short_string() {
        assert_eq!(FindValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn find_truncate_long_string() {
        let result = FindValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn find_is_ascii_printable() {
        assert!(FindValidator::is_ascii_printable("Hello World 123"));
        assert!(!FindValidator::is_ascii_printable("Hello\x00World"));
    }

    // -----------------------------------------------------------------------
    // FindReplaceState tests
    // -----------------------------------------------------------------------

    #[test]
    fn find_replace_state_new_defaults() {
        let state = FindReplaceState::new();
        assert!(!state.is_visible);
        assert!(!state.is_replace_visible);
        assert!(state.search_string.is_empty());
        assert!(state.replace_string.is_empty());
        assert!(!state.use_regex);
        assert!(!state.match_case);
        assert!(!state.match_whole_word);
        assert!(!state.preserve_case);
        assert!(!state.is_in_selection);
        assert!(state.history().is_empty());
    }

    #[test]
    fn find_replace_state_toggle_visibility() {
        let mut state = FindReplaceState::new();
        assert!(!state.is_visible);
        state.toggle_visibility();
        assert!(state.is_visible);
        state.toggle_visibility();
        assert!(!state.is_visible);
    }

    #[test]
    fn find_replace_state_toggle_replace() {
        let mut state = FindReplaceState::new();
        assert!(!state.is_replace_visible);
        state.toggle_replace();
        assert!(state.is_replace_visible);
        state.toggle_replace();
        assert!(!state.is_replace_visible);
    }

    #[test]
    fn find_replace_state_set_search_pushes_history() {
        let mut state = FindReplaceState::new();
        state.set_search("hello");
        assert_eq!(state.search_string, "hello");
        assert_eq!(state.history(), &["hello"]);

        state.set_search("world");
        assert_eq!(state.search_string, "world");
        assert_eq!(state.history(), &["hello", "world"]);
    }

    #[test]
    fn find_replace_state_set_search_no_duplicate() {
        let mut state = FindReplaceState::new();
        state.set_search("hello");
        state.set_search("hello");
        assert_eq!(state.history().len(), 1);
    }

    #[test]
    fn find_replace_state_set_search_empty_not_added() {
        let mut state = FindReplaceState::new();
        state.set_search("");
        assert!(state.history().is_empty());
    }

    #[test]
    fn find_replace_state_toggle_options() {
        let mut state = FindReplaceState::new();
        state.toggle_regex();
        assert!(state.use_regex);
        state.toggle_case();
        assert!(state.match_case);
        state.toggle_whole_word();
        assert!(state.match_whole_word);
        // Toggle back
        state.toggle_regex();
        assert!(!state.use_regex);
    }

    #[test]
    fn find_replace_state_to_find_options() {
        let mut state = FindReplaceState::new();
        state.set_search("test");
        state.toggle_regex();
        state.toggle_case();
        state.toggle_whole_word();
        state.preserve_case = true;

        let opts = state.to_find_options();
        assert_eq!(opts.search_string, "test");
        assert!(opts.is_regex);
        assert!(opts.case_sensitive);
        assert!(opts.whole_word);
        assert!(opts.preserve_case);
    }

    #[test]
    fn find_replace_state_previous_search() {
        let mut state = FindReplaceState::new();
        assert!(state.previous_search().is_none());

        state.set_search("first");
        assert!(state.previous_search().is_none());

        state.set_search("second");
        assert_eq!(state.previous_search(), Some("first"));

        state.set_search("third");
        assert_eq!(state.previous_search(), Some("second"));
    }

    #[test]
    fn find_replace_state_clear_history() {
        let mut state = FindReplaceState::new();
        state.set_search("a");
        state.set_search("b");
        assert_eq!(state.history().len(), 2);
        state.clear_history();
        assert!(state.history().is_empty());
    }

    #[test]
    fn find_replace_state_set_replace() {
        let mut state = FindReplaceState::new();
        state.set_replace("replacement");
        assert_eq!(state.replace_string, "replacement");
    }

    // -----------------------------------------------------------------------
    // find_all_matches tests
    // -----------------------------------------------------------------------

    #[test]
    fn find_all_matches_basic() {
        let text = "hello world hello rust";
        let opts = FindOptions::new("hello");
        let results = find_all_matches(text, &opts, 5);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].find_match.text, "hello");
        assert_eq!(results[0].context_before, "");
        assert_eq!(results[0].context_after, " worl");
    }

    #[test]
    fn find_all_matches_context_before() {
        let text = "say hello there";
        let opts = FindOptions::new("hello");
        let results = find_all_matches(text, &opts, 4);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].context_before, "say ");
        assert_eq!(results[0].context_after, " the");
    }

    #[test]
    fn find_all_matches_context_clipped_at_line_boundary() {
        let text = "hi";
        let opts = FindOptions::new("hi");
        let results = find_all_matches(text, &opts, 100);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].context_before, "");
        assert_eq!(results[0].context_after, "");
    }

    #[test]
    fn find_all_matches_multiline() {
        let text = "first hello\nsecond hello";
        let opts = FindOptions::new("hello");
        let results = find_all_matches(text, &opts, 3);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].find_match.line, 1);
        assert_eq!(results[0].context_before, "st ");
        assert_eq!(results[1].find_match.line, 2);
        assert_eq!(results[1].context_before, "nd ");
    }

    #[test]
    fn find_all_matches_zero_context() {
        let text = "find me here";
        let opts = FindOptions::new("me");
        let results = find_all_matches(text, &opts, 0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].context_before, "");
        assert_eq!(results[0].context_after, "");
    }

    #[test]
    fn find_all_matches_regex() {
        let text = "abc 123 def 456";
        let opts = FindOptions::new(r"\d+").with_regex(true);
        let results = find_all_matches(text, &opts, 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].find_match.text, "123");
        assert_eq!(results[0].context_before, "c ");
        assert_eq!(results[0].context_after, " d");
    }

    #[test]
    fn find_all_matches_no_match() {
        let text = "nothing here";
        let opts = FindOptions::new("xyz");
        let results = find_all_matches(text, &opts, 5);
        assert!(results.is_empty());
    }

    #[test]
    fn find_all_matches_at_end_of_line() {
        let text = "the end";
        let opts = FindOptions::new("end");
        let results = find_all_matches(text, &opts, 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].context_before, "the ");
        assert_eq!(results[0].context_after, "");
    }

    // -----------------------------------------------------------------------
    // FindHighlightDecoration tests
    // -----------------------------------------------------------------------

    #[test]
    fn highlight_decoration_new() {
        let d = FindHighlightDecoration::new(1, 5, 10, FindHighlightKind::Match);
        assert_eq!(d.line, 1);
        assert_eq!(d.start_col, 5);
        assert_eq!(d.end_col, 10);
        assert!(!d.is_current);
        assert_eq!(d.kind, FindHighlightKind::Match);
    }

    #[test]
    fn highlight_decoration_current_match() {
        let d = FindHighlightDecoration::new(1, 1, 5, FindHighlightKind::CurrentMatch);
        assert!(d.is_current);
        assert_eq!(d.kind, FindHighlightKind::CurrentMatch);
    }

    #[test]
    fn highlight_decoration_contains_position() {
        let d = FindHighlightDecoration::new(2, 5, 10, FindHighlightKind::Match);
        assert!(d.contains_position(2, 5));
        assert!(d.contains_position(2, 9));
        assert!(!d.contains_position(2, 10)); // exclusive
        assert!(!d.contains_position(1, 5)); // wrong line
        assert!(!d.contains_position(2, 4)); // before start
    }

    #[test]
    fn highlight_decoration_width() {
        let d = FindHighlightDecoration::new(1, 3, 8, FindHighlightKind::Match);
        assert_eq!(d.width(), 5);
    }

    #[test]
    fn highlight_decoration_replace_preview_kind() {
        let d = FindHighlightDecoration::new(1, 1, 4, FindHighlightKind::ReplacePreview);
        assert!(!d.is_current);
        assert_eq!(d.kind, FindHighlightKind::ReplacePreview);
    }

    #[test]
    fn generate_highlight_decorations_basic() {
        let matches = find_matches("hello world hello", &FindOptions::new("hello"));
        let decorations = generate_highlight_decorations(&matches, Some(0));
        assert_eq!(decorations.len(), 2);
        assert!(decorations[0].is_current);
        assert_eq!(decorations[0].kind, FindHighlightKind::CurrentMatch);
        assert!(!decorations[1].is_current);
        assert_eq!(decorations[1].kind, FindHighlightKind::Match);
    }

    #[test]
    fn generate_highlight_decorations_no_current() {
        let matches = find_matches("abc abc", &FindOptions::new("abc"));
        let decorations = generate_highlight_decorations(&matches, None);
        assert_eq!(decorations.len(), 2);
        assert!(decorations.iter().all(|d| !d.is_current));
        assert!(decorations.iter().all(|d| d.kind == FindHighlightKind::Match));
    }

    #[test]
    fn generate_highlight_decorations_second_current() {
        let matches = find_matches("ab ab ab", &FindOptions::new("ab"));
        let decorations = generate_highlight_decorations(&matches, Some(1));
        assert_eq!(decorations.len(), 3);
        assert!(!decorations[0].is_current);
        assert!(decorations[1].is_current);
        assert!(!decorations[2].is_current);
    }

    // -----------------------------------------------------------------------
    // find_with_line_context tests
    // -----------------------------------------------------------------------

    #[test]
    fn find_with_line_context_basic() {
        let text = "line1\nline2 match\nline3\nline4";
        let opts = FindOptions::new("match");
        let results = find_with_line_context(text, &opts, 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].lines_before, vec!["line1"]);
        assert_eq!(results[0].match_line, "line2 match");
        assert_eq!(results[0].lines_after, vec!["line3"]);
    }

    #[test]
    fn find_with_line_context_at_start() {
        let text = "match here\nline2\nline3";
        let opts = FindOptions::new("match");
        let results = find_with_line_context(text, &opts, 2);
        assert_eq!(results.len(), 1);
        assert!(results[0].lines_before.is_empty());
        assert_eq!(results[0].lines_after, vec!["line2", "line3"]);
    }

    #[test]
    fn find_with_line_context_at_end() {
        let text = "line1\nline2\nmatch here";
        let opts = FindOptions::new("match");
        let results = find_with_line_context(text, &opts, 2);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].lines_before, vec!["line1", "line2"]);
        assert!(results[0].lines_after.is_empty());
    }

    // -----------------------------------------------------------------------
    // replace preview tests
    // -----------------------------------------------------------------------

    #[test]
    fn replace_preview_basic() {
        let text = "hello world\ngoodbye world";
        let opts = FindOptions::new("world");
        let previews = generate_replace_previews(text, &opts, "rust");
        assert_eq!(previews.len(), 2);
        assert_eq!(previews[0].before, "hello world");
        assert_eq!(previews[0].after, "hello rust");
        assert_eq!(previews[1].before, "goodbye world");
        assert_eq!(previews[1].after, "goodbye rust");
    }

    // -----------------------------------------------------------------------
    // case-preserving replace tests
    // -----------------------------------------------------------------------

    #[test]
    fn case_preserve_all_upper() {
        let opts = FindOptions::new("HELLO").with_case_sensitive(false);
        let result = replace_all_preserve_case("say HELLO there", &opts, "goodbye");
        assert_eq!(result, "say GOODBYE there");
    }

    #[test]
    fn case_preserve_title_case() {
        let opts = FindOptions::new("Hello").with_case_sensitive(false);
        let result = replace_all_preserve_case("Hello hello HELLO", &opts, "goodbye");
        assert_eq!(result, "Goodbye goodbye GOODBYE");
    }

    #[test]
    fn case_preserve_all_lower() {
        let opts = FindOptions::new("hello").with_case_sensitive(false);
        let result = replace_all_preserve_case("hello", &opts, "GOODBYE");
        assert_eq!(result, "goodbye");
    }

    // -----------------------------------------------------------------------
    // multi-file find tests
    // -----------------------------------------------------------------------

    #[test]
    fn multi_file_search_basic() {
        let files = vec![
            ("a.rs", "fn hello() {}"),
            ("b.rs", "let x = hello();"),
            ("c.rs", "no match here"),
        ];
        let opts = FindOptions::new("hello");
        let results = MultiFileResults::search(&files, &opts);
        assert_eq!(results.total_matches(), 2);
        assert_eq!(results.file_count(), 2);

        let groups = results.grouped_by_file();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, "a.rs");
        assert_eq!(groups[1].0, "b.rs");
    }

    // -----------------------------------------------------------------------
    // find history tests
    // -----------------------------------------------------------------------

    #[test]
    fn find_history_push_and_dedup() {
        let mut h = FindHistory::new(5);
        h.push("alpha");
        h.push("beta");
        h.push("alpha"); // duplicate moves to front
        assert_eq!(h.len(), 2);
        assert_eq!(h.most_recent(), Some("alpha"));
        assert_eq!(h.entries(), &["alpha", "beta"]);
    }

    #[test]
    fn find_history_capacity() {
        let mut h = FindHistory::new(3);
        h.push("a");
        h.push("b");
        h.push("c");
        h.push("d"); // evicts oldest ("a")
        assert_eq!(h.len(), 3);
        assert_eq!(h.entries(), &["d", "c", "b"]);
    }

    #[test]
    fn find_history_search() {
        let mut h = FindHistory::new(10);
        h.push("findOptions");
        h.push("replace_all");
        h.push("find_matches");
        let results = h.search("find");
        assert_eq!(results.len(), 2);
        assert!(results.contains(&"find_matches"));
        assert!(results.contains(&"findOptions"));
    }

    #[test]
    fn find_history_empty_ignored() {
        let mut h = FindHistory::new(5);
        h.push("");
        assert!(h.is_empty());
    }

    // -- FindInSelection tests ------------------------------------------------

    #[test]
    fn find_in_selection_basic() {
        let text = "hello world\nfoo bar\nhello again";
        let sel = TextRange::new(0, 0, 2, 30);
        let matches = find_in_selection(text, "hello", &sel);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn find_in_selection_restricts_lines() {
        let text = "hello\nhello\nhello";
        let sel = TextRange::new(1, 0, 1, 10);
        let matches = find_in_selection(text, "hello", &sel);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, 1);
    }

    #[test]
    fn text_range_contains_line() {
        let range = TextRange::new(5, 0, 10, 0);
        assert!(range.contains_line(5));
        assert!(range.contains_line(10));
        assert!(!range.contains_line(11));
    }

    #[test]
    fn text_range_display() {
        let range = TextRange::new(1, 5, 3, 10);
        assert_eq!(range.to_string(), "[1:5-3:10]");
    }

    // -- FindPreserveCase tests -----------------------------------------------

    #[test]
    fn preserve_case_all_upper() {
        assert_eq!(preserve_case_replace("HELLO", "world"), "WORLD");
    }

    #[test]
    fn preserve_case_all_lower() {
        assert_eq!(preserve_case_replace("hello", "World"), "world");
    }

    #[test]
    fn preserve_case_title_case() {
        assert_eq!(preserve_case_replace("Hello", "world"), "World");
    }

    #[test]
    fn preserve_case_mixed_returns_as_is() {
        assert_eq!(preserve_case_replace("hElLo", "world"), "world");
    }

    // -- RegexGroupReplace tests ----------------------------------------------

    #[test]
    fn regex_group_replace_basic() {
        let result = regex_group_replace("hello world", r"(\w+) (\w+)", "$2 $1").unwrap();
        assert_eq!(result, "world hello");
    }

    #[test]
    fn regex_group_replace_invalid_pattern() {
        assert!(regex_group_replace("text", r"[invalid", "x").is_err());
    }

    #[test]
    fn regex_match_count_basic() {
        let count = regex_match_count("aaa bbb aaa", r"aaa").unwrap();
        assert_eq!(count, 2);
    }

    // -- Badge tests ----------------------------------------------------------

    #[test]
    fn format_match_badge_variants() {
        assert_eq!(format_match_badge(0, false), "No results");
        assert_eq!(format_match_badge(1, false), "1 result");
        assert_eq!(format_match_badge(5, false), "5 results");
        assert_eq!(format_match_badge(100, true), "100+ results");
    }

    // -- find_all_positions tests ---------------------------------------------

    #[test]
    fn find_all_positions_case_sensitive() {
        let positions = find_all_positions("Hello hello HELLO", "hello", true);
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].column, 6);
    }

    #[test]
    fn find_all_positions_case_insensitive() {
        let positions = find_all_positions("Hello hello HELLO", "hello", false);
        assert_eq!(positions.len(), 3);
    }

    #[test]
    fn match_position_display() {
        let pos = MatchPosition { line: 0, column: 5, length: 3 };
        assert_eq!(pos.to_string(), "1:6 (len 3)");
    }

    #[test]
    fn navigate_match_wraps() {
        let positions = vec![
            MatchPosition { line: 0, column: 0, length: 3 },
            MatchPosition { line: 1, column: 5, length: 3 },
        ];
        let m = navigate_match(&positions, 3).unwrap();
        assert_eq!(m.line, 1);
    }

    #[test] fn findHighlightAll_new() { let s = FindHighlightAll::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn findHighlightAll_add() { let mut s = FindHighlightAll::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn findHighlightAll_remove() { let mut s = FindHighlightAll::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn findHighlightAll_config() { let mut s = FindHighlightAll::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn findHighlightAll_nav() { let mut s = FindHighlightAll::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn findHighlightAll_filter() { let mut s = FindHighlightAll::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn findHighlightAll_display() { assert!(format!("{}", FindHighlightAll::new()).contains("FindHighlightAll")); }
    #[test] fn findHistoryPersistence_new() { let s = FindHistoryPersistence::new(); assert!(s.is_empty()); }
    #[test] fn findHistoryPersistence_add() { let mut s = FindHistoryPersistence::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn findHistoryPersistence_active() { let mut s = FindHistoryPersistence::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn findHistoryPersistence_error() { let mut s = FindHistoryPersistence::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn findHistoryPersistence_rm_group() { let mut s = FindHistoryPersistence::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn findHistoryPersistence_display() { assert!(format!("{}", FindHistoryPersistence::new()).contains("FindHistoryPersistence")); }


    #[test] fn findHighlightAll_snap_capture() {
        let s = FindHighlightAll::new();
        let snap = FindHighlightAllSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn findHighlightAll_snap_stale() {
        let s = FindHighlightAll::new();
        let snap = FindHighlightAllSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn findHighlightAll_snap_diff() {
        let s = FindHighlightAll::new();
        let s1v = FindHighlightAllSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn findHighlightAll_snap_display() {
        let s = FindHighlightAll::new();
        let snap = FindHighlightAllSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn findHistoryPersistence_stats_record() {
        let mut st = FindHistoryPersistenceStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn findHistoryPersistence_stats_hit_ratio() {
        let mut st = FindHistoryPersistenceStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn findHistoryPersistence_stats_merge() {
        let mut a = FindHistoryPersistenceStats::new();
        a.total_adds = 5;
        let mut b = FindHistoryPersistenceStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn findHistoryPersistence_stats_display() {
        let st = FindHistoryPersistenceStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn findHighlightAll_config_default() {
        let c = FindHighlightAllConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn findHighlightAll_config_builder() {
        let c = FindHighlightAllConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn findHighlightAll_config_labels() {
        let mut c = FindHighlightAllConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn findHighlightAll_config_cleanup_threshold() {
        let c = FindHighlightAllConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn findHighlightAll_config_display() {
        assert!(format!("{}", FindHighlightAllConfig::new()).contains("Config"));
    }
    #[test] fn findHistoryPersistence_stats_peaks() {
        let mut st = FindHistoryPersistenceStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // ── FindHighlighter tests ──

    #[test]
    fn highlight_compute_ranges() {
        let ranges = FindHighlighter::compute_ranges(&[(0, 5), (10, 3)]);
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].start, 0);
        assert_eq!(ranges[0].end, 5);
    }

    #[test]
    fn highlight_merge_overlapping() {
        let mut ranges = vec![
            HighlightRange::new(0, 5),
            HighlightRange::new(3, 8),
            HighlightRange::new(10, 15),
        ];
        FindHighlighter::merge_overlapping_ranges(&mut ranges);
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].end, 8);
    }

    #[test]
    fn highlight_merge_no_overlap() {
        let mut ranges = vec![HighlightRange::new(0, 3), HighlightRange::new(5, 8)];
        FindHighlighter::merge_overlapping_ranges(&mut ranges);
        assert_eq!(ranges.len(), 2);
    }

    #[test]
    fn highlight_visible_filter() {
        let ranges = vec![HighlightRange::new(0, 5), HighlightRange::new(10, 15), HighlightRange::new(20, 25)];
        let visible = FindHighlighter::visible_range_filter(&ranges, 8, 18);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].start, 10);
    }

    #[test]
    fn highlight_range_overlaps() {
        let a = HighlightRange::new(0, 5);
        let b = HighlightRange::new(3, 8);
        assert!(a.overlaps(&b));
        let c = HighlightRange::new(5, 10);
        assert!(!a.overlaps(&c));
    }

    // ── SearchHistory tests ──

    #[test]
    fn history_push_and_recent() {
        let mut h = SearchHistory::new(5);
        h.push_term("hello");
        h.push_term("world");
        assert_eq!(h.recent_terms(2), &["world", "hello"]);
    }

    #[test]
    fn history_dedup_on_push() {
        let mut h = SearchHistory::new(5);
        h.push_term("hello");
        h.push_term("world");
        h.push_term("hello");
        assert_eq!(h.len(), 2);
        assert_eq!(h.recent_terms(1), &["hello"]);
    }

    #[test]
    fn history_max_size() {
        let mut h = SearchHistory::new(2);
        h.push_term("a");
        h.push_term("b");
        h.push_term("c");
        assert_eq!(h.len(), 2);
        assert!(!h.contains("a"));
    }

    #[test]
    fn history_remove_and_clear() {
        let mut h = SearchHistory::new(5);
        h.push_term("hello");
        assert!(h.remove_term("hello"));
        assert!(!h.remove_term("hello"));
        h.push_term("a");
        h.clear();
        assert!(h.is_empty());
    }

    // ── ReplacePreview tests ──

    #[test]
    fn replace_preview_builder_basic() {
        let text = "hello world\nhello rust\ngoodbye";
        let previews = ReplacePreviewBuilder::compute(text, "hello", "hi");
        assert_eq!(previews.len(), 2);
        assert_eq!(previews[0].replaced_line, "hi world");
        assert_eq!(previews[0].change_count, 1);
    }

    #[test]
    fn replace_preview_multiple_per_line() {
        let text = "aa bb aa";
        let previews = ReplacePreviewBuilder::compute(text, "aa", "cc");
        assert_eq!(previews.len(), 1);
        assert_eq!(previews[0].change_count, 2);
        assert_eq!(previews[0].replaced_line, "cc bb cc");
    }

    #[test]
    fn replace_preview_no_match() {
        let previews = ReplacePreviewBuilder::compute("hello", "xyz", "abc");
        assert!(previews.is_empty());
        assert!(!ReplacePreviewBuilder::has_any_changes(&previews));
    }

    #[test]
    fn replace_preview_total_changes() {
        let text = "a a\na";
        let previews = ReplacePreviewBuilder::compute(text, "a", "b");
        assert_eq!(ReplacePreviewBuilder::total_changes(&previews), 3);
    }

    #[test]
    fn find_entry_creation() {
        let e = FindEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn find_entry_with_priority() {
        let e = FindEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn find_entry_metadata() {
        let e = FindEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn find_entry_remove_meta() {
        let mut e = FindEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn find_entry_activate_deactivate() {
        let mut e = FindEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn find_config_add_sorted() {
        let mut c = FindConfig::new(10);
        c.add(FindEntry::new("lo", "Lo").with_priority(1));
        c.add(FindEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn find_config_capacity() {
        let mut c = FindConfig::new(1);
        assert!(c.add(FindEntry::new("a", "A")));
        assert!(!c.add(FindEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn find_config_remove() {
        let mut c = FindConfig::new(10);
        c.add(FindEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn find_config_get() {
        let mut c = FindConfig::new(10);
        c.add(FindEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn find_config_active_entries() {
        let mut c = FindConfig::new(10);
        c.add(FindEntry::new("a", "A"));
        c.add(FindEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn find_config_enable_disable() {
        let mut c = FindConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn find_config_clear() {
        let mut c = FindConfig::new(10);
        c.add(FindEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn find_config_find_by_label() {
        let mut c = FindConfig::new(10);
        c.add(FindEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn find_config_top_n() {
        let mut c = FindConfig::new(10);
        c.add(FindEntry::new("a", "A").with_priority(1));
        c.add(FindEntry::new("b", "B").with_priority(2));
        c.add(FindEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn find_config_deactivate_activate_all() {
        let mut c = FindConfig::new(10);
        c.add(FindEntry::new("a", "A"));
        c.add(FindEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn find_config_highest_priority() {
        let mut c = FindConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(FindEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn find_config_contains() {
        let mut c = FindConfig::new(10);
        c.add(FindEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn find_config_labels() {
        let mut c = FindConfig::new(10);
        c.add(FindEntry::new("a", "Alpha"));
        c.add(FindEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn find_config_drain_inactive() {
        let mut c = FindConfig::new(10);
        c.add(FindEntry::new("a", "A"));
        c.add(FindEntry::new("b", "B"));
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


    // xa_ extended tests for find
    #[test]
    fn xa_find_ring_new() {
        let rb = super::XaFindRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_find_ring_push_len() {
        let mut rb = super::XaFindRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_find_ring_wrap() {
        let mut rb = super::XaFindRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_find_ring_mean_empty() {
        let rb = super::XaFindRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_find_ring_mean_values() {
        let mut rb = super::XaFindRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_find_ring_min_max() {
        let mut rb = super::XaFindRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_find_ring_iter() {
        let mut rb = super::XaFindRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_find_counter_new() {
        let c = super::XaFindCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_find_counter_inc() {
        let mut c = super::XaFindCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_find_counter_inc_by() {
        let mut c = super::XaFindCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_find_counter_reset() {
        let mut c = super::XaFindCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_find_counter_clear() {
        let mut c = super::XaFindCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_find_counter_default() {
        let c = super::XaFindCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 82 ----

    #[test]
    fn xc_82_pool_new_empty() {
        let pool: super::Xc82Pool<i32> = super::Xc82Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_82_pool_release_acquire() {
        let mut pool = super::Xc82Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_82_pool_acquire_empty() {
        let mut pool: super::Xc82Pool<i32> = super::Xc82Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_82_pool_full() {
        let mut pool = super::Xc82Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_82_pool_drain() {
        let mut pool = super::Xc82Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_82_pool_stats() {
        let mut pool = super::Xc82Pool::new(8);
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
    fn xc_82_pool_clear() {
        let mut pool = super::Xc82Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_82_pool_shrink() {
        let mut pool = super::Xc82Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_82_pool_default() {
        let pool: super::Xc82Pool<String> = super::Xc82Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_82_pool_extend() {
        let mut pool = super::Xc82Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_82_pool_retain() {
        let mut pool = super::Xc82Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_82_scheduler_round_robin() {
        let mut sched = super::Xc82Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_82_scheduler_empty() {
        let mut sched = super::Xc82Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_82_scheduler_reset() {
        let mut sched = super::Xc82Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_82_scheduler_add_remove() {
        let mut sched = super::Xc82Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_82_scheduler_targets() {
        let sched = super::Xc82Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_82_hash_empty() {
        assert_eq!(super::xc_82_hash(b""), 5381);
    }

    #[test]
    fn xc_82_hash_data() {
        let h = super::xc_82_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_82_hash(b"hello"), h);
    }

    #[test]
    fn xc_82_reverse_str() {
        assert_eq!(super::xc_82_reverse("abc"), "cba");
        assert_eq!(super::xc_82_reverse(""), "");
    }


    // --- xd_67 deepening tests ---

    #[test]
    fn xd_67_sm_initial_state() {
        let sm = Xd67StateMachine::new();
        assert_eq!(sm.current_state(), Xd67State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_67_sm_valid_idle_to_running() {
        let mut sm = Xd67StateMachine::new();
        assert!(sm.transition(Xd67State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd67State::Running);
    }

    #[test]
    fn xd_67_sm_valid_running_to_paused() {
        let mut sm = Xd67StateMachine::new();
        sm.transition(Xd67State::Running).unwrap();
        assert!(sm.transition(Xd67State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd67State::Paused);
    }

    #[test]
    fn xd_67_sm_valid_running_to_done() {
        let mut sm = Xd67StateMachine::new();
        sm.transition(Xd67State::Running).unwrap();
        assert!(sm.transition(Xd67State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd67State::Done);
    }

    #[test]
    fn xd_67_sm_valid_paused_to_running() {
        let mut sm = Xd67StateMachine::new();
        sm.transition(Xd67State::Running).unwrap();
        sm.transition(Xd67State::Paused).unwrap();
        assert!(sm.transition(Xd67State::Running).is_ok());
    }

    #[test]
    fn xd_67_sm_valid_done_to_idle() {
        let mut sm = Xd67StateMachine::new();
        sm.transition(Xd67State::Running).unwrap();
        sm.transition(Xd67State::Done).unwrap();
        assert!(sm.transition(Xd67State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd67State::Idle);
    }

    #[test]
    fn xd_67_sm_invalid_idle_to_done() {
        let mut sm = Xd67StateMachine::new();
        assert!(sm.transition(Xd67State::Done).is_err());
    }

    #[test]
    fn xd_67_sm_invalid_idle_to_paused() {
        let mut sm = Xd67StateMachine::new();
        assert!(sm.transition(Xd67State::Paused).is_err());
    }

    #[test]
    fn xd_67_sm_history_tracking() {
        let mut sm = Xd67StateMachine::new();
        sm.transition(Xd67State::Running).unwrap();
        sm.transition(Xd67State::Paused).unwrap();
        sm.transition(Xd67State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd67State::Idle);
        assert_eq!(sm.history()[0].to, Xd67State::Running);
        assert_eq!(sm.history()[1].from, Xd67State::Running);
        assert_eq!(sm.history()[2].to, Xd67State::Done);
    }

    #[test]
    fn xd_67_sm_serialize_deserialize() {
        let mut sm = Xd67StateMachine::new();
        sm.transition(Xd67State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd67StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd67State::Running));
    }

    #[test]
    fn xd_67_sm_deserialize_invalid() {
        assert_eq!(Xd67StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_67_sm_reset() {
        let mut sm = Xd67StateMachine::new();
        sm.transition(Xd67State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd67State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_67_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd67EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd67Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_67_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd67EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd67Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd67Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_67_bus_unsubscribe() {
        let mut bus = Xd67EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_67_event_kind_and_payload() {
        let e = Xd67Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd67Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_67_bus_clear_history() {
        let mut bus = Xd67EventBus::new();
        bus.publish(Xd67Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_67_sm_step_counter_increments() {
        let mut sm = Xd67StateMachine::new();
        sm.transition(Xd67State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd67State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #76 --

    #[test]
    fn xf76_trie_insert_search() {
        let mut t = Xf76Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf76_trie_starts_with() {
        let mut t = Xf76Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf76_trie_remove() {
        let mut t = Xf76Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf76_trie_word_count() {
        let mut t = Xf76Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf76_trie_longest_prefix() {
        let mut t = Xf76Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf76_trie_all_words() {
        let mut t = Xf76Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf76_trie_autocomplete() {
        let mut t = Xf76Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf76_trie_empty_search() {
        let t = Xf76Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf76_bloom_add_contains() {
        let mut bf = Xf76BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf76_bloom_probably_absent() {
        let bf = Xf76BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf76_bloom_false_positive_rate() {
        let mut bf = Xf76BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf76_bloom_clear() {
        let mut bf = Xf76BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf76_bloom_union() {
        let mut a = Xf76BloomFilter::xf_new(512, 2);
        let mut b = Xf76BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf76_bloom_intersection_estimate() {
        let mut a = Xf76BloomFilter::xf_new(512, 2);
        let mut b = Xf76BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf76_bloom_union_size_mismatch() {
        let a = Xf76BloomFilter::xf_new(256, 2);
        let b = Xf76BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh81_skip_insert_contains() {
        let mut sl = super::Xh81SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh81_skip_remove() {
        let mut sl = super::Xh81SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh81_skip_len() {
        let mut sl = super::Xh81SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh81_skip_range_query() {
        let mut sl = super::Xh81SkipList::xh_new(4);
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
    fn xh81_skip_floor_ceiling() {
        let mut sl = super::Xh81SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh81_skip_rank() {
        let mut sl = super::Xh81SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh81_skip_empty() {
        let sl = super::Xh81SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh81_skip_duplicates() {
        let mut sl = super::Xh81SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh81_bitset_set_test() {
        let mut bs = super::Xh81BitSet::xh_new(256);
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
    fn xh81_bitset_clear_count() {
        let mut bs = super::Xh81BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh81_bitset_and_or_xor() {
        let mut a = super::Xh81BitSet::xh_new(128);
        let mut b = super::Xh81BitSet::xh_new(128);
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
    fn xh81_bitset_iter_ones() {
        let mut bs = super::Xh81BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh81_bitset_first_last() {
        let mut bs = super::Xh81BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh81_bitset_empty() {
        let bs = super::Xh81BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}
