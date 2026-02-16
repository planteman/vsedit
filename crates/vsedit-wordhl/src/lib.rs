//! Word highlight (same symbol highlighting).
//!
//! Finds and highlights all occurrences of the word under the cursor,
//! distinguishing between read and write references.

/// Kind of highlight for a document symbol occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocumentHighlightKind {
    Text,
    Read,
    Write,
}

/// Convenience alias.
pub type HighlightKind = DocumentHighlightKind;

/// A highlight range within a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentHighlight {
    pub line: u32,
    pub start_column: u32,
    pub end_column: u32,
    pub kind: DocumentHighlightKind,
}

impl DocumentHighlight {
    pub fn new(line: u32, start_column: u32, end_column: u32, kind: DocumentHighlightKind) -> Self {
        Self { line, start_column, end_column, kind }
    }

    /// Length of the highlight in columns.
    pub fn len(&self) -> u32 {
        self.end_column.saturating_sub(self.start_column)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Extract the word at a given position (1-based line and column).
///
/// Returns `(word, start_column, end_column)` or `None` if no word is found.
pub fn find_word_at_position(lines: &[&str], line: u32, column: u32) -> Option<(String, u32, u32)> {
    if line == 0 || line as usize > lines.len() {
        return None;
    }
    let text = lines[(line - 1) as usize];
    if column == 0 || column as usize > text.len() {
        return None;
    }
    let col_idx = (column - 1) as usize;
    let bytes = text.as_bytes();

    if !is_word_char(bytes[col_idx]) {
        return None;
    }

    let mut start = col_idx;
    while start > 0 && is_word_char(bytes[start - 1]) {
        start -= 1;
    }

    let mut end = col_idx;
    while end < bytes.len() && is_word_char(bytes[end]) {
        end += 1;
    }

    let word = text[start..end].to_string();
    Some((word, (start + 1) as u32, (end + 1) as u32))
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Find all occurrences of a word in text (whole-word matching).
pub fn find_word_highlights(lines: &[&str], word: &str) -> Vec<DocumentHighlight> {
    let mut highlights = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let mut start = 0;
        while let Some(pos) = line[start..].find(word) {
            let abs_pos = start + pos;
            let before_ok = abs_pos == 0 || !is_word_char(line.as_bytes()[abs_pos - 1]);
            let after_pos = abs_pos + word.len();
            let after_ok = after_pos >= line.len() || !is_word_char(line.as_bytes()[after_pos]);
            if before_ok && after_ok {
                highlights.push(DocumentHighlight {
                    line: (i + 1) as u32,
                    start_column: (abs_pos + 1) as u32,
                    end_column: (after_pos + 1) as u32,
                    kind: DocumentHighlightKind::Text,
                });
            }
            start = abs_pos + word.len();
        }
    }
    highlights
}

/// Provider trait for document highlights.
pub trait DocumentHighlightProvider: Send + Sync {
    fn provide_document_highlights(&self, uri: &str, line: u32, column: u32) -> Vec<DocumentHighlight>;
}

/// Alias for the provider trait.
pub trait WordHighlightProvider: Send + Sync {
    fn highlight(&self, uri: &str, line: u32, column: u32) -> Vec<DocumentHighlight>;
}

/// Service that manages word highlight providers and resolves highlights.
pub struct WordHighlightService {
    providers: Vec<Box<dyn WordHighlightProvider>>,
}

impl WordHighlightService {
    pub fn new() -> Self {
        Self { providers: Vec::new() }
    }

    pub fn register(&mut self, provider: Box<dyn WordHighlightProvider>) {
        self.providers.push(provider);
    }

    /// Get highlights from all providers.
    pub fn highlights(&self, uri: &str, line: u32, column: u32) -> Vec<DocumentHighlight> {
        let mut all = Vec::new();
        for provider in &self.providers {
            all.extend(provider.highlight(uri, line, column));
        }
        all
    }

    /// Fallback: find word at position and highlight all occurrences in the given text.
    pub fn highlight_word_occurrences(
        lines: &[&str],
        cursor_line: u32,
        cursor_column: u32,
    ) -> Vec<DocumentHighlight> {
        match find_word_at_position(lines, cursor_line, cursor_column) {
            Some((word, _, _)) => find_word_highlights(lines, &word),
            None => Vec::new(),
        }
    }
}

impl Default for WordHighlightService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_word() {
        let lines = vec!["let x = 1;", "let y = x + 2;"];
        let hl = find_word_highlights(&lines, "x");
        assert_eq!(hl.len(), 2);
        assert_eq!(hl[0].line, 1);
        assert_eq!(hl[1].line, 2);
    }

    #[test]
    fn word_boundary() {
        let lines = vec!["prefix xx suffix"];
        let hl = find_word_highlights(&lines, "xx");
        assert_eq!(hl.len(), 1);
    }

    #[test]
    fn no_partial_match() {
        let lines = vec!["foobar"];
        let hl = find_word_highlights(&lines, "foo");
        assert_eq!(hl.len(), 0);
    }

    #[test]
    fn find_word_at_position_basic() {
        let lines = vec!["let hello = 42;"];
        let result = find_word_at_position(&lines, 1, 6);
        assert!(result.is_some());
        let (word, start, end) = result.unwrap();
        assert_eq!(word, "hello");
        assert_eq!(start, 5);
        assert_eq!(end, 10);
    }

    #[test]
    fn find_word_at_position_start_of_word() {
        let lines = vec!["abc def"];
        let (word, _, _) = find_word_at_position(&lines, 1, 1).unwrap();
        assert_eq!(word, "abc");
    }

    #[test]
    fn find_word_at_position_on_space() {
        let lines = vec!["abc def"];
        assert!(find_word_at_position(&lines, 1, 4).is_none());
    }

    #[test]
    fn find_word_at_position_out_of_bounds() {
        let lines = vec!["abc"];
        assert!(find_word_at_position(&lines, 0, 1).is_none());
        assert!(find_word_at_position(&lines, 2, 1).is_none());
        assert!(find_word_at_position(&lines, 1, 0).is_none());
        assert!(find_word_at_position(&lines, 1, 5).is_none());
    }

    #[test]
    fn highlight_word_occurrences_service() {
        let lines = vec!["let x = 1;", "return x;"];
        let hl = WordHighlightService::highlight_word_occurrences(&lines, 1, 5);
        assert_eq!(hl.len(), 2);
    }

    #[test]
    fn highlight_word_occurrences_no_word() {
        let lines = vec!["= + ;"];
        let hl = WordHighlightService::highlight_word_occurrences(&lines, 1, 1);
        assert!(hl.is_empty());
    }

    #[test]
    fn document_highlight_len() {
        let h = DocumentHighlight::new(1, 5, 10, DocumentHighlightKind::Read);
        assert_eq!(h.len(), 5);
        assert!(!h.is_empty());
    }

    #[test]
    fn highlight_kind_alias() {
        let k: HighlightKind = HighlightKind::Write;
        assert_eq!(k, DocumentHighlightKind::Write);
    }

    #[test]
    fn underscore_word_boundary() {
        let lines = vec!["my_var = my_var + 1"];
        let hl = find_word_highlights(&lines, "my_var");
        assert_eq!(hl.len(), 2);
    }
}
