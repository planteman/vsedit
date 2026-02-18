//! String manipulation and unicode utilities.
//!
//! Equivalent to VS Code's `vs/base/common/strings.ts`.

use std::collections::HashMap;
use std::fmt;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Get the display width of a string in terminal columns.
pub fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Check if a character is a high surrogate (for UTF-16 compatibility).
pub fn is_high_surrogate(c: u32) -> bool {
    (0xD800..=0xDBFF).contains(&c)
}

/// Check if a character is a low surrogate (for UTF-16 compatibility).
pub fn is_low_surrogate(c: u32) -> bool {
    (0xDC00..=0xDFFF).contains(&c)
}

/// Check if a character is a full-width character (CJK, etc.).
pub fn is_fullwidth_char(c: char) -> bool {
    unicode_width::UnicodeWidthChar::width(c).unwrap_or(0) >= 2
}

/// Compare strings case-insensitively.
pub fn equals_ignore_case(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Check if `haystack` starts with `needle`, case-insensitive.
pub fn starts_with_ignore_case(haystack: &str, needle: &str) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    haystack[..needle.len()].eq_ignore_ascii_case(needle)
}

/// Check if a string contains only whitespace.
pub fn is_whitespace_only(s: &str) -> bool {
    s.chars().all(|c| c.is_whitespace())
}

/// Escape special regex characters in a string.
pub fn escape_regex(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' | '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^'
            | '$' => {
                result.push('\\');
                result.push(c);
            }
            _ => result.push(c),
        }
    }
    result
}

/// Count the number of grapheme clusters in a string.
pub fn grapheme_count(s: &str) -> usize {
    s.graphemes(true).count()
}

/// Truncate a string to fit within `max_width` terminal columns,
/// appending an ellipsis if truncated.
pub fn truncate_to_width(s: &str, max_width: usize) -> String {
    if display_width(s) <= max_width {
        return s.to_string();
    }
    if max_width <= 1 {
        return "…".to_string();
    }

    let mut result = String::new();
    let mut width = 0;
    let target = max_width - 1; // reserve space for ellipsis

    for grapheme in s.graphemes(true) {
        let w = UnicodeWidthStr::width(grapheme);
        if width + w > target {
            break;
        }
        result.push_str(grapheme);
        width += w;
    }
    result.push('…');
    result
}

/// Get the common prefix length between two strings.
pub fn common_prefix_length(a: &str, b: &str) -> usize {
    a.chars()
        .zip(b.chars())
        .take_while(|(ca, cb)| ca == cb)
        .count()
}

/// Get the common suffix length between two strings.
pub fn common_suffix_length(a: &str, b: &str) -> usize {
    a.chars()
        .rev()
        .zip(b.chars().rev())
        .take_while(|(ca, cb)| ca == cb)
        .count()
}

/// Check if a character is a word separator (used for word-based operations).
pub fn is_word_separator(c: char) -> bool {
    matches!(
        c,
        ' ' | '\t'
            | '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | '<'
            | '>'
            | '"'
            | '\''
            | '`'
            | ':'
            | ';'
            | ','
            | '.'
            | '!'
            | '?'
            | '@'
            | '#'
            | '$'
            | '%'
            | '^'
            | '&'
            | '*'
            | '-'
            | '+'
            | '='
            | '|'
            | '\\'
            | '/'
            | '~'
    )
}

/// Pad a string on the left to reach the desired width.
pub fn pad_left(s: &str, width: usize, pad_char: char) -> String {
    let current_width = display_width(s);
    if current_width >= width {
        return s.to_string();
    }
    let padding: String = std::iter::repeat(pad_char).take(width - current_width).collect();
    format!("{padding}{s}")
}

/// Pad a string on the right to reach the desired width.
pub fn pad_right(s: &str, width: usize, pad_char: char) -> String {
    let current_width = display_width(s);
    if current_width >= width {
        return s.to_string();
    }
    let padding: String = std::iter::repeat(pad_char).take(width - current_width).collect();
    format!("{s}{padding}")
}

/// Check if `haystack` ends with `needle`, case-insensitive.
pub fn ends_with_ignore_case(haystack: &str, needle: &str) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    haystack[haystack.len() - needle.len()..].eq_ignore_ascii_case(needle)
}

/// Extract all words from a string, splitting on word separators.
pub fn extract_words(s: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in s.char_indices() {
        if is_word_separator(c) || c.is_whitespace() {
            if let Some(s_idx) = start {
                words.push(&s[s_idx..i]);
                start = None;
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s_idx) = start {
        words.push(&s[s_idx..]);
    }
    words
}

/// Count occurrences of a substring within a string.
pub fn count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Replace the first occurrence of `from` with `to`.
pub fn replace_first(s: &str, from: &str, to: &str) -> String {
    match s.find(from) {
        Some(idx) => {
            let mut result = String::with_capacity(s.len() - from.len() + to.len());
            result.push_str(&s[..idx]);
            result.push_str(to);
            result.push_str(&s[idx + from.len()..]);
            result
        }
        None => s.to_string(),
    }
}

/// Errors that can occur during string operations.
#[derive(Debug, Clone, PartialEq)]
pub enum StringError {
    /// The input string was empty when a non-empty string was required.
    EmptyInput,
    /// A width or length parameter was invalid.
    InvalidWidth { requested: usize, reason: &'static str },
    /// An index was out of the string's grapheme bounds.
    GraphemeIndexOutOfBounds { index: usize, count: usize },
    /// A regex pattern string was invalid.
    InvalidPattern(String),
}

impl std::fmt::Display for StringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StringError::EmptyInput => write!(f, "input string must not be empty"),
            StringError::InvalidWidth { requested, reason } => {
                write!(f, "invalid width {requested}: {reason}")
            }
            StringError::GraphemeIndexOutOfBounds { index, count } => {
                write!(
                    f,
                    "grapheme index {index} out of bounds for string with {count} graphemes"
                )
            }
            StringError::InvalidPattern(p) => write!(f, "invalid pattern: {p}"),
        }
    }
}

impl std::error::Error for StringError {}

/// A measured string that caches its display width and grapheme count.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasuredString {
    text: String,
    width: usize,
    grapheme_len: usize,
}

impl MeasuredString {
    /// Create a new `MeasuredString`, computing width and grapheme count once.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let width = display_width(&text);
        let grapheme_len = grapheme_count(&text);
        Self {
            text,
            width,
            grapheme_len,
        }
    }

    /// The raw text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Cached display width in terminal columns.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Cached number of grapheme clusters.
    pub fn grapheme_len(&self) -> usize {
        self.grapheme_len
    }

    /// Return a truncated copy that fits within `max_width` columns.
    pub fn truncated(&self, max_width: usize) -> String {
        truncate_to_width(&self.text, max_width)
    }

    /// Slice by grapheme indices (start inclusive, end exclusive).
    pub fn grapheme_slice(&self, start: usize, end: usize) -> Result<String, StringError> {
        if start > self.grapheme_len || end > self.grapheme_len {
            return Err(StringError::GraphemeIndexOutOfBounds {
                index: start.max(end),
                count: self.grapheme_len,
            });
        }
        Ok(self
            .text
            .graphemes(true)
            .skip(start)
            .take(end.saturating_sub(start))
            .collect())
    }

    /// Check whether the measured string is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Check if all characters in the string are ASCII.
    pub fn is_ascii(&self) -> bool {
        self.text.is_ascii()
    }

    /// Return the number of Unicode scalar values (chars) in the string.
    pub fn char_count(&self) -> usize {
        self.text.chars().count()
    }
}

impl std::fmt::Display for MeasuredString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)
    }
}

impl From<&str> for MeasuredString {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// Builder for constructing formatted, width-constrained display lines.
#[derive(Debug, Clone)]
pub struct LineBuilder {
    parts: Vec<String>,
    separator: String,
    max_width: Option<usize>,
}

impl LineBuilder {
    /// Create a new `LineBuilder`.
    pub fn new() -> Self {
        Self {
            parts: Vec::new(),
            separator: String::new(),
            max_width: None,
        }
    }

    /// Set the separator inserted between parts.
    pub fn separator(mut self, sep: &str) -> Self {
        self.separator = sep.to_string();
        self
    }

    /// Set the maximum display width for the final line.
    pub fn max_width(mut self, width: usize) -> Self {
        self.max_width = Some(width);
        self
    }

    /// Append a part to the line.
    pub fn push(mut self, part: &str) -> Self {
        self.parts.push(part.to_string());
        self
    }

    /// Build the final line string, truncating if a max width was set.
    pub fn build(self) -> String {
        let joined = self.parts.join(&self.separator);
        match self.max_width {
            Some(w) => truncate_to_width(&joined, w),
            None => joined,
        }
    }

    /// Return the number of parts currently in the builder.
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// Return whether the builder has no parts.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }
}

impl Default for LineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a byte offset in a string to a grapheme cluster index.
pub fn byte_offset_to_grapheme_index(s: &str, byte_offset: usize) -> Option<usize> {
    let mut idx = 0;
    for (gi, grapheme) in s.grapheme_indices(true) {
        if gi == byte_offset {
            return Some(idx);
        }
        if gi + grapheme.len() > byte_offset {
            return None; // byte_offset falls inside a grapheme
        }
        idx += 1;
    }
    if byte_offset == s.len() {
        Some(idx)
    } else {
        None
    }
}

/// Convert a grapheme cluster index to a byte offset.
pub fn grapheme_index_to_byte_offset(s: &str, grapheme_idx: usize) -> Option<usize> {
    for (gi, (byte_pos, _)) in s.grapheme_indices(true).enumerate() {
        if gi == grapheme_idx {
            return Some(byte_pos);
        }
    }
    if grapheme_idx == grapheme_count(s) {
        Some(s.len())
    } else {
        None
    }
}

/// Compute the Levenshtein edit distance between two strings.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    let mut prev = (0..=n).collect::<Vec<_>>();
    let mut curr = vec![0; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1)
                .min(curr[j - 1] + 1)
                .min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Normalize whitespace: collapse runs of whitespace into a single space and trim.
pub fn normalize_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prev_ws = true; // treat start as whitespace to trim leading
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_ws {
                result.push(' ');
            }
            prev_ws = true;
        } else {
            result.push(c);
            prev_ws = false;
        }
    }
    // trim trailing space
    if result.ends_with(' ') {
        result.pop();
    }
    result
}

/// Compute the longest common substring length between two strings.
pub fn longest_common_substring_len(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();
    if m == 0 || n == 0 {
        return 0;
    }

    let mut prev = vec![0usize; n + 1];
    let mut curr = vec![0usize; n + 1];
    let mut max_len = 0;

    for i in 1..=m {
        for j in 1..=n {
            if a_chars[i - 1] == b_chars[j - 1] {
                curr[j] = prev[j - 1] + 1;
                if curr[j] > max_len {
                    max_len = curr[j];
                }
            } else {
                curr[j] = 0;
            }
        }
        std::mem::swap(&mut prev, &mut curr);
        curr.iter_mut().for_each(|v| *v = 0);
    }
    max_len
}

// ---------------------------------------------------------------------------
// Word boundary, string similarity, fuzzy matching, case conversion
// ---------------------------------------------------------------------------

/// Find word boundary positions (start indices of each word) in the string.
pub fn word_boundary_positions(s: &str) -> Vec<usize> {
    let mut positions = Vec::new();
    let mut in_word = false;
    for (i, c) in s.char_indices() {
        if is_word_separator(c) || c.is_whitespace() {
            in_word = false;
        } else if !in_word {
            positions.push(i);
            in_word = true;
        }
    }
    positions
}

/// Compute a normalized similarity score between two strings (0.0 = different, 1.0 = identical).
pub fn similarity_score(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let max_len = a.len().max(b.len());
    if max_len == 0 {
        return 1.0;
    }
    let dist = edit_distance(a, b);
    1.0 - (dist as f64 / max_len as f64)
}

/// Compute a fuzzy matching score: number of query chars that appear in order in the target.
pub fn fuzzy_match_score(query: &str, target: &str) -> usize {
    let mut matched = 0;
    let mut target_iter = target.chars();
    for qc in query.chars() {
        let qc_lower = qc.to_lowercase().next().unwrap_or(qc);
        for tc in target_iter.by_ref() {
            let tc_lower = tc.to_lowercase().next().unwrap_or(tc);
            if qc_lower == tc_lower {
                matched += 1;
                break;
            }
        }
    }
    matched
}

/// Convert a camelCase or PascalCase string to snake_case.
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            for lc in c.to_lowercase() {
                result.push(lc);
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Convert a snake_case string to camelCase.
pub fn to_camel_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = false;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            for uc in c.to_uppercase() {
                result.push(uc);
            }
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// Convert a snake_case string to PascalCase.
pub fn to_pascal_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            for uc in c.to_uppercase() {
                result.push(uc);
            }
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// Check if a string is a valid identifier (ASCII alphanumeric + underscore, not starting with digit).
pub fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Fuzzy string matcher that scores a query against a target string.
#[derive(Debug, Clone)]
pub struct StringMatcher {
    query: String,
}

impl StringMatcher {
    /// Create a new matcher for the given query (lowercased internally).
    pub fn new(query: &str) -> Self {
        Self { query: query.to_lowercase() }
    }

    /// Score a target string against the query. Returns 0 for no match.
    /// Higher scores indicate better matches. Consecutive character matches
    /// receive a bonus.
    pub fn score(&self, target: &str) -> u32 {
        let target_lower = target.to_lowercase();
        let target_chars: Vec<char> = target_lower.chars().collect();
        let query_chars: Vec<char> = self.query.chars().collect();
        if query_chars.is_empty() {
            return 0;
        }
        let mut score: u32 = 0;
        let mut ti = 0;
        let mut prev_match = false;
        for &qc in &query_chars {
            let mut found = false;
            while ti < target_chars.len() {
                if target_chars[ti] == qc {
                    score += 1;
                    if prev_match {
                        score += 2; // consecutive bonus
                    }
                    ti += 1;
                    found = true;
                    prev_match = true;
                    break;
                }
                ti += 1;
                prev_match = false;
            }
            if !found {
                return 0;
            }
        }
        score
    }
}

/// Result of a fuzzy match, containing the score and the byte positions of matched characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzyMatchResult {
    pub score: u32,
    pub positions: Vec<usize>,
}

/// Fuzzy-match `query` against `target`.
///
/// Returns `None` if not every character in `query` can be found (in order) in `target`.
/// Scoring: +1 per match, +3 consecutive bonus, +5 word-start bonus (first char or char
/// after a separator such as ` `, `_`, `-`, `/`, `.`).
pub fn fuzzy_match(query: &str, target: &str) -> Option<FuzzyMatchResult> {
    if query.is_empty() {
        return None;
    }

    let target_lower: Vec<char> = target.chars().flat_map(|c| c.to_lowercase()).collect();
    let target_chars: Vec<char> = target.chars().collect();
    let query_lower: Vec<char> = query.chars().flat_map(|c| c.to_lowercase()).collect();

    let mut positions = Vec::with_capacity(query_lower.len());
    let mut score: u32 = 0;
    let mut t_idx = 0;
    let mut prev_matched_idx: Option<usize> = None;

    for qch in &query_lower {
        let mut found = false;
        while t_idx < target_lower.len() {
            if target_lower[t_idx] == *qch {
                score += 1;

                // Consecutive bonus
                if let Some(prev) = prev_matched_idx {
                    if t_idx == prev + 1 {
                        score += 3;
                    }
                }

                // Word-start bonus
                if t_idx == 0 || matches!(target_chars[t_idx - 1], ' ' | '_' | '-' | '/' | '.') {
                    score += 5;
                }

                // Record byte position
                let byte_pos: usize = target_chars[..t_idx]
                    .iter()
                    .map(|c| c.len_utf8())
                    .sum();
                positions.push(byte_pos);

                prev_matched_idx = Some(t_idx);
                t_idx += 1;
                found = true;
                break;
            }
            t_idx += 1;
        }
        if !found {
            return None;
        }
    }

    Some(FuzzyMatchResult { score, positions })
}

/// Truncate a string in the middle, preserving the start and end with `…` in between.
///
/// If the string's character count is already within `max_len`, it is returned as-is.
/// `max_len` must be at least 2; otherwise the original string is returned.
pub fn truncate_middle(s: &str, max_len: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_len || max_len < 2 {
        return s.to_string();
    }
    // The ellipsis `…` takes 1 character position
    let remaining = max_len - 1;
    let head_len = (remaining + 1) / 2;
    let tail_len = remaining - head_len;

    let head: String = s.chars().take(head_len).collect();
    let tail: String = s.chars().skip(char_count - tail_len).collect();
    format!("{head}\u{2026}{tail}")
}

/// Count the number of whitespace-separated words in a string.
pub fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Count the number of Unicode characters (scalar values) in a string.
pub fn char_count(s: &str) -> usize {
    s.chars().count()
}

/// Capitalize the first character of a string.
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut result = String::with_capacity(s.len());
            for c in first.to_uppercase() {
                result.push(c);
            }
            result.extend(chars);
            result
        }
    }
}

/// Lowercase the first character of a string.
pub fn decapitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut result = String::with_capacity(s.len());
            for c in first.to_lowercase() {
                result.push(c);
            }
            result.extend(chars);
            result
        }
    }
}

/// Check if a string is empty or contains only whitespace.
pub fn is_blank(s: &str) -> bool {
    s.is_empty() || s.chars().all(|c| c.is_whitespace())
}

/// Repeat a string `count` times.
pub fn repeat_string(s: &str, count: usize) -> String {
    s.repeat(count)
}

/// Prepend `indent` to each line of `text`.
pub fn indent_lines(text: &str, indent: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut result = String::with_capacity(text.len() + indent.len() * lines.len());
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        result.push_str(indent);
        result.push_str(line);
    }
    result
}

/// Remove up to `count` leading spaces from each line of `text`.
pub fn dedent_lines(text: &str, count: usize) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut result = String::with_capacity(text.len());
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        let spaces = line.chars().take_while(|c| *c == ' ').count();
        let remove = spaces.min(count);
        result.push_str(&line[remove..]);
    }
    result
}

/// Convert a camelCase or PascalCase string to snake_case, handling consecutive uppercase runs
/// (e.g. "HTMLParser" → "html_parser").
pub fn camel_to_snake(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::with_capacity(s.len() + 4);
    let len = chars.len();
    let mut i = 0;
    while i < len {
        let c = chars[i];
        if c.is_uppercase() {
            // Detect a run of uppercase characters.
            let run_start = i;
            while i < len && chars[i].is_uppercase() {
                i += 1;
            }
            let run_len = i - run_start;
            if run_len == 1 {
                // Single uppercase char: simple boundary.
                if run_start > 0 {
                    result.push('_');
                }
                result.extend(c.to_lowercase());
            } else if i < len && !chars[i].is_uppercase() {
                // Acronym followed by a lowercase char — last uppercase starts a new word.
                if run_start > 0 {
                    result.push('_');
                }
                for j in run_start..i - 1 {
                    result.extend(chars[j].to_lowercase());
                }
                result.push('_');
                result.extend(chars[i - 1].to_lowercase());
            } else {
                // Trailing uppercase run (e.g. "FOO" at end).
                if run_start > 0 {
                    result.push('_');
                }
                for j in run_start..i {
                    result.extend(chars[j].to_lowercase());
                }
            }
        } else {
            result.push(c);
            i += 1;
        }
    }
    result
}

/// Convert a string to title case, capitalising the first letter of each whitespace-separated word.
pub fn title_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for (i, word) in s.split_whitespace().enumerate() {
        if i > 0 {
            result.push(' ');
        }
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            for c in first.to_uppercase() {
                result.push(c);
            }
            result.extend(chars);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Levenshtein distance
// ---------------------------------------------------------------------------

/// Compute the Levenshtein edit distance between two strings.
///
/// This counts the minimum number of single-character insertions, deletions,
/// or substitutions needed to transform `a` into `b`.
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut prev = (0..=n).collect::<Vec<_>>();
    let mut curr = vec![0; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1)
                .min(curr[j - 1] + 1)
                .min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

// ---------------------------------------------------------------------------
// Common prefix/suffix extraction
// ---------------------------------------------------------------------------

/// Extract the longest common prefix of two strings.
pub fn common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let len = common_prefix_length(a, b);
    &a[..len]
}

/// Extract the longest common suffix of two strings.
pub fn common_suffix<'a>(a: &'a str, b: &str) -> &'a str {
    let count = common_suffix_length(a, b);
    if count == 0 {
        return "";
    }
    // Find the byte offset for the last `count` chars
    let byte_start = a
        .char_indices()
        .rev()
        .nth(count - 1)
        .map(|(i, _)| i)
        .unwrap_or(0);
    &a[byte_start..]
}

// ---------------------------------------------------------------------------
// Word boundary detection
// ---------------------------------------------------------------------------

/// Find all word boundary byte offsets in a string.
///
/// A word boundary is at the start or end of a contiguous sequence of
/// alphanumeric (or underscore) characters.
pub fn find_word_boundaries(s: &str) -> Vec<usize> {
    let mut boundaries = Vec::new();
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    if chars.is_empty() {
        return boundaries;
    }
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    // Start of first word
    if is_word(chars[0].1) {
        boundaries.push(0);
    }
    for window in chars.windows(2) {
        let (i0, c0) = window[0];
        let (i1, c1) = window[1];
        if !is_word(c0) && is_word(c1) {
            boundaries.push(i1);
        } else if is_word(c0) && !is_word(c1) {
            boundaries.push(i0 + c0.len_utf8());
        }
    }
    // End of last word
    if let Some(&(i, c)) = chars.last() {
        if is_word(c) {
            boundaries.push(i + c.len_utf8());
        }
    }
    boundaries
}

// ---------------------------------------------------------------------------
// String normalization
// ---------------------------------------------------------------------------

/// Collapse all whitespace sequences (including newlines, tabs) into single
/// spaces and trim the result.
pub fn normalize_whitespace_full(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prev_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_ws && !result.is_empty() {
                result.push(' ');
            }
            prev_ws = true;
        } else {
            result.push(c);
            prev_ws = false;
        }
    }
    if result.ends_with(' ') {
        result.pop();
    }
    result
}

/// Remove all diacritical marks (combining characters) from a string.
///
/// This performs a basic ASCII-folding by stripping characters in the
/// Unicode combining diacritical marks block (U+0300..U+036F).
pub fn strip_diacritics(s: &str) -> String {
    s.chars()
        .filter(|c| !('\u{0300}'..='\u{036F}').contains(c))
        .collect()
}

// ---------------------------------------------------------------------------
// Title case (already exists above), kebab-case conversion
// ---------------------------------------------------------------------------

/// Convert a string to kebab-case.
pub fn to_kebab_case(s: &str) -> String {
    let snake = to_snake_case(s);
    snake.replace('_', "-")
}

// ---------------------------------------------------------------------------
// Slug generation, word wrapping, sentence case, center padding,
// line/column ↔ offset, indentation detection, Hamming distance,
// run-length encoding, string rotation check
// ---------------------------------------------------------------------------

/// Generate a URL-safe slug from a string.
///
/// Lowercases, replaces non-alphanumeric runs with a single hyphen,
/// and trims leading/trailing hyphens.
pub fn slugify(s: &str) -> String {
    let mut slug = String::with_capacity(s.len());
    let mut prev_dash = true; // suppress leading dash
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    // trim trailing dash
    if slug.ends_with('-') {
        slug.pop();
    }
    slug
}

/// Wrap text to a given column width, breaking on word boundaries.
///
/// Existing newlines are preserved. Words longer than `width` are kept
/// intact on their own line.
pub fn word_wrap(text: &str, width: usize) -> String {
    if width == 0 {
        return text.to_string();
    }
    let mut result = String::with_capacity(text.len() + text.len() / width);
    for line in text.split('\n') {
        if !result.is_empty() {
            result.push('\n');
        }
        let mut col = 0usize;
        for (i, word) in line.split_whitespace().enumerate() {
            let wlen = word.len();
            if i == 0 {
                result.push_str(word);
                col = wlen;
            } else if col + 1 + wlen > width {
                result.push('\n');
                result.push_str(word);
                col = wlen;
            } else {
                result.push(' ');
                result.push_str(word);
                col += 1 + wlen;
            }
        }
    }
    result
}

/// Convert a string to sentence case (first letter uppercase, rest lowercase).
pub fn to_sentence_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut out = first.to_uppercase().to_string();
            for c in chars {
                out.extend(c.to_lowercase());
            }
            out
        }
    }
}

/// Center-pad a string to the given width using `pad_char`.
///
/// If the string is already at least `width` characters, it is returned
/// unchanged. When the padding is odd, the extra character goes on the right.
pub fn pad_center(s: &str, width: usize, pad_char: char) -> String {
    let len = s.chars().count();
    if len >= width {
        return s.to_string();
    }
    let total_pad = width - len;
    let left = total_pad / 2;
    let right = total_pad - left;
    let mut out = String::with_capacity(width);
    for _ in 0..left {
        out.push(pad_char);
    }
    out.push_str(s);
    for _ in 0..right {
        out.push(pad_char);
    }
    out
}

/// Convert a (0-based line, 0-based column) position to a byte offset.
///
/// Returns `None` if the line or column is out of bounds.
pub fn line_col_to_offset(text: &str, line: usize, col: usize) -> Option<usize> {
    let mut current_line = 0usize;
    let mut line_start = 0usize;
    for (i, c) in text.char_indices() {
        if current_line == line {
            let pos_in_line = i - line_start;
            if pos_in_line == col {
                return Some(i);
            }
        }
        if c == '\n' {
            if current_line == line {
                // col was beyond this line
                return None;
            }
            current_line += 1;
            line_start = i + 1;
        }
    }
    // handle position at end of last line
    if current_line == line {
        let pos_in_line = text.len() - line_start;
        if col == pos_in_line {
            return Some(text.len());
        }
    }
    None
}

/// Convert a byte offset to a (0-based line, 0-based column) position.
///
/// Returns `None` if `offset` is out of bounds or not on a char boundary.
pub fn offset_to_line_col(text: &str, offset: usize) -> Option<(usize, usize)> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return None;
    }
    let mut line = 0usize;
    let mut line_start = 0usize;
    for (i, c) in text.char_indices() {
        if i == offset {
            return Some((line, i - line_start));
        }
        if c == '\n' {
            line += 1;
            line_start = i + 1;
        }
    }
    // offset == text.len()
    Some((line, text.len() - line_start))
}

/// Detect the indentation (leading whitespace) of the first non-empty line.
///
/// Returns the indentation string, or an empty string if no indented line is found.
pub fn detect_indentation(text: &str) -> &str {
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let stripped = line.trim_start();
        let indent_len = line.len() - stripped.len();
        if indent_len > 0 {
            return &line[..indent_len];
        }
    }
    ""
}

/// Compute the Hamming distance between two strings of equal length.
///
/// Returns `None` if the strings differ in character count.
pub fn hamming_distance(a: &str, b: &str) -> Option<usize> {
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    if ac.len() != bc.len() {
        return None;
    }
    Some(ac.iter().zip(bc.iter()).filter(|(x, y)| x != y).count())
}

/// Simple run-length encoding of a string.
///
/// `"aaabbc"` → `"3a2b1c"`
pub fn run_length_encode(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    let mut chars = s.chars();
    let mut current = chars.next().unwrap();
    let mut count = 1usize;
    for c in chars {
        if c == current {
            count += 1;
        } else {
            out.push_str(&count.to_string());
            out.push(current);
            current = c;
            count = 1;
        }
    }
    out.push_str(&count.to_string());
    out.push(current);
    out
}

/// Check whether `a` is a rotation of `b`.
///
/// Two strings are rotations of each other if one can be obtained by moving
/// some prefix of the other to the end (e.g. "abcde" and "cdeab").
pub fn is_rotation(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    if a.is_empty() {
        return true;
    }
    let doubled = format!("{a}{a}");
    doubled.contains(b)
}

/// A configurable string tokenizer that splits input by any character in a
/// delimiter set, filtering out empty tokens.
///
/// # Examples
/// ```
/// # use vsedit_strings::StringTokenizer;
/// let tok = StringTokenizer::new(",; ");
/// assert_eq!(tok.tokenize("a, b; c"), vec!["a", "b", "c"]);
/// ```
pub struct StringTokenizer {
    delimiters: Vec<char>,
}

impl StringTokenizer {
    /// Create a new tokenizer whose delimiters are the characters in `delimiters`.
    pub fn new(delimiters: &str) -> Self {
        Self {
            delimiters: delimiters.chars().collect(),
        }
    }

    /// Split `input` on any delimiter character, discarding empty segments.
    pub fn tokenize<'a>(&self, input: &'a str) -> Vec<&'a str> {
        input
            .split(|c: char| self.delimiters.contains(&c))
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Return the number of non-empty tokens in `input`.
    pub fn token_count(&self, input: &str) -> usize {
        self.tokenize(input).len()
    }
}

/// Simple English pluralization.
///
/// Rules applied (in order):
/// 1. Words ending in `s`, `x`, `z`, `ch`, or `sh` → append `"es"`.
/// 2. Words ending in `y` preceded by a consonant → replace `y` with `"ies"`.
/// 3. Otherwise → append `"s"`.
///
/// Empty input is returned unchanged.
pub fn pluralize(word: &str) -> String {
    if word.is_empty() {
        return String::new();
    }
    if word.ends_with("ch")
        || word.ends_with("sh")
        || word.ends_with('s')
        || word.ends_with('x')
        || word.ends_with('z')
    {
        return format!("{word}es");
    }
    if word.ends_with('y') {
        if let Some(before_y) = word[..word.len() - 1].chars().last() {
            if !"aeiouAEIOU".contains(before_y) {
                return format!("{}ies", &word[..word.len() - 1]);
            }
        }
    }
    format!("{word}s")
}

/// Simple English singularization (reverse of [`pluralize`]).
///
/// Rules applied (in order):
/// 1. Words ending in `"ies"` (preceded by a consonant) → replace with `"y"`.
/// 2. Words ending in `"ches"`, `"shes"`, `"ses"`, `"xes"`, `"zes"` → remove `"es"`.
/// 3. Words ending in `"s"` (but not `"ss"`) → remove trailing `"s"`.
/// 4. Otherwise → return unchanged.
pub fn singularize(word: &str) -> String {
    if word.is_empty() {
        return String::new();
    }
    // "ies" → "y" when preceded by a consonant
    if word.ends_with("ies") && word.len() > 3 {
        let before = word[..word.len() - 3].chars().last().unwrap();
        if !"aeiouAEIOU".contains(before) {
            return format!("{}y", &word[..word.len() - 3]);
        }
    }
    // "ches" / "shes" / "ses" / "xes" / "zes" → drop "es"
    if word.ends_with("ches")
        || word.ends_with("shes")
        || word.ends_with("ses")
        || word.ends_with("xes")
        || word.ends_with("zes")
    {
        return word[..word.len() - 2].to_string();
    }
    // trailing "s" but not "ss"
    if word.ends_with('s') && !word.ends_with("ss") {
        return word[..word.len() - 1].to_string();
    }
    word.to_string()
}

/// Split `s` into byte-oriented chunks of at most `chunk_size` bytes each,
/// respecting UTF-8 character boundaries so that no character is split.
///
/// Returns an empty `Vec` when `chunk_size` is zero.
pub fn string_chunks(s: &str, chunk_size: usize) -> Vec<&str> {
    if chunk_size == 0 {
        return Vec::new();
    }
    let mut result = Vec::new();
    let bytes = s.as_bytes();
    let mut start = 0;
    while start < bytes.len() {
        let mut end = (start + chunk_size).min(bytes.len());
        // Walk backwards to a char boundary
        while end > start && !s.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            // chunk_size is smaller than next char; skip to next boundary
            end = start + 1;
            while end < bytes.len() && !s.is_char_boundary(end) {
                end += 1;
            }
        }
        result.push(&s[start..end]);
        start = end;
    }
    result
}

/// Return `true` if `haystack` contains any of the strings in `needles`.
pub fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

/// Strip `prefix` from the beginning of `s` if present; otherwise return `s`
/// unchanged.
pub fn remove_prefix<'a>(s: &'a str, prefix: &str) -> &'a str {
    s.strip_prefix(prefix).unwrap_or(s)
}

/// Strip `suffix` from the end of `s` if present; otherwise return `s`
/// unchanged.
pub fn remove_suffix<'a>(s: &'a str, suffix: &str) -> &'a str {
    s.strip_suffix(suffix).unwrap_or(s)
}

/// Create a `String` consisting of `count` repetitions of the character `c`.
pub fn repeat_char(c: char, count: usize) -> String {
    std::iter::repeat(c).take(count).collect()
}

// ---------------------------------------------------------------------------
// StringTemplate – `${var}` substitution
// ---------------------------------------------------------------------------

/// A compiled string template that supports `${variable}` substitution.
///
/// Variables are delimited by `${` and `}`. Unresolved variables are left
/// verbatim in the output.
///
/// ```
/// use std::collections::HashMap;
/// use vsedit_strings::StringTemplate;
///
/// let tpl = StringTemplate::new("Hello, ${name}! You have ${count} items.");
/// let mut vars = HashMap::new();
/// vars.insert("name".into(), "Alice".into());
/// vars.insert("count".into(), "3".into());
/// assert_eq!(tpl.render(&vars), "Hello, Alice! You have 3 items.");
/// ```
pub struct StringTemplate {
    template: String,
}

impl StringTemplate {
    /// Create a new template from a format string.
    pub fn new(template: &str) -> Self {
        Self {
            template: template.to_string(),
        }
    }

    /// Render the template by replacing `${key}` with the corresponding value
    /// from `vars`. Variables not present in `vars` are left as-is.
    pub fn render(&self, vars: &HashMap<String, String>) -> String {
        let mut result = String::with_capacity(self.template.len());
        let bytes = self.template.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            if i + 1 < len && bytes[i] == b'$' && bytes[i + 1] == b'{' {
                if let Some(end) = self.template[i + 2..].find('}') {
                    let var_name = &self.template[i + 2..i + 2 + end];
                    if let Some(val) = vars.get(var_name) {
                        result.push_str(val);
                    } else {
                        // Leave unresolved variables verbatim.
                        result.push_str(&self.template[i..i + 2 + end + 1]);
                    }
                    i += 2 + end + 1;
                } else {
                    result.push(bytes[i] as char);
                    i += 1;
                }
            } else {
                result.push(bytes[i] as char);
                i += 1;
            }
        }

        result
    }

    /// Extract the unique variable names referenced in the template, returned
    /// in the order of first occurrence.
    pub fn variables(&self) -> Vec<String> {
        let mut vars = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let bytes = self.template.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            if i + 1 < len && bytes[i] == b'$' && bytes[i + 1] == b'{' {
                if let Some(end) = self.template[i + 2..].find('}') {
                    let var_name = &self.template[i + 2..i + 2 + end];
                    if seen.insert(var_name.to_string()) {
                        vars.push(var_name.to_string());
                    }
                    i += 2 + end + 1;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }

        vars
    }
}

// ---------------------------------------------------------------------------
// StringTrimmer – configurable truncation
// ---------------------------------------------------------------------------

/// Where to apply truncation when a string exceeds the maximum length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrimPosition {
    /// Remove characters from the start.
    Start,
    /// Remove characters from the middle.
    Middle,
    /// Remove characters from the end (default).
    End,
}

/// Trims strings to a maximum *grapheme-cluster* length with a configurable
/// marker and position.
pub struct StringTrimmer {
    max_length: usize,
    trim_marker: String,
    trim_position: TrimPosition,
}

impl StringTrimmer {
    /// Create a new trimmer that truncates at `max_length` grapheme clusters.
    pub fn new(max_length: usize) -> Self {
        Self {
            max_length,
            trim_marker: "...".to_string(),
            trim_position: TrimPosition::End,
        }
    }

    /// Set the marker that indicates truncated content (default `"..."`).
    pub fn with_marker(mut self, marker: &str) -> Self {
        self.trim_marker = marker.to_string();
        self
    }

    /// Set the position at which truncation occurs.
    pub fn with_position(mut self, pos: TrimPosition) -> Self {
        self.trim_position = pos;
        self
    }

    /// Trim `input` according to the configured parameters. If the input is
    /// already within `max_length` it is returned unchanged.
    pub fn trim(&self, input: &str) -> String {
        let graphemes: Vec<&str> = input.graphemes(true).collect();
        let g_len = graphemes.len();

        if g_len <= self.max_length {
            return input.to_string();
        }

        let marker_graphemes: Vec<&str> = self.trim_marker.graphemes(true).collect();
        let marker_len = marker_graphemes.len();

        // If the marker alone fills the budget, return a truncated marker.
        if marker_len >= self.max_length {
            return marker_graphemes[..self.max_length].concat();
        }

        let budget = self.max_length - marker_len;

        match self.trim_position {
            TrimPosition::End => {
                let mut out = graphemes[..budget].concat();
                out.push_str(&self.trim_marker);
                out
            }
            TrimPosition::Start => {
                let mut out = self.trim_marker.clone();
                out.push_str(&graphemes[g_len - budget..].concat());
                out
            }
            TrimPosition::Middle => {
                let left = (budget + 1) / 2;
                let right = budget / 2;
                let mut out = graphemes[..left].concat();
                out.push_str(&self.trim_marker);
                out.push_str(&graphemes[g_len - right..].concat());
                out
            }
        }
    }
}

// ---------------------------------------------------------------------------
// StringEscaper – HTML and shell escaping
// ---------------------------------------------------------------------------

/// Provides static helper methods to escape strings for different contexts.
pub struct StringEscaper;

impl StringEscaper {
    /// Escape the five XML/HTML special characters:
    /// `&`, `<`, `>`, `"`, `'`.
    pub fn escape_html(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                '\'' => out.push_str("&#39;"),
                _ => out.push(c),
            }
        }
        out
    }

    /// Escape shell meta-characters by prefixing each with a backslash.
    ///
    /// The set of characters escaped covers common POSIX shells:
    /// `` \ ` $ " ! # & | ; ( ) { } [ ] < > * ? ~ ^ space tab newline ``
    pub fn escape_shell(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            if matches!(
                c,
                '\\' | '`'
                    | '$'
                    | '"'
                    | '!'
                    | '#'
                    | '&'
                    | '|'
                    | ';'
                    | '('
                    | ')'
                    | '{'
                    | '}'
                    | '['
                    | ']'
                    | '<'
                    | '>'
                    | '*'
                    | '?'
                    | '~'
                    | '^'
                    | ' '
                    | '\t'
                    | '\n'
                    | '\''
            ) {
                out.push('\\');
            }
            out.push(c);
        }
        out
    }
}

// ---------------------------------------------------------------------------
// StringFrequencyCounter – word / string frequency analysis
// ---------------------------------------------------------------------------

/// Counts occurrences of arbitrary strings.
///
/// ```
/// use vsedit_strings::StringFrequencyCounter;
///
/// let mut counter = StringFrequencyCounter::new();
/// counter.add("hello");
/// counter.add("world");
/// counter.add("hello");
/// assert_eq!(counter.count("hello"), 2);
/// assert_eq!(counter.unique_count(), 2);
/// ```
pub struct StringFrequencyCounter {
    counts: HashMap<String, usize>,
}

impl StringFrequencyCounter {
    /// Create an empty frequency counter.
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }

    /// Record one occurrence of `s`.
    pub fn add(&mut self, s: &str) {
        *self.counts.entry(s.to_string()).or_insert(0) += 1;
    }

    /// Return the number of times `s` has been recorded, or `0` if never seen.
    pub fn count(&self, s: &str) -> usize {
        self.counts.get(s).copied().unwrap_or(0)
    }

    /// Return the `n` most-common strings, ordered by descending frequency.
    /// Ties are broken alphabetically.
    pub fn most_common(&self, n: usize) -> Vec<(String, usize)> {
        let mut entries: Vec<(String, usize)> = self
            .counts
            .iter()
            .map(|(k, &v)| (k.clone(), v))
            .collect();
        entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        entries.truncate(n);
        entries
    }

    /// Return the number of distinct strings that have been recorded.
    pub fn unique_count(&self) -> usize {
        self.counts.len()
    }

    /// Return `true` if no strings have been recorded.
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// Reset all counts to zero.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for StringFrequencyCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ─── StrC LRU Cache ───────────────────────────────────────

/// A simple LRU cache for string interning.
#[derive(Debug)]
pub struct StrCLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> StrCLruCache<V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self { entries: Vec::with_capacity(capacity), capacity, hits: 0, misses: 0 }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<(String, V)> {
        let key = key.into();
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
            self.entries.insert(0, (key, value));
            return None;
        }
        let evicted = if self.entries.len() >= self.capacity {
            Some(self.entries.pop().unwrap())
        } else { None };
        self.entries.insert(0, (key, value));
        evicted
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            self.hits += 1;
            let entry = self.entries.remove(pos);
            self.entries.insert(0, entry);
            Some(&self.entries[0].1)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else { None }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    pub fn hits(&self) -> u64 { self.hits }
    pub fn misses(&self) -> u64 { self.misses }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

impl<V: Clone + fmt::Display> fmt::Display for StrCLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StrCLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}

// ─── StrF Formatter ───────────────────────────────────────

/// Formatting options for string output.
#[derive(Debug, Clone)]
pub struct StrFFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for StrFFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl StrFFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for string data.
pub struct StrFFmt {
    options: StrFFmtOpts,
}

impl StrFFmt {
    pub fn new(options: StrFFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: StrFFmtOpts::default() } }

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


/// String utility configuration manager.
#[derive(Debug, Clone)]
pub struct StringsConfig {
    entries: Vec<StringsEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single string utility entry.
#[derive(Debug, Clone, PartialEq)]
pub struct StringsEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl StringsEntry {
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

impl StringsConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: StringsEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&StringsEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut StringsEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&StringsEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&StringsEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&StringsEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<StringsEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// String manipulation helpers — extended utilities (qy)
// ---------------------------------------------------------------------------

/// Metric accumulator for strings operations.
#[derive(Debug, Clone)]
pub struct QyMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QyMetrics {
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

/// Sliding-window rate counter for strings.
#[derive(Debug, Clone)]
pub struct QyRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QyRateWindow {
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

/// A small LRU-style cache for strings lookups.
#[derive(Debug, Clone)]
pub struct QyLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QyLruCache {
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
// xb_ utilities – batch 15
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer15 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer15 {
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
pub fn xb_fnv1a_15(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_15<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_15<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_15(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_15(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 168
// ---------------------------------------------------------------------------

/// Generic object pool `Xc168Pool<T>`.
pub struct Xc168Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc168Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc168PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc168Pool<T> {
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
    pub fn stats(&self) -> Xc168PoolStats {
        Xc168PoolStats {
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

impl<T> Default for Xc168Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc168Scheduler`.
pub struct Xc168Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc168Scheduler {
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

impl Default for Xc168Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_168 hash for the given byte slice.
pub fn xc_168_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_168 convention.
pub fn xc_168_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe27 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe27Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe27PipelineError {
    pub stage: Xe27Stage,
    pub message: String,
}

impl std::fmt::Display for Xe27PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe27Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe27Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe27PipelineError>>>,
    stage_names: Vec<Xe27Stage>,
}

impl Xe27Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe27PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe27Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe27PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe27Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe27PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe27Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe27PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe27Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe27PipelineError> {
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

    pub fn compose(mut self, other: Xe27Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe27CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe27CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe27Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe27CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe27CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe27Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe27CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_27_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe27CacheEntry {
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

    fn xe_27_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe27CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_27_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe27PipelineError> {
    Ok(data)
}

pub fn xe_27_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe27PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_27_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe27PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_27_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe27PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_27_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe27PipelineError> {
    Err(Xe27PipelineError {
        stage: Xe27Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #112
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf112Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf112TrieNode {
    children: std::collections::HashMap<char, Xf112TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf112Trie {
    root: Xf112TrieNode,
    count: usize,
}

impl Xf112Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf112TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf112TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf112TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf112BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf112BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 167).
pub struct Xh167SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh167SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 209 as u64,
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

/// A compact bit set supporting boolean operations (variant 167).
pub struct Xh167BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh167BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 167).
pub struct Xi167Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi167Deque<T> {
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
pub struct Xi167Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi167Interval {
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

/// A simple interval tree (variant 167).
pub struct Xi167IntervalTree {
    xi_intervals: Vec<Xi167Interval>,
}

impl Xi167IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi167Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi167Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi167Interval) -> Vec<&Xi167Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi167Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi167Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi167Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi167Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi167Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi167Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 167) ---

/// Disjoint set / union-find for crate 167.
pub struct Xj167UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj167UnionFind {
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

const XJ167_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 167.
pub struct Xj167BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj167BTreeNode<K, V>>>,
    len: usize,
}

struct Xj167BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj167BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj167BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ167_BTREE_ORDER - 1
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
        let mid = XJ167_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj167BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj167BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj167BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj167BTreeNode::xj_new_leaf();
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


// --- xk_167 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk167SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk167SegmentTree {
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
pub struct Xk167DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk167DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_167).
#[derive(Debug, Clone)]
pub struct Xl167Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl167Rope {
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

/// Suffix array for efficient string searching (xl_167).
#[derive(Debug, Clone)]
pub struct Xl167SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl167SuffixArray {
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
pub struct Xm167MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm167MatrixSparse {
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
pub struct Xm167Tokenizer {
    text: String,
}

impl Xm167Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 167.
pub struct Xn167Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn167Fenwick {
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

// ----- AVL tree map — crate 167 -----

#[derive(Debug, Clone)]
struct Xn167AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn167AvlNode<K, V>>>,
    right: Option<Box<Xn167AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 167.
#[derive(Debug, Clone)]
pub struct Xn167AVL<K, V> {
    root: Option<Box<Xn167AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn167AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn167AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn167AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn167AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn167AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn167AvlNode<K, V>>) -> Box<Xn167AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn167AvlNode<K, V>>) -> Box<Xn167AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn167AvlNode<K, V>>) -> Box<Xn167AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn167AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn167AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn167AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn167AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn167AvlNode<K, V>>) -> &Xn167AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn167AvlNode<K, V>>) -> (Box<Xn167AvlNode<K, V>>, Option<Box<Xn167AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn167AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn167AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn167AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn167AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn167AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn167AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn167AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo167RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo167Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo167RBNode<K, V> {
    key: K,
    value: V,
    color: Xo167Color,
    left: Option<Box<Xo167RBNode<K, V>>>,
    right: Option<Box<Xo167RBNode<K, V>>>,
}

/// A red-black tree map for crate 167.
#[derive(Debug, Clone)]
pub struct Xo167RedBlack<K, V> {
    root: Option<Box<Xo167RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo167RedBlack<K, V> {
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
            r.color = Xo167Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo167RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo167RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo167RBNode {
                    key, value, color: Xo167Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo167RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo167Color::Red)
    }

    fn xo_balance(mut h: Box<Xo167RBNode<K, V>>) -> Box<Xo167RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo167Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo167RBNode<K, V>>) -> Box<Xo167RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo167Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo167RBNode<K, V>>) -> Box<Xo167RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo167Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo167RBNode<K, V>>) {
        h.color = Xo167Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo167Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo167Color::Black; }
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
            r.color = Xo167Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo167RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo167RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo167RBNode<K, V>) -> (K, V, Option<Box<Xo167RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo167RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo167Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo167RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo167ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 167.
#[derive(Debug, Clone)]
pub struct Xo167ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo167ConsistentHash {
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
            let vkey = format!("{}#xo167#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo167#{}", node, i);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_width() {
        assert_eq!(display_width("hello"), 5);
        assert_eq!(display_width("日本語"), 6); // CJK chars are double-width
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate_to_width("hello world", 5), "hell…");
        assert_eq!(truncate_to_width("hi", 5), "hi");
    }

    #[test]
    fn test_common_prefix() {
        assert_eq!(common_prefix_length("abcdef", "abcxyz"), 3);
        assert_eq!(common_prefix_length("abc", "xyz"), 0);
    }

    #[test]
    fn test_escape_regex() {
        assert_eq!(escape_regex("a.b*c"), "a\\.b\\*c");
    }

    #[test]
    fn test_equals_ignore_case() {
        assert!(equals_ignore_case("Hello", "hello"));
        assert!(!equals_ignore_case("Hello", "world"));
    }

    #[test]
    fn test_is_word_separator() {
        assert!(is_word_separator(' '));
        assert!(is_word_separator('.'));
        assert!(!is_word_separator('a'));
    }

    #[test]
    fn test_pad() {
        assert_eq!(pad_left("42", 5, '0'), "00042");
        assert_eq!(pad_right("hi", 5, ' '), "hi   ");
    }

    #[test]
    fn test_ends_with_ignore_case() {
        assert!(ends_with_ignore_case("Hello World", "WORLD"));
        assert!(ends_with_ignore_case("test.TXT", "txt"));
        assert!(!ends_with_ignore_case("abc", "abcd"));
        assert!(!ends_with_ignore_case("hello", "xyz"));
    }

    #[test]
    fn test_extract_words() {
        assert_eq!(extract_words("hello world"), vec!["hello", "world"]);
        assert_eq!(extract_words("foo.bar+baz"), vec!["foo", "bar", "baz"]);
        assert!(extract_words("   ").is_empty());
        assert_eq!(extract_words("single"), vec!["single"]);
    }

    #[test]
    fn test_count_occurrences() {
        assert_eq!(count_occurrences("abcabcabc", "abc"), 3);
        assert_eq!(count_occurrences("hello", "xyz"), 0);
        assert_eq!(count_occurrences("aaa", "a"), 3);
        assert_eq!(count_occurrences("anything", ""), 0);
    }

    #[test]
    fn test_replace_first() {
        assert_eq!(replace_first("aabaa", "a", "x"), "xabaa");
        assert_eq!(replace_first("hello", "xyz", "q"), "hello");
        assert_eq!(replace_first("foo bar foo", "foo", "baz"), "baz bar foo");
    }

    #[test]
    fn test_string_error_display() {
        let e = StringError::EmptyInput;
        assert_eq!(e.to_string(), "input string must not be empty");

        let e2 = StringError::InvalidWidth {
            requested: 0,
            reason: "must be positive",
        };
        assert!(e2.to_string().contains("invalid width 0"));

        let e3 = StringError::GraphemeIndexOutOfBounds {
            index: 10,
            count: 5,
        };
        assert!(e3.to_string().contains("out of bounds"));
    }

    #[test]
    fn test_measured_string() {
        let ms = MeasuredString::new("hello");
        assert_eq!(ms.width(), 5);
        assert_eq!(ms.grapheme_len(), 5);
        assert_eq!(ms.text(), "hello");
        assert!(!ms.is_empty());
        assert_eq!(ms.to_string(), "hello");

        let ms2 = MeasuredString::new("日本語");
        assert_eq!(ms2.width(), 6);
        assert_eq!(ms2.grapheme_len(), 3);

        // grapheme slicing
        assert_eq!(ms.grapheme_slice(1, 3).unwrap(), "el");
        assert!(ms.grapheme_slice(0, 10).is_err());

        // Clone + PartialEq
        let ms3 = ms.clone();
        assert_eq!(ms, ms3);

        // From<&str>
        let ms4: MeasuredString = "test".into();
        assert_eq!(ms4.text(), "test");
    }

    #[test]
    fn test_line_builder() {
        let line = LineBuilder::new()
            .separator(" | ")
            .push("Name")
            .push("Age")
            .push("City")
            .build();
        assert_eq!(line, "Name | Age | City");

        let truncated = LineBuilder::new()
            .separator(", ")
            .max_width(10)
            .push("hello")
            .push("world")
            .build();
        assert_eq!(display_width(&truncated), 10);
        assert!(truncated.ends_with('…'));

        let builder = LineBuilder::new();
        assert!(builder.is_empty());
        assert_eq!(builder.len(), 0);
    }

    #[test]
    fn test_edit_distance() {
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("abc", "abc"), 0);
        assert_eq!(edit_distance("abc", ""), 3);
        assert_eq!(edit_distance("flaw", "lawn"), 2);
    }

    #[test]
    fn test_normalize_whitespace() {
        assert_eq!(normalize_whitespace("  hello   world  "), "hello world");
        assert_eq!(normalize_whitespace("no\textra \n spaces"), "no extra spaces");
        assert_eq!(normalize_whitespace(""), "");
        assert_eq!(normalize_whitespace("single"), "single");
    }

    #[test]
    fn test_grapheme_byte_conversions() {
        // ASCII: each char is 1 byte
        assert_eq!(byte_offset_to_grapheme_index("hello", 0), Some(0));
        assert_eq!(byte_offset_to_grapheme_index("hello", 3), Some(3));
        assert_eq!(byte_offset_to_grapheme_index("hello", 5), Some(5));

        assert_eq!(grapheme_index_to_byte_offset("hello", 0), Some(0));
        assert_eq!(grapheme_index_to_byte_offset("hello", 5), Some(5));
        assert_eq!(grapheme_index_to_byte_offset("hello", 6), None);

        // Multi-byte: "日本語" — each char is 3 bytes
        assert_eq!(byte_offset_to_grapheme_index("日本語", 0), Some(0));
        assert_eq!(byte_offset_to_grapheme_index("日本語", 3), Some(1));
        assert_eq!(byte_offset_to_grapheme_index("日本語", 1), None); // mid-grapheme
    }

    #[test]
    fn test_longest_common_substring() {
        assert_eq!(longest_common_substring_len("abcdef", "xbcdx"), 3);
        assert_eq!(longest_common_substring_len("abc", "xyz"), 0);
        assert_eq!(longest_common_substring_len("", "abc"), 0);
        assert_eq!(longest_common_substring_len("abcabc", "abc"), 3);
    }

    #[test]
    fn test_common_suffix_length() {
        assert_eq!(common_suffix_length("abcdef", "xyzdef"), 3);
        assert_eq!(common_suffix_length("abc", "xyz"), 0);
        assert_eq!(common_suffix_length("test", "best"), 3);
    }

    #[test]
    fn test_surrogates() {
        assert!(is_high_surrogate(0xD800));
        assert!(is_high_surrogate(0xDBFF));
        assert!(!is_high_surrogate(0xDC00));
        assert!(is_low_surrogate(0xDC00));
        assert!(is_low_surrogate(0xDFFF));
        assert!(!is_low_surrogate(0xD800));
    }

    #[test]
    fn test_word_boundary_positions() {
        assert_eq!(word_boundary_positions("hello world"), vec![0, 6]);
        assert_eq!(word_boundary_positions("foo.bar+baz"), vec![0, 4, 8]);
        assert!(word_boundary_positions("   ").is_empty());
        assert_eq!(word_boundary_positions("single"), vec![0]);
    }

    #[test]
    fn test_similarity_score() {
        assert!((similarity_score("abc", "abc") - 1.0).abs() < f64::EPSILON);
        assert!(similarity_score("kitten", "sitting") > 0.0);
        assert!(similarity_score("kitten", "sitting") < 1.0);
        assert!((similarity_score("", "") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fuzzy_match_score() {
        assert_eq!(fuzzy_match_score("abc", "aXbXcX"), 3);
        assert_eq!(fuzzy_match_score("xyz", "abc"), 0);
        assert_eq!(fuzzy_match_score("fl", "FooBarList"), 2);
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("camelCase"), "camel_case");
        assert_eq!(to_snake_case("PascalCase"), "pascal_case");
        assert_eq!(to_snake_case("already_snake"), "already_snake");
        assert_eq!(to_snake_case("HTMLParser"), "h_t_m_l_parser");
    }

    #[test]
    fn test_to_camel_case() {
        assert_eq!(to_camel_case("snake_case"), "snakeCase");
        assert_eq!(to_camel_case("hello_world"), "helloWorld");
        assert_eq!(to_camel_case("already"), "already");
    }

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("snake_case"), "SnakeCase");
        assert_eq!(to_pascal_case("hello_world"), "HelloWorld");
    }

    #[test]
    fn test_is_valid_identifier() {
        assert!(is_valid_identifier("hello"));
        assert!(is_valid_identifier("_foo"));
        assert!(is_valid_identifier("bar_123"));
        assert!(!is_valid_identifier(""));
        assert!(!is_valid_identifier("123abc"));
        assert!(!is_valid_identifier("foo bar"));
    }

    #[test]
    fn matcher_exact() {
        let m = StringMatcher::new("abc");
        assert!(m.score("abc") > 0);
    }

    #[test]
    fn matcher_no_match() {
        let m = StringMatcher::new("xyz");
        assert_eq!(m.score("abc"), 0);
    }

    #[test]
    fn matcher_fuzzy() {
        let m = StringMatcher::new("fb");
        assert!(m.score("foobar") > 0);
    }

    #[test]
    fn matcher_consecutive_bonus() {
        let m = StringMatcher::new("ab");
        let consecutive = m.score("ab");
        let spread = m.score("a_b");
        assert!(consecutive > spread);
    }

    #[test]
    fn matcher_case_insensitive() {
        let m = StringMatcher::new("ABC");
        assert!(m.score("abcdef") > 0);
    }

    #[test]
    fn matcher_empty_query() {
        let m = StringMatcher::new("");
        assert_eq!(m.score("anything"), 0);
    }

    // --- fuzzy_match tests ---

    #[test]
    fn fuzzy_match_basic() {
        let result = fuzzy_match("fb", "foo bar").unwrap();
        assert!(result.score > 0);
        assert_eq!(result.positions.len(), 2);
        assert_eq!(result.positions[0], 0); // 'f' at byte 0
        assert_eq!(result.positions[1], 4); // 'b' at byte 4
    }

    #[test]
    fn fuzzy_match_no_match() {
        assert!(fuzzy_match("xyz", "hello").is_none());
    }

    #[test]
    fn fuzzy_match_empty_query() {
        assert!(fuzzy_match("", "anything").is_none());
    }

    #[test]
    fn fuzzy_match_consecutive_scores_higher() {
        let consecutive = fuzzy_match("ab", "xab").unwrap().score;
        let spread = fuzzy_match("ab", "xaxb").unwrap().score;
        assert!(consecutive > spread);
    }

    #[test]
    fn fuzzy_match_word_start_bonus() {
        // 'b' at word start should score higher than 'b' mid-word
        let word_start = fuzzy_match("b", "foo bar").unwrap().score;
        let mid_word = fuzzy_match("b", "abc").unwrap().score;
        assert!(word_start > mid_word);
    }

    // --- truncate_middle tests ---

    #[test]
    fn truncate_middle_short_string() {
        assert_eq!(truncate_middle("hello", 10), "hello");
    }

    #[test]
    fn truncate_middle_long_string() {
        let result = truncate_middle("hello world foo bar", 15);
        assert!(result.chars().count() <= 15);
        assert!(result.contains('\u{2026}'));
        // Should preserve start and end
        assert!(result.starts_with("hello"));
        assert!(result.ends_with("oo bar"));
    }

    #[test]
    fn truncate_middle_exact_fit() {
        let s = "abcde";
        assert_eq!(truncate_middle(s, 5), "abcde");
    }

    // --- word_count / char_count tests ---

    #[test]
    fn word_and_char_count() {
        assert_eq!(word_count("hello world"), 2);
        assert_eq!(word_count(""), 0);
        assert_eq!(word_count("  spaces  "), 1);
        assert_eq!(char_count("hello"), 5);
        assert_eq!(char_count("日本語"), 3);
        assert_eq!(char_count(""), 0);
    }

    // --- title_case tests ---

    #[test]
    fn title_case_basic() {
        assert_eq!(title_case("hello world"), "Hello World");
        assert_eq!(title_case("HELLO WORLD"), "HELLO WORLD");
        assert_eq!(title_case("a b c"), "A B C");
    }

    #[test]
    fn title_case_empty() {
        assert_eq!(title_case(""), "");
    }

    #[test]
    fn title_case_unicode() {
        assert_eq!(title_case("über cool"), "Über Cool");
    }

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize("hello"), "Hello");
        assert_eq!(capitalize(""), "");
        assert_eq!(capitalize("Hello"), "Hello");
        assert_eq!(capitalize("über"), "Über");
    }

    #[test]
    fn test_decapitalize() {
        assert_eq!(decapitalize("Hello"), "hello");
        assert_eq!(decapitalize(""), "");
        assert_eq!(decapitalize("ABC"), "aBC");
    }

    #[test]
    fn test_is_blank() {
        assert!(is_blank(""));
        assert!(is_blank("   "));
        assert!(is_blank("\t\n "));
        assert!(!is_blank("a"));
        assert!(!is_blank("  x  "));
    }

    #[test]
    fn test_repeat_string() {
        assert_eq!(repeat_string("ab", 3), "ababab");
        assert_eq!(repeat_string("x", 0), "");
        assert_eq!(repeat_string("", 5), "");
    }

    #[test]
    fn test_indent_and_dedent_lines() {
        let text = "line1\nline2\nline3";
        let indented = indent_lines(text, "  ");
        assert_eq!(indented, "  line1\n  line2\n  line3");
        let dedented = dedent_lines(&indented, 2);
        assert_eq!(dedented, text);
        // dedent more than available spaces is safe
        assert_eq!(dedent_lines("  hi", 10), "hi");
    }

    #[test]
    fn test_camel_to_snake() {
        assert_eq!(camel_to_snake("camelCase"), "camel_case");
        assert_eq!(camel_to_snake("HTMLParser"), "html_parser");
        assert_eq!(camel_to_snake("PascalCase"), "pascal_case");
        assert_eq!(camel_to_snake("already_snake"), "already_snake");
        assert_eq!(camel_to_snake("getHTTPSUrl"), "get_https_url");
        assert_eq!(camel_to_snake("ABC"), "abc");
    }

    #[test]
    fn test_measured_string_is_ascii_and_char_count() {
        let ascii = MeasuredString::new("hello");
        assert!(ascii.is_ascii());
        assert_eq!(ascii.char_count(), 5);

        let unicode = MeasuredString::new("日本語");
        assert!(!unicode.is_ascii());
        assert_eq!(unicode.char_count(), 3);

        let empty = MeasuredString::new("");
        assert!(empty.is_ascii());
        assert_eq!(empty.char_count(), 0);
    }

    // -- new tests --

    #[test]
    fn test_levenshtein_distance_identical() {
        assert_eq!(levenshtein_distance("hello", "hello"), 0);
    }

    #[test]
    fn test_levenshtein_distance_one_edit() {
        assert_eq!(levenshtein_distance("kitten", "sitten"), 1);
        assert_eq!(levenshtein_distance("", "abc"), 3);
        assert_eq!(levenshtein_distance("abc", ""), 3);
    }

    #[test]
    fn test_levenshtein_distance_multiple_edits() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn test_common_prefix_extraction() {
        assert_eq!(common_prefix("foobar", "foobaz"), "fooba");
        assert_eq!(common_prefix("abc", "xyz"), "");
        assert_eq!(common_prefix("abc", "abc"), "abc");
    }

    #[test]
    fn test_common_suffix_extraction() {
        assert_eq!(common_suffix("testing", "running"), "ing");
        assert_eq!(common_suffix("abc", "xyz"), "");
        assert_eq!(common_suffix("abc", "abc"), "abc");
    }

    #[test]
    fn test_find_word_boundaries() {
        let boundaries = find_word_boundaries("hello world");
        // Boundaries: 0 (start "hello"), 5 (end "hello"), 6 (start "world"), 11 (end "world")
        assert_eq!(boundaries, vec![0, 5, 6, 11]);
    }

    #[test]
    fn test_find_word_boundaries_empty() {
        assert!(find_word_boundaries("").is_empty());
    }

    #[test]
    fn test_normalize_whitespace_full() {
        assert_eq!(normalize_whitespace_full("  hello   world  "), "hello world");
        assert_eq!(normalize_whitespace_full("a\t\nb"), "a b");
    }

    #[test]
    fn test_strip_diacritics() {
        // Precomposed chars won't be stripped, but combining marks will be
        let s = "e\u{0301}"; // e + combining acute accent
        let stripped = strip_diacritics(s);
        assert_eq!(stripped, "e");
    }

    #[test]
    fn test_to_kebab_case() {
        assert_eq!(to_kebab_case("camelCase"), "camel-case");
        assert_eq!(to_kebab_case("PascalCase"), "pascal-case");
        assert_eq!(to_kebab_case("already_snake"), "already-snake");
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
        assert_eq!(slugify("  foo   bar  "), "foo-bar");
        assert_eq!(slugify("Rust 2024 — Release"), "rust-2024-release");
        assert_eq!(slugify("already-slug"), "already-slug");
        assert_eq!(slugify(""), "");
    }

    #[test]
    fn test_word_wrap() {
        assert_eq!(word_wrap("hello world foo", 11), "hello world\nfoo");
        assert_eq!(word_wrap("short", 80), "short");
        assert_eq!(
            word_wrap("one two three four", 9),
            "one two\nthree\nfour"
        );
        // preserves existing newlines
        assert_eq!(word_wrap("a\nb", 80), "a\nb");
    }

    #[test]
    fn test_to_sentence_case() {
        assert_eq!(to_sentence_case("hELLO WORLD"), "Hello world");
        assert_eq!(to_sentence_case(""), "");
        assert_eq!(to_sentence_case("a"), "A");
    }

    #[test]
    fn test_pad_center() {
        assert_eq!(pad_center("hi", 6, '-'), "--hi--");
        assert_eq!(pad_center("hi", 7, '-'), "--hi---");
        assert_eq!(pad_center("long enough", 5, '-'), "long enough");
    }

    #[test]
    fn test_line_col_to_offset_and_back() {
        let text = "hello\nworld\nfoo";
        assert_eq!(line_col_to_offset(text, 0, 0), Some(0));
        assert_eq!(line_col_to_offset(text, 1, 0), Some(6));
        assert_eq!(line_col_to_offset(text, 2, 3), Some(15));
        assert_eq!(line_col_to_offset(text, 5, 0), None);

        assert_eq!(offset_to_line_col(text, 0), Some((0, 0)));
        assert_eq!(offset_to_line_col(text, 6), Some((1, 0)));
        assert_eq!(offset_to_line_col(text, 15), Some((2, 3)));
    }

    #[test]
    fn test_detect_indentation() {
        assert_eq!(detect_indentation("    hello\n    world"), "    ");
        assert_eq!(detect_indentation("\thello"), "\t");
        assert_eq!(detect_indentation("no indent"), "");
        assert_eq!(detect_indentation("\n  indented"), "  ");
    }

    #[test]
    fn test_hamming_distance() {
        assert_eq!(hamming_distance("karolin", "kathrin"), Some(3));
        assert_eq!(hamming_distance("abc", "abc"), Some(0));
        assert_eq!(hamming_distance("ab", "abc"), None);
    }

    #[test]
    fn test_run_length_encode() {
        assert_eq!(run_length_encode("aaabbc"), "3a2b1c");
        assert_eq!(run_length_encode("a"), "1a");
        assert_eq!(run_length_encode(""), "");
    }

    #[test]
    fn test_is_rotation() {
        assert!(is_rotation("abcde", "cdeab"));
        assert!(is_rotation("", ""));
        assert!(!is_rotation("abc", "cab1"));
        assert!(!is_rotation("abc", "bca1"));
        assert!(is_rotation("abc", "bca"));
    }

    #[test]
    fn test_string_tokenizer_basic() {
        let tok = StringTokenizer::new(",; ");
        assert_eq!(tok.tokenize("hello, world; foo bar"), vec!["hello", "world", "foo", "bar"]);
    }

    #[test]
    fn test_string_tokenizer_empty_input() {
        let tok = StringTokenizer::new(",");
        assert_eq!(tok.tokenize(""), Vec::<&str>::new());
        assert_eq!(tok.token_count(",,,"), 0);
    }

    #[test]
    fn test_string_tokenizer_count() {
        let tok = StringTokenizer::new(" ");
        assert_eq!(tok.token_count("a b c d"), 4);
        assert_eq!(tok.token_count("  leading  "), 1);
    }

    #[test]
    fn test_pluralize() {
        assert_eq!(pluralize("cat"), "cats");
        assert_eq!(pluralize("bus"), "buses");
        assert_eq!(pluralize("box"), "boxes");
        assert_eq!(pluralize("buzz"), "buzzes");
        assert_eq!(pluralize("church"), "churches");
        assert_eq!(pluralize("wish"), "wishes");
        assert_eq!(pluralize("baby"), "babies");
        assert_eq!(pluralize("day"), "days"); // vowel before y
        assert_eq!(pluralize(""), "");
    }

    #[test]
    fn test_singularize() {
        assert_eq!(singularize("cats"), "cat");
        assert_eq!(singularize("buses"), "bus");
        assert_eq!(singularize("boxes"), "box");
        assert_eq!(singularize("buzzes"), "buzz");
        assert_eq!(singularize("churches"), "church");
        assert_eq!(singularize("wishes"), "wish");
        assert_eq!(singularize("babies"), "baby");
        assert_eq!(singularize(""), "");
    }

    #[test]
    fn test_pluralize_singularize_roundtrip() {
        for word in &["cat", "bus", "box", "church", "wish", "baby"] {
            assert_eq!(singularize(&pluralize(word)), *word);
        }
    }

    #[test]
    fn test_string_chunks_ascii() {
        assert_eq!(string_chunks("abcdefg", 3), vec!["abc", "def", "g"]);
        assert_eq!(string_chunks("abc", 10), vec!["abc"]);
        assert_eq!(string_chunks("", 5), Vec::<&str>::new());
        assert_eq!(string_chunks("abc", 0), Vec::<&str>::new());
    }

    #[test]
    fn test_string_chunks_unicode() {
        // 'é' is 2 bytes in UTF-8; chunk_size=3 must not split it
        let chunks = string_chunks("aéb", 3);
        assert_eq!(chunks, vec!["a\u{e9}", "b"]);
    }

    #[test]
    fn test_contains_any() {
        assert!(contains_any("hello world", &["world", "xyz"]));
        assert!(!contains_any("hello", &["xyz", "abc"]));
        assert!(!contains_any("hello", &[]));
    }

    #[test]
    fn test_remove_prefix_and_suffix() {
        assert_eq!(remove_prefix("hello world", "hello "), "world");
        assert_eq!(remove_prefix("hello", "xyz"), "hello");
        assert_eq!(remove_suffix("hello world", " world"), "hello");
        assert_eq!(remove_suffix("hello", "xyz"), "hello");
    }

    #[test]
    fn test_repeat_char() {
        assert_eq!(repeat_char('x', 5), "xxxxx");
        assert_eq!(repeat_char('☆', 3), "☆☆☆");
        assert_eq!(repeat_char('a', 0), "");
    }

    // -----------------------------------------------------------------------
    // StringTemplate tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_template_basic_substitution() {
        let tpl = StringTemplate::new("Hello, ${name}!");
        let mut vars = HashMap::new();
        vars.insert("name".into(), "World".into());
        assert_eq!(tpl.render(&vars), "Hello, World!");
    }

    #[test]
    fn test_template_multiple_variables() {
        let tpl = StringTemplate::new("${greeting}, ${name}! You have ${n} items.");
        let mut vars = HashMap::new();
        vars.insert("greeting".into(), "Hi".into());
        vars.insert("name".into(), "Bob".into());
        vars.insert("n".into(), "42".into());
        assert_eq!(tpl.render(&vars), "Hi, Bob! You have 42 items.");
    }

    #[test]
    fn test_template_unresolved_variable() {
        let tpl = StringTemplate::new("${known} and ${unknown}");
        let mut vars = HashMap::new();
        vars.insert("known".into(), "yes".into());
        assert_eq!(tpl.render(&vars), "yes and ${unknown}");
    }

    #[test]
    fn test_template_variables_extraction() {
        let tpl = StringTemplate::new("${a} ${b} ${a} ${c}");
        let vars = tpl.variables();
        assert_eq!(vars, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_template_no_variables() {
        let tpl = StringTemplate::new("plain text");
        assert!(tpl.variables().is_empty());
        let vars = HashMap::new();
        assert_eq!(tpl.render(&vars), "plain text");
    }

    // -----------------------------------------------------------------------
    // StringTrimmer tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_trimmer_end() {
        let trimmer = StringTrimmer::new(8);
        assert_eq!(trimmer.trim("Hello, World!"), "Hello...");
        assert_eq!(trimmer.trim("short"), "short");
    }

    #[test]
    fn test_trimmer_start() {
        let trimmer = StringTrimmer::new(8).with_position(TrimPosition::Start);
        assert_eq!(trimmer.trim("Hello, World!"), "...orld!");
    }

    #[test]
    fn test_trimmer_middle() {
        let trimmer = StringTrimmer::new(10).with_position(TrimPosition::Middle);
        assert_eq!(trimmer.trim("Hello, World!"), "Hell...ld!");
    }

    #[test]
    fn test_trimmer_custom_marker() {
        let trimmer = StringTrimmer::new(7).with_marker("…");
        assert_eq!(trimmer.trim("abcdefghij"), "abcdef…");
    }

    // -----------------------------------------------------------------------
    // StringEscaper tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_escape_html() {
        assert_eq!(
            StringEscaper::escape_html("<b>Hello & 'World'</b>"),
            "&lt;b&gt;Hello &amp; &#39;World&#39;&lt;/b&gt;"
        );
        assert_eq!(
            StringEscaper::escape_html("a < b && c > d"),
            "a &lt; b &amp;&amp; c &gt; d"
        );
        assert_eq!(StringEscaper::escape_html("safe"), "safe");
    }

    #[test]
    fn test_escape_shell() {
        assert_eq!(StringEscaper::escape_shell("hello"), "hello");
        assert_eq!(
            StringEscaper::escape_shell("echo $HOME"),
            "echo\\ \\$HOME"
        );
        assert_eq!(
            StringEscaper::escape_shell("it's \"fine\""),
            "it\\'s\\ \\\"fine\\\""
        );
    }

    // -----------------------------------------------------------------------
    // StringFrequencyCounter tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_frequency_counter_basic() {
        let mut counter = StringFrequencyCounter::new();
        counter.add("apple");
        counter.add("banana");
        counter.add("apple");
        counter.add("cherry");
        counter.add("apple");
        counter.add("banana");

        assert_eq!(counter.count("apple"), 3);
        assert_eq!(counter.count("banana"), 2);
        assert_eq!(counter.count("cherry"), 1);
        assert_eq!(counter.count("durian"), 0);
        assert_eq!(counter.unique_count(), 3);
    }

    #[test]
    fn test_frequency_counter_most_common() {
        let mut counter = StringFrequencyCounter::new();
        for _ in 0..5 {
            counter.add("a");
        }
        for _ in 0..3 {
            counter.add("b");
        }
        counter.add("c");

        let top = counter.most_common(2);
        assert_eq!(top, vec![("a".into(), 5), ("b".into(), 3)]);
    }

    #[test]
    fn test_frequency_counter_empty() {
        let counter = StringFrequencyCounter::new();
        assert!(counter.is_empty());
        assert_eq!(counter.unique_count(), 0);
        assert_eq!(counter.count("anything"), 0);
        assert!(counter.most_common(5).is_empty());
    }

    #[test]
    fn strc_lru_insert_get() {
        let mut c = StrCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn strc_lru_eviction() {
        let mut c = StrCLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn strc_lru_hit_ratio() {
        let mut c = StrCLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn strc_lru_clear() {
        let mut c = StrCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn strc_lru_remove() {
        let mut c = StrCLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn strc_lru_peek() {
        let mut c = StrCLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }

    #[test]
    fn strf_fmt_list() {
        let f = StrFFmt::new(StrFFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn strf_fmt_kv() {
        let f = StrFFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn strf_fmt_section() {
        let f = StrFFmt::new(StrFFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn strf_fmt_truncate() {
        let f = StrFFmt::new(StrFFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn strf_fmt_opts_defaults() {
        let o = StrFFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
    }


    #[test]
    fn strings_entry_creation() {
        let e = StringsEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn strings_entry_with_priority() {
        let e = StringsEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn strings_entry_metadata() {
        let e = StringsEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn strings_entry_remove_meta() {
        let mut e = StringsEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn strings_entry_activate_deactivate() {
        let mut e = StringsEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn strings_config_add_sorted() {
        let mut c = StringsConfig::new(10);
        c.add(StringsEntry::new("lo", "Lo").with_priority(1));
        c.add(StringsEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn strings_config_capacity() {
        let mut c = StringsConfig::new(1);
        assert!(c.add(StringsEntry::new("a", "A")));
        assert!(!c.add(StringsEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn strings_config_remove() {
        let mut c = StringsConfig::new(10);
        c.add(StringsEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn strings_config_get() {
        let mut c = StringsConfig::new(10);
        c.add(StringsEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn strings_config_active_entries() {
        let mut c = StringsConfig::new(10);
        c.add(StringsEntry::new("a", "A"));
        c.add(StringsEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn strings_config_enable_disable() {
        let mut c = StringsConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn strings_config_clear() {
        let mut c = StringsConfig::new(10);
        c.add(StringsEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn strings_config_find_by_label() {
        let mut c = StringsConfig::new(10);
        c.add(StringsEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn strings_config_top_n() {
        let mut c = StringsConfig::new(10);
        c.add(StringsEntry::new("a", "A").with_priority(1));
        c.add(StringsEntry::new("b", "B").with_priority(2));
        c.add(StringsEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn strings_config_deactivate_activate_all() {
        let mut c = StringsConfig::new(10);
        c.add(StringsEntry::new("a", "A"));
        c.add(StringsEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn strings_config_highest_priority() {
        let mut c = StringsConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(StringsEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn strings_config_contains() {
        let mut c = StringsConfig::new(10);
        c.add(StringsEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn strings_config_labels() {
        let mut c = StringsConfig::new(10);
        c.add(StringsEntry::new("a", "Alpha"));
        c.add(StringsEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn strings_config_drain_inactive() {
        let mut c = StringsConfig::new(10);
        c.add(StringsEntry::new("a", "A"));
        c.add(StringsEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn qy_metrics_empty() {
        let m = QyMetrics::new("strings");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qy_metrics_record_and_mean() {
        let mut m = QyMetrics::new("strings");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qy_metrics_min_max() {
        let mut m = QyMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qy_metrics_variance_and_std() {
        let mut m = QyMetrics::new("v");
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
    fn qy_metrics_percentile() {
        let mut m = QyMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qy_metrics_merge() {
        let mut a = QyMetrics::new("a");
        a.record(1.0);
        let mut b = QyMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qy_metrics_reset() {
        let mut m = QyMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qy_rate_window_empty() {
        let rw = QyRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qy_rate_window_tick_and_rate() {
        let mut rw = QyRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qy_lru_cache_basic() {
        let mut c = QyLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qy_lru_cache_contains_and_keys() {
        let mut c = QyLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qy_lru_cache_remove() {
        let mut c = QyLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qy_metrics_sum() {
        let mut m = QyMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qy_metrics_label() {
        let m = QyMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qy_lru_cache_clear() {
        let mut c = QyLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_15_push_and_len() {
        let mut rb = super::XbRingBuffer15::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_15_overwrite() {
        let mut rb = super::XbRingBuffer15::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_15_get_out_of_bounds() {
        let rb = super::XbRingBuffer15::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_15_drain_all() {
        let mut rb = super::XbRingBuffer15::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_15_peek_front_back() {
        let mut rb = super::XbRingBuffer15::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_15_clear() {
        let mut rb = super::XbRingBuffer15::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_15_capacity() {
        let rb = super::XbRingBuffer15::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_15_basic() {
        let h = super::xb_fnv1a_15(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_15(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_15_different_inputs() {
        let h1 = super::xb_fnv1a_15(b"abc");
        let h2 = super::xb_fnv1a_15(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_15_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_15(&data);
        let dec = super::xb_rle_decode_15(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_15_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_15(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_15(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_15_values() {
        assert!((super::xb_clamp_15(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_15(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_15(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_15_values() {
        assert!((super::xb_lerp_15(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_15(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_15(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_15_wrap_around_twice() {
        let mut rb = super::XbRingBuffer15::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 168 ----

    #[test]
    fn xc_168_pool_new_empty() {
        let pool: super::Xc168Pool<i32> = super::Xc168Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_168_pool_release_acquire() {
        let mut pool = super::Xc168Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_168_pool_acquire_empty() {
        let mut pool: super::Xc168Pool<i32> = super::Xc168Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_168_pool_full() {
        let mut pool = super::Xc168Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_168_pool_drain() {
        let mut pool = super::Xc168Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_168_pool_stats() {
        let mut pool = super::Xc168Pool::new(8);
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
    fn xc_168_pool_clear() {
        let mut pool = super::Xc168Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_168_pool_shrink() {
        let mut pool = super::Xc168Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_168_pool_default() {
        let pool: super::Xc168Pool<String> = super::Xc168Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_168_pool_extend() {
        let mut pool = super::Xc168Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_168_pool_retain() {
        let mut pool = super::Xc168Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_168_scheduler_round_robin() {
        let mut sched = super::Xc168Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_168_scheduler_empty() {
        let mut sched = super::Xc168Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_168_scheduler_reset() {
        let mut sched = super::Xc168Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_168_scheduler_add_remove() {
        let mut sched = super::Xc168Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_168_scheduler_targets() {
        let sched = super::Xc168Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_168_hash_empty() {
        assert_eq!(super::xc_168_hash(b""), 5381);
    }

    #[test]
    fn xc_168_hash_data() {
        let h = super::xc_168_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_168_hash(b"hello"), h);
    }

    #[test]
    fn xc_168_reverse_str() {
        assert_eq!(super::xc_168_reverse("abc"), "cba");
        assert_eq!(super::xc_168_reverse(""), "");
    }


    #[test]
    fn xe_27_pipeline_empty() {
        let p = super::Xe27Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_27_pipeline_parse_stage() {
        let p = super::Xe27Pipeline::new()
            .add_parse(super::xe_27_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_27_pipeline_transform_double() {
        let p = super::Xe27Pipeline::new()
            .add_transform(super::xe_27_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_27_pipeline_validate_reverse() {
        let p = super::Xe27Pipeline::new()
            .add_validate(super::xe_27_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_27_pipeline_emit_filter() {
        let p = super::Xe27Pipeline::new()
            .add_emit(super::xe_27_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_27_pipeline_multi_stage() {
        let p = super::Xe27Pipeline::new()
            .add_parse(super::xe_27_pipeline_identity)
            .add_transform(super::xe_27_pipeline_double)
            .add_validate(super::xe_27_pipeline_reverse)
            .add_emit(super::xe_27_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_27_pipeline_error_propagation() {
        let p = super::Xe27Pipeline::new()
            .add_parse(super::xe_27_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe27Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_27_pipeline_compose() {
        let p1 = super::Xe27Pipeline::new()
            .add_parse(super::xe_27_pipeline_identity);
        let p2 = super::Xe27Pipeline::new()
            .add_transform(super::xe_27_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_27_pipeline_error_display() {
        let e = super::Xe27PipelineError {
            stage: super::Xe27Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_27_cache_put_get() {
        let mut c = super::Xe27Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_27_cache_miss() {
        let mut c: super::Xe27Cache<&str, i32> = super::Xe27Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_27_cache_ttl_expiry() {
        let mut c = super::Xe27Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_27_cache_evict() {
        let mut c = super::Xe27Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_27_cache_capacity() {
        let mut c = super::Xe27Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_27_cache_stats() {
        let mut c = super::Xe27Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_27_cache_clear() {
        let mut c = super::Xe27Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #112 --

    #[test]
    fn xf112_trie_insert_search() {
        let mut t = Xf112Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf112_trie_starts_with() {
        let mut t = Xf112Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf112_trie_remove() {
        let mut t = Xf112Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf112_trie_word_count() {
        let mut t = Xf112Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf112_trie_longest_prefix() {
        let mut t = Xf112Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf112_trie_all_words() {
        let mut t = Xf112Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf112_trie_autocomplete() {
        let mut t = Xf112Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf112_trie_empty_search() {
        let t = Xf112Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf112_bloom_add_contains() {
        let mut bf = Xf112BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf112_bloom_probably_absent() {
        let bf = Xf112BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf112_bloom_false_positive_rate() {
        let mut bf = Xf112BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf112_bloom_clear() {
        let mut bf = Xf112BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf112_bloom_union() {
        let mut a = Xf112BloomFilter::xf_new(512, 2);
        let mut b = Xf112BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf112_bloom_intersection_estimate() {
        let mut a = Xf112BloomFilter::xf_new(512, 2);
        let mut b = Xf112BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf112_bloom_union_size_mismatch() {
        let a = Xf112BloomFilter::xf_new(256, 2);
        let b = Xf112BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh167_skip_insert_contains() {
        let mut sl = super::Xh167SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh167_skip_remove() {
        let mut sl = super::Xh167SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh167_skip_len() {
        let mut sl = super::Xh167SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh167_skip_range_query() {
        let mut sl = super::Xh167SkipList::xh_new(4);
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
    fn xh167_skip_floor_ceiling() {
        let mut sl = super::Xh167SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh167_skip_rank() {
        let mut sl = super::Xh167SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh167_skip_empty() {
        let sl = super::Xh167SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh167_skip_duplicates() {
        let mut sl = super::Xh167SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh167_bitset_set_test() {
        let mut bs = super::Xh167BitSet::xh_new(256);
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
    fn xh167_bitset_clear_count() {
        let mut bs = super::Xh167BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh167_bitset_and_or_xor() {
        let mut a = super::Xh167BitSet::xh_new(128);
        let mut b = super::Xh167BitSet::xh_new(128);
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
    fn xh167_bitset_iter_ones() {
        let mut bs = super::Xh167BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh167_bitset_first_last() {
        let mut bs = super::Xh167BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh167_bitset_empty() {
        let bs = super::Xh167BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi167_deque_push_pop_back() {
        let mut dq = super::Xi167Deque::xi_new(4);
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
    fn xi167_deque_push_pop_front() {
        let mut dq = super::Xi167Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi167_deque_mixed_ops() {
        let mut dq = super::Xi167Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi167_deque_get_and_split() {
        let mut dq = super::Xi167Deque::xi_new(8);
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
    fn xi167_deque_rotate_left() {
        let mut dq = super::Xi167Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi167_deque_rotate_right() {
        let mut dq = super::Xi167Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi167_deque_grow() {
        let mut dq = super::Xi167Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi167_deque_empty() {
        let dq = super::Xi167Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi167_interval_tree_insert_query() {
        let mut tree = super::Xi167IntervalTree::xi_new();
        tree.xi_insert(super::Xi167Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi167Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi167Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi167_interval_tree_overlap() {
        let mut tree = super::Xi167IntervalTree::xi_new();
        tree.xi_insert(super::Xi167Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi167Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi167Interval::xi_new(12, 20));
        let q = super::Xi167Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi167_interval_tree_remove() {
        let mut tree = super::Xi167IntervalTree::xi_new();
        tree.xi_insert(super::Xi167Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi167Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi167_interval_tree_gaps() {
        let mut tree = super::Xi167IntervalTree::xi_new();
        tree.xi_insert(super::Xi167Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi167Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi167Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi167Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi167Interval::xi_new(8, 10));
    }

    #[test]
    fn xi167_interval_tree_merge() {
        let mut tree = super::Xi167IntervalTree::xi_new();
        tree.xi_insert(super::Xi167Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi167Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi167Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi167Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi167Interval::xi_new(10, 15));
    }

    #[test]
    fn xi167_interval_tree_all() {
        let mut tree = super::Xi167IntervalTree::xi_new();
        tree.xi_insert(super::Xi167Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi167Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi167_interval_tree_empty() {
        let tree = super::Xi167IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi167_interval_tree_contains_point() {
        let iv = super::Xi167Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 167) ---

    #[test]
    fn xj_167_uf_make_and_find() {
        let mut uf = super::Xj167UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_167_uf_union_connected() {
        let mut uf = super::Xj167UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_167_uf_component_count() {
        let mut uf = super::Xj167UnionFind::xj_new();
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
    fn xj_167_uf_component_size() {
        let mut uf = super::Xj167UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_167_uf_largest_component() {
        let mut uf = super::Xj167UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_167_uf_many_elements() {
        let mut uf = super::Xj167UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_167_uf_separate_components() {
        let mut uf = super::Xj167UnionFind::xj_new();
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
    fn xj_167_uf_path_compression() {
        let mut uf = super::Xj167UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_167_bt_insert_get() {
        let mut bt = super::Xj167BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_167_bt_contains_len() {
        let mut bt = super::Xj167BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_167_bt_replace() {
        let mut bt = super::Xj167BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_167_bt_remove() {
        let mut bt = super::Xj167BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_167_bt_keys_values() {
        let mut bt = super::Xj167BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_167_bt_range() {
        let mut bt = super::Xj167BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_167_bt_min_max() {
        let mut bt = super::Xj167BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_167_bt_many_inserts() {
        let mut bt = super::Xj167BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_167 segment tree tests ---

    #[test]
    fn xk_167_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk167SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_167_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk167SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_167_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk167SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_167_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk167SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_167_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk167SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_167_st_single_element() {
        let data = vec![42];
        let st = super::Xk167SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_167_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk167SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_167_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk167SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_167 disjoint intervals tests ---

    #[test]
    fn xk_167_di_add_and_count() {
        let mut di = super::Xk167DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_167_di_merge_overlap() {
        let mut di = super::Xk167DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_167_di_contains() {
        let mut di = super::Xk167DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_167_di_remove() {
        let mut di = super::Xk167DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_167_di_covered_length() {
        let mut di = super::Xk167DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_167_di_gaps() {
        let mut di = super::Xk167DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_167_di_merge_adjacent() {
        let mut di = super::Xk167DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_167_di_empty() {
        let di = super::Xk167DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_167_rope_new_empty() {
        let rope = super::Xl167Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_167_rope_from_str() {
        let rope = super::Xl167Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_167_rope_insert_at() {
        let mut rope = super::Xl167Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_167_rope_delete_range() {
        let mut rope = super::Xl167Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_167_rope_char_at() {
        let rope = super::Xl167Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_167_rope_split_concat() {
        let rope = super::Xl167Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_167_rope_line_count() {
        let rope = super::Xl167Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_167_rope_line_at() {
        let rope = super::Xl167Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_167_sa_build_and_search() {
        let sa = super::Xl167SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_167_sa_count() {
        let sa = super::Xl167SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_167_sa_longest_repeated() {
        let sa = super::Xl167SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_167_sa_all_positions() {
        let sa = super::Xl167SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_167_sa_len() {
        let sa = super::Xl167SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_167_sa_empty() {
        let sa = super::Xl167SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_167_rope_slice() {
        let rope = super::Xl167Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_167_sa_search_start() {
        let sa = super::Xl167SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_167_sparse_set_get() {
        let mut m = super::Xm167MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_167_sparse_row_col() {
        let mut m = super::Xm167MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_167_sparse_transpose() {
        let mut m = super::Xm167MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_167_sparse_multiply_vec() {
        let mut m = super::Xm167MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_167_sparse_nnz_density() {
        let mut m = super::Xm167MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_167_sparse_clear() {
        let mut m = super::Xm167MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_167_sparse_overwrite_zero() {
        let mut m = super::Xm167MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_167_tokenizer_basic() {
        let t = super::Xm167Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_167_tokenizer_count() {
        let t = super::Xm167Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_167_tokenizer_unique() {
        let t = super::Xm167Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_167_tokenizer_frequency() {
        let t = super::Xm167Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_167_tokenizer_delimiter() {
        let t = super::Xm167Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_167_tokenizer_whitespace() {
        let t = super::Xm167Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_167_tokenizer_empty() {
        let t = super::Xm167Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 167 ----

    #[test]
    fn xn_167_fenwick_prefix_sum() {
        let mut ft = super::Xn167Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_167_fenwick_range_sum() {
        let mut ft = super::Xn167Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_167_fenwick_point_query() {
        let mut ft = super::Xn167Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_167_fenwick_len() {
        let ft = super::Xn167Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_167_fenwick_multiple_updates() {
        let mut ft = super::Xn167Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_167_fenwick_single_element() {
        let mut ft = super::Xn167Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_167_fenwick_find_kth() {
        let mut ft = super::Xn167Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_167_fenwick_negative_delta() {
        let mut ft = super::Xn167Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 167 ----

    #[test]
    fn xn_167_avl_insert_get() {
        let mut m = super::Xn167AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_167_avl_remove() {
        let mut m = super::Xn167AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_167_avl_in_order() {
        let mut m = super::Xn167AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_167_avl_min_max() {
        let mut m = super::Xn167AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_167_avl_floor_ceiling() {
        let mut m = super::Xn167AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_167_avl_height_balanced() {
        let mut m = super::Xn167AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_167_avl_overwrite() {
        let mut m = super::Xn167AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_167_avl_empty() {
        let m: super::Xn167AVL<i32, i32> = super::Xn167AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo167RedBlack tests ---

    #[test]
    fn xo_167_rb_insert_and_get() {
        let mut tree = super::Xo167RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_167_rb_len_and_empty() {
        let mut tree = super::Xo167RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_167_rb_min_max() {
        let mut tree = super::Xo167RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_167_rb_contains() {
        let mut tree = super::Xo167RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_167_rb_remove() {
        let mut tree = super::Xo167RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_167_rb_in_order() {
        let mut tree = super::Xo167RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_167_rb_black_height() {
        let mut tree = super::Xo167RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_167_rb_overwrite() {
        let mut tree = super::Xo167RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo167ConsistentHash tests ---

    #[test]
    fn xo_167_ch_add_and_count() {
        let mut ring = super::Xo167ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_167_ch_remove_node() {
        let mut ring = super::Xo167ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_167_ch_get_node() {
        let mut ring = super::Xo167ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_167_ch_empty_ring() {
        let ring = super::Xo167ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_167_ch_distribution() {
        let mut ring = super::Xo167ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_167_ch_rebalance() {
        let mut ring = super::Xo167ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_167_ch_virtual_nodes() {
        let mut ring = super::Xo167ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_167_ch_consistent_lookup() {
        let mut ring = super::Xo167ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }

}