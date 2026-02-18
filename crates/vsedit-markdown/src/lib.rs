//! Markdown tokenisation and plain-text rendering.
//!
//! Provides a lightweight inline tokeniser that recognises common Markdown
//! constructs without any external dependencies.

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Tokens produced by the inline tokeniser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownToken {
    Text(String),
    Bold(String),
    Italic(String),
    Code(String),
    CodeBlock(String, Option<String>),
    Link(String, String),
    Heading(u8, String),
    ListItemMd(String),
    Paragraph,
    LineBreak,
}

// ---------------------------------------------------------------------------
// Inline tokeniser
// ---------------------------------------------------------------------------

/// Tokenise a single line (or short block) of inline Markdown.
///
/// Handles `**bold**`, `*italic*`, `` `code` ``, and `[text](url)`.
/// Unrecognised text is emitted as `Text` tokens.
pub fn tokenize_inline(text: &str) -> Vec<MarkdownToken> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut buf = String::new();

    while i < len {
        // Bold: **...**
        if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
            if let Some(end) = find_closing(&chars, i + 2, &['*', '*']) {
                flush_buf(&mut buf, &mut tokens);
                let content: String = chars[i + 2..end].iter().collect();
                tokens.push(MarkdownToken::Bold(content));
                i = end + 2;
                continue;
            }
        }

        // Italic: *...*
        if chars[i] == '*' {
            if let Some(end) = find_single_closing(&chars, i + 1, '*') {
                flush_buf(&mut buf, &mut tokens);
                let content: String = chars[i + 1..end].iter().collect();
                tokens.push(MarkdownToken::Italic(content));
                i = end + 1;
                continue;
            }
        }

        // Inline code: `...`
        if chars[i] == '`' {
            if let Some(end) = find_single_closing(&chars, i + 1, '`') {
                flush_buf(&mut buf, &mut tokens);
                let content: String = chars[i + 1..end].iter().collect();
                tokens.push(MarkdownToken::Code(content));
                i = end + 1;
                continue;
            }
        }

        // Link: [text](url)
        if chars[i] == '[' {
            if let Some(close_bracket) = find_single_closing(&chars, i + 1, ']') {
                if close_bracket + 1 < len && chars[close_bracket + 1] == '(' {
                    if let Some(close_paren) = find_single_closing(&chars, close_bracket + 2, ')')
                    {
                        flush_buf(&mut buf, &mut tokens);
                        let label: String = chars[i + 1..close_bracket].iter().collect();
                        let url: String =
                            chars[close_bracket + 2..close_paren].iter().collect();
                        tokens.push(MarkdownToken::Link(label, url));
                        i = close_paren + 1;
                        continue;
                    }
                }
            }
        }

        buf.push(chars[i]);
        i += 1;
    }

    flush_buf(&mut buf, &mut tokens);
    tokens
}

// ---------------------------------------------------------------------------
// Plain-text renderer
// ---------------------------------------------------------------------------

/// Render a slice of tokens to plain text by stripping all formatting.
pub fn render_to_plain_text(tokens: &[MarkdownToken]) -> String {
    let mut out = String::new();
    for token in tokens {
        match token {
            MarkdownToken::Text(t)
            | MarkdownToken::Bold(t)
            | MarkdownToken::Italic(t)
            | MarkdownToken::Code(t)
            | MarkdownToken::CodeBlock(t, _)
            | MarkdownToken::ListItemMd(t) => out.push_str(t),
            MarkdownToken::Link(label, _) => out.push_str(label),
            MarkdownToken::Heading(_, t) => out.push_str(t),
            MarkdownToken::Paragraph | MarkdownToken::LineBreak => out.push('\n'),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Block tokeniser
// ---------------------------------------------------------------------------

/// Tokenise a multi-line Markdown string into block-level tokens.
///
/// Recognises headings (`#`–`######`), list items (`- ` / `* `), fenced code
/// blocks (`` ``` ``), and paragraphs (separated by blank lines). Inline
/// formatting within each block is also tokenised.
pub fn tokenize_block(text: &str) -> Vec<MarkdownToken> {
    let mut tokens = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let len = lines.len();
    let mut i = 0;

    while i < len {
        let line = lines[i];

        // Blank line → paragraph separator
        if line.trim().is_empty() {
            tokens.push(MarkdownToken::Paragraph);
            i += 1;
            continue;
        }

        // Fenced code block
        if line.trim_start().starts_with("```") {
            let lang = line.trim_start().trim_start_matches('`').trim();
            let lang = if lang.is_empty() { None } else { Some(lang.to_string()) };
            let mut code = String::new();
            i += 1;
            while i < len && !lines[i].trim_start().starts_with("```") {
                if !code.is_empty() {
                    code.push('\n');
                }
                code.push_str(lines[i]);
                i += 1;
            }
            tokens.push(MarkdownToken::CodeBlock(code, lang));
            i += 1; // skip closing fence
            continue;
        }

        // Heading
        if line.starts_with('#') {
            let level = line.chars().take_while(|&c| c == '#').count().min(6) as u8;
            let content = line[level as usize..].trim().to_string();
            tokens.push(MarkdownToken::Heading(level, content));
            i += 1;
            continue;
        }

        // List item (- or *)
        let trimmed = line.trim_start();
        if (trimmed.starts_with("- ") || trimmed.starts_with("* ")) && trimmed.len() > 2 {
            let content = trimmed[2..].to_string();
            tokens.push(MarkdownToken::ListItemMd(content));
            i += 1;
            continue;
        }

        // Paragraph text – collect consecutive non-blank, non-special lines
        let mut para = String::new();
        while i < len {
            let l = lines[i];
            if l.trim().is_empty()
                || l.starts_with('#')
                || l.trim_start().starts_with("```")
                || l.trim_start().starts_with("- ")
                || l.trim_start().starts_with("* ")
            {
                break;
            }
            if !para.is_empty() {
                para.push(' ');
            }
            para.push_str(l.trim());
            i += 1;
        }
        let inline_tokens = tokenize_inline(&para);
        tokens.extend(inline_tokens);
    }

    tokens
}

// ---------------------------------------------------------------------------
// HTML renderer
// ---------------------------------------------------------------------------

/// Render a slice of tokens to an HTML string.
pub fn render_to_html(tokens: &[MarkdownToken]) -> String {
    let mut out = String::new();
    for token in tokens {
        match token {
            MarkdownToken::Text(t) => out.push_str(&html_escape(t)),
            MarkdownToken::Bold(t) => {
                out.push_str("<strong>");
                out.push_str(&html_escape(t));
                out.push_str("</strong>");
            }
            MarkdownToken::Italic(t) => {
                out.push_str("<em>");
                out.push_str(&html_escape(t));
                out.push_str("</em>");
            }
            MarkdownToken::Code(t) => {
                out.push_str("<code>");
                out.push_str(&html_escape(t));
                out.push_str("</code>");
            }
            MarkdownToken::CodeBlock(t, lang) => {
                if let Some(l) = lang {
                    out.push_str(&format!("<pre><code class=\"language-{}\">", html_escape(l)));
                } else {
                    out.push_str("<pre><code>");
                }
                out.push_str(&html_escape(t));
                out.push_str("</code></pre>");
            }
            MarkdownToken::Link(label, url) => {
                out.push_str(&format!(
                    "<a href=\"{}\">{}</a>",
                    html_escape(url),
                    html_escape(label)
                ));
            }
            MarkdownToken::Heading(level, t) => {
                out.push_str(&format!("<h{0}>{1}</h{0}>", level, html_escape(t)));
            }
            MarkdownToken::ListItemMd(t) => {
                out.push_str("<li>");
                out.push_str(&html_escape(t));
                out.push_str("</li>");
            }
            MarkdownToken::Paragraph => out.push_str("<p></p>"),
            MarkdownToken::LineBreak => out.push_str("<br>"),
        }
    }
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ---------------------------------------------------------------------------
// Convenience & extraction helpers
// ---------------------------------------------------------------------------

/// Tokenise then render to plain text in one step.
pub fn strip_markdown(text: &str) -> String {
    let tokens = tokenize_inline(text);
    render_to_plain_text(&tokens)
}

/// Extract all links as `(label, url)` pairs from a token slice.
pub fn extract_links(tokens: &[MarkdownToken]) -> Vec<(String, String)> {
    tokens
        .iter()
        .filter_map(|t| {
            if let MarkdownToken::Link(label, url) = t {
                Some((label.clone(), url.clone()))
            } else {
                None
            }
        })
        .collect()
}

/// Extract all headings as `(level, text)` pairs from a token slice.
pub fn extract_headings(tokens: &[MarkdownToken]) -> Vec<(u8, String)> {
    tokens
        .iter()
        .filter_map(|t| {
            if let MarkdownToken::Heading(level, text) = t {
                Some((*level, text.clone()))
            } else {
                None
            }
        })
        .collect()
}

/// Count words across all text-bearing tokens.
pub fn word_count(tokens: &[MarkdownToken]) -> usize {
    tokens
        .iter()
        .map(|t| {
            let s = match t {
                MarkdownToken::Text(s)
                | MarkdownToken::Bold(s)
                | MarkdownToken::Italic(s)
                | MarkdownToken::Code(s)
                | MarkdownToken::CodeBlock(s, _)
                | MarkdownToken::ListItemMd(s) => s.as_str(),
                MarkdownToken::Link(label, _) => label.as_str(),
                MarkdownToken::Heading(_, s) => s.as_str(),
                MarkdownToken::Paragraph | MarkdownToken::LineBreak => "",
            };
            s.split_whitespace().count()
        })
        .sum()
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration options for Markdown processing.
#[derive(Debug, Clone)]
pub struct MarkdownConfig {
    /// Treat single newlines as hard line breaks (like GitHub-flavoured Markdown).
    pub hard_line_breaks: bool,
    /// Replace straight quotes with curly/smart quotes.
    pub smart_quotes: bool,
}

impl Default for MarkdownConfig {
    fn default() -> Self {
        Self {
            hard_line_breaks: false,
            smart_quotes: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn flush_buf(buf: &mut String, tokens: &mut Vec<MarkdownToken>) {
    if !buf.is_empty() {
        tokens.push(MarkdownToken::Text(buf.clone()));
        buf.clear();
    }
}

/// Find closing pair of two identical chars (e.g. `**`).
fn find_closing(chars: &[char], from: usize, pair: &[char; 2]) -> Option<usize> {
    let len = chars.len();
    let mut j = from;
    while j + 1 < len {
        if chars[j] == pair[0] && chars[j + 1] == pair[1] {
            return Some(j);
        }
        j += 1;
    }
    None
}

/// Find a single closing delimiter.
fn find_single_closing(chars: &[char], from: usize, delim: char) -> Option<usize> {
    chars.iter().enumerate().skip(from).find_map(|(j, &c)| if c == delim { Some(j) } else { None })
}

// ---------------------------------------------------------------------------
// Table parsing
// ---------------------------------------------------------------------------

/// A single row of a parsed Markdown table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRow {
    pub cells: Vec<String>,
}

/// A parsed Markdown table with header and data rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownTable {
    /// Header row (first row of the table).
    pub headers: Vec<String>,
    /// Data rows (everything after the separator).
    pub rows: Vec<Vec<String>>,
}

impl MarkdownTable {
    /// Parse a pipe-delimited Markdown table from text.
    /// Returns `None` if the input is not a valid table.
    pub fn parse(text: &str) -> Option<Self> {
        let all_rows = parse_markdown_table(text)?;
        if all_rows.is_empty() {
            return None;
        }
        let headers = all_rows[0].cells.clone();
        let rows = all_rows[1..].iter().map(|r| r.cells.clone()).collect();
        Some(Self { headers, rows })
    }

    /// Render the table back to a Markdown string.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("| ");
        out.push_str(&self.headers.join(" | "));
        out.push_str(" |\n| ");
        out.push_str(&self.headers.iter().map(|_| "---").collect::<Vec<_>>().join(" | "));
        out.push_str(" |\n");
        for row in &self.rows {
            out.push_str("| ");
            out.push_str(&row.join(" | "));
            out.push_str(" |\n");
        }
        out
    }

    /// Number of columns in the table.
    pub fn column_count(&self) -> usize {
        self.headers.len()
    }

    /// Number of data rows (excluding header).
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
}

/// Parse a simple pipe-delimited Markdown table.
///
/// Expects at least a header row and a separator row (`|---|---|`).
/// Returns `None` if the input does not look like a valid table.
pub fn parse_markdown_table(text: &str) -> Option<Vec<TableRow>> {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() < 2 {
        return None;
    }

    let parse_row = |line: &str| -> Vec<String> {
        let trimmed = line.trim();
        let trimmed = trimmed.strip_prefix('|').unwrap_or(trimmed);
        let trimmed = trimmed.strip_suffix('|').unwrap_or(trimmed);
        trimmed.split('|').map(|c| c.trim().to_string()).collect()
    };

    // Validate separator row (second line must contain only dashes, pipes, colons, spaces)
    let sep = lines[1].trim();
    if !sep.chars().all(|c| c == '-' || c == '|' || c == ':' || c == ' ') {
        return None;
    }

    let mut rows = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        if idx == 1 {
            continue; // skip separator
        }
        rows.push(TableRow {
            cells: parse_row(line),
        });
    }
    Some(rows)
}

/// Extract all `[label](url)` links from raw markdown text, returning `(label, url)` pairs.
pub fn extract_links_from_text(text: &str) -> Vec<(String, String)> {
    let mut links = Vec::new();
    for line in text.lines() {
        let tokens = tokenize_inline(line);
        for token in tokens {
            if let MarkdownToken::Link(label, url) = token {
                links.push((label, url));
            }
        }
    }
    links
}

/// Extract all headings from raw markdown text, returning `(level, text)` pairs
/// suitable for building a table of contents.
pub fn extract_headings_from_text(text: &str) -> Vec<(u8, String)> {
    let tokens = tokenize_block(text);
    extract_headings(&tokens)
}

// ---------------------------------------------------------------------------
// Statistics
// ---------------------------------------------------------------------------

/// Aggregate statistics about a token stream.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MarkdownStats {
    pub headings: usize,
    pub links: usize,
    pub code_blocks: usize,
    pub list_items: usize,
    pub paragraphs: usize,
    pub total_tokens: usize,
}

/// Compute aggregate statistics from a token slice.
pub fn compute_stats(tokens: &[MarkdownToken]) -> MarkdownStats {
    let mut stats = MarkdownStats {
        total_tokens: tokens.len(),
        ..Default::default()
    };
    for token in tokens {
        match token {
            MarkdownToken::Heading(_, _) => stats.headings += 1,
            MarkdownToken::Link(_, _) => stats.links += 1,
            MarkdownToken::CodeBlock(_, _) => stats.code_blocks += 1,
            MarkdownToken::ListItemMd(_) => stats.list_items += 1,
            MarkdownToken::Paragraph => stats.paragraphs += 1,
            _ => {}
        }
    }
    stats
}

// ---------------------------------------------------------------------------
// Table of contents
// ---------------------------------------------------------------------------

/// Generate a Markdown table of contents from heading tokens.
///
/// Each heading is rendered as an indented list item. The indentation is
/// relative to the minimum heading level found in the token stream.
pub fn generate_toc(tokens: &[MarkdownToken]) -> String {
    let headings: Vec<(u8, &str)> = tokens
        .iter()
        .filter_map(|t| {
            if let MarkdownToken::Heading(level, text) = t {
                Some((*level, text.as_str()))
            } else {
                None
            }
        })
        .collect();

    if headings.is_empty() {
        return String::new();
    }

    let min_level = headings.iter().map(|(l, _)| *l).min().unwrap_or(1);
    let mut out = String::new();
    for (level, text) in &headings {
        let indent = "  ".repeat((*level - min_level) as usize);
        let anchor: String = text
            .chars()
            .map(|c| {
                if c.is_alphanumeric() {
                    c.to_ascii_lowercase()
                } else if c == ' ' {
                    '-'
                } else {
                    '-'
                }
            })
            .collect();
        out.push_str(&format!("{}- [{}](#{})\n", indent, text, anchor));
    }
    out
}

// ---------------------------------------------------------------------------
// Whitespace normalisation
// ---------------------------------------------------------------------------

/// Collapse runs of whitespace (spaces, tabs, newlines) into single spaces
/// and trim leading/trailing whitespace.
pub fn normalize_whitespace(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_ws = true; // treat start as whitespace to trim leading
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !prev_ws {
                result.push(' ');
            }
            prev_ws = true;
        } else {
            result.push(ch);
            prev_ws = false;
        }
    }
    if result.ends_with(' ') {
        result.pop();
    }
    result
}

// ---------------------------------------------------------------------------
// Code block extraction
// ---------------------------------------------------------------------------

/// Extract all code blocks from a token slice as `(language, code)` pairs.
pub fn extract_code_blocks(tokens: &[MarkdownToken]) -> Vec<(Option<String>, String)> {
    tokens
        .iter()
        .filter_map(|t| {
            if let MarkdownToken::CodeBlock(code, lang) = t {
                Some((lang.clone(), code.clone()))
            } else {
                None
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Heading outline for document structure
// ---------------------------------------------------------------------------

/// An entry in a document outline derived from headings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineEntry {
    pub level: u8,
    pub text: String,
    pub line_number: usize,
}

/// Build an outline from a multi-line Markdown document by extracting headings
/// and tracking their line numbers.
pub fn build_outline(text: &str) -> Vec<OutlineEntry> {
    let mut entries = Vec::new();
    for (line_idx, line) in text.lines().enumerate() {
        if line.starts_with('#') {
            let level = line.chars().take_while(|&c| c == '#').count().min(6) as u8;
            let content = line[level as usize..].trim().to_string();
            entries.push(OutlineEntry {
                level,
                text: content,
                line_number: line_idx + 1,
            });
        }
    }
    entries
}

// ---------------------------------------------------------------------------
// Link collection with line tracking
// ---------------------------------------------------------------------------

/// A link found in the document together with its location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundLink {
    pub label: String,
    pub url: String,
    pub line_number: usize,
}

/// Collect all inline links from a multi-line Markdown document.
pub fn collect_links(text: &str) -> Vec<FoundLink> {
    let mut links = Vec::new();
    for (line_idx, line) in text.lines().enumerate() {
        let tokens = tokenize_inline(line);
        for token in tokens {
            if let MarkdownToken::Link(label, url) = token {
                links.push(FoundLink {
                    label,
                    url,
                    line_number: line_idx + 1,
                });
            }
        }
    }
    links
}

// ---------------------------------------------------------------------------
// Block-level structure detection
// ---------------------------------------------------------------------------

/// The structural kind of a Markdown line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    Heading(u8),
    ListItem,
    CodeFence,
    BlankLine,
    Paragraph,
}

/// Classify each line of a document into its block-level structure.
pub fn detect_block_structure(text: &str) -> Vec<BlockKind> {
    text.lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                BlockKind::BlankLine
            } else if trimmed.starts_with("```") {
                BlockKind::CodeFence
            } else if line.starts_with('#') {
                let level = line.chars().take_while(|&c| c == '#').count().min(6) as u8;
                BlockKind::Heading(level)
            } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                BlockKind::ListItem
            } else {
                BlockKind::Paragraph
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Front matter
// ---------------------------------------------------------------------------

/// Parsed YAML-like front matter from a markdown document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownFrontMatter {
    pub fields: Vec<(String, String)>,
}

impl MarkdownFrontMatter {
    /// Parse front matter delimited by `---` at the start of a document.
    /// Returns `None` if no front matter is found.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim_start();
        if !text.starts_with("---") {
            return None;
        }
        let after_opening = &text[3..];
        let end = after_opening.find("\n---")?;
        let block = &after_opening[..end];
        let mut fields = Vec::new();
        for line in block.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(colon) = line.find(':') {
                let key = line[..colon].trim().to_string();
                let value = line[colon + 1..].trim().to_string();
                if !key.is_empty() {
                    fields.push((key, value));
                }
            }
        }
        Some(Self { fields })
    }

    /// Get the value for a given key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Number of fields in the front matter.
    pub fn len(&self) -> usize {
        self.fields.len()
    }

    /// Whether the front matter has no fields.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Return only the body text after the front matter block.
    pub fn strip_front_matter(text: &str) -> &str {
        let text = text.trim_start();
        if !text.starts_with("---") {
            return text;
        }
        let after_opening = &text[3..];
        match after_opening.find("\n---") {
            Some(end) => {
                let rest = &after_opening[end + 4..];
                rest.trim_start_matches('\n')
            }
            None => text,
        }
    }
}

impl std::fmt::Display for MarkdownFrontMatter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FrontMatter({} fields)", self.fields.len())
    }
}

// ---------------------------------------------------------------------------
// Table of contents
// ---------------------------------------------------------------------------

/// Generate a table of contents string from a markdown document.
/// Each heading becomes an indented entry. H1 = no indent, H2 = 2 spaces, etc.
pub fn markdown_toc(text: &str) -> String {
    let mut toc = String::new();
    for line in text.lines() {
        if line.starts_with('#') {
            let level = line.chars().take_while(|&c| c == '#').count().min(6);
            let title = line[level..].trim();
            if !title.is_empty() {
                let indent = "  ".repeat(level.saturating_sub(1));
                let slug = title
                    .to_lowercase()
                    .chars()
                    .map(|c| if c.is_alphanumeric() { c } else { '-' })
                    .collect::<String>();
                toc.push_str(&format!("{indent}- [{title}](#{slug})\n"));
            }
        }
    }
    toc
}

// ---------------------------------------------------------------------------
// Document statistics
// ---------------------------------------------------------------------------

/// Statistics about a markdown document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MarkdownDocStats {
    pub words: usize,
    pub lines: usize,
    pub chars: usize,
    pub code_block_lines: usize,
}

impl std::fmt::Display for MarkdownDocStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} words, {} lines, {} chars",
            self.words, self.lines, self.chars
        )
    }
}

/// Count words in a markdown document, excluding front matter and code blocks.
pub fn markdown_word_count(text: &str) -> MarkdownDocStats {
    let body = MarkdownFrontMatter::strip_front_matter(text);
    let mut total_words = 0usize;
    let mut total_lines = 0usize;
    let mut total_chars = 0usize;
    let mut in_code_block = false;
    let mut code_block_lines = 0usize;

    for line in body.lines() {
        total_lines += 1;
        total_chars += line.len();
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            code_block_lines += 1;
            continue;
        }
        total_words += line.split_whitespace().count();
    }

    MarkdownDocStats {
        words: total_words,
        lines: total_lines,
        chars: total_chars,
        code_block_lines,
    }
}


// ---------------------------------------------------------------------------
// MarkdownToken helpers
// ---------------------------------------------------------------------------

impl MarkdownToken {
    /// Returns the inner text content of this token.
    pub fn text_content(&self) -> &str {
        match self {
            MarkdownToken::Text(s) => s,
            MarkdownToken::Bold(s) => s,
            MarkdownToken::Italic(s) => s,
            MarkdownToken::Code(s) => s,
            MarkdownToken::CodeBlock(s, _) => s,
            MarkdownToken::Link(text, _) => text,
            MarkdownToken::Heading(_, text) => text,
            MarkdownToken::ListItemMd(s) => s,
            MarkdownToken::Paragraph => "",
            MarkdownToken::LineBreak => "",
        }
    }

    /// Returns true if this is a block-level token.
    pub fn is_block(&self) -> bool {
        matches!(
            self,
            MarkdownToken::Heading(_, _)
                | MarkdownToken::CodeBlock(_, _)
                | MarkdownToken::ListItemMd(_)
                | MarkdownToken::Paragraph
        )
    }

    /// Returns true if this is an inline formatting token.
    pub fn is_inline(&self) -> bool {
        matches!(
            self,
            MarkdownToken::Text(_)
                | MarkdownToken::Bold(_)
                | MarkdownToken::Italic(_)
                | MarkdownToken::Code(_)
                | MarkdownToken::Link(_, _)
        )
    }

    /// Returns the token kind as a static string.
    pub fn kind_name(&self) -> &'static str {
        match self {
            MarkdownToken::Text(_) => "text",
            MarkdownToken::Bold(_) => "bold",
            MarkdownToken::Italic(_) => "italic",
            MarkdownToken::Code(_) => "code",
            MarkdownToken::CodeBlock(_, _) => "code_block",
            MarkdownToken::Link(_, _) => "link",
            MarkdownToken::Heading(_, _) => "heading",
            MarkdownToken::ListItemMd(_) => "list_item",
            MarkdownToken::Paragraph => "paragraph",
            MarkdownToken::LineBreak => "line_break",
        }
    }
}

impl fmt::Display for MarkdownToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MarkdownToken::Text(s) => write!(f, "{s}"),
            MarkdownToken::Bold(s) => write!(f, "**{s}**"),
            MarkdownToken::Italic(s) => write!(f, "*{s}*"),
            MarkdownToken::Code(s) => write!(f, "`{s}`"),
            MarkdownToken::CodeBlock(s, lang) => {
                let l = lang.as_deref().unwrap_or("");
                write!(f, "```{l}\n{s}\n```")
            }
            MarkdownToken::Link(text, url) => write!(f, "[{text}]({url})"),
            MarkdownToken::Heading(level, text) => {
                let hashes = "#".repeat(*level as usize);
                write!(f, "{hashes} {text}")
            }
            MarkdownToken::ListItemMd(s) => write!(f, "- {s}"),
            MarkdownToken::Paragraph => write!(f, ""),
            MarkdownToken::LineBreak => writeln!(f),
        }
    }
}

// ---------------------------------------------------------------------------
// Token counting
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::fmt;

/// Count tokens by kind.
pub fn count_tokens_by_kind(tokens: &[MarkdownToken]) -> std::collections::HashMap<&'static str, usize> {
    let mut counts = std::collections::HashMap::new();
    for token in tokens {
        *counts.entry(token.kind_name()).or_insert(0) += 1;
    }
    counts
}

/// Total character count across all tokens.
pub fn total_text_length(tokens: &[MarkdownToken]) -> usize {
    tokens.iter().map(|t| t.text_content().len()).sum()
}

/// Extract all URLs from link tokens.
pub fn extract_urls(tokens: &[MarkdownToken]) -> Vec<String> {
    tokens.iter()
        .filter_map(|t| match t {
            MarkdownToken::Link(_, url) => Some(url.clone()),
            _ => None,
        })
        .collect()
}

/// Extract all code snippets (inline and block).
pub fn extract_all_code(tokens: &[MarkdownToken]) -> Vec<String> {
    tokens.iter()
        .filter_map(|t| match t {
            MarkdownToken::Code(s) | MarkdownToken::CodeBlock(s, _) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

/// Estimates reading time in minutes (assuming 200 words/min).
pub fn estimated_reading_time(text: &str) -> f64 {
    let word_count = text.split_whitespace().count();
    word_count as f64 / 200.0
}

// ---------------------------------------------------------------------------
// MarkdownDocument
// ---------------------------------------------------------------------------

/// A parsed Markdown document wrapping a token list.
#[derive(Debug, Clone)]
pub struct MarkdownDocument {
    pub tokens: Vec<MarkdownToken>,
}

impl MarkdownDocument {
    pub fn parse(text: &str) -> Self {
        Self { tokens: tokenize_block(text) }
    }

    pub fn from_tokens(tokens: Vec<MarkdownToken>) -> Self {
        Self { tokens }
    }

    /// Returns all heading tokens.
    pub fn headings(&self) -> Vec<(u8, &str)> {
        self.tokens.iter().filter_map(|t| match t {
            MarkdownToken::Heading(level, text) => Some((*level, text.as_str())),
            _ => None,
        }).collect()
    }

    /// Returns all links as (text, url) pairs.
    pub fn links(&self) -> Vec<(&str, &str)> {
        self.tokens.iter().filter_map(|t| match t {
            MarkdownToken::Link(text, url) => Some((text.as_str(), url.as_str())),
            _ => None,
        }).collect()
    }

    /// Returns all code blocks.
    pub fn code_blocks(&self) -> Vec<&str> {
        self.tokens.iter().filter_map(|t| match t {
            MarkdownToken::CodeBlock(code, _) => Some(code.as_str()),
            _ => None,
        }).collect()
    }

    /// Counts words across all text-like tokens.
    pub fn word_count(&self) -> usize {
        self.tokens.iter().map(|t| match t {
            MarkdownToken::Text(s) | MarkdownToken::Bold(s) | MarkdownToken::Italic(s)
            | MarkdownToken::Heading(_, s) | MarkdownToken::ListItemMd(s) => {
                s.split_whitespace().count()
            }
            _ => 0,
        }).sum()
    }

    /// Returns a plain-text summary truncated to `max_len` characters.
    pub fn summary(&self, max_len: usize) -> String {
        let full = render_to_plain_text(&self.tokens);
        if full.len() <= max_len {
            full
        } else {
            let mut s = full[..max_len].to_string();
            s.push('…');
            s
        }
    }

    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }
}

impl std::fmt::Display for MarkdownDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MarkdownDocument({} tokens, {} words)", self.token_count(), self.word_count())
    }
}

// ---------------------------------------------------------------------------
// TableOfContents
// ---------------------------------------------------------------------------

/// An entry in the table of contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocEntry {
    pub level: u8,
    pub title: String,
}

impl std::fmt::Display for TocEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let indent = "  ".repeat((self.level.saturating_sub(1)) as usize);
        write!(f, "{indent}- {}", self.title)
    }
}

/// Table of contents built from document headings.
#[derive(Debug, Clone)]
pub struct TableOfContents {
    pub entries: Vec<TocEntry>,
}

impl TableOfContents {
    /// Build a ToC from a list of tokens.
    pub fn from_tokens(tokens: &[MarkdownToken]) -> Self {
        let entries = tokens.iter().filter_map(|t| match t {
            MarkdownToken::Heading(level, text) => Some(TocEntry {
                level: *level,
                title: text.clone(),
            }),
            _ => None,
        }).collect();
        Self { entries }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn max_depth(&self) -> u8 {
        self.entries.iter().map(|e| e.level).max().unwrap_or(0)
    }
}

impl std::fmt::Display for TableOfContents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for entry in &self.entries {
            writeln!(f, "{entry}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Extended Markdown methods
// ---------------------------------------------------------------------------

/// Escape special Markdown characters in a plain-text string.
///
/// Characters escaped: `\`, `` ` ``, `*`, `_`, `{`, `}`, `[`, `]`, `(`, `)`,
/// `#`, `+`, `-`, `.`, `!`, `|`.
pub fn escape_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if "\\`*_{}[]()#+-.!|".contains(ch) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Remove backslash escapes from a Markdown string.
///
/// A backslash followed by a special character is replaced with just the
/// character.
pub fn unescape_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        if chars[i] == '\\' && i + 1 < len && "\\`*_{}[]()#+-.!|".contains(chars[i + 1]) {
            out.push(chars[i + 1]);
            i += 2;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

impl MarkdownConfig {
    /// Create a config with hard line breaks enabled.
    pub fn with_hard_breaks() -> Self {
        Self {
            hard_line_breaks: true,
            smart_quotes: false,
        }
    }

    /// Create a config with smart quotes enabled.
    pub fn with_smart_quotes() -> Self {
        Self {
            hard_line_breaks: false,
            smart_quotes: true,
        }
    }

    /// Return a config with both features enabled.
    pub fn all_features() -> Self {
        Self {
            hard_line_breaks: true,
            smart_quotes: true,
        }
    }

    /// Returns `true` if no features are enabled.
    pub fn is_default(&self) -> bool {
        !self.hard_line_breaks && !self.smart_quotes
    }
}

impl MarkdownTable {
    /// Add a data row. Returns `Err` if the row length doesn't match the
    /// header count.
    pub fn add_row(&mut self, row: Vec<String>) -> Result<(), String> {
        if row.len() != self.headers.len() {
            return Err(format!(
                "expected {} columns, got {}",
                self.headers.len(),
                row.len()
            ));
        }
        self.rows.push(row);
        Ok(())
    }

    /// Remove the data row at `index`. Returns the removed row, or `None` if
    /// out of bounds.
    pub fn remove_row(&mut self, index: usize) -> Option<Vec<String>> {
        if index < self.rows.len() {
            Some(self.rows.remove(index))
        } else {
            None
        }
    }

    /// Render the table with columns padded to uniform widths.
    pub fn render_aligned(&self) -> String {
        let col_count = self.headers.len();
        let mut widths = vec![0usize; col_count];
        for (i, h) in self.headers.iter().enumerate() {
            widths[i] = widths[i].max(h.len());
        }
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < col_count {
                    widths[i] = widths[i].max(cell.len());
                }
            }
        }
        let mut out = String::new();
        // Header
        out.push_str("| ");
        for (i, h) in self.headers.iter().enumerate() {
            out.push_str(&format!("{:<width$}", h, width = widths[i]));
            if i + 1 < col_count {
                out.push_str(" | ");
            }
        }
        out.push_str(" |\n| ");
        // Separator
        for (i, &w) in widths.iter().enumerate() {
            out.push_str(&"-".repeat(w));
            if i + 1 < col_count {
                out.push_str(" | ");
            }
        }
        out.push_str(" |\n");
        // Data rows
        for row in &self.rows {
            out.push_str("| ");
            for (i, cell) in row.iter().enumerate() {
                let w = if i < col_count { widths[i] } else { cell.len() };
                out.push_str(&format!("{:<width$}", cell, width = w));
                if i + 1 < col_count {
                    out.push_str(" | ");
                }
            }
            out.push_str(" |\n");
        }
        out
    }
}

impl MarkdownStats {
    /// Total non-text tokens (headings + links + code_blocks + list_items).
    pub fn structural_count(&self) -> usize {
        self.headings + self.links + self.code_blocks + self.list_items
    }

    /// Returns `true` if there are no tokens at all.
    pub fn is_empty(&self) -> bool {
        self.total_tokens == 0
    }
}

// ---------------------------------------------------------------------------
// MarkdownTableRenderer – render tables for terminal
// ---------------------------------------------------------------------------

/// Renders markdown tables as formatted terminal output.
pub struct MarkdownTableRenderer {
    /// Minimum column width.
    pub min_col_width: usize,
    /// Maximum column width.
    pub max_col_width: usize,
    /// Column separator character.
    pub separator: char,
}

impl MarkdownTableRenderer {
    /// Create a table renderer with defaults.
    pub fn new() -> Self {
        Self { min_col_width: 3, max_col_width: 40, separator: '|' }
    }

    /// Render a table given headers and rows.
    pub fn render(&self, headers: &[&str], rows: &[Vec<String>]) -> String {
        let col_count = headers.len();
        let mut widths: Vec<usize> = headers.iter().map(|h| h.len().max(self.min_col_width).min(self.max_col_width)).collect();
        for row in rows {
            for (i, cell) in row.iter().enumerate() {
                if i < col_count {
                    widths[i] = widths[i].max(cell.len().min(self.max_col_width));
                }
            }
        }

        let mut out = String::new();
        // Header
        out.push(self.separator);
        for (i, h) in headers.iter().enumerate() {
            out.push_str(&format!(" {:width$} {}", h, self.separator, width = widths[i]));
        }
        out.push('\n');
        // Separator line
        out.push(self.separator);
        for w in &widths {
            out.push_str(&format!("{}{}", "-".repeat(*w + 2), self.separator));
        }
        out.push('\n');
        // Rows
        for row in rows {
            out.push(self.separator);
            for (i, cell) in row.iter().enumerate() {
                let w = widths.get(i).copied().unwrap_or(self.min_col_width);
                let truncated = if cell.len() > w { &cell[..w] } else { cell.as_str() };
                out.push_str(&format!(" {:width$} {}", truncated, self.separator, width = w));
            }
            out.push('\n');
        }
        out
    }

    /// Column count from a markdown table text.
    pub fn column_count_from_text(text: &str) -> usize {
        text.lines().next()
            .map(|line| line.split('|').filter(|s| !s.trim().is_empty()).count())
            .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// MarkdownChecklistRenderer – render checklists with toggle state
// ---------------------------------------------------------------------------

/// A single checklist item.
#[derive(Debug, Clone)]
pub struct ChecklistItem {
    pub text: String,
    pub checked: bool,
}

/// Parses and manages markdown checklists.
pub struct MarkdownChecklistRenderer {
    items: Vec<ChecklistItem>,
}

impl MarkdownChecklistRenderer {
    /// Create an empty checklist.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Parse checklist items from markdown text.
    pub fn parse(text: &str) -> Self {
        let items = text.lines().filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("- [x] ") || trimmed.starts_with("- [X] ") {
                Some(ChecklistItem { text: trimmed[6..].to_string(), checked: true })
            } else if trimmed.starts_with("- [ ] ") {
                Some(ChecklistItem { text: trimmed[6..].to_string(), checked: false })
            } else {
                None
            }
        }).collect();
        Self { items }
    }

    /// Toggle an item by index.
    pub fn toggle(&mut self, index: usize) -> Option<bool> {
        if let Some(item) = self.items.get_mut(index) {
            item.checked = !item.checked;
            Some(item.checked)
        } else {
            None
        }
    }

    /// Number of items.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Number of checked items.
    pub fn checked_count(&self) -> usize {
        self.items.iter().filter(|i| i.checked).count()
    }

    /// Progress as a fraction (0.0 to 1.0).
    pub fn progress(&self) -> f64 {
        if self.items.is_empty() { return 0.0; }
        self.checked_count() as f64 / self.items.len() as f64
    }

    /// Render the checklist back to markdown.
    pub fn to_markdown(&self) -> String {
        self.items.iter().map(|i| {
            let mark = if i.checked { "x" } else { " " };
            format!("- [{}] {}", mark, i.text)
        }).collect::<Vec<_>>().join("\n")
    }
}

// ---------------------------------------------------------------------------
// MarkdownLinkResolver – resolve relative paths
// ---------------------------------------------------------------------------

/// Resolves relative links in markdown to absolute paths.
pub struct MarkdownLinkResolver {
    base_dir: String,
}

impl MarkdownLinkResolver {
    /// Create a resolver with a base directory.
    pub fn new(base_dir: impl Into<String>) -> Self {
        Self { base_dir: base_dir.into() }
    }

    /// Resolve a relative path to absolute.
    pub fn resolve(&self, relative_path: &str) -> String {
        if relative_path.starts_with("http://") || relative_path.starts_with("https://") || relative_path.starts_with('/') {
            return relative_path.to_string();
        }
        let base = self.base_dir.trim_end_matches('/');
        format!("{}/{}", base, relative_path)
    }

    /// Resolve all links in a list of tokens.
    pub fn resolve_all_links(&self, tokens: &[MarkdownToken]) -> Vec<MarkdownToken> {
        tokens.iter().map(|t| match t {
            MarkdownToken::Link(text, url) => {
                MarkdownToken::Link(text.clone(), self.resolve(url))
            }
            other => other.clone(),
        }).collect()
    }

    /// Get the base directory.
    pub fn base_dir(&self) -> &str {
        &self.base_dir
    }
}

// ---------------------------------------------------------------------------
// Heading extraction for outline
// ---------------------------------------------------------------------------

/// Extract headings from markdown text for building an outline/TOC.
pub fn extract_heading_outline(text: &str) -> Vec<(u8, String, usize)> {
    let mut headings = Vec::new();
    for (line_num, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|&c| c == '#').count() as u8;
            if level <= 6 {
                let title = trimmed[level as usize..].trim().to_string();
                if !title.is_empty() {
                    headings.push((level, title, line_num + 1));
                }
            }
        }
    }
    headings
}

/// Build a hierarchical indent string for outline display.
pub fn format_outline(headings: &[(u8, String, usize)]) -> String {
    headings.iter().map(|(level, title, line)| {
        let indent = "  ".repeat((*level as usize).saturating_sub(1));
        format!("{}{}  (line {})", indent, title, line)
    }).collect::<Vec<_>>().join("\n")
}


// ---------------------------------------------------------------------------
// Footnote renderer
// ---------------------------------------------------------------------------

/// Represents a footnote reference found in Markdown text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootnoteRef {
    /// The label used in the text, e.g. `[^1]`.
    pub label: String,
    /// Zero-based index into the footnote list.
    pub index: usize,
}

/// Collects and renders Markdown footnotes.
///
/// Footnotes are stored as `[^label]: content` and referenced inline with
/// `[^label]`.  The renderer resolves references to numeric indices and
/// produces a rendered footnote section.
#[derive(Debug, Clone)]
pub struct MarkdownFootnoteRenderer {
    definitions: Vec<(String, String)>,
}

impl MarkdownFootnoteRenderer {
    /// Create a new empty footnote renderer.
    pub fn new() -> Self {
        Self { definitions: Vec::new() }
    }

    /// Register a footnote definition.
    pub fn add_definition(&mut self, label: impl Into<String>, content: impl Into<String>) {
        self.definitions.push((label.into(), content.into()));
    }

    /// Look up a footnote by label and return its index.
    pub fn resolve(&self, label: &str) -> Option<FootnoteRef> {
        self.definitions.iter().enumerate().find_map(|(i, (l, _))| {
            if l == label {
                Some(FootnoteRef { label: label.to_string(), index: i })
            } else {
                None
            }
        })
    }

    /// Render the footnote section as plain text.
    pub fn render_section(&self) -> String {
        if self.definitions.is_empty() {
            return String::new();
        }
        let mut out = String::from("---\n");
        for (i, (label, content)) in self.definitions.iter().enumerate() {
            out.push_str(&format!("[^{}] ({}): {}\n", i + 1, label, content));
        }
        out
    }

    /// Return the total number of registered footnotes.
    pub fn count(&self) -> usize {
        self.definitions.len()
    }

    /// Return all labels in definition order.
    pub fn labels(&self) -> Vec<&str> {
        self.definitions.iter().map(|(l, _)| l.as_str()).collect()
    }

    /// Parse footnote definitions from a block of Markdown text.
    ///
    /// Lines matching `[^label]: content` are extracted.
    pub fn parse_definitions(text: &str) -> Vec<(String, String)> {
        let mut defs = Vec::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("[^") {
                if let Some(close) = trimmed.find("]:") {
                    let label = trimmed[2..close].to_string();
                    let content = trimmed[close + 2..].trim().to_string();
                    defs.push((label, content));
                }
            }
        }
        defs
    }
}

// ---------------------------------------------------------------------------
// Task-list toggling
// ---------------------------------------------------------------------------

/// Represents the state of a Markdown task-list item (`- [ ]` / `- [x]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Incomplete,
    Complete,
}

/// Locates and toggles Markdown task-list checkboxes.
#[derive(Debug, Clone)]
pub struct MarkdownTaskListToggle {
    items: Vec<(usize, TaskState, String)>,
}

impl MarkdownTaskListToggle {
    /// Parse task-list items from Markdown source.
    pub fn parse(source: &str) -> Self {
        let mut items = Vec::new();
        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("- [ ] ") {
                let text = trimmed[6..].to_string();
                items.push((line_num, TaskState::Incomplete, text));
            } else if trimmed.starts_with("- [x] ") || trimmed.starts_with("- [X] ") {
                let text = trimmed[6..].to_string();
                items.push((line_num, TaskState::Complete, text));
            }
        }
        Self { items }
    }

    /// Return a slice of all parsed items.
    pub fn items(&self) -> &[(usize, TaskState, String)] {
        &self.items
    }

    /// Count of incomplete tasks.
    pub fn incomplete_count(&self) -> usize {
        self.items.iter().filter(|(_, s, _)| *s == TaskState::Incomplete).count()
    }

    /// Count of completed tasks.
    pub fn complete_count(&self) -> usize {
        self.items.iter().filter(|(_, s, _)| *s == TaskState::Complete).count()
    }

    /// Toggle the state of the task at the given index, returning the new
    /// source text.  If the index is out of range the original text is returned
    /// unchanged.
    pub fn toggle(&self, source: &str, item_index: usize) -> String {
        if item_index >= self.items.len() {
            return source.to_string();
        }
        let (target_line, state, _) = &self.items[item_index];
        let mut result_lines: Vec<String> = Vec::new();
        for (i, line) in source.lines().enumerate() {
            if i == *target_line {
                let new_line = match state {
                    TaskState::Incomplete => line.replace("- [ ] ", "- [x] "),
                    TaskState::Complete => {
                        line.replace("- [x] ", "- [ ] ").replace("- [X] ", "- [ ] ")
                    }
                };
                result_lines.push(new_line);
            } else {
                result_lines.push(line.to_string());
            }
        }
        result_lines.join("\n")
    }

    /// Progress as a percentage (0–100).
    pub fn progress_percent(&self) -> u8 {
        if self.items.is_empty() {
            return 0;
        }
        let done = self.complete_count();
        ((done * 100) / self.items.len()) as u8
    }
}

// ---------------------------------------------------------------------------
// Anchor generation
// ---------------------------------------------------------------------------

/// Generate a GitHub-compatible anchor slug from heading text.
///
/// Rules: lowercase, strip non-alphanumeric characters except hyphens and
/// spaces, collapse whitespace to hyphens.
pub fn generate_anchor(heading: &str) -> String {
    let mut slug = String::new();
    for ch in heading.chars() {
        if ch.is_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if ch == ' ' || ch == '-' {
            if !slug.ends_with('-') {
                slug.push('-');
            }
        }
    }
    slug.trim_matches('-').to_string()
}

/// Generate anchors for every heading in a list and return `(level, title, anchor)`.
pub fn generate_heading_anchors(headings: &[(u8, String, usize)]) -> Vec<(u8, String, String)> {
    headings.iter().map(|(level, title, _line)| {
        (*level, title.clone(), generate_anchor(title))
    }).collect()
}

/// Build a Markdown table-of-contents from headings with anchor links.
pub fn build_toc(headings: &[(u8, String, usize)]) -> String {
    let anchored = generate_heading_anchors(headings);
    let mut toc = String::new();
    for (level, title, anchor) in &anchored {
        let indent = "  ".repeat((*level as usize).saturating_sub(1));
        toc.push_str(&format!("{}- [{}](#{})\n", indent, title, anchor));
    }
    toc
}

// ---------------------------------------------------------------------------
// Code-fence language detection
// ---------------------------------------------------------------------------

/// Detect the language identifier from a fenced code block opening line.
///
/// Given a line like `` ```rust `` returns `Some("rust")`.
pub fn detect_code_fence_language(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with("```") {
        return None;
    }
    let after = trimmed[3..].trim();
    if after.is_empty() || after == "```" {
        return None;
    }
    // Take the first word (no spaces allowed in the info string's language).
    let lang = after.split_whitespace().next()?;
    Some(lang.to_string())
}

/// Map a code-fence language tag to a display-friendly name.
pub fn language_display_name(tag: &str) -> &str {
    match tag {
        "rs" | "rust" => "Rust",
        "py" | "python" | "python3" => "Python",
        "js" | "javascript" => "JavaScript",
        "ts" | "typescript" => "TypeScript",
        "rb" | "ruby" => "Ruby",
        "go" | "golang" => "Go",
        "java" => "Java",
        "c" => "C",
        "cpp" | "c++" | "cxx" => "C++",
        "cs" | "csharp" => "C#",
        "sh" | "bash" | "shell" => "Shell",
        "json" => "JSON",
        "yaml" | "yml" => "YAML",
        "toml" => "TOML",
        "xml" => "XML",
        "html" => "HTML",
        "css" => "CSS",
        "sql" => "SQL",
        "md" | "markdown" => "Markdown",
        other => other,
    }
}

/// Extract all fenced code blocks from raw Markdown source text, returning
/// `(language, code_content)` pairs.
pub fn extract_fenced_code_blocks(source: &str) -> Vec<(Option<String>, String)> {
    let mut blocks = Vec::new();
    let mut in_block = false;
    let mut current_lang: Option<String> = None;
    let mut current_code = String::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if !in_block && trimmed.starts_with("```") {
            in_block = true;
            current_lang = detect_code_fence_language(line);
            current_code.clear();
        } else if in_block && trimmed == "```" {
            blocks.push((current_lang.take(), current_code.clone()));
            current_code.clear();
            in_block = false;
        } else if in_block {
            if !current_code.is_empty() {
                current_code.push('\n');
            }
            current_code.push_str(line);
        }
    }
    blocks
}

// --- MarkdownTableFormatterV2 ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnAlignment {
    Left,
    Center,
    Right,
}

pub struct MarkdownTableFormatterV2 {
    rows: Vec<Vec<String>>,
    alignments: Vec<ColumnAlignment>,
}

impl MarkdownTableFormatterV2 {
    pub fn new() -> Self { Self { rows: Vec::new(), alignments: Vec::new() } }

    pub fn parse_rows(text: &str) -> Vec<Vec<String>> {
        text.lines()
            .filter(|l| l.contains('|') && !l.trim().starts_with("|---") && !l.trim().starts_with("| ---"))
            .map(|line| {
                line.split('|')
                    .map(|c| c.trim().to_string())
                    .filter(|c| !c.is_empty())
                    .collect()
            })
            .collect()
    }

    pub fn set_rows(&mut self, rows: Vec<Vec<String>>) { self.rows = rows; }

    pub fn set_alignments(&mut self, aligns: Vec<ColumnAlignment>) { self.alignments = aligns; }

    pub fn compute_column_widths(&self) -> Vec<usize> {
        let cols = self.rows.iter().map(|r| r.len()).max().unwrap_or(0);
        (0..cols).map(|c| {
            self.rows.iter().map(|r| r.get(c).map(|s| s.len()).unwrap_or(0)).max().unwrap_or(0)
        }).collect()
    }

    pub fn format_table(&self) -> String {
        let widths = self.compute_column_widths();
        let mut out = String::new();
        for (i, row) in self.rows.iter().enumerate() {
            out.push('|');
            for (c, cell) in row.iter().enumerate() {
                let w = widths.get(c).copied().unwrap_or(cell.len());
                out.push_str(&format!(" {:width$} |", cell, width = w));
            }
            out.push('\n');
            if i == 0 {
                out.push('|');
                for &w in &widths {
                    out.push_str(&format!(" {} |", "-".repeat(w)));
                }
                out.push('\n');
            }
        }
        out
    }

    pub fn validate_table(&self) -> bool {
        if self.rows.is_empty() { return false; }
        let expected = self.rows[0].len();
        self.rows.iter().all(|r| r.len() == expected)
    }
}

// --- MarkdownLinkExtractorV2 ---

#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedLink {
    pub label: String,
    pub url: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkCategory {
    Http,
    Relative,
    Anchor,
}

pub struct MarkdownLinkExtractorV2;

impl MarkdownLinkExtractorV2 {
    pub fn extract_links(text: &str) -> Vec<ExtractedLink> {
        let mut links = Vec::new();
        let mut chars = text.chars().peekable();
        while let Some(&ch) = chars.peek() {
            if ch == '[' {
                chars.next();
                let label: String = chars.by_ref().take_while(|&c| c != ']').collect();
                if chars.peek() == Some(&'(') {
                    chars.next();
                    let url: String = chars.by_ref().take_while(|&c| c != ')').collect();
                    links.push(ExtractedLink { label, url, title: None });
                }
            } else {
                chars.next();
            }
        }
        links
    }

    pub fn categorize(url: &str) -> LinkCategory {
        if url.starts_with('#') { LinkCategory::Anchor }
        else if url.starts_with("http://") || url.starts_with("https://") { LinkCategory::Http }
        else { LinkCategory::Relative }
    }

    pub fn link_count(text: &str) -> usize { Self::extract_links(text).len() }

    pub fn unique_urls(text: &str) -> Vec<String> {
        let mut urls: Vec<String> = Self::extract_links(text).into_iter().map(|l| l.url).collect();
        urls.sort();
        urls.dedup();
        urls
    }
}

// --- MarkdownTocGeneratorV2 ---

#[derive(Debug, Clone)]
pub struct TocHeading {
    pub level: usize,
    pub text: String,
    pub anchor: String,
}

pub struct MarkdownTocGeneratorV2;

impl MarkdownTocGeneratorV2 {
    pub fn extract_headings(text: &str) -> Vec<TocHeading> {
        text.lines().filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                let level = trimmed.chars().take_while(|&c| c == '#').count();
                let text = trimmed[level..].trim().to_string();
                let anchor = text.to_lowercase().replace(' ', "-");
                Some(TocHeading { level, text, anchor })
            } else {
                None
            }
        }).collect()
    }

    pub fn toc_as_markdown(headings: &[TocHeading], max_depth: usize, numbered: bool) -> String {
        let mut out = String::new();
        let mut counter = 0usize;
        for h in headings {
            if h.level > max_depth { continue; }
            let indent = "  ".repeat(h.level.saturating_sub(1));
            counter += 1;
            if numbered {
                out.push_str(&format!("{}{}. [{}](#{})\n", indent, counter, h.text, h.anchor));
            } else {
                out.push_str(&format!("{}- [{}](#{})\n", indent, h.text, h.anchor));
            }
        }
        out
    }
}


/// Configuration manager for markdown functionality.
pub struct MarkdownConfigDetail {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl MarkdownConfigDetail {
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

    pub fn merge(&mut self, other: &MarkdownConfigDetail) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for markdown operations.
pub struct MarkdownRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl MarkdownRateTracker {
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

/// Validation result collector for markdown.
pub struct MarkdownValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl MarkdownValidator {
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

    pub fn merge(&mut self, other: &MarkdownValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Markdown parsing and rendering — extended utilities (xg)
// ---------------------------------------------------------------------------

/// Metric accumulator for markdown operations.
#[derive(Debug, Clone)]
pub struct XgMetrics {
    samples: Vec<f64>,
    label: String,
}

impl XgMetrics {
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

/// Sliding-window rate counter for markdown.
#[derive(Debug, Clone)]
pub struct XgRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl XgRateWindow {
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

/// A small LRU-style cache for markdown lookups.
#[derive(Debug, Clone)]
pub struct XgLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl XgLruCache {
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
// xb_ utilities – batch 23
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer23 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer23 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_23(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_23<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_23<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_23(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_23(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 119
// ---------------------------------------------------------------------------

/// Generic object pool `Xc119Pool<T>`.
pub struct Xc119Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc119Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc119PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc119Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc119PoolStats {
        Xc119PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc119Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc119Scheduler`.
pub struct Xc119Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc119Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc119Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_119 hash for the given byte slice.
pub fn xc_119_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_119 convention.
pub fn xc_119_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe35 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe35Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe35PipelineError {
    pub stage: Xe35Stage,
    pub message: String,
}

impl std::fmt::Display for Xe35PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe35Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe35Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe35PipelineError>>>,
    stage_names: Vec<Xe35Stage>,
}

impl Xe35Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe35PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe35Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe35PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe35Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe35PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe35Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe35PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe35Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe35PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe35Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe35CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe35CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe35Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe35CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe35CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe35Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe35CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_35_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe35CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_35_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe35CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_35_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe35PipelineError> {
    Ok(data)
}

pub fn xe_35_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe35PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_35_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe35PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_35_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe35PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_35_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe35PipelineError> {
    Err(Xe35PipelineError {
        stage: Xe35Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #121
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf121Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf121TrieNode {
    children: std::collections::HashMap<char, Xf121TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf121Trie {
    root: Xf121TrieNode,
    count: usize,
}

impl Xf121Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf121TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf121TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf121TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf121BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf121BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 118).
pub struct Xh118SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh118SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 160 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 118).
pub struct Xh118BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh118BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 118).
pub struct Xi118Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi118Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi118Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi118Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 118).
pub struct Xi118IntervalTree {
    xi_intervals: Vec<Xi118Interval>,
}

impl Xi118IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi118Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi118Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi118Interval) -> Vec<&Xi118Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi118Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi118Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi118Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi118Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi118Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi118Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 118) ---

/// Disjoint set / union-find for crate 118.
pub struct Xj118UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj118UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ118_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 118.
pub struct Xj118BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj118BTreeNode<K, V>>>,
    len: usize,
}

struct Xj118BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj118BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj118BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ118_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ118_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj118BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj118BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj118BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj118BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_118 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk118SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk118SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk118DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk118DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_118).
#[derive(Debug, Clone)]
pub struct Xl118Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl118Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_118).
#[derive(Debug, Clone)]
pub struct Xl118SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl118SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm118MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm118MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm118Tokenizer {
    text: String,
}

impl Xm118Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 118.
pub struct Xn118Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn118Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 118 -----

#[derive(Debug, Clone)]
struct Xn118AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn118AvlNode<K, V>>>,
    right: Option<Box<Xn118AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 118.
#[derive(Debug, Clone)]
pub struct Xn118AVL<K, V> {
    root: Option<Box<Xn118AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn118AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn118AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn118AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn118AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn118AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn118AvlNode<K, V>>) -> Box<Xn118AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn118AvlNode<K, V>>) -> Box<Xn118AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn118AvlNode<K, V>>) -> Box<Xn118AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn118AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn118AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn118AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn118AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn118AvlNode<K, V>>) -> &Xn118AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn118AvlNode<K, V>>) -> (Box<Xn118AvlNode<K, V>>, Option<Box<Xn118AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn118AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn118AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn118AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn118AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn118AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn118AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn118AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo118RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo118Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo118RBNode<K, V> {
    key: K,
    value: V,
    color: Xo118Color,
    left: Option<Box<Xo118RBNode<K, V>>>,
    right: Option<Box<Xo118RBNode<K, V>>>,
}

/// A red-black tree map for crate 118.
#[derive(Debug, Clone)]
pub struct Xo118RedBlack<K, V> {
    root: Option<Box<Xo118RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo118RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo118Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo118RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo118RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo118RBNode {
                    key, value, color: Xo118Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo118RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo118Color::Red)
    }

    fn xo_balance(mut h: Box<Xo118RBNode<K, V>>) -> Box<Xo118RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo118Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo118RBNode<K, V>>) -> Box<Xo118RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo118Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo118RBNode<K, V>>) -> Box<Xo118RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo118Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo118RBNode<K, V>>) {
        h.color = Xo118Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo118Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo118Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo118Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo118RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo118RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo118RBNode<K, V>) -> (K, V, Option<Box<Xo118RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo118RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo118Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo118RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo118ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 118.
#[derive(Debug, Clone)]
pub struct Xo118ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo118ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo118#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo118#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 118).
#[derive(Debug)]
pub struct Xp118SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp118Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp118Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp118Node<K, V>>>,
    xp_right: Option<Box<Xp118Node<K, V>>>,
}

impl<K: Ord, V> Xp118Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp118SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp118SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp118Node<K, V>>>, key: &K) -> Option<Box<Xp118Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp118Node<K, V>>) -> Box<Xp118Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp118Node<K, V>>) -> Box<Xp118Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp118Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp118Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp118Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq118Treap ---------------

use std::cmp::Ordering as Xq118Ord;

struct Xq118TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq118TreapNode<K, V>>>,
    right: Option<Box<Xq118TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq118Treap<K, V> {
    root: Option<Box<Xq118TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq118TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_118_size<K, V>(node: &Option<Box<Xq118TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_118_update_size<K, V>(node: &mut Xq118TreapNode<K, V>) {
    node.size = 1 + xq_118_size(&node.left) + xq_118_size(&node.right);
}

fn xq_118_rotate_right<K, V>(mut node: Box<Xq118TreapNode<K, V>>) -> Box<Xq118TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_118_update_size(&mut node);
    left.right = Some(node);
    xq_118_update_size(&mut left);
    left
}

fn xq_118_rotate_left<K, V>(mut node: Box<Xq118TreapNode<K, V>>) -> Box<Xq118TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_118_update_size(&mut node);
    right.left = Some(node);
    xq_118_update_size(&mut right);
    right
}

fn xq_118_insert_node<K: Ord, V>(
    node: Option<Box<Xq118TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq118TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq118TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq118Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq118Ord::Less => {
                let (new_left, old) = xq_118_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_118_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_118_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq118Ord::Greater => {
                let (new_right, old) = xq_118_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_118_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_118_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_118_remove_node<K: Ord, V>(
    node: Option<Box<Xq118TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq118TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq118Ord::Less => {
                let (new_left, old) = xq_118_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_118_update_size(&mut n);
                (Some(n), old)
            }
            Xq118Ord::Greater => {
                let (new_right, old) = xq_118_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_118_update_size(&mut n);
                (Some(n), old)
            }
            Xq118Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_118_rotate_right(n);
                    let (new_right, old) = xq_118_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_118_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_118_rotate_left(n);
                    let (new_left, old) = xq_118_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_118_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_118_find_min<K, V>(node: &Option<Box<Xq118TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_118_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_118_find_max<K, V>(node: &Option<Box<Xq118TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_118_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_118_rank<K: Ord, V>(node: &Option<Box<Xq118TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq118Ord::Less => xq_118_rank(&n.left, key),
            Xq118Ord::Equal => xq_118_size(&n.left),
            Xq118Ord::Greater => 1 + xq_118_size(&n.left) + xq_118_rank(&n.right, key),
        },
    }
}

fn xq_118_kth<K, V>(node: &Option<Box<Xq118TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_118_size(&n.left);
        if k < left_size {
            xq_118_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_118_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_118_in_order<K: Clone, V>(node: &Option<Box<Xq118TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_118_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_118_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq118Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 118 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_118_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq118Ord::Equal => return Some(&n.value),
                Xq118Ord::Less => cur = &n.left,
                Xq118Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_118_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_118_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_118_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_118_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_118_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_118_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_118_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq118VEBTree ---------------

pub struct Xq118VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq118VEBTree>>,
    clusters: Vec<Option<Box<Xq118VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq118VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq118VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq118VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr118KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr118KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr118BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr118KDNode {
    xr_point: Xr118KDPoint,
    xr_left: Option<Box<Xr118KDNode>>,
    xr_right: Option<Box<Xr118KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr118KDTree {
    xr_root: Option<Box<Xr118KDNode>>,
    xr_size: usize,
}

impl Xr118KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr118KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr118KDNode>>,
        point: Xr118KDPoint,
        depth: usize,
    ) -> Box<Xr118KDNode> {
        match node {
            None => Box::new(Xr118KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr118KDPoint) -> Option<Xr118KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr118KDNode>,
        query: &Xr118KDPoint,
        depth: usize,
        best: &mut Xr118KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr118KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr118KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr118KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr118KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr118KDNode>>, pts: &mut Vec<Xr118KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr118KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr118BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr118BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs118PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs118PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs118PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs118PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs118ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs118ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs118ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs118RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs118RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs118RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs118CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs118CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs118CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
    }
}


// --- xt_ Fibonacci Heap ---

/// A node in a Fibonacci heap, storing a key and value with parent/child/sibling pointers.
#[derive(Debug, Clone)]
pub struct XtFibNode<K: Ord + Clone, V: Clone> {
    pub xt_key: K,
    pub xt_value: V,
    xt_degree: usize,
    xt_marked: bool,
    xt_children: Vec<usize>,
    xt_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XtFibNode<K, V> {
    /// Create a new Fibonacci heap node.
    pub fn xt_new(key: K, value: V) -> Self {
        Self {
            xt_key: key,
            xt_value: value,
            xt_degree: 0,
            xt_marked: false,
            xt_children: Vec::new(),
            xt_parent: None,
        }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibNode(key={}, val={}, deg={})", self.xt_key, self.xt_value, self.xt_degree)
    }
}

/// Fibonacci heap with lazy consolidation for amortized O(1) insert and decrease-key.
#[derive(Debug, Clone)]
pub struct XtFibonacciHeap<K: Ord + Clone, V: Clone> {
    xt_nodes: Vec<XtFibNode<K, V>>,
    xt_roots: Vec<usize>,
    xt_min_idx: Option<usize>,
    xt_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XtFibonacciHeap<K, V> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibonacciHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibHeap(size={}, roots={})", self.xt_size, self.xt_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XtFibonacciHeap<K, V> {
    /// Create an empty Fibonacci heap.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_roots: Vec::new(),
            xt_min_idx: None,
            xt_size: 0,
        }
    }

    /// Return the number of elements.
    pub fn xt_len(&self) -> usize {
        self.xt_size
    }

    /// Check if the heap is empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_size == 0
    }

    /// Insert a key-value pair, returning its node index.
    pub fn xt_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xt_nodes.len();
        self.xt_nodes.push(XtFibNode::xt_new(key, value));
        self.xt_roots.push(idx);
        match self.xt_min_idx {
            None => self.xt_min_idx = Some(idx),
            Some(mi) => {
                if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                    self.xt_min_idx = Some(idx);
                }
            }
        }
        self.xt_size += 1;
        idx
    }

    /// Peek at the minimum key-value pair.
    pub fn xt_find_min(&self) -> Option<(&K, &V)> {
        self.xt_min_idx.map(|i| (&self.xt_nodes[i].xt_key, &self.xt_nodes[i].xt_value))
    }

    /// Extract the minimum element.
    pub fn xt_extract_min(&mut self) -> Option<(K, V)> {
        let mi = self.xt_min_idx?;
        let children = self.xt_nodes[mi].xt_children.clone();
        for &c in &children {
            self.xt_nodes[c].xt_parent = None;
            self.xt_roots.push(c);
        }
        self.xt_roots.retain(|&r| r != mi);
        if self.xt_roots.is_empty() {
            self.xt_min_idx = None;
        } else {
            self.xt_min_idx = Some(self.xt_roots[0]);
            self.xt_consolidate();
        }
        self.xt_size -= 1;
        let node = &self.xt_nodes[mi];
        Some((node.xt_key.clone(), node.xt_value.clone()))
    }

    fn xt_consolidate(&mut self) {
        let max_deg = (self.xt_size as f64).log2().ceil() as usize + 2;
        let mut degree_table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xt_roots.clone();
        self.xt_roots.clear();
        for root in roots {
            let mut x = root;
            let mut d = self.xt_nodes[x].xt_degree;
            while d < degree_table.len() {
                if let Some(y) = degree_table[d] {
                    degree_table[d] = None;
                    let (parent, child) = if self.xt_nodes[x].xt_key <= self.xt_nodes[y].xt_key {
                        (x, y)
                    } else {
                        (y, x)
                    };
                    self.xt_nodes[parent].xt_children.push(child);
                    self.xt_nodes[child].xt_parent = Some(parent);
                    self.xt_nodes[parent].xt_degree += 1;
                    self.xt_nodes[child].xt_marked = false;
                    x = parent;
                    d = self.xt_nodes[x].xt_degree;
                } else {
                    break;
                }
            }
            if d < degree_table.len() {
                degree_table[d] = Some(x);
            }
            self.xt_roots.push(x);
        }
        self.xt_roots.sort();
        self.xt_roots.dedup();
        self.xt_min_idx = self.xt_roots.iter().copied()
            .min_by(|&a, &b| self.xt_nodes[a].xt_key.cmp(&self.xt_nodes[b].xt_key));
    }

    /// Decrease the key of a node (key must be smaller than current).
    pub fn xt_decrease_key(&mut self, idx: usize, new_key: K) {
        if new_key >= self.xt_nodes[idx].xt_key {
            return;
        }
        self.xt_nodes[idx].xt_key = new_key;
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[p].xt_key {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
        if let Some(mi) = self.xt_min_idx {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                self.xt_min_idx = Some(idx);
            }
        }
    }

    fn xt_cut(&mut self, x: usize, p: usize) {
        self.xt_nodes[p].xt_children.retain(|&c| c != x);
        self.xt_nodes[p].xt_degree = self.xt_nodes[p].xt_children.len();
        self.xt_nodes[x].xt_parent = None;
        self.xt_nodes[x].xt_marked = false;
        self.xt_roots.push(x);
    }

    fn xt_cascading_cut(&mut self, idx: usize) {
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if !self.xt_nodes[idx].xt_marked {
                self.xt_nodes[idx].xt_marked = true;
            } else {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
    }

    /// Merge another Fibonacci heap into this one.
    pub fn xt_merge(&mut self, other: &mut XtFibonacciHeap<K, V>) {
        let offset = self.xt_nodes.len();
        for mut node in other.xt_nodes.drain(..) {
            node.xt_parent = node.xt_parent.map(|p| p + offset);
            node.xt_children = node.xt_children.iter().map(|&c| c + offset).collect();
            self.xt_nodes.push(node);
        }
        for r in other.xt_roots.drain(..) {
            self.xt_roots.push(r + offset);
        }
        match (self.xt_min_idx, other.xt_min_idx) {
            (None, Some(oi)) => self.xt_min_idx = Some(oi + offset),
            (Some(si), Some(oi)) => {
                let oi2 = oi + offset;
                if self.xt_nodes[oi2].xt_key < self.xt_nodes[si].xt_key {
                    self.xt_min_idx = Some(oi2);
                }
            }
            _ => {}
        }
        self.xt_size += other.xt_size;
        other.xt_size = 0;
        other.xt_min_idx = None;
    }

    /// Return all keys in sorted order (destructive).
    pub fn xt_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xt_size);
        while let Some(pair) = self.xt_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_roots.clear();
        self.xt_min_idx = None;
        self.xt_size = 0;
    }
}

// --- xt_ Doubly-Linked List with Cursors ---

/// A node in a doubly-linked list with prev/next indices.
#[derive(Debug, Clone)]
pub struct XtDllNode<T: Clone> {
    pub xt_value: T,
    xt_prev: Option<usize>,
    xt_next: Option<usize>,
    xt_active: bool,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDllNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DllNode({})", self.xt_value)
    }
}

/// Doubly-linked list with O(1) insertion/deletion at any position via cursor indices.
#[derive(Debug, Clone)]
pub struct XtDoublyLinkedList<T: Clone> {
    xt_nodes: Vec<XtDllNode<T>>,
    xt_head: Option<usize>,
    xt_tail: Option<usize>,
    xt_len: usize,
    xt_free: Vec<usize>,
}

impl<T: Clone> Default for XtDoublyLinkedList<T> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDoublyLinkedList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DLL(len={})", self.xt_len)
    }
}

impl<T: Clone> XtDoublyLinkedList<T> {
    /// Create an empty doubly-linked list.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_head: None,
            xt_tail: None,
            xt_len: 0,
            xt_free: Vec::new(),
        }
    }

    /// Return the length.
    pub fn xt_len(&self) -> usize {
        self.xt_len
    }

    /// Check if empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_len == 0
    }

    fn xt_alloc(&mut self, value: T) -> usize {
        if let Some(idx) = self.xt_free.pop() {
            self.xt_nodes[idx] = XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            };
            idx
        } else {
            let idx = self.xt_nodes.len();
            self.xt_nodes.push(XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            });
            idx
        }
    }

    /// Push a value to the front, returning its index.
    pub fn xt_push_front(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_head {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_head) => {
                self.xt_nodes[idx].xt_next = Some(old_head);
                self.xt_nodes[old_head].xt_prev = Some(idx);
                self.xt_head = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Push a value to the back, returning its index.
    pub fn xt_push_back(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_tail {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_tail) => {
                self.xt_nodes[idx].xt_prev = Some(old_tail);
                self.xt_nodes[old_tail].xt_next = Some(idx);
                self.xt_tail = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value after the given index, returning the new index.
    pub fn xt_insert_after(&mut self, after: usize, value: T) -> usize {
        if !self.xt_nodes[after].xt_active {
            return self.xt_push_back(value);
        }
        let idx = self.xt_alloc(value);
        let next = self.xt_nodes[after].xt_next;
        self.xt_nodes[after].xt_next = Some(idx);
        self.xt_nodes[idx].xt_prev = Some(after);
        self.xt_nodes[idx].xt_next = next;
        if let Some(n) = next {
            self.xt_nodes[n].xt_prev = Some(idx);
        } else {
            self.xt_tail = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value before the given index, returning the new index.
    pub fn xt_insert_before(&mut self, before: usize, value: T) -> usize {
        if !self.xt_nodes[before].xt_active {
            return self.xt_push_front(value);
        }
        let idx = self.xt_alloc(value);
        let prev = self.xt_nodes[before].xt_prev;
        self.xt_nodes[before].xt_prev = Some(idx);
        self.xt_nodes[idx].xt_next = Some(before);
        self.xt_nodes[idx].xt_prev = prev;
        if let Some(p) = prev {
            self.xt_nodes[p].xt_next = Some(idx);
        } else {
            self.xt_head = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Remove the node at the given index.
    pub fn xt_remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.xt_nodes.len() || !self.xt_nodes[idx].xt_active {
            return None;
        }
        let prev = self.xt_nodes[idx].xt_prev;
        let next = self.xt_nodes[idx].xt_next;
        match prev {
            Some(p) => self.xt_nodes[p].xt_next = next,
            None => self.xt_head = next,
        }
        match next {
            Some(n) => self.xt_nodes[n].xt_prev = prev,
            None => self.xt_tail = prev,
        }
        self.xt_nodes[idx].xt_active = false;
        self.xt_nodes[idx].xt_prev = None;
        self.xt_nodes[idx].xt_next = None;
        self.xt_free.push(idx);
        self.xt_len -= 1;
        Some(self.xt_nodes[idx].xt_value.clone())
    }

    /// Pop from front.
    pub fn xt_pop_front(&mut self) -> Option<T> {
        self.xt_head.and_then(|h| self.xt_remove(h))
    }

    /// Pop from back.
    pub fn xt_pop_back(&mut self) -> Option<T> {
        self.xt_tail.and_then(|t| self.xt_remove(t))
    }

    /// Peek at the front value.
    pub fn xt_peek_front(&self) -> Option<&T> {
        self.xt_head.map(|h| &self.xt_nodes[h].xt_value)
    }

    /// Peek at the back value.
    pub fn xt_peek_back(&self) -> Option<&T> {
        self.xt_tail.map(|t| &self.xt_nodes[t].xt_value)
    }

    /// Get value at a given index.
    pub fn xt_get(&self, idx: usize) -> Option<&T> {
        if idx < self.xt_nodes.len() && self.xt_nodes[idx].xt_active {
            Some(&self.xt_nodes[idx].xt_value)
        } else {
            None
        }
    }

    /// Iterate from head to tail.
    pub fn xt_iter_forward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_next;
        }
        result
    }

    /// Iterate from tail to head.
    pub fn xt_iter_backward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_tail;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_prev;
        }
        result
    }

    /// Collect all values into a Vec (front to back).
    pub fn xt_to_vec(&self) -> Vec<T> {
        self.xt_iter_forward().into_iter().cloned().collect()
    }

    /// Clear the list.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_head = None;
        self.xt_tail = None;
        self.xt_len = 0;
        self.xt_free.clear();
    }

    /// Return the head cursor index.
    pub fn xt_head_cursor(&self) -> Option<usize> {
        self.xt_head
    }

    /// Return the tail cursor index.
    pub fn xt_tail_cursor(&self) -> Option<usize> {
        self.xt_tail
    }

    /// Move cursor to next.
    pub fn xt_cursor_next(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_next
        } else {
            None
        }
    }

    /// Move cursor to prev.
    pub fn xt_cursor_prev(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_prev
        } else {
            None
        }
    }

    /// Reverse the list in place.
    pub fn xt_reverse(&mut self) {
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            let next = self.xt_nodes[idx].xt_next;
            let prev = self.xt_nodes[idx].xt_prev;
            self.xt_nodes[idx].xt_next = prev;
            self.xt_nodes[idx].xt_prev = next;
            cur = next;
        }
        std::mem::swap(&mut self.xt_head, &mut self.xt_tail);
    }
}


// --- xu_ Binomial Heap ---

/// A node in a binomial heap.
#[derive(Debug, Clone)]
pub struct XuBinomialNode<K: Ord + Clone, V: Clone> {
    pub xu_key: K,
    pub xu_value: V,
    xu_degree: usize,
    xu_children: Vec<usize>,
    xu_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XuBinomialNode<K, V> {
    /// Create a new binomial node.
    pub fn xu_new(key: K, value: V) -> Self {
        Self { xu_key: key, xu_value: value, xu_degree: 0, xu_children: Vec::new(), xu_parent: None }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinNode(key={}, deg={})", self.xu_key, self.xu_degree)
    }
}

/// Binomial heap with O(log n) insert, extract-min, and merge.
#[derive(Debug, Clone)]
pub struct XuBinomialHeap<K: Ord + Clone, V: Clone> {
    xu_nodes: Vec<XuBinomialNode<K, V>>,
    xu_roots: Vec<usize>,
    xu_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XuBinomialHeap<K, V> {
    fn default() -> Self { Self::xu_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XuBinomialHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BinHeap(size={}, trees={})", self.xu_size, self.xu_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XuBinomialHeap<K, V> {
    /// Create an empty binomial heap.
    pub fn xu_new() -> Self {
        Self { xu_nodes: Vec::new(), xu_roots: Vec::new(), xu_size: 0 }
    }

    /// Return the number of elements.
    pub fn xu_len(&self) -> usize { self.xu_size }

    /// Check if the heap is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_size == 0 }

    /// Insert a key-value pair.
    pub fn xu_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xu_nodes.len();
        self.xu_nodes.push(XuBinomialNode::xu_new(key, value));
        self.xu_add_root(idx);
        self.xu_size += 1;
        self.xu_consolidate();
        idx
    }

    fn xu_add_root(&mut self, idx: usize) {
        self.xu_nodes[idx].xu_parent = None;
        self.xu_roots.push(idx);
    }

    fn xu_consolidate(&mut self) {
        let max_deg = (self.xu_size as f64).log2().ceil() as usize + 2;
        let mut table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xu_roots.clone();
        self.xu_roots.clear();
        for root in roots {
            let mut x = root;
            loop {
                let d = self.xu_nodes[x].xu_degree;
                if d >= table.len() { break; }
                match table[d] {
                    None => { table[d] = Some(x); break; }
                    Some(y) => {
                        table[d] = None;
                        let (p, c) = if self.xu_nodes[x].xu_key <= self.xu_nodes[y].xu_key { (x, y) } else { (y, x) };
                        self.xu_nodes[p].xu_children.push(c);
                        self.xu_nodes[c].xu_parent = Some(p);
                        self.xu_nodes[p].xu_degree += 1;
                        x = p;
                    }
                }
            }
        }
        for slot in &table {
            if let Some(r) = slot {
                self.xu_roots.push(*r);
            }
        }
        self.xu_roots.sort_by_key(|&r| self.xu_nodes[r].xu_degree);
    }

    /// Peek at the minimum.
    pub fn xu_find_min(&self) -> Option<(&K, &V)> {
        self.xu_roots.iter()
            .min_by(|&&a, &&b| self.xu_nodes[a].xu_key.cmp(&self.xu_nodes[b].xu_key))
            .map(|&i| (&self.xu_nodes[i].xu_key, &self.xu_nodes[i].xu_value))
    }

    /// Extract the minimum element.
    pub fn xu_extract_min(&mut self) -> Option<(K, V)> {
        if self.xu_roots.is_empty() { return None; }
        let min_pos = self.xu_roots.iter().enumerate()
            .min_by(|(_, a), (_, b)| self.xu_nodes[**a].xu_key.cmp(&self.xu_nodes[**b].xu_key))
            .map(|(pos, _)| pos)?;
        let min_idx = self.xu_roots.remove(min_pos);
        let children = self.xu_nodes[min_idx].xu_children.clone();
        for &c in &children {
            self.xu_nodes[c].xu_parent = None;
            self.xu_roots.push(c);
        }
        self.xu_size -= 1;
        if !self.xu_roots.is_empty() {
            self.xu_consolidate();
        }
        let n = &self.xu_nodes[min_idx];
        Some((n.xu_key.clone(), n.xu_value.clone()))
    }

    /// Merge another binomial heap into this one.
    pub fn xu_merge(&mut self, other: &mut XuBinomialHeap<K, V>) {
        let off = self.xu_nodes.len();
        for mut n in other.xu_nodes.drain(..) {
            n.xu_parent = n.xu_parent.map(|p| p + off);
            n.xu_children = n.xu_children.iter().map(|&c| c + off).collect();
            self.xu_nodes.push(n);
        }
        for r in other.xu_roots.drain(..) {
            self.xu_roots.push(r + off);
        }
        self.xu_size += other.xu_size;
        other.xu_size = 0;
        self.xu_consolidate();
    }

    /// Drain all elements in sorted order.
    pub fn xu_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xu_size);
        while let Some(pair) = self.xu_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xu_clear(&mut self) {
        self.xu_nodes.clear();
        self.xu_roots.clear();
        self.xu_size = 0;
    }
}

// --- xu_ Disjoint Sparse Table ---

/// Disjoint sparse table for O(1) range queries on static data with an associative operation.
#[derive(Debug, Clone)]
pub struct XuDisjointSparseTable<T: Clone> {
    xu_table: Vec<Vec<T>>,
    xu_data: Vec<T>,
    xu_len: usize,
    xu_levels: usize,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XuDisjointSparseTable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DST(len={}, levels={})", self.xu_len, self.xu_levels)
    }
}

impl<T: Clone + Default + std::ops::Add<Output = T>> XuDisjointSparseTable<T> {
    /// Build a disjoint sparse table for range-sum queries.
    pub fn xu_build(data: &[T]) -> Self {
        let n = data.len();
        if n == 0 {
            return Self { xu_table: Vec::new(), xu_data: Vec::new(), xu_len: 0, xu_levels: 0 };
        }
        let levels = (n as f64).log2().ceil() as usize + 1;
        let mut table = Vec::with_capacity(levels);
        for level in 0..levels {
            let block = 1 << level;
            let mut row = data.to_vec();
            let mut mid = block;
            while mid < n {
                // Build prefix sums going left from mid
                if mid > 0 && mid - 1 < n {
                    let start = if mid >= block { mid - block } else { 0 };
                    let mut i = mid.saturating_sub(1);
                    loop {
                        if i < start { break; }
                        if i + 1 < mid && i + 1 < n {
                            row[i] = row[i].clone() + row[i + 1].clone();
                        }
                        if i == start { break; }
                        i -= 1;
                    }
                }
                // Build prefix sums going right from mid
                let end = std::cmp::min(mid + block, n);
                for i in (mid + 1)..end {
                    row[i] = row[i - 1].clone() + row[i].clone();
                }
                mid += 2 * block;
            }
            table.push(row);
        }
        Self { xu_table: table, xu_data: data.to_vec(), xu_len: n, xu_levels: levels }
    }

    /// Query the sum of elements in the range [l, r] (inclusive).
    pub fn xu_query(&self, l: usize, r: usize) -> T {
        if l == r {
            return self.xu_data[l].clone();
        }
        if l >= self.xu_len || r >= self.xu_len || l > r {
            return T::default();
        }
        // Find the highest bit where l and r differ
        let xor = l ^ r;
        if xor == 0 {
            return self.xu_data[l].clone();
        }
        let level = (usize::BITS - xor.leading_zeros() - 1) as usize;
        if level < self.xu_levels && l < self.xu_table[level].len() && r < self.xu_table[level].len() {
            self.xu_table[level][l].clone() + self.xu_table[level][r].clone()
        } else {
            // Fallback: linear sum
            let mut sum = self.xu_data[l].clone();
            for i in (l + 1)..=r {
                sum = sum + self.xu_data[i].clone();
            }
            sum
        }
    }

    /// Return the length.
    pub fn xu_len(&self) -> usize { self.xu_len }

    /// Check if empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_len == 0 }

    /// Get element at index.
    pub fn xu_get(&self, idx: usize) -> Option<&T> {
        self.xu_data.get(idx)
    }
}

// --- xu_ Monotonic Stack ---

/// Monotonic stack that maintains elements in non-decreasing or non-increasing order.
#[derive(Debug, Clone)]
pub struct XuMonotonicStack<T: Clone + Ord> {
    xu_data: Vec<T>,
    xu_increasing: bool,
}

impl<T: Clone + Ord + std::fmt::Display> std::fmt::Display for XuMonotonicStack<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MonoStack(len={}, inc={})", self.xu_data.len(), self.xu_increasing)
    }
}

impl<T: Clone + Ord> XuMonotonicStack<T> {
    /// Create a monotonically increasing stack.
    pub fn xu_increasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: true }
    }

    /// Create a monotonically decreasing stack.
    pub fn xu_decreasing() -> Self {
        Self { xu_data: Vec::new(), xu_increasing: false }
    }

    /// Push a value, popping elements that violate the monotonic invariant.
    pub fn xu_push(&mut self, value: T) -> Vec<T> {
        let mut popped = Vec::new();
        if self.xu_increasing {
            while let Some(top) = self.xu_data.last() {
                if *top > value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        } else {
            while let Some(top) = self.xu_data.last() {
                if *top < value { popped.push(self.xu_data.pop().unwrap()); } else { break; }
            }
        }
        self.xu_data.push(value);
        popped
    }

    /// Peek at the top.
    pub fn xu_peek(&self) -> Option<&T> { self.xu_data.last() }

    /// Pop from top.
    pub fn xu_pop(&mut self) -> Option<T> { self.xu_data.pop() }

    /// Length.
    pub fn xu_len(&self) -> usize { self.xu_data.len() }

    /// Is empty.
    pub fn xu_is_empty(&self) -> bool { self.xu_data.is_empty() }

    /// Get all elements.
    pub fn xu_as_slice(&self) -> &[T] { &self.xu_data }

    /// Clear the stack.
    pub fn xu_clear(&mut self) { self.xu_data.clear(); }
}


// --- xv_ Cartesian Tree ---

/// A node in a Cartesian tree (BST by key, heap by priority).
#[derive(Debug, Clone)]
pub struct XvCartesianNode<K: Ord + Clone, P: Ord + Clone> {
    pub xv_key: K,
    pub xv_priority: P,
    xv_left: Option<Box<XvCartesianNode<K, P>>>,
    xv_right: Option<Box<XvCartesianNode<K, P>>>,
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianNode<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartNode(k={}, p={})", self.xv_key, self.xv_priority)
    }
}

/// Cartesian tree — BST by key, min-heap by priority. Used for range-minimum queries.
#[derive(Debug, Clone)]
pub struct XvCartesianTree<K: Ord + Clone, P: Ord + Clone> {
    xv_root: Option<Box<XvCartesianNode<K, P>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, P: Ord + Clone> Default for XvCartesianTree<K, P> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, P: Ord + Clone + std::fmt::Display> std::fmt::Display for XvCartesianTree<K, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CartTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, P: Ord + Clone> XvCartesianTree<K, P> {
    /// Create an empty Cartesian tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Return the number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Check if empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    /// Insert a (key, priority) pair maintaining BST-by-key and min-heap-by-priority.
    pub fn xv_insert(&mut self, key: K, priority: P) {
        self.xv_root = Self::xv_insert_node(self.xv_root.take(), key, priority);
        self.xv_size += 1;
    }

    fn xv_insert_node(node: Option<Box<XvCartesianNode<K, P>>>, key: K, priority: P) -> Option<Box<XvCartesianNode<K, P>>> {
        match node {
            None => Some(Box::new(XvCartesianNode { xv_key: key, xv_priority: priority, xv_left: None, xv_right: None })),
            Some(mut n) => {
                if key < n.xv_key {
                    n.xv_left = Self::xv_insert_node(n.xv_left.take(), key.clone(), priority.clone());
                    if n.xv_left.as_ref().is_some_and(|l| l.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_right(n);
                    }
                    Some(n)
                } else {
                    n.xv_right = Self::xv_insert_node(n.xv_right.take(), key.clone(), priority.clone());
                    if n.xv_right.as_ref().is_some_and(|r| r.xv_priority < n.xv_priority) {
                        n = Self::xv_rotate_left(n);
                    }
                    Some(n)
                }
            }
        }
    }

    fn xv_rotate_right(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        left.xv_right = Some(node);
        left
    }

    fn xv_rotate_left(mut node: Box<XvCartesianNode<K, P>>) -> Box<XvCartesianNode<K, P>> {
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        right.xv_left = Some(node);
        right
    }

    /// Search for a key.
    pub fn xv_contains(&self, key: &K) -> bool {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search(node: &Option<Box<XvCartesianNode<K, P>>>, key: &K) -> bool {
        match node {
            None => false,
            Some(n) => {
                if *key == n.xv_key { true }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// In-order traversal returning keys.
    pub fn xv_inorder(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder_walk(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder_walk(node: &Option<Box<XvCartesianNode<K, P>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder_walk(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder_walk(&n.xv_right, result);
        }
    }

    /// Get the root priority (minimum priority).
    pub fn xv_min_priority(&self) -> Option<&P> {
        self.xv_root.as_ref().map(|n| &n.xv_priority)
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Build from a sequence of (key, priority) pairs.
    pub fn xv_from_pairs(pairs: &[(K, P)]) -> Self {
        let mut tree = Self::xv_new();
        for (k, p) in pairs { tree.xv_insert(k.clone(), p.clone()); }
        tree
    }

    /// Height of the tree.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvCartesianNode<K, P>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(
                Self::xv_node_height(&n.xv_left),
                Self::xv_node_height(&n.xv_right),
            ),
        }
    }
}

// --- xv_ Weight-Balanced Tree ---

/// A node in a weight-balanced tree (BB[α] tree).
#[derive(Debug, Clone)]
pub struct XvWBNode<K: Ord + Clone, V: Clone> {
    pub xv_key: K,
    pub xv_value: V,
    xv_left: Option<Box<XvWBNode<K, V>>>,
    xv_right: Option<Box<XvWBNode<K, V>>>,
    xv_weight: usize,
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWBNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBNode(k={}, w={})", self.xv_key, self.xv_weight)
    }
}

/// Weight-balanced tree (BB[α] tree) with α = 0.29 for balanced operations.
#[derive(Debug, Clone)]
pub struct XvWeightBalancedTree<K: Ord + Clone, V: Clone> {
    xv_root: Option<Box<XvWBNode<K, V>>>,
    xv_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XvWeightBalancedTree<K, V> {
    fn default() -> Self { Self::xv_new() }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone> std::fmt::Display for XvWeightBalancedTree<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WBTree(size={})", self.xv_size)
    }
}

impl<K: Ord + Clone, V: Clone> XvWeightBalancedTree<K, V> {
    const ALPHA: f64 = 0.29;

    /// Create an empty weight-balanced tree.
    pub fn xv_new() -> Self { Self { xv_root: None, xv_size: 0 } }

    /// Number of elements.
    pub fn xv_len(&self) -> usize { self.xv_size }

    /// Is the tree empty.
    pub fn xv_is_empty(&self) -> bool { self.xv_size == 0 }

    fn xv_weight(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node { None => 1, Some(n) => n.xv_weight }
    }

    fn xv_update_weight(node: &mut Box<XvWBNode<K, V>>) {
        node.xv_weight = Self::xv_weight(&node.xv_left) + Self::xv_weight(&node.xv_right);
    }

    fn xv_is_balanced(node: &Box<XvWBNode<K, V>>) -> bool {
        let lw = Self::xv_weight(&node.xv_left) as f64;
        let rw = Self::xv_weight(&node.xv_right) as f64;
        let total = node.xv_weight as f64;
        lw >= Self::ALPHA * total && rw >= Self::ALPHA * total
    }

    /// Insert a key-value pair.
    pub fn xv_insert(&mut self, key: K, value: V) {
        let inserted = Self::xv_insert_node(self.xv_root.take(), key, value);
        self.xv_root = inserted.0;
        if inserted.1 { self.xv_size += 1; }
    }

    fn xv_insert_node(node: Option<Box<XvWBNode<K, V>>>, key: K, value: V) -> (Option<Box<XvWBNode<K, V>>>, bool) {
        match node {
            None => {
                let n = Box::new(XvWBNode { xv_key: key, xv_value: value, xv_left: None, xv_right: None, xv_weight: 2 });
                (Some(n), true)
            }
            Some(mut n) => {
                let inserted;
                if key < n.xv_key {
                    let r = Self::xv_insert_node(n.xv_left.take(), key, value);
                    n.xv_left = r.0;
                    inserted = r.1;
                } else if key > n.xv_key {
                    let r = Self::xv_insert_node(n.xv_right.take(), key, value);
                    n.xv_right = r.0;
                    inserted = r.1;
                } else {
                    n.xv_value = value;
                    return (Some(n), false);
                }
                Self::xv_update_weight(&mut n);
                let n = Self::xv_rebalance(n);
                (Some(n), inserted)
            }
        }
    }

    fn xv_rebalance(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if !Self::xv_is_balanced(&node) {
            let lw = Self::xv_weight(&node.xv_left);
            let rw = Self::xv_weight(&node.xv_right);
            if lw < rw {
                node = Self::xv_rotate_left_wb(node);
            } else {
                node = Self::xv_rotate_right_wb(node);
            }
        }
        node
    }

    fn xv_rotate_left_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_right.is_none() { return node; }
        let mut right = node.xv_right.take().unwrap();
        node.xv_right = right.xv_left.take();
        Self::xv_update_weight(&mut node);
        right.xv_left = Some(node);
        Self::xv_update_weight(&mut right);
        right
    }

    fn xv_rotate_right_wb(mut node: Box<XvWBNode<K, V>>) -> Box<XvWBNode<K, V>> {
        if node.xv_left.is_none() { return node; }
        let mut left = node.xv_left.take().unwrap();
        node.xv_left = left.xv_right.take();
        Self::xv_update_weight(&mut node);
        left.xv_right = Some(node);
        Self::xv_update_weight(&mut left);
        left
    }

    /// Look up a key.
    pub fn xv_get(&self, key: &K) -> Option<&V> {
        Self::xv_search(&self.xv_root, key)
    }

    fn xv_search<'a>(node: &'a Option<Box<XvWBNode<K, V>>>, key: &K) -> Option<&'a V> {
        match node {
            None => None,
            Some(n) => {
                if *key == n.xv_key { Some(&n.xv_value) }
                else if *key < n.xv_key { Self::xv_search(&n.xv_left, key) }
                else { Self::xv_search(&n.xv_right, key) }
            }
        }
    }

    /// Check if key exists.
    pub fn xv_contains(&self, key: &K) -> bool { self.xv_get(key).is_some() }

    /// In-order traversal.
    pub fn xv_keys(&self) -> Vec<K> {
        let mut result = Vec::new();
        Self::xv_inorder(&self.xv_root, &mut result);
        result
    }

    fn xv_inorder(node: &Option<Box<XvWBNode<K, V>>>, result: &mut Vec<K>) {
        if let Some(n) = node {
            Self::xv_inorder(&n.xv_left, result);
            result.push(n.xv_key.clone());
            Self::xv_inorder(&n.xv_right, result);
        }
    }

    /// Clear the tree.
    pub fn xv_clear(&mut self) { self.xv_root = None; self.xv_size = 0; }

    /// Height.
    pub fn xv_height(&self) -> usize {
        Self::xv_node_height(&self.xv_root)
    }

    fn xv_node_height(node: &Option<Box<XvWBNode<K, V>>>) -> usize {
        match node {
            None => 0,
            Some(n) => 1 + std::cmp::max(Self::xv_node_height(&n.xv_left), Self::xv_node_height(&n.xv_right)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_bold_and_italic() {
        let tokens = tokenize_inline("hello **world** and *italic*");
        assert!(tokens.contains(&MarkdownToken::Bold("world".into())));
        assert!(tokens.contains(&MarkdownToken::Italic("italic".into())));
    }

    #[test]
    fn tokenize_inline_code() {
        let tokens = tokenize_inline("use `cargo build` here");
        assert!(tokens.contains(&MarkdownToken::Code("cargo build".into())));
    }

    #[test]
    fn tokenize_link() {
        let tokens = tokenize_inline("see [docs](https://example.com)");
        assert!(tokens.contains(&MarkdownToken::Link(
            "docs".into(),
            "https://example.com".into()
        )));
    }

    #[test]
    fn render_plain_text_strips_formatting() {
        let tokens = vec![
            MarkdownToken::Text("hello ".into()),
            MarkdownToken::Bold("world".into()),
        ];
        assert_eq!(render_to_plain_text(&tokens), "hello world");
    }

    #[test]
    fn block_tokenize_headings() {
        let tokens = tokenize_block("# Title\n## Subtitle");
        assert_eq!(tokens[0], MarkdownToken::Heading(1, "Title".into()));
        assert_eq!(tokens[1], MarkdownToken::Heading(2, "Subtitle".into()));
    }

    #[test]
    fn block_tokenize_list_items() {
        let tokens = tokenize_block("- first\n* second");
        assert_eq!(tokens[0], MarkdownToken::ListItemMd("first".into()));
        assert_eq!(tokens[1], MarkdownToken::ListItemMd("second".into()));
    }

    #[test]
    fn block_tokenize_code_block() {
        let input = "```rust\nfn main() {}\n```";
        let tokens = tokenize_block(input);
        assert_eq!(
            tokens[0],
            MarkdownToken::CodeBlock("fn main() {}".into(), Some("rust".into()))
        );
    }

    #[test]
    fn block_tokenize_code_block_no_lang() {
        let input = "```\nhello\nworld\n```";
        let tokens = tokenize_block(input);
        assert_eq!(
            tokens[0],
            MarkdownToken::CodeBlock("hello\nworld".into(), None)
        );
    }

    #[test]
    fn block_tokenize_paragraphs() {
        let tokens = tokenize_block("first para\n\nsecond para");
        assert!(tokens.contains(&MarkdownToken::Paragraph));
        assert!(tokens.contains(&MarkdownToken::Text("first para".into())));
        assert!(tokens.contains(&MarkdownToken::Text("second para".into())));
    }

    #[test]
    fn render_html_basic() {
        let tokens = vec![
            MarkdownToken::Bold("hi".into()),
            MarkdownToken::Italic("there".into()),
            MarkdownToken::Code("x".into()),
        ];
        let html = render_to_html(&tokens);
        assert_eq!(html, "<strong>hi</strong><em>there</em><code>x</code>");
    }

    #[test]
    fn render_html_heading_and_link() {
        let tokens = vec![
            MarkdownToken::Heading(2, "Title".into()),
            MarkdownToken::Link("click".into(), "https://example.com".into()),
        ];
        let html = render_to_html(&tokens);
        assert!(html.contains("<h2>Title</h2>"));
        assert!(html.contains("<a href=\"https://example.com\">click</a>"));
    }

    #[test]
    fn render_html_escapes_entities() {
        let tokens = vec![MarkdownToken::Text("<script>&".into())];
        let html = render_to_html(&tokens);
        assert_eq!(html, "&lt;script&gt;&amp;");
    }

    #[test]
    fn extract_links_from_tokens() {
        let tokens = tokenize_inline("see [a](http://a.com) and [b](http://b.com)");
        let links = extract_links(&tokens);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0], ("a".into(), "http://a.com".into()));
        assert_eq!(links[1], ("b".into(), "http://b.com".into()));
    }

    #[test]
    fn extract_headings_from_block() {
        let tokens = tokenize_block("# One\nsome text\n## Two\n### Three");
        let headings = extract_headings(&tokens);
        assert_eq!(headings.len(), 3);
        assert_eq!(headings[0], (1, "One".into()));
        assert_eq!(headings[1], (2, "Two".into()));
        assert_eq!(headings[2], (3, "Three".into()));
    }

    #[test]
    fn word_count_across_tokens() {
        let tokens = vec![
            MarkdownToken::Text("hello world".into()),
            MarkdownToken::Bold("one more".into()),
            MarkdownToken::Paragraph,
        ];
        assert_eq!(word_count(&tokens), 4);
    }

    #[test]
    fn strip_markdown_convenience() {
        assert_eq!(strip_markdown("**bold** and *italic*"), "bold and italic");
    }

    // --- new tests ---

    #[test]
    fn parse_table_basic() {
        let table = "| Name | Age |\n|---|---|\n| Alice | 30 |\n| Bob | 25 |";
        let rows = parse_markdown_table(table).unwrap();
        assert_eq!(rows.len(), 3); // header + 2 data rows
        assert_eq!(rows[0].cells, vec!["Name", "Age"]);
        assert_eq!(rows[1].cells, vec!["Alice", "30"]);
        assert_eq!(rows[2].cells, vec!["Bob", "25"]);
    }

    #[test]
    fn parse_table_returns_none_for_non_table() {
        assert!(parse_markdown_table("just some text").is_none());
        assert!(parse_markdown_table("one\ntwo").is_none());
    }

    #[test]
    fn compute_stats_counts_correctly() {
        let tokens = tokenize_block(
            "# Heading\n\nSome text\n\n- item1\n- item2\n\n```rust\ncode\n```\n\n[link](url)",
        );
        let stats = compute_stats(&tokens);
        assert_eq!(stats.headings, 1);
        assert_eq!(stats.list_items, 2);
        assert_eq!(stats.code_blocks, 1);
        assert!(stats.paragraphs >= 1);
        assert_eq!(stats.total_tokens, tokens.len());
    }

    #[test]
    fn generate_toc_produces_links() {
        let tokens = tokenize_block("# Introduction\n## Getting Started\n## API\n### Details");
        let toc = generate_toc(&tokens);
        assert!(toc.contains("- [Introduction](#introduction)"));
        assert!(toc.contains("  - [Getting Started](#getting-started)"));
        assert!(toc.contains("  - [API](#api)"));
        assert!(toc.contains("    - [Details](#details)"));
    }

    #[test]
    fn generate_toc_empty_for_no_headings() {
        let tokens = tokenize_block("just text\n\nmore text");
        assert!(generate_toc(&tokens).is_empty());
    }

    #[test]
    fn normalize_whitespace_collapses() {
        assert_eq!(normalize_whitespace("  hello   world  "), "hello world");
        assert_eq!(normalize_whitespace("a\n\n\tb"), "a b");
        assert_eq!(normalize_whitespace(""), "");
        assert_eq!(normalize_whitespace("   "), "");
    }

    #[test]
    fn extract_code_blocks_returns_all() {
        let input = "```rust\nfn main() {}\n```\n\ntext\n\n```\nplain\n```";
        let tokens = tokenize_block(input);
        let blocks = extract_code_blocks(&tokens);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0], (Some("rust".into()), "fn main() {}".into()));
        assert_eq!(blocks[1], (None, "plain".into()));
    }

    #[test]
    fn compute_stats_empty_tokens() {
        let stats = compute_stats(&[]);
        assert_eq!(stats, MarkdownStats::default());
    }

    #[test]
    fn build_outline_extracts_headings_with_line_numbers() {
        let doc = "# Intro\nsome text\n## Details\nmore text\n### Deep";
        let outline = build_outline(doc);
        assert_eq!(outline.len(), 3);
        assert_eq!(outline[0], OutlineEntry { level: 1, text: "Intro".into(), line_number: 1 });
        assert_eq!(outline[1], OutlineEntry { level: 2, text: "Details".into(), line_number: 3 });
        assert_eq!(outline[2], OutlineEntry { level: 3, text: "Deep".into(), line_number: 5 });
    }

    #[test]
    fn build_outline_empty_doc() {
        assert!(build_outline("no headings here").is_empty());
    }

    #[test]
    fn collect_links_finds_all_links() {
        let doc = "see [a](http://a.com)\nnormal line\n[b](http://b.com) end";
        let links = collect_links(doc);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0], FoundLink { label: "a".into(), url: "http://a.com".into(), line_number: 1 });
        assert_eq!(links[1], FoundLink { label: "b".into(), url: "http://b.com".into(), line_number: 3 });
    }

    #[test]
    fn collect_links_empty_doc() {
        assert!(collect_links("no links").is_empty());
    }

    #[test]
    fn detect_block_structure_classifies_lines() {
        let doc = "# Title\n\n- item\n```\ncode\n```\nparagraph";
        let blocks = detect_block_structure(doc);
        assert_eq!(blocks[0], BlockKind::Heading(1));
        assert_eq!(blocks[1], BlockKind::BlankLine);
        assert_eq!(blocks[2], BlockKind::ListItem);
        assert_eq!(blocks[3], BlockKind::CodeFence);
        assert_eq!(blocks[4], BlockKind::Paragraph);
        assert_eq!(blocks[5], BlockKind::CodeFence);
        assert_eq!(blocks[6], BlockKind::Paragraph);
    }

    #[test]
    fn markdown_table_parse_and_render() {
        let input = "| Name | Age |\n|---|---|\n| Alice | 30 |\n| Bob | 25 |";
        let table = MarkdownTable::parse(input).unwrap();
        assert_eq!(table.headers, vec!["Name", "Age"]);
        assert_eq!(table.row_count(), 2);
        assert_eq!(table.column_count(), 2);
        assert_eq!(table.rows[0], vec!["Alice", "30"]);
        let rendered = table.render();
        assert!(rendered.contains("| Name | Age |"));
        assert!(rendered.contains("| Alice | 30 |"));
    }

    #[test]
    fn markdown_table_parse_returns_none_for_non_table() {
        assert!(MarkdownTable::parse("just text").is_none());
    }

    #[test]
    fn extract_links_from_text_finds_all() {
        let doc = "See [rust](https://rust-lang.org) and\n[docs](https://docs.rs) for info.";
        let links = extract_links_from_text(doc);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].0, "rust");
        assert_eq!(links[0].1, "https://rust-lang.org");
        assert_eq!(links[1].0, "docs");
        assert_eq!(links[1].1, "https://docs.rs");
    }

    #[test]
    fn extract_links_from_text_empty() {
        assert!(extract_links_from_text("no links here").is_empty());
    }

    #[test]
    fn extract_headings_from_text_returns_toc() {
        let doc = "# Title\ntext\n## Section A\n## Section B\n### Subsection";
        let headings = extract_headings_from_text(doc);
        assert_eq!(headings.len(), 4);
        assert_eq!(headings[0], (1, "Title".into()));
        assert_eq!(headings[1], (2, "Section A".into()));
        assert_eq!(headings[2], (2, "Section B".into()));
        assert_eq!(headings[3], (3, "Subsection".into()));
    }

    #[test]
    fn extract_headings_from_text_empty() {
        assert!(extract_headings_from_text("no headings").is_empty());
    }

    #[test]
    fn front_matter_parse_basic() {
        let doc = "---\ntitle: Hello World\nauthor: Alice\n---\n# Heading\nBody text";
        let fm = MarkdownFrontMatter::parse(doc).unwrap();
        assert_eq!(fm.len(), 2);
        assert_eq!(fm.get("title"), Some("Hello World"));
        assert_eq!(fm.get("author"), Some("Alice"));
    }

    #[test]
    fn front_matter_parse_none_when_missing() {
        assert!(MarkdownFrontMatter::parse("# Just a heading").is_none());
    }

    #[test]
    fn front_matter_strip() {
        let doc = "---\ntitle: Test\n---\n# Heading\nBody";
        let body = MarkdownFrontMatter::strip_front_matter(doc);
        assert!(body.starts_with("# Heading"));
    }

    #[test]
    fn front_matter_display() {
        let fm = MarkdownFrontMatter {
            fields: vec![("a".into(), "b".into())],
        };
        assert_eq!(fm.to_string(), "FrontMatter(1 fields)");
    }

    #[test]
    fn markdown_toc_generates_entries() {
        let doc = "# Title\n## Section A\n## Section B\n### Subsection";
        let toc = markdown_toc(doc);
        assert!(toc.contains("- [Title]"));
        assert!(toc.contains("  - [Section A]"));
        assert!(toc.contains("    - [Subsection]"));
    }

    #[test]
    fn markdown_toc_empty_for_no_headings() {
        assert!(markdown_toc("just text\nmore text").is_empty());
    }

    #[test]
    fn markdown_word_count_basic() {
        let doc = "# Title\n\nHello world this is a test.\n\n```\ncode here\n```\n\nMore words.";
        let stats = markdown_word_count(doc);
        assert_eq!(stats.words, 10); // # Title (2) + Hello world this is a test (6) + More words (2)
        assert!(stats.code_block_lines > 0);
    }

    #[test]
    fn markdown_word_count_with_front_matter() {
        let doc = "---\ntitle: Test\n---\nHello world.";
        let stats = markdown_word_count(doc);
        assert_eq!(stats.words, 2);
    }

    #[test]
    fn test_token_text_content() {
        assert_eq!(MarkdownToken::Bold("hello".into()).text_content(), "hello");
        assert_eq!(MarkdownToken::Paragraph.text_content(), "");
        assert_eq!(MarkdownToken::Link("text".into(), "url".into()).text_content(), "text");
    }

    #[test]
    fn test_token_is_block_and_inline() {
        assert!(MarkdownToken::Heading(1, "H1".into()).is_block());
        assert!(!MarkdownToken::Heading(1, "H1".into()).is_inline());
        assert!(MarkdownToken::Bold("b".into()).is_inline());
        assert!(!MarkdownToken::Bold("b".into()).is_block());
    }

    #[test]
    fn test_token_kind_name() {
        assert_eq!(MarkdownToken::Bold("b".into()).kind_name(), "bold");
        assert_eq!(MarkdownToken::Code("c".into()).kind_name(), "code");
        assert_eq!(MarkdownToken::Paragraph.kind_name(), "paragraph");
    }

    #[test]
    fn test_token_display() {
        assert_eq!(format!("{}", MarkdownToken::Bold("hello".into())), "**hello**");
        assert_eq!(format!("{}", MarkdownToken::Code("x".into())), "`x`");
        assert_eq!(format!("{}", MarkdownToken::Heading(2, "Title".into())), "## Title");
        assert_eq!(format!("{}", MarkdownToken::Link("text".into(), "url".into())), "[text](url)");
        assert_eq!(format!("{}", MarkdownToken::ListItemMd("item".into())), "- item");
    }

    #[test]
    fn test_count_tokens_by_kind() {
        let tokens = vec![
            MarkdownToken::Text("a".into()),
            MarkdownToken::Bold("b".into()),
            MarkdownToken::Text("c".into()),
        ];
        let counts = count_tokens_by_kind(&tokens);
        assert_eq!(counts["text"], 2);
        assert_eq!(counts["bold"], 1);
    }

    #[test]
    fn test_total_text_length() {
        let tokens = vec![
            MarkdownToken::Text("hello".into()),
            MarkdownToken::Bold("world".into()),
        ];
        assert_eq!(total_text_length(&tokens), 10);
    }

    #[test]
    fn test_extract_urls() {
        let tokens = vec![
            MarkdownToken::Link("a".into(), "https://a.com".into()),
            MarkdownToken::Text("x".into()),
            MarkdownToken::Link("b".into(), "https://b.com".into()),
        ];
        let urls = extract_urls(&tokens);
        assert_eq!(urls, vec!["https://a.com", "https://b.com"]);
    }

    #[test]
    fn test_extract_all_code() {
        let tokens = vec![
            MarkdownToken::Code("inline".into()),
            MarkdownToken::CodeBlock("block".into(), None),
        ];
        let code = extract_all_code(&tokens);
        assert_eq!(code, vec!["inline", "block"]);
    }

    #[test]
    fn test_estimated_reading_time() {
        let text = (0..400).map(|_| "word").collect::<Vec<_>>().join(" ");
        let time = estimated_reading_time(&text);
        assert!((time - 2.0).abs() < 0.1);
    }

    #[test]
    fn test_markdown_document_parse() {
        let doc = MarkdownDocument::parse("# Title\n\nHello **world**\n\n## Section\n\nSome text");
        assert!(doc.token_count() > 0);
        let headings = doc.headings();
        assert_eq!(headings.len(), 2);
        assert_eq!(headings[0], (1, "Title"));
        assert_eq!(headings[1], (2, "Section"));
        assert!(doc.word_count() > 0);
        assert!(format!("{doc}").contains("tokens"));
    }

    #[test]
    fn test_markdown_document_links() {
        let doc = MarkdownDocument::parse("[Rust](https://rust-lang.org) and [More](https://example.com)");
        let links = doc.links();
        assert_eq!(links.len(), 2);
        assert_eq!(links[0], ("Rust", "https://rust-lang.org"));
    }

    #[test]
    fn test_markdown_document_summary() {
        let doc = MarkdownDocument::parse("Hello world this is a long document with many words");
        let summary = doc.summary(10);
        assert!(summary.len() <= 15); // includes multi-byte ellipsis
        assert!(summary.ends_with('…'));
    }

    #[test]
    fn test_table_of_contents() {
        let tokens = vec![
            MarkdownToken::Heading(1, "Introduction".into()),
            MarkdownToken::Text("Some text".into()),
            MarkdownToken::Heading(2, "Details".into()),
            MarkdownToken::Heading(2, "Examples".into()),
        ];
        let toc = TableOfContents::from_tokens(&tokens);
        assert_eq!(toc.len(), 3);
        assert_eq!(toc.max_depth(), 2);
        let s = format!("{toc}");
        assert!(s.contains("Introduction"));
        assert!(s.contains("  - Details"));
    }

    #[test]
    fn test_table_of_contents_empty() {
        let toc = TableOfContents::from_tokens(&[]);
        assert!(toc.is_empty());
        assert_eq!(toc.max_depth(), 0);
    }

    #[test]
    fn test_toc_entry_display() {
        let entry = TocEntry { level: 3, title: "Sub-section".into() };
        let s = format!("{entry}");
        assert!(s.contains("    - Sub-section"));
    }

    // -- New functionality tests --

    #[test]
    fn escape_markdown_special_chars() {
        let escaped = escape_markdown("Hello *world* [link](url)");
        assert!(escaped.contains("\\*"));
        assert!(escaped.contains("\\["));
        assert!(escaped.contains("\\]"));
        assert!(escaped.contains("\\("));
    }

    #[test]
    fn unescape_markdown_roundtrip() {
        let original = "Hello *bold* and `code`";
        let escaped = escape_markdown(original);
        let unescaped = unescape_markdown(&escaped);
        assert_eq!(unescaped, original);
    }

    #[test]
    fn unescape_markdown_plain_backslash() {
        let result = unescape_markdown("no special \\x here");
        assert_eq!(result, "no special \\x here");
    }

    #[test]
    fn markdown_config_new_defaults() {
        let cfg = MarkdownConfigDetail::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn markdown_config_set_and_get_option() {
        let mut cfg = MarkdownConfigDetail::new();
        cfg.set_option("hard_breaks", "true");
        assert!(cfg.has_option("hard_breaks"));
        assert_eq!(cfg.get_option("hard_breaks"), Some("true"));
    }

    #[test]
    fn table_add_row_ok() {
        let mut table = MarkdownTable {
            headers: vec!["A".into(), "B".into()],
            rows: vec![],
        };
        assert!(table.add_row(vec!["1".into(), "2".into()]).is_ok());
        assert_eq!(table.row_count(), 1);
    }

    #[test]
    fn table_add_row_wrong_columns() {
        let mut table = MarkdownTable {
            headers: vec!["A".into(), "B".into()],
            rows: vec![],
        };
        assert!(table.add_row(vec!["1".into()]).is_err());
    }

    #[test]
    fn table_remove_row() {
        let mut table = MarkdownTable {
            headers: vec!["A".into()],
            rows: vec![vec!["1".into()], vec!["2".into()], vec!["3".into()]],
        };
        let removed = table.remove_row(1);
        assert_eq!(removed, Some(vec!["2".into()]));
        assert_eq!(table.row_count(), 2);
        assert!(table.remove_row(99).is_none());
    }

    #[test]
    fn table_render_aligned() {
        let table = MarkdownTable {
            headers: vec!["Name".into(), "Age".into()],
            rows: vec![
                vec!["Alice".into(), "30".into()],
                vec!["Bob".into(), "25".into()],
            ],
        };
        let rendered = table.render_aligned();
        assert!(rendered.contains("Alice"));
        assert!(rendered.contains("---"));
        // Headers and data should be present
        assert!(rendered.lines().count() >= 4);
    }

    #[test]
    fn markdown_stats_structural_count() {
        let stats = MarkdownStats {
            headings: 2,
            links: 3,
            code_blocks: 1,
            list_items: 4,
            paragraphs: 5,
            total_tokens: 15,
        };
        assert_eq!(stats.structural_count(), 10);
        assert!(!stats.is_empty());
    }

    #[test]
    fn markdown_stats_empty() {
        let stats = MarkdownStats::default();
        assert!(stats.is_empty());
        assert_eq!(stats.structural_count(), 0);
    }

    // -- MarkdownTableRenderer tests --

    #[test]
    fn table_renderer_basic() {
        let r = MarkdownTableRenderer::new();
        let out = r.render(&["Name", "Age"], &[
            vec!["Alice".into(), "30".into()],
            vec!["Bob".into(), "25".into()],
        ]);
        assert!(out.contains("Alice"));
        assert!(out.contains("Bob"));
        assert!(out.contains("|"));
    }

    #[test]
    fn table_renderer_empty() {
        let r = MarkdownTableRenderer::new();
        let out = r.render(&["A"], &[]);
        assert!(out.contains("A"));
    }

    #[test]
    fn table_column_count_from_text() {
        let text = "| A | B | C |\n|---|---|---|";
        assert_eq!(MarkdownTableRenderer::column_count_from_text(text), 3);
    }

    // -- MarkdownChecklistRenderer tests --

    #[test]
    fn checklist_parse_and_toggle() {
        let text = "- [x] Done\n- [ ] Pending\n- [ ] Also pending";
        let mut cl = MarkdownChecklistRenderer::parse(text);
        assert_eq!(cl.item_count(), 3);
        assert_eq!(cl.checked_count(), 1);
        cl.toggle(1);
        assert_eq!(cl.checked_count(), 2);
    }

    #[test]
    fn checklist_progress() {
        let text = "- [x] A\n- [x] B\n- [ ] C\n- [ ] D";
        let cl = MarkdownChecklistRenderer::parse(text);
        assert!((cl.progress() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn checklist_to_markdown() {
        let text = "- [x] Done\n- [ ] Pending";
        let cl = MarkdownChecklistRenderer::parse(text);
        let md = cl.to_markdown();
        assert!(md.contains("- [x] Done"));
        assert!(md.contains("- [ ] Pending"));
    }

    // -- MarkdownLinkResolver tests --

    #[test]
    fn link_resolver_relative() {
        let r = MarkdownLinkResolver::new("/docs");
        assert_eq!(r.resolve("image.png"), "/docs/image.png");
        assert_eq!(r.resolve("sub/page.md"), "/docs/sub/page.md");
    }

    #[test]
    fn link_resolver_absolute_unchanged() {
        let r = MarkdownLinkResolver::new("/docs");
        assert_eq!(r.resolve("https://example.com"), "https://example.com");
        assert_eq!(r.resolve("/root/file"), "/root/file");
    }

    // -- Heading outline tests --

    #[test]
    fn heading_outline_extraction() {
        let text = "# Title\n\nSome text\n\n## Section 1\n\n### Subsection\n\n## Section 2";
        let headings = extract_heading_outline(text);
        assert_eq!(headings.len(), 4);
        assert_eq!(headings[0], (1, "Title".into(), 1));
        assert_eq!(headings[1], (2, "Section 1".into(), 5));
    }

    #[test]
    fn heading_outline_format() {
        let headings = vec![
            (1, "Title".into(), 1),
            (2, "Section".into(), 3),
        ];
        let formatted = format_outline(&headings);
        assert!(formatted.contains("Title"));
        assert!(formatted.contains("  Section")); // indented
    }

    // -----------------------------------------------------------------------
    // MarkdownFootnoteRenderer tests
    // -----------------------------------------------------------------------

    #[test]
    fn footnote_add_and_resolve() {
        let mut renderer = MarkdownFootnoteRenderer::new();
        renderer.add_definition("note1", "First footnote");
        renderer.add_definition("note2", "Second footnote");
        assert_eq!(renderer.count(), 2);
        let r = renderer.resolve("note1").unwrap();
        assert_eq!(r.index, 0);
        assert_eq!(r.label, "note1");
        assert!(renderer.resolve("missing").is_none());
    }

    #[test]
    fn footnote_render_section() {
        let mut renderer = MarkdownFootnoteRenderer::new();
        renderer.add_definition("a", "Alpha");
        renderer.add_definition("b", "Beta");
        let section = renderer.render_section();
        assert!(section.starts_with("---\n"));
        assert!(section.contains("[^1] (a): Alpha"));
        assert!(section.contains("[^2] (b): Beta"));
    }

    #[test]
    fn footnote_empty_section() {
        let renderer = MarkdownFootnoteRenderer::new();
        assert_eq!(renderer.render_section(), "");
    }

    #[test]
    fn footnote_labels() {
        let mut renderer = MarkdownFootnoteRenderer::new();
        renderer.add_definition("x", "");
        renderer.add_definition("y", "");
        assert_eq!(renderer.labels(), vec!["x", "y"]);
    }

    #[test]
    fn footnote_parse_definitions() {
        let text = "[^fn1]: First\n[^fn2]: Second\nNormal line\n";
        let defs = MarkdownFootnoteRenderer::parse_definitions(text);
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0], ("fn1".to_string(), "First".to_string()));
    }

    // -----------------------------------------------------------------------
    // MarkdownTaskListToggle tests
    // -----------------------------------------------------------------------

    #[test]
    fn task_list_parse() {
        let src = "- [ ] Task A\n- [x] Task B\n- [ ] Task C\n";
        let toggle = MarkdownTaskListToggle::parse(src);
        assert_eq!(toggle.items().len(), 3);
        assert_eq!(toggle.incomplete_count(), 2);
        assert_eq!(toggle.complete_count(), 1);
    }

    #[test]
    fn task_list_toggle_incomplete() {
        let src = "- [ ] Task A\n- [x] Task B\n";
        let toggle = MarkdownTaskListToggle::parse(src);
        let result = toggle.toggle(src, 0);
        assert!(result.contains("- [x] Task A"));
    }

    #[test]
    fn task_list_toggle_complete() {
        let src = "- [ ] Task A\n- [x] Task B\n";
        let toggle = MarkdownTaskListToggle::parse(src);
        let result = toggle.toggle(src, 1);
        assert!(result.contains("- [ ] Task B"));
    }

    #[test]
    fn task_list_progress() {
        let src = "- [x] Done\n- [ ] Todo\n- [x] Also Done\n- [ ] Another\n";
        let toggle = MarkdownTaskListToggle::parse(src);
        assert_eq!(toggle.progress_percent(), 50);
    }

    #[test]
    fn task_list_empty_progress() {
        let toggle = MarkdownTaskListToggle::parse("No tasks here");
        assert_eq!(toggle.progress_percent(), 0);
    }

    // -----------------------------------------------------------------------
    // Anchor generation tests
    // -----------------------------------------------------------------------

    #[test]
    fn anchor_simple_heading() {
        assert_eq!(generate_anchor("Hello World"), "hello-world");
    }

    #[test]
    fn anchor_special_chars() {
        assert_eq!(generate_anchor("What's New?"), "whats-new");
    }

    #[test]
    fn anchor_multiple_spaces() {
        assert_eq!(generate_anchor("A   B"), "a-b");
    }

    #[test]
    fn anchor_heading_anchors() {
        let headings = vec![
            (1, "Title".into(), 1),
            (2, "Section One".into(), 5),
        ];
        let anchored = generate_heading_anchors(&headings);
        assert_eq!(anchored[0].2, "title");
        assert_eq!(anchored[1].2, "section-one");
    }

    #[test]
    fn toc_generation() {
        let headings = vec![
            (1, "Intro".into(), 1),
            (2, "Details".into(), 3),
        ];
        let toc = build_toc(&headings);
        assert!(toc.contains("[Intro](#intro)"));
        assert!(toc.contains("[Details](#details)"));
    }

    // -----------------------------------------------------------------------
    // Code-fence language detection tests
    // -----------------------------------------------------------------------

    #[test]
    fn detect_language_rust() {
        assert_eq!(detect_code_fence_language("```rust"), Some("rust".into()));
    }

    #[test]
    fn detect_language_none() {
        assert_eq!(detect_code_fence_language("```"), None);
    }

    #[test]
    fn detect_language_with_spaces() {
        assert_eq!(detect_code_fence_language("  ```python  "), Some("python".into()));
    }

    #[test]
    fn language_display() {
        assert_eq!(language_display_name("rs"), "Rust");
        assert_eq!(language_display_name("py"), "Python");
        assert_eq!(language_display_name("unknown"), "unknown");
    }

    #[test]
    fn extract_single_code_block() {
        let src = "text\n```rust\nfn main() {}\n```\nmore";
        let blocks = extract_fenced_code_blocks(src);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, Some("rust".into()));
        assert_eq!(blocks[0].1, "fn main() {}");
    }

    #[test]
    fn extract_multiple_code_blocks() {
        let src = "```python\nprint(1)\n```\n\n```\nplain\n```\n";
        let blocks = extract_fenced_code_blocks(src);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].0, Some("python".into()));
        assert_eq!(blocks[1].0, None);
    }

    #[test]
    fn table_formatter_v2_parse_rows() {
        let rows = MarkdownTableFormatterV2::parse_rows("| A | B |\n|---|---|\n| 1 | 2 |");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec!["A", "B"]);
    }

    #[test]
    fn table_formatter_v2_column_widths() {
        let mut f = MarkdownTableFormatterV2::new();
        f.set_rows(vec![
            vec!["Name".into(), "Age".into()],
            vec!["Alice".into(), "30".into()],
        ]);
        assert_eq!(f.compute_column_widths(), vec![5, 3]);
    }

    #[test]
    fn table_formatter_v2_validate() {
        let mut f = MarkdownTableFormatterV2::new();
        f.set_rows(vec![vec!["A".into(), "B".into()], vec!["1".into(), "2".into()]]);
        assert!(f.validate_table());
    }

    #[test]
    fn table_formatter_v2_validate_fails() {
        let mut f = MarkdownTableFormatterV2::new();
        f.set_rows(vec![vec!["A".into(), "B".into()], vec!["1".into()]]);
        assert!(!f.validate_table());
    }

    #[test]
    fn table_formatter_v2_format() {
        let mut f = MarkdownTableFormatterV2::new();
        f.set_rows(vec![vec!["X".into()], vec!["Y".into()]]);
        let out = f.format_table();
        assert!(out.contains("| X |"));
        assert!(out.contains("| - |"));
    }

    #[test]
    fn link_extractor_v2_extract() {
        let links = MarkdownLinkExtractorV2::extract_links("See [docs](https://example.com) and [home](/).");
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].label, "docs");
    }

    #[test]
    fn link_extractor_v2_categorize_http() {
        assert_eq!(MarkdownLinkExtractorV2::categorize("https://x.com"), LinkCategory::Http);
    }

    #[test]
    fn link_extractor_v2_categorize_anchor() {
        assert_eq!(MarkdownLinkExtractorV2::categorize("#section"), LinkCategory::Anchor);
    }

    #[test]
    fn link_extractor_v2_categorize_relative() {
        assert_eq!(MarkdownLinkExtractorV2::categorize("./file.md"), LinkCategory::Relative);
    }

    #[test]
    fn link_extractor_v2_unique_urls() {
        let urls = MarkdownLinkExtractorV2::unique_urls("[a](x) [b](y) [c](x)");
        assert_eq!(urls, vec!["x", "y"]);
    }

    #[test]
    fn toc_generator_v2_extract_headings() {
        let headings = MarkdownTocGeneratorV2::extract_headings("# Title\n## Section\n### Sub");
        assert_eq!(headings.len(), 3);
        assert_eq!(headings[0].level, 1);
        assert_eq!(headings[1].text, "Section");
    }

    #[test]
    fn toc_generator_v2_as_markdown_bulleted() {
        let headings = MarkdownTocGeneratorV2::extract_headings("# Intro\n## Details");
        let toc = MarkdownTocGeneratorV2::toc_as_markdown(&headings, 3, false);
        assert!(toc.contains("- [Intro](#intro)"));
        assert!(toc.contains("- [Details](#details)"));
    }



    #[test]
    fn markdown_config_new() {
        let cfg = MarkdownConfigDetail::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn markdown_config_set_get() {
        let mut cfg = MarkdownConfigDetail::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn markdown_config_remove() {
        let mut cfg = MarkdownConfigDetail::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn markdown_config_keys_sorted() {
        let mut cfg = MarkdownConfigDetail::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn markdown_config_bump_version() {
        let mut cfg = MarkdownConfigDetail::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn markdown_config_clear() {
        let mut cfg = MarkdownConfigDetail::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn markdown_config_merge() {
        let mut cfg1 = MarkdownConfigDetail::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = MarkdownConfigDetail::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn markdown_config_disable() {
        let mut cfg = MarkdownConfigDetail::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn markdown_rate_tracker_empty() {
        let rt = MarkdownRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn markdown_rate_tracker_record() {
        let mut rt = MarkdownRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn markdown_rate_tracker_prune() {
        let mut rt = MarkdownRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn markdown_validator_valid() {
        let v = MarkdownValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn markdown_validator_errors() {
        let mut v = MarkdownValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn markdown_validator_clear() {
        let mut v = MarkdownValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn markdown_validator_merge() {
        let mut v1 = MarkdownValidator::new();
        v1.add_error("e1");
        let mut v2 = MarkdownValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn markdown_rate_tracker_clear() {
        let mut rt = MarkdownRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn xg_metrics_empty() {
        let m = XgMetrics::new("markdown");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xg_metrics_record_and_mean() {
        let mut m = XgMetrics::new("markdown");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xg_metrics_min_max() {
        let mut m = XgMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xg_metrics_variance_and_std() {
        let mut m = XgMetrics::new("v");
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
    fn xg_metrics_percentile() {
        let mut m = XgMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn xg_metrics_merge() {
        let mut a = XgMetrics::new("a");
        a.record(1.0);
        let mut b = XgMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn xg_metrics_reset() {
        let mut m = XgMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn xg_rate_window_empty() {
        let rw = XgRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn xg_rate_window_tick_and_rate() {
        let mut rw = XgRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn xg_lru_cache_basic() {
        let mut c = XgLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn xg_lru_cache_contains_and_keys() {
        let mut c = XgLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn xg_lru_cache_remove() {
        let mut c = XgLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn xg_metrics_sum() {
        let mut m = XgMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xg_metrics_label() {
        let m = XgMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn xg_lru_cache_clear() {
        let mut c = XgLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_23_push_and_len() {
        let mut rb = super::XbRingBuffer23::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_23_overwrite() {
        let mut rb = super::XbRingBuffer23::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_23_get_out_of_bounds() {
        let rb = super::XbRingBuffer23::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_23_drain_all() {
        let mut rb = super::XbRingBuffer23::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_23_peek_front_back() {
        let mut rb = super::XbRingBuffer23::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_23_clear() {
        let mut rb = super::XbRingBuffer23::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_23_capacity() {
        let rb = super::XbRingBuffer23::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_23_basic() {
        let h = super::xb_fnv1a_23(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_23(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_23_different_inputs() {
        let h1 = super::xb_fnv1a_23(b"abc");
        let h2 = super::xb_fnv1a_23(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_23_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_23(&data);
        let dec = super::xb_rle_decode_23(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_23_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_23(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_23(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_23_values() {
        assert!((super::xb_clamp_23(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_23(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_23(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_23_values() {
        assert!((super::xb_lerp_23(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_23(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_23(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_23_wrap_around_twice() {
        let mut rb = super::XbRingBuffer23::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 119 ----

    #[test]
    fn xc_119_pool_new_empty() {
        let pool: super::Xc119Pool<i32> = super::Xc119Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_119_pool_release_acquire() {
        let mut pool = super::Xc119Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_119_pool_acquire_empty() {
        let mut pool: super::Xc119Pool<i32> = super::Xc119Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_119_pool_full() {
        let mut pool = super::Xc119Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_119_pool_drain() {
        let mut pool = super::Xc119Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_119_pool_stats() {
        let mut pool = super::Xc119Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_119_pool_clear() {
        let mut pool = super::Xc119Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_119_pool_shrink() {
        let mut pool = super::Xc119Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_119_pool_default() {
        let pool: super::Xc119Pool<String> = super::Xc119Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_119_pool_extend() {
        let mut pool = super::Xc119Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_119_pool_retain() {
        let mut pool = super::Xc119Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_119_scheduler_round_robin() {
        let mut sched = super::Xc119Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_119_scheduler_empty() {
        let mut sched = super::Xc119Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_119_scheduler_reset() {
        let mut sched = super::Xc119Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_119_scheduler_add_remove() {
        let mut sched = super::Xc119Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_119_scheduler_targets() {
        let sched = super::Xc119Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_119_hash_empty() {
        assert_eq!(super::xc_119_hash(b""), 5381);
    }

    #[test]
    fn xc_119_hash_data() {
        let h = super::xc_119_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_119_hash(b"hello"), h);
    }

    #[test]
    fn xc_119_reverse_str() {
        assert_eq!(super::xc_119_reverse("abc"), "cba");
        assert_eq!(super::xc_119_reverse(""), "");
    }


    #[test]
    fn xe_35_pipeline_empty() {
        let p = super::Xe35Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_35_pipeline_parse_stage() {
        let p = super::Xe35Pipeline::new()
            .add_parse(super::xe_35_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_35_pipeline_transform_double() {
        let p = super::Xe35Pipeline::new()
            .add_transform(super::xe_35_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_35_pipeline_validate_reverse() {
        let p = super::Xe35Pipeline::new()
            .add_validate(super::xe_35_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_35_pipeline_emit_filter() {
        let p = super::Xe35Pipeline::new()
            .add_emit(super::xe_35_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_35_pipeline_multi_stage() {
        let p = super::Xe35Pipeline::new()
            .add_parse(super::xe_35_pipeline_identity)
            .add_transform(super::xe_35_pipeline_double)
            .add_validate(super::xe_35_pipeline_reverse)
            .add_emit(super::xe_35_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_35_pipeline_error_propagation() {
        let p = super::Xe35Pipeline::new()
            .add_parse(super::xe_35_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe35Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_35_pipeline_compose() {
        let p1 = super::Xe35Pipeline::new()
            .add_parse(super::xe_35_pipeline_identity);
        let p2 = super::Xe35Pipeline::new()
            .add_transform(super::xe_35_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_35_pipeline_error_display() {
        let e = super::Xe35PipelineError {
            stage: super::Xe35Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_35_cache_put_get() {
        let mut c = super::Xe35Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_35_cache_miss() {
        let mut c: super::Xe35Cache<&str, i32> = super::Xe35Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_35_cache_ttl_expiry() {
        let mut c = super::Xe35Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_35_cache_evict() {
        let mut c = super::Xe35Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_35_cache_capacity() {
        let mut c = super::Xe35Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_35_cache_stats() {
        let mut c = super::Xe35Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_35_cache_clear() {
        let mut c = super::Xe35Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #121 --

    #[test]
    fn xf121_trie_insert_search() {
        let mut t = Xf121Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf121_trie_starts_with() {
        let mut t = Xf121Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf121_trie_remove() {
        let mut t = Xf121Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf121_trie_word_count() {
        let mut t = Xf121Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf121_trie_longest_prefix() {
        let mut t = Xf121Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf121_trie_all_words() {
        let mut t = Xf121Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf121_trie_autocomplete() {
        let mut t = Xf121Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf121_trie_empty_search() {
        let t = Xf121Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf121_bloom_add_contains() {
        let mut bf = Xf121BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf121_bloom_probably_absent() {
        let bf = Xf121BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf121_bloom_false_positive_rate() {
        let mut bf = Xf121BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf121_bloom_clear() {
        let mut bf = Xf121BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf121_bloom_union() {
        let mut a = Xf121BloomFilter::xf_new(512, 2);
        let mut b = Xf121BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf121_bloom_intersection_estimate() {
        let mut a = Xf121BloomFilter::xf_new(512, 2);
        let mut b = Xf121BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf121_bloom_union_size_mismatch() {
        let a = Xf121BloomFilter::xf_new(256, 2);
        let b = Xf121BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh118_skip_insert_contains() {
        let mut sl = super::Xh118SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh118_skip_remove() {
        let mut sl = super::Xh118SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh118_skip_len() {
        let mut sl = super::Xh118SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh118_skip_range_query() {
        let mut sl = super::Xh118SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh118_skip_floor_ceiling() {
        let mut sl = super::Xh118SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh118_skip_rank() {
        let mut sl = super::Xh118SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh118_skip_empty() {
        let sl = super::Xh118SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh118_skip_duplicates() {
        let mut sl = super::Xh118SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh118_bitset_set_test() {
        let mut bs = super::Xh118BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh118_bitset_clear_count() {
        let mut bs = super::Xh118BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh118_bitset_and_or_xor() {
        let mut a = super::Xh118BitSet::xh_new(128);
        let mut b = super::Xh118BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh118_bitset_iter_ones() {
        let mut bs = super::Xh118BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh118_bitset_first_last() {
        let mut bs = super::Xh118BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh118_bitset_empty() {
        let bs = super::Xh118BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi118_deque_push_pop_back() {
        let mut dq = super::Xi118Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi118_deque_push_pop_front() {
        let mut dq = super::Xi118Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi118_deque_mixed_ops() {
        let mut dq = super::Xi118Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi118_deque_get_and_split() {
        let mut dq = super::Xi118Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi118_deque_rotate_left() {
        let mut dq = super::Xi118Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi118_deque_rotate_right() {
        let mut dq = super::Xi118Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi118_deque_grow() {
        let mut dq = super::Xi118Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi118_deque_empty() {
        let dq = super::Xi118Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi118_interval_tree_insert_query() {
        let mut tree = super::Xi118IntervalTree::xi_new();
        tree.xi_insert(super::Xi118Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi118Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi118Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi118_interval_tree_overlap() {
        let mut tree = super::Xi118IntervalTree::xi_new();
        tree.xi_insert(super::Xi118Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi118Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi118Interval::xi_new(12, 20));
        let q = super::Xi118Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi118_interval_tree_remove() {
        let mut tree = super::Xi118IntervalTree::xi_new();
        tree.xi_insert(super::Xi118Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi118Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi118_interval_tree_gaps() {
        let mut tree = super::Xi118IntervalTree::xi_new();
        tree.xi_insert(super::Xi118Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi118Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi118Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi118Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi118Interval::xi_new(8, 10));
    }

    #[test]
    fn xi118_interval_tree_merge() {
        let mut tree = super::Xi118IntervalTree::xi_new();
        tree.xi_insert(super::Xi118Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi118Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi118Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi118Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi118Interval::xi_new(10, 15));
    }

    #[test]
    fn xi118_interval_tree_all() {
        let mut tree = super::Xi118IntervalTree::xi_new();
        tree.xi_insert(super::Xi118Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi118Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi118_interval_tree_empty() {
        let tree = super::Xi118IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi118_interval_tree_contains_point() {
        let iv = super::Xi118Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 118) ---

    #[test]
    fn xj_118_uf_make_and_find() {
        let mut uf = super::Xj118UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_118_uf_union_connected() {
        let mut uf = super::Xj118UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_118_uf_component_count() {
        let mut uf = super::Xj118UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_118_uf_component_size() {
        let mut uf = super::Xj118UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_118_uf_largest_component() {
        let mut uf = super::Xj118UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_118_uf_many_elements() {
        let mut uf = super::Xj118UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_118_uf_separate_components() {
        let mut uf = super::Xj118UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_118_uf_path_compression() {
        let mut uf = super::Xj118UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_118_bt_insert_get() {
        let mut bt = super::Xj118BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_118_bt_contains_len() {
        let mut bt = super::Xj118BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_118_bt_replace() {
        let mut bt = super::Xj118BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_118_bt_remove() {
        let mut bt = super::Xj118BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_118_bt_keys_values() {
        let mut bt = super::Xj118BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_118_bt_range() {
        let mut bt = super::Xj118BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_118_bt_min_max() {
        let mut bt = super::Xj118BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_118_bt_many_inserts() {
        let mut bt = super::Xj118BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_118 segment tree tests ---

    #[test]
    fn xk_118_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk118SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_118_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk118SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_118_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk118SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_118_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk118SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_118_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk118SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_118_st_single_element() {
        let data = vec![42];
        let st = super::Xk118SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_118_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk118SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_118_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk118SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_118 disjoint intervals tests ---

    #[test]
    fn xk_118_di_add_and_count() {
        let mut di = super::Xk118DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_118_di_merge_overlap() {
        let mut di = super::Xk118DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_118_di_contains() {
        let mut di = super::Xk118DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_118_di_remove() {
        let mut di = super::Xk118DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_118_di_covered_length() {
        let mut di = super::Xk118DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_118_di_gaps() {
        let mut di = super::Xk118DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_118_di_merge_adjacent() {
        let mut di = super::Xk118DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_118_di_empty() {
        let di = super::Xk118DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_118_rope_new_empty() {
        let rope = super::Xl118Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_118_rope_from_str() {
        let rope = super::Xl118Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_118_rope_insert_at() {
        let mut rope = super::Xl118Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_118_rope_delete_range() {
        let mut rope = super::Xl118Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_118_rope_char_at() {
        let rope = super::Xl118Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_118_rope_split_concat() {
        let rope = super::Xl118Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_118_rope_line_count() {
        let rope = super::Xl118Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_118_rope_line_at() {
        let rope = super::Xl118Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_118_sa_build_and_search() {
        let sa = super::Xl118SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_118_sa_count() {
        let sa = super::Xl118SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_118_sa_longest_repeated() {
        let sa = super::Xl118SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_118_sa_all_positions() {
        let sa = super::Xl118SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_118_sa_len() {
        let sa = super::Xl118SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_118_sa_empty() {
        let sa = super::Xl118SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_118_rope_slice() {
        let rope = super::Xl118Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_118_sa_search_start() {
        let sa = super::Xl118SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_118_sparse_set_get() {
        let mut m = super::Xm118MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_118_sparse_row_col() {
        let mut m = super::Xm118MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_118_sparse_transpose() {
        let mut m = super::Xm118MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_118_sparse_multiply_vec() {
        let mut m = super::Xm118MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_118_sparse_nnz_density() {
        let mut m = super::Xm118MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_118_sparse_clear() {
        let mut m = super::Xm118MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_118_sparse_overwrite_zero() {
        let mut m = super::Xm118MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_118_tokenizer_basic() {
        let t = super::Xm118Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_118_tokenizer_count() {
        let t = super::Xm118Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_118_tokenizer_unique() {
        let t = super::Xm118Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_118_tokenizer_frequency() {
        let t = super::Xm118Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_118_tokenizer_delimiter() {
        let t = super::Xm118Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_118_tokenizer_whitespace() {
        let t = super::Xm118Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_118_tokenizer_empty() {
        let t = super::Xm118Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 118 ----

    #[test]
    fn xn_118_fenwick_prefix_sum() {
        let mut ft = super::Xn118Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_118_fenwick_range_sum() {
        let mut ft = super::Xn118Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_118_fenwick_point_query() {
        let mut ft = super::Xn118Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_118_fenwick_len() {
        let ft = super::Xn118Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_118_fenwick_multiple_updates() {
        let mut ft = super::Xn118Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_118_fenwick_single_element() {
        let mut ft = super::Xn118Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_118_fenwick_find_kth() {
        let mut ft = super::Xn118Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_118_fenwick_negative_delta() {
        let mut ft = super::Xn118Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 118 ----

    #[test]
    fn xn_118_avl_insert_get() {
        let mut m = super::Xn118AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_118_avl_remove() {
        let mut m = super::Xn118AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_118_avl_in_order() {
        let mut m = super::Xn118AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_118_avl_min_max() {
        let mut m = super::Xn118AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_118_avl_floor_ceiling() {
        let mut m = super::Xn118AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_118_avl_height_balanced() {
        let mut m = super::Xn118AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_118_avl_overwrite() {
        let mut m = super::Xn118AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_118_avl_empty() {
        let m: super::Xn118AVL<i32, i32> = super::Xn118AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo118RedBlack tests ---

    #[test]
    fn xo_118_rb_insert_and_get() {
        let mut tree = super::Xo118RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_118_rb_len_and_empty() {
        let mut tree = super::Xo118RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_118_rb_min_max() {
        let mut tree = super::Xo118RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_118_rb_contains() {
        let mut tree = super::Xo118RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_118_rb_remove() {
        let mut tree = super::Xo118RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_118_rb_in_order() {
        let mut tree = super::Xo118RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_118_rb_black_height() {
        let mut tree = super::Xo118RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_118_rb_overwrite() {
        let mut tree = super::Xo118RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo118ConsistentHash tests ---

    #[test]
    fn xo_118_ch_add_and_count() {
        let mut ring = super::Xo118ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_118_ch_remove_node() {
        let mut ring = super::Xo118ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_118_ch_get_node() {
        let mut ring = super::Xo118ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_118_ch_empty_ring() {
        let ring = super::Xo118ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_118_ch_distribution() {
        let mut ring = super::Xo118ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_118_ch_rebalance() {
        let mut ring = super::Xo118ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_118_ch_virtual_nodes() {
        let mut ring = super::Xo118ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_118_ch_consistent_lookup() {
        let mut ring = super::Xo118ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_118_splay_insert_get() {
        let mut t = super::Xp118SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_118_splay_remove() {
        let mut t = super::Xp118SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_118_splay_count_increases() {
        let mut t = super::Xp118SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_118_splay_depth() {
        let mut t = super::Xp118SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_118_splay_len_empty() {
        let t = super::Xp118SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_118_splay_min_max() {
        let mut t = super::Xp118SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_118_splay_overwrite() {
        let mut t = super::Xp118SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_118_splay_remove_missing() {
        let mut t = super::Xp118SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_118 treap tests ----
    #[test]
    fn xq_118_treap_empty() {
        let t = super::Xq118Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_118_treap_insert_get() {
        let mut t = super::Xq118Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_118_treap_overwrite() {
        let mut t = super::Xq118Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_118_treap_remove() {
        let mut t = super::Xq118Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_118_treap_min_max() {
        let mut t = super::Xq118Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_118_treap_rank() {
        let mut t = super::Xq118Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_118_treap_kth() {
        let mut t = super::Xq118Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_118_treap_in_order() {
        let mut t = super::Xq118Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_118 VEB tree tests ----
    #[test]
    fn xq_118_veb_empty() {
        let v = super::Xq118VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_118_veb_insert_contains() {
        let mut v = super::Xq118VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_118_veb_min_max() {
        let mut v = super::Xq118VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_118_veb_delete() {
        let mut v = super::Xq118VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_118_veb_successor() {
        let mut v = super::Xq118VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_118_veb_predecessor() {
        let mut v = super::Xq118VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_118_veb_count() {
        let mut v = super::Xq118VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_118_veb_duplicate_insert() {
        let mut v = super::Xq118VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_118_kdtree_empty() {
        let tree = super::Xr118KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_118_kdtree_insert_one() {
        let mut tree = super::Xr118KDTree::xr_new();
        tree.xr_insert(super::Xr118KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_118_kdtree_insert_multiple() {
        let mut tree = super::Xr118KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr118KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_118_kdtree_nearest_neighbor() {
        let mut tree = super::Xr118KDTree::xr_new();
        tree.xr_insert(super::Xr118KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr118KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr118KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_118_kdtree_nn_empty() {
        let tree = super::Xr118KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr118KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_118_kdtree_range_search() {
        let mut tree = super::Xr118KDTree::xr_new();
        tree.xr_insert(super::Xr118KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr118KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr118KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_118_kdtree_range_empty() {
        let mut tree = super::Xr118KDTree::xr_new();
        tree.xr_insert(super::Xr118KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_118_kdtree_all_points() {
        let mut tree = super::Xr118KDTree::xr_new();
        tree.xr_insert(super::Xr118KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr118KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_118_kdtree_depth() {
        let mut tree = super::Xr118KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr118KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_118_kdtree_bounding_box() {
        let mut tree = super::Xr118KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr118KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr118KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_118_persistent_array_new() {
        let arr = super::Xs118PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_118_persistent_array_push() {
        let mut arr = super::Xs118PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_118_persistent_array_set() {
        let mut arr = super::Xs118PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_118_persistent_array_diff() {
        let mut arr = super::Xs118PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_118_persistent_array_rollback() {
        let mut arr = super::Xs118PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_118_persistent_array_history() {
        let mut arr = super::Xs118PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_118_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs118PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_118_persistent_array_from_vec() {
        let arr = super::Xs118PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_118_concurrent_queue_new() {
        let q = super::Xs118ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_118_concurrent_queue_push_pop() {
        let mut q = super::Xs118ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_118_concurrent_queue_full() {
        let mut q = super::Xs118ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_118_concurrent_queue_drain() {
        let mut q = super::Xs118ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_118_concurrent_queue_try_pop() {
        let mut q = super::Xs118ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_118_concurrent_queue_clear() {
        let mut q = super::Xs118ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_118_range_map_new() {
        let rm = super::Xs118RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_118_range_map_insert_get() {
        let mut rm = super::Xs118RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_118_range_map_overlap() {
        let mut rm = super::Xs118RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_118_range_map_remove() {
        let mut rm = super::Xs118RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_118_range_map_gaps() {
        let mut rm = super::Xs118RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_118_range_map_coverage() {
        let mut rm = super::Xs118RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_118_range_map_contains() {
        let mut rm = super::Xs118RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_118_range_map_clear() {
        let mut rm = super::Xs118RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_118_circular_buffer_new() {
        let buf = super::Xs118CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_118_circular_buffer_push_pop() {
        let mut buf = super::Xs118CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_118_circular_buffer_overwrite() {
        let mut buf = super::Xs118CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_118_circular_buffer_peek() {
        let mut buf = super::Xs118CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_118_circular_buffer_is_full() {
        let mut buf = super::Xs118CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_118_circular_buffer_iter() {
        let mut buf = super::Xs118CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_118_circular_buffer_clear() {
        let mut buf = super::Xs118CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_118_circular_buffer_to_vec() {
        let mut buf = super::Xs118CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }


    // --- xt_ Fibonacci Heap tests ---

    #[test]
    fn xt_fib_heap_new() {
        let h = super::XtFibonacciHeap::<i32, &str>::xt_new();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_len(), 0);
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_insert_find_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(5, "five");
        h.xt_insert(3, "three");
        h.xt_insert(7, "seven");
        assert_eq!(h.xt_len(), 3);
        assert_eq!(h.xt_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xt_fib_heap_extract_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "ten");
        h.xt_insert(2, "two");
        h.xt_insert(8, "eight");
        h.xt_insert(1, "one");
        assert_eq!(h.xt_extract_min(), Some((1, "one")));
        assert_eq!(h.xt_extract_min(), Some((2, "two")));
        assert_eq!(h.xt_len(), 2);
    }

    #[test]
    fn xt_fib_heap_extract_all_sorted() {
        let mut h = super::XtFibonacciHeap::xt_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xt_insert(v, v * 10);
        }
        let sorted = h.xt_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xt_fib_heap_decrease_key() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "a");
        let idx = h.xt_insert(20, "b");
        h.xt_insert(15, "c");
        h.xt_decrease_key(idx, 5);
        assert_eq!(h.xt_find_min(), Some((&5, &"b")));
    }

    #[test]
    fn xt_fib_heap_merge() {
        let mut h1 = super::XtFibonacciHeap::xt_new();
        h1.xt_insert(3, "three");
        h1.xt_insert(7, "seven");
        let mut h2 = super::XtFibonacciHeap::xt_new();
        h2.xt_insert(1, "one");
        h2.xt_insert(5, "five");
        h1.xt_merge(&mut h2);
        assert_eq!(h1.xt_len(), 4);
        assert_eq!(h1.xt_find_min(), Some((&1, &"one")));
        assert!(h2.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_clear() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "a");
        h.xt_insert(2, "b");
        h.xt_clear();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_single_element() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(42, "answer");
        assert_eq!(h.xt_extract_min(), Some((42, "answer")));
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_display() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "one");
        let s = format!("{}", h);
        assert!(s.contains("FibHeap"));
    }

    #[test]
    fn xt_fib_heap_default() {
        let h = super::XtFibonacciHeap::<i32, i32>::default();
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_node_display() {
        let n = super::XtFibNode::xt_new(10, "ten");
        let s = format!("{}", n);
        assert!(s.contains("FibNode"));
    }

    // --- xt_ Doubly-Linked List tests ---

    #[test]
    fn xt_dll_new() {
        let dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert!(dll.xt_is_empty());
        assert_eq!(dll.xt_len(), 0);
    }

    #[test]
    fn xt_dll_push_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_front(1);
        dll.xt_push_front(2);
        dll.xt_push_front(3);
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_push_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_pop_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_front(), Some(10));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_pop_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_back(), Some(20));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_insert_after() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(3);
        dll.xt_insert_after(a, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_insert_before() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let b = dll.xt_push_back(3);
        dll.xt_insert_before(b, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_remove_middle() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let mid = dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_remove(mid);
        assert_eq!(dll.xt_to_vec(), vec![1, 3]);
    }

    #[test]
    fn xt_dll_peek() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_peek_front(), Some(&10));
        assert_eq!(dll.xt_peek_back(), Some(&20));
    }

    #[test]
    fn xt_dll_get() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let idx = dll.xt_push_back(42);
        assert_eq!(dll.xt_get(idx), Some(&42));
        assert_eq!(dll.xt_get(999), None);
    }

    #[test]
    fn xt_dll_iter_backward() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        let rev: Vec<&i32> = dll.xt_iter_backward();
        assert_eq!(rev, vec![&3, &2, &1]);
    }

    #[test]
    fn xt_dll_cursor_navigation() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        dll.xt_push_back(30);
        let c = dll.xt_head_cursor().unwrap();
        assert_eq!(dll.xt_get(c), Some(&10));
        let c2 = dll.xt_cursor_next(c).unwrap();
        assert_eq!(dll.xt_get(c2), Some(&20));
        let c3 = dll.xt_cursor_next(c2).unwrap();
        assert_eq!(dll.xt_get(c3), Some(&30));
        assert_eq!(dll.xt_cursor_next(c3), None);
        let c2b = dll.xt_cursor_prev(c3).unwrap();
        assert_eq!(dll.xt_get(c2b), Some(&20));
    }

    #[test]
    fn xt_dll_reverse() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_reverse();
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_clear() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_clear();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_default() {
        let dll = super::XtDoublyLinkedList::<i32>::default();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_display() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let s = format!("{}", dll);
        assert!(s.contains("DLL"));
    }

    #[test]
    fn xt_dll_reuse_freed_slots() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_remove(a);
        let c = dll.xt_push_back(3);
        assert_eq!(c, a);
        assert_eq!(dll.xt_to_vec(), vec![2, 3]);
    }

    #[test]
    fn xt_dll_tail_cursor() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        let tc = dll.xt_tail_cursor().unwrap();
        assert_eq!(dll.xt_get(tc), Some(&2));
    }

    #[test]
    fn xt_dll_empty_operations() {
        let mut dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert_eq!(dll.xt_pop_front(), None);
        assert_eq!(dll.xt_pop_back(), None);
        assert_eq!(dll.xt_peek_front(), None);
        assert_eq!(dll.xt_peek_back(), None);
        assert_eq!(dll.xt_head_cursor(), None);
        assert_eq!(dll.xt_tail_cursor(), None);
    }


    // --- xu_ Binomial Heap tests ---

    #[test]
    fn xu_bin_heap_new() {
        let h = super::XuBinomialHeap::<i32, &str>::xu_new();
        assert!(h.xu_is_empty());
        assert_eq!(h.xu_len(), 0);
    }

    #[test]
    fn xu_bin_heap_insert_find_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(5, "five");
        h.xu_insert(3, "three");
        h.xu_insert(7, "seven");
        assert_eq!(h.xu_len(), 3);
        assert_eq!(h.xu_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xu_bin_heap_extract_min() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(10, "a");
        h.xu_insert(2, "b");
        h.xu_insert(8, "c");
        h.xu_insert(1, "d");
        assert_eq!(h.xu_extract_min(), Some((1, "d")));
        assert_eq!(h.xu_extract_min(), Some((2, "b")));
    }

    #[test]
    fn xu_bin_heap_sorted_drain() {
        let mut h = super::XuBinomialHeap::xu_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xu_insert(v, v * 10);
        }
        let sorted = h.xu_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xu_bin_heap_merge() {
        let mut h1 = super::XuBinomialHeap::xu_new();
        h1.xu_insert(3, "a");
        h1.xu_insert(7, "b");
        let mut h2 = super::XuBinomialHeap::xu_new();
        h2.xu_insert(1, "c");
        h2.xu_insert(5, "d");
        h1.xu_merge(&mut h2);
        assert_eq!(h1.xu_len(), 4);
        assert_eq!(h1.xu_find_min(), Some((&1, &"c")));
    }

    #[test]
    fn xu_bin_heap_clear() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "a");
        h.xu_clear();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_heap_display() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(1, "x");
        assert!(format!("{}", h).contains("BinHeap"));
    }

    #[test]
    fn xu_bin_heap_default() {
        let h = super::XuBinomialHeap::<i32, i32>::default();
        assert!(h.xu_is_empty());
    }

    #[test]
    fn xu_bin_node_display() {
        let n = super::XuBinomialNode::xu_new(5, "v");
        assert!(format!("{}", n).contains("BinNode"));
    }

    #[test]
    fn xu_bin_heap_single() {
        let mut h = super::XuBinomialHeap::xu_new();
        h.xu_insert(42, "answer");
        assert_eq!(h.xu_extract_min(), Some((42, "answer")));
        assert!(h.xu_is_empty());
    }

    // --- xu_ Disjoint Sparse Table tests ---

    #[test]
    fn xu_dst_build() {
        let data = vec![1, 2, 3, 4, 5];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_len(), 5);
        assert!(!dst.xu_is_empty());
    }

    #[test]
    fn xu_dst_single_element_query() {
        let data = vec![10, 20, 30];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_query(0, 0), 10);
        assert_eq!(dst.xu_query(1, 1), 20);
        assert_eq!(dst.xu_query(2, 2), 30);
    }

    #[test]
    fn xu_dst_get() {
        let data = vec![5, 10, 15];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert_eq!(dst.xu_get(0), Some(&5));
        assert_eq!(dst.xu_get(2), Some(&15));
        assert_eq!(dst.xu_get(10), None);
    }

    #[test]
    fn xu_dst_empty() {
        let dst = super::XuDisjointSparseTable::<i32>::xu_build(&[]);
        assert!(dst.xu_is_empty());
        assert_eq!(dst.xu_len(), 0);
    }

    #[test]
    fn xu_dst_display() {
        let data = vec![1, 2, 3];
        let dst = super::XuDisjointSparseTable::xu_build(&data);
        assert!(format!("{}", dst).contains("DST"));
    }

    // --- xu_ Monotonic Stack tests ---

    #[test]
    fn xu_mono_stack_increasing() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        assert!(s.xu_is_empty());
        let popped = s.xu_push(3);
        assert!(popped.is_empty());
        let popped = s.xu_push(5);
        assert!(popped.is_empty());
        let popped = s.xu_push(2);
        assert_eq!(popped, vec![5, 3]);
        assert_eq!(s.xu_as_slice(), &[2]);
    }

    #[test]
    fn xu_mono_stack_decreasing() {
        let mut s = super::XuMonotonicStack::xu_decreasing();
        s.xu_push(2);
        s.xu_push(1);
        let popped = s.xu_push(5);
        assert_eq!(popped, vec![1, 2]);
        assert_eq!(s.xu_as_slice(), &[5]);
    }

    #[test]
    fn xu_mono_stack_peek_pop() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(3);
        s.xu_push(5);
        assert_eq!(s.xu_peek(), Some(&5));
        assert_eq!(s.xu_pop(), Some(5));
        assert_eq!(s.xu_len(), 2);
    }

    #[test]
    fn xu_mono_stack_clear() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        s.xu_push(2);
        s.xu_clear();
        assert!(s.xu_is_empty());
    }

    #[test]
    fn xu_mono_stack_display() {
        let mut s = super::XuMonotonicStack::xu_increasing();
        s.xu_push(1);
        assert!(format!("{}", s).contains("MonoStack"));
    }


    // --- xv_ Cartesian Tree tests ---

    #[test]
    fn xv_cart_tree_new() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_cart_tree_insert_contains() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        t.xv_insert(3, 2);
        t.xv_insert(7, 3);
        assert!(t.xv_contains(&5));
        assert!(t.xv_contains(&3));
        assert!(t.xv_contains(&7));
        assert!(!t.xv_contains(&4));
        assert_eq!(t.xv_len(), 3);
    }

    #[test]
    fn xv_cart_tree_inorder() {
        let mut t = super::XvCartesianTree::xv_new();
        for (k, p) in [(5, 3), (3, 1), (7, 2), (1, 5), (9, 4)] {
            t.xv_insert(k, p);
        }
        let keys = t.xv_inorder();
        assert_eq!(keys, vec![1, 3, 5, 7, 9]);
    }

    #[test]
    fn xv_cart_tree_min_priority() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 10);
        t.xv_insert(3, 2);
        t.xv_insert(7, 5);
        assert_eq!(t.xv_min_priority(), Some(&2));
    }

    #[test]
    fn xv_cart_tree_from_pairs() {
        let t = super::XvCartesianTree::xv_from_pairs(&[(3, 1), (1, 3), (5, 2)]);
        assert_eq!(t.xv_len(), 3);
        assert!(t.xv_contains(&1));
    }

    #[test]
    fn xv_cart_tree_height() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(5, 1);
        assert!(t.xv_height() >= 1);
    }

    #[test]
    fn xv_cart_tree_clear() {
        let mut t = super::XvCartesianTree::xv_new();
        t.xv_insert(1, 1);
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_tree_display() {
        let t = super::XvCartesianTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("CartTree"));
    }

    #[test]
    fn xv_cart_tree_default() {
        let t = super::XvCartesianTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_cart_node_display() {
        let n = super::XvCartesianNode { xv_key: 1, xv_priority: 2, xv_left: None, xv_right: None };
        assert!(format!("{}", n).contains("CartNode"));
    }

    // --- xv_ Weight-Balanced Tree tests ---

    #[test]
    fn xv_wb_tree_new() {
        let t = super::XvWeightBalancedTree::<i32, &str>::xv_new();
        assert!(t.xv_is_empty());
        assert_eq!(t.xv_len(), 0);
    }

    #[test]
    fn xv_wb_tree_insert_get() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "five");
        t.xv_insert(3, "three");
        t.xv_insert(7, "seven");
        assert_eq!(t.xv_get(&5), Some(&"five"));
        assert_eq!(t.xv_get(&3), Some(&"three"));
        assert_eq!(t.xv_get(&7), Some(&"seven"));
        assert_eq!(t.xv_get(&4), None);
    }

    #[test]
    fn xv_wb_tree_contains() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(10, "a");
        assert!(t.xv_contains(&10));
        assert!(!t.xv_contains(&20));
    }

    #[test]
    fn xv_wb_tree_keys_sorted() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            t.xv_insert(k, k * 10);
        }
        assert_eq!(t.xv_keys(), vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xv_wb_tree_replace_value() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(5, "old");
        t.xv_insert(5, "new");
        assert_eq!(t.xv_get(&5), Some(&"new"));
        assert_eq!(t.xv_len(), 1);
    }

    #[test]
    fn xv_wb_tree_height() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        for k in 1..=15 {
            t.xv_insert(k, k);
        }
        assert!(t.xv_height() <= 20);
    }

    #[test]
    fn xv_wb_tree_clear() {
        let mut t = super::XvWeightBalancedTree::xv_new();
        t.xv_insert(1, "a");
        t.xv_clear();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_tree_display() {
        let t = super::XvWeightBalancedTree::<i32, i32>::xv_new();
        assert!(format!("{}", t).contains("WBTree"));
    }

    #[test]
    fn xv_wb_tree_default() {
        let t = super::XvWeightBalancedTree::<i32, i32>::default();
        assert!(t.xv_is_empty());
    }

    #[test]
    fn xv_wb_node_display() {
        let n = super::XvWBNode { xv_key: 1, xv_value: "a", xv_left: None, xv_right: None, xv_weight: 2 };
        assert!(format!("{}", n).contains("WBNode"));
    }

}
