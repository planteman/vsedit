//! String manipulation and unicode utilities.
//!
//! Equivalent to VS Code's `vs/base/common/strings.ts`.

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
}
