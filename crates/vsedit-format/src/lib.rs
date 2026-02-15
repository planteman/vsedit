//! Document formatting support.

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
}
