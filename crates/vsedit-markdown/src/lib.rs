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
}
