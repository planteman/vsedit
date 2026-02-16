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

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur when building or validating hover data.
#[derive(Debug, Clone, PartialEq)]
pub enum HoverError {
    /// The hover range is invalid (end precedes start).
    InvalidRange {
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    },
    /// The hover has no content.
    EmptyContent,
    /// A URI was expected but was empty or invalid.
    InvalidUri(String),
}

impl std::fmt::Display for HoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HoverError::InvalidRange { start_line, start_col, end_line, end_col } => {
                write!(
                    f,
                    "invalid hover range: ({start_line}:{start_col}) to ({end_line}:{end_col})"
                )
            }
            HoverError::EmptyContent => write!(f, "hover has no content"),
            HoverError::InvalidUri(uri) => write!(f, "invalid URI: {uri}"),
        }
    }
}

impl std::error::Error for HoverError {}

// ---------------------------------------------------------------------------
// Display for HoverRange
// ---------------------------------------------------------------------------

impl std::fmt::Display for HoverRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}-{}:{}",
            self.start_line, self.start_col, self.end_line, self.end_col
        )
    }
}

// ---------------------------------------------------------------------------
// HoverRange helpers
// ---------------------------------------------------------------------------

impl HoverRange {
    /// Validate that the range is well-formed (start ≤ end).
    pub fn validate(&self) -> Result<(), HoverError> {
        let valid = self.start_line < self.end_line
            || (self.start_line == self.end_line && self.start_col <= self.end_col);
        if valid {
            Ok(())
        } else {
            Err(HoverError::InvalidRange {
                start_line: self.start_line,
                start_col: self.start_col,
                end_line: self.end_line,
                end_col: self.end_col,
            })
        }
    }

    /// Number of lines this range spans (always ≥ 1).
    pub fn line_span(&self) -> u32 {
        self.end_line - self.start_line + 1
    }

    /// Extend this range to also cover `other`, producing the smallest
    /// enclosing range.
    pub fn union(&self, other: &HoverRange) -> HoverRange {
        let (sl, sc) = if self.start_line < other.start_line
            || (self.start_line == other.start_line && self.start_col <= other.start_col)
        {
            (self.start_line, self.start_col)
        } else {
            (other.start_line, other.start_col)
        };
        let (el, ec) = if self.end_line > other.end_line
            || (self.end_line == other.end_line && self.end_col >= other.end_col)
        {
            (self.end_line, self.end_col)
        } else {
            (other.end_line, other.end_col)
        };
        HoverRange {
            start_line: sl,
            start_col: sc,
            end_line: el,
            end_col: ec,
        }
    }
}

// ---------------------------------------------------------------------------
// MarkdownString helpers
// ---------------------------------------------------------------------------

impl MarkdownString {
    /// Render a section heading followed by body text.
    pub fn section(heading: &str, body: &str) -> Self {
        Self {
            value: format!("### {heading}\n\n{body}"),
            is_trusted: false,
        }
    }

    /// Approximate word count of the underlying markdown source.
    pub fn word_count(&self) -> usize {
        self.value.split_whitespace().count()
    }

    /// Strip all markdown formatting and return plain text using `vsedit_markdown`.
    pub fn to_plain_text(&self) -> String {
        vsedit_markdown::strip_markdown(&self.value)
    }
}

// ---------------------------------------------------------------------------
// HoverBuilder – validated construction
// ---------------------------------------------------------------------------

/// Builder for constructing a [`Hover`] with validation.
#[derive(Debug, Clone)]
pub struct HoverBuilder {
    contents: Vec<MarkdownString>,
    range: Option<HoverRange>,
}

impl Default for HoverBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HoverBuilder {
    pub fn new() -> Self {
        Self {
            contents: Vec::new(),
            range: None,
        }
    }

    pub fn content(mut self, md: MarkdownString) -> Self {
        self.contents.push(md);
        self
    }

    pub fn code(mut self, code: &str, language: &str) -> Self {
        self.contents.push(MarkdownString::code(code, language));
        self
    }

    pub fn range(mut self, range: HoverRange) -> Self {
        self.range = Some(range);
        self
    }

    /// Build the hover, returning an error if the range is invalid or
    /// there is no content.
    pub fn build(self) -> Result<Hover, HoverError> {
        if self.contents.is_empty() {
            return Err(HoverError::EmptyContent);
        }
        if let Some(ref r) = self.range {
            r.validate()?;
        }
        Ok(Hover {
            contents: self.contents,
            range: self.range,
        })
    }
}

// ---------------------------------------------------------------------------
// HoverRegistry – manage multiple providers
// ---------------------------------------------------------------------------

/// Collects [`HoverProvider`] implementations and queries them all.
pub struct HoverRegistry {
    providers: Vec<Box<dyn HoverProvider>>,
}

impl HoverRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register(&mut self, provider: Box<dyn HoverProvider>) {
        self.providers.push(provider);
    }

    /// Query every registered provider and merge results.
    pub fn hover_at(&self, uri: &str, line: u32, col: u32) -> Option<Hover> {
        let hovers: Vec<Hover> = self
            .providers
            .iter()
            .filter_map(|p| p.provide_hover(uri, line, col))
            .collect();
        if hovers.is_empty() {
            None
        } else {
            Some(merge_hovers(hovers))
        }
    }

    /// Number of registered providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

impl Default for HoverRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate a URI string (non-empty and contains a colon scheme separator).
pub fn validate_uri(uri: &str) -> Result<(), HoverError> {
    if uri.is_empty() || !uri.contains(':') {
        Err(HoverError::InvalidUri(uri.to_string()))
    } else {
        Ok(())
    }
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

    // --- new tests ---

    #[test]
    fn hover_error_display_invalid_range() {
        let err = HoverError::InvalidRange {
            start_line: 5,
            start_col: 10,
            end_line: 3,
            end_col: 2,
        };
        let msg = format!("{err}");
        assert!(msg.contains("(5:10)"));
        assert!(msg.contains("(3:2)"));
    }

    #[test]
    fn hover_error_display_empty_content() {
        assert_eq!(format!("{}", HoverError::EmptyContent), "hover has no content");
    }

    #[test]
    fn hover_error_display_invalid_uri() {
        let err = HoverError::InvalidUri("bad".into());
        assert!(format!("{err}").contains("bad"));
    }

    #[test]
    fn hover_range_display() {
        let r = HoverRange::from_word(3, 5, 12);
        assert_eq!(format!("{r}"), "3:5-3:12");
    }

    #[test]
    fn hover_range_validate_ok() {
        assert!(HoverRange::from_word(1, 0, 5).validate().is_ok());
        // zero-width range is valid
        assert!(HoverRange::from_word(1, 3, 3).validate().is_ok());
    }

    #[test]
    fn hover_range_validate_err() {
        let r = HoverRange {
            start_line: 5,
            start_col: 10,
            end_line: 5,
            end_col: 2,
        };
        assert!(r.validate().is_err());
    }

    #[test]
    fn hover_range_line_span() {
        assert_eq!(HoverRange::from_word(0, 0, 5).line_span(), 1);
        let multi = HoverRange { start_line: 2, start_col: 0, end_line: 7, end_col: 0 };
        assert_eq!(multi.line_span(), 6);
    }

    #[test]
    fn hover_range_union() {
        let a = HoverRange::from_word(1, 5, 10);
        let b = HoverRange { start_line: 0, start_col: 3, end_line: 2, end_col: 8 };
        let u = a.union(&b);
        assert_eq!(u.start_line, 0);
        assert_eq!(u.start_col, 3);
        assert_eq!(u.end_line, 2);
        assert_eq!(u.end_col, 8);
    }

    #[test]
    fn markdown_string_section() {
        let ms = MarkdownString::section("Title", "Some body text.");
        assert!(ms.value.starts_with("### Title"));
        assert!(ms.value.contains("Some body text."));
    }

    #[test]
    fn markdown_string_word_count() {
        let ms = MarkdownString::new("hello world foo");
        assert_eq!(ms.word_count(), 3);
        assert_eq!(MarkdownString::new("").word_count(), 0);
    }

    #[test]
    fn markdown_string_to_plain_text() {
        let ms = MarkdownString::new("**bold** and *italic*");
        let plain = ms.to_plain_text();
        assert!(plain.contains("bold"));
        assert!(!plain.contains("**"));
    }

    #[test]
    fn hover_builder_success() {
        let hover = HoverBuilder::new()
            .content(MarkdownString::new("info"))
            .code("let x = 1;", "rust")
            .range(HoverRange::from_word(0, 0, 5))
            .build()
            .unwrap();
        assert_eq!(hover.contents.len(), 2);
        assert!(hover.range.is_some());
    }

    #[test]
    fn hover_builder_empty_content_error() {
        let result = HoverBuilder::new().build();
        assert_eq!(result, Err(HoverError::EmptyContent));
    }

    #[test]
    fn hover_builder_invalid_range_error() {
        let bad_range = HoverRange {
            start_line: 10,
            start_col: 0,
            end_line: 5,
            end_col: 0,
        };
        let result = HoverBuilder::new()
            .content(MarkdownString::new("x"))
            .range(bad_range)
            .build();
        assert!(matches!(result, Err(HoverError::InvalidRange { .. })));
    }

    #[test]
    fn validate_uri_ok() {
        assert!(validate_uri("file:///foo.rs").is_ok());
        assert!(validate_uri("https://example.com").is_ok());
    }

    #[test]
    fn validate_uri_err() {
        assert!(validate_uri("").is_err());
        assert!(validate_uri("no_scheme").is_err());
    }

    #[test]
    fn hover_registry_collects_providers() {
        struct DummyProvider;
        impl HoverProvider for DummyProvider {
            fn provide_hover(&self, _uri: &str, _line: u32, _col: u32) -> Option<Hover> {
                Some(Hover::new(vec![MarkdownString::new("dummy")]))
            }
        }
        let mut reg = HoverRegistry::new();
        assert!(reg.is_empty());
        reg.register(Box::new(DummyProvider));
        assert_eq!(reg.len(), 1);
        let hover = reg.hover_at("file:///x.rs", 0, 0).unwrap();
        assert_eq!(hover.contents[0].value, "dummy");
    }

    #[test]
    fn hover_registry_returns_none_when_no_providers_match() {
        struct NoneProvider;
        impl HoverProvider for NoneProvider {
            fn provide_hover(&self, _uri: &str, _line: u32, _col: u32) -> Option<Hover> {
                None
            }
        }
        let mut reg = HoverRegistry::new();
        reg.register(Box::new(NoneProvider));
        assert!(reg.hover_at("file:///x.rs", 0, 0).is_none());
    }
}
