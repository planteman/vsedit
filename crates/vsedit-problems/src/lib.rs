//! Problems panel view.
//!
//! Displays diagnostics (errors, warnings, info, hints) grouped by file
//! with filtering and sorting — rendered via ratatui.

use std::fmt;
use std::collections::{BTreeMap, HashMap, HashSet};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Severity level of a diagnostic problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProblemSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl ProblemSeverity {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Error => "✖",
            Self::Warning => "⚠",
            Self::Info => "ℹ",
            Self::Hint => "💡",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Self::Error => Color::Red,
            Self::Warning => Color::Yellow,
            Self::Info => Color::Blue,
            Self::Hint => Color::Green,
        }
    }

    /// Numeric severity level where higher means more severe.
    /// Error(3) > Warning(2) > Info(1) > Hint(0).
    pub fn severity_level(&self) -> u8 {
        match self {
            Self::Error => 3,
            Self::Warning => 2,
            Self::Info => 1,
            Self::Hint => 0,
        }
    }
}

/// How to sort the problems list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    File,
    Severity,
    Message,
}

/// A single diagnostic problem.
#[derive(Debug, Clone)]
pub struct Problem {
    pub severity: ProblemSeverity,
    pub message: String,
    pub source: String,
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub code: Option<String>,
}

impl Problem {
    pub fn new(
        severity: ProblemSeverity,
        message: impl Into<String>,
        source: impl Into<String>,
        file_path: impl Into<String>,
        line: u32,
        column: u32,
    ) -> Self {
        Self {
            severity,
            message: message.into(),
            source: source.into(),
            file_path: file_path.into(),
            line,
            column,
            code: None,
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
}

/// Filters applied to the problem list.
#[derive(Debug, Clone)]
pub struct ProblemFilter {
    pub show_errors: bool,
    pub show_warnings: bool,
    pub show_info: bool,
    pub filter_text: String,
}

impl Default for ProblemFilter {
    fn default() -> Self {
        Self {
            show_errors: true,
            show_warnings: true,
            show_info: true,
            filter_text: String::new(),
        }
    }
}

/// Problems grouped by file path.
#[derive(Debug, Clone)]
pub struct GroupedProblems {
    pub file_path: String,
    pub problems: Vec<Problem>,
}

// ---------------------------------------------------------------------------
// ProblemsPanel
// ---------------------------------------------------------------------------

/// Problems panel that displays diagnostics with filtering and sorting.
#[derive(Debug, Clone)]
pub struct ProblemsPanel {
    pub problems: Vec<Problem>,
    pub filter: ProblemFilter,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub sort_by: SortBy,
}

impl ProblemsPanel {
    pub fn new() -> Self {
        Self {
            problems: Vec::new(),
            filter: ProblemFilter::default(),
            selected_index: 0,
            scroll_offset: 0,
            sort_by: SortBy::File,
        }
    }

    /// Get filtered problems based on current filter settings.
    pub fn filtered_problems(&self) -> Vec<&Problem> {
        self.problems
            .iter()
            .filter(|p| match p.severity {
                ProblemSeverity::Error => self.filter.show_errors,
                ProblemSeverity::Warning => self.filter.show_warnings,
                ProblemSeverity::Info | ProblemSeverity::Hint => self.filter.show_info,
            })
            .filter(|p| {
                if self.filter.filter_text.is_empty() {
                    true
                } else {
                    let lower = self.filter.filter_text.to_lowercase();
                    p.message.to_lowercase().contains(&lower)
                        || p.file_path.to_lowercase().contains(&lower)
                        || p.source.to_lowercase().contains(&lower)
                }
            })
            .collect()
    }

    /// Group filtered problems by file path.
    pub fn grouped_by_file(&self) -> Vec<GroupedProblems> {
        let mut groups: BTreeMap<String, Vec<Problem>> = BTreeMap::new();
        for p in self.filtered_problems() {
            groups
                .entry(p.file_path.clone())
                .or_default()
                .push(p.clone());
        }
        groups
            .into_iter()
            .map(|(file_path, problems)| GroupedProblems {
                file_path,
                problems,
            })
            .collect()
    }

    /// Count problems by severity.
    pub fn count_by_severity(&self, severity: ProblemSeverity) -> usize {
        self.problems.iter().filter(|p| p.severity == severity).count()
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        let len = self.filtered_problems().len();
        if len > 0 {
            self.selected_index = (self.selected_index + 1).min(len - 1);
        }
    }

    /// Move selection up.
    pub fn select_previous(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    /// Render the problems panel.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 || area.width < 10 {
            return;
        }

        // Filter / counts bar (first row).
        let bar_area = Rect { height: 1, ..area };
        self.render_counts_bar(bar_area, buf);

        // Problem list (remaining rows).
        let list_area = Rect {
            y: area.y + 1,
            height: area.height.saturating_sub(1),
            ..area
        };
        self.render_problem_list(list_area, buf);
    }

    fn render_counts_bar(&self, area: Rect, buf: &mut Buffer) {
        let errors = self.count_by_severity(ProblemSeverity::Error);
        let warnings = self.count_by_severity(ProblemSeverity::Warning);
        let infos = self.count_by_severity(ProblemSeverity::Info);

        let spans = vec![
            Span::styled(
                format!("✖ {} ", errors),
                Style::default().fg(Color::Red),
            ),
            Span::styled(
                format!("⚠ {} ", warnings),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                format!("ℹ {} ", infos),
                Style::default().fg(Color::Blue),
            ),
        ];
        let line = Line::from(spans);
        line.render(area, buf);
    }

    fn render_problem_list(&self, area: Rect, buf: &mut Buffer) {
        let filtered = self.filtered_problems();

        if filtered.is_empty() {
            let msg = Line::from(Span::styled(
                "No problems have been detected",
                Style::default().fg(Color::DarkGray),
            ));
            msg.render(area, buf);
            return;
        }

        let visible = area.height as usize;
        let start = self.scroll_offset;

        for (i, problem) in filtered.iter().skip(start).take(visible).enumerate() {
            let is_selected = start + i == self.selected_index;
            let icon = problem.severity.icon();
            let label = format!(
                "{} {}:{} {} [{}]",
                icon, problem.file_path, problem.line, problem.message, problem.source
            );
            let style = if is_selected {
                Style::default()
                    .fg(problem.severity.color())
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::DarkGray)
            } else {
                Style::default().fg(problem.severity.color())
            };
            let truncated: String = label.chars().take(area.width as usize).collect();
            let line = Line::from(vec![Span::styled(truncated, style)]);
            let row = Rect {
                y: area.y + i as u16,
                height: 1,
                ..area
            };
            line.render(row, buf);
        }
    }
}

impl Default for ProblemsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ProblemSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Error => write!(f, "Error"),
            Self::Warning => write!(f, "Warning"),
            Self::Info => write!(f, "Info"),
            Self::Hint => write!(f, "Hint"),
        }
    }
}

impl std::fmt::Display for SortBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File => write!(f, "File"),
            Self::Severity => write!(f, "Severity"),
            Self::Message => write!(f, "Message"),
        }
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}:{}:{} {}", self.severity.icon(), self.file_path, self.line, self.column, self.message)
    }
}

impl PartialEq for Problem {
    fn eq(&self, other: &Self) -> bool {
        self.severity == other.severity
            && self.message == other.message
            && self.file_path == other.file_path
            && self.line == other.line
            && self.column == other.column
            && self.source == other.source
            && self.code == other.code
    }
}

impl Eq for Problem {}

/// Aggregate statistics about problems.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProblemStatistics {
    pub error_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
    pub hint_count: usize,
    pub file_count: usize,
    pub source_count: usize,
}

impl ProblemStatistics {
    pub fn total(&self) -> usize {
        self.error_count + self.warning_count + self.info_count + self.hint_count
    }

    pub fn has_errors(&self) -> bool {
        self.error_count > 0
    }
}

impl std::fmt::Display for ProblemStatistics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} errors, {} warnings, {} info, {} hints across {} files",
            self.error_count, self.warning_count, self.info_count,
            self.hint_count, self.file_count
        )
    }
}

impl ProblemsPanel {
    /// Compute aggregate statistics.
    pub fn statistics(&self) -> ProblemStatistics {
        let mut sources: Vec<&str> = self.problems.iter().map(|p| p.source.as_str()).collect();
        sources.sort_unstable();
        sources.dedup();
        let mut files: Vec<&str> = self.problems.iter().map(|p| p.file_path.as_str()).collect();
        files.sort_unstable();
        files.dedup();
        ProblemStatistics {
            error_count: self.count_by_severity(ProblemSeverity::Error),
            warning_count: self.count_by_severity(ProblemSeverity::Warning),
            info_count: self.count_by_severity(ProblemSeverity::Info),
            hint_count: self.count_by_severity(ProblemSeverity::Hint),
            file_count: files.len(),
            source_count: sources.len(),
        }
    }

    /// Add a problem to the panel.
    pub fn add_problem(&mut self, problem: Problem) {
        self.problems.push(problem);
    }

    /// Remove all problems for a specific file.
    pub fn clear_file(&mut self, file_path: &str) -> usize {
        let before = self.problems.len();
        self.problems.retain(|p| p.file_path != file_path);
        before - self.problems.len()
    }

    /// Remove all problems from a specific source.
    pub fn clear_source(&mut self, source: &str) -> usize {
        let before = self.problems.len();
        self.problems.retain(|p| p.source != source);
        before - self.problems.len()
    }

    /// Clear all problems.
    pub fn clear_all(&mut self) {
        self.problems.clear();
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Sort problems based on the current sort_by setting.
    pub fn sort_problems(&mut self) {
        match self.sort_by {
            SortBy::File => {
                self.problems.sort_by(|a, b| {
                    a.file_path.cmp(&b.file_path)
                        .then(a.line.cmp(&b.line))
                        .then(a.column.cmp(&b.column))
                });
            }
            SortBy::Severity => {
                self.problems.sort_by(|a, b| {
                    a.severity.cmp(&b.severity)
                        .then(a.file_path.cmp(&b.file_path))
                });
            }
            SortBy::Message => {
                self.problems.sort_by(|a, b| a.message.cmp(&b.message));
            }
        }
    }

    /// Get all unique file paths with problems.
    pub fn affected_files(&self) -> Vec<&str> {
        let mut files: Vec<&str> = self.problems.iter().map(|p| p.file_path.as_str()).collect();
        files.sort_unstable();
        files.dedup();
        files
    }

    /// Get all unique sources.
    pub fn unique_sources(&self) -> Vec<&str> {
        let mut sources: Vec<&str> = self.problems.iter().map(|p| p.source.as_str()).collect();
        sources.sort_unstable();
        sources.dedup();
        sources
    }

    /// Get problems at a specific file location.
    pub fn problems_at_location(&self, file: &str, line: u32) -> Vec<&Problem> {
        self.problems.iter().filter(|p| p.file_path == file && p.line == line).collect()
    }

    /// Get the most severe problem overall.
    pub fn most_severe(&self) -> Option<&Problem> {
        self.problems.iter().min_by_key(|p| p.severity)
    }

    /// Check if there are any errors.
    pub fn has_errors(&self) -> bool {
        self.problems.iter().any(|p| p.severity == ProblemSeverity::Error)
    }

    /// Return the total number of problems (unfiltered).
    pub fn total_count(&self) -> usize {
        self.problems.len()
    }

    /// Set the sort order and immediately sort.
    pub fn set_sort_order(&mut self, sort_by: SortBy) {
        self.sort_by = sort_by;
        self.sort_problems();
    }

    /// Reset selection and scroll to top.
    pub fn reset_selection(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Return the file path, line, and column for the currently selected problem.
    /// Used for click-to-navigate functionality.
    pub fn navigate_to_selected(&self) -> Option<(&str, u32, u32)> {
        let filtered = self.filtered_problems();
        filtered.get(self.selected_index).map(|p| (p.file_path.as_str(), p.line, p.column))
    }

    /// Format a statusbar summary string showing error/warning counts.
    pub fn statusbar_summary(&self) -> String {
        let errors = self.count_by_severity(ProblemSeverity::Error);
        let warnings = self.count_by_severity(ProblemSeverity::Warning);
        format!("✖ {errors}  ⚠ {warnings}")
    }

    /// Replace all problems from a list of diagnostic-like tuples.
    ///
    /// Each tuple is `(severity, message, source, file_path, line, column, code)`.
    pub fn set_problems_from_diagnostics(
        &mut self,
        diagnostics: Vec<(ProblemSeverity, String, String, String, u32, u32, Option<String>)>,
    ) {
        self.problems.clear();
        for (sev, msg, src, file, line, col, code) in diagnostics {
            let mut p = Problem::new(sev, msg, src, file, line, col);
            if let Some(c) = code {
                p = p.with_code(c);
            }
            self.problems.push(p);
        }
        self.sort_problems();
        self.reset_selection();
    }

    /// Adjust scroll offset to keep the selected item visible within a viewport.
    pub fn ensure_visible(&mut self, viewport_height: usize) {
        if viewport_height == 0 {
            return;
        }
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + viewport_height {
            self.scroll_offset = self.selected_index - viewport_height + 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Quick-fix suggestions
// ---------------------------------------------------------------------------

/// A suggested fix for a diagnostic problem.
#[derive(Debug, Clone)]
pub struct ProblemQuickFix {
    /// Human-readable title describing what the fix does.
    pub title: String,
    /// Path of the file the fix applies to.
    pub file_path: String,
    /// Replacement text to insert at the fix location.
    pub replacement_text: String,
    /// Line number where the replacement starts (1-based).
    pub line: u32,
    /// Column number where the replacement starts (1-based).
    pub column: u32,
}

impl ProblemQuickFix {
    pub fn new(
        title: impl Into<String>,
        file_path: impl Into<String>,
        replacement_text: impl Into<String>,
        line: u32,
        column: u32,
    ) -> Self {
        Self {
            title: title.into(),
            file_path: file_path.into(),
            replacement_text: replacement_text.into(),
            line,
            column,
        }
    }

    /// Returns `true` if this fix only removes code (replacement is empty).
    pub fn is_deletion(&self) -> bool {
        self.replacement_text.is_empty()
    }
}

impl ProblemsPanel {
    /// Suggest quick fixes for the currently selected problem.
    ///
    /// Returns a list of applicable fixes based on the problem's code and
    /// severity. This is a heuristic lookup — real IDE integrations would
    /// query a language server.
    pub fn suggest_fixes(&self) -> Vec<ProblemQuickFix> {
        let filtered = self.filtered_problems();
        let problem = match filtered.get(self.selected_index) {
            Some(p) => p,
            None => return Vec::new(),
        };

        let mut fixes = Vec::new();

        if let Some(ref code) = problem.code {
            if code.starts_with('E') {
                fixes.push(ProblemQuickFix::new(
                    format!("Apply suggested fix for {}", code),
                    &problem.file_path,
                    "/* fix applied */",
                    problem.line,
                    problem.column,
                ));
            }
        }

        match problem.severity {
            ProblemSeverity::Warning => {
                fixes.push(ProblemQuickFix::new(
                    "Suppress this warning",
                    &problem.file_path,
                    "#[allow(warnings)]",
                    problem.line.saturating_sub(1).max(1),
                    1,
                ));
            }
            ProblemSeverity::Hint => {
                fixes.push(ProblemQuickFix::new(
                    problem.message.clone(),
                    &problem.file_path,
                    "",
                    problem.line,
                    problem.column,
                ));
            }
            _ => {}
        }

        fixes
    }
}

impl ProblemsPanel {
    /// Add a diagnostic to the panel by individual fields.
    pub fn add_diagnostic(
        &mut self,
        file: impl Into<String>,
        line: u32,
        column: u32,
        severity: ProblemSeverity,
        message: impl Into<String>,
    ) {
        self.problems.push(Problem {
            severity,
            message: message.into(),
            source: String::new(),
            file_path: file.into(),
            line,
            column,
            code: None,
        });
    }

    /// Clear all diagnostics for a specific file path.
    pub fn clear_file_diagnostics(&mut self, file: &str) -> usize {
        let before = self.problems.len();
        self.problems.retain(|p| p.file_path != file);
        before - self.problems.len()
    }

    /// Return the number of error-severity problems.
    pub fn error_count(&self) -> usize {
        self.count_by_severity(ProblemSeverity::Error)
    }

    /// Return the number of warning-severity problems.
    pub fn warning_count(&self) -> usize {
        self.count_by_severity(ProblemSeverity::Warning)
    }
}

// ---------------------------------------------------------------------------
// Free functions and ProblemsSummary
// ---------------------------------------------------------------------------

/// Group diagnostics by their severity level.
pub fn problems_group_by_severity(diagnostics: &[Problem]) -> HashMap<ProblemSeverity, Vec<&Problem>> {
    let mut map: HashMap<ProblemSeverity, Vec<&Problem>> = HashMap::new();
    for d in diagnostics {
        map.entry(d.severity).or_default().push(d);
    }
    map
}

/// Return only diagnostics at or above the given minimum severity.
/// Ordering: Error > Warning > Info > Hint.
pub fn problems_severity_filter(diagnostics: &[Problem], min_severity: ProblemSeverity) -> Vec<&Problem> {
    let min_level = min_severity.severity_level();
    diagnostics
        .iter()
        .filter(|d| d.severity.severity_level() >= min_level)
        .collect()
}

/// Summary statistics for a set of diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProblemsSummary {
    pub total: usize,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub hints: usize,
    pub files_affected: usize,
}

impl ProblemsSummary {
    /// Build a summary from a slice of diagnostics.
    pub fn from_diagnostics(diagnostics: &[Problem]) -> Self {
        let mut errors = 0;
        let mut warnings = 0;
        let mut infos = 0;
        let mut hints = 0;
        let mut files: HashSet<&str> = HashSet::new();
        for d in diagnostics {
            match d.severity {
                ProblemSeverity::Error => errors += 1,
                ProblemSeverity::Warning => warnings += 1,
                ProblemSeverity::Info => infos += 1,
                ProblemSeverity::Hint => hints += 1,
            }
            files.insert(&d.file_path);
        }
        Self {
            total: diagnostics.len(),
            errors,
            warnings,
            infos,
            hints,
            files_affected: files.len(),
        }
    }

    /// Returns `true` if there is at least one error.
    pub fn has_errors(&self) -> bool {
        self.errors > 0
    }

    /// Returns `true` if there are no diagnostics at all.
    pub fn is_clean(&self) -> bool {
        self.total == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_problems() -> Vec<Problem> {
        vec![
            Problem::new(ProblemSeverity::Error, "unused variable", "rustc", "src/main.rs", 10, 5)
                .with_code("E0001"),
            Problem::new(ProblemSeverity::Warning, "deprecated fn", "clippy", "src/lib.rs", 20, 1),
            Problem::new(ProblemSeverity::Info, "consider refactoring", "clippy", "src/lib.rs", 25, 1),
            Problem::new(ProblemSeverity::Error, "type mismatch", "rustc", "src/main.rs", 15, 8),
            Problem::new(ProblemSeverity::Hint, "add type annotation", "rustc", "src/util.rs", 5, 1),
        ]
    }

    #[test]
    fn creation() {
        let p = ProblemsPanel::new();
        assert!(p.problems.is_empty());
        assert_eq!(p.sort_by, SortBy::File);
    }

    #[test]
    fn count_by_severity() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        assert_eq!(p.count_by_severity(ProblemSeverity::Error), 2);
        assert_eq!(p.count_by_severity(ProblemSeverity::Warning), 1);
        assert_eq!(p.count_by_severity(ProblemSeverity::Info), 1);
        assert_eq!(p.count_by_severity(ProblemSeverity::Hint), 1);
    }

    #[test]
    fn filter_by_severity() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        p.filter.show_warnings = false;
        let filtered = p.filtered_problems();
        assert!(filtered.iter().all(|prob| prob.severity != ProblemSeverity::Warning));
    }

    #[test]
    fn filter_by_text() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        p.filter.filter_text = "unused".to_string();
        let filtered = p.filtered_problems();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message, "unused variable");
    }

    #[test]
    fn grouped_by_file() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        let groups = p.grouped_by_file();
        assert!(groups.len() >= 2);
    }

    #[test]
    fn select_next_and_previous() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        p.select_next();
        assert_eq!(p.selected_index, 1);
        p.select_previous();
        assert_eq!(p.selected_index, 0);
        p.select_previous();
        assert_eq!(p.selected_index, 0);
    }

    #[test]
    fn problem_with_code() {
        let p = Problem::new(ProblemSeverity::Error, "oops", "rustc", "a.rs", 1, 1)
            .with_code("E0001");
        assert_eq!(p.code, Some("E0001".to_string()));
    }

    #[test]
    fn severity_icon_and_color() {
        assert_eq!(ProblemSeverity::Error.icon(), "✖");
        assert_eq!(ProblemSeverity::Warning.color(), Color::Yellow);
    }

    #[test]
    fn render_does_not_panic() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        p.render(area, &mut buf);
    }

    #[test]
    fn render_empty_no_panic() {
        let p = ProblemsPanel::new();
        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        p.render(area, &mut buf);
    }

    #[test]
    fn default_impl() {
        let p = ProblemsPanel::default();
        assert!(p.filter.show_errors);
    }

    #[test]
    fn problem_severity_display() {
        assert_eq!(ProblemSeverity::Error.to_string(), "Error");
        assert_eq!(ProblemSeverity::Warning.to_string(), "Warning");
        assert_eq!(ProblemSeverity::Info.to_string(), "Info");
        assert_eq!(ProblemSeverity::Hint.to_string(), "Hint");
    }

    #[test]
    fn sort_by_display() {
        assert_eq!(SortBy::File.to_string(), "File");
        assert_eq!(SortBy::Severity.to_string(), "Severity");
        assert_eq!(SortBy::Message.to_string(), "Message");
    }

    #[test]
    fn problem_display() {
        let p = Problem::new(ProblemSeverity::Error, "bad", "rustc", "a.rs", 1, 2);
        let display = p.to_string();
        assert!(display.contains("✖"));
        assert!(display.contains("a.rs"));
        assert!(display.contains("bad"));
    }

    #[test]
    fn problem_equality() {
        let a = Problem::new(ProblemSeverity::Error, "msg", "rustc", "a.rs", 1, 1);
        let b = Problem::new(ProblemSeverity::Error, "msg", "rustc", "a.rs", 1, 1);
        let c = Problem::new(ProblemSeverity::Warning, "msg", "rustc", "a.rs", 1, 1);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn statistics() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        let stats = p.statistics();
        assert_eq!(stats.error_count, 2);
        assert_eq!(stats.warning_count, 1);
        assert_eq!(stats.total(), 5);
        assert!(stats.has_errors());
        assert!(stats.file_count >= 2);
    }

    #[test]
    fn statistics_display() {
        let stats = ProblemStatistics {
            error_count: 2, warning_count: 1, info_count: 0, hint_count: 0,
            file_count: 3, source_count: 2,
        };
        let display = stats.to_string();
        assert!(display.contains("2 errors"));
        assert!(display.contains("1 warnings"));
    }

    #[test]
    fn add_and_clear_problems() {
        let mut p = ProblemsPanel::new();
        p.add_problem(Problem::new(ProblemSeverity::Error, "e", "rustc", "a.rs", 1, 1));
        p.add_problem(Problem::new(ProblemSeverity::Warning, "w", "clippy", "a.rs", 2, 1));
        assert_eq!(p.total_count(), 2);
        let cleared = p.clear_file("a.rs");
        assert_eq!(cleared, 2);
        assert_eq!(p.total_count(), 0);
    }

    #[test]
    fn clear_source() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        let cleared = p.clear_source("clippy");
        assert_eq!(cleared, 2);
        assert!(p.problems.iter().all(|prob| prob.source != "clippy"));
    }

    #[test]
    fn clear_all() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        p.selected_index = 3;
        p.clear_all();
        assert_eq!(p.total_count(), 0);
        assert_eq!(p.selected_index, 0);
    }

    #[test]
    fn sort_by_severity() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        p.set_sort_order(SortBy::Severity);
        assert_eq!(p.problems[0].severity, ProblemSeverity::Error);
    }

    #[test]
    fn sort_by_message() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        p.set_sort_order(SortBy::Message);
        // alphabetical
        for i in 1..p.problems.len() {
            assert!(p.problems[i - 1].message <= p.problems[i].message);
        }
    }

    #[test]
    fn affected_files() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        let files = p.affected_files();
        assert!(files.contains(&"src/main.rs"));
        assert!(files.contains(&"src/lib.rs"));
    }

    #[test]
    fn unique_sources() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        let sources = p.unique_sources();
        assert!(sources.contains(&"rustc"));
        assert!(sources.contains(&"clippy"));
    }

    #[test]
    fn problems_at_location() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        let at_10 = p.problems_at_location("src/main.rs", 10);
        assert_eq!(at_10.len(), 1);
        assert_eq!(at_10[0].message, "unused variable");
    }

    #[test]
    fn most_severe_and_has_errors() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        assert!(p.has_errors());
        let most = p.most_severe().unwrap();
        assert_eq!(most.severity, ProblemSeverity::Error);
    }

    #[test]
    fn reset_selection() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        p.selected_index = 3;
        p.scroll_offset = 2;
        p.reset_selection();
        assert_eq!(p.selected_index, 0);
        assert_eq!(p.scroll_offset, 0);
    }

    // -- Navigation and statusbar tests --

    #[test]
    fn navigate_to_selected_returns_location() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        let loc = p.navigate_to_selected().unwrap();
        assert_eq!(loc.0, "src/main.rs");
        assert_eq!(loc.1, 10); // line
        assert_eq!(loc.2, 5);  // column
    }

    #[test]
    fn navigate_to_selected_with_filter() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        p.filter.show_errors = false;
        p.filter.show_info = false;
        // Only warnings visible, first one is "deprecated fn" at src/lib.rs:20
        let loc = p.navigate_to_selected().unwrap();
        assert_eq!(loc.0, "src/lib.rs");
        assert_eq!(loc.1, 20);
    }

    #[test]
    fn navigate_to_selected_empty() {
        let p = ProblemsPanel::new();
        assert!(p.navigate_to_selected().is_none());
    }

    #[test]
    fn statusbar_summary_format() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        let summary = p.statusbar_summary();
        assert!(summary.contains("✖ 2"));
        assert!(summary.contains("⚠ 1"));
    }

    #[test]
    fn set_problems_from_diagnostics() {
        let mut p = ProblemsPanel::new();
        p.set_problems_from_diagnostics(vec![
            (ProblemSeverity::Error, "err1".into(), "rustc".into(), "a.rs".into(), 1, 0, Some("E001".into())),
            (ProblemSeverity::Warning, "warn1".into(), "clippy".into(), "b.rs".into(), 5, 0, None),
        ]);
        assert_eq!(p.total_count(), 2);
        assert_eq!(p.count_by_severity(ProblemSeverity::Error), 1);
        assert_eq!(p.problems[0].code, Some("E001".into()));
        // Should be sorted and selection reset
        assert_eq!(p.selected_index, 0);
    }

    #[test]
    fn ensure_visible_scrolls_down() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        p.selected_index = 4;
        p.scroll_offset = 0;
        p.ensure_visible(3);
        assert_eq!(p.scroll_offset, 2); // 4 - 3 + 1
    }

    #[test]
    fn ensure_visible_scrolls_up() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        p.selected_index = 1;
        p.scroll_offset = 3;
        p.ensure_visible(3);
        assert_eq!(p.scroll_offset, 1);
    }

    #[test]
    fn ensure_visible_zero_viewport() {
        let mut p = ProblemsPanel::new();
        p.problems = sample_problems();
        p.scroll_offset = 5;
        p.ensure_visible(0);
        assert_eq!(p.scroll_offset, 5); // unchanged
    }

    // -- Quick fix tests ---------------------------------------------------

    #[test]
    fn quick_fix_new_and_is_deletion() {
        let fix = ProblemQuickFix::new("Remove unused import", "src/main.rs", "", 5, 1);
        assert!(fix.is_deletion());
        let fix2 = ProblemQuickFix::new("Replace", "src/main.rs", "new_code()", 5, 1);
        assert!(!fix2.is_deletion());
    }

    #[test]
    fn suggest_fixes_for_error_with_code() {
        let mut p = ProblemsPanel::new();
        p.problems = vec![
            Problem::new(ProblemSeverity::Error, "unused variable", "rustc", "src/main.rs", 10, 5)
                .with_code("E0001"),
        ];
        let fixes = p.suggest_fixes();
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].title.contains("E0001"));
    }

    #[test]
    fn suggest_fixes_for_warning() {
        let mut p = ProblemsPanel::new();
        p.problems = vec![
            Problem::new(ProblemSeverity::Warning, "deprecated fn", "clippy", "src/lib.rs", 20, 1),
        ];
        let fixes = p.suggest_fixes();
        assert!(fixes.iter().any(|f| f.replacement_text.contains("allow")));
    }

    #[test]
    fn suggest_fixes_for_hint_is_deletion() {
        let mut p = ProblemsPanel::new();
        p.problems = vec![
            Problem::new(ProblemSeverity::Hint, "add type annotation", "rustc", "src/util.rs", 5, 1),
        ];
        let fixes = p.suggest_fixes();
        assert!(!fixes.is_empty());
        assert!(fixes[0].is_deletion());
    }

    #[test]
    fn suggest_fixes_empty_when_no_selection() {
        let p = ProblemsPanel::new();
        let fixes = p.suggest_fixes();
        assert!(fixes.is_empty());
    }

    #[test]
    fn suggest_fixes_respects_filter() {
        let mut p = ProblemsPanel::new();
        p.problems = vec![
            Problem::new(ProblemSeverity::Error, "unused variable", "rustc", "src/main.rs", 10, 5)
                .with_code("E0001"),
        ];
        p.filter.show_errors = false;
        let fixes = p.suggest_fixes();
        assert!(fixes.is_empty());
    }

    // -- add_diagnostic tests -----------------------------------------------

    #[test]
    fn add_diagnostic_basic() {
        let mut p = ProblemsPanel::new();
        p.add_diagnostic("src/main.rs", 10, 5, ProblemSeverity::Error, "unused variable");
        assert_eq!(p.total_count(), 1);
        assert_eq!(p.problems[0].file_path, "src/main.rs");
        assert_eq!(p.problems[0].line, 10);
        assert_eq!(p.problems[0].column, 5);
        assert_eq!(p.problems[0].severity, ProblemSeverity::Error);
        assert_eq!(p.problems[0].message, "unused variable");
    }

    #[test]
    fn add_diagnostic_multiple() {
        let mut p = ProblemsPanel::new();
        p.add_diagnostic("a.rs", 1, 1, ProblemSeverity::Error, "err");
        p.add_diagnostic("b.rs", 2, 3, ProblemSeverity::Warning, "warn");
        p.add_diagnostic("c.rs", 5, 1, ProblemSeverity::Info, "info");
        assert_eq!(p.total_count(), 3);
        assert_eq!(p.error_count(), 1);
        assert_eq!(p.warning_count(), 1);
    }

    // -- clear_file_diagnostics tests ---------------------------------------

    #[test]
    fn clear_file_diagnostics_removes_matching() {
        let mut p = ProblemsPanel::new();
        p.add_diagnostic("a.rs", 1, 1, ProblemSeverity::Error, "e1");
        p.add_diagnostic("a.rs", 2, 1, ProblemSeverity::Warning, "w1");
        p.add_diagnostic("b.rs", 1, 1, ProblemSeverity::Error, "e2");
        let removed = p.clear_file_diagnostics("a.rs");
        assert_eq!(removed, 2);
        assert_eq!(p.total_count(), 1);
        assert_eq!(p.problems[0].file_path, "b.rs");
    }

    #[test]
    fn clear_file_diagnostics_no_match() {
        let mut p = ProblemsPanel::new();
        p.add_diagnostic("a.rs", 1, 1, ProblemSeverity::Error, "e1");
        let removed = p.clear_file_diagnostics("nonexistent.rs");
        assert_eq!(removed, 0);
        assert_eq!(p.total_count(), 1);
    }

    // -- error_count / warning_count tests ----------------------------------

    #[test]
    fn error_and_warning_counts() {
        let mut p = ProblemsPanel::new();
        p.add_diagnostic("a.rs", 1, 1, ProblemSeverity::Error, "e1");
        p.add_diagnostic("a.rs", 2, 1, ProblemSeverity::Error, "e2");
        p.add_diagnostic("b.rs", 1, 1, ProblemSeverity::Warning, "w1");
        p.add_diagnostic("c.rs", 1, 1, ProblemSeverity::Info, "i1");
        assert_eq!(p.error_count(), 2);
        assert_eq!(p.warning_count(), 1);
    }

    #[test]
    fn error_count_empty() {
        let p = ProblemsPanel::new();
        assert_eq!(p.error_count(), 0);
        assert_eq!(p.warning_count(), 0);
    }

    // -- problems_group_by_severity / problems_severity_filter / ProblemsSummary --

    #[test]
    fn test_group_by_severity() {
        let problems = sample_problems();
        let grouped = problems_group_by_severity(&problems);
        assert_eq!(grouped[&ProblemSeverity::Error].len(), 2);
        assert_eq!(grouped[&ProblemSeverity::Warning].len(), 1);
        assert_eq!(grouped[&ProblemSeverity::Info].len(), 1);
        assert_eq!(grouped[&ProblemSeverity::Hint].len(), 1);
    }

    #[test]
    fn test_severity_filter_errors_only() {
        let problems = sample_problems();
        let filtered = problems_severity_filter(&problems, ProblemSeverity::Error);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|p| p.severity == ProblemSeverity::Error));
    }

    #[test]
    fn test_severity_filter_warnings_and_above() {
        let problems = sample_problems();
        let filtered = problems_severity_filter(&problems, ProblemSeverity::Warning);
        assert_eq!(filtered.len(), 3); // 2 errors + 1 warning
        assert!(filtered.iter().all(|p| matches!(p.severity, ProblemSeverity::Error | ProblemSeverity::Warning)));
    }

    #[test]
    fn test_summary_from_diagnostics() {
        let problems = sample_problems();
        let summary = ProblemsSummary::from_diagnostics(&problems);
        assert_eq!(summary.total, 5);
        assert_eq!(summary.errors, 2);
        assert_eq!(summary.warnings, 1);
        assert_eq!(summary.infos, 1);
        assert_eq!(summary.hints, 1);
        assert_eq!(summary.files_affected, 3); // main.rs, lib.rs, util.rs
    }

    #[test]
    fn test_summary_has_errors() {
        let problems = sample_problems();
        let summary = ProblemsSummary::from_diagnostics(&problems);
        assert!(summary.has_errors());

        let no_errors = vec![
            Problem::new(ProblemSeverity::Warning, "warn", "clippy", "a.rs", 1, 1),
        ];
        let summary2 = ProblemsSummary::from_diagnostics(&no_errors);
        assert!(!summary2.has_errors());
    }

    #[test]
    fn test_summary_is_clean() {
        let empty: Vec<Problem> = vec![];
        let summary = ProblemsSummary::from_diagnostics(&empty);
        assert!(summary.is_clean());
        assert_eq!(summary.total, 0);

        let problems = sample_problems();
        let summary2 = ProblemsSummary::from_diagnostics(&problems);
        assert!(!summary2.is_clean());
    }
}
