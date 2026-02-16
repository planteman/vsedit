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
// Tests
// ---------------------------------------------------------------------------

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
}
