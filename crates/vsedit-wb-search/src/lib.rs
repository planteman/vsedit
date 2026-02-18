//! Workspace text search.
//!
//! Provides file-system-backed search, replace, fuzzy file name matching,
//! and regex-based symbol extraction.

use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// SearchHistory
// ---------------------------------------------------------------------------

/// Tracks recent search queries.
#[derive(Debug, Clone)]
pub struct SearchHistory {
    entries: Vec<String>,
    max_size: usize,
}

impl SearchHistory {
    pub fn new(max_size: usize) -> Self {
        Self { entries: Vec::new(), max_size }
    }

    pub fn push(&mut self, query: impl Into<String>) {
        let query = query.into();
        self.entries.retain(|e| e != &query);
        if self.entries.len() >= self.max_size {
            self.entries.remove(0);
        }
        self.entries.push(query);
    }

    pub fn last(&self) -> Option<&str> {
        self.entries.last().map(|s| s.as_str())
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains(&self, query: &str) -> bool {
        self.entries.iter().any(|e| e == query)
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn recent(&self, n: usize) -> Vec<&str> {
        self.entries.iter().rev().take(n).map(|s| s.as_str()).collect()
    }
}

impl fmt::Display for SearchHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SearchHistory({} entries)", self.entries.len())
    }
}

// ---------------------------------------------------------------------------
// SearchStats
// ---------------------------------------------------------------------------

/// Tracks aggregate search statistics.
#[derive(Debug, Clone, Default)]
pub struct SearchStats {
    pub total_searches: u64,
    pub total_matches: u64,
}

impl SearchStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, match_count: u64) {
        self.total_searches += 1;
        self.total_matches += match_count;
    }

    pub fn average_matches(&self) -> f64 {
        if self.total_searches == 0 {
            0.0
        } else {
            self.total_matches as f64 / self.total_searches as f64
        }
    }
}

impl fmt::Display for SearchStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SearchStats(searches={}, matches={}, avg={:.1})",
            self.total_searches, self.total_matches, self.average_matches()
        )
    }
}

// ---------------------------------------------------------------------------
// SearchFilter
// ---------------------------------------------------------------------------

/// A filter combining include/exclude patterns for file paths.
#[derive(Debug, Clone)]
pub struct SearchFilter {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

impl SearchFilter {
    pub fn new() -> Self {
        Self { include: Vec::new(), exclude: Vec::new() }
    }

    pub fn with_include(mut self, pattern: impl Into<String>) -> Self {
        self.include.push(pattern.into());
        self
    }

    pub fn with_exclude(mut self, pattern: impl Into<String>) -> Self {
        self.exclude.push(pattern.into());
        self
    }

    /// Checks if a path matches the filter rules.
    /// If include patterns exist, the path must match at least one.
    /// If exclude patterns exist, the path must not match any.
    pub fn matches_path(&self, path: &str) -> bool {
        if !self.exclude.is_empty() {
            for pat in &self.exclude {
                if path.contains(pat.as_str()) {
                    return false;
                }
            }
        }
        if self.include.is_empty() {
            return true;
        }
        self.include.iter().any(|pat| path.contains(pat.as_str()))
    }
}

impl Default for SearchFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SearchFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SearchFilter(include={}, exclude={})", self.include.len(), self.exclude.len())
    }
}

// ---------------------------------------------------------------------------
// SearchMatch utilities
// ---------------------------------------------------------------------------

impl SearchMatch {
    /// Create a new search match.
    pub fn new(uri: impl Into<String>, line: u32, column: u32, length: u32, preview: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            line,
            column,
            length,
            preview: preview.into(),
        }
    }

    /// Returns the end column (column + length).
    pub fn end_column(&self) -> u32 {
        self.column + self.length
    }

    /// Returns true if this match overlaps with another on the same line.
    pub fn overlaps(&self, other: &SearchMatch) -> bool {
        self.uri == other.uri
            && self.line == other.line
            && self.column < other.end_column()
            && other.column < self.end_column()
    }
}

// ---------------------------------------------------------------------------
// SearchResult utilities
// ---------------------------------------------------------------------------

impl SearchResult {
    /// Total number of matches.
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// Return the set of unique file URIs that have matches.
    pub fn unique_files(&self) -> Vec<String> {
        let mut seen = Vec::new();
        for m in &self.matches {
            if !seen.contains(&m.uri) {
                seen.push(m.uri.clone());
            }
        }
        seen
    }

    /// Return only matches for a specific file URI.
    pub fn matches_in_file(&self, uri: &str) -> Vec<&SearchMatch> {
        self.matches.iter().filter(|m| m.uri == uri).collect()
    }

    /// Compute per-file match counts.
    pub fn file_match_counts(&self) -> Vec<(String, usize)> {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for m in &self.matches {
            if let Some(entry) = counts.iter_mut().find(|(u, _)| *u == m.uri) {
                entry.1 += 1;
            } else {
                counts.push((m.uri.clone(), 1));
            }
        }
        counts
    }
}

impl fmt::Display for SearchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let files = self.unique_files().len();
        write!(
            f,
            "{} matches in {} files{}",
            self.matches.len(),
            files,
            if self.is_complete { "" } else { " (truncated)" }
        )
    }
}

// ---------------------------------------------------------------------------
// SymbolEntry utilities
// ---------------------------------------------------------------------------

impl SymbolEntry {
    /// Create a new symbol entry.
    pub fn new(name: impl Into<String>, kind: SymbolKind, line: u32, column: u32) -> Self {
        Self {
            name: name.into(),
            kind,
            line,
            column,
            container_name: None,
        }
    }

    /// Set container name (builder pattern).
    pub fn with_container(mut self, container: impl Into<String>) -> Self {
        self.container_name = Some(container.into());
        self
    }

    /// Returns a fully-qualified name: `container.name` or just `name`.
    pub fn qualified_name(&self) -> String {
        match &self.container_name {
            Some(c) => format!("{}.{}", c, self.name),
            None => self.name.clone(),
        }
    }
}

/// Filter symbols by kind.
pub fn filter_symbols(symbols: &[SymbolEntry], kind: SymbolKind) -> Vec<&SymbolEntry> {
    symbols.iter().filter(|s| s.kind == kind).collect()
}

/// Group symbols by kind.
pub fn group_symbols_by_kind(symbols: &[SymbolEntry]) -> Vec<(SymbolKind, Vec<&SymbolEntry>)> {
    let mut groups: Vec<(SymbolKind, Vec<&SymbolEntry>)> = Vec::new();
    for sym in symbols {
        if let Some(entry) = groups.iter_mut().find(|(k, _)| *k == sym.kind) {
            entry.1.push(sym);
        } else {
            groups.push((sym.kind, vec![sym]));
        }
    }
    groups
}

// ---------------------------------------------------------------------------
// SearchHistoryTracker
// ---------------------------------------------------------------------------

/// Tracks search frequency and recency for query suggestions.
///
/// Unlike [`SearchHistory`], this tracker records every invocation so it can
/// report how often each query has been used and surface the most popular
/// searches.
#[derive(Debug, Clone)]
pub struct SearchHistoryTracker {
    /// Ordered log of every recorded search query.
    log: Vec<String>,
    /// Per-query invocation count.
    frequencies: HashMap<String, usize>,
}

impl SearchHistoryTracker {
    /// Create an empty tracker.
    pub fn new() -> Self {
        Self {
            log: Vec::new(),
            frequencies: HashMap::new(),
        }
    }

    /// Record a search query, updating both the log and the frequency map.
    pub fn record_search(&mut self, query: &str) {
        self.log.push(query.to_string());
        *self.frequencies.entry(query.to_string()).or_insert(0) += 1;
    }

    /// Return the number of times `query` has been recorded.
    pub fn frequency(&self, query: &str) -> usize {
        self.frequencies.get(query).copied().unwrap_or(0)
    }

    /// Return the top `n` most frequent queries sorted by count descending.
    pub fn most_frequent(&self, n: usize) -> Vec<(&str, usize)> {
        let mut pairs: Vec<(&str, usize)> = self
            .frequencies
            .iter()
            .map(|(k, &v)| (k.as_str(), v))
            .collect();
        pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        pairs.truncate(n);
        pairs
    }

    /// Return the last `n` unique queries in reverse-chronological order.
    pub fn recent_unique(&self, n: usize) -> Vec<&str> {
        let mut seen = Vec::new();
        for q in self.log.iter().rev() {
            if !seen.contains(&q.as_str()) {
                seen.push(q.as_str());
                if seen.len() == n {
                    break;
                }
            }
        }
        seen
    }

    /// Total number of recorded searches (including duplicates).
    pub fn total_searches(&self) -> usize {
        self.log.len()
    }

    /// Number of distinct queries recorded.
    pub fn unique_count(&self) -> usize {
        self.frequencies.len()
    }
}

impl Default for SearchHistoryTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SearchHistoryTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SearchHistoryTracker({} total, {} unique)",
            self.log.len(),
            self.frequencies.len()
        )
    }
}

// ---------------------------------------------------------------------------
// SearchPreset / SearchPresetManager
// ---------------------------------------------------------------------------

/// A saved search configuration with a human-readable name.
#[derive(Debug, Clone)]
pub struct SearchPreset {
    /// User-facing name for this preset.
    pub name: String,
    /// The query configuration to execute.
    pub query: SearchQuery,
    /// Monotonic counter used as a creation timestamp.
    pub created_at: u64,
}

/// Global counter for assigning monotonically increasing preset ids.
static PRESET_COUNTER: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

impl SearchPreset {
    /// Create a new preset with the given name and query.
    pub fn new(name: &str, query: SearchQuery) -> Self {
        Self {
            name: name.to_string(),
            query,
            created_at: PRESET_COUNTER
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        }
    }
}

impl fmt::Display for SearchPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.name, self.query)
    }
}

/// Manages a collection of named [`SearchPreset`]s.
#[derive(Debug, Clone)]
pub struct SearchPresetManager {
    presets: Vec<SearchPreset>,
}

impl SearchPresetManager {
    /// Create an empty manager.
    pub fn new() -> Self {
        Self {
            presets: Vec::new(),
        }
    }

    /// Add a new preset. If a preset with the same name already exists it is
    /// replaced.
    pub fn add_preset(&mut self, name: &str, query: SearchQuery) {
        self.remove_preset(name);
        self.presets.push(SearchPreset::new(name, query));
    }

    /// Look up a preset by name.
    pub fn get_preset(&self, name: &str) -> Option<&SearchPreset> {
        self.presets.iter().find(|p| p.name == name)
    }

    /// Remove a preset by name, returning `true` if it existed.
    pub fn remove_preset(&mut self, name: &str) -> bool {
        let before = self.presets.len();
        self.presets.retain(|p| p.name != name);
        self.presets.len() < before
    }

    /// List all stored presets ordered by creation time.
    pub fn list_presets(&self) -> Vec<&SearchPreset> {
        self.presets.iter().collect()
    }

    /// Number of stored presets.
    pub fn count(&self) -> usize {
        self.presets.len()
    }
}

impl Default for SearchPresetManager {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SearchPresetManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SearchPresetManager({} presets)", self.presets.len())
    }
}

// ---------------------------------------------------------------------------
// SearchResultRanker
// ---------------------------------------------------------------------------

/// A search result annotated with a relevance score.
#[derive(Debug, Clone)]
pub struct RankedResult {
    /// Path (URI) of the file.
    pub file_path: String,
    /// Computed relevance score.
    pub score: i64,
    /// Number of matches in this file.
    pub match_count: usize,
}

impl fmt::Display for RankedResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (score={}, matches={})",
            self.file_path, self.score, self.match_count
        )
    }
}

/// Ranks search results by a simple heuristic: more matches and shorter file
/// paths score higher.
pub struct SearchResultRanker;

impl SearchResultRanker {
    /// Rank a slice of [`SearchResult`]s, producing one [`RankedResult`] per
    /// unique file URI.  Score = `match_count * 10 + (200 - file_path_len)`.
    pub fn rank(results: &[SearchResult]) -> Vec<RankedResult> {
        let mut file_counts: Vec<(String, usize)> = Vec::new();
        for result in results {
            for m in &result.matches {
                if let Some(entry) = file_counts.iter_mut().find(|(u, _)| *u == m.uri) {
                    entry.1 += 1;
                } else {
                    file_counts.push((m.uri.clone(), 1));
                }
            }
        }

        let mut ranked: Vec<RankedResult> = file_counts
            .into_iter()
            .map(|(path, count)| {
                let length_penalty = 200_i64.saturating_sub(path.len() as i64);
                RankedResult {
                    file_path: path,
                    score: (count as i64) * 10 + length_penalty,
                    match_count: count,
                }
            })
            .collect();

        ranked.sort_by(|a, b| b.score.cmp(&a.score));
        ranked
    }

    /// Return only the top `n` ranked results.
    pub fn top_n(results: &[SearchResult], n: usize) -> Vec<RankedResult> {
        let mut ranked = Self::rank(results);
        ranked.truncate(n);
        ranked
    }
}


// ---------------------------------------------------------------------------
// SearchExcludePatternEditor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SearchExcludePatternEditor {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl SearchExcludePatternEditor {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for SearchExcludePatternEditor {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for SearchExcludePatternEditor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "SearchExcludePatternEditor({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// SearchRegexValidator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SearchRegexValidator {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl SearchRegexValidator {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for SearchRegexValidator {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for SearchRegexValidator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "SearchRegexValidator({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// SearchExcludePatternEditorSnapshot — point-in-time snapshot of SearchExcludePatternEditor state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SearchExcludePatternEditorSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl SearchExcludePatternEditorSnapshot {
    pub fn capture(source: &SearchExcludePatternEditor, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for SearchExcludePatternEditorSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// SearchRegexValidatorStats — aggregate statistics for SearchRegexValidator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct SearchRegexValidatorStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl SearchRegexValidatorStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for SearchRegexValidatorStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// SearchExcludePatternEditorConfig — configuration for SearchExcludePatternEditor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SearchExcludePatternEditorConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl SearchExcludePatternEditorConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for SearchExcludePatternEditorConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for SearchExcludePatternEditorConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// SearchResultGrouper
// ---------------------------------------------------------------------------

/// Group search results by file.
#[derive(Debug, Clone)]
pub struct SearchResultGroup {
    pub file: String,
    pub matches: Vec<String>,
    pub collapsed: bool,
}

#[derive(Debug, Clone)]
pub struct SearchResultGrouper {
    groups: Vec<SearchResultGroup>,
    max_results_per_file: usize,
}

impl SearchResultGrouper {
    pub fn new(max_results_per_file: usize) -> Self {
        Self {
            groups: Vec::new(),
            max_results_per_file,
        }
    }

    pub fn add_match(&mut self, file: &str, match_text: &str) {
        if let Some(group) = self.groups.iter_mut().find(|g| g.file == file) {
            if group.matches.len() < self.max_results_per_file {
                group.matches.push(match_text.to_string());
            }
        } else {
            self.groups.push(SearchResultGroup {
                file: file.to_string(),
                matches: vec![match_text.to_string()],
                collapsed: false,
            });
        }
    }

    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    pub fn total_matches(&self) -> usize {
        self.groups.iter().map(|g| g.matches.len()).sum()
    }

    pub fn files_with_matches(&self) -> Vec<&str> {
        self.groups.iter().map(|g| g.file.as_str()).collect()
    }

    pub fn collapse_group(&mut self, file: &str) {
        if let Some(g) = self.groups.iter_mut().find(|g| g.file == file) {
            g.collapsed = true;
        }
    }

    pub fn expand_group(&mut self, file: &str) {
        if let Some(g) = self.groups.iter_mut().find(|g| g.file == file) {
            g.collapsed = false;
        }
    }

    pub fn file_groups(&self) -> &[SearchResultGroup] {
        &self.groups
    }
}

// ---------------------------------------------------------------------------
// SearchReplacePreview
// ---------------------------------------------------------------------------

/// Preview search-and-replace operations.
#[derive(Debug, Clone)]
pub struct ReplacePair {
    pub original_line: String,
    pub replaced_line: String,
}

#[derive(Debug, Clone)]
pub struct SearchReplacePreview {
    pairs: Vec<(String, ReplacePair)>,
}

impl SearchReplacePreview {
    pub fn new() -> Self {
        Self { pairs: Vec::new() }
    }

    pub fn add_replacement(&mut self, file: &str, original: &str, replaced: &str) {
        self.pairs.push((
            file.to_string(),
            ReplacePair {
                original_line: original.to_string(),
                replaced_line: replaced.to_string(),
            },
        ));
    }

    pub fn total_replacements(&self) -> usize {
        self.pairs.len()
    }

    pub fn files_affected(&self) -> Vec<String> {
        let mut files: Vec<String> = self.pairs.iter().map(|(f, _)| f.clone()).collect();
        files.sort();
        files.dedup();
        files
    }

    pub fn estimate_diff_size(&self) -> usize {
        self.pairs
            .iter()
            .map(|(_, p)| p.original_line.len() + p.replaced_line.len() + 10)
            .sum()
    }
}

// ---------------------------------------------------------------------------
// SearchExcludePattern
// ---------------------------------------------------------------------------

/// Manage exclude patterns for search.
#[derive(Debug, Clone)]
pub struct SearchExcludePatternManager {
    patterns: Vec<(String, bool)>,
}

impl SearchExcludePatternManager {
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    pub fn with_defaults() -> Self {
        let mut mgr = Self::new();
        mgr.add_pattern("node_modules");
        mgr.add_pattern(".git");
        mgr.add_pattern("target");
        mgr
    }

    pub fn add_pattern(&mut self, pattern: &str) {
        if !self.patterns.iter().any(|(p, _)| p == pattern) {
            self.patterns.push((pattern.to_string(), true));
        }
    }

    pub fn remove_pattern(&mut self, pattern: &str) {
        self.patterns.retain(|(p, _)| p != pattern);
    }

    pub fn toggle_pattern(&mut self, pattern: &str) {
        if let Some(entry) = self.patterns.iter_mut().find(|(p, _)| p == pattern) {
            entry.1 = !entry.1;
        }
    }

    pub fn is_excluded(&self, path: &str) -> bool {
        self.patterns
            .iter()
            .filter(|(_, active)| *active)
            .any(|(p, _)| path.contains(p))
    }

    pub fn active_patterns(&self) -> Vec<&str> {
        self.patterns
            .iter()
            .filter(|(_, active)| *active)
            .map(|(p, _)| p.as_str())
            .collect()
    }

    pub fn default_excludes() -> Vec<&'static str> {
        vec!["node_modules", ".git", "target"]
    }
}


/// Configuration manager for wb_search functionality.
pub struct WbSearchConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl WbSearchConfig {
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

    pub fn merge(&mut self, other: &WbSearchConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for wb_search operations.
pub struct WbSearchRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl WbSearchRateTracker {
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

/// Validation result collector for wb_search.
pub struct WbSearchValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl WbSearchValidator {
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

    pub fn merge(&mut self, other: &WbSearchValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Workspace search coordination — extended utilities (qf)
// ---------------------------------------------------------------------------

/// Metric accumulator for wb_search operations.
#[derive(Debug, Clone)]
pub struct QfMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QfMetrics {
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

/// Sliding-window rate counter for wb_search.
#[derive(Debug, Clone)]
pub struct QfRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QfRateWindow {
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

/// A small LRU-style cache for wb_search lookups.
#[derive(Debug, Clone)]
pub struct QfLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QfLruCache {
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
// xa_ extended helpers for wb_search
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWbSearchRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWbSearchRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaWbSearchCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWbSearchCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaWbSearchCounter {
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

    #[test]
    fn test_search_history_push_dedup() {
        let mut hist = SearchHistory::new(5);
        hist.push("hello");
        hist.push("world");
        hist.push("hello"); // moves to end
        assert_eq!(hist.len(), 2);
        assert_eq!(hist.last(), Some("hello"));
        assert!(hist.contains("world"));
        assert!(format!("{hist}").contains("2 entries"));
    }

    #[test]
    fn test_search_history_overflow() {
        let mut hist = SearchHistory::new(2);
        hist.push("a");
        hist.push("b");
        hist.push("c");
        assert_eq!(hist.len(), 2);
        assert!(!hist.contains("a"));
        assert!(hist.contains("b"));
        assert!(hist.contains("c"));
    }

    #[test]
    fn test_search_history_recent() {
        let mut hist = SearchHistory::new(10);
        hist.push("a");
        hist.push("b");
        hist.push("c");
        let recent = hist.recent(2);
        assert_eq!(recent, vec!["c", "b"]);
    }

    #[test]
    fn test_search_stats() {
        let mut stats = SearchStats::new();
        stats.record(10);
        stats.record(20);
        assert_eq!(stats.total_searches, 2);
        assert_eq!(stats.total_matches, 30);
        assert!((stats.average_matches() - 15.0).abs() < f64::EPSILON);
        assert!(format!("{stats}").contains("avg=15.0"));
    }

    #[test]
    fn test_search_stats_empty() {
        let stats = SearchStats::new();
        assert!((stats.average_matches() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_search_filter_matches() {
        let filter = SearchFilter::new()
            .with_include(".rs")
            .with_exclude("target");
        assert!(filter.matches_path("src/main.rs"));
        assert!(!filter.matches_path("target/debug/main.rs"));
        assert!(!filter.matches_path("src/main.py"));
        assert!(format!("{filter}").contains("include=1"));
    }

    #[test]
    fn test_search_match_end_column_and_overlap() {
        let a = SearchMatch::new("file.rs", 10, 5, 3, "foo");
        assert_eq!(a.end_column(), 8);
        let b = SearchMatch::new("file.rs", 10, 7, 4, "obar");
        assert!(a.overlaps(&b));
        let c = SearchMatch::new("file.rs", 10, 8, 2, "ba");
        assert!(!a.overlaps(&c));
        let d = SearchMatch::new("other.rs", 10, 5, 3, "foo");
        assert!(!a.overlaps(&d));
    }

    #[test]
    fn test_search_result_unique_files_and_counts() {
        let result = SearchResult {
            matches: vec![
                SearchMatch::new("a.rs", 1, 0, 3, "foo"),
                SearchMatch::new("a.rs", 5, 0, 3, "foo"),
                SearchMatch::new("b.rs", 2, 0, 3, "foo"),
            ],
            is_complete: true,
        };
        assert_eq!(result.match_count(), 3);
        assert_eq!(result.unique_files(), vec!["a.rs", "b.rs"]);
        let counts = result.file_match_counts();
        assert_eq!(counts.iter().find(|(u, _)| u == "a.rs").unwrap().1, 2);
        assert_eq!(result.matches_in_file("b.rs").len(), 1);
    }

    #[test]
    fn test_search_result_display() {
        let result = SearchResult {
            matches: vec![
                SearchMatch::new("a.rs", 1, 0, 3, "foo"),
            ],
            is_complete: false,
        };
        let s = format!("{}", result);
        assert!(s.contains("1 matches"));
        assert!(s.contains("truncated"));
    }

    #[test]
    fn test_symbol_entry_builder_and_qualified() {
        let sym = SymbolEntry::new("process", SymbolKind::Function, 10, 4)
            .with_container("MyStruct");
        assert_eq!(sym.qualified_name(), "MyStruct.process");
        let sym2 = SymbolEntry::new("main", SymbolKind::Function, 1, 0);
        assert_eq!(sym2.qualified_name(), "main");
    }

    #[test]
    fn test_filter_symbols_by_kind() {
        let syms = vec![
            SymbolEntry::new("foo", SymbolKind::Function, 1, 0),
            SymbolEntry::new("Bar", SymbolKind::Struct, 5, 0),
            SymbolEntry::new("baz", SymbolKind::Function, 10, 0),
        ];
        let fns = filter_symbols(&syms, SymbolKind::Function);
        assert_eq!(fns.len(), 2);
        let structs = filter_symbols(&syms, SymbolKind::Struct);
        assert_eq!(structs.len(), 1);
    }

    #[test]
    fn test_group_symbols_by_kind() {
        let syms = vec![
            SymbolEntry::new("a", SymbolKind::Function, 1, 0),
            SymbolEntry::new("B", SymbolKind::Struct, 5, 0),
            SymbolEntry::new("c", SymbolKind::Function, 10, 0),
        ];
        let groups = group_symbols_by_kind(&syms);
        assert_eq!(groups.len(), 2);
        let fn_group = groups.iter().find(|(k, _)| *k == SymbolKind::Function).unwrap();
        assert_eq!(fn_group.1.len(), 2);
    }

    // -----------------------------------------------------------------------
    // SearchHistoryTracker tests
    // -----------------------------------------------------------------------

    #[test]
    fn tracker_records_frequency() {
        let mut t = SearchHistoryTracker::new();
        t.record_search("foo");
        t.record_search("bar");
        t.record_search("foo");
        assert_eq!(t.frequency("foo"), 2);
        assert_eq!(t.frequency("bar"), 1);
        assert_eq!(t.frequency("baz"), 0);
        assert_eq!(t.total_searches(), 3);
        assert_eq!(t.unique_count(), 2);
    }

    #[test]
    fn tracker_most_frequent() {
        let mut t = SearchHistoryTracker::new();
        t.record_search("a");
        t.record_search("b");
        t.record_search("a");
        t.record_search("c");
        t.record_search("b");
        t.record_search("a");
        let top = t.most_frequent(2);
        assert_eq!(top[0], ("a", 3));
        assert_eq!(top[1], ("b", 2));
    }

    #[test]
    fn tracker_recent_unique() {
        let mut t = SearchHistoryTracker::new();
        t.record_search("x");
        t.record_search("y");
        t.record_search("x");
        t.record_search("z");
        let recent = t.recent_unique(3);
        assert_eq!(recent, vec!["z", "x", "y"]);
    }

    #[test]
    fn tracker_display() {
        let mut t = SearchHistoryTracker::new();
        t.record_search("hello");
        let s = format!("{t}");
        assert!(s.contains("1 total"));
        assert!(s.contains("1 unique"));
    }

    // -----------------------------------------------------------------------
    // SearchPreset / SearchPresetManager tests
    // -----------------------------------------------------------------------

    #[test]
    fn preset_display() {
        let q = simple_query("TODO");
        let p = SearchPreset::new("todos", q);
        let s = format!("{p}");
        assert!(s.contains("[todos]"));
        assert!(s.contains("TODO"));
        assert!(p.created_at > 0);
    }

    #[test]
    fn preset_manager_add_get_remove() {
        let mut mgr = SearchPresetManager::new();
        mgr.add_preset("rust-only", SearchQueryBuilder::new("fn ")
            .include("*.rs")
            .build());
        mgr.add_preset("todo", simple_query("TODO"));
        assert_eq!(mgr.count(), 2);

        let p = mgr.get_preset("rust-only").unwrap();
        assert_eq!(p.query.pattern, "fn ");
        assert!(mgr.get_preset("nope").is_none());

        assert!(mgr.remove_preset("todo"));
        assert_eq!(mgr.count(), 1);
        assert!(!mgr.remove_preset("todo")); // already gone
    }

    #[test]
    fn preset_manager_replace_duplicate_name() {
        let mut mgr = SearchPresetManager::new();
        mgr.add_preset("p", simple_query("old"));
        mgr.add_preset("p", simple_query("new"));
        assert_eq!(mgr.count(), 1);
        assert_eq!(mgr.get_preset("p").unwrap().query.pattern, "new");
    }

    #[test]
    fn preset_manager_list_and_display() {
        let mut mgr = SearchPresetManager::new();
        mgr.add_preset("a", simple_query("alpha"));
        mgr.add_preset("b", simple_query("beta"));
        let list = mgr.list_presets();
        assert_eq!(list.len(), 2);
        let s = format!("{mgr}");
        assert!(s.contains("2 presets"));
    }

    // -----------------------------------------------------------------------
    // SearchResultRanker tests
    // -----------------------------------------------------------------------

    #[test]
    fn ranker_basic_scoring() {
        let results = vec![
            SearchResult {
                matches: vec![
                    SearchMatch::new("short.rs", 1, 0, 3, "foo"),
                    SearchMatch::new("short.rs", 5, 0, 3, "foo"),
                ],
                is_complete: true,
            },
            SearchResult {
                matches: vec![
                    SearchMatch::new("very/long/path/to/file.rs", 1, 0, 3, "foo"),
                ],
                is_complete: true,
            },
        ];
        let ranked = SearchResultRanker::rank(&results);
        assert_eq!(ranked.len(), 2);
        // short.rs has 2 matches => higher score
        assert_eq!(ranked[0].file_path, "short.rs");
        assert_eq!(ranked[0].match_count, 2);
        assert!(ranked[0].score > ranked[1].score);
    }

    #[test]
    fn ranker_top_n() {
        let results = vec![SearchResult {
            matches: vec![
                SearchMatch::new("a.rs", 1, 0, 2, "hi"),
                SearchMatch::new("b.rs", 1, 0, 2, "hi"),
                SearchMatch::new("c.rs", 1, 0, 2, "hi"),
            ],
            is_complete: true,
        }];
        let top = SearchResultRanker::top_n(&results, 2);
        assert_eq!(top.len(), 2);
    }

    #[test]
    fn ranked_result_display() {
        let r = RankedResult {
            file_path: "src/lib.rs".into(),
            score: 42,
            match_count: 3,
        };
        let s = format!("{r}");
        assert!(s.contains("score=42"));
        assert!(s.contains("matches=3"));
    }

    #[test]
    fn ranker_empty_input() {
        let ranked = SearchResultRanker::rank(&[]);
        assert!(ranked.is_empty());
        let top = SearchResultRanker::top_n(&[], 5);
        assert!(top.is_empty());
    }

    #[test] fn searchExcludePatternEditor_new() { let s = SearchExcludePatternEditor::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn searchExcludePatternEditor_add() { let mut s = SearchExcludePatternEditor::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn searchExcludePatternEditor_remove() { let mut s = SearchExcludePatternEditor::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn searchExcludePatternEditor_config() { let mut s = SearchExcludePatternEditor::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn searchExcludePatternEditor_nav() { let mut s = SearchExcludePatternEditor::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn searchExcludePatternEditor_filter() { let mut s = SearchExcludePatternEditor::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn searchExcludePatternEditor_display() { assert!(format!("{}", SearchExcludePatternEditor::new()).contains("SearchExcludePatternEditor")); }
    #[test] fn searchRegexValidator_new() { let s = SearchRegexValidator::new(); assert!(s.is_empty()); }
    #[test] fn searchRegexValidator_add() { let mut s = SearchRegexValidator::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn searchRegexValidator_active() { let mut s = SearchRegexValidator::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn searchRegexValidator_error() { let mut s = SearchRegexValidator::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn searchRegexValidator_rm_group() { let mut s = SearchRegexValidator::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn searchRegexValidator_display() { assert!(format!("{}", SearchRegexValidator::new()).contains("SearchRegexValidator")); }


    #[test] fn searchExcludePatternEditor_snap_capture() {
        let s = SearchExcludePatternEditor::new();
        let snap = SearchExcludePatternEditorSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn searchExcludePatternEditor_snap_stale() {
        let s = SearchExcludePatternEditor::new();
        let snap = SearchExcludePatternEditorSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn searchExcludePatternEditor_snap_diff() {
        let s = SearchExcludePatternEditor::new();
        let s1v = SearchExcludePatternEditorSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn searchExcludePatternEditor_snap_display() {
        let s = SearchExcludePatternEditor::new();
        let snap = SearchExcludePatternEditorSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn searchRegexValidator_stats_record() {
        let mut st = SearchRegexValidatorStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn searchRegexValidator_stats_hit_ratio() {
        let mut st = SearchRegexValidatorStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn searchRegexValidator_stats_merge() {
        let mut a = SearchRegexValidatorStats::new();
        a.total_adds = 5;
        let mut b = SearchRegexValidatorStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn searchRegexValidator_stats_display() {
        let st = SearchRegexValidatorStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn searchExcludePatternEditor_config_default() {
        let c = SearchExcludePatternEditorConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn searchExcludePatternEditor_config_builder() {
        let c = SearchExcludePatternEditorConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn searchExcludePatternEditor_config_labels() {
        let mut c = SearchExcludePatternEditorConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn searchExcludePatternEditor_config_cleanup_threshold() {
        let c = SearchExcludePatternEditorConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn searchExcludePatternEditor_config_display() {
        assert!(format!("{}", SearchExcludePatternEditorConfig::new()).contains("Config"));
    }
    #[test] fn searchRegexValidator_stats_peaks() {
        let mut st = SearchRegexValidatorStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- SearchResultGrouper -----------------------------------------------

    #[test]
    fn grouper_add_match() {
        let mut g = SearchResultGrouper::new(100);
        g.add_match("src/main.rs", "fn main()");
        g.add_match("src/main.rs", "fn helper()");
        assert_eq!(g.group_count(), 1);
        assert_eq!(g.total_matches(), 2);
    }

    #[test]
    fn grouper_multiple_files() {
        let mut g = SearchResultGrouper::new(100);
        g.add_match("a.rs", "line1");
        g.add_match("b.rs", "line2");
        assert_eq!(g.group_count(), 2);
        assert_eq!(g.files_with_matches(), vec!["a.rs", "b.rs"]);
    }

    #[test]
    fn grouper_max_per_file() {
        let mut g = SearchResultGrouper::new(2);
        g.add_match("f.rs", "a");
        g.add_match("f.rs", "b");
        g.add_match("f.rs", "c");
        assert_eq!(g.total_matches(), 2);
    }

    #[test]
    fn grouper_collapse_expand() {
        let mut g = SearchResultGrouper::new(100);
        g.add_match("f.rs", "x");
        g.collapse_group("f.rs");
        assert!(g.file_groups()[0].collapsed);
        g.expand_group("f.rs");
        assert!(!g.file_groups()[0].collapsed);
    }

    // -- SearchReplacePreview ----------------------------------------------

    #[test]
    fn replace_preview_basic() {
        let mut p = SearchReplacePreview::new();
        p.add_replacement("f.rs", "old_fn()", "new_fn()");
        assert_eq!(p.total_replacements(), 1);
        assert_eq!(p.files_affected(), vec!["f.rs"]);
    }

    #[test]
    fn replace_preview_multiple_files() {
        let mut p = SearchReplacePreview::new();
        p.add_replacement("a.rs", "x", "y");
        p.add_replacement("b.rs", "x", "y");
        assert_eq!(p.files_affected().len(), 2);
    }

    #[test]
    fn replace_preview_diff_size() {
        let mut p = SearchReplacePreview::new();
        p.add_replacement("f.rs", "hello", "world");
        assert!(p.estimate_diff_size() > 0);
    }

    // -- SearchExcludePatternManager ---------------------------------------

    #[test]
    fn exclude_pattern_basic() {
        let mut mgr = SearchExcludePatternManager::new();
        mgr.add_pattern("node_modules");
        assert!(mgr.is_excluded("project/node_modules/pkg/index.js"));
        assert!(!mgr.is_excluded("src/main.rs"));
    }

    #[test]
    fn exclude_pattern_toggle() {
        let mut mgr = SearchExcludePatternManager::new();
        mgr.add_pattern(".git");
        mgr.toggle_pattern(".git");
        assert!(!mgr.is_excluded(".git/config"));
    }

    #[test]
    fn exclude_pattern_defaults() {
        let mgr = SearchExcludePatternManager::with_defaults();
        assert!(mgr.is_excluded("node_modules/foo"));
        assert!(mgr.is_excluded(".git/HEAD"));
        assert!(mgr.is_excluded("target/debug/bin"));
    }

    #[test]
    fn exclude_pattern_remove() {
        let mut mgr = SearchExcludePatternManager::with_defaults();
        mgr.remove_pattern("target");
        assert!(!mgr.is_excluded("target/release/bin"));
    }


    #[test]
    fn wb_search_config_new() {
        let cfg = WbSearchConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn wb_search_config_set_get() {
        let mut cfg = WbSearchConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn wb_search_config_remove() {
        let mut cfg = WbSearchConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn wb_search_config_keys_sorted() {
        let mut cfg = WbSearchConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn wb_search_config_bump_version() {
        let mut cfg = WbSearchConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn wb_search_config_clear() {
        let mut cfg = WbSearchConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn wb_search_config_merge() {
        let mut cfg1 = WbSearchConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = WbSearchConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn wb_search_config_disable() {
        let mut cfg = WbSearchConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn wb_search_rate_tracker_empty() {
        let rt = WbSearchRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn wb_search_rate_tracker_record() {
        let mut rt = WbSearchRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn wb_search_rate_tracker_prune() {
        let mut rt = WbSearchRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn wb_search_validator_valid() {
        let v = WbSearchValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn wb_search_validator_errors() {
        let mut v = WbSearchValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn wb_search_validator_clear() {
        let mut v = WbSearchValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn wb_search_validator_merge() {
        let mut v1 = WbSearchValidator::new();
        v1.add_error("e1");
        let mut v2 = WbSearchValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn wb_search_rate_tracker_clear() {
        let mut rt = WbSearchRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn qf_metrics_empty() {
        let m = QfMetrics::new("wb_search");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qf_metrics_record_and_mean() {
        let mut m = QfMetrics::new("wb_search");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qf_metrics_min_max() {
        let mut m = QfMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qf_metrics_variance_and_std() {
        let mut m = QfMetrics::new("v");
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
    fn qf_metrics_percentile() {
        let mut m = QfMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qf_metrics_merge() {
        let mut a = QfMetrics::new("a");
        a.record(1.0);
        let mut b = QfMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qf_metrics_reset() {
        let mut m = QfMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qf_rate_window_empty() {
        let rw = QfRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qf_rate_window_tick_and_rate() {
        let mut rw = QfRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qf_lru_cache_basic() {
        let mut c = QfLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qf_lru_cache_contains_and_keys() {
        let mut c = QfLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qf_lru_cache_remove() {
        let mut c = QfLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qf_metrics_sum() {
        let mut m = QfMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qf_metrics_label() {
        let m = QfMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qf_lru_cache_clear() {
        let mut c = QfLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for wb_search
    #[test]
    fn xa_wb_search_ring_new() {
        let rb = super::XaWbSearchRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_wb_search_ring_push_len() {
        let mut rb = super::XaWbSearchRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_wb_search_ring_wrap() {
        let mut rb = super::XaWbSearchRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_wb_search_ring_mean_empty() {
        let rb = super::XaWbSearchRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_wb_search_ring_mean_values() {
        let mut rb = super::XaWbSearchRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_wb_search_ring_min_max() {
        let mut rb = super::XaWbSearchRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_wb_search_ring_iter() {
        let mut rb = super::XaWbSearchRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_wb_search_counter_new() {
        let c = super::XaWbSearchCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_search_counter_inc() {
        let mut c = super::XaWbSearchCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_wb_search_counter_inc_by() {
        let mut c = super::XaWbSearchCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_wb_search_counter_reset() {
        let mut c = super::XaWbSearchCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_wb_search_counter_clear() {
        let mut c = super::XaWbSearchCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_search_counter_default() {
        let c = super::XaWbSearchCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }

}
