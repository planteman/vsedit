//! Workspace text search.

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub pattern: String,
    pub is_regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub include_pattern: Option<String>,
    pub exclude_pattern: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub uri: String,
    pub line: u32,
    pub column: u32,
    pub length: u32,
    pub preview: String,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub matches: Vec<SearchMatch>,
    pub is_complete: bool,
}

#[derive(Debug, Clone)]
pub struct TextSearchOptions {
    pub max_results: usize,
    pub follow_symlinks: bool,
    pub encoding: Option<String>,
}

/// Service for search workbench functionality.
pub struct SearchService {
    pub results: Vec<SearchResult>,
}

impl SearchService {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }

    /// Returns (start, length) pairs for each match of the query pattern in `line`.
    pub fn text_matches(query: &SearchQuery, line: &str) -> Vec<(usize, usize)> {
        let mut matches = Vec::new();
        let (haystack, needle);
        if query.case_sensitive {
            haystack = line.to_string();
            needle = query.pattern.clone();
        } else {
            haystack = line.to_lowercase();
            needle = query.pattern.to_lowercase();
        }
        if needle.is_empty() {
            return matches;
        }
        let mut start = 0;
        while let Some(pos) = haystack[start..].find(&needle) {
            let abs = start + pos;
            if query.whole_word {
                let before_ok = abs == 0
                    || !line.as_bytes()[abs - 1].is_ascii_alphanumeric();
                let after_ok = abs + needle.len() >= line.len()
                    || !line.as_bytes()[abs + needle.len()].is_ascii_alphanumeric();
                if before_ok && after_ok {
                    matches.push((abs, needle.len()));
                }
            } else {
                matches.push((abs, needle.len()));
            }
            start = abs + 1;
        }
        matches
    }

    pub fn match_count(result: &SearchResult) -> usize {
        result.matches.len()
    }

    pub fn file_count(result: &SearchResult) -> usize {
        let mut uris: Vec<&str> = result.matches.iter().map(|m| m.uri.as_str()).collect();
        uris.sort();
        uris.dedup();
        uris.len()
    }
}

impl Default for SearchService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_query(pattern: &str) -> SearchQuery {
        SearchQuery {
            pattern: pattern.into(),
            is_regex: false,
            case_sensitive: true,
            whole_word: false,
            include_pattern: None,
            exclude_pattern: None,
        }
    }

    #[test]
    fn text_matches_basic() {
        let q = simple_query("foo");
        let m = SearchService::text_matches(&q, "foo bar foo");
        assert_eq!(m, vec![(0, 3), (8, 3)]);
    }

    #[test]
    fn text_matches_case_insensitive() {
        let q = SearchQuery {
            case_sensitive: false,
            ..simple_query("Hello")
        };
        let m = SearchService::text_matches(&q, "hello HELLO Hello");
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn file_count_deduplicates() {
        let result = SearchResult {
            matches: vec![
                SearchMatch { uri: "a.rs".into(), line: 1, column: 0, length: 3, preview: String::new() },
                SearchMatch { uri: "a.rs".into(), line: 2, column: 0, length: 3, preview: String::new() },
                SearchMatch { uri: "b.rs".into(), line: 1, column: 0, length: 3, preview: String::new() },
            ],
            is_complete: true,
        };
        assert_eq!(SearchService::file_count(&result), 2);
        assert_eq!(SearchService::match_count(&result), 3);
    }

    #[test]
    fn whole_word_matching() {
        let q = SearchQuery {
            whole_word: true,
            ..simple_query("foo")
        };
        let m = SearchService::text_matches(&q, "foo foobar baz foo");
        assert_eq!(m, vec![(0, 3), (15, 3)]);
    }
}
