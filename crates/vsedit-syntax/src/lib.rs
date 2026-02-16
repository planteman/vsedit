use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

/// A colored span of text within a line.
#[derive(Debug, Clone)]
pub struct ColoredSpan {
    pub text: String,
    pub fg: (u8, u8, u8),
    pub bg: (u8, u8, u8),
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

/// Syntax highlighter backed by syntect.
pub struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme: Theme,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set.themes["base16-ocean.dark"].clone();
        Self { syntax_set, theme }
    }

    /// Get the syntax definition for a file by extension.
    pub fn syntax_for_file(&self, filename: &str) -> Option<&SyntaxReference> {
        self.syntax_set.find_syntax_for_file(filename).ok().flatten()
    }

    /// Highlight a single line of code, returning colored spans.
    pub fn highlight_line(&self, line: &str, syntax: &SyntaxReference) -> Vec<ColoredSpan> {
        let mut h = HighlightLines::new(syntax, &self.theme);
        match h.highlight_line(line, &self.syntax_set) {
            Ok(ranges) => ranges
                .iter()
                .map(|(style, text)| ColoredSpan {
                    text: text.to_string(),
                    fg: (style.foreground.r, style.foreground.g, style.foreground.b),
                    bg: (style.background.r, style.background.g, style.background.b),
                    bold: style.font_style.contains(FontStyle::BOLD),
                    italic: style.font_style.contains(FontStyle::ITALIC),
                    underline: style.font_style.contains(FontStyle::UNDERLINE),
                })
                .collect(),
            Err(_) => vec![ColoredSpan {
                text: line.to_string(),
                fg: (255, 255, 255),
                bg: (0, 0, 0),
                bold: false,
                italic: false,
                underline: false,
            }],
        }
    }

    /// Highlight multiple lines (full document).
    pub fn highlight_lines(
        &self,
        lines: &[&str],
        syntax: &SyntaxReference,
    ) -> Vec<Vec<ColoredSpan>> {
        let mut h = HighlightLines::new(syntax, &self.theme);
        lines
            .iter()
            .map(|line| {
                match h.highlight_line(line, &self.syntax_set) {
                    Ok(ranges) => ranges
                        .iter()
                        .map(|(style, text)| ColoredSpan {
                            text: text.to_string(),
                            fg: (style.foreground.r, style.foreground.g, style.foreground.b),
                            bg: (style.background.r, style.background.g, style.background.b),
                            bold: style.font_style.contains(FontStyle::BOLD),
                            italic: style.font_style.contains(FontStyle::ITALIC),
                            underline: style.font_style.contains(FontStyle::UNDERLINE),
                        })
                        .collect(),
                    Err(_) => vec![ColoredSpan {
                        text: line.to_string(),
                        fg: (255, 255, 255),
                        bg: (0, 0, 0),
                        bold: false,
                        italic: false,
                        underline: false,
                    }],
                }
            })
            .collect()
    }

    /// List available syntax names.
    pub fn available_syntaxes(&self) -> Vec<&str> {
        self.syntax_set
            .syntaxes()
            .iter()
            .map(|s| s.name.as_str())
            .collect()
    }

    /// List available theme names.
    pub fn available_themes() -> Vec<String> {
        ThemeSet::load_defaults()
            .themes
            .keys()
            .cloned()
            .collect()
    }

    /// Set the active theme by name.
    pub fn set_theme(&mut self, name: &str) {
        let ts = ThemeSet::load_defaults();
        if let Some(theme) = ts.themes.get(name) {
            self.theme = theme.clone();
        }
    }
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_syntaxes_includes_rust() {
        let hl = SyntaxHighlighter::new();
        let syntaxes = hl.available_syntaxes();
        assert!(syntaxes.contains(&"Rust"), "should contain Rust syntax");
    }

    #[test]
    fn available_syntaxes_includes_javascript() {
        let hl = SyntaxHighlighter::new();
        let syntaxes = hl.available_syntaxes();
        assert!(
            syntaxes.contains(&"JavaScript"),
            "should contain JavaScript syntax"
        );
    }

    #[test]
    fn available_syntaxes_is_non_empty() {
        let hl = SyntaxHighlighter::new();
        assert!(!hl.available_syntaxes().is_empty());
    }

    #[test]
    fn syntax_for_rust_file() {
        let hl = SyntaxHighlighter::new();
        let syn = hl.syntax_for_file("main.rs");
        assert!(syn.is_some(), "should find syntax for .rs files");
        assert_eq!(syn.unwrap().name, "Rust");
    }

    #[test]
    fn syntax_for_unknown_file_returns_none() {
        let hl = SyntaxHighlighter::new();
        let syn = hl.syntax_for_file("file.zzzzunknown");
        assert!(syn.is_none(), "unknown extension should return None");
    }

    #[test]
    fn highlight_rust_line_produces_spans() {
        let hl = SyntaxHighlighter::new();
        let syntax = hl.syntax_for_file("test.rs").unwrap();
        let spans = hl.highlight_line("fn main() {}\n", syntax);
        assert!(!spans.is_empty(), "should produce at least one span");
        let combined: String = spans.iter().map(|s| s.text.as_str()).collect();
        assert!(combined.contains("fn"), "combined text should contain 'fn'");
    }

    #[test]
    fn highlight_line_fg_colors_are_set() {
        let hl = SyntaxHighlighter::new();
        let syntax = hl.syntax_for_file("test.rs").unwrap();
        let spans = hl.highlight_line("let x = 42;\n", syntax);
        // At least one span should have a non-black foreground
        let has_color = spans.iter().any(|s| s.fg != (0, 0, 0));
        assert!(has_color, "should have non-black foreground colors");
    }

    #[test]
    fn highlight_lines_multiple() {
        let hl = SyntaxHighlighter::new();
        let syntax = hl.syntax_for_file("test.rs").unwrap();
        let lines = vec!["fn main() {\n", "    println!(\"hello\");\n", "}\n"];
        let result = hl.highlight_lines(&lines, syntax);
        assert_eq!(result.len(), 3, "should return one vec per line");
        for line_spans in &result {
            assert!(!line_spans.is_empty());
        }
    }

    #[test]
    fn available_themes_non_empty() {
        let themes = SyntaxHighlighter::available_themes();
        assert!(!themes.is_empty(), "should have at least one theme");
    }

    #[test]
    fn set_theme_changes_theme() {
        let mut hl = SyntaxHighlighter::new();
        let themes = SyntaxHighlighter::available_themes();
        // Pick a theme different from default if possible
        if let Some(name) = themes.iter().find(|t| *t != "base16-ocean.dark") {
            hl.set_theme(name);
            // Verify highlighting still works after theme change
            let syntax = hl.syntax_for_file("test.rs").unwrap();
            let spans = hl.highlight_line("let x = 1;\n", syntax);
            assert!(!spans.is_empty());
        }
    }

    #[test]
    fn set_theme_ignores_unknown() {
        let mut hl = SyntaxHighlighter::new();
        let syntax = hl.syntax_for_file("test.rs").unwrap();
        let before = hl.highlight_line("let x = 1;\n", syntax);
        hl.set_theme("nonexistent-theme-12345");
        let syntax = hl.syntax_for_file("test.rs").unwrap();
        let after = hl.highlight_line("let x = 1;\n", syntax);
        // Colors should be the same since the unknown theme was ignored
        assert_eq!(before.len(), after.len());
        for (b, a) in before.iter().zip(after.iter()) {
            assert_eq!(b.fg, a.fg);
        }
    }

    #[test]
    fn colored_span_debug_and_clone() {
        let span = ColoredSpan {
            text: "test".to_string(),
            fg: (255, 0, 0),
            bg: (0, 0, 0),
            bold: true,
            italic: false,
            underline: true,
        };
        let cloned = span.clone();
        assert_eq!(cloned.text, "test");
        assert!(cloned.bold);
        assert!(!cloned.italic);
        assert!(cloned.underline);
        // Debug should work without panic
        let _ = format!("{:?}", span);
    }
}
