//! TextMate grammar loading and syntax highlighting via syntect.

use std::path::Path;

use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SyntectStyle, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

/// Manages loaded TextMate grammars and themes.
pub struct TextMateService {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    active_theme: String,
}

impl TextMateService {
    /// Create with syntect's default bundled grammars and themes.
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            active_theme: "base16-ocean.dark".to_string(),
        }
    }

    /// Find syntax definition for a file path.
    pub fn find_syntax_for_file(&self, path: &Path) -> Option<&SyntaxReference> {
        self.syntax_set.find_syntax_for_file(path).ok().flatten()
    }

    /// Find syntax definition by language name.
    pub fn find_syntax_by_name(&self, name: &str) -> Option<&SyntaxReference> {
        self.syntax_set.find_syntax_by_name(name)
    }

    /// Find syntax by extension.
    pub fn find_syntax_by_extension(&self, ext: &str) -> Option<&SyntaxReference> {
        self.syntax_set.find_syntax_by_extension(ext)
    }

    /// Get the active theme.
    pub fn get_active_theme(&self) -> &Theme {
        self.theme_set
            .themes
            .get(&self.active_theme)
            .unwrap_or_else(|| self.theme_set.themes.values().next().unwrap())
    }

    /// Set the active theme by name.
    pub fn set_theme(&mut self, name: &str) {
        if self.theme_set.themes.contains_key(name) {
            self.active_theme = name.to_string();
        }
    }

    /// List available theme names.
    pub fn available_themes(&self) -> Vec<&str> {
        self.theme_set.themes.keys().map(|s| s.as_str()).collect()
    }

    /// List available syntax names.
    pub fn available_syntaxes(&self) -> Vec<&str> {
        self.syntax_set
            .syntaxes()
            .iter()
            .map(|s| s.name.as_str())
            .collect()
    }

    /// Highlight a single line, returning styled segments.
    pub fn highlight_line<'a>(
        &self,
        highlighter: &mut HighlightLines<'a>,
        line: &str,
    ) -> Vec<(SyntectStyle, String)> {
        match highlighter.highlight_line(line, &self.syntax_set) {
            Ok(ranges) => ranges.into_iter().map(|(s, t)| (s, t.to_string())).collect(),
            Err(_) => vec![(SyntectStyle::default(), line.to_string())],
        }
    }

    /// Create a highlighter for a specific syntax.
    pub fn create_highlighter<'a>(&'a self, syntax: &'a SyntaxReference) -> HighlightLines<'a> {
        HighlightLines::new(syntax, self.get_active_theme())
    }

    /// Get a reference to the syntax set.
    pub fn syntax_set(&self) -> &SyntaxSet {
        &self.syntax_set
    }
}

impl Default for TextMateService {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a syntect style to RGB (r, g, b) tuple.
pub fn syntect_to_rgb(style: SyntectStyle) -> (u8, u8, u8) {
    (style.foreground.r, style.foreground.g, style.foreground.b)
}

/// Convert a syntect style to a ratatui Color.
pub fn syntect_to_ratatui_color(style: SyntectStyle) -> ratatui::style::Color {
    ratatui::style::Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use syntect::highlighting::Color;

    #[test]
    fn service_creation() {
        let svc = TextMateService::new();
        assert!(!svc.available_syntaxes().is_empty());
        assert!(!svc.available_themes().is_empty());
    }

    #[test]
    fn default_trait() {
        let svc = TextMateService::default();
        assert!(!svc.available_syntaxes().is_empty());
    }

    #[test]
    fn find_rust_by_extension() {
        let svc = TextMateService::new();
        let syntax = svc.find_syntax_by_extension("rs");
        assert!(syntax.is_some());
        assert_eq!(syntax.unwrap().name, "Rust");
    }

    #[test]
    fn find_python_by_name() {
        let svc = TextMateService::new();
        let syntax = svc.find_syntax_by_name("Python");
        assert!(syntax.is_some());
        assert_eq!(syntax.unwrap().name, "Python");
    }

    #[test]
    fn find_syntax_by_file_path() {
        let svc = TextMateService::new();
        let syntax = svc.find_syntax_for_file(Path::new("main.rs"));
        assert!(syntax.is_some());
        assert_eq!(syntax.unwrap().name, "Rust");
    }

    #[test]
    fn find_syntax_by_file_path_python() {
        let svc = TextMateService::new();
        let syntax = svc.find_syntax_for_file(&PathBuf::from("script.py"));
        assert!(syntax.is_some());
        assert_eq!(syntax.unwrap().name, "Python");
    }

    #[test]
    fn unknown_syntax_returns_none() {
        let svc = TextMateService::new();
        assert!(svc.find_syntax_by_extension("zzzzz").is_none());
        assert!(svc.find_syntax_by_name("NoSuchLanguage").is_none());
    }

    #[test]
    fn list_available_themes() {
        let svc = TextMateService::new();
        let themes = svc.available_themes();
        assert!(themes.contains(&"base16-ocean.dark"));
    }

    #[test]
    fn list_available_syntaxes() {
        let svc = TextMateService::new();
        let syntaxes = svc.available_syntaxes();
        assert!(syntaxes.contains(&"Rust"));
        assert!(syntaxes.contains(&"Python"));
        assert!(syntaxes.contains(&"JavaScript"));
    }

    #[test]
    fn set_theme() {
        let mut svc = TextMateService::new();
        let other: String = svc
            .available_themes()
            .into_iter()
            .find(|t| *t != "base16-ocean.dark")
            .unwrap()
            .to_string();
        svc.set_theme(&other);
        assert_eq!(svc.active_theme, other);
    }

    #[test]
    fn set_invalid_theme_is_noop() {
        let mut svc = TextMateService::new();
        svc.set_theme("nonexistent-theme");
        assert_eq!(svc.active_theme, "base16-ocean.dark");
    }

    #[test]
    fn highlight_rust_line() {
        let svc = TextMateService::new();
        let syntax = svc.find_syntax_by_extension("rs").unwrap();
        let mut hl = svc.create_highlighter(syntax);
        let result = svc.highlight_line(&mut hl, "fn main() {\n");
        assert!(!result.is_empty());
        let combined: String = result.iter().map(|(_, t)| t.as_str()).collect();
        assert!(combined.contains("fn"));
    }

    #[test]
    fn highlight_python_line() {
        let svc = TextMateService::new();
        let syntax = svc.find_syntax_by_name("Python").unwrap();
        let mut hl = svc.create_highlighter(syntax);
        let result = svc.highlight_line(&mut hl, "def hello():\n");
        assert!(!result.is_empty());
        let combined: String = result.iter().map(|(_, t)| t.as_str()).collect();
        assert!(combined.contains("def"));
    }

    #[test]
    fn syntect_to_rgb_conversion() {
        let style = SyntectStyle {
            foreground: Color { r: 255, g: 128, b: 0, a: 255 },
            background: Color { r: 0, g: 0, b: 0, a: 255 },
            font_style: Default::default(),
        };
        assert_eq!(syntect_to_rgb(style), (255, 128, 0));
    }

    #[test]
    fn syntect_to_ratatui_color_conversion() {
        let style = SyntectStyle {
            foreground: Color { r: 10, g: 20, b: 30, a: 255 },
            background: Color { r: 0, g: 0, b: 0, a: 255 },
            font_style: Default::default(),
        };
        let color = syntect_to_ratatui_color(style);
        assert_eq!(color, ratatui::style::Color::Rgb(10, 20, 30));
    }

    #[test]
    fn syntax_set_accessor() {
        let svc = TextMateService::new();
        let ss = svc.syntax_set();
        assert!(!ss.syntaxes().is_empty());
    }
}
