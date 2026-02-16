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
        assert_eq!(to_snake_case("PascalCase"), "_pascal_case");
        assert_eq!(to_snake_case("already_snake"), "already_snake");
        assert_eq!(to_snake_case("HTMLParser"), "_h_t_m_l_parser");
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
}
