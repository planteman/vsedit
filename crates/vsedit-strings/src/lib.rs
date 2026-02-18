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

}