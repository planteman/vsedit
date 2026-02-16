//! Whitespace visualization and trimming.

/// Whitespace rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhitespaceMode {
    /// Don't render whitespace characters.
    None,
    /// Render only at word boundaries.
    Boundary,
    /// Render only in selected text.
    Selection,
    /// Render only trailing whitespace.
    Trailing,
    /// Render all whitespace characters.
    All,
}

/// The kind of whitespace character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhitespaceKind {
    Space,
    Tab,
    /// Non-breaking space (U+00A0).
    Nbsp,
}

/// A single detected whitespace character with its position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhitespaceChar {
    /// Byte offset within the line.
    pub offset: usize,
    pub kind: WhitespaceKind,
    /// Display width in columns.
    pub width: usize,
}

/// Detect whitespace characters in a single line.
pub fn detect_whitespace(line: &str, tab_size: usize) -> Vec<WhitespaceChar> {
    let mut result = Vec::new();
    let mut col = 0;
    for (offset, ch) in line.char_indices() {
        match ch {
            ' ' => {
                result.push(WhitespaceChar {
                    offset,
                    kind: WhitespaceKind::Space,
                    width: 1,
                });
                col += 1;
            }
            '\t' => {
                let w = tab_size - (col % tab_size);
                result.push(WhitespaceChar {
                    offset,
                    kind: WhitespaceKind::Tab,
                    width: w,
                });
                col += w;
            }
            '\u{00A0}' => {
                result.push(WhitespaceChar {
                    offset,
                    kind: WhitespaceKind::Nbsp,
                    width: 1,
                });
                col += 1;
            }
            _ => {
                col += 1;
            }
        }
    }
    result
}

/// Convert whitespace characters to visible glyphs: `·` for space, `→` for tab, `°` for NBSP.
pub fn render_whitespace_chars(chars: &[WhitespaceChar]) -> Vec<(usize, String)> {
    chars
        .iter()
        .map(|wc| {
            let glyph = match wc.kind {
                WhitespaceKind::Space => "·".to_string(),
                WhitespaceKind::Tab => {
                    let mut s = "→".to_string();
                    for _ in 1..wc.width {
                        s.push(' ');
                    }
                    s
                }
                WhitespaceKind::Nbsp => "°".to_string(),
            };
            (wc.offset, glyph)
        })
        .collect()
}

/// Detect trailing whitespace on a single line. Returns `Some((start_offset, count))`.
pub fn detect_trailing_whitespace(line: &str) -> Option<(usize, usize)> {
    let trimmed = line.trim_end();
    let count = line.len() - trimmed.len();
    if count > 0 {
        Some((trimmed.len(), count))
    } else {
        None
    }
}

/// Trim trailing whitespace from all lines.
pub fn trim_trailing_whitespace(text: &str) -> String {
    text.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Ensure file ends with exactly one newline.
pub fn ensure_final_newline(text: &str) -> String {
    let trimmed = text.trim_end_matches('\n');
    format!("{}\n", trimmed)
}

/// Trim final empty lines.
pub fn trim_final_newlines(text: &str) -> String {
    let mut result = text.trim_end().to_string();
    if !result.is_empty() {
        result.push('\n');
    }
    result
}

/// Count trailing whitespace characters on a line.
pub fn trailing_whitespace_count(line: &str) -> usize {
    line.len() - line.trim_end().len()
}

/// Replace tabs with spaces.
pub fn tabs_to_spaces(text: &str, tab_size: usize) -> String {
    let mut result = String::with_capacity(text.len());
    let mut col = 0;
    for ch in text.chars() {
        if ch == '\t' {
            let spaces = tab_size - (col % tab_size);
            for _ in 0..spaces {
                result.push(' ');
            }
            col += spaces;
        } else if ch == '\n' {
            result.push(ch);
            col = 0;
        } else {
            result.push(ch);
            col += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_trailing() {
        assert_eq!(trim_trailing_whitespace("hello   \nworld  "), "hello\nworld");
    }

    #[test]
    fn final_newline() {
        assert_eq!(ensure_final_newline("hello"), "hello\n");
        assert_eq!(ensure_final_newline("hello\n\n"), "hello\n");
    }

    #[test]
    fn trim_final() {
        assert_eq!(trim_final_newlines("hello\n\n\n"), "hello\n");
    }

    #[test]
    fn trailing_count() {
        assert_eq!(trailing_whitespace_count("hello   "), 3);
        assert_eq!(trailing_whitespace_count("hello"), 0);
    }

    #[test]
    fn tab_conversion() {
        assert_eq!(tabs_to_spaces("\thello", 4), "    hello");
        assert_eq!(tabs_to_spaces("ab\tcd", 4), "ab  cd");
    }

    #[test]
    fn detect_whitespace_spaces_and_tabs() {
        let chars = detect_whitespace("a b\tc", 4);
        assert_eq!(chars.len(), 2);
        assert_eq!(chars[0].kind, WhitespaceKind::Space);
        assert_eq!(chars[0].offset, 1);
        assert_eq!(chars[1].kind, WhitespaceKind::Tab);
    }

    #[test]
    fn detect_whitespace_nbsp() {
        let line = "a\u{00A0}b";
        let chars = detect_whitespace(line, 4);
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].kind, WhitespaceKind::Nbsp);
    }

    #[test]
    fn detect_whitespace_empty() {
        let chars = detect_whitespace("hello", 4);
        assert!(chars.is_empty());
    }

    #[test]
    fn render_whitespace_glyphs() {
        let chars = vec![
            WhitespaceChar { offset: 0, kind: WhitespaceKind::Space, width: 1 },
            WhitespaceChar { offset: 1, kind: WhitespaceKind::Tab, width: 4 },
            WhitespaceChar { offset: 5, kind: WhitespaceKind::Nbsp, width: 1 },
        ];
        let rendered = render_whitespace_chars(&chars);
        assert_eq!(rendered[0].1, "·");
        assert_eq!(rendered[1].1, "→   ");
        assert_eq!(rendered[2].1, "°");
    }

    #[test]
    fn detect_trailing_whitespace_found() {
        let result = detect_trailing_whitespace("hello   ");
        assert_eq!(result, Some((5, 3)));
    }

    #[test]
    fn detect_trailing_whitespace_none() {
        assert_eq!(detect_trailing_whitespace("hello"), None);
    }

    #[test]
    fn whitespace_mode_enum_exists() {
        // Ensure all modes are constructible
        let modes = [
            WhitespaceMode::None,
            WhitespaceMode::Boundary,
            WhitespaceMode::Selection,
            WhitespaceMode::Trailing,
            WhitespaceMode::All,
        ];
        assert_eq!(modes.len(), 5);
    }
}
