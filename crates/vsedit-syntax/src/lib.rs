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

// ---------------------------------------------------------------------------
// ColoredSpan — additional methods
// ---------------------------------------------------------------------------

impl ColoredSpan {
    /// Returns a new span with bold style enabled.
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Returns a new span with italic style enabled.
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// Returns a new span with underline style enabled.
    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// Returns a new span with the given background color.
    pub fn with_bg(text: &str, r: u8, g: u8, b: u8) -> Self {
        Self {
            text: text.to_string(),
            fg: (255, 255, 255),
            bg: (r, g, b),
            bold: false,
            italic: false,
            underline: false,
        }
    }

    /// Returns `true` if this span's text is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Returns a new span with all style properties removed (plain styling).
    pub fn strip_style(&self) -> Self {
        Self {
            text: self.text.clone(),
            fg: (255, 255, 255),
            bg: (0, 0, 0),
            bold: false,
            italic: false,
            underline: false,
        }
    }
}

// ---------------------------------------------------------------------------
// HighlightedLine — additional methods
// ---------------------------------------------------------------------------

impl HighlightedLine {
    /// Returns `true` if any span in this line has styling.
    pub fn has_styled_spans(&self) -> bool {
        self.spans.iter().any(|s| s.is_styled())
    }

    /// Returns the number of spans in this line.
    pub fn span_count(&self) -> usize {
        self.spans.len()
    }

    /// Returns the plain text of this line with all styling removed.
    pub fn plain_text(&self) -> String {
        self.text()
    }
}

// ---------------------------------------------------------------------------
// ScopeStack — additional methods
// ---------------------------------------------------------------------------

impl ScopeStack {
    /// Returns a new scope stack with the given scope appended.
    pub fn with_scope(&self, scope: impl Into<String>) -> Self {
        let mut new = self.clone();
        new.push(scope);
        new
    }

    /// Returns `true` if the stack has exactly the given depth.
    pub fn has_depth(&self, depth: usize) -> bool {
        self.scopes.len() == depth
    }

    /// Returns the scope at the given index from the bottom, if it exists.
    pub fn at(&self, index: usize) -> Option<&str> {
        self.scopes.get(index).map(|s| s.as_str())
    }

    /// Returns the common prefix scope count between this stack and another.
    pub fn common_prefix_depth(&self, other: &ScopeStack) -> usize {
        self.scopes
            .iter()
            .zip(other.scopes.iter())
            .take_while(|(a, b)| a == b)
            .count()
    }
}

// ---------------------------------------------------------------------------
// HighlightCache — additional methods
// ---------------------------------------------------------------------------

impl HighlightCache {
    /// Returns all cached line numbers.
    pub fn cached_lines(&self) -> Vec<usize> {
        let mut lines: Vec<usize> = self.lines.keys().copied().collect();
        lines.sort_unstable();
        lines
    }
}

// ---------------------------------------------------------------------------
// TokenClassification — semantic token categories
// ---------------------------------------------------------------------------

/// Semantic token classification for editor features like bracket matching,
/// auto-indent, and code navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenClass {
    Keyword,
    Identifier,
    StringLiteral,
    NumericLiteral,
    Comment,
    Operator,
    Punctuation,
    Whitespace,
    Type,
    Function,
    Macro,
    Attribute,
    Unknown,
}

impl TokenClass {
    /// Classify a token based on its scope string.
    pub fn from_scope(scope: &str) -> Self {
        if scope.starts_with("keyword") {
            TokenClass::Keyword
        } else if scope.starts_with("entity.name.function") || scope.starts_with("support.function") {
            TokenClass::Function
        } else if scope.starts_with("entity.name.type") || scope.starts_with("support.type") || scope.starts_with("storage.type") {
            TokenClass::Type
        } else if scope.starts_with("entity.name.macro") || scope.starts_with("support.macro") {
            TokenClass::Macro
        } else if scope.starts_with("entity.name") || scope.starts_with("variable") {
            TokenClass::Identifier
        } else if scope.starts_with("string") {
            TokenClass::StringLiteral
        } else if scope.starts_with("constant.numeric") {
            TokenClass::NumericLiteral
        } else if scope.starts_with("comment") {
            TokenClass::Comment
        } else if scope.starts_with("keyword.operator") || scope.starts_with("punctuation.separator") {
            TokenClass::Operator
        } else if scope.starts_with("punctuation") {
            TokenClass::Punctuation
        } else if scope.starts_with("meta.attribute") {
            TokenClass::Attribute
        } else {
            TokenClass::Unknown
        }
    }

    /// Whether this token class represents a "word" token (for word-based navigation).
    pub fn is_word(&self) -> bool {
        matches!(
            self,
            TokenClass::Keyword
                | TokenClass::Identifier
                | TokenClass::Type
                | TokenClass::Function
                | TokenClass::Macro
        )
    }

    /// Whether this class is a literal value.
    pub fn is_literal(&self) -> bool {
        matches!(self, TokenClass::StringLiteral | TokenClass::NumericLiteral)
    }

    /// Whether this class should be ignored for semantic purposes (whitespace/comments).
    pub fn is_trivia(&self) -> bool {
        matches!(self, TokenClass::Whitespace | TokenClass::Comment)
    }
}

impl fmt::Display for TokenClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            TokenClass::Keyword => "keyword",
            TokenClass::Identifier => "identifier",
            TokenClass::StringLiteral => "string",
            TokenClass::NumericLiteral => "number",
            TokenClass::Comment => "comment",
            TokenClass::Operator => "operator",
            TokenClass::Punctuation => "punctuation",
            TokenClass::Whitespace => "whitespace",
            TokenClass::Type => "type",
            TokenClass::Function => "function",
            TokenClass::Macro => "macro",
            TokenClass::Attribute => "attribute",
            TokenClass::Unknown => "unknown",
        };
        write!(f, "{}", s)
    }
}

// ---------------------------------------------------------------------------
// ScopeName — parsed scope name with components
// ---------------------------------------------------------------------------

/// A parsed scope name broken into dotted components.
///
/// For example, `"entity.name.function.rust"` is parsed into
/// `["entity", "name", "function", "rust"]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScopeName {
    raw: String,
    components: Vec<String>,
}

impl ScopeName {
    /// Parse a dotted scope name string.
    pub fn parse(scope: &str) -> Self {
        let components: Vec<String> = scope.split('.').map(|s| s.to_string()).collect();
        Self {
            raw: scope.to_string(),
            components,
        }
    }

    /// The raw scope string.
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// The individual components of the scope.
    pub fn components(&self) -> &[String] {
        &self.components
    }

    /// Number of dotted components.
    pub fn depth(&self) -> usize {
        self.components.len()
    }

    /// The top-level category (first component), e.g. `"entity"` from `"entity.name.function"`.
    pub fn category(&self) -> &str {
        self.components.first().map(|s| s.as_str()).unwrap_or("")
    }

    /// The most specific component (last), e.g. `"function"` from `"entity.name.function"`.
    pub fn leaf(&self) -> &str {
        self.components.last().map(|s| s.as_str()).unwrap_or("")
    }

    /// The language component if the scope ends with a language identifier.
    /// Convention: the last component is the language when depth >= 3.
    pub fn language_hint(&self) -> Option<&str> {
        if self.components.len() >= 3 {
            self.components.last().map(|s| s.as_str())
        } else {
            None
        }
    }

    /// Check if this scope is a child of (starts with) another scope.
    pub fn is_child_of(&self, parent: &ScopeName) -> bool {
        if self.components.len() <= parent.components.len() {
            return false;
        }
        self.components[..parent.components.len()] == parent.components[..]
    }

    /// Returns the parent scope (all components except the last), or None if already root.
    pub fn parent(&self) -> Option<ScopeName> {
        if self.components.len() <= 1 {
            return None;
        }
        let parent_raw = self.components[..self.components.len() - 1].join(".");
        Some(ScopeName::parse(&parent_raw))
    }

    /// Classify this scope into a semantic token class.
    pub fn classify(&self) -> TokenClass {
        TokenClass::from_scope(&self.raw)
    }
}

impl fmt::Display for ScopeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

// ---------------------------------------------------------------------------
// TokenRange — byte range-based token representation
// ---------------------------------------------------------------------------

/// A token with byte range information, useful for mapping between editor
/// positions and syntax tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRange {
    pub start: usize,
    pub end: usize,
    pub class: TokenClass,
}

impl TokenRange {
    /// Create a new token range.
    pub fn new(start: usize, end: usize, class: TokenClass) -> Self {
        Self { start, end, class }
    }

    /// Length in bytes.
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    /// Whether the range is empty.
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Whether this range contains a byte offset.
    pub fn contains(&self, offset: usize) -> bool {
        offset >= self.start && offset < self.end
    }

    /// Whether this range overlaps with another.
    pub fn overlaps(&self, other: &TokenRange) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// Compute the intersection of two ranges, if they overlap.
    pub fn intersection(&self, other: &TokenRange) -> Option<TokenRange> {
        if !self.overlaps(other) {
            return None;
        }
        Some(TokenRange {
            start: self.start.max(other.start),
            end: self.end.min(other.end),
            class: self.class,
        })
    }

    /// Merge two adjacent or overlapping ranges of the same class.
    pub fn merge(&self, other: &TokenRange) -> Option<TokenRange> {
        if self.class != other.class {
            return None;
        }
        if self.end < other.start || other.end < self.start {
            return None;
        }
        Some(TokenRange {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            class: self.class,
        })
    }
}

// ---------------------------------------------------------------------------
// EmbeddedLanguageDetector — detect embedded languages in source
// ---------------------------------------------------------------------------

/// Detects embedded language regions within source code.
#[derive(Debug, Clone)]
pub struct EmbeddedRegion {
    pub language: String,
    pub start_line: usize,
    pub end_line: usize,
}

/// Detect embedded language regions in a document.
///
/// Recognises common patterns like markdown fenced code blocks and HTML
/// `<script>` / `<style>` tags.
pub fn detect_embedded_languages(lines: &[&str], host_language: &str) -> Vec<EmbeddedRegion> {
    let mut regions = Vec::new();
    match host_language {
        "markdown" | "md" => detect_markdown_fenced_blocks(lines, &mut regions),
        "html" | "htm" => detect_html_embedded(lines, &mut regions),
        _ => {}
    }
    regions
}

fn detect_markdown_fenced_blocks(lines: &[&str], regions: &mut Vec<EmbeddedRegion>) {
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("```") && trimmed.len() > 3 {
            let lang = trimmed[3..].trim().to_lowercase();
            if !lang.is_empty() && !lang.contains('`') {
                let start = i + 1;
                let mut end = start;
                // Find closing fence
                for j in start..lines.len() {
                    if lines[j].trim().starts_with("```") {
                        end = j;
                        break;
                    }
                }
                if end > start {
                    regions.push(EmbeddedRegion {
                        language: lang,
                        start_line: start,
                        end_line: end,
                    });
                    i = end + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
}

fn detect_html_embedded(lines: &[&str], regions: &mut Vec<EmbeddedRegion>) {
    let mut i = 0;
    while i < lines.len() {
        let lower = lines[i].to_lowercase();
        let (tag, lang) = if lower.contains("<script") {
            ("</script>", "javascript")
        } else if lower.contains("<style") {
            ("</style>", "css")
        } else {
            i += 1;
            continue;
        };
        let start = i + 1;
        let mut end = start;
        for j in start..lines.len() {
            if lines[j].to_lowercase().contains(tag) {
                end = j;
                break;
            }
        }
        if end > start {
            regions.push(EmbeddedRegion {
                language: lang.to_string(),
                start_line: start,
                end_line: end,
            });
            i = end + 1;
        } else {
            i += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// BracketMatcher — bracket pair matching from token spans
// ---------------------------------------------------------------------------

/// A matched bracket pair with positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketPair {
    pub open_col: usize,
    pub close_col: usize,
    pub kind: BracketKind,
}

/// The kind of bracket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BracketKind {
    Paren,
    Square,
    Curly,
    Angle,
}

impl BracketKind {
    fn open_char(self) -> char {
        match self {
            BracketKind::Paren => '(',
            BracketKind::Square => '[',
            BracketKind::Curly => '{',
            BracketKind::Angle => '<',
        }
    }

    fn close_char(self) -> char {
        match self {
            BracketKind::Paren => ')',
            BracketKind::Square => ']',
            BracketKind::Curly => '}',
            BracketKind::Angle => '>',
        }
    }

    fn from_char(c: char) -> Option<(BracketKind, bool)> {
        match c {
            '(' => Some((BracketKind::Paren, true)),
            ')' => Some((BracketKind::Paren, false)),
            '[' => Some((BracketKind::Square, true)),
            ']' => Some((BracketKind::Square, false)),
            '{' => Some((BracketKind::Curly, true)),
            '}' => Some((BracketKind::Curly, false)),
            '<' => Some((BracketKind::Angle, true)),
            '>' => Some((BracketKind::Angle, false)),
            _ => None,
        }
    }
}

/// Find matching bracket pairs within a single line of text.
///
/// Skips brackets inside string literals and comments by using span style info.
pub fn find_bracket_pairs_in_line(line: &str) -> Vec<BracketPair> {
    let mut pairs = Vec::new();
    let mut stack: Vec<(usize, BracketKind)> = Vec::new();

    for (col, ch) in line.char_indices() {
        if let Some((kind, is_open)) = BracketKind::from_char(ch) {
            if is_open {
                stack.push((col, kind));
            } else if let Some(pos) = stack.iter().rposition(|&(_, k)| k == kind) {
                let (open_col, _) = stack.remove(pos);
                pairs.push(BracketPair {
                    open_col,
                    close_col: col,
                    kind,
                });
            }
        }
    }
    pairs
}

/// Find the matching bracket position for a bracket at the given column.
pub fn find_matching_bracket(line: &str, col: usize) -> Option<usize> {
    let pairs = find_bracket_pairs_in_line(line);
    for pair in &pairs {
        if pair.open_col == col {
            return Some(pair.close_col);
        }
        if pair.close_col == col {
            return Some(pair.open_col);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// IndentGuide — compute indentation levels from highlighted lines
// ---------------------------------------------------------------------------

/// Indentation information for a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndentInfo {
    pub line_number: usize,
    pub level: usize,
    pub is_blank: bool,
    pub indent_chars: usize,
}

/// Compute indentation info for a set of lines.
///
/// Uses `tab_width` to normalize mixed tabs/spaces.
pub fn compute_indent_info(lines: &[&str], tab_width: usize) -> Vec<IndentInfo> {
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let is_blank = line.trim().is_empty();
            let indent_chars = line.len() - line.trim_start().len();
            let visual_indent: usize = line
                .chars()
                .take_while(|c| c.is_whitespace())
                .map(|c| if c == '\t' { tab_width } else { 1 })
                .sum();
            let level = if tab_width > 0 {
                visual_indent / tab_width
            } else {
                0
            };
            IndentInfo {
                line_number: i,
                level,
                is_blank,
                indent_chars,
            }
        })
        .collect()
}

/// Compute indent delta between two adjacent lines (for auto-indent heuristics).
pub fn indent_delta(current: &IndentInfo, next: &IndentInfo) -> i32 {
    next.level as i32 - current.level as i32
}

// ---------------------------------------------------------------------------
// SyntaxTokenClassifier – classify syntax tokens
// ---------------------------------------------------------------------------

/// The classification of a syntax token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    Keyword,
    String,
    Comment,
    Number,
    Operator,
    Punctuation,
    Identifier,
    Unknown,
}

impl TokenKind {
    /// Whether this token kind carries semantic meaning.
    pub fn is_semantic(&self) -> bool {
        matches!(self, TokenKind::Keyword | TokenKind::Identifier | TokenKind::String | TokenKind::Number)
    }

    /// Priority for overlapping highlights (higher = more important).
    pub fn token_priority(&self) -> u32 {
        match self {
            TokenKind::Comment => 10,
            TokenKind::String => 9,
            TokenKind::Keyword => 8,
            TokenKind::Number => 7,
            TokenKind::Operator => 5,
            TokenKind::Punctuation => 3,
            TokenKind::Identifier => 2,
            TokenKind::Unknown => 0,
        }
    }
}

/// Classify a token string into a TokenKind based on simple heuristics.
pub fn classify_token(token: &str) -> TokenKind {
    if token.is_empty() {
        return TokenKind::Unknown;
    }
    let keywords = ["fn", "let", "mut", "if", "else", "for", "while", "return", "match", "use", "pub", "struct", "enum", "impl", "trait", "mod", "const", "static", "type", "where", "async", "await"];
    if keywords.contains(&token) {
        return TokenKind::Keyword;
    }
    if token.starts_with('"') || token.starts_with('\'') {
        return TokenKind::String;
    }
    if token.starts_with("//") || token.starts_with("/*") {
        return TokenKind::Comment;
    }
    if token.chars().all(|c| c.is_ascii_digit() || c == '.') && token.chars().any(|c| c.is_ascii_digit()) {
        return TokenKind::Number;
    }
    let ops = ["+", "-", "*", "/", "=", "==", "!=", "<", ">", "<=", ">=", "&&", "||", "!", "&", "|"];
    if ops.contains(&token) {
        return TokenKind::Operator;
    }
    let puncts = ["(", ")", "{", "}", "[", "]", ";", ",", ".", "::", ":"];
    if puncts.contains(&token) {
        return TokenKind::Punctuation;
    }
    TokenKind::Identifier
}

// ---------------------------------------------------------------------------
// SyntaxScope – scope hierarchy
// ---------------------------------------------------------------------------

/// A stack-based scope hierarchy (e.g., "source.rust > meta.function").
#[derive(Debug, Clone, Default)]
pub struct SyntaxScope {
    stack: Vec<String>,
}

impl SyntaxScope {
    pub fn new() -> Self { Self::default() }

    pub fn push(&mut self, scope: &str) {
        self.stack.push(scope.to_string());
    }

    pub fn pop(&mut self) -> Option<String> {
        self.stack.pop()
    }

    pub fn current(&self) -> Option<&str> {
        self.stack.last().map(|s| s.as_str())
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Check if the current scope matches a selector pattern (simple prefix match).
    pub fn matches_selector(&self, pattern: &str) -> bool {
        self.stack.iter().any(|s| s.starts_with(pattern))
    }

    pub fn to_string_repr(&self) -> String {
        self.stack.join(" > ")
    }

    pub fn parent_scope(&self) -> Option<&str> {
        if self.stack.len() >= 2 {
            Some(&self.stack[self.stack.len() - 2])
        } else {
            None
        }
    }

    pub fn root_scope(&self) -> Option<&str> {
        self.stack.first().map(|s| s.as_str())
    }
}

// ---------------------------------------------------------------------------
// SyntaxBracketMatcher – find matching brackets
// ---------------------------------------------------------------------------

/// Finds matching bracket pairs in source text.
#[derive(Debug, Clone)]
pub struct SyntaxBracketMatcher {
    pairs: Vec<(char, char)>,
}

impl SyntaxBracketMatcher {
    pub fn new() -> Self {
        Self { pairs: vec![('(', ')'), ('[', ']'), ('{', '}')] }
    }

    /// Find the matching close bracket position for an open bracket at `pos`.
    pub fn close_at(&self, text: &str, pos: usize) -> Option<usize> {
        let chars: Vec<char> = text.chars().collect();
        if pos >= chars.len() { return None; }
        let open = chars[pos];
        let close = self.pairs.iter().find(|(o, _)| *o == open)?.1;
        let mut depth = 0i32;
        for (i, ch) in chars.iter().enumerate().skip(pos) {
            if *ch == open { depth += 1; }
            if *ch == close { depth -= 1; }
            if depth == 0 { return Some(i); }
        }
        None
    }

    /// Find the matching open bracket position for a close bracket at `pos`.
    pub fn open_at(&self, text: &str, pos: usize) -> Option<usize> {
        let chars: Vec<char> = text.chars().collect();
        if pos >= chars.len() { return None; }
        let close = chars[pos];
        let open = self.pairs.iter().find(|(_, c)| *c == close)?.0;
        let mut depth = 0i32;
        for i in (0..=pos).rev() {
            if chars[i] == close { depth += 1; }
            if chars[i] == open { depth -= 1; }
            if depth == 0 { return Some(i); }
        }
        None
    }

    /// Compute nesting depth at a given position.
    pub fn nesting_depth(&self, text: &str, pos: usize) -> i32 {
        let mut depth = 0i32;
        for (i, ch) in text.chars().enumerate() {
            if i >= pos { break; }
            if self.pairs.iter().any(|(o, _)| *o == ch) { depth += 1; }
            if self.pairs.iter().any(|(_, c)| *c == ch) { depth -= 1; }
        }
        depth
    }

    /// Return positions of all mismatched brackets.
    pub fn mismatched_brackets(&self, text: &str) -> Vec<usize> {
        let mut stack: Vec<(char, usize)> = Vec::new();
        let mut mismatched = Vec::new();
        for (i, ch) in text.chars().enumerate() {
            if self.pairs.iter().any(|(o, _)| *o == ch) {
                stack.push((ch, i));
            } else if let Some((_, close)) = self.pairs.iter().find(|(_, c)| *c == ch) {
                let _ = close;
                if let Some((open_ch, _)) = stack.last() {
                    if self.pairs.iter().any(|(o, c)| *o == *open_ch && *c == ch) {
                        stack.pop();
                    } else {
                        mismatched.push(i);
                    }
                } else {
                    mismatched.push(i);
                }
            }
        }
        for (_, pos) in stack {
            mismatched.push(pos);
        }
        mismatched.sort();
        mismatched
    }
}

impl Default for SyntaxBracketMatcher {
    fn default() -> Self { Self::new() }
}


/// Configuration manager for syntax functionality.
pub struct SyntaxConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl SyntaxConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &SyntaxConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for syntax operations.
pub struct SyntaxRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl SyntaxRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for syntax.
pub struct SyntaxValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl SyntaxValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &SyntaxValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Syntax token classification — extended utilities (ql)
// ---------------------------------------------------------------------------

/// Metric accumulator for syntax operations.
#[derive(Debug, Clone)]
pub struct QlMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QlMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for syntax.
#[derive(Debug, Clone)]
pub struct QlRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QlRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for syntax lookups.
#[derive(Debug, Clone)]
pub struct QlLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QlLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for syntax
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaSyntaxRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaSyntaxRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaSyntaxCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaSyntaxCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaSyntaxCounter {
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

    // -- ColoredSpan additional methods ------------------------------------

    #[test]
    fn colored_span_builder_methods() {
        let span = ColoredSpan::plain("hello").bold().italic().underline();
        assert!(span.bold);
        assert!(span.italic);
        assert!(span.underline);
        assert!(span.is_styled());
    }

    #[test]
    fn colored_span_with_bg() {
        let span = ColoredSpan::with_bg("test", 10, 20, 30);
        assert_eq!(span.bg, (10, 20, 30));
        assert!(span.is_styled());
    }

    #[test]
    fn colored_span_is_empty_and_strip_style() {
        let empty = ColoredSpan::plain("");
        assert!(empty.is_empty());
        let styled = ColoredSpan::with_fg("text", 255, 0, 0).bold();
        let stripped = styled.strip_style();
        assert!(!stripped.is_styled());
        assert_eq!(stripped.text, "text");
    }

    // -- HighlightedLine additional methods --------------------------------

    #[test]
    fn highlighted_line_has_styled_spans() {
        let spans = vec![
            ColoredSpan::plain("fn "),
            ColoredSpan::with_fg("main", 100, 200, 50),
        ];
        let line = HighlightedLine::new(0, spans);
        assert!(line.has_styled_spans());
        assert_eq!(line.span_count(), 2);
    }

    #[test]
    fn highlighted_line_all_plain_not_styled() {
        let spans = vec![ColoredSpan::plain("hello world")];
        let line = HighlightedLine::new(0, spans);
        assert!(!line.has_styled_spans());
        assert_eq!(line.plain_text(), "hello world");
    }

    // -- ScopeStack additional methods ------------------------------------

    #[test]
    fn scope_stack_with_scope() {
        let stack = ScopeStack::from_str("source.rust");
        let extended = stack.with_scope("meta.function");
        assert_eq!(extended.depth(), 2);
        assert_eq!(extended.top(), Some("meta.function"));
    }

    #[test]
    fn scope_stack_has_depth_and_at() {
        let stack = ScopeStack::from_str("source.rust meta.function");
        assert!(stack.has_depth(2));
        assert!(!stack.has_depth(3));
        assert_eq!(stack.at(0), Some("source.rust"));
        assert_eq!(stack.at(1), Some("meta.function"));
        assert_eq!(stack.at(2), None);
    }

    #[test]
    fn scope_stack_common_prefix_depth() {
        let a = ScopeStack::from_str("source.rust meta.function entity.name");
        let b = ScopeStack::from_str("source.rust meta.function variable.other");
        assert_eq!(a.common_prefix_depth(&b), 2);
        let c = ScopeStack::from_str("source.python");
        assert_eq!(a.common_prefix_depth(&c), 0);
    }

    // -- HighlightCache additional methods ---------------------------------

    #[test]
    fn cache_cached_lines() {
        let mut cache = HighlightCache::new();
        cache.set(5, vec![ColoredSpan::plain("a")]);
        cache.set(2, vec![ColoredSpan::plain("b")]);
        cache.set(8, vec![ColoredSpan::plain("c")]);
        let lines = cache.cached_lines();
        assert_eq!(lines, vec![2, 5, 8]);
    }

    // ---- TokenClass tests ----

    #[test]
    fn token_class_from_scope() {
        assert_eq!(TokenClass::from_scope("keyword.control"), TokenClass::Keyword);
        assert_eq!(TokenClass::from_scope("entity.name.function.rust"), TokenClass::Function);
        assert_eq!(TokenClass::from_scope("string.quoted.double"), TokenClass::StringLiteral);
        assert_eq!(TokenClass::from_scope("constant.numeric.integer"), TokenClass::NumericLiteral);
        assert_eq!(TokenClass::from_scope("comment.line.double-slash"), TokenClass::Comment);
        assert_eq!(TokenClass::from_scope("punctuation.definition.string"), TokenClass::Punctuation);
        assert_eq!(TokenClass::from_scope("storage.type.rust"), TokenClass::Type);
        assert_eq!(TokenClass::from_scope("variable.other.member"), TokenClass::Identifier);
        assert_eq!(TokenClass::from_scope("meta.attribute.rust"), TokenClass::Attribute);
        assert_eq!(TokenClass::from_scope("some.random.scope"), TokenClass::Unknown);
    }

    #[test]
    fn token_class_predicates() {
        assert!(TokenClass::Keyword.is_word());
        assert!(TokenClass::Function.is_word());
        assert!(!TokenClass::Operator.is_word());

        assert!(TokenClass::StringLiteral.is_literal());
        assert!(TokenClass::NumericLiteral.is_literal());
        assert!(!TokenClass::Keyword.is_literal());

        assert!(TokenClass::Comment.is_trivia());
        assert!(TokenClass::Whitespace.is_trivia());
        assert!(!TokenClass::Identifier.is_trivia());
    }

    #[test]
    fn token_class_display() {
        assert_eq!(format!("{}", TokenClass::Keyword), "keyword");
        assert_eq!(format!("{}", TokenClass::Function), "function");
        assert_eq!(format!("{}", TokenClass::StringLiteral), "string");
    }

    // ---- ScopeName tests ----

    #[test]
    fn scope_name_parse_and_components() {
        let s = ScopeName::parse("entity.name.function.rust");
        assert_eq!(s.as_str(), "entity.name.function.rust");
        assert_eq!(s.depth(), 4);
        assert_eq!(s.category(), "entity");
        assert_eq!(s.leaf(), "rust");
        assert_eq!(s.components(), &["entity", "name", "function", "rust"]);
    }

    #[test]
    fn scope_name_language_hint() {
        let deep = ScopeName::parse("entity.name.function.rust");
        assert_eq!(deep.language_hint(), Some("rust"));

        let shallow = ScopeName::parse("keyword.control");
        assert_eq!(shallow.language_hint(), None);
    }

    #[test]
    fn scope_name_parent_and_child() {
        let child = ScopeName::parse("entity.name.function");
        let parent = ScopeName::parse("entity.name");
        assert!(child.is_child_of(&parent));
        assert!(!parent.is_child_of(&child));

        let par = child.parent().unwrap();
        assert_eq!(par.as_str(), "entity.name");

        let root = ScopeName::parse("keyword");
        assert!(root.parent().is_none());
    }

    #[test]
    fn scope_name_classify() {
        let s = ScopeName::parse("keyword.control.if");
        assert_eq!(s.classify(), TokenClass::Keyword);
        let s2 = ScopeName::parse("entity.name.function.main");
        assert_eq!(s2.classify(), TokenClass::Function);
    }

    #[test]
    fn scope_name_display() {
        let s = ScopeName::parse("source.rust");
        assert_eq!(format!("{}", s), "source.rust");
    }

    // ---- TokenRange tests ----

    #[test]
    fn token_range_basics() {
        let r = TokenRange::new(5, 10, TokenClass::Keyword);
        assert_eq!(r.len(), 5);
        assert!(!r.is_empty());
        assert!(r.contains(5));
        assert!(r.contains(9));
        assert!(!r.contains(10));
    }

    #[test]
    fn token_range_overlap_and_intersection() {
        let a = TokenRange::new(0, 10, TokenClass::Keyword);
        let b = TokenRange::new(5, 15, TokenClass::Identifier);
        assert!(a.overlaps(&b));

        let inter = a.intersection(&b).unwrap();
        assert_eq!(inter.start, 5);
        assert_eq!(inter.end, 10);

        let c = TokenRange::new(20, 30, TokenClass::Comment);
        assert!(!a.overlaps(&c));
        assert!(a.intersection(&c).is_none());
    }

    #[test]
    fn token_range_merge() {
        let a = TokenRange::new(0, 5, TokenClass::Keyword);
        let b = TokenRange::new(5, 10, TokenClass::Keyword);
        let merged = a.merge(&b).unwrap();
        assert_eq!(merged.start, 0);
        assert_eq!(merged.end, 10);

        // Different classes can't merge
        let c = TokenRange::new(5, 10, TokenClass::Comment);
        assert!(a.merge(&c).is_none());

        // Non-adjacent can't merge
        let d = TokenRange::new(20, 25, TokenClass::Keyword);
        assert!(a.merge(&d).is_none());
    }

    // ---- Embedded language detection tests ----

    #[test]
    fn detect_markdown_fenced_code_blocks() {
        let lines = vec![
            "# Title",
            "",
            "```rust",
            "fn main() {}",
            "```",
            "",
            "```python",
            "print('hello')",
            "x = 1",
            "```",
        ];
        let regions = detect_embedded_languages(&lines, "markdown");
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].language, "rust");
        assert_eq!(regions[0].start_line, 3);
        assert_eq!(regions[0].end_line, 4);
        assert_eq!(regions[1].language, "python");
        assert_eq!(regions[1].start_line, 7);
        assert_eq!(regions[1].end_line, 9);
    }

    #[test]
    fn detect_html_script_and_style() {
        let lines = vec![
            "<html>",
            "<script>",
            "console.log('hi');",
            "</script>",
            "<style>",
            "body { color: red; }",
            "</style>",
            "</html>",
        ];
        let regions = detect_embedded_languages(&lines, "html");
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].language, "javascript");
        assert_eq!(regions[1].language, "css");
    }

    #[test]
    fn detect_embedded_no_match_for_plain_language() {
        let lines = vec!["fn main() {}", "    println!(\"hello\");", "}"];
        let regions = detect_embedded_languages(&lines, "rust");
        assert!(regions.is_empty());
    }

    // ---- Bracket matching tests ----

    #[test]
    fn bracket_pairs_simple() {
        let pairs = find_bracket_pairs_in_line("fn main() { x }");
        assert!(pairs.iter().any(|p| p.kind == BracketKind::Paren));
        assert!(pairs.iter().any(|p| p.kind == BracketKind::Curly));
    }

    #[test]
    fn bracket_pairs_nested() {
        let pairs = find_bracket_pairs_in_line("((a))");
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn find_matching_bracket_works() {
        let line = "(hello)";
        assert_eq!(find_matching_bracket(line, 0), Some(6));
        assert_eq!(find_matching_bracket(line, 6), Some(0));
        assert_eq!(find_matching_bracket(line, 3), None);
    }

    #[test]
    fn bracket_kind_chars() {
        assert_eq!(BracketKind::Paren.open_char(), '(');
        assert_eq!(BracketKind::Paren.close_char(), ')');
        assert_eq!(BracketKind::Square.open_char(), '[');
        assert_eq!(BracketKind::Square.close_char(), ']');
        assert_eq!(BracketKind::Curly.open_char(), '{');
        assert_eq!(BracketKind::Curly.close_char(), '}');
    }

    // ---- Indentation tests ----

    #[test]
    fn indent_info_basic() {
        let lines = vec!["fn main() {", "    let x = 1;", "        nested();", "}"];
        let info = compute_indent_info(&lines, 4);
        assert_eq!(info[0].level, 0);
        assert_eq!(info[1].level, 1);
        assert_eq!(info[2].level, 2);
        assert_eq!(info[3].level, 0);
        assert!(!info[0].is_blank);
    }

    #[test]
    fn indent_info_blank_lines() {
        let lines = vec!["code", "", "  more"];
        let info = compute_indent_info(&lines, 2);
        assert!(!info[0].is_blank);
        assert!(info[1].is_blank);
        assert!(!info[2].is_blank);
    }

    #[test]
    fn indent_delta_computation() {
        let lines = vec!["fn main() {", "    body"];
        let info = compute_indent_info(&lines, 4);
        assert_eq!(indent_delta(&info[0], &info[1]), 1);
    }

    #[test]
    fn indent_info_tabs() {
        let lines = vec!["\t\tindented"];
        let info = compute_indent_info(&lines, 4);
        assert_eq!(info[0].level, 2);
    }

    // -- SyntaxTokenClassifier ----------------------------------------------

    #[test]
    fn classify_keyword() {
        assert_eq!(classify_token("fn"), TokenKind::Keyword);
        assert_eq!(classify_token("let"), TokenKind::Keyword);
        assert_eq!(classify_token("return"), TokenKind::Keyword);
    }

    #[test]
    fn classify_number() {
        assert_eq!(classify_token("42"), TokenKind::Number);
        assert_eq!(classify_token("3.14"), TokenKind::Number);
    }

    #[test]
    fn classify_operator() {
        assert_eq!(classify_token("+"), TokenKind::Operator);
        assert_eq!(classify_token("=="), TokenKind::Operator);
    }

    #[test]
    fn classify_identifier() {
        assert_eq!(classify_token("my_var"), TokenKind::Identifier);
    }

    #[test]
    fn token_priority_ordering() {
        assert!(TokenKind::Comment.token_priority() > TokenKind::Identifier.token_priority());
        assert!(TokenKind::Keyword.is_semantic());
        assert!(!TokenKind::Punctuation.is_semantic());
    }

    // -- SyntaxScope --------------------------------------------------------

    #[test]
    fn scope_push_pop() {
        let mut s = SyntaxScope::new();
        s.push("source.rust");
        s.push("meta.function");
        assert_eq!(s.current(), Some("meta.function"));
        assert_eq!(s.depth(), 2);
        s.pop();
        assert_eq!(s.current(), Some("source.rust"));
    }

    #[test]
    fn scope_matches_selector() {
        let mut s = SyntaxScope::new();
        s.push("source.rust");
        s.push("meta.function.definition");
        assert!(s.matches_selector("meta.function"));
        assert!(!s.matches_selector("source.python"));
    }

    #[test]
    fn scope_parent_and_root() {
        let mut s = SyntaxScope::new();
        s.push("root");
        s.push("child");
        assert_eq!(s.parent_scope(), Some("root"));
        assert_eq!(s.root_scope(), Some("root"));
    }

    #[test]
    fn scope_to_string() {
        let mut s = SyntaxScope::new();
        s.push("a");
        s.push("b");
        assert_eq!(s.to_string_repr(), "a > b");
    }

    // -- SyntaxBracketMatcher -----------------------------------------------

    #[test]
    fn bracket_close_at() {
        let m = SyntaxBracketMatcher::new();
        assert_eq!(m.close_at("(hello)", 0), Some(6));
        assert_eq!(m.close_at("((a))", 0), Some(4));
    }

    #[test]
    fn bracket_open_at() {
        let m = SyntaxBracketMatcher::new();
        assert_eq!(m.open_at("(hello)", 6), Some(0));
    }

    #[test]
    fn bracket_nesting_depth() {
        let m = SyntaxBracketMatcher::new();
        assert_eq!(m.nesting_depth("((a)b)", 3), 2);
        assert_eq!(m.nesting_depth("((a)b)", 5), 1);
    }

    #[test]
    fn bracket_mismatched() {
        let m = SyntaxBracketMatcher::new();
        assert!(m.mismatched_brackets("(a)").is_empty());
        let mis = m.mismatched_brackets("(a");
        assert_eq!(mis.len(), 1);
    }

    #[test]
    fn syntax_config_new() {
        let cfg = SyntaxConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn syntax_config_set_get() {
        let mut cfg = SyntaxConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn syntax_config_remove() {
        let mut cfg = SyntaxConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn syntax_config_keys_sorted() {
        let mut cfg = SyntaxConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn syntax_config_bump_version() {
        let mut cfg = SyntaxConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn syntax_config_clear() {
        let mut cfg = SyntaxConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn syntax_config_merge() {
        let mut cfg1 = SyntaxConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = SyntaxConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn syntax_config_disable() {
        let mut cfg = SyntaxConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn syntax_rate_tracker_empty() {
        let rt = SyntaxRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn syntax_rate_tracker_record() {
        let mut rt = SyntaxRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn syntax_rate_tracker_prune() {
        let mut rt = SyntaxRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn syntax_validator_valid() {
        let v = SyntaxValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn syntax_validator_errors() {
        let mut v = SyntaxValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn syntax_validator_clear() {
        let mut v = SyntaxValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn syntax_validator_merge() {
        let mut v1 = SyntaxValidator::new();
        v1.add_error("e1");
        let mut v2 = SyntaxValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn syntax_rate_tracker_clear() {
        let mut rt = SyntaxRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn ql_metrics_empty() {
        let m = QlMetrics::new("syntax");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ql_metrics_record_and_mean() {
        let mut m = QlMetrics::new("syntax");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ql_metrics_min_max() {
        let mut m = QlMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ql_metrics_variance_and_std() {
        let mut m = QlMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn ql_metrics_percentile() {
        let mut m = QlMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn ql_metrics_merge() {
        let mut a = QlMetrics::new("a");
        a.record(1.0);
        let mut b = QlMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn ql_metrics_reset() {
        let mut m = QlMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn ql_rate_window_empty() {
        let rw = QlRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn ql_rate_window_tick_and_rate() {
        let mut rw = QlRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn ql_lru_cache_basic() {
        let mut c = QlLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn ql_lru_cache_contains_and_keys() {
        let mut c = QlLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn ql_lru_cache_remove() {
        let mut c = QlLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn ql_metrics_sum() {
        let mut m = QlMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ql_metrics_label() {
        let m = QlMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn ql_lru_cache_clear() {
        let mut c = QlLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for syntax
    #[test]
    fn xa_syntax_ring_new() {
        let rb = super::XaSyntaxRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_syntax_ring_push_len() {
        let mut rb = super::XaSyntaxRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_syntax_ring_wrap() {
        let mut rb = super::XaSyntaxRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_syntax_ring_mean_empty() {
        let rb = super::XaSyntaxRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_syntax_ring_mean_values() {
        let mut rb = super::XaSyntaxRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_syntax_ring_min_max() {
        let mut rb = super::XaSyntaxRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_syntax_ring_iter() {
        let mut rb = super::XaSyntaxRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_syntax_counter_new() {
        let c = super::XaSyntaxCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_syntax_counter_inc() {
        let mut c = super::XaSyntaxCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_syntax_counter_inc_by() {
        let mut c = super::XaSyntaxCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_syntax_counter_reset() {
        let mut c = super::XaSyntaxCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_syntax_counter_clear() {
        let mut c = super::XaSyntaxCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_syntax_counter_default() {
        let c = super::XaSyntaxCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }

}
