//! String manipulation and unicode utilities.
//!
//! Equivalent to VS Code's `vs/base/common/strings.ts`.

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
}
