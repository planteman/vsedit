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


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 224
// ---------------------------------------------------------------------------

/// Generic object pool `Xc224Pool<T>`.
pub struct Xc224Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc224Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc224PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc224Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc224PoolStats {
        Xc224PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc224Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc224Scheduler`.
pub struct Xc224Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc224Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc224Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_224 hash for the given byte slice.
pub fn xc_224_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_224 convention.
pub fn xc_224_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe1 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe1Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe1PipelineError {
    pub stage: Xe1Stage,
    pub message: String,
}

impl std::fmt::Display for Xe1PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe1Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe1Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe1PipelineError>>>,
    stage_names: Vec<Xe1Stage>,
}

impl Xe1Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe1PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe1Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe1PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe1Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe1PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe1Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe1PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe1Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe1PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe1Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe1CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe1CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe1Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe1CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe1CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe1Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe1CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_1_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe1CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_1_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe1CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_1_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe1PipelineError> {
    Ok(data)
}

pub fn xe_1_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe1PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_1_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe1PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_1_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe1PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_1_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe1PipelineError> {
    Err(Xe1PipelineError {
        stage: Xe1Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #61
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf61Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf61TrieNode {
    children: std::collections::HashMap<char, Xf61TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf61Trie {
    root: Xf61TrieNode,
    count: usize,
}

impl Xf61Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf61TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf61TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf61TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf61BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf61BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 223).
pub struct Xh223SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh223SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 265 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 223).
pub struct Xh223BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh223BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 223).
pub struct Xi223Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi223Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi223Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi223Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 223).
pub struct Xi223IntervalTree {
    xi_intervals: Vec<Xi223Interval>,
}

impl Xi223IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi223Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi223Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi223Interval) -> Vec<&Xi223Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi223Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi223Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi223Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi223Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi223Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi223Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 223) ---

/// Disjoint set / union-find for crate 223.
pub struct Xj223UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj223UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ223_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 223.
pub struct Xj223BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj223BTreeNode<K, V>>>,
    len: usize,
}

struct Xj223BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj223BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj223BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ223_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ223_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj223BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj223BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj223BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj223BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_223 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk223SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk223SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk223DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk223DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_223).
#[derive(Debug, Clone)]
pub struct Xl223Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl223Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_223).
#[derive(Debug, Clone)]
pub struct Xl223SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl223SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm223MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm223MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm223Tokenizer {
    text: String,
}

impl Xm223Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 223.
pub struct Xn223Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn223Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 223 -----

#[derive(Debug, Clone)]
struct Xn223AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn223AvlNode<K, V>>>,
    right: Option<Box<Xn223AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 223.
#[derive(Debug, Clone)]
pub struct Xn223AVL<K, V> {
    root: Option<Box<Xn223AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn223AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn223AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn223AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn223AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn223AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn223AvlNode<K, V>>) -> Box<Xn223AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn223AvlNode<K, V>>) -> Box<Xn223AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn223AvlNode<K, V>>) -> Box<Xn223AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn223AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn223AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn223AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn223AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn223AvlNode<K, V>>) -> &Xn223AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn223AvlNode<K, V>>) -> (Box<Xn223AvlNode<K, V>>, Option<Box<Xn223AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn223AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn223AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn223AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn223AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn223AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn223AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn223AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
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


    // ---- xc_ pool / scheduler tests – block 224 ----

    #[test]
    fn xc_224_pool_new_empty() {
        let pool: super::Xc224Pool<i32> = super::Xc224Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_224_pool_release_acquire() {
        let mut pool = super::Xc224Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_224_pool_acquire_empty() {
        let mut pool: super::Xc224Pool<i32> = super::Xc224Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_224_pool_full() {
        let mut pool = super::Xc224Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_224_pool_drain() {
        let mut pool = super::Xc224Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_224_pool_stats() {
        let mut pool = super::Xc224Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_224_pool_clear() {
        let mut pool = super::Xc224Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_224_pool_shrink() {
        let mut pool = super::Xc224Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_224_pool_default() {
        let pool: super::Xc224Pool<String> = super::Xc224Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_224_pool_extend() {
        let mut pool = super::Xc224Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_224_pool_retain() {
        let mut pool = super::Xc224Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_224_scheduler_round_robin() {
        let mut sched = super::Xc224Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_224_scheduler_empty() {
        let mut sched = super::Xc224Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_224_scheduler_reset() {
        let mut sched = super::Xc224Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_224_scheduler_add_remove() {
        let mut sched = super::Xc224Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_224_scheduler_targets() {
        let sched = super::Xc224Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_224_hash_empty() {
        assert_eq!(super::xc_224_hash(b""), 5381);
    }

    #[test]
    fn xc_224_hash_data() {
        let h = super::xc_224_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_224_hash(b"hello"), h);
    }

    #[test]
    fn xc_224_reverse_str() {
        assert_eq!(super::xc_224_reverse("abc"), "cba");
        assert_eq!(super::xc_224_reverse(""), "");
    }


    #[test]
    fn xe_1_pipeline_empty() {
        let p = super::Xe1Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_1_pipeline_parse_stage() {
        let p = super::Xe1Pipeline::new()
            .add_parse(super::xe_1_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_1_pipeline_transform_double() {
        let p = super::Xe1Pipeline::new()
            .add_transform(super::xe_1_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_1_pipeline_validate_reverse() {
        let p = super::Xe1Pipeline::new()
            .add_validate(super::xe_1_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_1_pipeline_emit_filter() {
        let p = super::Xe1Pipeline::new()
            .add_emit(super::xe_1_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_1_pipeline_multi_stage() {
        let p = super::Xe1Pipeline::new()
            .add_parse(super::xe_1_pipeline_identity)
            .add_transform(super::xe_1_pipeline_double)
            .add_validate(super::xe_1_pipeline_reverse)
            .add_emit(super::xe_1_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_1_pipeline_error_propagation() {
        let p = super::Xe1Pipeline::new()
            .add_parse(super::xe_1_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe1Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_1_pipeline_compose() {
        let p1 = super::Xe1Pipeline::new()
            .add_parse(super::xe_1_pipeline_identity);
        let p2 = super::Xe1Pipeline::new()
            .add_transform(super::xe_1_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_1_pipeline_error_display() {
        let e = super::Xe1PipelineError {
            stage: super::Xe1Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_1_cache_put_get() {
        let mut c = super::Xe1Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_1_cache_miss() {
        let mut c: super::Xe1Cache<&str, i32> = super::Xe1Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_1_cache_ttl_expiry() {
        let mut c = super::Xe1Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_1_cache_evict() {
        let mut c = super::Xe1Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_1_cache_capacity() {
        let mut c = super::Xe1Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_1_cache_stats() {
        let mut c = super::Xe1Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_1_cache_clear() {
        let mut c = super::Xe1Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #61 --

    #[test]
    fn xf61_trie_insert_search() {
        let mut t = Xf61Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf61_trie_starts_with() {
        let mut t = Xf61Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf61_trie_remove() {
        let mut t = Xf61Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf61_trie_word_count() {
        let mut t = Xf61Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf61_trie_longest_prefix() {
        let mut t = Xf61Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf61_trie_all_words() {
        let mut t = Xf61Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf61_trie_autocomplete() {
        let mut t = Xf61Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf61_trie_empty_search() {
        let t = Xf61Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf61_bloom_add_contains() {
        let mut bf = Xf61BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf61_bloom_probably_absent() {
        let bf = Xf61BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf61_bloom_false_positive_rate() {
        let mut bf = Xf61BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf61_bloom_clear() {
        let mut bf = Xf61BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf61_bloom_union() {
        let mut a = Xf61BloomFilter::xf_new(512, 2);
        let mut b = Xf61BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf61_bloom_intersection_estimate() {
        let mut a = Xf61BloomFilter::xf_new(512, 2);
        let mut b = Xf61BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf61_bloom_union_size_mismatch() {
        let a = Xf61BloomFilter::xf_new(256, 2);
        let b = Xf61BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh223_skip_insert_contains() {
        let mut sl = super::Xh223SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh223_skip_remove() {
        let mut sl = super::Xh223SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh223_skip_len() {
        let mut sl = super::Xh223SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh223_skip_range_query() {
        let mut sl = super::Xh223SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh223_skip_floor_ceiling() {
        let mut sl = super::Xh223SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh223_skip_rank() {
        let mut sl = super::Xh223SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh223_skip_empty() {
        let sl = super::Xh223SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh223_skip_duplicates() {
        let mut sl = super::Xh223SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh223_bitset_set_test() {
        let mut bs = super::Xh223BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh223_bitset_clear_count() {
        let mut bs = super::Xh223BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh223_bitset_and_or_xor() {
        let mut a = super::Xh223BitSet::xh_new(128);
        let mut b = super::Xh223BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh223_bitset_iter_ones() {
        let mut bs = super::Xh223BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh223_bitset_first_last() {
        let mut bs = super::Xh223BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh223_bitset_empty() {
        let bs = super::Xh223BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi223_deque_push_pop_back() {
        let mut dq = super::Xi223Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi223_deque_push_pop_front() {
        let mut dq = super::Xi223Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi223_deque_mixed_ops() {
        let mut dq = super::Xi223Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi223_deque_get_and_split() {
        let mut dq = super::Xi223Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi223_deque_rotate_left() {
        let mut dq = super::Xi223Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi223_deque_rotate_right() {
        let mut dq = super::Xi223Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi223_deque_grow() {
        let mut dq = super::Xi223Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi223_deque_empty() {
        let dq = super::Xi223Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi223_interval_tree_insert_query() {
        let mut tree = super::Xi223IntervalTree::xi_new();
        tree.xi_insert(super::Xi223Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi223Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi223Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi223_interval_tree_overlap() {
        let mut tree = super::Xi223IntervalTree::xi_new();
        tree.xi_insert(super::Xi223Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi223Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi223Interval::xi_new(12, 20));
        let q = super::Xi223Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi223_interval_tree_remove() {
        let mut tree = super::Xi223IntervalTree::xi_new();
        tree.xi_insert(super::Xi223Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi223Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi223_interval_tree_gaps() {
        let mut tree = super::Xi223IntervalTree::xi_new();
        tree.xi_insert(super::Xi223Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi223Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi223Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi223Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi223Interval::xi_new(8, 10));
    }

    #[test]
    fn xi223_interval_tree_merge() {
        let mut tree = super::Xi223IntervalTree::xi_new();
        tree.xi_insert(super::Xi223Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi223Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi223Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi223Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi223Interval::xi_new(10, 15));
    }

    #[test]
    fn xi223_interval_tree_all() {
        let mut tree = super::Xi223IntervalTree::xi_new();
        tree.xi_insert(super::Xi223Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi223Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi223_interval_tree_empty() {
        let tree = super::Xi223IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi223_interval_tree_contains_point() {
        let iv = super::Xi223Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 223) ---

    #[test]
    fn xj_223_uf_make_and_find() {
        let mut uf = super::Xj223UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_223_uf_union_connected() {
        let mut uf = super::Xj223UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_223_uf_component_count() {
        let mut uf = super::Xj223UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_223_uf_component_size() {
        let mut uf = super::Xj223UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_223_uf_largest_component() {
        let mut uf = super::Xj223UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_223_uf_many_elements() {
        let mut uf = super::Xj223UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_223_uf_separate_components() {
        let mut uf = super::Xj223UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_223_uf_path_compression() {
        let mut uf = super::Xj223UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_223_bt_insert_get() {
        let mut bt = super::Xj223BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_223_bt_contains_len() {
        let mut bt = super::Xj223BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_223_bt_replace() {
        let mut bt = super::Xj223BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_223_bt_remove() {
        let mut bt = super::Xj223BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_223_bt_keys_values() {
        let mut bt = super::Xj223BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_223_bt_range() {
        let mut bt = super::Xj223BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_223_bt_min_max() {
        let mut bt = super::Xj223BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_223_bt_many_inserts() {
        let mut bt = super::Xj223BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_223 segment tree tests ---

    #[test]
    fn xk_223_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk223SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_223_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk223SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_223_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk223SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_223_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk223SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_223_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk223SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_223_st_single_element() {
        let data = vec![42];
        let st = super::Xk223SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_223_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk223SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_223_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk223SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_223 disjoint intervals tests ---

    #[test]
    fn xk_223_di_add_and_count() {
        let mut di = super::Xk223DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_223_di_merge_overlap() {
        let mut di = super::Xk223DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_223_di_contains() {
        let mut di = super::Xk223DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_223_di_remove() {
        let mut di = super::Xk223DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_223_di_covered_length() {
        let mut di = super::Xk223DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_223_di_gaps() {
        let mut di = super::Xk223DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_223_di_merge_adjacent() {
        let mut di = super::Xk223DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_223_di_empty() {
        let di = super::Xk223DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_223_rope_new_empty() {
        let rope = super::Xl223Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_223_rope_from_str() {
        let rope = super::Xl223Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_223_rope_insert_at() {
        let mut rope = super::Xl223Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_223_rope_delete_range() {
        let mut rope = super::Xl223Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_223_rope_char_at() {
        let rope = super::Xl223Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_223_rope_split_concat() {
        let rope = super::Xl223Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_223_rope_line_count() {
        let rope = super::Xl223Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_223_rope_line_at() {
        let rope = super::Xl223Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_223_sa_build_and_search() {
        let sa = super::Xl223SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_223_sa_count() {
        let sa = super::Xl223SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_223_sa_longest_repeated() {
        let sa = super::Xl223SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_223_sa_all_positions() {
        let sa = super::Xl223SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_223_sa_len() {
        let sa = super::Xl223SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_223_sa_empty() {
        let sa = super::Xl223SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_223_rope_slice() {
        let rope = super::Xl223Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_223_sa_search_start() {
        let sa = super::Xl223SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_223_sparse_set_get() {
        let mut m = super::Xm223MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_223_sparse_row_col() {
        let mut m = super::Xm223MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_223_sparse_transpose() {
        let mut m = super::Xm223MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_223_sparse_multiply_vec() {
        let mut m = super::Xm223MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_223_sparse_nnz_density() {
        let mut m = super::Xm223MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_223_sparse_clear() {
        let mut m = super::Xm223MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_223_sparse_overwrite_zero() {
        let mut m = super::Xm223MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_223_tokenizer_basic() {
        let t = super::Xm223Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_223_tokenizer_count() {
        let t = super::Xm223Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_223_tokenizer_unique() {
        let t = super::Xm223Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_223_tokenizer_frequency() {
        let t = super::Xm223Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_223_tokenizer_delimiter() {
        let t = super::Xm223Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_223_tokenizer_whitespace() {
        let t = super::Xm223Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_223_tokenizer_empty() {
        let t = super::Xm223Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 223 ----

    #[test]
    fn xn_223_fenwick_prefix_sum() {
        let mut ft = super::Xn223Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_223_fenwick_range_sum() {
        let mut ft = super::Xn223Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_223_fenwick_point_query() {
        let mut ft = super::Xn223Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_223_fenwick_len() {
        let ft = super::Xn223Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_223_fenwick_multiple_updates() {
        let mut ft = super::Xn223Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_223_fenwick_single_element() {
        let mut ft = super::Xn223Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_223_fenwick_find_kth() {
        let mut ft = super::Xn223Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_223_fenwick_negative_delta() {
        let mut ft = super::Xn223Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 223 ----

    #[test]
    fn xn_223_avl_insert_get() {
        let mut m = super::Xn223AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_223_avl_remove() {
        let mut m = super::Xn223AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_223_avl_in_order() {
        let mut m = super::Xn223AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_223_avl_min_max() {
        let mut m = super::Xn223AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_223_avl_floor_ceiling() {
        let mut m = super::Xn223AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_223_avl_height_balanced() {
        let mut m = super::Xn223AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_223_avl_overwrite() {
        let mut m = super::Xn223AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_223_avl_empty() {
        let m: super::Xn223AVL<i32, i32> = super::Xn223AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }
}
