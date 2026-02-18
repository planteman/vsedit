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

}
