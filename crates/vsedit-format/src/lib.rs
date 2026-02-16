//! Document formatting support.

use std::fmt;

/// A text edit from formatting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub new_text: String,
}

/// Options for formatting.
#[derive(Debug, Clone)]
pub struct FormattingOptions {
    pub tab_size: u32,
    pub insert_spaces: bool,
    pub trim_trailing_whitespace: bool,
    pub insert_final_newline: bool,
    pub trim_final_newlines: bool,
}

impl Default for FormattingOptions {
    fn default() -> Self {
        Self {
            tab_size: 4,
            insert_spaces: true,
            trim_trailing_whitespace: true,
            insert_final_newline: true,
            trim_final_newlines: true,
        }
    }
}

/// Provider for document formatting.
pub trait DocumentFormattingProvider: Send + Sync {
    fn format_document(&self, uri: &str, options: &FormattingOptions) -> Vec<TextEdit>;
}

/// Provider for range formatting.
pub trait DocumentRangeFormattingProvider: Send + Sync {
    fn format_range(&self, uri: &str, start_line: u32, end_line: u32, options: &FormattingOptions) -> Vec<TextEdit>;
}

/// Provider for on-type formatting.
pub trait OnTypeFormattingProvider: Send + Sync {
    fn trigger_characters(&self) -> Vec<char>;
    fn format_on_type(&self, uri: &str, line: u32, column: u32, ch: char, options: &FormattingOptions) -> Vec<TextEdit>;
}

/// Apply basic whitespace formatting.
pub fn format_whitespace(text: &str, options: &FormattingOptions) -> String {
    let mut lines: Vec<String> = text.lines().map(|l| {
        let mut line = l.to_string();
        if options.trim_trailing_whitespace {
            line = line.trim_end().to_string();
        }
        line
    }).collect();

    if options.trim_final_newlines {
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
    }

    let mut result = lines.join("\n");
    if options.insert_final_newline && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// The kind of edit operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    /// Insert new text at a position (start == end).
    Insert,
    /// Delete text in a range (new_text is empty).
    Delete,
    /// Replace text in a range with new text.
    Replace,
}

impl TextEdit {
    /// Determine the kind of this edit.
    pub fn kind(&self) -> EditKind {
        let same_pos = self.start_line == self.end_line && self.start_column == self.end_column;
        if same_pos {
            EditKind::Insert
        } else if self.new_text.is_empty() {
            EditKind::Delete
        } else {
            EditKind::Replace
        }
    }

    /// Apply this single edit to the given text, returning the result.
    pub fn apply(&self, text: &str) -> String {
        let lines: Vec<&str> = text.lines().collect();
        let mut result = String::new();

        for (i, line) in lines.iter().enumerate() {
            let li = i as u32;
            if li < self.start_line || li > self.end_line {
                result.push_str(line);
                result.push('\n');
            } else if li == self.start_line && li == self.end_line {
                let col_s = self.start_column as usize;
                let col_e = self.end_column as usize;
                result.push_str(&line[..col_s.min(line.len())]);
                result.push_str(&self.new_text);
                result.push_str(&line[col_e.min(line.len())..]);
                result.push('\n');
            } else if li == self.start_line {
                let col_s = self.start_column as usize;
                result.push_str(&line[..col_s.min(line.len())]);
                result.push_str(&self.new_text);
            } else if li == self.end_line {
                let col_e = self.end_column as usize;
                result.push_str(&line[col_e.min(line.len())..]);
                result.push('\n');
            }
            // lines strictly between start and end are dropped (replaced)
        }

        // Preserve trailing content if text had no trailing newline
        if !text.ends_with('\n') && result.ends_with('\n') {
            result.pop();
        }
        result
    }
}

impl fmt::Display for TextEdit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}:{}-{}:{}] {:?} \"{}\"",
            self.start_line,
            self.start_column,
            self.end_line,
            self.end_column,
            self.kind(),
            self.new_text,
        )
    }
}

/// Apply multiple edits to text. Edits are sorted by position in reverse
/// order so that earlier edits don't shift the positions of later ones.
pub fn apply_edits(text: &str, edits: &[TextEdit]) -> String {
    let mut sorted: Vec<&TextEdit> = edits.iter().collect();
    sorted.sort_by(|a, b| {
        b.start_line
            .cmp(&a.start_line)
            .then(b.start_column.cmp(&a.start_column))
    });

    let mut result = text.to_string();
    for edit in sorted {
        result = edit.apply(&result);
    }
    result
}

/// Builder for `FormattingOptions`.
#[derive(Debug, Clone)]
pub struct FormattingOptionsBuilder {
    options: FormattingOptions,
}

impl FormattingOptionsBuilder {
    pub fn new() -> Self {
        Self {
            options: FormattingOptions::default(),
        }
    }

    pub fn tab_size(mut self, size: u32) -> Self {
        self.options.tab_size = size;
        self
    }

    pub fn insert_spaces(mut self, yes: bool) -> Self {
        self.options.insert_spaces = yes;
        self
    }

    pub fn trim_trailing_whitespace(mut self, yes: bool) -> Self {
        self.options.trim_trailing_whitespace = yes;
        self
    }

    pub fn insert_final_newline(mut self, yes: bool) -> Self {
        self.options.insert_final_newline = yes;
        self
    }

    pub fn trim_final_newlines(mut self, yes: bool) -> Self {
        self.options.trim_final_newlines = yes;
        self
    }

    pub fn build(self) -> FormattingOptions {
        self.options
    }
}

impl Default for FormattingOptionsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert all leading tabs to spaces using the given tab size.
pub fn convert_tabs_to_spaces(text: &str, tab_size: u32) -> String {
    let spaces: String = " ".repeat(tab_size as usize);
    text.lines()
        .map(|line| {
            let leading_tabs = line.len() - line.trim_start_matches('\t').len();
            if leading_tabs > 0 {
                format!("{}{}", spaces.repeat(leading_tabs), &line[leading_tabs..])
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Convert leading spaces (in multiples of tab_size) to tabs.
pub fn convert_spaces_to_tabs(text: &str, tab_size: u32) -> String {
    let ts = tab_size as usize;
    text.lines()
        .map(|line| {
            let leading_spaces = line.len() - line.trim_start_matches(' ').len();
            let tab_count = leading_spaces / ts;
            let remaining_spaces = leading_spaces % ts;
            if tab_count > 0 {
                format!(
                    "{}{}{}",
                    "\t".repeat(tab_count),
                    " ".repeat(remaining_spaces),
                    &line[leading_spaces..]
                )
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_trims_trailing() {
        let result = format_whitespace("hello   \nworld  \n", &FormattingOptions::default());
        assert_eq!(result, "hello\nworld\n");
    }

    #[test]
    fn format_inserts_final_newline() {
        let result = format_whitespace("hello", &FormattingOptions::default());
        assert!(result.ends_with('\n'));
    }

    #[test]
    fn format_trims_final_newlines() {
        let result = format_whitespace("hello\n\n\n", &FormattingOptions::default());
        assert_eq!(result, "hello\n");
    }

    #[test]
    fn formatting_options_default() {
        let opts = FormattingOptions::default();
        assert_eq!(opts.tab_size, 4);
        assert!(opts.insert_spaces);
    }

    #[test]
    fn edit_kind_insert() {
        let edit = TextEdit {
            start_line: 0, start_column: 5, end_line: 0, end_column: 5,
            new_text: "world".into(),
        };
        assert_eq!(edit.kind(), EditKind::Insert);
    }

    #[test]
    fn edit_kind_delete() {
        let edit = TextEdit {
            start_line: 0, start_column: 0, end_line: 0, end_column: 5,
            new_text: String::new(),
        };
        assert_eq!(edit.kind(), EditKind::Delete);
    }

    #[test]
    fn edit_kind_replace() {
        let edit = TextEdit {
            start_line: 0, start_column: 0, end_line: 0, end_column: 5,
            new_text: "hi".into(),
        };
        assert_eq!(edit.kind(), EditKind::Replace);
    }

    #[test]
    fn apply_single_edit() {
        let edit = TextEdit {
            start_line: 0, start_column: 5, end_line: 0, end_column: 5,
            new_text: " world".into(),
        };
        assert_eq!(edit.apply("hello"), "hello world");
    }

    #[test]
    fn apply_multiple_edits() {
        let text = "aaa bbb ccc";
        let edits = vec![
            TextEdit {
                start_line: 0, start_column: 0, end_line: 0, end_column: 3,
                new_text: "AAA".into(),
            },
            TextEdit {
                start_line: 0, start_column: 8, end_line: 0, end_column: 11,
                new_text: "CCC".into(),
            },
        ];
        assert_eq!(apply_edits(text, &edits), "AAA bbb CCC");
    }

    #[test]
    fn text_edit_display() {
        let edit = TextEdit {
            start_line: 1, start_column: 0, end_line: 1, end_column: 3,
            new_text: "foo".into(),
        };
        let s = format!("{}", edit);
        assert!(s.contains("1:0-1:3"));
        assert!(s.contains("Replace"));
    }

    #[test]
    fn builder_pattern() {
        let opts = FormattingOptionsBuilder::new()
            .tab_size(2)
            .insert_spaces(false)
            .trim_trailing_whitespace(false)
            .build();
        assert_eq!(opts.tab_size, 2);
        assert!(!opts.insert_spaces);
        assert!(!opts.trim_trailing_whitespace);
    }

    #[test]
    fn tabs_to_spaces() {
        let input = "\t\thello";
        let result = convert_tabs_to_spaces(input, 4);
        assert_eq!(result, "        hello");
    }

    #[test]
    fn spaces_to_tabs() {
        let input = "        hello";
        let result = convert_spaces_to_tabs(input, 4);
        assert_eq!(result, "\t\thello");
    }
}
