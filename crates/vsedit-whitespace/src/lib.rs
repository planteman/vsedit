//! Whitespace visualization and trimming.

use std::fmt;

/// Errors that can occur during whitespace operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WhitespaceError {
    /// Tab size must be at least 1.
    InvalidTabSize(usize),
    /// The indentation width is not a multiple of the configured unit.
    InconsistentIndentation { line: usize, found: usize, expected_multiple: usize },
    /// Mixed tabs and spaces in indentation.
    MixedIndentation { line: usize },
}

impl fmt::Display for WhitespaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WhitespaceError::InvalidTabSize(s) => {
                write!(f, "invalid tab size {s}: must be >= 1")
            }
            WhitespaceError::InconsistentIndentation { line, found, expected_multiple } => {
                write!(
                    f,
                    "line {line}: indentation width {found} is not a multiple of {expected_multiple}"
                )
            }
            WhitespaceError::MixedIndentation { line } => {
                write!(f, "line {line}: mixed tabs and spaces in indentation")
            }
        }
    }
}

impl std::error::Error for WhitespaceError {}

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

impl fmt::Display for WhitespaceMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            WhitespaceMode::None => "none",
            WhitespaceMode::Boundary => "boundary",
            WhitespaceMode::Selection => "selection",
            WhitespaceMode::Trailing => "trailing",
            WhitespaceMode::All => "all",
        };
        f.write_str(label)
    }
}

/// The kind of whitespace character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhitespaceKind {
    Space,
    Tab,
    /// Non-breaking space (U+00A0).
    Nbsp,
}

impl fmt::Display for WhitespaceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WhitespaceKind::Space => f.write_str("space"),
            WhitespaceKind::Tab => f.write_str("tab"),
            WhitespaceKind::Nbsp => f.write_str("nbsp"),
        }
    }
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

/// Replace leading spaces with tabs where possible.
pub fn spaces_to_tabs(text: &str, tab_size: usize) -> String {
    text.lines()
        .map(|line| {
            let indent_len = line.len() - line.trim_start_matches(' ').len();
            let full_tabs = indent_len / tab_size;
            let remaining = indent_len % tab_size;
            let mut out = String::new();
            for _ in 0..full_tabs {
                out.push('\t');
            }
            for _ in 0..remaining {
                out.push(' ');
            }
            out.push_str(&line[indent_len..]);
            out
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Configuration builder for whitespace processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhitespaceConfig {
    pub tab_size: usize,
    pub trim_trailing: bool,
    pub ensure_final_newline: bool,
    pub trim_final_newlines: bool,
    pub insert_spaces: bool,
}

impl Default for WhitespaceConfig {
    fn default() -> Self {
        Self {
            tab_size: 4,
            trim_trailing: true,
            ensure_final_newline: true,
            trim_final_newlines: false,
            insert_spaces: true,
        }
    }
}

impl WhitespaceConfig {
    /// Create a new builder starting from defaults.
    pub fn builder() -> WhitespaceConfigBuilder {
        WhitespaceConfigBuilder(WhitespaceConfig::default())
    }

    /// Validate the configuration, returning an error if invalid.
    pub fn validate(&self) -> Result<(), WhitespaceError> {
        if self.tab_size == 0 {
            return Err(WhitespaceError::InvalidTabSize(0));
        }
        Ok(())
    }

    /// Apply all configured transformations to the input text.
    pub fn apply(&self, text: &str) -> Result<String, WhitespaceError> {
        self.validate()?;
        let mut result = text.to_string();

        if self.insert_spaces {
            result = tabs_to_spaces(&result, self.tab_size);
        } else {
            result = spaces_to_tabs(&result, self.tab_size);
        }

        if self.trim_trailing {
            result = trim_trailing_whitespace(&result);
        }

        if self.trim_final_newlines {
            result = crate::trim_final_newlines(&result);
        } else if self.ensure_final_newline {
            result = crate::ensure_final_newline(&result);
        }

        Ok(result)
    }
}

impl fmt::Display for WhitespaceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "tab_size={}, spaces={}, trim_trail={}, final_nl={}",
            self.tab_size, self.insert_spaces, self.trim_trailing, self.ensure_final_newline
        )
    }
}

/// Builder for [`WhitespaceConfig`].
#[derive(Debug, Clone)]
pub struct WhitespaceConfigBuilder(WhitespaceConfig);

impl WhitespaceConfigBuilder {
    pub fn tab_size(mut self, size: usize) -> Self {
        self.0.tab_size = size;
        self
    }

    pub fn trim_trailing(mut self, enabled: bool) -> Self {
        self.0.trim_trailing = enabled;
        self
    }

    pub fn ensure_final_newline(mut self, enabled: bool) -> Self {
        self.0.ensure_final_newline = enabled;
        self
    }

    pub fn trim_final_newlines(mut self, enabled: bool) -> Self {
        self.0.trim_final_newlines = enabled;
        self
    }

    pub fn insert_spaces(mut self, enabled: bool) -> Self {
        self.0.insert_spaces = enabled;
        self
    }

    /// Build and validate the configuration.
    pub fn build(self) -> Result<WhitespaceConfig, WhitespaceError> {
        self.0.validate()?;
        Ok(self.0)
    }
}

/// Compute the indentation level (number of leading whitespace columns) of a line.
pub fn indentation_width(line: &str, tab_size: usize) -> usize {
    let mut col = 0;
    for ch in line.chars() {
        match ch {
            ' ' => col += 1,
            '\t' => col += tab_size - (col % tab_size),
            _ => break,
        }
    }
    col
}

/// Check that every line's indentation is a multiple of `indent_size` columns.
pub fn check_indentation_consistency(
    text: &str,
    tab_size: usize,
    indent_size: usize,
) -> Result<(), WhitespaceError> {
    if tab_size == 0 {
        return Err(WhitespaceError::InvalidTabSize(0));
    }
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let width = indentation_width(line, tab_size);
        if width % indent_size != 0 {
            return Err(WhitespaceError::InconsistentIndentation {
                line: i + 1,
                found: width,
                expected_multiple: indent_size,
            });
        }
    }
    Ok(())
}

/// Detect whether a line uses mixed tabs and spaces in its leading whitespace.
pub fn has_mixed_indentation(line: &str) -> bool {
    let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
    if indent.is_empty() {
        return false;
    }
    let has_tab = indent.contains('\t');
    let has_space = indent.contains(' ');
    has_tab && has_space
}

/// Scan all lines and return errors for any that have mixed indentation.
pub fn check_mixed_indentation(text: &str) -> Vec<WhitespaceError> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| has_mixed_indentation(line))
        .map(|(i, _)| WhitespaceError::MixedIndentation { line: i + 1 })
        .collect()
}

/// Summary statistics for whitespace in a document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WhitespaceStats {
    pub total_lines: usize,
    pub blank_lines: usize,
    pub lines_with_trailing: usize,
    pub total_trailing_chars: usize,
    pub tab_count: usize,
    pub space_count: usize,
    pub nbsp_count: usize,
}

impl fmt::Display for WhitespaceStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} lines ({} blank), {} trailing ws on {} lines, tabs={} spaces={} nbsp={}",
            self.total_lines,
            self.blank_lines,
            self.total_trailing_chars,
            self.lines_with_trailing,
            self.tab_count,
            self.space_count,
            self.nbsp_count,
        )
    }
}

/// Gather whitespace statistics for an entire document.
pub fn compute_stats(text: &str, tab_size: usize) -> WhitespaceStats {
    let mut stats = WhitespaceStats::default();
    for line in text.lines() {
        stats.total_lines += 1;
        if line.trim().is_empty() {
            stats.blank_lines += 1;
        }
        let trailing = trailing_whitespace_count(line);
        if trailing > 0 {
            stats.lines_with_trailing += 1;
            stats.total_trailing_chars += trailing;
        }
        for wc in detect_whitespace(line, tab_size) {
            match wc.kind {
                WhitespaceKind::Space => stats.space_count += 1,
                WhitespaceKind::Tab => stats.tab_count += 1,
                WhitespaceKind::Nbsp => stats.nbsp_count += 1,
            }
        }
    }
    stats
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

    #[test]
    fn whitespace_mode_display() {
        assert_eq!(WhitespaceMode::None.to_string(), "none");
        assert_eq!(WhitespaceMode::All.to_string(), "all");
        assert_eq!(WhitespaceMode::Trailing.to_string(), "trailing");
    }

    #[test]
    fn whitespace_kind_display() {
        assert_eq!(WhitespaceKind::Space.to_string(), "space");
        assert_eq!(WhitespaceKind::Tab.to_string(), "tab");
        assert_eq!(WhitespaceKind::Nbsp.to_string(), "nbsp");
    }

    #[test]
    fn whitespace_error_display() {
        let e = WhitespaceError::InvalidTabSize(0);
        assert_eq!(e.to_string(), "invalid tab size 0: must be >= 1");

        let e2 = WhitespaceError::MixedIndentation { line: 3 };
        assert!(e2.to_string().contains("line 3"));
    }

    #[test]
    fn config_builder_defaults() {
        let cfg = WhitespaceConfig::builder().build().unwrap();
        assert_eq!(cfg.tab_size, 4);
        assert!(cfg.trim_trailing);
        assert!(cfg.ensure_final_newline);
        assert!(cfg.insert_spaces);
    }

    #[test]
    fn config_builder_custom() {
        let cfg = WhitespaceConfig::builder()
            .tab_size(2)
            .trim_trailing(false)
            .insert_spaces(false)
            .build()
            .unwrap();
        assert_eq!(cfg.tab_size, 2);
        assert!(!cfg.trim_trailing);
        assert!(!cfg.insert_spaces);
    }

    #[test]
    fn config_builder_invalid_tab_size() {
        let result = WhitespaceConfig::builder().tab_size(0).build();
        assert_eq!(result, Err(WhitespaceError::InvalidTabSize(0)));
    }

    #[test]
    fn config_apply_trims_and_converts() {
        let cfg = WhitespaceConfig::builder()
            .tab_size(4)
            .trim_trailing(true)
            .ensure_final_newline(true)
            .build()
            .unwrap();
        let out = cfg.apply("\thello   \nworld  ").unwrap();
        assert_eq!(out, "    hello\nworld\n");
    }

    #[test]
    fn spaces_to_tabs_conversion() {
        assert_eq!(spaces_to_tabs("        hello", 4), "\t\thello");
        assert_eq!(spaces_to_tabs("      hello", 4), "\t  hello");
    }

    #[test]
    fn indentation_width_spaces() {
        assert_eq!(indentation_width("    hello", 4), 4);
        assert_eq!(indentation_width("\thello", 4), 4);
        assert_eq!(indentation_width("  \thello", 4), 4);
    }

    #[test]
    fn check_indentation_consistency_ok() {
        let text = "    line1\n        line2\nline3\n";
        assert!(check_indentation_consistency(text, 4, 4).is_ok());
    }

    #[test]
    fn check_indentation_consistency_err() {
        let text = "   bad_indent\n";
        let err = check_indentation_consistency(text, 4, 4);
        assert!(matches!(err, Err(WhitespaceError::InconsistentIndentation { .. })));
    }

    #[test]
    fn mixed_indentation_detection() {
        assert!(has_mixed_indentation("\t hello"));
        assert!(!has_mixed_indentation("\thello"));
        assert!(!has_mixed_indentation("    hello"));
        assert!(!has_mixed_indentation("hello"));
    }

    #[test]
    fn check_mixed_indentation_report() {
        let text = "    good\n\t bad\n\talso_good\n";
        let errors = check_mixed_indentation(text);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0], WhitespaceError::MixedIndentation { line: 2 });
    }

    #[test]
    fn compute_stats_basic() {
        let text = "hello   \n\n\tworld\n";
        let stats = compute_stats(text, 4);
        assert_eq!(stats.total_lines, 3);
        assert_eq!(stats.blank_lines, 1);
        assert_eq!(stats.lines_with_trailing, 1);
        assert_eq!(stats.total_trailing_chars, 3);
        assert_eq!(stats.tab_count, 1);
        assert_eq!(stats.space_count, 3);
    }

    #[test]
    fn whitespace_stats_display() {
        let stats = WhitespaceStats {
            total_lines: 10,
            blank_lines: 2,
            lines_with_trailing: 1,
            total_trailing_chars: 3,
            tab_count: 0,
            space_count: 5,
            nbsp_count: 0,
        };
        let s = stats.to_string();
        assert!(s.contains("10 lines"));
        assert!(s.contains("2 blank"));
    }

    #[test]
    fn config_display() {
        let cfg = WhitespaceConfig::default();
        let s = cfg.to_string();
        assert!(s.contains("tab_size=4"));
        assert!(s.contains("spaces=true"));
    }
}
