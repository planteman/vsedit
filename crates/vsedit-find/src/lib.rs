//! Find and replace functionality.
//!
//! Equivalent to VS Code's `vs/editor/contrib/find`.
//! Provides text search with regex, case sensitivity, whole word, and replace.

use regex::Regex;

/// Options for a find operation.
#[derive(Debug, Clone)]
pub struct FindOptions {
    pub search_string: String,
    pub is_regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub preserve_case: bool,
}

impl FindOptions {
    pub fn new(search_string: impl Into<String>) -> Self {
        Self {
            search_string: search_string.into(),
            is_regex: false,
            case_sensitive: false,
            whole_word: false,
            preserve_case: false,
        }
    }

    pub fn with_case_sensitive(mut self, v: bool) -> Self {
        self.case_sensitive = v;
        self
    }

    pub fn with_regex(mut self, v: bool) -> Self {
        self.is_regex = v;
        self
    }

    pub fn with_whole_word(mut self, v: bool) -> Self {
        self.whole_word = v;
        self
    }
}

/// A match found in the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindMatch {
    pub line: u32,     // 1-based
    pub start_col: u32, // 1-based
    pub end_col: u32,   // 1-based, exclusive
    pub text: String,
}

/// Find all matches in text.
pub fn find_matches(text: &str, options: &FindOptions) -> Vec<FindMatch> {
    if options.search_string.is_empty() {
        return Vec::new();
    }

    let pattern = if options.is_regex {
        options.search_string.clone()
    } else {
        regex::escape(&options.search_string)
    };

    let pattern = if options.whole_word {
        format!(r"\b{}\b", pattern)
    } else {
        pattern
    };

    let re = if options.case_sensitive {
        Regex::new(&pattern)
    } else {
        Regex::new(&format!("(?i){}", pattern))
    };

    let re = match re {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut matches = Vec::new();
    for (line_idx, line) in text.lines().enumerate() {
        for m in re.find_iter(line) {
            matches.push(FindMatch {
                line: (line_idx + 1) as u32,
                start_col: (m.start() + 1) as u32,
                end_col: (m.end() + 1) as u32,
                text: m.as_str().to_string(),
            });
        }
    }

    matches
}

/// Replace all matches in text.
pub fn replace_all(text: &str, options: &FindOptions, replacement: &str) -> String {
    if options.search_string.is_empty() {
        return text.to_string();
    }

    let pattern = if options.is_regex {
        options.search_string.clone()
    } else {
        regex::escape(&options.search_string)
    };

    let pattern = if options.whole_word {
        format!(r"\b{}\b", pattern)
    } else {
        pattern
    };

    let re = if options.case_sensitive {
        Regex::new(&pattern)
    } else {
        Regex::new(&format!("(?i){}", pattern))
    };

    match re {
        Ok(r) => r.replace_all(text, replacement).to_string(),
        Err(_) => text.to_string(),
    }
}

/// Find state for incremental search.
#[derive(Debug, Clone)]
pub struct FindState {
    pub options: FindOptions,
    pub matches: Vec<FindMatch>,
    pub current_match: Option<usize>,
    pub replace_string: String,
}

impl FindState {
    pub fn new() -> Self {
        Self {
            options: FindOptions::new(""),
            matches: Vec::new(),
            current_match: None,
            replace_string: String::new(),
        }
    }

    /// Update the search and recompute matches.
    pub fn search(&mut self, text: &str) {
        self.matches = find_matches(text, &self.options);
        if self.matches.is_empty() {
            self.current_match = None;
        } else {
            self.current_match = Some(0);
        }
    }

    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    pub fn next_match(&mut self) {
        if let Some(idx) = self.current_match {
            if !self.matches.is_empty() {
                self.current_match = Some((idx + 1) % self.matches.len());
            }
        }
    }

    pub fn previous_match(&mut self) {
        if let Some(idx) = self.current_match {
            if !self.matches.is_empty() {
                self.current_match = Some(if idx == 0 {
                    self.matches.len() - 1
                } else {
                    idx - 1
                });
            }
        }
    }

    pub fn current(&self) -> Option<&FindMatch> {
        self.current_match.and_then(|i| self.matches.get(i))
    }
}

impl Default for FindState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_simple() {
        let matches = find_matches("hello world\nhello rust", &FindOptions::new("hello"));
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line, 1);
        assert_eq!(matches[0].start_col, 1);
        assert_eq!(matches[1].line, 2);
    }

    #[test]
    fn find_case_insensitive() {
        let opts = FindOptions::new("hello").with_case_sensitive(false);
        let matches = find_matches("Hello HELLO hello", &opts);
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn find_case_sensitive() {
        let opts = FindOptions::new("Hello").with_case_sensitive(true);
        let matches = find_matches("Hello hello HELLO", &opts);
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn find_regex() {
        let opts = FindOptions::new(r"\d+").with_regex(true);
        let matches = find_matches("abc 123 def 456", &opts);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].text, "123");
    }

    #[test]
    fn find_whole_word() {
        let opts = FindOptions::new("he").with_whole_word(true);
        let matches = find_matches("he hello the he", &opts);
        assert_eq!(matches.len(), 2); // "he" at start and end
    }

    #[test]
    fn find_no_match() {
        let matches = find_matches("hello world", &FindOptions::new("xyz"));
        assert!(matches.is_empty());
    }

    #[test]
    fn replace() {
        let result = replace_all("hello world", &FindOptions::new("world"), "rust");
        assert_eq!(result, "hello rust");
    }

    #[test]
    fn find_state_navigation() {
        let mut state = FindState::new();
        state.options = FindOptions::new("a");
        state.search("a b a c a");
        assert_eq!(state.match_count(), 3);
        assert_eq!(state.current(), Some(&state.matches[0]));

        state.next_match();
        assert_eq!(state.current(), Some(&state.matches[1]));

        state.next_match();
        state.next_match(); // wraps around
        assert_eq!(state.current(), Some(&state.matches[0]));

        state.previous_match(); // wraps back
        assert_eq!(state.current(), Some(&state.matches[2]));
    }

    #[test]
    fn empty_search() {
        let matches = find_matches("hello", &FindOptions::new(""));
        assert!(matches.is_empty());
    }
}
