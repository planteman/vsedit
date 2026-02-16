//! Search across files view — equivalent to VS Code's search sidebar (Ctrl+Shift+F).
//!
//! Provides [`SearchQuery`], [`SearchEngine`], [`SearchResults`], [`SearchView`],
//! and [`ReplaceOperation`] for workspace-wide find and replace.
//!
//! Also re-exports file-system search, replace, fuzzy file name search, and
//! symbol extraction from [`vsedit_wb_search`].

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
// Tests
// ---------------------------------------------------------------------------

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
    fn replace_all_in_file() {
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
}
