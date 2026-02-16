//! Hover tooltip contribution.

/// Markdown-formatted string for hover content.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkdownString {
    pub value: String,
    pub is_trusted: bool,
}

impl MarkdownString {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            is_trusted: false,
        }
    }

    pub fn trusted(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            is_trusted: true,
        }
    }

    /// Create a markdown string containing only a fenced code block.
    pub fn code(code: &str, language: &str) -> Self {
        Self {
            value: format!("```{language}\n{code}\n```"),
            is_trusted: false,
        }
    }

    /// Append raw text to the markdown value.
    pub fn append(&mut self, text: &str) {
        self.value.push_str(text);
    }

    /// Append a fenced code block to the markdown value.
    pub fn append_codeblock(&mut self, code: &str, language: &str) {
        self.value.push_str(&format!("\n```{language}\n{code}\n```"));
    }

    /// Returns `true` if the markdown value is empty.
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
}

impl std::fmt::Display for MarkdownString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

/// A range in a document where a hover applies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoverRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl HoverRange {
    /// Create a range spanning a single word on one line.
    pub fn from_word(line: u32, start_col: u32, end_col: u32) -> Self {
        Self {
            start_line: line,
            start_col,
            end_line: line,
            end_col,
        }
    }

    /// Returns `true` if the given position is inside this range (inclusive).
    pub fn contains_position(&self, line: u32, col: u32) -> bool {
        if line < self.start_line || line > self.end_line {
            return false;
        }
        if line == self.start_line && col < self.start_col {
            return false;
        }
        if line == self.end_line && col > self.end_col {
            return false;
        }
        true
    }

    /// Returns `true` if the range spans only a single line.
    pub fn is_single_line(&self) -> bool {
        self.start_line == self.end_line
    }
}

/// A hover tooltip with markdown contents and an optional range.
#[derive(Debug, Clone, PartialEq)]
pub struct Hover {
    pub contents: Vec<MarkdownString>,
    pub range: Option<HoverRange>,
}

impl Hover {
    pub fn new(contents: Vec<MarkdownString>) -> Self {
        Self {
            contents,
            range: None,
        }
    }

    pub fn with_range(mut self, start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        self.range = Some(HoverRange {
            start_line,
            start_col,
            end_line,
            end_col,
        });
        self
    }

    /// Builder method to add a content item to the hover.
    pub fn with_contents(mut self, content: MarkdownString) -> Self {
        self.contents.push(content);
        self
    }

    /// Returns `true` if the hover has no contents.
    pub fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }
}

/// Trait for types that can provide hover information.
pub trait HoverProvider {
    fn provide_hover(&self, uri: &str, line: u32, col: u32) -> Option<Hover>;
}

/// Merge multiple hovers into a single hover, concatenating contents.
/// Uses the range from the first hover that has one.
pub fn merge_hovers(hovers: Vec<Hover>) -> Hover {
    let mut contents = Vec::new();
    let mut range = None;
    for hover in hovers {
        if range.is_none() {
            range = hover.range;
        }
        contents.extend(hover.contents);
    }
    Hover { contents, range }
}

/// Filter hovers to only those whose range contains the given position.
/// Hovers with no range are always included.
pub fn filter_hovers(hovers: Vec<Hover>, line: u32, col: u32) -> Vec<Hover> {
    hovers
        .into_iter()
        .filter(|h| match &h.range {
            Some(r) => r.contains_position(line, col),
            None => true,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_new_and_with_range() {
        let hover = Hover::new(vec![MarkdownString::new("hello")])
            .with_range(1, 0, 1, 5);
        assert_eq!(hover.contents.len(), 1);
        assert_eq!(hover.contents[0].value, "hello");
        let r = hover.range.unwrap();
        assert_eq!((r.start_line, r.start_col, r.end_line, r.end_col), (1, 0, 1, 5));
    }

    #[test]
    fn merge_hovers_combines_contents_and_takes_first_range() {
        let h1 = Hover::new(vec![MarkdownString::new("a")]).with_range(0, 0, 0, 1);
        let h2 = Hover::new(vec![MarkdownString::new("b"), MarkdownString::new("c")])
            .with_range(2, 0, 2, 5);
        let merged = merge_hovers(vec![h1, h2]);
        assert_eq!(merged.contents.len(), 3);
        assert_eq!(merged.range.unwrap().start_line, 0);
    }

    #[test]
    fn merge_hovers_empty() {
        let merged = merge_hovers(vec![]);
        assert!(merged.contents.is_empty());
        assert!(merged.range.is_none());
    }

    #[test]
    fn markdown_string_trusted() {
        let ms = MarkdownString::trusted("cmd");
        assert!(ms.is_trusted);
        assert_eq!(ms.value, "cmd");
    }

    #[test]
    fn markdown_string_append_and_is_empty() {
        let mut ms = MarkdownString::new("");
        assert!(ms.is_empty());
        ms.append("hello");
        assert!(!ms.is_empty());
        assert_eq!(ms.value, "hello");
    }

    #[test]
    fn markdown_string_append_codeblock() {
        let mut ms = MarkdownString::new("docs");
        ms.append_codeblock("let x = 1;", "rust");
        assert!(ms.value.contains("```rust\nlet x = 1;\n```"));
    }

    #[test]
    fn markdown_string_code_constructor() {
        let ms = MarkdownString::code("fn main() {}", "rust");
        assert_eq!(ms.value, "```rust\nfn main() {}\n```");
        assert!(!ms.is_trusted);
    }

    #[test]
    fn markdown_string_display() {
        let ms = MarkdownString::new("**bold**");
        assert_eq!(format!("{ms}"), "**bold**");
    }

    #[test]
    fn hover_range_contains_position() {
        let r = HoverRange::from_word(5, 2, 8);
        assert!(r.contains_position(5, 2));
        assert!(r.contains_position(5, 8));
        assert!(r.contains_position(5, 5));
        assert!(!r.contains_position(5, 1));
        assert!(!r.contains_position(5, 9));
        assert!(!r.contains_position(4, 5));
        assert!(!r.contains_position(6, 5));
    }

    #[test]
    fn hover_range_multiline_contains() {
        let r = HoverRange { start_line: 2, start_col: 5, end_line: 4, end_col: 3 };
        assert!(r.contains_position(3, 0));
        assert!(r.contains_position(2, 5));
        assert!(r.contains_position(4, 3));
        assert!(!r.contains_position(2, 4));
        assert!(!r.contains_position(4, 4));
    }

    #[test]
    fn hover_range_is_single_line() {
        assert!(HoverRange::from_word(1, 0, 5).is_single_line());
        let multi = HoverRange { start_line: 0, start_col: 0, end_line: 1, end_col: 0 };
        assert!(!multi.is_single_line());
    }

    #[test]
    fn hover_with_contents_builder() {
        let hover = Hover::new(vec![])
            .with_contents(MarkdownString::new("a"))
            .with_contents(MarkdownString::new("b"));
        assert_eq!(hover.contents.len(), 2);
        assert_eq!(hover.contents[0].value, "a");
    }

    #[test]
    fn hover_is_empty() {
        assert!(Hover::new(vec![]).is_empty());
        assert!(!Hover::new(vec![MarkdownString::new("x")]).is_empty());
    }

    #[test]
    fn filter_hovers_keeps_matching_and_rangeless() {
        let h1 = Hover::new(vec![MarkdownString::new("a")]).with_range(1, 0, 1, 5);
        let h2 = Hover::new(vec![MarkdownString::new("b")]).with_range(3, 0, 3, 5);
        let h3 = Hover::new(vec![MarkdownString::new("c")]); // no range
        let result = filter_hovers(vec![h1, h2, h3], 1, 3);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].contents[0].value, "a");
        assert_eq!(result[1].contents[0].value, "c");
    }
}
