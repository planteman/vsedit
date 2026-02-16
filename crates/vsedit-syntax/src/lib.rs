use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use syntect::easy::HighlightLines;
use syntect::highlighting::{
    Color as SyntectColor, FontStyle, Style, Theme, ThemeSet,
};
use syntect::parsing::{SyntaxReference, SyntaxSet};

/// A colored span of text within a line.
#[derive(Debug, Clone, PartialEq)]
pub struct ColoredSpan {
    pub text: String,
    pub fg: (u8, u8, u8),
    pub bg: (u8, u8, u8),
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl ColoredSpan {
    /// Create a plain (unstyled) span with default white-on-black.
    pub fn plain(text: &str) -> Self {
        Self {
            text: text.to_string(),
            fg: (255, 255, 255),
            bg: (0, 0, 0),
            bold: false,
            italic: false,
            underline: false,
        }
    }

    /// Create a span with specific foreground color.
    pub fn with_fg(text: &str, r: u8, g: u8, b: u8) -> Self {
        Self {
            text: text.to_string(),
            fg: (r, g, b),
            bg: (0, 0, 0),
            bold: false,
            italic: false,
            underline: false,
        }
    }

    /// The byte length of the text content.
    pub fn byte_len(&self) -> usize {
        self.text.len()
    }

    /// The character count of the text content.
    pub fn char_count(&self) -> usize {
        self.text.chars().count()
    }

    /// Whether this span has any styling applied.
    pub fn is_styled(&self) -> bool {
        self.bold || self.italic || self.underline || self.fg != (255, 255, 255) || self.bg != (0, 0, 0)
    }

    /// Merge adjacent spans that share the same style.
    pub fn merge_adjacent(spans: &[ColoredSpan]) -> Vec<ColoredSpan> {
        if spans.is_empty() {
            return Vec::new();
        }
        let mut merged = Vec::with_capacity(spans.len());
        let mut current = spans[0].clone();
        for span in &spans[1..] {
            if span.fg == current.fg
                && span.bg == current.bg
                && span.bold == current.bold
                && span.italic == current.italic
                && span.underline == current.underline
            {
                current.text.push_str(&span.text);
            } else {
                merged.push(current);
                current = span.clone();
            }
        }
        merged.push(current);
        merged
    }

    /// Split a span at a character offset, returning (left, right).
    pub fn split_at_char(&self, idx: usize) -> (ColoredSpan, ColoredSpan) {
        let byte_idx = self
            .text
            .char_indices()
            .nth(idx)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len());
        let (left, right) = self.text.split_at(byte_idx);
        (
            ColoredSpan {
                text: left.to_string(),
                ..self.clone()
            },
            ColoredSpan {
                text: right.to_string(),
                ..self.clone()
            },
        )
    }

    /// Trim whitespace from both ends of the span text, preserving style.
    pub fn trim(&self) -> ColoredSpan {
        ColoredSpan {
            text: self.text.trim().to_string(),
            ..self.clone()
        }
    }
}

/// A fully highlighted line with metadata.
#[derive(Debug, Clone)]
pub struct HighlightedLine {
    pub line_number: usize,
    pub spans: Vec<ColoredSpan>,
    pub is_blank: bool,
}

impl HighlightedLine {
    /// Create from spans with line number.
    pub fn new(line_number: usize, spans: Vec<ColoredSpan>) -> Self {
        let is_blank = spans.iter().all(|s| s.text.trim().is_empty());
        Self { line_number, spans, is_blank }
    }

    /// The combined text of all spans.
    pub fn text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }

    /// Total character count.
    pub fn char_count(&self) -> usize {
        self.spans.iter().map(|s| s.char_count()).sum()
    }

    /// Get the span and offset within it for a given character column.
    pub fn span_at_column(&self, col: usize) -> Option<(usize, usize)> {
        let mut offset = 0;
        for (i, span) in self.spans.iter().enumerate() {
            let char_count = span.char_count();
            if col < offset + char_count {
                return Some((i, col - offset));
            }
            offset += char_count;
        }
        None
    }

    /// Merge adjacent same-styled spans.
    pub fn merge_spans(&mut self) {
        self.spans = ColoredSpan::merge_adjacent(&self.spans);
    }
}

/// Token scope information for semantic analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenInfo {
    pub text: String,
    pub scope: String,
    pub start_col: usize,
    pub end_col: usize,
}

/// Language detection result.
#[derive(Debug, Clone, PartialEq)]
pub struct DetectedLanguage {
    pub name: String,
    pub extensions: Vec<String>,
}

/// Theme color palette extracted from a syntax theme.
#[derive(Debug, Clone)]
pub struct ThemePalette {
    pub foreground: (u8, u8, u8),
    pub background: (u8, u8, u8),
    pub caret: (u8, u8, u8),
    pub selection: (u8, u8, u8),
    pub line_highlight: (u8, u8, u8),
    pub gutter_foreground: (u8, u8, u8),
}

impl ThemePalette {
    fn from_theme(theme: &Theme) -> Self {
        let settings = &theme.settings;
        let extract = |c: Option<SyntectColor>| -> (u8, u8, u8) {
            c.map(|c| (c.r, c.g, c.b)).unwrap_or((200, 200, 200))
        };
        Self {
            foreground: extract(settings.foreground),
            background: extract(settings.background),
            caret: extract(settings.caret),
            selection: extract(settings.selection),
            line_highlight: extract(settings.line_highlight),
            gutter_foreground: extract(settings.gutter_foreground),
        }
    }
}

/// Cached highlight state for incremental updates.
#[derive(Debug)]
pub struct HighlightCache {
    lines: HashMap<usize, Vec<ColoredSpan>>,
    dirty_from: Option<usize>,
}

impl HighlightCache {
    pub fn new() -> Self {
        Self {
            lines: HashMap::new(),
            dirty_from: None,
        }
    }

    /// Get cached spans for a line.
    pub fn get(&self, line: usize) -> Option<&Vec<ColoredSpan>> {
        if let Some(dirty) = self.dirty_from {
            if line >= dirty {
                return None;
            }
        }
        self.lines.get(&line)
    }

    /// Store highlighted spans for a line.
    pub fn set(&mut self, line: usize, spans: Vec<ColoredSpan>) {
        self.lines.insert(line, spans);
    }

    /// Mark all lines from `line` onward as dirty.
    pub fn invalidate_from(&mut self, line: usize) {
        match self.dirty_from {
            Some(current) if current <= line => {}
            _ => self.dirty_from = Some(line),
        }
        self.lines.retain(|&k, _| k < line);
    }

    /// Invalidate a single line.
    pub fn invalidate_line(&mut self, line: usize) {
        self.lines.remove(&line);
    }

    /// Clear the entire cache.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.dirty_from = None;
    }

    /// Number of cached lines.
    pub fn cached_count(&self) -> usize {
        self.lines.len()
    }

    /// Whether a line needs re-highlighting.
    pub fn is_dirty(&self, line: usize) -> bool {
        if let Some(dirty) = self.dirty_from {
            if line >= dirty {
                return true;
            }
        }
        !self.lines.contains_key(&line)
    }
}

impl Default for HighlightCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Syntax highlighter backed by syntect.
pub struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme: Theme,
    theme_name: String,
    cache: HighlightCache,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set.themes["base16-ocean.dark"].clone();
        Self {
            syntax_set,
            theme,
            theme_name: "base16-ocean.dark".to_string(),
            cache: HighlightCache::new(),
        }
    }

    /// Create with a specific theme name.
    pub fn with_theme(theme_name: &str) -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set
            .themes
            .get(theme_name)
            .cloned()
            .unwrap_or_else(|| theme_set.themes["base16-ocean.dark"].clone());
        let actual_name = if theme_set.themes.contains_key(theme_name) {
            theme_name.to_string()
        } else {
            "base16-ocean.dark".to_string()
        };
        Self {
            syntax_set,
            theme,
            theme_name: actual_name,
            cache: HighlightCache::new(),
        }
    }

    /// Get the active theme name.
    pub fn theme_name(&self) -> &str {
        &self.theme_name
    }

    /// Get the theme color palette.
    pub fn palette(&self) -> ThemePalette {
        ThemePalette::from_theme(&self.theme)
    }

    /// Get the syntax definition for a file by extension.
    pub fn syntax_for_file(&self, filename: &str) -> Option<&SyntaxReference> {
        self.syntax_set.find_syntax_for_file(filename).ok().flatten()
    }

    /// Get syntax by language name (case-insensitive).
    pub fn syntax_by_name(&self, name: &str) -> Option<&SyntaxReference> {
        let lower = name.to_lowercase();
        self.syntax_set
            .syntaxes()
            .iter()
            .find(|s| s.name.to_lowercase() == lower)
    }

    /// Get syntax by first line content (shebangs, modelines).
    pub fn syntax_by_first_line(&self, first_line: &str) -> Option<&SyntaxReference> {
        self.syntax_set.find_syntax_by_first_line(first_line)
    }

    /// Detect language from file path, first line, or content heuristics.
    pub fn detect_language(&self, path: &str, first_line: Option<&str>) -> Option<DetectedLanguage> {
        // Try by file extension first
        if let Some(syn) = self.syntax_for_file(path) {
            return Some(DetectedLanguage {
                name: syn.name.clone(),
                extensions: syn.file_extensions.clone(),
            });
        }
        // Try by first line (shebang)
        if let Some(line) = first_line {
            if let Some(syn) = self.syntax_by_first_line(line) {
                return Some(DetectedLanguage {
                    name: syn.name.clone(),
                    extensions: syn.file_extensions.clone(),
                });
            }
        }
        None
    }

    fn style_to_span(style: &Style, text: &str) -> ColoredSpan {
        ColoredSpan {
            text: text.to_string(),
            fg: (style.foreground.r, style.foreground.g, style.foreground.b),
            bg: (style.background.r, style.background.g, style.background.b),
            bold: style.font_style.contains(FontStyle::BOLD),
            italic: style.font_style.contains(FontStyle::ITALIC),
            underline: style.font_style.contains(FontStyle::UNDERLINE),
        }
    }

    /// Highlight a single line of code, returning colored spans.
    pub fn highlight_line(&self, line: &str, syntax: &SyntaxReference) -> Vec<ColoredSpan> {
        let mut h = HighlightLines::new(syntax, &self.theme);
        match h.highlight_line(line, &self.syntax_set) {
            Ok(ranges) => ranges
                .iter()
                .map(|(style, text)| Self::style_to_span(style, text))
                .collect(),
            Err(_) => vec![ColoredSpan::plain(line)],
        }
    }

    /// Highlight a single line with caching.
    pub fn highlight_line_cached(
        &mut self,
        line_num: usize,
        line: &str,
        syntax: &SyntaxReference,
    ) -> Vec<ColoredSpan> {
        if let Some(cached) = self.cache.get(line_num) {
            return cached.clone();
        }
        let spans = self.highlight_line(line, syntax);
        self.cache.set(line_num, spans.clone());
        spans
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
            .map(|line| match h.highlight_line(line, &self.syntax_set) {
                Ok(ranges) => ranges
                    .iter()
                    .map(|(style, text)| Self::style_to_span(style, text))
                    .collect(),
                Err(_) => vec![ColoredSpan::plain(line)],
            })
            .collect()
    }

    /// Highlight a range of lines (viewport optimization).
    pub fn highlight_range(
        &self,
        lines: &[&str],
        syntax: &SyntaxReference,
        start_line: usize,
        end_line: usize,
    ) -> Vec<HighlightedLine> {
        let end = end_line.min(lines.len());
        let start = start_line.min(end);
        let mut h = HighlightLines::new(syntax, &self.theme);
        // Process lines before start to get correct state
        for line in &lines[..start] {
            let _ = h.highlight_line(line, &self.syntax_set);
        }
        // Now highlight the visible range
        lines[start..end]
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let spans = match h.highlight_line(line, &self.syntax_set) {
                    Ok(ranges) => ranges
                        .iter()
                        .map(|(style, text)| Self::style_to_span(style, text))
                        .collect(),
                    Err(_) => vec![ColoredSpan::plain(line)],
                };
                HighlightedLine::new(start + i, spans)
            })
            .collect()
    }

    /// Highlight a document and return HighlightedLine structs.
    pub fn highlight_document(
        &self,
        content: &str,
        syntax: &SyntaxReference,
    ) -> Vec<HighlightedLine> {
        let lines: Vec<&str> = content.lines().collect();
        let mut h = HighlightLines::new(syntax, &self.theme);
        lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let line_with_nl = format!("{}\n", line);
                let spans = match h.highlight_line(&line_with_nl, &self.syntax_set) {
                    Ok(ranges) => ranges
                        .iter()
                        .map(|(style, text)| Self::style_to_span(style, text))
                        .collect(),
                    Err(_) => vec![ColoredSpan::plain(&line_with_nl)],
                };
                HighlightedLine::new(i, spans)
            })
            .collect()
    }

    /// Get token scope information at a specific position.
    /// Uses the highlighter to identify token boundaries.
    pub fn tokens_in_line(&self, line: &str, syntax: &SyntaxReference) -> Vec<TokenInfo> {
        let mut h = HighlightLines::new(syntax, &self.theme);
        match h.highlight_line(line, &self.syntax_set) {
            Ok(ranges) => {
                let mut tokens = Vec::new();
                let mut col = 0;
                for (_style, text) in &ranges {
                    let end_col = col + text.len();
                    if !text.is_empty() {
                        tokens.push(TokenInfo {
                            text: text.to_string(),
                            scope: String::new(),
                            start_col: col,
                            end_col,
                        });
                    }
                    col = end_col;
                }
                tokens
            }
            Err(_) => {
                vec![TokenInfo {
                    text: line.to_string(),
                    scope: String::new(),
                    start_col: 0,
                    end_col: line.len(),
                }]
            }
        }
    }

    /// Invalidate highlight cache from a specific line.
    pub fn invalidate_from(&mut self, line: usize) {
        self.cache.invalidate_from(line);
    }

    /// Invalidate a single cache line.
    pub fn invalidate_line(&mut self, line: usize) {
        self.cache.invalidate_line(line);
    }

    /// Clear the entire highlight cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// List available syntax names.
    pub fn available_syntaxes(&self) -> Vec<&str> {
        self.syntax_set
            .syntaxes()
            .iter()
            .map(|s| s.name.as_str())
            .collect()
    }

    /// List available syntax names sorted alphabetically.
    pub fn available_syntaxes_sorted(&self) -> Vec<&str> {
        let mut names = self.available_syntaxes();
        names.sort_unstable();
        names
    }

    /// Get file extensions associated with a syntax.
    pub fn extensions_for_syntax(&self, name: &str) -> Vec<String> {
        self.syntax_by_name(name)
            .map(|s| s.file_extensions.clone())
            .unwrap_or_default()
    }

    /// Get the number of available syntaxes.
    pub fn syntax_count(&self) -> usize {
        self.syntax_set.syntaxes().len()
    }

    /// List available theme names.
    pub fn available_themes() -> Vec<String> {
        let mut themes: Vec<String> = ThemeSet::load_defaults().themes.keys().cloned().collect();
        themes.sort();
        themes
    }

    /// Set the active theme by name.
    pub fn set_theme(&mut self, name: &str) {
        let ts = ThemeSet::load_defaults();
        if let Some(theme) = ts.themes.get(name) {
            self.theme = theme.clone();
            self.theme_name = name.to_string();
            self.cache.clear();
        }
    }

    /// Map a syntect theme to a VS Code-compatible token color rule set.
    pub fn theme_token_colors(&self) -> Vec<ThemeTokenRule> {
        self.theme
            .scopes
            .iter()
            .map(|item| ThemeTokenRule {
                scope: format!("{:?}", item.scope),
                foreground: item
                    .style
                    .foreground
                    .map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)),
                background: item
                    .style
                    .background
                    .map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)),
                font_style: item.style.font_style.map(|fs| {
                    let mut styles = Vec::new();
                    if fs.contains(FontStyle::BOLD) {
                        styles.push("bold");
                    }
                    if fs.contains(FontStyle::ITALIC) {
                        styles.push("italic");
                    }
                    if fs.contains(FontStyle::UNDERLINE) {
                        styles.push("underline");
                    }
                    styles.join(" ")
                }),
            })
            .collect()
    }

    /// Detect language from a file path using extension-based heuristics.
    pub fn language_id_for_path(&self, path: &str) -> Option<String> {
        let ext = Path::new(path).extension()?.to_str()?;
        // Map common extensions to VS Code language IDs
        let lang_id = match ext {
            "rs" => "rust",
            "py" | "pyw" => "python",
            "js" | "mjs" | "cjs" => "javascript",
            "ts" | "mts" | "cts" => "typescript",
            "tsx" => "typescriptreact",
            "jsx" => "javascriptreact",
            "java" => "java",
            "c" | "h" => "c",
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" => "cpp",
            "cs" => "csharp",
            "go" => "go",
            "rb" | "rake" => "ruby",
            "php" => "php",
            "swift" => "swift",
            "kt" | "kts" => "kotlin",
            "scala" => "scala",
            "r" | "R" => "r",
            "lua" => "lua",
            "pl" | "pm" => "perl",
            "sh" | "bash" | "zsh" => "shellscript",
            "ps1" => "powershell",
            "bat" | "cmd" => "bat",
            "html" | "htm" => "html",
            "css" => "css",
            "scss" => "scss",
            "less" => "less",
            "json" | "jsonc" => "json",
            "xml" | "xsl" | "xslt" => "xml",
            "yaml" | "yml" => "yaml",
            "toml" => "toml",
            "ini" | "cfg" => "ini",
            "md" | "markdown" => "markdown",
            "tex" | "latex" => "latex",
            "sql" => "sql",
            "graphql" | "gql" => "graphql",
            "dockerfile" => "dockerfile",
            "makefile" => "makefile",
            "cmake" => "cmake",
            "zig" => "zig",
            "nim" => "nim",
            "dart" => "dart",
            "ex" | "exs" => "elixir",
            "erl" | "hrl" => "erlang",
            "hs" | "lhs" => "haskell",
            "ml" | "mli" => "ocaml",
            "fs" | "fsi" | "fsx" => "fsharp",
            "clj" | "cljs" | "cljc" => "clojure",
            "vue" => "vue",
            "svelte" => "svelte",
            "astro" => "astro",
            "wasm" => "wasm",
            "proto" => "proto",
            "tf" | "tfvars" => "terraform",
            _ => return None,
        };
        Some(lang_id.to_string())
    }
}

/// A theme token color rule (VS Code compatible format).
#[derive(Debug, Clone)]
pub struct ThemeTokenRule {
    pub scope: String,
    pub foreground: Option<String>,
    pub background: Option<String>,
    pub font_style: Option<String>,
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ScopeStack — nested syntax scopes
// ---------------------------------------------------------------------------

/// A stack of nested syntax scopes (e.g. "source.rust", "meta.function", "entity.name").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeStack {
    scopes: Vec<String>,
}

impl ScopeStack {
    /// Create an empty scope stack.
    pub fn new() -> Self {
        Self { scopes: Vec::new() }
    }

    /// Create a scope stack from a dotted scope string like "source.rust meta.function".
    pub fn from_str(scope_str: &str) -> Self {
        let scopes = scope_str.split_whitespace().map(|s| s.to_string()).collect();
        Self { scopes }
    }

    /// Push a new scope onto the stack.
    pub fn push(&mut self, scope: impl Into<String>) {
        self.scopes.push(scope.into());
    }

    /// Pop the top scope.
    pub fn pop(&mut self) -> Option<String> {
        self.scopes.pop()
    }

    /// The current depth of the scope stack.
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// Returns true if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.scopes.is_empty()
    }

    /// The top (most specific) scope.
    pub fn top(&self) -> Option<&str> {
        self.scopes.last().map(|s| s.as_str())
    }

    /// The bottom (most general) scope.
    pub fn bottom(&self) -> Option<&str> {
        self.scopes.first().map(|s| s.as_str())
    }

    /// All scopes as a slice.
    pub fn scopes(&self) -> &[String] {
        &self.scopes
    }

    /// Convert to a space-separated string.
    pub fn to_scope_string(&self) -> String {
        self.scopes.join(" ")
    }

    /// Check whether this stack matches a scope selector.
    /// A selector matches if any scope in the stack starts with the selector prefix.
    pub fn matches_selector(&self, selector: &str) -> bool {
        self.scopes.iter().any(|s| scope_matches_selector(s, selector))
    }

    /// Compute specificity score for a selector match.
    /// Higher = more specific match. Returns 0 for no match.
    pub fn specificity(&self, selector: &str) -> u32 {
        let mut best = 0u32;
        for (i, scope) in self.scopes.iter().enumerate() {
            if scope_matches_selector(scope, selector) {
                // Deeper position + more dots = more specific
                let depth_score = (i as u32 + 1) * 10;
                let parts_score = selector.matches('.').count() as u32 + 1;
                let score = depth_score + parts_score;
                if score > best {
                    best = score;
                }
            }
        }
        best
    }

    /// Find the best matching selector from a list.
    /// Returns the selector with the highest specificity score.
    pub fn best_match<'a>(&self, selectors: &[&'a str]) -> Option<&'a str> {
        let mut best_selector: Option<&str> = None;
        let mut best_score = 0u32;
        for &sel in selectors {
            let score = self.specificity(sel);
            if score > best_score {
                best_score = score;
                best_selector = Some(sel);
            }
        }
        best_selector
    }

    /// Returns true if this stack has a scope that exactly equals the given scope.
    pub fn contains_exact(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    /// Returns true if any scope in the stack starts with the given prefix.
    pub fn contains_prefix(&self, prefix: &str) -> bool {
        self.scopes.iter().any(|s| s.starts_with(prefix))
    }
}

impl Default for ScopeStack {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ScopeStack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_scope_string())
    }
}

/// Check if a scope matches a selector.
/// A selector "entity.name" matches "entity.name.function" but not "entity".
fn scope_matches_selector(scope: &str, selector: &str) -> bool {
    if scope == selector {
        return true;
    }
    // The selector is a prefix: "entity.name" matches "entity.name.function"
    scope.starts_with(selector) && scope.as_bytes().get(selector.len()) == Some(&b'.')
}

// ---------------------------------------------------------------------------
// ScopeSelector — with wildcard pattern matching
// ---------------------------------------------------------------------------

/// A scope selector that can include wildcard patterns.
///
/// Supports:
/// - Exact match: "source.rust"
/// - Prefix match: "source.rust" matches "source.rust.macro"
/// - Wildcard: "source.*" matches "source.rust", "source.python"
/// - Double wildcard: "**.function" matches any scope ending with ".function"
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeSelector {
    pub pattern: String,
    pub parts: Vec<ScopeSelectorPart>,
}

/// A part of a scope selector pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeSelectorPart {
    Exact(String),
    Wildcard,
    DoubleWildcard,
}

impl ScopeSelector {
    /// Parse a scope selector pattern.
    pub fn new(pattern: &str) -> Self {
        let parts: Vec<ScopeSelectorPart> = pattern
            .split('.')
            .map(|part| match part {
                "**" => ScopeSelectorPart::DoubleWildcard,
                "*" => ScopeSelectorPart::Wildcard,
                other => ScopeSelectorPart::Exact(other.to_string()),
            })
            .collect();
        Self {
            pattern: pattern.to_string(),
            parts,
        }
    }

    /// Test if a scope string matches this selector.
    pub fn matches(&self, scope: &str) -> bool {
        let scope_parts: Vec<&str> = scope.split('.').collect();
        Self::match_parts(&self.parts, &scope_parts)
    }

    fn match_parts(selector_parts: &[ScopeSelectorPart], scope_parts: &[&str]) -> bool {
        if selector_parts.is_empty() {
            return true; // empty selector matches everything
        }
        if scope_parts.is_empty() {
            return selector_parts.iter().all(|p| matches!(p, ScopeSelectorPart::DoubleWildcard));
        }

        match &selector_parts[0] {
            ScopeSelectorPart::Exact(expected) => {
                if scope_parts[0] == expected.as_str() {
                    Self::match_parts(&selector_parts[1..], &scope_parts[1..])
                } else {
                    false
                }
            }
            ScopeSelectorPart::Wildcard => {
                // Match exactly one part
                Self::match_parts(&selector_parts[1..], &scope_parts[1..])
            }
            ScopeSelectorPart::DoubleWildcard => {
                // Match zero or more parts
                for i in 0..=scope_parts.len() {
                    if Self::match_parts(&selector_parts[1..], &scope_parts[i..]) {
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Compute the specificity score for a match.
    /// More exact parts = higher specificity. Returns 0 for no match.
    pub fn specificity(&self, scope: &str) -> u32 {
        if !self.matches(scope) {
            return 0;
        }
        let mut score = 0u32;
        for part in &self.parts {
            match part {
                ScopeSelectorPart::Exact(_) => score += 10,
                ScopeSelectorPart::Wildcard => score += 1,
                ScopeSelectorPart::DoubleWildcard => score += 0,
            }
        }
        score
    }
}

impl fmt::Display for ScopeSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.pattern)
    }
}

/// Find the best matching selector from a list for a given scope.
pub fn best_selector_match<'a>(scope: &str, selectors: &[&'a ScopeSelector]) -> Option<&'a ScopeSelector> {
    let mut best: Option<&ScopeSelector> = None;
    let mut best_score = 0u32;
    for &sel in selectors {
        let score = sel.specificity(scope);
        if score > best_score {
            best_score = score;
            best = Some(sel);
        }
    }
    best
}

/// Resolve a scope stack against a list of selectors, returning the best match.
pub fn resolve_scope_stack<'a>(stack: &ScopeStack, selectors: &[&'a ScopeSelector]) -> Option<&'a ScopeSelector> {
    let mut best: Option<&ScopeSelector> = None;
    let mut best_score = 0u32;
    for scope in stack.scopes() {
        for &sel in selectors {
            let score = sel.specificity(scope);
            if score > best_score {
                best_score = score;
                best = Some(sel);
            }
        }
    }
    best
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
    fn available_themes_sorted() {
        let themes = SyntaxHighlighter::available_themes();
        let mut sorted = themes.clone();
        sorted.sort();
        assert_eq!(themes, sorted, "themes should be sorted");
    }

    #[test]
    fn set_theme_changes_theme() {
        let mut hl = SyntaxHighlighter::new();
        let themes = SyntaxHighlighter::available_themes();
        if let Some(name) = themes.iter().find(|t| *t != "base16-ocean.dark") {
            hl.set_theme(name);
            assert_eq!(hl.theme_name(), name);
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
        assert_eq!(hl.theme_name(), "base16-ocean.dark");
        let syntax = hl.syntax_for_file("test.rs").unwrap();
        let after = hl.highlight_line("let x = 1;\n", syntax);
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
        let _ = format!("{:?}", span);
    }

    // ColoredSpan helper tests
    #[test]
    fn colored_span_plain() {
        let span = ColoredSpan::plain("hello");
        assert_eq!(span.text, "hello");
        assert_eq!(span.fg, (255, 255, 255));
        assert_eq!(span.bg, (0, 0, 0));
        assert!(!span.is_styled());
    }

    #[test]
    fn colored_span_with_fg() {
        let span = ColoredSpan::with_fg("hi", 128, 0, 255);
        assert_eq!(span.fg, (128, 0, 255));
        assert!(span.is_styled());
    }

    #[test]
    fn colored_span_byte_and_char_len() {
        let span = ColoredSpan::plain("café");
        assert_eq!(span.char_count(), 4);
        assert_eq!(span.byte_len(), 5); // é is 2 bytes
    }

    #[test]
    fn colored_span_is_styled() {
        let mut span = ColoredSpan::plain("x");
        assert!(!span.is_styled());
        span.bold = true;
        assert!(span.is_styled());
    }

    #[test]
    fn merge_adjacent_spans() {
        let spans = vec![
            ColoredSpan::plain("hello"),
            ColoredSpan::plain(" world"),
            ColoredSpan::with_fg("!", 255, 0, 0),
        ];
        let merged = ColoredSpan::merge_adjacent(&spans);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].text, "hello world");
        assert_eq!(merged[1].text, "!");
    }

    #[test]
    fn merge_empty_spans() {
        let merged = ColoredSpan::merge_adjacent(&[]);
        assert!(merged.is_empty());
    }

    #[test]
    fn split_at_char() {
        let span = ColoredSpan::with_fg("hello", 100, 200, 50);
        let (left, right) = span.split_at_char(3);
        assert_eq!(left.text, "hel");
        assert_eq!(right.text, "lo");
        assert_eq!(left.fg, (100, 200, 50));
        assert_eq!(right.fg, (100, 200, 50));
    }

    #[test]
    fn split_at_char_unicode() {
        let span = ColoredSpan::plain("café");
        let (left, right) = span.split_at_char(3);
        assert_eq!(left.text, "caf");
        assert_eq!(right.text, "é");
    }

    #[test]
    fn trim_span() {
        let span = ColoredSpan::plain("  hello  ");
        let trimmed = span.trim();
        assert_eq!(trimmed.text, "hello");
    }

    // HighlightedLine tests
    #[test]
    fn highlighted_line_new() {
        let spans = vec![ColoredSpan::plain("fn main() {}")];
        let line = HighlightedLine::new(0, spans);
        assert_eq!(line.line_number, 0);
        assert!(!line.is_blank);
        assert_eq!(line.text(), "fn main() {}");
    }

    #[test]
    fn highlighted_line_blank() {
        let spans = vec![ColoredSpan::plain("   ")];
        let line = HighlightedLine::new(5, spans);
        assert!(line.is_blank);
    }

    #[test]
    fn highlighted_line_char_count() {
        let spans = vec![
            ColoredSpan::plain("fn "),
            ColoredSpan::with_fg("main", 100, 200, 50),
        ];
        let line = HighlightedLine::new(0, spans);
        assert_eq!(line.char_count(), 7);
    }

    #[test]
    fn highlighted_line_span_at_column() {
        let spans = vec![
            ColoredSpan::plain("fn "),
            ColoredSpan::with_fg("main", 100, 200, 50),
        ];
        let line = HighlightedLine::new(0, spans);
        assert_eq!(line.span_at_column(0), Some((0, 0)));
        assert_eq!(line.span_at_column(2), Some((0, 2)));
        assert_eq!(line.span_at_column(3), Some((1, 0)));
        assert_eq!(line.span_at_column(6), Some((1, 3)));
        assert_eq!(line.span_at_column(7), None);
    }

    #[test]
    fn highlighted_line_merge_spans() {
        let spans = vec![
            ColoredSpan::plain("hel"),
            ColoredSpan::plain("lo"),
            ColoredSpan::with_fg("!", 255, 0, 0),
        ];
        let mut line = HighlightedLine::new(0, spans);
        line.merge_spans();
        assert_eq!(line.spans.len(), 2);
        assert_eq!(line.spans[0].text, "hello");
    }

    // HighlightCache tests
    #[test]
    fn cache_get_set() {
        let mut cache = HighlightCache::new();
        assert!(cache.get(0).is_none());
        cache.set(0, vec![ColoredSpan::plain("hello")]);
        assert!(cache.get(0).is_some());
        assert_eq!(cache.cached_count(), 1);
    }

    #[test]
    fn cache_invalidate_line() {
        let mut cache = HighlightCache::new();
        cache.set(0, vec![ColoredSpan::plain("a")]);
        cache.set(1, vec![ColoredSpan::plain("b")]);
        cache.invalidate_line(0);
        assert!(cache.get(0).is_none());
        assert!(cache.get(1).is_some());
    }

    #[test]
    fn cache_invalidate_from() {
        let mut cache = HighlightCache::new();
        cache.set(0, vec![ColoredSpan::plain("a")]);
        cache.set(1, vec![ColoredSpan::plain("b")]);
        cache.set(2, vec![ColoredSpan::plain("c")]);
        cache.invalidate_from(1);
        assert!(cache.get(0).is_some());
        assert!(cache.get(1).is_none());
        assert!(cache.get(2).is_none());
    }

    #[test]
    fn cache_clear() {
        let mut cache = HighlightCache::new();
        cache.set(0, vec![ColoredSpan::plain("a")]);
        cache.set(1, vec![ColoredSpan::plain("b")]);
        cache.clear();
        assert_eq!(cache.cached_count(), 0);
    }

    #[test]
    fn cache_is_dirty() {
        let mut cache = HighlightCache::new();
        assert!(cache.is_dirty(0));
        cache.set(0, vec![ColoredSpan::plain("a")]);
        assert!(!cache.is_dirty(0));
        cache.invalidate_from(0);
        assert!(cache.is_dirty(0));
    }

    // SyntaxHighlighter extended tests
    #[test]
    fn with_theme_constructor() {
        let hl = SyntaxHighlighter::with_theme("base16-ocean.dark");
        assert_eq!(hl.theme_name(), "base16-ocean.dark");
    }

    #[test]
    fn with_theme_unknown_falls_back() {
        let hl = SyntaxHighlighter::with_theme("nonexistent-theme");
        assert_eq!(hl.theme_name(), "base16-ocean.dark");
    }

    #[test]
    fn syntax_by_name() {
        let hl = SyntaxHighlighter::new();
        let syn = hl.syntax_by_name("rust");
        assert!(syn.is_some());
        assert_eq!(syn.unwrap().name, "Rust");
    }

    #[test]
    fn syntax_by_name_case_insensitive() {
        let hl = SyntaxHighlighter::new();
        assert!(hl.syntax_by_name("RUST").is_some());
        assert!(hl.syntax_by_name("Rust").is_some());
        assert!(hl.syntax_by_name("rust").is_some());
    }

    #[test]
    fn syntax_by_first_line_shebang() {
        let hl = SyntaxHighlighter::new();
        let syn = hl.syntax_by_first_line("#!/usr/bin/env python3");
        assert!(syn.is_some());
    }

    #[test]
    fn detect_language_by_extension() {
        let hl = SyntaxHighlighter::new();
        let lang = hl.detect_language("main.rs", None);
        assert!(lang.is_some());
        assert_eq!(lang.unwrap().name, "Rust");
    }

    #[test]
    fn detect_language_by_first_line() {
        let hl = SyntaxHighlighter::new();
        let lang = hl.detect_language("script", Some("#!/usr/bin/env python"));
        assert!(lang.is_some());
    }

    #[test]
    fn detect_language_unknown() {
        let hl = SyntaxHighlighter::new();
        let lang = hl.detect_language("unknown.zzz", None);
        assert!(lang.is_none());
    }

    #[test]
    fn palette_returns_colors() {
        let hl = SyntaxHighlighter::new();
        let palette = hl.palette();
        // base16-ocean.dark has specific known colors
        assert_ne!(palette.foreground, (0, 0, 0));
    }

    #[test]
    fn highlight_line_cached() {
        let mut hl = SyntaxHighlighter::new();
        let spans1 = {
            let syntax = hl.syntax_for_file("test.rs").unwrap();
            hl.highlight_line("let x = 1;\n", syntax)
        };
        hl.cache.set(0, spans1.clone());
        let spans2 = hl.cache.get(0).unwrap().clone();
        assert_eq!(spans1.len(), spans2.len());
        assert_eq!(hl.cache.cached_count(), 1);
    }

    #[test]
    fn highlight_range_visible_subset() {
        let hl = SyntaxHighlighter::new();
        let syntax = hl.syntax_for_file("test.rs").unwrap();
        let lines = vec![
            "fn main() {\n",
            "    let x = 1;\n",
            "    let y = 2;\n",
            "    println!(\"{}\", x + y);\n",
            "}\n",
        ];
        let result = hl.highlight_range(&lines, syntax, 1, 3);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].line_number, 1);
        assert_eq!(result[1].line_number, 2);
    }

    #[test]
    fn highlight_document() {
        let hl = SyntaxHighlighter::new();
        let syntax = hl.syntax_for_file("test.rs").unwrap();
        let content = "fn main() {\n    println!(\"hello\");\n}";
        let result = hl.highlight_document(content, syntax);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].line_number, 0);
        assert_eq!(result[2].line_number, 2);
    }

    #[test]
    fn available_syntaxes_sorted() {
        let hl = SyntaxHighlighter::new();
        let sorted = hl.available_syntaxes_sorted();
        let mut expected = sorted.clone();
        expected.sort_unstable();
        assert_eq!(sorted, expected);
    }

    #[test]
    fn extensions_for_syntax() {
        let hl = SyntaxHighlighter::new();
        let exts = hl.extensions_for_syntax("rust");
        assert!(exts.contains(&"rs".to_string()));
    }

    #[test]
    fn extensions_for_unknown_syntax() {
        let hl = SyntaxHighlighter::new();
        let exts = hl.extensions_for_syntax("nonexistent");
        assert!(exts.is_empty());
    }

    #[test]
    fn syntax_count() {
        let hl = SyntaxHighlighter::new();
        assert!(hl.syntax_count() > 10);
    }

    #[test]
    fn theme_token_colors() {
        let hl = SyntaxHighlighter::new();
        let rules = hl.theme_token_colors();
        assert!(!rules.is_empty());
        // At least some rules should have foreground colors
        let has_fg = rules.iter().any(|r| r.foreground.is_some());
        assert!(has_fg);
    }

    #[test]
    fn language_id_for_path_rust() {
        let hl = SyntaxHighlighter::new();
        assert_eq!(hl.language_id_for_path("main.rs"), Some("rust".to_string()));
    }

    #[test]
    fn language_id_for_path_python() {
        let hl = SyntaxHighlighter::new();
        assert_eq!(hl.language_id_for_path("script.py"), Some("python".to_string()));
    }

    #[test]
    fn language_id_for_path_typescript() {
        let hl = SyntaxHighlighter::new();
        assert_eq!(hl.language_id_for_path("app.ts"), Some("typescript".to_string()));
        assert_eq!(hl.language_id_for_path("component.tsx"), Some("typescriptreact".to_string()));
    }

    #[test]
    fn language_id_for_path_various() {
        let hl = SyntaxHighlighter::new();
        assert_eq!(hl.language_id_for_path("file.go"), Some("go".to_string()));
        assert_eq!(hl.language_id_for_path("file.java"), Some("java".to_string()));
        assert_eq!(hl.language_id_for_path("file.cpp"), Some("cpp".to_string()));
        assert_eq!(hl.language_id_for_path("file.rb"), Some("ruby".to_string()));
        assert_eq!(hl.language_id_for_path("file.sh"), Some("shellscript".to_string()));
        assert_eq!(hl.language_id_for_path("file.html"), Some("html".to_string()));
        assert_eq!(hl.language_id_for_path("file.css"), Some("css".to_string()));
        assert_eq!(hl.language_id_for_path("file.json"), Some("json".to_string()));
        assert_eq!(hl.language_id_for_path("file.yaml"), Some("yaml".to_string()));
        assert_eq!(hl.language_id_for_path("file.md"), Some("markdown".to_string()));
        assert_eq!(hl.language_id_for_path("file.sql"), Some("sql".to_string()));
        assert_eq!(hl.language_id_for_path("file.toml"), Some("toml".to_string()));
    }

    #[test]
    fn language_id_for_unknown_extension() {
        let hl = SyntaxHighlighter::new();
        assert_eq!(hl.language_id_for_path("file.zzzzunknown"), None);
    }

    #[test]
    fn language_id_for_no_extension() {
        let hl = SyntaxHighlighter::new();
        assert_eq!(hl.language_id_for_path("Makefile"), None);
    }

    #[test]
    fn invalidate_cache_methods() {
        let mut hl = SyntaxHighlighter::new();
        let spans1 = {
            let syntax = hl.syntax_for_file("test.rs").unwrap();
            hl.highlight_line("let a = 1;\n", syntax)
        };
        hl.cache.set(0, spans1);
        let spans2 = {
            let syntax = hl.syntax_for_file("test.rs").unwrap();
            hl.highlight_line("let b = 2;\n", syntax)
        };
        hl.cache.set(1, spans2);
        assert_eq!(hl.cache.cached_count(), 2);
        hl.invalidate_line(0);
        assert_eq!(hl.cache.cached_count(), 1);
        hl.clear_cache();
        assert_eq!(hl.cache.cached_count(), 0);
    }

    #[test]
    fn set_theme_clears_cache() {
        let mut hl = SyntaxHighlighter::new();
        let spans = {
            let syntax = hl.syntax_for_file("test.rs").unwrap();
            hl.highlight_line("let a = 1;\n", syntax)
        };
        hl.cache.set(0, spans);
        assert_eq!(hl.cache.cached_count(), 1);
        let themes = SyntaxHighlighter::available_themes();
        if let Some(t) = themes.iter().find(|t| *t != "base16-ocean.dark") {
            hl.set_theme(t);
            assert_eq!(hl.cache.cached_count(), 0);
        }
    }

    #[test]
    fn tokens_in_line_rust() {
        let hl = SyntaxHighlighter::new();
        let syntax = hl.syntax_for_file("test.rs").unwrap();
        let tokens = hl.tokens_in_line("let x = 42;\n", syntax);
        assert!(!tokens.is_empty());
        let all_text: String = tokens.iter().map(|t| t.text.as_str()).collect();
        assert!(all_text.contains("let"));
    }

    #[test]
    fn theme_palette_from_different_themes() {
        let themes = SyntaxHighlighter::available_themes();
        for theme_name in &themes {
            let hl = SyntaxHighlighter::with_theme(theme_name);
            let palette = hl.palette();
            // All themes should produce valid palette
            let _ = format!("{:?}", palette);
        }
    }

    #[test]
    fn colored_span_partial_eq() {
        let a = ColoredSpan::plain("hello");
        let b = ColoredSpan::plain("hello");
        assert_eq!(a, b);
        let c = ColoredSpan::with_fg("hello", 255, 0, 0);
        assert_ne!(a, c);
    }

    #[test]
    fn token_info_partial_eq() {
        let a = TokenInfo {
            text: "let".to_string(),
            scope: "keyword".to_string(),
            start_col: 0,
            end_col: 3,
        };
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn detected_language_partial_eq() {
        let a = DetectedLanguage {
            name: "Rust".to_string(),
            extensions: vec!["rs".to_string()],
        };
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn highlight_cache_default() {
        let cache = HighlightCache::default();
        assert_eq!(cache.cached_count(), 0);
    }

    #[test]
    fn syntax_highlighter_default() {
        let hl = SyntaxHighlighter::default();
        assert_eq!(hl.theme_name(), "base16-ocean.dark");
    }

    // ---- ScopeStack tests ----

    #[test]
    fn scope_stack_push_pop() {
        let mut stack = ScopeStack::new();
        assert!(stack.is_empty());
        stack.push("source.rust");
        stack.push("meta.function");
        stack.push("entity.name.function");
        assert_eq!(stack.depth(), 3);
        assert_eq!(stack.top(), Some("entity.name.function"));
        assert_eq!(stack.bottom(), Some("source.rust"));
        let popped = stack.pop();
        assert_eq!(popped, Some("entity.name.function".to_string()));
        assert_eq!(stack.depth(), 2);
    }

    #[test]
    fn scope_stack_from_str() {
        let stack = ScopeStack::from_str("source.rust meta.function entity.name");
        assert_eq!(stack.depth(), 3);
        assert_eq!(stack.to_scope_string(), "source.rust meta.function entity.name");
    }

    #[test]
    fn scope_stack_matches_selector() {
        let stack = ScopeStack::from_str("source.rust meta.function entity.name.function");
        assert!(stack.matches_selector("source.rust"));
        assert!(stack.matches_selector("entity.name"));
        assert!(!stack.matches_selector("source.python"));
    }

    #[test]
    fn scope_stack_specificity_scoring() {
        let stack = ScopeStack::from_str("source.rust meta.function entity.name.function");
        let score_source = stack.specificity("source");
        let score_entity = stack.specificity("entity.name.function");
        // entity.name.function is deeper and more specific
        assert!(score_entity > score_source);
    }

    #[test]
    fn scope_stack_best_match() {
        let stack = ScopeStack::from_str("source.rust entity.name.function");
        let selectors = vec!["source", "entity.name", "entity.name.function"];
        let best = stack.best_match(&selectors);
        assert_eq!(best, Some("entity.name.function"));
    }

    // ---- ScopeSelector tests ----

    #[test]
    fn scope_selector_exact_match() {
        let sel = ScopeSelector::new("source.rust");
        assert!(sel.matches("source.rust"));
        assert!(!sel.matches("source.python"));
        assert!(!sel.matches("source"));
    }

    #[test]
    fn scope_selector_wildcard() {
        let sel = ScopeSelector::new("source.*");
        assert!(sel.matches("source.rust"));
        assert!(sel.matches("source.python"));
        assert!(!sel.matches("meta.function"));
    }

    #[test]
    fn scope_selector_double_wildcard() {
        let sel = ScopeSelector::new("**.function");
        assert!(sel.matches("entity.name.function"));
        assert!(sel.matches("meta.function"));
        assert!(sel.matches("function"));
    }

    #[test]
    fn scope_selector_specificity() {
        let exact = ScopeSelector::new("source.rust");
        let wild = ScopeSelector::new("source.*");
        let s_exact = exact.specificity("source.rust");
        let s_wild = wild.specificity("source.rust");
        assert!(s_exact > s_wild);
    }

    #[test]
    fn resolve_scope_stack_best_selector() {
        let stack = ScopeStack::from_str("source.rust entity.name.function");
        let sel1 = ScopeSelector::new("source.*");
        let sel2 = ScopeSelector::new("entity.name.function");
        let selectors: Vec<&ScopeSelector> = vec![&sel1, &sel2];
        let best = resolve_scope_stack(&stack, &selectors);
        assert_eq!(best.unwrap().pattern, "entity.name.function");
    }
}
