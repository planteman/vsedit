//! Search across files view — equivalent to VS Code's search sidebar (Ctrl+Shift+F).
//!
//! Provides [`SearchQuery`], [`SearchEngine`], [`SearchResults`], [`SearchView`],
//! and [`ReplaceOperation`] for workspace-wide find and replace.
//!
//! Also re-exports file-system search, replace, fuzzy file name search, and
//! symbol extraction from [`vsedit_wb_search`].

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

// Re-export workspace search functionality
pub use vsedit_wb_search::{
    execute_replace, extract_symbols, preview_replace, replace_all, search_file_names,
    search_files, FileQuickPick, ReplaceQuery, SymbolEntry, SymbolKind,
};

use globset::{Glob, GlobMatcher};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
};
use regex::Regex;
use walkdir::WalkDir;

// ---------------------------------------------------------------------------
// SearchQuery
// ---------------------------------------------------------------------------

/// Parameters for a workspace-wide search.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub pattern: String,
    pub is_regex: bool,
    pub is_case_sensitive: bool,
    pub is_whole_word: bool,
    pub include_pattern: Option<String>,
    pub exclude_pattern: Option<String>,
}

impl SearchQuery {
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            is_regex: false,
            is_case_sensitive: false,
            is_whole_word: false,
            include_pattern: None,
            exclude_pattern: None,
        }
    }

    pub fn with_regex(mut self, v: bool) -> Self {
        self.is_regex = v;
        self
    }

    pub fn with_case_sensitive(mut self, v: bool) -> Self {
        self.is_case_sensitive = v;
        self
    }

    pub fn with_whole_word(mut self, v: bool) -> Self {
        self.is_whole_word = v;
        self
    }

    pub fn with_include(mut self, pattern: impl Into<String>) -> Self {
        self.include_pattern = Some(pattern.into());
        self
    }

    pub fn with_exclude(mut self, pattern: impl Into<String>) -> Self {
        self.exclude_pattern = Some(pattern.into());
        self
    }

    /// Build a [`Regex`] from the query options. Returns `None` on invalid pattern.
    pub fn build_regex(&self) -> Option<Regex> {
        if self.pattern.is_empty() {
            return None;
        }

        let pat = if self.is_regex {
            self.pattern.clone()
        } else {
            regex::escape(&self.pattern)
        };

        let pat = if self.is_whole_word {
            format!(r"\b{pat}\b")
        } else {
            pat
        };

        let pat = if self.is_case_sensitive {
            pat
        } else {
            format!("(?i){pat}")
        };

        Regex::new(&pat).ok()
    }
}

// ---------------------------------------------------------------------------
// SearchMatch
// ---------------------------------------------------------------------------

/// A single match within a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub file_path: PathBuf,
    pub line_number: u32,
    pub column: u32,
    pub line_content: String,
    pub match_range: Range<usize>,
    pub preview: String,
}

// ---------------------------------------------------------------------------
// FileMatches
// ---------------------------------------------------------------------------

/// All matches within a single file.
#[derive(Debug, Clone)]
pub struct FileMatches {
    pub file_path: PathBuf,
    pub matches: Vec<SearchMatch>,
    pub is_expanded: bool,
}

impl FileMatches {
    pub fn new(file_path: PathBuf, matches: Vec<SearchMatch>) -> Self {
        Self {
            file_path,
            matches,
            is_expanded: true,
        }
    }

    pub fn match_count(&self) -> usize {
        self.matches.len()
    }
}

// ---------------------------------------------------------------------------
// SearchResults
// ---------------------------------------------------------------------------

/// Aggregated search results grouped by file.
#[derive(Debug, Clone)]
pub struct SearchResults {
    file_matches: Vec<FileMatches>,
}

impl SearchResults {
    pub fn new(file_matches: Vec<FileMatches>) -> Self {
        Self { file_matches }
    }

    pub fn empty() -> Self {
        Self {
            file_matches: Vec::new(),
        }
    }

    pub fn total_matches(&self) -> usize {
        self.file_matches.iter().map(|f| f.matches.len()).sum()
    }

    pub fn total_files(&self) -> usize {
        self.file_matches.len()
    }

    pub fn files(&self) -> &[FileMatches] {
        &self.file_matches
    }

    pub fn files_mut(&mut self) -> &mut [FileMatches] {
        &mut self.file_matches
    }
}

// ---------------------------------------------------------------------------
// SearchEngine
// ---------------------------------------------------------------------------

/// Maximum number of results before stopping the search.
const MAX_RESULTS: usize = 10_000;

/// Performs workspace-wide text search.
pub struct SearchEngine;

impl SearchEngine {
    /// Walk `root` and search every matching file.
    pub fn search(query: &SearchQuery, root: &Path) -> SearchResults {
        let re = match query.build_regex() {
            Some(r) => r,
            None => return SearchResults::empty(),
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

        let mut file_matches: Vec<FileMatches> = Vec::new();
        let mut total = 0usize;

        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            if total >= MAX_RESULTS {
                break;
            }

            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            // Skip hidden / .git directories
            if path
                .components()
                .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
            {
                continue;
            }

            if !Self::matches_glob(&include_matcher, &exclude_matcher, path) {
                continue;
            }

            if Self::is_binary(path) {
                continue;
            }

            let matches = Self::search_in_file_with_regex(&re, path, MAX_RESULTS - total);
            if !matches.is_empty() {
                total += matches.len();
                file_matches.push(FileMatches::new(path.to_path_buf(), matches));
            }
        }

        SearchResults::new(file_matches)
    }

    /// Search a single file for matches.
    pub fn search_in_file(query: &SearchQuery, path: &Path) -> Vec<SearchMatch> {
        let re = match query.build_regex() {
            Some(r) => r,
            None => return Vec::new(),
        };
        Self::search_in_file_with_regex(&re, path, MAX_RESULTS)
    }

    fn search_in_file_with_regex(re: &Regex, path: &Path, limit: usize) -> Vec<SearchMatch> {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut matches = Vec::new();
        for (line_idx, line) in content.lines().enumerate() {
            for m in re.find_iter(line) {
                matches.push(SearchMatch {
                    file_path: path.to_path_buf(),
                    line_number: (line_idx + 1) as u32,
                    column: (m.start() + 1) as u32,
                    line_content: line.to_string(),
                    match_range: m.start()..m.end(),
                    preview: Self::build_preview(line, m.start(), m.end()),
                });
                if matches.len() >= limit {
                    return matches;
                }
            }
        }
        matches
    }

    /// Public wrapper for `build_preview`, used by `SearchView::search_in_files`.
    pub fn build_preview_public(line: &str, start: usize, end: usize) -> String {
        Self::build_preview(line, start, end)
    }

    fn build_preview(line: &str, start: usize, end: usize) -> String {
        let context = 20;
        let pre = start.saturating_sub(context);
        let post = (end + context).min(line.len());

        let mut preview = String::new();
        if pre > 0 {
            preview.push_str("…");
        }
        preview.push_str(&line[pre..post]);
        if post < line.len() {
            preview.push_str("…");
        }
        preview
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

    /// Heuristic: read first 512 bytes and look for NUL.
    fn is_binary(path: &Path) -> bool {
        use std::io::Read;
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
}

// ---------------------------------------------------------------------------
// ReplaceOperation
// ---------------------------------------------------------------------------

/// Operations for replacing search matches.
pub struct ReplaceOperation {
    pub replace_text: String,
}

impl ReplaceOperation {
    pub fn new(replace_text: impl Into<String>) -> Self {
        Self {
            replace_text: replace_text.into(),
        }
    }

    /// Replace a single match within a line, returning the modified line.
    pub fn replace_match(&self, search_match: &SearchMatch) -> String {
        let line = &search_match.line_content;
        let range = &search_match.match_range;
        let mut result = String::with_capacity(line.len());
        result.push_str(&line[..range.start]);
        result.push_str(&self.replace_text);
        result.push_str(&line[range.end..]);
        result
    }

    /// Replace all matches within a single file and write the result.
    /// Returns `true` on success.
    pub fn replace_all_in_file(&self, file_matches: &FileMatches) -> bool {
        let content = match fs::read_to_string(&file_matches.file_path) {
            Ok(c) => c,
            Err(_) => return false,
        };

        let lines: Vec<&str> = content.lines().collect();
        let mut result_lines: Vec<String> = lines.iter().map(|l| (*l).to_string()).collect();

        // Group matches by line (1-based) and apply in reverse column order
        // to preserve byte offsets.
        let mut by_line: std::collections::HashMap<u32, Vec<&SearchMatch>> =
            std::collections::HashMap::new();
        for m in &file_matches.matches {
            by_line.entry(m.line_number).or_default().push(m);
        }

        for (line_num, mut line_matches) in by_line {
            let idx = (line_num - 1) as usize;
            if idx >= result_lines.len() {
                continue;
            }
            // Sort by column descending so replacements don't shift earlier offsets.
            line_matches.sort_by(|a, b| b.match_range.start.cmp(&a.match_range.start));
            let mut line = result_lines[idx].clone();
            for m in &line_matches {
                if m.match_range.end <= line.len() {
                    line = format!(
                        "{}{}{}",
                        &line[..m.match_range.start],
                        self.replace_text,
                        &line[m.match_range.end..]
                    );
                }
            }
            result_lines[idx] = line;
        }

        // Preserve trailing newline if original had one.
        let mut output = result_lines.join("\n");
        if content.ends_with('\n') {
            output.push('\n');
        }

        fs::write(&file_matches.file_path, output).is_ok()
    }
}

// ---------------------------------------------------------------------------
// SearchView
// ---------------------------------------------------------------------------

/// Which input field is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveField {
    Search,
    Replace,
    Include,
    Exclude,
}

/// UI state for the search sidebar view.
pub struct SearchView {
    // Input fields
    pub search_text: String,
    pub replace_text: String,
    pub include_text: String,
    pub exclude_text: String,

    // Toggle states
    pub is_regex: bool,
    pub is_case_sensitive: bool,
    pub is_whole_word: bool,
    pub show_replace: bool,

    // UI state
    pub active_field: ActiveField,
    pub selected_result: Option<usize>,
    pub scroll_offset: usize,

    // Results
    pub results: SearchResults,
}

impl SearchView {
    pub fn new() -> Self {
        Self {
            search_text: String::new(),
            replace_text: String::new(),
            include_text: String::new(),
            exclude_text: String::new(),
            is_regex: false,
            is_case_sensitive: false,
            is_whole_word: false,
            show_replace: false,
            active_field: ActiveField::Search,
            selected_result: None,
            scroll_offset: 0,
            results: SearchResults::empty(),
        }
    }

    /// Build a [`SearchQuery`] from the current UI state.
    pub fn build_query(&self) -> SearchQuery {
        let mut q = SearchQuery::new(&self.search_text)
            .with_regex(self.is_regex)
            .with_case_sensitive(self.is_case_sensitive)
            .with_whole_word(self.is_whole_word);

        if !self.include_text.is_empty() {
            q = q.with_include(&self.include_text);
        }
        if !self.exclude_text.is_empty() {
            q = q.with_exclude(&self.exclude_text);
        }
        q
    }

    /// Execute the search against the given workspace root.
    pub fn execute_search(&mut self, root: &Path) {
        let query = self.build_query();
        self.results = SearchEngine::search(&query, root);
        self.selected_result = if self.results.total_matches() > 0 {
            Some(0)
        } else {
            None
        };
        self.scroll_offset = 0;
    }

    /// Toggle file expansion in results.
    pub fn toggle_file_expanded(&mut self, file_idx: usize) {
        if let Some(fm) = self.results.files_mut().get_mut(file_idx) {
            fm.is_expanded = !fm.is_expanded;
        }
    }

    /// Move selection to the next result entry.
    pub fn select_next(&mut self) {
        let total = self.visible_entry_count();
        if total == 0 {
            return;
        }
        self.selected_result = Some(match self.selected_result {
            Some(i) => (i + 1) % total,
            None => 0,
        });
    }

    /// Move selection to the previous result entry.
    pub fn select_previous(&mut self) {
        let total = self.visible_entry_count();
        if total == 0 {
            return;
        }
        self.selected_result = Some(match self.selected_result {
            Some(0) | None => total.saturating_sub(1),
            Some(i) => i - 1,
        });
    }

    /// Total visible entries (file headers + expanded matches).
    fn visible_entry_count(&self) -> usize {
        let mut count = 0;
        for fm in self.results.files() {
            count += 1; // file header
            if fm.is_expanded {
                count += fm.matches.len();
            }
        }
        count
    }

    /// Render the search view into a ratatui buffer.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let mut y = area.y;

        // -- Header: "SEARCH" title
        if y < area.y + area.height {
            let title = "SEARCH";
            let style = Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD);
            for (i, ch) in title.chars().enumerate() {
                let x = area.x + 1 + i as u16;
                if x < area.x + area.width {
                    buf[(x, y)].set_char(ch).set_style(style);
                }
            }
            y += 1;
        }

        // -- Search input
        if y < area.y + area.height {
            self.render_input_line("Search: ", &self.search_text, ActiveField::Search, area, y, buf);
            y += 1;
        }

        // -- Toggle indicators
        if y < area.y + area.height {
            let toggles = format!(
                " [{}Aa] [{}.*] [{}ab]",
                if self.is_case_sensitive { "●" } else { " " },
                if self.is_regex { "●" } else { " " },
                if self.is_whole_word { "●" } else { " " },
            );
            let style = Style::default().fg(Color::DarkGray);
            for (i, ch) in toggles.chars().enumerate() {
                let x = area.x + i as u16;
                if x < area.x + area.width {
                    buf[(x, y)].set_char(ch).set_style(style);
                }
            }
            y += 1;
        }

        // -- Replace input (if visible)
        if self.show_replace && y < area.y + area.height {
            self.render_input_line("Replace:", &self.replace_text, ActiveField::Replace, area, y, buf);
            y += 1;
        }

        // -- Include/Exclude
        if y < area.y + area.height {
            self.render_input_line("Include:", &self.include_text, ActiveField::Include, area, y, buf);
            y += 1;
        }
        if y < area.y + area.height {
            self.render_input_line("Exclude:", &self.exclude_text, ActiveField::Exclude, area, y, buf);
            y += 1;
        }

        // -- Summary line
        if y < area.y + area.height {
            let summary = format!(
                "{} results in {} files",
                self.results.total_matches(),
                self.results.total_files(),
            );
            let style = Style::default().fg(Color::DarkGray);
            for (i, ch) in summary.chars().enumerate() {
                let x = area.x + 1 + i as u16;
                if x < area.x + area.width {
                    buf[(x, y)].set_char(ch).set_style(style);
                }
            }
            y += 1;
        }

        // -- Results tree
        let mut entry_idx = 0usize;
        let mut skipped = 0usize;
        for fm in self.results.files() {
            // File header
            if skipped >= self.scroll_offset {
                if y >= area.y + area.height {
                    break;
                }
                let arrow = if fm.is_expanded { "▼" } else { "▶" };
                let file_name = fm
                    .file_path
                    .file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default();
                let label = format!(
                    "{arrow} {file_name} ({count})",
                    count = fm.matches.len()
                );
                let is_selected = self.selected_result == Some(entry_idx);
                let style = if is_selected {
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::Rgb(38, 79, 120))
                } else {
                    Style::default().fg(Color::Yellow)
                };
                for (i, ch) in label.chars().enumerate() {
                    let x = area.x + 1 + i as u16;
                    if x < area.x + area.width {
                        buf[(x, y)].set_char(ch).set_style(style);
                    }
                }
                y += 1;
            } else {
                skipped += 1;
            }
            entry_idx += 1;

            // Match lines
            if fm.is_expanded {
                for m in &fm.matches {
                    if skipped < self.scroll_offset {
                        skipped += 1;
                        entry_idx += 1;
                        continue;
                    }
                    if y >= area.y + area.height {
                        break;
                    }
                    let is_selected = self.selected_result == Some(entry_idx);
                    let prefix = format!("  {}:{} ", m.line_number, m.column);
                    let bg = if is_selected {
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Rgb(38, 79, 120))
                    } else {
                        Style::default().fg(Color::Gray)
                    };

                    // Render prefix
                    let mut col = 0u16;
                    for ch in prefix.chars() {
                        let x = area.x + 1 + col;
                        if x < area.x + area.width {
                            buf[(x, y)].set_char(ch).set_style(bg);
                        }
                        col += 1;
                    }

                    // Render line content with match highlighted
                    let content = &m.line_content;
                    let max_w = area.width.saturating_sub(2 + col);
                    for (ci, ch) in content.chars().take(max_w as usize).enumerate() {
                        let x = area.x + 1 + col + ci as u16;
                        if x >= area.x + area.width {
                            break;
                        }
                        let s = if ci >= m.match_range.start && ci < m.match_range.end {
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Yellow)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            bg
                        };
                        buf[(x, y)].set_char(ch).set_style(s);
                    }

                    y += 1;
                    entry_idx += 1;
                }
            }
        }
    }

    fn render_input_line(
        &self,
        label: &str,
        value: &str,
        field: ActiveField,
        area: Rect,
        y: u16,
        buf: &mut Buffer,
    ) {
        let is_active = self.active_field == field;
        let label_style = Style::default().fg(Color::DarkGray);
        let value_style = if is_active {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::Gray)
        };

        let mut col = 0u16;
        for ch in label.chars() {
            let x = area.x + 1 + col;
            if x < area.x + area.width {
                buf[(x, y)].set_char(ch).set_style(label_style);
            }
            col += 1;
        }
        for ch in value.chars() {
            let x = area.x + 1 + col;
            if x < area.x + area.width {
                buf[(x, y)].set_char(ch).set_style(value_style);
            }
            col += 1;
        }

        // Cursor indicator
        if is_active {
            let x = area.x + 1 + col;
            if x < area.x + area.width {
                buf[(x, y)]
                    .set_char(' ')
                    .set_style(Style::default().add_modifier(Modifier::REVERSED));
            }
        }
    }

    /// Execute search using the `vsedit-wb-search` file-system engine and
    /// convert results into the view's [`SearchResults`] format.
    pub fn execute_search_via_service(&mut self, root: &Path) {
        let wb_query = vsedit_wb_search::SearchQuery {
            pattern: self.search_text.clone(),
            is_regex: self.is_regex,
            case_sensitive: self.is_case_sensitive,
            whole_word: self.is_whole_word,
            include_pattern: if self.include_text.is_empty() {
                None
            } else {
                Some(self.include_text.clone())
            },
            exclude_pattern: if self.exclude_text.is_empty() {
                None
            } else {
                Some(self.exclude_text.clone())
            },
        };

        let wb_results = vsedit_wb_search::search_files(&wb_query, root);

        let file_matches: Vec<FileMatches> = wb_results
            .into_iter()
            .map(|sr| {
                let file_path = PathBuf::from(&sr.matches[0].uri);
                let matches = sr
                    .matches
                    .iter()
                    .map(|m| SearchMatch {
                        file_path: PathBuf::from(&m.uri),
                        line_number: m.line,
                        column: m.column,
                        line_content: m.preview.clone(),
                        match_range: (m.column as usize)..(m.column as usize + m.length as usize),
                        preview: m.preview.clone(),
                    })
                    .collect();
                FileMatches::new(file_path, matches)
            })
            .collect();

        self.results = SearchResults::new(file_matches);
        self.selected_result = if self.results.total_matches() > 0 {
            Some(0)
        } else {
            None
        };
        self.scroll_offset = 0;
    }

    /// Preview what replacing the selected match would look like.
    pub fn preview_replace_selected(&self) -> Option<String> {
        let idx = self.selected_result?;
        let m = self.get_match_at_entry(idx)?;
        let wb_match = vsedit_wb_search::SearchMatch {
            uri: m.file_path.to_string_lossy().to_string(),
            line: m.line_number,
            column: m.column,
            length: (m.match_range.end - m.match_range.start) as u32,
            preview: m.line_content.clone(),
        };
        Some(vsedit_wb_search::preview_replace(&wb_match, &self.replace_text))
    }

    /// Replace all search results across files using the wb-search engine.
    pub fn replace_all_via_service(&mut self, root: &Path) -> usize {
        let wb_query = vsedit_wb_search::SearchQuery {
            pattern: self.search_text.clone(),
            is_regex: self.is_regex,
            case_sensitive: self.is_case_sensitive,
            whole_word: self.is_whole_word,
            include_pattern: if self.include_text.is_empty() {
                None
            } else {
                Some(self.include_text.clone())
            },
            exclude_pattern: if self.exclude_text.is_empty() {
                None
            } else {
                Some(self.exclude_text.clone())
            },
        };
        let rq = vsedit_wb_search::ReplaceQuery::new(wb_query, &self.replace_text);
        let count = vsedit_wb_search::replace_all(&rq, root);
        // Re-run search to refresh results
        self.execute_search_via_service(root);
        count
    }

    /// Set the search query text.
    pub fn set_query(&mut self, query: &str) {
        self.search_text = query.to_string();
    }

    /// Search through in-memory file contents (path, content) pairs using
    /// case-insensitive substring matching.  Results are grouped by file.
    pub fn search_in_files(&mut self, files: &[(String, String)]) {
        if self.search_text.is_empty() {
            self.results = SearchResults::empty();
            self.selected_result = None;
            self.scroll_offset = 0;
            return;
        }

        let query = self.build_query();
        let re = match query.build_regex() {
            Some(r) => r,
            None => {
                self.results = SearchResults::empty();
                self.selected_result = None;
                self.scroll_offset = 0;
                return;
            }
        };

        let mut file_matches: Vec<FileMatches> = Vec::new();
        let mut total = 0usize;

        for (path, content) in files {
            let mut matches = Vec::new();
            for (line_idx, line) in content.lines().enumerate() {
                for m in re.find_iter(line) {
                    matches.push(SearchMatch {
                        file_path: PathBuf::from(path),
                        line_number: (line_idx + 1) as u32,
                        column: (m.start() + 1) as u32,
                        line_content: line.to_string(),
                        match_range: m.start()..m.end(),
                        preview: SearchEngine::build_preview_public(line, m.start(), m.end()),
                    });
                    total += 1;
                    if total >= MAX_RESULTS {
                        break;
                    }
                }
                if total >= MAX_RESULTS {
                    break;
                }
            }
            if !matches.is_empty() {
                file_matches.push(FileMatches::new(PathBuf::from(path), matches));
            }
            if total >= MAX_RESULTS {
                break;
            }
        }

        self.results = SearchResults::new(file_matches);
        self.selected_result = if self.results.total_matches() > 0 {
            Some(0)
        } else {
            None
        };
        self.scroll_offset = 0;
    }

    /// Return search results as (file_path, vec of (line_number, line_content)) pairs.
    pub fn get_results(&self) -> Vec<(String, Vec<(usize, String)>)> {
        self.results
            .files()
            .iter()
            .map(|fm| {
                let path = fm.file_path.to_string_lossy().to_string();
                let lines = fm
                    .matches
                    .iter()
                    .map(|m| (m.line_number as usize, m.line_content.clone()))
                    .collect();
                (path, lines)
            })
            .collect()
    }

    /// Get the match at a given visible entry index, or `None` if it's a file header.
    fn get_match_at_entry(&self, entry_idx: usize) -> Option<&SearchMatch> {
        let mut idx = 0;
        for fm in self.results.files() {
            if idx == entry_idx {
                return None; // file header
            }
            idx += 1;
            if fm.is_expanded {
                for m in &fm.matches {
                    if idx == entry_idx {
                        return Some(m);
                    }
                    idx += 1;
                }
            }
        }
        None
    }
}

impl Default for SearchView {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SearchStatistics
// ---------------------------------------------------------------------------

/// Computed statistics for a set of search results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchStatistics {
    pub files_matched: usize,
    pub total_matches: usize,
    pub lines_with_matches: usize,
}

impl SearchStatistics {
    /// Compute statistics from [`SearchResults`].
    pub fn from_results(results: &SearchResults) -> Self {
        let files_matched = results.total_files();
        let total_matches = results.total_matches();
        let lines_with_matches: usize = results
            .files()
            .iter()
            .map(|fm| {
                let mut lines: Vec<u32> = fm.matches.iter().map(|m| m.line_number).collect();
                lines.sort_unstable();
                lines.dedup();
                lines.len()
            })
            .sum();
        Self {
            files_matched,
            total_matches,
            lines_with_matches,
        }
    }

    /// Format a human-readable summary string.
    pub fn summary(&self) -> String {
        format!(
            "{} match{} across {} file{}, {} line{}",
            self.total_matches,
            if self.total_matches == 1 { "" } else { "es" },
            self.files_matched,
            if self.files_matched == 1 { "" } else { "s" },
            self.lines_with_matches,
            if self.lines_with_matches == 1 { "" } else { "s" },
        )
    }
}

// ---------------------------------------------------------------------------
// SearchQueryHistory
// ---------------------------------------------------------------------------

/// Maintains a deduplicated, bounded history of search queries.
#[derive(Debug, Clone)]
pub struct SearchQueryHistory {
    entries: Vec<String>,
    capacity: usize,
    cursor: Option<usize>,
}

impl SearchQueryHistory {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity: capacity.max(1),
            cursor: None,
        }
    }

    /// Push a query to the front. If it already exists, move it to front.
    /// Empty strings are ignored.
    pub fn push(&mut self, query: &str) {
        if query.is_empty() {
            return;
        }
        // Remove duplicate if present
        self.entries.retain(|e| e != query);
        self.entries.insert(0, query.to_string());
        if self.entries.len() > self.capacity {
            self.entries.truncate(self.capacity);
        }
        self.cursor = None;
    }

    /// Navigate to the previous (older) entry. Returns `None` if empty or at the end.
    pub fn previous(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        let next = match self.cursor {
            None => 0,
            Some(i) if i + 1 < self.entries.len() => i + 1,
            Some(_) => return None,
        };
        self.cursor = Some(next);
        Some(&self.entries[next])
    }

    /// Navigate to the next (newer) entry. Returns `None` if at the newest.
    pub fn next(&mut self) -> Option<&str> {
        match self.cursor {
            None | Some(0) => {
                self.cursor = None;
                None
            }
            Some(i) => {
                self.cursor = Some(i - 1);
                Some(&self.entries[i - 1])
            }
        }
    }

    /// Reset cursor to the most-recent position (no selection).
    pub fn reset_cursor(&mut self) {
        self.cursor = None;
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------------------
// SearchResults — collapse / expand helpers
// ---------------------------------------------------------------------------

impl SearchResults {
    /// Collapse all file groups.
    pub fn collapse_all(&mut self) {
        for fm in &mut self.file_matches {
            fm.is_expanded = false;
        }
    }

    /// Expand all file groups.
    pub fn expand_all(&mut self) {
        for fm in &mut self.file_matches {
            fm.is_expanded = true;
        }
    }

    /// Return computed [`SearchStatistics`].
    pub fn statistics(&self) -> SearchStatistics {
        SearchStatistics::from_results(self)
    }
}

// ---------------------------------------------------------------------------
// SearchView — match navigation helpers
// ---------------------------------------------------------------------------

/// Location of the currently selected match (file index + match index within file).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchLocation {
    pub file_index: usize,
    pub match_index: usize,
}

impl SearchView {
    /// Collect a flat list of all (file_idx, match_idx) pairs across results.
    fn all_match_locations(&self) -> Vec<MatchLocation> {
        let mut locs = Vec::new();
        for (fi, fm) in self.results.files().iter().enumerate() {
            for mi in 0..fm.matches.len() {
                locs.push(MatchLocation {
                    file_index: fi,
                    match_index: mi,
                });
            }
        }
        locs
    }

    /// Navigate to the next match (skipping file headers), wrapping around.
    /// Returns the [`MatchLocation`] if one exists.
    pub fn next_match(&mut self) -> Option<MatchLocation> {
        let locs = self.all_match_locations();
        if locs.is_empty() {
            return None;
        }

        // Find current match location from the selected visible entry
        let current_loc = self.selected_result.and_then(|e| self.get_match_at_entry(e)).map(|m| {
            let file_idx = self.results.files().iter().position(|fm| fm.file_path == m.file_path).unwrap_or(0);
            let match_idx = self.results.files()[file_idx]
                .matches
                .iter()
                .position(|mm| std::ptr::eq(mm, m))
                .unwrap_or(0);
            MatchLocation { file_index: file_idx, match_index: match_idx }
        });

        let next = match current_loc {
            Some(cur) => {
                let pos = locs.iter().position(|l| *l == cur).unwrap_or(0);
                locs[(pos + 1) % locs.len()]
            }
            None => locs[0],
        };

        // Ensure the target file is expanded and set visible entry index
        self.results.files_mut()[next.file_index].is_expanded = true;
        self.selected_result = Some(self.entry_index_for_match(next));
        Some(next)
    }

    /// Navigate to the previous match (skipping file headers), wrapping around.
    pub fn previous_match(&mut self) -> Option<MatchLocation> {
        let locs = self.all_match_locations();
        if locs.is_empty() {
            return None;
        }

        let current_loc = self.selected_result.and_then(|e| self.get_match_at_entry(e)).map(|m| {
            let file_idx = self.results.files().iter().position(|fm| fm.file_path == m.file_path).unwrap_or(0);
            let match_idx = self.results.files()[file_idx]
                .matches
                .iter()
                .position(|mm| std::ptr::eq(mm, m))
                .unwrap_or(0);
            MatchLocation { file_index: file_idx, match_index: match_idx }
        });

        let prev = match current_loc {
            Some(cur) => {
                let pos = locs.iter().position(|l| *l == cur).unwrap_or(0);
                if pos == 0 { locs[locs.len() - 1] } else { locs[pos - 1] }
            }
            None => locs[locs.len() - 1],
        };

        self.results.files_mut()[prev.file_index].is_expanded = true;
        self.selected_result = Some(self.entry_index_for_match(prev));
        Some(prev)
    }

    /// Compute the visible entry index for a given [`MatchLocation`].
    fn entry_index_for_match(&self, loc: MatchLocation) -> usize {
        let mut idx = 0;
        for (fi, fm) in self.results.files().iter().enumerate() {
            idx += 1; // file header
            if fi == loc.file_index {
                return idx + loc.match_index;
            }
            if fm.is_expanded {
                idx += fm.matches.len();
            }
        }
        0
    }

    /// Preview replacements for all matches in a file group.
    /// Returns pairs of (original_line, replaced_line) for each unique line.
    pub fn preview_replace_for_file(&self, file_idx: usize) -> Vec<(String, String)> {
        let fm = match self.results.files().get(file_idx) {
            Some(fm) => fm,
            None => return Vec::new(),
        };

        let mut seen_lines = std::collections::HashSet::new();
        let mut previews = Vec::new();

        for m in &fm.matches {
            if !seen_lines.insert(m.line_number) {
                continue;
            }

            // Collect all matches on this line
            let line_matches: Vec<&SearchMatch> = fm
                .matches
                .iter()
                .filter(|mm| mm.line_number == m.line_number)
                .collect();

            // Apply replacements in reverse column order
            let mut replaced = m.line_content.clone();
            let mut sorted: Vec<&&SearchMatch> = line_matches.iter().collect();
            sorted.sort_by(|a, b| b.match_range.start.cmp(&a.match_range.start));
            for sm in sorted {
                if sm.match_range.end <= replaced.len() {
                    replaced = format!(
                        "{}{}{}",
                        &replaced[..sm.match_range.start],
                        self.replace_text,
                        &replaced[sm.match_range.end..],
                    );
                }
            }

            previews.push((m.line_content.clone(), replaced));
        }

        previews
    }

    /// Get statistics for the current results.
    pub fn statistics(&self) -> SearchStatistics {
        self.results.statistics()
    }
}

// ---------------------------------------------------------------------------
// SearchResultDecorator — highlight match ranges in search results
// ---------------------------------------------------------------------------

/// A single decorated line split at a match boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoratedLine {
    pub before: String,
    pub matched: String,
    pub after: String,
}

impl fmt::Display for DecoratedLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[{}]{}", self.before, self.matched, self.after)
    }
}

/// A segment of text that may or may not be a match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoratedSegment {
    pub text: String,
    pub is_match: bool,
}

impl fmt::Display for DecoratedSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_match {
            write!(f, "[{}]", self.text)
        } else {
            write!(f, "{}", self.text)
        }
    }
}

/// Highlights match ranges within search result lines.
#[derive(Debug, Clone)]
pub struct SearchResultDecorator;

impl SearchResultDecorator {
    pub fn new() -> Self {
        Self
    }

    /// Split `line` into before / matched / after at the given byte range.
    pub fn decorate(&self, line: &str, match_start: usize, match_end: usize) -> DecoratedLine {
        let start = match_start.min(line.len());
        let end = match_end.min(line.len()).max(start);
        DecoratedLine {
            before: line[..start].to_string(),
            matched: line[start..end].to_string(),
            after: line[end..].to_string(),
        }
    }

    /// Split `line` into alternating non-match / match segments for multiple
    /// non-overlapping, sorted ranges.
    pub fn decorate_all(&self, line: &str, ranges: &[(usize, usize)]) -> Vec<DecoratedSegment> {
        let mut segments = Vec::new();
        let mut cursor = 0usize;

        for &(start, end) in ranges {
            let s = start.min(line.len());
            let e = end.min(line.len()).max(s);
            if s < cursor {
                continue; // skip overlapping / out-of-order ranges
            }
            if cursor < s {
                segments.push(DecoratedSegment {
                    text: line[cursor..s].to_string(),
                    is_match: false,
                });
            }
            if s < e {
                segments.push(DecoratedSegment {
                    text: line[s..e].to_string(),
                    is_match: true,
                });
            }
            cursor = e;
        }

        if cursor < line.len() {
            segments.push(DecoratedSegment {
                text: line[cursor..].to_string(),
                is_match: false,
            });
        }

        segments
    }
}

// ---------------------------------------------------------------------------
// SearchFileGrouper — group results by directory
// ---------------------------------------------------------------------------

/// A single line match inside a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineMatch {
    pub line_num: usize,
    pub line_text: String,
}

/// All matches within a single file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileResult {
    pub file_name: String,
    pub matches: Vec<LineMatch>,
}

/// A group of files within the same directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileGroup {
    pub directory: String,
    pub files: Vec<FileResult>,
}

impl fmt::Display for FileGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({} file{}, {} match{})",
            self.directory,
            self.files.len(),
            if self.files.len() == 1 { "" } else { "s" },
            self.files.iter().map(|fr| fr.matches.len()).sum::<usize>(),
            if self.files.iter().map(|fr| fr.matches.len()).sum::<usize>() == 1 {
                ""
            } else {
                "es"
            },
        )
    }
}

/// Groups search results by their parent directory.
#[derive(Debug, Clone)]
pub struct SearchFileGrouper {
    /// directory -> (file_name -> Vec<LineMatch>)
    groups: BTreeMap<String, BTreeMap<String, Vec<LineMatch>>>,
}

impl SearchFileGrouper {
    pub fn new() -> Self {
        Self {
            groups: BTreeMap::new(),
        }
    }

    /// Record a match for the given file path.
    pub fn add_result(&mut self, file_path: &str, line_num: usize, line_text: &str) {
        let path = Path::new(file_path);
        let directory = path
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| file_path.to_string());

        self.groups
            .entry(directory)
            .or_default()
            .entry(file_name)
            .or_default()
            .push(LineMatch {
                line_num,
                line_text: line_text.to_string(),
            });
    }

    /// Return all groups sorted by directory name.
    pub fn groups(&self) -> Vec<FileGroup> {
        self.groups
            .iter()
            .map(|(dir, files_map)| FileGroup {
                directory: dir.clone(),
                files: files_map
                    .iter()
                    .map(|(name, matches)| FileResult {
                        file_name: name.clone(),
                        matches: matches.clone(),
                    })
                    .collect(),
            })
            .collect()
    }

    /// Total number of matches across all groups.
    pub fn total_matches(&self) -> usize {
        self.groups
            .values()
            .flat_map(|f| f.values())
            .map(|m| m.len())
            .sum()
    }

    /// Total number of distinct files.
    pub fn file_count(&self) -> usize {
        self.groups.values().map(|f| f.len()).sum()
    }
}

impl fmt::Display for SearchFileGrouper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} match{} in {} file{}",
            self.total_matches(),
            if self.total_matches() == 1 { "" } else { "es" },
            self.file_count(),
            if self.file_count() == 1 { "" } else { "s" },
        )
    }
}

// ---------------------------------------------------------------------------
// SearchBatchReplace — plan batch replacements across files
// ---------------------------------------------------------------------------

/// Preview of a single replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementPreview {
    pub file_path: String,
    pub line_num: usize,
    pub original: String,
    pub replaced: String,
}

impl fmt::Display for ReplacementPreview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {} -> {}",
            self.file_path, self.line_num, self.original, self.replaced
        )
    }
}

/// Planned match for batch replacement.
#[derive(Debug, Clone)]
struct PlannedMatch {
    file_path: String,
    line_num: usize,
    original_line: String,
}

/// Plans batch find-and-replace across multiple files without touching the
/// filesystem.
#[derive(Debug, Clone)]
pub struct SearchBatchReplace {
    find: String,
    replace: String,
    matches: Vec<PlannedMatch>,
}

impl SearchBatchReplace {
    pub fn new(find: &str, replace: &str) -> Self {
        Self {
            find: find.to_string(),
            replace: replace.to_string(),
            matches: Vec::new(),
        }
    }

    /// Register a match to be replaced.
    pub fn add_file_match(&mut self, file_path: &str, line_num: usize, original_line: &str) {
        self.matches.push(PlannedMatch {
            file_path: file_path.to_string(),
            line_num,
            original_line: original_line.to_string(),
        });
    }

    /// Generate replacement previews by applying a literal find/replace on each
    /// registered line.
    pub fn preview(&self) -> Vec<ReplacementPreview> {
        self.matches
            .iter()
            .map(|m| {
                let replaced = m.original_line.replace(&self.find, &self.replace);
                ReplacementPreview {
                    file_path: m.file_path.clone(),
                    line_num: m.line_num,
                    original: m.original_line.clone(),
                    replaced,
                }
            })
            .collect()
    }

    /// Number of registered matches.
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }
}

impl fmt::Display for SearchBatchReplace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Replace '{}' -> '{}' ({} match{})",
            self.find,
            self.replace,
            self.match_count(),
            if self.match_count() == 1 { "" } else { "es" },
        )
    }
}

// ---------------------------------------------------------------------------
// SearchResultPreview — context-aware preview of matches
// ---------------------------------------------------------------------------

/// A single line in a context-aware preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewLine {
    pub line_num: usize,
    pub text: String,
    pub is_match: bool,
}

impl fmt::Display for PreviewLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let marker = if self.is_match { ">" } else { " " };
        write!(f, "{} {:>4}: {}", marker, self.line_num, self.text)
    }
}

/// Builds a context-aware preview of search matches, interleaving match lines
/// with surrounding context lines.
#[derive(Debug, Clone)]
pub struct SearchResultPreview {
    context_lines: usize,
    entries: BTreeMap<usize, (String, bool)>,
}

impl SearchResultPreview {
    pub fn new(context_lines: usize) -> Self {
        Self {
            context_lines,
            entries: BTreeMap::new(),
        }
    }

    /// Add a line that is a match. If a context line was already recorded at
    /// this line number it will be promoted to a match.
    pub fn add_match(&mut self, line_num: usize, line: &str) {
        self.entries
            .insert(line_num, (line.to_string(), true));
    }

    /// Add a context (non-match) line. Will not overwrite an existing match
    /// line at the same number.
    pub fn add_context(&mut self, line_num: usize, line: &str) {
        self.entries
            .entry(line_num)
            .or_insert_with(|| (line.to_string(), false));
    }

    /// Render the preview, returning lines in line-number order.  Only context
    /// lines within `context_lines` of a match line are included.
    pub fn render(&self) -> Vec<PreviewLine> {
        let match_nums: Vec<usize> = self
            .entries
            .iter()
            .filter(|(_, (_, is_m))| *is_m)
            .map(|(n, _)| *n)
            .collect();

        self.entries
            .iter()
            .filter(|(num, (_, is_match))| {
                *is_match
                    || match_nums.iter().any(|&mn| {
                        let n = **num;
                        let dist = if n > mn { n - mn } else { mn - n };
                        dist <= self.context_lines
                    })
            })
            .map(|(num, (text, is_match))| PreviewLine {
                line_num: *num,
                text: text.clone(),
                is_match: *is_match,
            })
            .collect()
    }

    /// Number of match lines (not context lines).
    pub fn match_count(&self) -> usize {
        self.entries.values().filter(|(_, m)| *m).count()
    }
}

impl fmt::Display for SearchResultPreview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lines = self.render();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{line}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// search_view – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XSearchViewLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XSearchViewPanelState {
    pub region: XSearchViewLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XSearchViewPanelState {
    pub fn new(region: XSearchViewLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_search_view_total_visible_area(panels: &[XSearchViewPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_search_view_count_in_region(
    panels: &[XSearchViewPanelState],
    region: XSearchViewLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_search_view_widest_panel(panels: &[XSearchViewPanelState]) -> Option<&XSearchViewPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_search_view_collapse_region(
    panels: &mut [XSearchViewPanelState],
    region: XSearchViewLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XSearchViewLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XSearchViewLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}


/// Configuration manager for search_view functionality.
pub struct SearchViewConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl SearchViewConfig {
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

    pub fn merge(&mut self, other: &SearchViewConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for search_view operations.
pub struct SearchViewRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl SearchViewRateTracker {
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

/// Validation result collector for search_view.
pub struct SearchViewValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl SearchViewValidator {
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

    pub fn merge(&mut self, other: &SearchViewValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "vsedit_search_test_{}_{id}",
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

    // -- SearchQuery tests --

    #[test]
    fn query_builder() {
        let q = SearchQuery::new("foo")
            .with_regex(true)
            .with_case_sensitive(true)
            .with_whole_word(true)
            .with_include("*.rs")
            .with_exclude("target/**");
        assert_eq!(q.pattern, "foo");
        assert!(q.is_regex);
        assert!(q.is_case_sensitive);
        assert!(q.is_whole_word);
        assert_eq!(q.include_pattern.as_deref(), Some("*.rs"));
        assert_eq!(q.exclude_pattern.as_deref(), Some("target/**"));
    }

    #[test]
    fn query_build_regex_empty_pattern() {
        let q = SearchQuery::new("");
        assert!(q.build_regex().is_none());
    }

    #[test]
    fn query_build_regex_literal() {
        let q = SearchQuery::new("hello");
        let re = q.build_regex().unwrap();
        assert!(re.is_match("say hello world"));
        // Case insensitive by default
        assert!(re.is_match("HELLO"));
    }

    #[test]
    fn query_build_regex_case_sensitive() {
        let q = SearchQuery::new("Hello").with_case_sensitive(true);
        let re = q.build_regex().unwrap();
        assert!(re.is_match("Hello"));
        assert!(!re.is_match("hello"));
    }

    #[test]
    fn query_build_regex_whole_word() {
        let q = SearchQuery::new("he").with_whole_word(true);
        let re = q.build_regex().unwrap();
        assert!(re.is_match("he said"));
        assert!(!re.is_match("hello"));
    }

    // -- SearchMatch / FileMatches / SearchResults tests --

    #[test]
    fn file_matches_count() {
        let fm = FileMatches::new(
            PathBuf::from("test.rs"),
            vec![
                SearchMatch {
                    file_path: PathBuf::from("test.rs"),
                    line_number: 1,
                    column: 1,
                    line_content: "hello".into(),
                    match_range: 0..5,
                    preview: "hello".into(),
                },
                SearchMatch {
                    file_path: PathBuf::from("test.rs"),
                    line_number: 3,
                    column: 1,
                    line_content: "hello again".into(),
                    match_range: 0..5,
                    preview: "hello again".into(),
                },
            ],
        );
        assert_eq!(fm.match_count(), 2);
        assert!(fm.is_expanded);
    }

    #[test]
    fn search_results_totals() {
        let results = SearchResults::new(vec![
            FileMatches::new(
                PathBuf::from("a.rs"),
                vec![SearchMatch {
                    file_path: PathBuf::from("a.rs"),
                    line_number: 1,
                    column: 1,
                    line_content: "x".into(),
                    match_range: 0..1,
                    preview: "x".into(),
                }],
            ),
            FileMatches::new(
                PathBuf::from("b.rs"),
                vec![
                    SearchMatch {
                        file_path: PathBuf::from("b.rs"),
                        line_number: 1,
                        column: 1,
                        line_content: "x".into(),
                        match_range: 0..1,
                        preview: "x".into(),
                    },
                    SearchMatch {
                        file_path: PathBuf::from("b.rs"),
                        line_number: 2,
                        column: 1,
                        line_content: "x".into(),
                        match_range: 0..1,
                        preview: "x".into(),
                    },
                ],
            ),
        ]);
        assert_eq!(results.total_matches(), 3);
        assert_eq!(results.total_files(), 2);
    }

    #[test]
    fn search_results_empty() {
        let results = SearchResults::empty();
        assert_eq!(results.total_matches(), 0);
        assert_eq!(results.total_files(), 0);
    }

    // -- SearchEngine tests --

    #[test]
    fn search_in_file_literal() {
        let dir = temp_dir();
        let path = write_file(&dir, "sample.txt", "hello world\ngoodbye world\nhello again");
        let q = SearchQuery::new("hello");
        let matches = SearchEngine::search_in_file(&q, &path);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line_number, 1);
        assert_eq!(matches[0].column, 1);
        assert_eq!(matches[1].line_number, 3);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn search_in_file_regex() {
        let dir = temp_dir();
        let path = write_file(&dir, "nums.txt", "foo 123 bar\nbaz 456 qux");
        let q = SearchQuery::new(r"\d+").with_regex(true);
        let matches = SearchEngine::search_in_file(&q, &path);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line_content, "foo 123 bar");
        assert_eq!(matches[0].match_range, 4..7);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn search_walks_directory() {
        let dir = temp_dir();
        write_file(&dir, "a.txt", "find_me here");
        write_file(&dir, "sub/b.txt", "also find_me");
        write_file(&dir, "c.txt", "nothing here");

        let q = SearchQuery::new("find_me");
        let results = SearchEngine::search(&q, &dir);
        assert_eq!(results.total_matches(), 2);
        assert_eq!(results.total_files(), 2);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn search_skips_binary_files() {
        let dir = temp_dir();
        write_file(&dir, "text.txt", "hello");
        let bin_path = dir.join("binary.bin");
        fs::write(&bin_path, b"hello\x00world").unwrap();

        let q = SearchQuery::new("hello");
        let results = SearchEngine::search(&q, &dir);
        assert_eq!(results.total_files(), 1);
        assert_eq!(results.files()[0].file_path.file_name().unwrap(), "text.txt");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn search_include_pattern() {
        let dir = temp_dir();
        write_file(&dir, "code.rs", "find_me");
        write_file(&dir, "readme.md", "find_me");

        let q = SearchQuery::new("find_me").with_include("*.rs");
        let results = SearchEngine::search(&q, &dir);
        assert_eq!(results.total_files(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn search_exclude_pattern() {
        let dir = temp_dir();
        write_file(&dir, "code.rs", "find_me");
        write_file(&dir, "readme.md", "find_me");

        let q = SearchQuery::new("find_me").with_exclude("*.md");
        let results = SearchEngine::search(&q, &dir);
        assert_eq!(results.total_files(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    // -- ReplaceOperation tests --

    #[test]
    fn replace_single_match() {
        let op = ReplaceOperation::new("universe");
        let m = SearchMatch {
            file_path: PathBuf::from("test.rs"),
            line_number: 1,
            column: 7,
            line_content: "hello world".into(),
            match_range: 6..11,
            preview: "hello world".into(),
        };
        assert_eq!(op.replace_match(&m), "hello universe");
    }

    #[test]
    fn replace_all_in_file_works() {
        let dir = temp_dir();
        let path = write_file(&dir, "replace.txt", "hello world\ngoodbye world\n");
        let fm = FileMatches::new(
            path.clone(),
            vec![
                SearchMatch {
                    file_path: path.clone(),
                    line_number: 1,
                    column: 7,
                    line_content: "hello world".into(),
                    match_range: 6..11,
                    preview: "hello world".into(),
                },
                SearchMatch {
                    file_path: path.clone(),
                    line_number: 2,
                    column: 9,
                    line_content: "goodbye world".into(),
                    match_range: 8..13,
                    preview: "goodbye world".into(),
                },
            ],
        );

        let op = ReplaceOperation::new("universe");
        assert!(op.replace_all_in_file(&fm));

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("hello universe"));
        assert!(content.contains("goodbye universe"));
        fs::remove_dir_all(&dir).unwrap();
    }

    // -- SearchView tests --

    #[test]
    fn view_creation() {
        let v = SearchView::new();
        assert!(v.search_text.is_empty());
        assert!(!v.is_regex);
        assert!(!v.is_case_sensitive);
        assert!(!v.is_whole_word);
        assert_eq!(v.active_field, ActiveField::Search);
        assert_eq!(v.results.total_matches(), 0);
    }

    #[test]
    fn view_default_trait() {
        let v = SearchView::default();
        assert!(v.search_text.is_empty());
    }

    #[test]
    fn view_build_query() {
        let mut v = SearchView::new();
        v.search_text = "pattern".into();
        v.is_regex = true;
        v.is_case_sensitive = true;
        v.include_text = "*.rs".into();

        let q = v.build_query();
        assert_eq!(q.pattern, "pattern");
        assert!(q.is_regex);
        assert!(q.is_case_sensitive);
        assert_eq!(q.include_pattern.as_deref(), Some("*.rs"));
    }

    #[test]
    fn view_execute_search() {
        let dir = temp_dir();
        write_file(&dir, "f.txt", "needle in haystack\nneedle again");

        let mut v = SearchView::new();
        v.search_text = "needle".into();
        v.execute_search(&dir);

        assert_eq!(v.results.total_matches(), 2);
        assert_eq!(v.selected_result, Some(0));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn view_navigation() {
        let mut v = SearchView::new();
        v.results = SearchResults::new(vec![FileMatches::new(
            PathBuf::from("a.txt"),
            vec![
                SearchMatch {
                    file_path: PathBuf::from("a.txt"),
                    line_number: 1,
                    column: 1,
                    line_content: "a".into(),
                    match_range: 0..1,
                    preview: "a".into(),
                },
                SearchMatch {
                    file_path: PathBuf::from("a.txt"),
                    line_number: 2,
                    column: 1,
                    line_content: "a".into(),
                    match_range: 0..1,
                    preview: "a".into(),
                },
            ],
        )]);
        v.selected_result = Some(0);

        // 1 file header + 2 matches = 3 entries
        v.select_next();
        assert_eq!(v.selected_result, Some(1));
        v.select_next();
        assert_eq!(v.selected_result, Some(2));
        v.select_next(); // wraps
        assert_eq!(v.selected_result, Some(0));

        v.select_previous(); // wraps back
        assert_eq!(v.selected_result, Some(2));
    }

    #[test]
    fn view_toggle_expand() {
        let mut v = SearchView::new();
        v.results = SearchResults::new(vec![FileMatches::new(
            PathBuf::from("a.txt"),
            vec![SearchMatch {
                file_path: PathBuf::from("a.txt"),
                line_number: 1,
                column: 1,
                line_content: "x".into(),
                match_range: 0..1,
                preview: "x".into(),
            }],
        )]);

        assert!(v.results.files()[0].is_expanded);
        v.toggle_file_expanded(0);
        assert!(!v.results.files()[0].is_expanded);
        v.toggle_file_expanded(0);
        assert!(v.results.files()[0].is_expanded);
    }

    #[test]
    fn render_does_not_panic_empty() {
        let v = SearchView::new();
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        v.render(area, &mut buf);
    }

    #[test]
    fn render_does_not_panic_with_results() {
        let mut v = SearchView::new();
        v.search_text = "hello".into();
        v.results = SearchResults::new(vec![FileMatches::new(
            PathBuf::from("test.rs"),
            vec![SearchMatch {
                file_path: PathBuf::from("test.rs"),
                line_number: 1,
                column: 1,
                line_content: "hello world".into(),
                match_range: 0..5,
                preview: "hello world".into(),
            }],
        )]);
        v.selected_result = Some(0);
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        v.render(area, &mut buf);
    }

    #[test]
    fn render_zero_area() {
        let v = SearchView::new();
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(area);
        v.render(area, &mut buf);
    }

    #[test]
    fn preview_context() {
        let line = "the quick brown fox jumps over the lazy dog";
        let preview = SearchEngine::build_preview(line, 16, 19);
        // Should contain "fox" and surrounding context
        assert!(preview.contains("fox"));
    }

    // -- Wired service tests (vsedit-wb-search integration) --

    #[test]
    fn execute_search_via_service_finds_files() {
        let dir = temp_dir();
        write_file(&dir, "data.txt", "needle in haystack\nneedle again");
        write_file(&dir, "other.txt", "no match here");

        let mut v = SearchView::new();
        v.search_text = "needle".into();
        v.execute_search_via_service(&dir);

        assert_eq!(v.results.total_matches(), 2);
        assert_eq!(v.results.total_files(), 1);
        assert_eq!(v.selected_result, Some(0));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn execute_search_via_service_with_include() {
        let dir = temp_dir();
        write_file(&dir, "code.rs", "find_me");
        write_file(&dir, "readme.md", "find_me");

        let mut v = SearchView::new();
        v.search_text = "find_me".into();
        v.include_text = "*.rs".into();
        v.execute_search_via_service(&dir);

        assert_eq!(v.results.total_files(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn preview_replace_selected_works() {
        let mut v = SearchView::new();
        v.replace_text = "universe".into();
        v.results = SearchResults::new(vec![FileMatches::new(
            PathBuf::from("test.rs"),
            vec![SearchMatch {
                file_path: PathBuf::from("test.rs"),
                line_number: 1,
                column: 6,
                line_content: "hello world".into(),
                match_range: 6..11,
                preview: "hello world".into(),
            }],
        )]);
        // Entry 0 is file header, entry 1 is the match
        v.selected_result = Some(1);
        let preview = v.preview_replace_selected().unwrap();
        assert_eq!(preview, "hello universe");
    }

    #[test]
    fn preview_replace_on_file_header_returns_none() {
        let mut v = SearchView::new();
        v.replace_text = "x".into();
        v.results = SearchResults::new(vec![FileMatches::new(
            PathBuf::from("a.txt"),
            vec![SearchMatch {
                file_path: PathBuf::from("a.txt"),
                line_number: 1,
                column: 0,
                line_content: "hello".into(),
                match_range: 0..5,
                preview: "hello".into(),
            }],
        )]);
        v.selected_result = Some(0); // file header
        assert!(v.preview_replace_selected().is_none());
    }

    #[test]
    fn replace_all_via_service_modifies_files() {
        let dir = temp_dir();
        write_file(&dir, "a.txt", "foo bar\nfoo baz\n");

        let mut v = SearchView::new();
        v.search_text = "foo".into();
        v.is_case_sensitive = true;
        v.replace_text = "qux".into();

        let count = v.replace_all_via_service(&dir);
        assert_eq!(count, 2);

        let content = fs::read_to_string(dir.join("a.txt")).unwrap();
        assert!(content.contains("qux bar"));
        assert!(content.contains("qux baz"));
        assert!(!content.contains("foo"));

        // Results should be empty after replace (no more matches)
        assert_eq!(v.results.total_matches(), 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn get_match_at_entry_returns_correct_match() {
        let mut v = SearchView::new();
        v.results = SearchResults::new(vec![FileMatches::new(
            PathBuf::from("test.rs"),
            vec![
                SearchMatch {
                    file_path: PathBuf::from("test.rs"),
                    line_number: 5,
                    column: 3,
                    line_content: "abc".into(),
                    match_range: 3..6,
                    preview: "abc".into(),
                },
                SearchMatch {
                    file_path: PathBuf::from("test.rs"),
                    line_number: 10,
                    column: 0,
                    line_content: "def".into(),
                    match_range: 0..3,
                    preview: "def".into(),
                },
            ],
        )]);

        assert!(v.get_match_at_entry(0).is_none()); // file header
        let m1 = v.get_match_at_entry(1).unwrap();
        assert_eq!(m1.line_number, 5);
        let m2 = v.get_match_at_entry(2).unwrap();
        assert_eq!(m2.line_number, 10);
        assert!(v.get_match_at_entry(3).is_none()); // out of bounds
    }

    #[test]
    fn re_exported_types_accessible() {
        // Verify re-exports from vsedit-wb-search are available
        let _qp = FileQuickPick::new();
        let _sk = SymbolKind::Function;
        let _rq = ReplaceQuery::new(
            vsedit_wb_search::SearchQuery {
                pattern: "x".into(),
                is_regex: false,
                case_sensitive: true,
                whole_word: false,
                include_pattern: None,
                exclude_pattern: None,
            },
            "y",
        );
    }

    // -- SearchStatistics tests --

    #[test]
    fn statistics_from_results() {
        // Two files: file a has 3 matches on 2 lines, file b has 1 match on 1 line
        let results = SearchResults::new(vec![
            FileMatches::new(
                PathBuf::from("a.rs"),
                vec![
                    SearchMatch {
                        file_path: PathBuf::from("a.rs"),
                        line_number: 1,
                        column: 1,
                        line_content: "xx".into(),
                        match_range: 0..1,
                        preview: "xx".into(),
                    },
                    SearchMatch {
                        file_path: PathBuf::from("a.rs"),
                        line_number: 1,
                        column: 2,
                        line_content: "xx".into(),
                        match_range: 1..2,
                        preview: "xx".into(),
                    },
                    SearchMatch {
                        file_path: PathBuf::from("a.rs"),
                        line_number: 5,
                        column: 1,
                        line_content: "x".into(),
                        match_range: 0..1,
                        preview: "x".into(),
                    },
                ],
            ),
            FileMatches::new(
                PathBuf::from("b.rs"),
                vec![SearchMatch {
                    file_path: PathBuf::from("b.rs"),
                    line_number: 10,
                    column: 1,
                    line_content: "x".into(),
                    match_range: 0..1,
                    preview: "x".into(),
                }],
            ),
        ]);
        let stats = results.statistics();
        assert_eq!(stats.files_matched, 2);
        assert_eq!(stats.total_matches, 4);
        assert_eq!(stats.lines_with_matches, 3); // lines 1,5 in a.rs + line 10 in b.rs
        assert_eq!(stats.summary(), "4 matches across 2 files, 3 lines");
    }

    #[test]
    fn statistics_summary_singular() {
        let stats = SearchStatistics {
            files_matched: 1,
            total_matches: 1,
            lines_with_matches: 1,
        };
        assert_eq!(stats.summary(), "1 match across 1 file, 1 line");
    }

    // -- SearchQueryHistory tests --

    #[test]
    fn query_history_deduplication_and_ordering() {
        let mut h = SearchQueryHistory::new(5);
        h.push("alpha");
        h.push("beta");
        h.push("alpha"); // duplicate, should move to front
        assert_eq!(h.entries(), &["alpha", "beta"]);
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn query_history_capacity_and_navigation() {
        let mut h = SearchQueryHistory::new(3);
        h.push("one");
        h.push("two");
        h.push("three");
        h.push("four"); // "one" should be evicted
        assert_eq!(h.entries(), &["four", "three", "two"]);

        // Navigate backwards through history
        assert_eq!(h.previous(), Some("four"));
        assert_eq!(h.previous(), Some("three"));
        assert_eq!(h.previous(), Some("two"));
        assert_eq!(h.previous(), None); // at the end

        // Navigate forwards
        assert_eq!(h.next(), Some("three"));
        assert_eq!(h.next(), Some("four"));
        assert_eq!(h.next(), None); // at the newest

        // Reset cursor
        h.reset_cursor();
        assert_eq!(h.previous(), Some("four"));
    }

    #[test]
    fn query_history_ignores_empty() {
        let mut h = SearchQueryHistory::new(5);
        h.push("");
        assert!(h.is_empty());
    }

    // -- collapse/expand all tests --

    #[test]
    fn collapse_and_expand_all() {
        let mut results = SearchResults::new(vec![
            FileMatches::new(
                PathBuf::from("a.rs"),
                vec![SearchMatch {
                    file_path: PathBuf::from("a.rs"),
                    line_number: 1,
                    column: 1,
                    line_content: "x".into(),
                    match_range: 0..1,
                    preview: "x".into(),
                }],
            ),
            FileMatches::new(
                PathBuf::from("b.rs"),
                vec![SearchMatch {
                    file_path: PathBuf::from("b.rs"),
                    line_number: 1,
                    column: 1,
                    line_content: "x".into(),
                    match_range: 0..1,
                    preview: "x".into(),
                }],
            ),
        ]);

        // All start expanded
        assert!(results.files().iter().all(|f| f.is_expanded));

        results.collapse_all();
        assert!(results.files().iter().all(|f| !f.is_expanded));

        results.expand_all();
        assert!(results.files().iter().all(|f| f.is_expanded));
    }

    // -- next_match / previous_match navigation tests --

    #[test]
    fn next_and_previous_match_navigation() {
        let mut v = SearchView::new();
        v.results = SearchResults::new(vec![
            FileMatches::new(
                PathBuf::from("a.txt"),
                vec![
                    SearchMatch {
                        file_path: PathBuf::from("a.txt"),
                        line_number: 1,
                        column: 1,
                        line_content: "aaa".into(),
                        match_range: 0..1,
                        preview: "aaa".into(),
                    },
                    SearchMatch {
                        file_path: PathBuf::from("a.txt"),
                        line_number: 2,
                        column: 1,
                        line_content: "aaa".into(),
                        match_range: 0..1,
                        preview: "aaa".into(),
                    },
                ],
            ),
            FileMatches::new(
                PathBuf::from("b.txt"),
                vec![SearchMatch {
                    file_path: PathBuf::from("b.txt"),
                    line_number: 1,
                    column: 1,
                    line_content: "bbb".into(),
                    match_range: 0..1,
                    preview: "bbb".into(),
                }],
            ),
        ]);
        v.selected_result = None;

        // First next_match goes to a.txt match 0
        let loc = v.next_match().unwrap();
        assert_eq!(loc, MatchLocation { file_index: 0, match_index: 0 });

        // Second goes to a.txt match 1
        let loc = v.next_match().unwrap();
        assert_eq!(loc, MatchLocation { file_index: 0, match_index: 1 });

        // Third goes to b.txt match 0
        let loc = v.next_match().unwrap();
        assert_eq!(loc, MatchLocation { file_index: 1, match_index: 0 });

        // Fourth wraps back to a.txt match 0
        let loc = v.next_match().unwrap();
        assert_eq!(loc, MatchLocation { file_index: 0, match_index: 0 });

        // Previous goes back to b.txt match 0
        let loc = v.previous_match().unwrap();
        assert_eq!(loc, MatchLocation { file_index: 1, match_index: 0 });
    }

    // -- preview_replace_for_file tests --

    #[test]
    fn preview_replace_for_file_multiple_matches() {
        let mut v = SearchView::new();
        v.replace_text = "YY".into();
        v.results = SearchResults::new(vec![FileMatches::new(
            PathBuf::from("f.rs"),
            vec![
                SearchMatch {
                    file_path: PathBuf::from("f.rs"),
                    line_number: 1,
                    column: 1,
                    line_content: "aa bb aa".into(),
                    match_range: 0..2,
                    preview: "aa bb aa".into(),
                },
                SearchMatch {
                    file_path: PathBuf::from("f.rs"),
                    line_number: 1,
                    column: 7,
                    line_content: "aa bb aa".into(),
                    match_range: 6..8,
                    preview: "aa bb aa".into(),
                },
                SearchMatch {
                    file_path: PathBuf::from("f.rs"),
                    line_number: 3,
                    column: 1,
                    line_content: "aa only".into(),
                    match_range: 0..2,
                    preview: "aa only".into(),
                },
            ],
        )]);

        let previews = v.preview_replace_for_file(0);
        assert_eq!(previews.len(), 2); // two unique lines
        assert_eq!(previews[0].0, "aa bb aa");
        assert_eq!(previews[0].1, "YY bb YY"); // both matches replaced
        assert_eq!(previews[1].0, "aa only");
        assert_eq!(previews[1].1, "YY only");
    }

    #[test]
    fn preview_replace_for_file_invalid_index() {
        let v = SearchView::new();
        assert!(v.preview_replace_for_file(99).is_empty());
    }

    #[test]
    fn view_statistics_method() {
        let mut v = SearchView::new();
        v.results = SearchResults::new(vec![FileMatches::new(
            PathBuf::from("z.rs"),
            vec![
                SearchMatch {
                    file_path: PathBuf::from("z.rs"),
                    line_number: 1,
                    column: 1,
                    line_content: "x".into(),
                    match_range: 0..1,
                    preview: "x".into(),
                },
                SearchMatch {
                    file_path: PathBuf::from("z.rs"),
                    line_number: 2,
                    column: 1,
                    line_content: "x".into(),
                    match_range: 0..1,
                    preview: "x".into(),
                },
            ],
        )]);
        let stats = v.statistics();
        assert_eq!(stats.files_matched, 1);
        assert_eq!(stats.total_matches, 2);
        assert_eq!(stats.lines_with_matches, 2);
    }

    // -- SearchResultDecorator tests --

    #[test]
    fn decorator_single_match() {
        let dec = SearchResultDecorator::new();
        let line = "hello world";
        let result = dec.decorate(line, 6, 11);
        assert_eq!(result.before, "hello ");
        assert_eq!(result.matched, "world");
        assert_eq!(result.after, "");
        assert_eq!(result.to_string(), "hello [world]");
    }

    #[test]
    fn decorator_match_at_start() {
        let dec = SearchResultDecorator::new();
        let result = dec.decorate("foobar", 0, 3);
        assert_eq!(result.before, "");
        assert_eq!(result.matched, "foo");
        assert_eq!(result.after, "bar");
    }

    #[test]
    fn decorator_out_of_bounds_clamped() {
        let dec = SearchResultDecorator::new();
        let result = dec.decorate("abc", 1, 100);
        assert_eq!(result.matched, "bc");
        assert_eq!(result.after, "");
    }

    #[test]
    fn decorator_all_multiple_ranges() {
        let dec = SearchResultDecorator::new();
        let segs = dec.decorate_all("abcdefgh", &[(0, 2), (4, 6)]);
        assert_eq!(segs.len(), 4);
        assert_eq!(segs[0], DecoratedSegment { text: "ab".into(), is_match: true });
        assert_eq!(segs[1], DecoratedSegment { text: "cd".into(), is_match: false });
        assert_eq!(segs[2], DecoratedSegment { text: "ef".into(), is_match: true });
        assert_eq!(segs[3], DecoratedSegment { text: "gh".into(), is_match: false });
    }

    // -- SearchFileGrouper tests --

    #[test]
    fn grouper_groups_by_directory() {
        let mut g = SearchFileGrouper::new();
        g.add_result("src/main.rs", 10, "use std;");
        g.add_result("src/lib.rs", 5, "mod foo;");
        g.add_result("tests/test.rs", 1, "assert!");
        let groups = g.groups();
        assert_eq!(groups.len(), 2);
        assert_eq!(g.file_count(), 3);
        assert_eq!(g.total_matches(), 3);
    }

    #[test]
    fn grouper_multiple_matches_same_file() {
        let mut g = SearchFileGrouper::new();
        g.add_result("src/lib.rs", 1, "first");
        g.add_result("src/lib.rs", 5, "second");
        assert_eq!(g.total_matches(), 2);
        assert_eq!(g.file_count(), 1);
        let groups = g.groups();
        assert_eq!(groups[0].files[0].matches.len(), 2);
    }

    #[test]
    fn grouper_display() {
        let mut g = SearchFileGrouper::new();
        g.add_result("a/b.rs", 1, "x");
        let display = format!("{g}");
        assert!(display.contains("1 match"));
    }

    // -- SearchBatchReplace tests --

    #[test]
    fn batch_replace_preview() {
        let mut br = SearchBatchReplace::new("foo", "bar");
        br.add_file_match("src/main.rs", 10, "let foo = foo;");
        br.add_file_match("src/lib.rs", 3, "fn foo()");
        assert_eq!(br.match_count(), 2);
        let previews = br.preview();
        assert_eq!(previews[0].replaced, "let bar = bar;");
        assert_eq!(previews[1].replaced, "fn bar()");
        assert_eq!(previews[0].file_path, "src/main.rs");
    }

    #[test]
    fn batch_replace_display() {
        let br = SearchBatchReplace::new("old", "new");
        let s = format!("{br}");
        assert!(s.contains("'old' -> 'new'"));
        assert!(s.contains("0 matches"));
    }

    // -- SearchResultPreview tests --

    #[test]
    fn preview_renders_with_context() {
        let mut p = SearchResultPreview::new(1);
        p.add_context(1, "line one");
        p.add_match(2, "MATCH line two");
        p.add_context(3, "line three");
        p.add_context(4, "line four");
        let lines = p.render();
        assert_eq!(lines.len(), 3);
        assert!(!lines[0].is_match);
        assert!(lines[1].is_match);
        assert!(!lines[2].is_match);
        assert_eq!(p.match_count(), 1);
    }

    #[test]
    fn preview_context_excludes_distant_lines() {
        let mut p = SearchResultPreview::new(0);
        p.add_context(1, "far away");
        p.add_match(5, "match");
        p.add_context(10, "also far");
        let lines = p.render();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].is_match);
    }

    #[test]
    fn preview_display_format() {
        let mut p = SearchResultPreview::new(0);
        p.add_match(42, "found it");
        let s = format!("{p}");
        assert!(s.contains(">"));
        assert!(s.contains("42"));
        assert!(s.contains("found it"));
    }

    #[test]
    fn view_statistics_after_new_structs() {
        let mut v = SearchView::new();
        v.results = SearchResults::new(vec![FileMatches::new(
            PathBuf::from("z.rs"),
            vec![
                SearchMatch {
                    file_path: PathBuf::from("z.rs"),
                    line_number: 1,
                    column: 1,
                    line_content: "x".into(),
                    match_range: 0..1,
                    preview: "x".into(),
                },
                SearchMatch {
                    file_path: PathBuf::from("z.rs"),
                    line_number: 2,
                    column: 1,
                    line_content: "x".into(),
                    match_range: 0..1,
                    preview: "x".into(),
                },
            ],
        )]);
        let stats = v.statistics();
        assert_eq!(stats.files_matched, 1);
        assert_eq!(stats.total_matches, 2);
        assert_eq!(stats.lines_with_matches, 2);
    }

    // -- search_view additional tests -------------------------------------------

    #[test]
    fn x_search_view_panel_state_new() {
        let p = XSearchViewPanelState::new(XSearchViewLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XSearchViewLayoutRegion::Sidebar);
    }

    #[test]
    fn x_search_view_panel_area() {
        let p = XSearchViewPanelState::new(XSearchViewLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_search_view_panel_toggle() {
        let mut p = XSearchViewPanelState::new(XSearchViewLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_search_view_panel_resize() {
        let mut p = XSearchViewPanelState::new(XSearchViewLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_search_view_panel_is_narrow() {
        let mut p = XSearchViewPanelState::new(XSearchViewLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_search_view_total_visible_area_basic() {
        let panels = vec![
            XSearchViewPanelState::new(XSearchViewLayoutRegion::Sidebar, "a"),
            XSearchViewPanelState::new(XSearchViewLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_search_view_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_search_view_total_visible_area_hidden() {
        let mut panels = vec![
            XSearchViewPanelState::new(XSearchViewLayoutRegion::Sidebar, "a"),
            XSearchViewPanelState::new(XSearchViewLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_search_view_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_search_view_count_in_region_basic() {
        let panels = vec![
            XSearchViewPanelState::new(XSearchViewLayoutRegion::Sidebar, "a"),
            XSearchViewPanelState::new(XSearchViewLayoutRegion::Sidebar, "b"),
            XSearchViewPanelState::new(XSearchViewLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_search_view_count_in_region(&panels, XSearchViewLayoutRegion::Sidebar), 2);
        assert_eq!(x_search_view_count_in_region(&panels, XSearchViewLayoutRegion::Editor), 1);
        assert_eq!(x_search_view_count_in_region(&panels, XSearchViewLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_search_view_widest_panel_basic() {
        let mut panels = vec![
            XSearchViewPanelState::new(XSearchViewLayoutRegion::Sidebar, "narrow"),
            XSearchViewPanelState::new(XSearchViewLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_search_view_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_search_view_collapse_region_basic() {
        let mut panels = vec![
            XSearchViewPanelState::new(XSearchViewLayoutRegion::Sidebar, "a"),
            XSearchViewPanelState::new(XSearchViewLayoutRegion::Sidebar, "b"),
            XSearchViewPanelState::new(XSearchViewLayoutRegion::Editor, "c"),
        ];
        x_search_view_collapse_region(&mut panels, XSearchViewLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_search_view_layout_constraint_clamp() {
        let lc = XSearchViewLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_search_view_layout_constraint_satisfied() {
        let lc = XSearchViewLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_search_view_widest_panel_empty() {
        let panels: Vec<XSearchViewPanelState> = vec![];
        assert!(x_search_view_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_search_view_layout_region_eq() {
        assert_eq!(XSearchViewLayoutRegion::Sidebar, XSearchViewLayoutRegion::Sidebar);
        assert_ne!(XSearchViewLayoutRegion::Sidebar, XSearchViewLayoutRegion::Panel);
    }


    #[test]
    fn search_view_config_new() {
        let cfg = SearchViewConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn search_view_config_set_get() {
        let mut cfg = SearchViewConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn search_view_config_remove() {
        let mut cfg = SearchViewConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn search_view_config_keys_sorted() {
        let mut cfg = SearchViewConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn search_view_config_bump_version() {
        let mut cfg = SearchViewConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn search_view_config_clear() {
        let mut cfg = SearchViewConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn search_view_config_merge() {
        let mut cfg1 = SearchViewConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = SearchViewConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn search_view_config_disable() {
        let mut cfg = SearchViewConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn search_view_rate_tracker_empty() {
        let rt = SearchViewRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn search_view_rate_tracker_record() {
        let mut rt = SearchViewRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn search_view_rate_tracker_prune() {
        let mut rt = SearchViewRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn search_view_validator_valid() {
        let v = SearchViewValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn search_view_validator_errors() {
        let mut v = SearchViewValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn search_view_validator_clear() {
        let mut v = SearchViewValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn search_view_validator_merge() {
        let mut v1 = SearchViewValidator::new();
        v1.add_error("e1");
        let mut v2 = SearchViewValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn search_view_rate_tracker_clear() {
        let mut rt = SearchViewRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }

}
