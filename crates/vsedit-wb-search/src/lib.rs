//! Workspace text search.
//!
//! Provides file-system-backed search, replace, fuzzy file name matching,
//! and regex-based symbol extraction.

use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobMatcher};
use regex::Regex;
use walkdir::WalkDir;

/// Errors that can occur during search operations.
#[derive(Debug, Clone, PartialEq)]
pub enum SearchError {
    EmptyPattern,
    InvalidRegex(String),
    TooManyResults(usize),
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchError::EmptyPattern => write!(f, "search pattern must not be empty"),
            SearchError::InvalidRegex(msg) => write!(f, "invalid regex: {msg}"),
            SearchError::TooManyResults(n) => write!(f, "too many results: {n}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub pattern: String,
    pub is_regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub include_pattern: Option<String>,
    pub exclude_pattern: Option<String>,
}

impl fmt::Display for SearchQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut flags = Vec::new();
        if self.is_regex {
            flags.push("regex");
        }
        if self.case_sensitive {
            flags.push("case-sensitive");
        }
        if self.whole_word {
            flags.push("whole-word");
        }
        if flags.is_empty() {
            write!(f, "{}", self.pattern)
        } else {
            write!(f, "{} ({})", self.pattern, flags.join(", "))
        }
    }
}

/// Builder for constructing a [`SearchQuery`] step by step.
pub struct SearchQueryBuilder {
    pattern: String,
    is_regex: bool,
    case_sensitive: bool,
    whole_word: bool,
    include_pattern: Option<String>,
    exclude_pattern: Option<String>,
}

impl SearchQueryBuilder {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            is_regex: false,
            case_sensitive: true,
            whole_word: false,
            include_pattern: None,
            exclude_pattern: None,
        }
    }

    pub fn regex(mut self, yes: bool) -> Self {
        self.is_regex = yes;
        self
    }

    pub fn case_sensitive(mut self, yes: bool) -> Self {
        self.case_sensitive = yes;
        self
    }

    pub fn whole_word(mut self, yes: bool) -> Self {
        self.whole_word = yes;
        self
    }

    pub fn include(mut self, pattern: impl Into<String>) -> Self {
        self.include_pattern = Some(pattern.into());
        self
    }

    pub fn exclude(mut self, pattern: impl Into<String>) -> Self {
        self.exclude_pattern = Some(pattern.into());
        self
    }

    pub fn build(self) -> SearchQuery {
        SearchQuery {
            pattern: self.pattern,
            is_regex: self.is_regex,
            case_sensitive: self.case_sensitive,
            whole_word: self.whole_word,
            include_pattern: self.include_pattern,
            exclude_pattern: self.exclude_pattern,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub uri: String,
    pub line: u32,
    pub column: u32,
    pub length: u32,
    pub preview: String,
}

impl fmt::Display for SearchMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{} {}", self.uri, self.line, self.column, self.preview)
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub matches: Vec<SearchMatch>,
    pub is_complete: bool,
}

impl SearchResult {
    /// Returns `true` when there are no matches.
    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }

    /// Groups matches by their URI, preserving encounter order.
    pub fn group_by_file(&self) -> Vec<(String, Vec<&SearchMatch>)> {
        let mut groups: Vec<(String, Vec<&SearchMatch>)> = Vec::new();
        for m in &self.matches {
            if let Some(g) = groups.iter_mut().find(|(uri, _)| uri == &m.uri) {
                g.1.push(m);
            } else {
                groups.push((m.uri.clone(), vec![m]));
            }
        }
        groups
    }
}

#[derive(Debug, Clone)]
pub struct TextSearchOptions {
    pub max_results: usize,
    pub follow_symlinks: bool,
    pub encoding: Option<String>,
}

impl Default for TextSearchOptions {
    fn default() -> Self {
        Self {
            max_results: 10_000,
            follow_symlinks: false,
            encoding: None,
        }
    }
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

    /// Searches full text (multiple lines) and returns a [`SearchResult`].
    /// Each line is searched independently; the `uri` field is set to the
    /// provided value for every match.
    pub fn search_in_text(query: &SearchQuery, text: &str, uri: &str) -> SearchResult {
        let mut matches = Vec::new();
        for (line_idx, line) in text.lines().enumerate() {
            for (col, len) in Self::text_matches(query, line) {
                matches.push(SearchMatch {
                    uri: uri.to_string(),
                    line: (line_idx + 1) as u32,
                    column: col as u32,
                    length: len as u32,
                    preview: line.to_string(),
                });
            }
        }
        SearchResult {
            matches,
            is_complete: true,
        }
    }

    /// Replaces all occurrences of the query pattern in `line` with `replacement`.
    pub fn replace_matches(query: &SearchQuery, line: &str, replacement: &str) -> String {
        let hits = Self::text_matches(query, line);
        if hits.is_empty() {
            return line.to_string();
        }
        let mut result = String::with_capacity(line.len());
        let mut prev_end = 0;
        for (start, len) in &hits {
            result.push_str(&line[prev_end..*start]);
            result.push_str(replacement);
            prev_end = start + len;
        }
        result.push_str(&line[prev_end..]);
        result
    }

    /// Wraps each match in `line` with `>>` and `<<` markers.
    pub fn highlight_matches(query: &SearchQuery, line: &str) -> String {
        let hits = Self::text_matches(query, line);
        if hits.is_empty() {
            return line.to_string();
        }
        let mut result = String::with_capacity(line.len() + hits.len() * 4);
        let mut prev_end = 0;
        for (start, len) in &hits {
            result.push_str(&line[prev_end..*start]);
            result.push_str(">>");
            result.push_str(&line[*start..*start + len]);
            result.push_str("<<");
            prev_end = start + len;
        }
        result.push_str(&line[prev_end..]);
        result
    }

    /// Returns true if results is empty.
    pub fn is_results_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// Get the first result, if any.
    pub fn first_result(&self) -> Option<&SearchResult> {
        self.results.first()
    }

    /// Get the last result, if any.
    pub fn last_result(&self) -> Option<&SearchResult> {
        self.results.last()
    }

    /// Retain only results matching the predicate.
    pub fn retain_results(&mut self, f: impl Fn(&SearchResult) -> bool) {
        self.results.retain(|item| f(item));
    }
}

impl Default for SearchService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// File-system search
// ---------------------------------------------------------------------------

/// Maximum number of results before stopping.
const MAX_FILE_RESULTS: usize = 10_000;

/// Search files on disk matching `query` under `root_dir`.
///
/// Walks the directory tree, respects `.gitignore` patterns when present,
/// skips hidden/dot directories and binary files, and honours the query's
/// include/exclude glob patterns.
pub fn search_files(query: &SearchQuery, root_dir: &Path) -> Vec<SearchResult> {
    let re = match build_regex(query) {
        Some(r) => r,
        None => return Vec::new(),
    };

    let include_matcher = query
        .include_pattern
        .as_deref()
        .and_then(|p| Glob::new(p).ok())
        .map(|g| g.compile_matcher());
    let exclude_matcher = query
        .exclude_pattern
        .as_deref()
        .and_then(|p| Glob::new(p).ok())
        .map(|g| g.compile_matcher());

    let gitignore_patterns = load_gitignore(root_dir);

    let mut results: Vec<SearchResult> = Vec::new();
    let mut total = 0usize;

    for entry in WalkDir::new(root_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if total >= MAX_FILE_RESULTS {
            break;
        }

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        // Skip hidden / dot directories
        if path
            .strip_prefix(root_dir)
            .unwrap_or(path)
            .components()
            .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
        {
            continue;
        }

        if is_gitignored(path, root_dir, &gitignore_patterns) {
            continue;
        }

        if !matches_glob(&include_matcher, &exclude_matcher, path) {
            continue;
        }

        if is_binary(path) {
            continue;
        }

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let uri = path.to_string_lossy().to_string();
        let mut matches = Vec::new();
        for (line_idx, line) in content.lines().enumerate() {
            for m in re.find_iter(line) {
                matches.push(SearchMatch {
                    uri: uri.clone(),
                    line: (line_idx + 1) as u32,
                    column: m.start() as u32,
                    length: (m.end() - m.start()) as u32,
                    preview: line.to_string(),
                });
                total += 1;
                if total >= MAX_FILE_RESULTS {
                    break;
                }
            }
            if total >= MAX_FILE_RESULTS {
                break;
            }
        }

        if !matches.is_empty() {
            results.push(SearchResult {
                matches,
                is_complete: total < MAX_FILE_RESULTS,
            });
        }
    }

    results
}

// ---------------------------------------------------------------------------
// Replace in files
// ---------------------------------------------------------------------------

/// Extends a [`SearchQuery`] with a replacement string.
#[derive(Debug, Clone)]
pub struct ReplaceQuery {
    pub query: SearchQuery,
    pub replacement: String,
}

impl ReplaceQuery {
    pub fn new(query: SearchQuery, replacement: impl Into<String>) -> Self {
        Self {
            query,
            replacement: replacement.into(),
        }
    }
}

/// Show what a single match line would look like after replacement.
pub fn preview_replace(search_match: &SearchMatch, replacement: &str) -> String {
    let line = &search_match.preview;
    let start = search_match.column as usize;
    let end = start + search_match.length as usize;
    if end > line.len() {
        return line.clone();
    }
    format!("{}{}{}", &line[..start], replacement, &line[end..])
}

/// Apply replacements for all matches in a single file (grouped by URI).
/// Returns `true` on success.
pub fn execute_replace(file_result: &SearchResult, replacement: &str) -> bool {
    if file_result.matches.is_empty() {
        return true;
    }

    let path = Path::new(&file_result.matches[0].uri);
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let lines: Vec<&str> = content.lines().collect();
    let mut result_lines: Vec<String> = lines.iter().map(|l| l.to_string()).collect();

    // Group matches by line
    let mut by_line: std::collections::HashMap<u32, Vec<&SearchMatch>> =
        std::collections::HashMap::new();
    for m in &file_result.matches {
        by_line.entry(m.line).or_default().push(m);
    }

    for (line_num, mut line_matches) in by_line {
        let idx = (line_num - 1) as usize;
        if idx >= result_lines.len() {
            continue;
        }
        // Sort by column descending to preserve offsets
        line_matches.sort_by(|a, b| b.column.cmp(&a.column));
        let mut line = result_lines[idx].clone();
        for m in &line_matches {
            let start = m.column as usize;
            let end = start + m.length as usize;
            if end <= line.len() {
                line = format!("{}{}{}", &line[..start], replacement, &line[end..]);
            }
        }
        result_lines[idx] = line;
    }

    let mut output = result_lines.join("\n");
    if content.ends_with('\n') {
        output.push('\n');
    }

    fs::write(path, output).is_ok()
}

/// Search all files and replace matches in place.
pub fn replace_all(query: &ReplaceQuery, root_dir: &Path) -> usize {
    let results = search_files(&query.query, root_dir);
    let mut replaced = 0usize;
    for result in &results {
        let count = result.matches.len();
        if execute_replace(result, &query.replacement) {
            replaced += count;
        }
    }
    replaced
}

// ---------------------------------------------------------------------------
// File name search (Ctrl+P quick open)
// ---------------------------------------------------------------------------

/// Fuzzy match file names under `root_dir`.
///
/// Scoring: exact name match > prefix > contains > fuzzy subsequence.
pub fn search_file_names(query: &str, root_dir: &Path) -> Vec<PathBuf> {
    let lower_query = query.to_lowercase();
    let mut scored: Vec<(PathBuf, i64)> = Vec::new();

    for entry in WalkDir::new(root_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        // Skip hidden/dot directories
        if path
            .strip_prefix(root_dir)
            .unwrap_or(path)
            .components()
            .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
        {
            continue;
        }

        let name = match path.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };
        let lower_name = name.to_lowercase();

        let score = if query.is_empty() {
            // Return all files when query is empty
            0
        } else if lower_name == lower_query {
            1000 // exact match
        } else if lower_name.starts_with(&lower_query) {
            500 // prefix
        } else if lower_name.contains(&lower_query) {
            200 // contains
        } else if fuzzy_match(&lower_query, &lower_name) {
            100 // fuzzy subsequence
        } else {
            continue; // no match
        };

        scored.push((path.to_path_buf(), score));
    }

    scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    scored.into_iter().map(|(p, _)| p).collect()
}

/// Check if `query` is a subsequence of `text`.
fn fuzzy_match(query: &str, text: &str) -> bool {
    let mut query_chars = query.chars();
    let mut current = match query_chars.next() {
        Some(c) => c,
        None => return true,
    };
    for ch in text.chars() {
        if ch == current {
            current = match query_chars.next() {
                Some(c) => c,
                None => return true,
            };
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Symbol search (Ctrl+Shift+O)
// ---------------------------------------------------------------------------

/// Kind of symbol extracted from source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    Const,
    Type,
    Module,
    Other,
}

impl fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let icon = match self {
            SymbolKind::Function => "ƒ",
            SymbolKind::Struct => "S",
            SymbolKind::Enum => "E",
            SymbolKind::Trait => "T",
            SymbolKind::Impl => "I",
            SymbolKind::Const => "C",
            SymbolKind::Type => "τ",
            SymbolKind::Module => "M",
            SymbolKind::Other => "?",
        };
        write!(f, "{icon}")
    }
}

/// A symbol extracted from source code.
#[derive(Debug, Clone)]
pub struct SymbolEntry {
    pub name: String,
    pub kind: SymbolKind,
    pub line: u32,
    pub column: u32,
    pub container_name: Option<String>,
}

impl fmt::Display for SymbolEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref container) = self.container_name {
            write!(f, "{} {} {}.{}", self.kind, self.name, container, self.name)
        } else {
            write!(f, "{} {}", self.kind, self.name)
        }
    }
}

/// Extract symbols from source code using regex patterns.
pub fn extract_symbols(source: &str) -> Vec<SymbolEntry> {
    let patterns: &[(&str, SymbolKind)] = &[
        (r"(?m)^\s*(?:pub\s+)?fn\s+(\w+)", SymbolKind::Function),
        (r"(?m)^\s*(?:pub\s+)?struct\s+(\w+)", SymbolKind::Struct),
        (r"(?m)^\s*(?:pub\s+)?enum\s+(\w+)", SymbolKind::Enum),
        (r"(?m)^\s*(?:pub\s+)?trait\s+(\w+)", SymbolKind::Trait),
        (r"(?m)^\s*impl(?:<[^>]*>)?\s+(\w+)", SymbolKind::Impl),
        (r"(?m)^\s*(?:pub\s+)?const\s+(\w+)", SymbolKind::Const),
        (r"(?m)^\s*(?:pub\s+)?type\s+(\w+)", SymbolKind::Type),
        (r"(?m)^\s*(?:pub\s+)?mod\s+(\w+)", SymbolKind::Module),
    ];

    let mut symbols = Vec::new();
    for (pat, kind) in patterns {
        let re = match Regex::new(pat) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for caps in re.captures_iter(source) {
            if let Some(name_match) = caps.get(1) {
                let line = source[..name_match.start()]
                    .matches('\n')
                    .count() as u32
                    + 1;
                let line_start = source[..name_match.start()]
                    .rfind('\n')
                    .map(|p| p + 1)
                    .unwrap_or(0);
                let column = (name_match.start() - line_start) as u32;
                symbols.push(SymbolEntry {
                    name: name_match.as_str().to_string(),
                    kind: *kind,
                    line,
                    column,
                    container_name: None,
                });
            }
        }
    }

    symbols.sort_by_key(|s| s.line);
    symbols
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a [`Regex`] from a [`SearchQuery`].
fn build_regex(query: &SearchQuery) -> Option<Regex> {
    if query.pattern.is_empty() {
        return None;
    }

    let pat = if query.is_regex {
        query.pattern.clone()
    } else {
        regex::escape(&query.pattern)
    };

    let pat = if query.whole_word {
        format!(r"\b{pat}\b")
    } else {
        pat
    };

    let pat = if query.case_sensitive {
        pat
    } else {
        format!("(?i){pat}")
    };

    Regex::new(&pat).ok()
}

fn matches_glob(
    include: &Option<GlobMatcher>,
    exclude: &Option<GlobMatcher>,
    path: &Path,
) -> bool {
    if let Some(inc) = include {
        if !inc.is_match(path) {
            return false;
        }
    }
    if let Some(exc) = exclude {
        if exc.is_match(path) {
            return false;
        }
    }
    true
}

/// Heuristic: read first 512 bytes and check for NUL.
fn is_binary(path: &Path) -> bool {
    let mut file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return true,
    };
    let mut buf = [0u8; 512];
    let n = match file.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return true,
    };
    buf[..n].contains(&0)
}

/// Load `.gitignore` patterns from `root`, returns compiled glob matchers.
fn load_gitignore(root: &Path) -> Vec<GlobMatcher> {
    let gitignore = root.join(".gitignore");
    let content = match fs::read_to_string(gitignore) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut matchers = Vec::new();
    for line in content.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let pattern = l.trim_end_matches('/');
        // Match the entry itself and anything beneath it
        if let Ok(g) = Glob::new(&format!("**/{pattern}")) {
            matchers.push(g.compile_matcher());
        }
        if let Ok(g) = Glob::new(&format!("**/{pattern}/**")) {
            matchers.push(g.compile_matcher());
        }
        // Also try the pattern as given (for already-globbed patterns)
        if let Ok(g) = Glob::new(pattern) {
            matchers.push(g.compile_matcher());
        }
    }

    matchers
}

/// Check if a path matches any gitignore pattern.
fn is_gitignored(path: &Path, root: &Path, patterns: &[GlobMatcher]) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    patterns.iter().any(|p| p.is_match(relative))
}

// ---------------------------------------------------------------------------
// FileQuickPick — overlay for Ctrl+P
// ---------------------------------------------------------------------------

/// State for the quick-open file picker overlay.
#[derive(Debug)]
pub struct FileQuickPick {
    pub query: String,
    pub results: Vec<PathBuf>,
    pub selected: usize,
    pub recent_files: Vec<PathBuf>,
}

impl FileQuickPick {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            recent_files: Vec::new(),
        }
    }

    /// Update results based on the current query.
    pub fn update(&mut self, root_dir: &Path) {
        if self.query.is_empty() {
            // Show recent files first, then all files
            let mut all = self.recent_files.clone();
            for path in search_file_names("", root_dir) {
                if !all.contains(&path) {
                    all.push(path);
                }
            }
            self.results = all;
        } else {
            self.results = search_file_names(&self.query, root_dir);
        }
        self.selected = 0;
    }

    /// Select next item.
    pub fn select_next(&mut self) {
        if !self.results.is_empty() {
            self.selected = (self.selected + 1) % self.results.len();
        }
    }

    /// Select previous item.
    pub fn select_previous(&mut self) {
        if !self.results.is_empty() {
            self.selected = if self.selected == 0 {
                self.results.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    /// Get the currently selected path.
    pub fn selected_path(&self) -> Option<&Path> {
        self.results.get(self.selected).map(|p| p.as_path())
    }

    /// Record a file as recently opened.
    pub fn add_recent(&mut self, path: PathBuf) {
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path);
        if self.recent_files.len() > 20 {
            self.recent_files.truncate(20);
        }
    }
}

impl Default for FileQuickPick {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir(prefix: &str) -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "vsedit_wb_search_test_{prefix}_{id}_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
        path
    }

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

    #[test]
    fn search_error_display_empty() {
        let e = SearchError::EmptyPattern;
        assert_eq!(e.to_string(), "search pattern must not be empty");
    }

    #[test]
    fn search_error_display_invalid_regex() {
        let e = SearchError::InvalidRegex("bad group".into());
        assert_eq!(e.to_string(), "invalid regex: bad group");
    }

    #[test]
    fn search_error_display_too_many() {
        let e = SearchError::TooManyResults(5000);
        assert_eq!(e.to_string(), "too many results: 5000");
    }

    #[test]
    fn search_query_display_no_flags() {
        let q = SearchQuery {
            case_sensitive: false,
            ..simple_query("hello")
        };
        assert_eq!(q.to_string(), "hello");
    }

    #[test]
    fn search_query_display_with_flags() {
        let q = SearchQuery {
            is_regex: true,
            whole_word: true,
            ..simple_query("pat")
        };
        assert_eq!(q.to_string(), "pat (regex, case-sensitive, whole-word)");
    }

    #[test]
    fn search_match_display() {
        let m = SearchMatch {
            uri: "file.rs".into(),
            line: 10,
            column: 5,
            length: 3,
            preview: "hello".into(),
        };
        assert_eq!(m.to_string(), "file.rs:10:5 hello");
    }

    #[test]
    fn search_in_text_multiline() {
        let q = simple_query("fn");
        let text = "fn main() {\n    let x = 1;\n    fn helper() {}\n}";
        let result = SearchService::search_in_text(&q, text, "main.rs");
        assert_eq!(result.matches.len(), 2);
        assert_eq!(result.matches[0].line, 1);
        assert_eq!(result.matches[1].line, 3);
        assert_eq!(result.matches[0].uri, "main.rs");
    }

    #[test]
    fn replace_matches_basic() {
        let q = simple_query("foo");
        let out = SearchService::replace_matches(&q, "foo bar foo", "baz");
        assert_eq!(out, "baz bar baz");
    }

    #[test]
    fn replace_matches_no_match() {
        let q = simple_query("xyz");
        let out = SearchService::replace_matches(&q, "hello world", "replaced");
        assert_eq!(out, "hello world");
    }

    #[test]
    fn highlight_matches_basic() {
        let q = simple_query("bar");
        let out = SearchService::highlight_matches(&q, "foo bar baz bar");
        assert_eq!(out, "foo >>bar<< baz >>bar<<");
    }

    #[test]
    fn query_builder_defaults() {
        let q = SearchQueryBuilder::new("test").build();
        assert_eq!(q.pattern, "test");
        assert!(q.case_sensitive);
        assert!(!q.is_regex);
        assert!(!q.whole_word);
        assert!(q.include_pattern.is_none());
        assert!(q.exclude_pattern.is_none());
    }

    #[test]
    fn query_builder_all_options() {
        let q = SearchQueryBuilder::new("pat")
            .regex(true)
            .case_sensitive(false)
            .whole_word(true)
            .include("*.rs")
            .exclude("target/")
            .build();
        assert!(q.is_regex);
        assert!(!q.case_sensitive);
        assert!(q.whole_word);
        assert_eq!(q.include_pattern.as_deref(), Some("*.rs"));
        assert_eq!(q.exclude_pattern.as_deref(), Some("target/"));
    }

    #[test]
    fn group_by_file_ordering() {
        let result = SearchResult {
            matches: vec![
                SearchMatch { uri: "a.rs".into(), line: 1, column: 0, length: 3, preview: String::new() },
                SearchMatch { uri: "b.rs".into(), line: 1, column: 0, length: 3, preview: String::new() },
                SearchMatch { uri: "a.rs".into(), line: 5, column: 0, length: 3, preview: String::new() },
            ],
            is_complete: true,
        };
        let groups = result.group_by_file();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, "a.rs");
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].0, "b.rs");
        assert_eq!(groups[1].1.len(), 1);
    }

    #[test]
    fn search_result_is_empty() {
        let empty = SearchResult { matches: vec![], is_complete: true };
        assert!(empty.is_empty());

        let non_empty = SearchResult {
            matches: vec![SearchMatch { uri: "x".into(), line: 1, column: 0, length: 1, preview: String::new() }],
            is_complete: true,
        };
        assert!(!non_empty.is_empty());
    }

    #[test]
    fn text_search_options_default() {
        let opts = TextSearchOptions::default();
        assert_eq!(opts.max_results, 10_000);
        assert!(!opts.follow_symlinks);
        assert!(opts.encoding.is_none());
    }

    #[test]
    fn display_searcherror_variants() {
        assert!(!SearchError::EmptyPattern.to_string().is_empty());
    }

    #[test]
    fn search_files_finds_matches() {
        let dir = temp_dir("search_files");
        write_file(&dir, "a.txt", "hello world\ngoodbye world");
        write_file(&dir, "b.txt", "nothing here");
        write_file(&dir, "c.txt", "hello again");

        let q = simple_query("hello");
        let results = search_files(&q, &dir);
        let total: usize = results.iter().map(|r| r.matches.len()).sum();
        assert_eq!(total, 2);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_files_include_filter() {
        let dir = temp_dir("include_filter");
        write_file(&dir, "code.rs", "find_me");
        write_file(&dir, "readme.md", "find_me");

        let q = SearchQuery {
            include_pattern: Some("*.rs".into()),
            ..simple_query("find_me")
        };
        let results = search_files(&q, &dir);
        assert_eq!(results.len(), 1);
        assert!(results[0].matches[0].uri.ends_with("code.rs"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_files_exclude_filter() {
        let dir = temp_dir("exclude_filter");
        write_file(&dir, "code.rs", "find_me");
        write_file(&dir, "readme.md", "find_me");

        let q = SearchQuery {
            exclude_pattern: Some("*.md".into()),
            ..simple_query("find_me")
        };
        let results = search_files(&q, &dir);
        assert_eq!(results.len(), 1);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_files_skips_binary() {
        let dir = temp_dir("skip_binary");
        write_file(&dir, "text.txt", "hello");
        let bin = dir.join("binary.bin");
        fs::write(&bin, b"hello\x00world").unwrap();

        let q = simple_query("hello");
        let results = search_files(&q, &dir);
        assert_eq!(results.len(), 1);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_files_skips_dot_dirs() {
        let dir = temp_dir("dot_dirs");
        write_file(&dir, "visible.txt", "hello");
        write_file(&dir, ".hidden/secret.txt", "hello");

        let q = simple_query("hello");
        let results = search_files(&q, &dir);
        assert_eq!(results.len(), 1);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn preview_replace_basic() {
        let m = SearchMatch {
            uri: "test.rs".into(),
            line: 1,
            column: 6,
            length: 5,
            preview: "hello world".into(),
        };
        assert_eq!(preview_replace(&m, "universe"), "hello universe");
    }

    #[test]
    fn execute_replace_in_file() {
        let dir = temp_dir("exec_replace");
        let path = write_file(&dir, "replace.txt", "hello world\ngoodbye world\n");

        let result = SearchResult {
            matches: vec![
                SearchMatch {
                    uri: path.to_string_lossy().into(),
                    line: 1,
                    column: 6,
                    length: 5,
                    preview: "hello world".into(),
                },
                SearchMatch {
                    uri: path.to_string_lossy().into(),
                    line: 2,
                    column: 8,
                    length: 5,
                    preview: "goodbye world".into(),
                },
            ],
            is_complete: true,
        };

        assert!(execute_replace(&result, "universe"));
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("hello universe"));
        assert!(content.contains("goodbye universe"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn replace_all_across_files() {
        let dir = temp_dir("replace_all");
        write_file(&dir, "a.txt", "foo bar\n");
        write_file(&dir, "b.txt", "bar baz foo\n");

        let rq = ReplaceQuery::new(simple_query("foo"), "qux");
        let count = replace_all(&rq, &dir);
        assert!(count >= 2);

        let a = fs::read_to_string(dir.join("a.txt")).unwrap();
        assert!(a.contains("qux"));
        let b = fs::read_to_string(dir.join("b.txt")).unwrap();
        assert!(b.contains("qux"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_file_names_exact_match() {
        let dir = temp_dir("fname_exact");
        write_file(&dir, "main.rs", "");
        write_file(&dir, "lib.rs", "");
        write_file(&dir, "utils.rs", "");

        let results = search_file_names("main.rs", &dir);
        assert!(!results.is_empty());
        assert!(results[0].file_name().unwrap().to_string_lossy() == "main.rs");
    }

    #[test]
    fn search_file_names_fuzzy() {
        let dir = temp_dir("fname_fuzzy");
        write_file(&dir, "search_view.rs", "");
        write_file(&dir, "other.txt", "");

        let results = search_file_names("sv", &dir);
        assert!(results.iter().any(|p| p.file_name().unwrap().to_string_lossy().contains("search_view")));
    }

    #[test]
    fn search_file_names_empty_query() {
        let dir = temp_dir("fname_empty");
        write_file(&dir, "a.txt", "");
        write_file(&dir, "b.txt", "");

        let results = search_file_names("", &dir);
        assert!(results.len() >= 2);
    }

    #[test]
    fn fuzzy_match_subsequence() {
        assert!(fuzzy_match("abc", "aXbYcZ"));
        assert!(!fuzzy_match("xyz", "abc"));
        assert!(fuzzy_match("", "anything"));
    }

    #[test]
    fn extract_symbols_from_rust() {
        let source = r#"
pub fn main() {}
struct Foo {}
pub enum Bar { A, B }
trait MyTrait {}
impl Foo {}
const MAX: usize = 10;
type Alias = u32;
mod submod;
"#;
        let symbols = extract_symbols(source);
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"main"));
        assert!(names.contains(&"Foo"));
        assert!(names.contains(&"Bar"));
        assert!(names.contains(&"MyTrait"));
        assert!(names.contains(&"MAX"));
        assert!(names.contains(&"Alias"));
        assert!(names.contains(&"submod"));
    }

    #[test]
    fn extract_symbols_line_numbers() {
        let source = "fn alpha() {}\nfn beta() {}\n";
        let symbols = extract_symbols(source);
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].line, 1);
        assert_eq!(symbols[1].line, 2);
    }

    #[test]
    fn symbol_kind_display() {
        assert_eq!(format!("{}", SymbolKind::Function), "ƒ");
        assert_eq!(format!("{}", SymbolKind::Struct), "S");
        assert_eq!(format!("{}", SymbolKind::Other), "?");
    }

    #[test]
    fn symbol_entry_display() {
        let s = SymbolEntry {
            name: "foo".into(),
            kind: SymbolKind::Function,
            line: 1,
            column: 0,
            container_name: None,
        };
        assert_eq!(s.to_string(), "ƒ foo");

        let s2 = SymbolEntry {
            name: "bar".into(),
            kind: SymbolKind::Struct,
            line: 5,
            column: 0,
            container_name: Some("module".into()),
        };
        assert!(s2.to_string().contains("module.bar"));
    }

    #[test]
    fn file_quick_pick_navigation() {
        let dir = temp_dir("quickpick_nav");
        write_file(&dir, "a.txt", "");
        write_file(&dir, "b.txt", "");
        write_file(&dir, "c.txt", "");

        let mut qp = FileQuickPick::new();
        qp.update(&dir);
        assert!(qp.results.len() >= 3);
        assert_eq!(qp.selected, 0);

        qp.select_next();
        assert_eq!(qp.selected, 1);
        qp.select_previous();
        assert_eq!(qp.selected, 0);
        qp.select_previous(); // wrap
        assert_eq!(qp.selected, qp.results.len() - 1);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn file_quick_pick_recent_files() {
        let mut qp = FileQuickPick::new();
        qp.add_recent(PathBuf::from("/tmp/recent1.txt"));
        qp.add_recent(PathBuf::from("/tmp/recent2.txt"));
        assert_eq!(qp.recent_files.len(), 2);
        // Most recent first
        assert_eq!(qp.recent_files[0], PathBuf::from("/tmp/recent2.txt"));

        // Adding same path moves it to front
        qp.add_recent(PathBuf::from("/tmp/recent1.txt"));
        assert_eq!(qp.recent_files[0], PathBuf::from("/tmp/recent1.txt"));
        assert_eq!(qp.recent_files.len(), 2);
    }

    #[test]
    fn file_quick_pick_selected_path() {
        let mut qp = FileQuickPick::new();
        assert!(qp.selected_path().is_none());
        qp.results = vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")];
        assert_eq!(qp.selected_path().unwrap(), Path::new("a.txt"));
    }

    #[test]
    fn replace_query_new() {
        let q = simple_query("foo");
        let rq = ReplaceQuery::new(q, "bar");
        assert_eq!(rq.replacement, "bar");
        assert_eq!(rq.query.pattern, "foo");
    }

    #[test]
    fn gitignore_patterns_loaded() {
        let dir = temp_dir("gitignore");
        write_file(&dir, ".gitignore", "target\n*.log\n");
        write_file(&dir, "main.rs", "hello");
        write_file(&dir, "target/debug.rs", "hello");
        write_file(&dir, "app.log", "hello");

        let q = simple_query("hello");
        let results = search_files(&q, &dir);
        // Only main.rs should be found
        assert_eq!(results.len(), 1);
        assert!(results[0].matches[0].uri.contains("main.rs"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_files_case_insensitive() {
        let dir = temp_dir("case_insens");
        write_file(&dir, "a.txt", "Hello World\nhello world\nHELLO");

        let q = SearchQuery {
            case_sensitive: false,
            ..simple_query("hello")
        };
        let results = search_files(&q, &dir);
        let total: usize = results.iter().map(|r| r.matches.len()).sum();
        assert_eq!(total, 3);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_files_whole_word() {
        let dir = temp_dir("whole_word_file");
        write_file(&dir, "a.txt", "foo foobar baz foo");

        let q = SearchQuery {
            whole_word: true,
            ..simple_query("foo")
        };
        let results = search_files(&q, &dir);
        let total: usize = results.iter().map(|r| r.matches.len()).sum();
        assert_eq!(total, 2);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_files_regex_mode() {
        let dir = temp_dir("regex_file");
        write_file(&dir, "nums.txt", "abc 123 def\nghi 456 jkl");

        let q = SearchQuery {
            is_regex: true,
            ..simple_query(r"\d+")
        };
        let results = search_files(&q, &dir);
        let total: usize = results.iter().map(|r| r.matches.len()).sum();
        assert_eq!(total, 2);
        fs::remove_dir_all(&dir).ok();
    }
}
