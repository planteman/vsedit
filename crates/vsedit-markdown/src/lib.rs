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
}
