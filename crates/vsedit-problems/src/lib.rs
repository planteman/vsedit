//! Problems panel view.
//!
//! Displays diagnostics (errors, warnings, info, hints) grouped by file
//! with filtering and sorting — rendered via ratatui.

use std::collections::HashMap;
use std::fmt;
use std::collections::{BTreeMap, HashSet};

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

// ---------------------------------------------------------------------------
// ProblemMatcher — parse compiler output lines into Problems
// ---------------------------------------------------------------------------

/// A pattern-based matcher that parses compiler/linter output lines into
/// [`Problem`] values. Each matcher has a regex-like pattern describing the
/// expected format of a diagnostic line.
#[derive(Debug, Clone)]
pub struct ProblemMatcher {
    /// Human-readable name for this matcher (e.g. "rustc", "gcc").
    pub name: String,
    /// The source label assigned to problems created by this matcher.
    pub source: String,
    /// Separator between file path, line, column, severity, and message.
    /// For example `:` for `file.rs:10:5: error: msg`.
    pub separator: char,
    /// Index of the file-path field (0-based) after splitting by separator.
    pub file_index: usize,
    /// Index of the line-number field.
    pub line_index: usize,
    /// Index of the column-number field.
    pub column_index: usize,
    /// Index of the severity keyword field.
    pub severity_index: usize,
    /// Index of the message field. Everything from this index onward is joined.
    pub message_index: usize,
}

impl ProblemMatcher {
    /// Create a matcher for the common `file:line:col: severity: message` format
    /// used by rustc, gcc, and many other compilers.
    pub fn standard(name: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: source.into(),
            separator: ':',
            file_index: 0,
            line_index: 1,
            column_index: 2,
            severity_index: 3,
            message_index: 4,
        }
    }

    /// Try to parse a single output line into a [`Problem`].
    ///
    /// Returns `None` if the line does not match the expected format.
    pub fn parse_line(&self, line: &str) -> Option<Problem> {
        let parts: Vec<&str> = line.splitn(self.message_index + 2, self.separator).collect();
        if parts.len() <= self.message_index {
            return None;
        }

        let file_path = parts.get(self.file_index)?.trim();
        let line_num: u32 = parts.get(self.line_index)?.trim().parse().ok()?;
        let col_num: u32 = parts.get(self.column_index)?.trim().parse().ok()?;
        let severity_str = parts.get(self.severity_index)?.trim().to_lowercase();
        let message = parts[self.message_index..].join(&self.separator.to_string());
        let message = message.trim();

        let severity = match severity_str.as_str() {
            "error" => ProblemSeverity::Error,
            "warning" | "warn" => ProblemSeverity::Warning,
            "info" | "note" => ProblemSeverity::Info,
            "hint" | "help" => ProblemSeverity::Hint,
            _ => return None,
        };

        Some(Problem::new(severity, message, &self.source, file_path, line_num, col_num))
    }

    /// Parse multiple output lines, returning all successfully matched problems.
    pub fn parse_output(&self, output: &str) -> Vec<Problem> {
        output.lines().filter_map(|l| self.parse_line(l)).collect()
    }
}

// ---------------------------------------------------------------------------
// DiagnosticCodeAction — structured code actions attached to diagnostics
// ---------------------------------------------------------------------------

/// The kind of code action that can be applied to resolve a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeActionKind {
    /// A quick-fix that directly resolves the diagnostic.
    QuickFix,
    /// A refactoring suggestion related to the diagnostic.
    Refactor,
    /// An action that extracts code into a new scope / function.
    Extract,
    /// A source-level organisation action (e.g. sort imports).
    SourceOrganize,
}

impl fmt::Display for CodeActionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QuickFix => write!(f, "quickfix"),
            Self::Refactor => write!(f, "refactor"),
            Self::Extract => write!(f, "extract"),
            Self::SourceOrganize => write!(f, "source.organize"),
        }
    }
}

/// A code action that can be applied to fix or improve a diagnostic.
#[derive(Debug, Clone)]
pub struct DiagnosticCodeAction {
    pub title: String,
    pub kind: CodeActionKind,
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub new_text: String,
    /// Whether this action is the *preferred* fix for the diagnostic.
    pub is_preferred: bool,
}

impl DiagnosticCodeAction {
    pub fn new(
        title: impl Into<String>,
        kind: CodeActionKind,
        file_path: impl Into<String>,
        line: u32,
        column: u32,
        new_text: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            kind,
            file_path: file_path.into(),
            line,
            column,
            new_text: new_text.into(),
            is_preferred: false,
        }
    }

    pub fn preferred(mut self) -> Self {
        self.is_preferred = true;
        self
    }
}

// ---------------------------------------------------------------------------
// ProblemHeatmap — per-file problem density
// ---------------------------------------------------------------------------

/// Entry in a problem heatmap showing how many problems a single file has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeatmapEntry {
    pub file_path: String,
    pub error_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
    pub hint_count: usize,
}

impl HeatmapEntry {
    pub fn total(&self) -> usize {
        self.error_count + self.warning_count + self.info_count + self.hint_count
    }

    /// Weighted score: errors count more than warnings, etc.
    pub fn weighted_score(&self) -> usize {
        self.error_count * 4 + self.warning_count * 2 + self.info_count + self.hint_count
    }
}

/// Computes a heatmap of per-file problem density, sorted by weighted score
/// descending (hottest files first).
pub fn problem_heatmap(problems: &[Problem]) -> Vec<HeatmapEntry> {
    let mut map: BTreeMap<&str, (usize, usize, usize, usize)> = BTreeMap::new();
    for p in problems {
        let entry = map.entry(p.file_path.as_str()).or_default();
        match p.severity {
            ProblemSeverity::Error => entry.0 += 1,
            ProblemSeverity::Warning => entry.1 += 1,
            ProblemSeverity::Info => entry.2 += 1,
            ProblemSeverity::Hint => entry.3 += 1,
        }
    }
    let mut entries: Vec<HeatmapEntry> = map
        .into_iter()
        .map(|(file, (e, w, i, h))| HeatmapEntry {
            file_path: file.to_string(),
            error_count: e,
            warning_count: w,
            info_count: i,
            hint_count: h,
        })
        .collect();
    entries.sort_by(|a, b| b.weighted_score().cmp(&a.weighted_score()));
    entries
}

// ---------------------------------------------------------------------------
// ProblemExporter — serialize problems to various text formats
// ---------------------------------------------------------------------------

/// Output format for exporting problems.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Tab-separated values.
    Tsv,
    /// Simple human-readable text list.
    Plain,
}

/// Export a list of problems to a string in the given format.
pub fn export_problems(problems: &[Problem], format: ExportFormat) -> String {
    let mut out = String::new();
    match format {
        ExportFormat::Tsv => {
            out.push_str("severity\tfile\tline\tcol\tsource\tcode\tmessage\n");
            for p in problems {
                out.push_str(&format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    p.severity,
                    p.file_path,
                    p.line,
                    p.column,
                    p.source,
                    p.code.as_deref().unwrap_or(""),
                    p.message,
                ));
            }
        }
        ExportFormat::Plain => {
            for p in problems {
                out.push_str(&format!("{}\n", p));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// ProblemBatch — bulk operations on problem sets
// ---------------------------------------------------------------------------

/// Batch operations for adding/removing problems from multiple sources.
#[derive(Debug, Clone, Default)]
pub struct ProblemBatch {
    /// Problems to add, grouped by source.
    additions: Vec<Problem>,
    /// Sources whose existing problems should be cleared before adding.
    clear_sources: Vec<String>,
}

impl ProblemBatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a problem for addition.
    pub fn add(&mut self, problem: Problem) {
        self.additions.push(problem);
    }

    /// Mark a source to be cleared before additions are applied.
    pub fn clear_source(&mut self, source: impl Into<String>) {
        self.clear_sources.push(source.into());
    }

    /// Number of queued additions.
    pub fn addition_count(&self) -> usize {
        self.additions.len()
    }

    /// Apply the batch to a [`ProblemsPanel`]: first clear the listed sources,
    /// then add all queued problems.
    pub fn apply(self, panel: &mut ProblemsPanel) -> BatchResult {
        let mut removed = 0usize;
        for src in &self.clear_sources {
            removed += panel.clear_source(src);
        }
        let added = self.additions.len();
        for p in self.additions {
            panel.add_problem(p);
        }
        BatchResult { added, removed }
    }
}

/// Outcome of applying a [`ProblemBatch`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchResult {
    pub added: usize,
    pub removed: usize,
}

impl fmt::Display for BatchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Batch: +{} -{}", self.added, self.removed)
    }
}

// ---------------------------------------------------------------------------
// Problem deduplication
// ---------------------------------------------------------------------------

impl ProblemsPanel {
    /// Remove duplicate problems (same file, line, column, severity, message).
    /// Returns the number of duplicates removed.
    pub fn dedup(&mut self) -> usize {
        let before = self.problems.len();
        let mut seen = HashSet::new();
        self.problems.retain(|p| {
            let key = (
                p.file_path.clone(),
                p.line,
                p.column,
                p.severity,
                p.message.clone(),
            );
            seen.insert(key)
        });
        before - self.problems.len()
    }

    /// Return problems whose message matches a substring (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&Problem> {
        let lower = query.to_lowercase();
        self.problems
            .iter()
            .filter(|p| p.message.to_lowercase().contains(&lower))
            .collect()
    }

    /// Return the highest-severity level present in the panel, or None if empty.
    pub fn worst_severity(&self) -> Option<ProblemSeverity> {
        self.problems.iter().map(|p| p.severity).min()
    }

    /// Partition problems into (errors, warnings, info_and_hints).
    pub fn partition(&self) -> (Vec<&Problem>, Vec<&Problem>, Vec<&Problem>) {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut rest = Vec::new();
        for p in &self.problems {
            match p.severity {
                ProblemSeverity::Error => errors.push(p),
                ProblemSeverity::Warning => warnings.push(p),
                ProblemSeverity::Info | ProblemSeverity::Hint => rest.push(p),
            }
        }
        (errors, warnings, rest)
    }

    /// Count problems in a specific file.
    pub fn count_for_file(&self, file_path: &str) -> usize {
        self.problems.iter().filter(|p| p.file_path == file_path).count()
    }
}

// ---------------------------------------------------------------------------
// Grouping
// ---------------------------------------------------------------------------

/// Groups problems by file path, severity, or source for organized display.
pub struct ProblemGrouper;

impl ProblemGrouper {
    /// Group problems by their file path, returning a sorted map.
    pub fn group_by_file<'a>(problems: &'a [Problem]) -> BTreeMap<String, Vec<&'a Problem>> {
        let mut map: BTreeMap<String, Vec<&Problem>> = BTreeMap::new();
        for p in problems {
            map.entry(p.file_path.clone()).or_default().push(p);
        }
        map
    }

    /// Group problems by their severity level.
    pub fn group_by_severity<'a>(
        problems: &'a [Problem],
    ) -> HashMap<ProblemSeverity, Vec<&'a Problem>> {
        let mut map: HashMap<ProblemSeverity, Vec<&Problem>> = HashMap::new();
        for p in problems {
            map.entry(p.severity).or_default().push(p);
        }
        map
    }

    /// Group problems by their source tool (e.g. "rustc", "clippy").
    pub fn group_by_source<'a>(problems: &'a [Problem]) -> HashMap<String, Vec<&'a Problem>> {
        let mut map: HashMap<String, Vec<&Problem>> = HashMap::new();
        for p in problems {
            map.entry(p.source.clone()).or_default().push(p);
        }
        map
    }

    /// Group problems first by file path, then by severity within each file.
    pub fn group_by_file_and_severity<'a>(
        problems: &'a [Problem],
    ) -> BTreeMap<String, HashMap<ProblemSeverity, Vec<&'a Problem>>> {
        let mut map: BTreeMap<String, HashMap<ProblemSeverity, Vec<&Problem>>> = BTreeMap::new();
        for p in problems {
            map.entry(p.file_path.clone())
                .or_default()
                .entry(p.severity)
                .or_default()
                .push(p);
        }
        map
    }
}

// ---------------------------------------------------------------------------
// Aggregation
// ---------------------------------------------------------------------------

/// Aggregates diagnostic statistics across multiple files and problem sets.
pub struct ProblemAggregator {
    /// Problems stored per file path.
    file_problems: BTreeMap<String, Vec<Problem>>,
}

impl ProblemAggregator {
    /// Create an empty aggregator.
    pub fn new() -> Self {
        Self {
            file_problems: BTreeMap::new(),
        }
    }

    /// Add a set of problems associated with a file path.
    pub fn add_problems(&mut self, file: &str, problems: &[Problem]) {
        self.file_problems
            .entry(file.to_string())
            .or_default()
            .extend(problems.iter().cloned());
    }

    /// Total number of problems across all files.
    pub fn total_count(&self) -> usize {
        self.file_problems.values().map(|v| v.len()).sum()
    }

    /// Number of distinct files with at least one problem.
    pub fn file_count(&self) -> usize {
        self.file_problems.keys().len()
    }

    /// Count of problems broken down by severity.
    pub fn severity_counts(&self) -> HashMap<ProblemSeverity, usize> {
        let mut counts: HashMap<ProblemSeverity, usize> = HashMap::new();
        for problems in self.file_problems.values() {
            for p in problems {
                *counts.entry(p.severity).or_insert(0) += 1;
            }
        }
        counts
    }

    /// Return the file with the most problems, along with its count.
    pub fn most_problematic_file(&self) -> Option<(&str, usize)> {
        self.file_problems
            .iter()
            .max_by_key(|(_, v)| v.len())
            .map(|(k, v)| (k.as_str(), v.len()))
    }

    /// Count of problems broken down by source tool.
    pub fn source_summary(&self) -> HashMap<String, usize> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for problems in self.file_problems.values() {
            for p in problems {
                *counts.entry(p.source.clone()).or_insert(0) += 1;
            }
        }
        counts
    }
}

impl Default for ProblemAggregator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Batch removal helpers
// ---------------------------------------------------------------------------

/// Remove all problems originating from the given `source`, returning the
/// number of problems removed.
pub fn batch_remove_by_source(problems: &mut Vec<Problem>, source: &str) -> usize {
    let before = problems.len();
    problems.retain(|p| p.source != source);
    before - problems.len()
}

/// Remove all problems associated with the given `file_path`, returning the
/// number of problems removed.
pub fn batch_remove_by_file(problems: &mut Vec<Problem>, file_path: &str) -> usize {
    let before = problems.len();
    problems.retain(|p| p.file_path != file_path);
    before - problems.len()
}

// ---------------------------------------------------------------------------
// Deduplication
// ---------------------------------------------------------------------------

/// Deduplicates problems that share the same file, line, column, and message.
pub struct ProblemDeduplicator;

impl ProblemDeduplicator {
    /// Return a deduplicated list of problem references.
    ///
    /// Two problems are considered duplicates when their `file_path`, `line`,
    /// `column`, and `message` fields are all equal.  The first occurrence in
    /// the input slice is kept; subsequent duplicates are discarded.
    pub fn deduplicate<'a>(problems: &'a [Problem]) -> Vec<&'a Problem> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for p in problems {
            let key = (&p.file_path, p.line, p.column, &p.message);
            if seen.insert(key) {
                result.push(p);
            }
        }
        result
    }
}


// ---------------------------------------------------------------------------
// ProblemQuickFix
// ---------------------------------------------------------------------------

/// A suggested quick-fix action for a diagnostic problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickFixAction {
    /// Short label shown in the UI.
    pub title: String,
    /// The kind of fix (e.g. "quickfix", "refactor").
    pub kind: String,
    /// Whether this fix is the preferred one.
    pub is_preferred: bool,
}

impl QuickFixAction {
    /// Create a new quick-fix action.
    pub fn new(title: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            kind: kind.into(),
            is_preferred: false,
        }
    }

    /// Mark this action as the preferred fix.
    pub fn preferred(mut self) -> Self {
        self.is_preferred = true;
        self
    }
}

/// Manages quick-fix actions associated with problems.
#[derive(Debug, Clone)]
pub struct ProblemQuickFixList {
    /// Map from problem index → list of available fixes.
    fixes: Vec<(usize, Vec<QuickFixAction>)>,
}

impl ProblemQuickFixList {
    /// Create a new empty quick-fix list.
    pub fn new() -> Self {
        Self { fixes: Vec::new() }
    }

    /// Register quick-fix actions for a given problem index.
    pub fn add_fixes(&mut self, problem_index: usize, actions: Vec<QuickFixAction>) {
        if let Some(entry) = self.fixes.iter_mut().find(|(idx, _)| *idx == problem_index) {
            entry.1.extend(actions);
        } else {
            self.fixes.push((problem_index, actions));
        }
    }

    /// Get all fixes for a specific problem.
    pub fn fixes_for(&self, problem_index: usize) -> &[QuickFixAction] {
        self.fixes
            .iter()
            .find(|(idx, _)| *idx == problem_index)
            .map(|(_, actions)| actions.as_slice())
            .unwrap_or(&[])
    }

    /// Get the preferred fix for a problem, if any.
    pub fn preferred_fix(&self, problem_index: usize) -> Option<&QuickFixAction> {
        self.fixes_for(problem_index).iter().find(|a| a.is_preferred)
    }

    /// Total number of registered fixes across all problems.
    pub fn total_fix_count(&self) -> usize {
        self.fixes.iter().map(|(_, v)| v.len()).sum()
    }

    /// Number of problems that have at least one fix.
    pub fn problems_with_fixes(&self) -> usize {
        self.fixes.iter().filter(|(_, v)| !v.is_empty()).count()
    }
}

// ---------------------------------------------------------------------------
// ProblemWorkspaceFilter
// ---------------------------------------------------------------------------

/// Filter problems to a specific workspace folder or set of files.
#[derive(Debug, Clone)]
pub struct ProblemWorkspaceFilter {
    /// Workspace root paths to include.
    included_roots: Vec<String>,
    /// Specific file paths to exclude.
    excluded_files: HashSet<String>,
}

impl ProblemWorkspaceFilter {
    /// Create a new workspace filter with the given root paths.
    pub fn new(roots: Vec<String>) -> Self {
        Self {
            included_roots: roots,
            excluded_files: HashSet::new(),
        }
    }

    /// Add a file path to the exclusion set.
    pub fn exclude_file(&mut self, path: impl Into<String>) {
        self.excluded_files.insert(path.into());
    }

    /// Check whether a problem passes this workspace filter.
    pub fn matches(&self, problem: &Problem) -> bool {
        if self.excluded_files.contains(&problem.file_path) {
            return false;
        }
        if self.included_roots.is_empty() {
            return true;
        }
        self.included_roots.iter().any(|root| problem.file_path.starts_with(root))
    }

    /// Filter a slice of problems, returning only those that match.
    pub fn filter<'a>(&self, problems: &'a [Problem]) -> Vec<&'a Problem> {
        problems.iter().filter(|p| self.matches(p)).collect()
    }

    /// Return the number of included workspace roots.
    pub fn root_count(&self) -> usize {
        self.included_roots.len()
    }

    /// Return the number of excluded files.
    pub fn excluded_count(&self) -> usize {
        self.excluded_files.len()
    }
}

// ---------------------------------------------------------------------------
// Severity counter widget
// ---------------------------------------------------------------------------

/// Counts problems by severity level for status-bar display.
#[derive(Debug, Clone, Default)]
pub struct SeverityCounter {
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub hints: usize,
}

impl SeverityCounter {
    /// Count severities from a slice of problems.
    pub fn from_problems(problems: &[Problem]) -> Self {
        let mut counter = Self::default();
        for p in problems {
            match p.severity {
                ProblemSeverity::Error => counter.errors += 1,
                ProblemSeverity::Warning => counter.warnings += 1,
                ProblemSeverity::Info => counter.infos += 1,
                ProblemSeverity::Hint => counter.hints += 1,
            }
        }
        counter
    }

    /// Total number of problems across all severities.
    pub fn total(&self) -> usize {
        self.errors + self.warnings + self.infos + self.hints
    }

    /// Return `true` if there are any error-level problems.
    pub fn has_errors(&self) -> bool {
        self.errors > 0
    }

    /// Return a one-line summary string like "2E 3W 1I 0H".
    pub fn summary(&self) -> String {
        format!("{}E {}W {}I {}H", self.errors, self.warnings, self.infos, self.hints)
    }

    /// Return the highest severity that has at least one problem.
    pub fn worst_severity(&self) -> Option<ProblemSeverity> {
        if self.errors > 0 {
            Some(ProblemSeverity::Error)
        } else if self.warnings > 0 {
            Some(ProblemSeverity::Warning)
        } else if self.infos > 0 {
            Some(ProblemSeverity::Info)
        } else if self.hints > 0 {
            Some(ProblemSeverity::Hint)
        } else {
            None
        }
    }

    /// Merge another counter into this one.
    pub fn merge(&mut self, other: &SeverityCounter) {
        self.errors += other.errors;
        self.warnings += other.warnings;
        self.infos += other.infos;
        self.hints += other.hints;
    }
}

impl fmt::Display for SeverityCounter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

// ---------------------------------------------------------------------------
// Problem source aggregator
// ---------------------------------------------------------------------------

/// Aggregates problems by their `source` field (e.g. "rustc", "clippy").
#[derive(Debug, Clone)]
pub struct ProblemSourceAggregator {
    source_counts: BTreeMap<String, SeverityCounter>,
}

impl ProblemSourceAggregator {
    /// Build aggregation from a slice of problems.
    pub fn from_problems(problems: &[Problem]) -> Self {
        let mut source_counts: BTreeMap<String, SeverityCounter> = BTreeMap::new();
        for p in problems {
            let counter = source_counts.entry(p.source.clone()).or_default();
            match p.severity {
                ProblemSeverity::Error => counter.errors += 1,
                ProblemSeverity::Warning => counter.warnings += 1,
                ProblemSeverity::Info => counter.infos += 1,
                ProblemSeverity::Hint => counter.hints += 1,
            }
        }
        Self { source_counts }
    }

    /// Get the counter for a specific source.
    pub fn counts_for(&self, source: &str) -> Option<&SeverityCounter> {
        self.source_counts.get(source)
    }

    /// Return all sources sorted alphabetically.
    pub fn sources(&self) -> Vec<&str> {
        self.source_counts.keys().map(|s| s.as_str()).collect()
    }

    /// Return the source with the most errors.
    pub fn worst_source(&self) -> Option<&str> {
        self.source_counts
            .iter()
            .max_by_key(|(_, c)| c.errors)
            .filter(|(_, c)| c.errors > 0)
            .map(|(s, _)| s.as_str())
    }

    /// Total problems across all sources.
    pub fn total(&self) -> usize {
        self.source_counts.values().map(|c| c.total()).sum()
    }

    /// Format a summary table for display.
    pub fn summary_table(&self) -> String {
        let mut out = String::new();
        for (source, counter) in &self.source_counts {
            out.push_str(&format!("{}: {}\n", source, counter.summary()));
        }
        out
    }
}

// --- ProblemGrouperV2: group diagnostics by various criteria ---

pub struct ProblemGrouperV2;

impl ProblemGrouperV2 {
    pub fn by_file(problems: &[Problem]) -> HashMap<String, Vec<&Problem>> {
        let mut map: HashMap<String, Vec<&Problem>> = HashMap::new();
        for p in problems { map.entry(p.file_path.clone()).or_default().push(p); }
        map
    }

    pub fn by_severity(problems: &[Problem]) -> HashMap<String, Vec<&Problem>> {
        let mut map: HashMap<String, Vec<&Problem>> = HashMap::new();
        for p in problems {
            let key = format!("{:?}", p.severity);
            map.entry(key).or_default().push(p);
        }
        map
    }

    pub fn by_source(problems: &[Problem]) -> HashMap<String, Vec<&Problem>> {
        let mut map: HashMap<String, Vec<&Problem>> = HashMap::new();
        for p in problems { map.entry(p.source.clone()).or_default().push(p); }
        map
    }

    pub fn by_code(problems: &[Problem]) -> HashMap<String, Vec<&Problem>> {
        let mut map: HashMap<String, Vec<&Problem>> = HashMap::new();
        for p in problems {
            let key = p.code.clone().unwrap_or_else(|| "(none)".into());
            map.entry(key).or_default().push(p);
        }
        map
    }

    pub fn group_counts(problems: &[Problem]) -> HashMap<String, usize> {
        let by_file = Self::by_file(problems);
        by_file.into_iter().map(|(k, v)| (k, v.len())).collect()
    }

    pub fn largest_group_file(problems: &[Problem]) -> Option<String> {
        Self::group_counts(problems).into_iter().max_by_key(|(_, c)| *c).map(|(k, _)| k)
    }
}

// --- ProblemQuickFixV2: suggested fix ---

pub struct ProblemQuickFixV2 {
    pub title: String,
    pub replacement: String,
    pub is_preferred: bool,
}

impl ProblemQuickFixV2 {
    pub fn new(title: &str, replacement: &str, is_preferred: bool) -> Self {
        Self { title: title.into(), replacement: replacement.into(), is_preferred }
    }

    pub fn apply(&self, text: &str, start: usize, end: usize) -> String {
        if start > text.len() || end > text.len() || start > end { return text.to_string(); }
        let mut result = text[..start].to_string();
        result.push_str(&self.replacement);
        result.push_str(&text[end..]);
        result
    }

    pub fn matches_diagnostic(&self, message: &str) -> bool {
        message.to_lowercase().contains(&self.title.to_lowercase())
    }
}

// --- ProblemFilterV2: filter problems ---

pub struct ProblemFilterV2 {
    min_severity: Option<ProblemSeverity>,
    file_contains: Option<String>,
    message_contains: Option<String>,
    source_filter: Option<String>,
}

impl ProblemFilterV2 {
    pub fn new() -> Self {
        Self { min_severity: None, file_contains: None, message_contains: None, source_filter: None }
    }

    pub fn with_severity(mut self, sev: ProblemSeverity) -> Self { self.min_severity = Some(sev); self }
    pub fn with_file(mut self, pat: &str) -> Self { self.file_contains = Some(pat.to_string()); self }
    pub fn with_message(mut self, pat: &str) -> Self { self.message_contains = Some(pat.to_string()); self }
    pub fn with_source(mut self, src: &str) -> Self { self.source_filter = Some(src.to_string()); self }

    pub fn matches(&self, problem: &Problem) -> bool {
        if let Some(ref min) = self.min_severity {
            if problem.severity > *min { return false; }
        }
        if let Some(ref pat) = self.file_contains {
            if !problem.file_path.contains(pat.as_str()) { return false; }
        }
        if let Some(ref pat) = self.message_contains {
            if !problem.message.contains(pat.as_str()) { return false; }
        }
        if let Some(ref src) = self.source_filter {
            if problem.source != *src { return false; }
        }
        true
    }

    pub fn active_filter_count(&self) -> usize {
        let mut c = 0;
        if self.min_severity.is_some() { c += 1; }
        if self.file_contains.is_some() { c += 1; }
        if self.message_contains.is_some() { c += 1; }
        if self.source_filter.is_some() { c += 1; }
        c
    }
}


/// Diagnostics problem configuration manager.
#[derive(Debug, Clone)]
pub struct ProblemsConfig {
    entries: Vec<ProblemsEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single diagnostics problem entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ProblemsEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl ProblemsEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl ProblemsConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: ProblemsEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&ProblemsEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut ProblemsEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&ProblemsEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&ProblemsEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&ProblemsEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<ProblemsEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Diagnostics and problem markers — extended utilities (qt)
// ---------------------------------------------------------------------------

/// Metric accumulator for problems operations.
#[derive(Debug, Clone)]
pub struct QtMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QtMetrics {
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

/// Sliding-window rate counter for problems.
#[derive(Debug, Clone)]
pub struct QtRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QtRateWindow {
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

/// A small LRU-style cache for problems lookups.
#[derive(Debug, Clone)]
pub struct QtLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QtLruCache {
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
// xb_ utilities – batch 9
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer9 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer9 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_9(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_9<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_9<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_9(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_9(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 141
// ---------------------------------------------------------------------------

/// Generic object pool `Xc141Pool<T>`.
pub struct Xc141Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc141Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc141PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc141Pool<T> {
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
    pub fn stats(&self) -> Xc141PoolStats {
        Xc141PoolStats {
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

impl<T> Default for Xc141Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc141Scheduler`.
pub struct Xc141Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc141Scheduler {
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

impl Default for Xc141Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_141 hash for the given byte slice.
pub fn xc_141_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_141 convention.
pub fn xc_141_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe21 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe21Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe21PipelineError {
    pub stage: Xe21Stage,
    pub message: String,
}

impl std::fmt::Display for Xe21PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe21Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe21Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe21PipelineError>>>,
    stage_names: Vec<Xe21Stage>,
}

impl Xe21Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe21PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe21Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe21PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe21Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe21PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe21Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe21PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe21Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe21PipelineError> {
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

    pub fn compose(mut self, other: Xe21Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe21CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe21CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe21Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe21CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe21CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe21Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe21CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_21_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe21CacheEntry {
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

    fn xe_21_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe21CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_21_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe21PipelineError> {
    Ok(data)
}

pub fn xe_21_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe21PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_21_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe21PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_21_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe21PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_21_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe21PipelineError> {
    Err(Xe21PipelineError {
        stage: Xe21Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #95
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf95Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf95TrieNode {
    children: std::collections::HashMap<char, Xf95TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf95Trie {
    root: Xf95TrieNode,
    count: usize,
}

impl Xf95Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf95TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf95TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf95TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf95BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf95BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 140).
pub struct Xh140SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh140SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 182 as u64,
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

/// A compact bit set supporting boolean operations (variant 140).
pub struct Xh140BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh140BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 140).
pub struct Xi140Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi140Deque<T> {
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
pub struct Xi140Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi140Interval {
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

/// A simple interval tree (variant 140).
pub struct Xi140IntervalTree {
    xi_intervals: Vec<Xi140Interval>,
}

impl Xi140IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi140Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi140Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi140Interval) -> Vec<&Xi140Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi140Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi140Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi140Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi140Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi140Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi140Interval> = Vec::new();
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

    // -- ProblemMatcher tests -----------------------------------------------

    #[test]
    fn problem_matcher_parse_standard_line() {
        let matcher = ProblemMatcher::standard("rustc", "rustc");
        let line = "src/main.rs:10:5: error: unused variable `x`";
        let problem = matcher.parse_line(line).unwrap();
        assert_eq!(problem.file_path, "src/main.rs");
        assert_eq!(problem.line, 10);
        assert_eq!(problem.column, 5);
        assert_eq!(problem.severity, ProblemSeverity::Error);
        assert!(problem.message.contains("unused variable"));
    }

    #[test]
    fn problem_matcher_parse_warning() {
        let matcher = ProblemMatcher::standard("gcc", "gcc");
        let line = "lib.c:42:1: warning: implicit declaration";
        let problem = matcher.parse_line(line).unwrap();
        assert_eq!(problem.severity, ProblemSeverity::Warning);
        assert_eq!(problem.line, 42);
    }

    #[test]
    fn problem_matcher_rejects_invalid_line() {
        let matcher = ProblemMatcher::standard("rustc", "rustc");
        assert!(matcher.parse_line("not a diagnostic").is_none());
        assert!(matcher.parse_line("").is_none());
    }

    #[test]
    fn problem_matcher_parse_output_multi() {
        let matcher = ProblemMatcher::standard("rustc", "rustc");
        let output = "\
src/a.rs:1:1: error: msg1
some noise
src/b.rs:5:3: warning: msg2
";
        let problems = matcher.parse_output(output);
        assert_eq!(problems.len(), 2);
        assert_eq!(problems[0].severity, ProblemSeverity::Error);
        assert_eq!(problems[1].severity, ProblemSeverity::Warning);
    }

    #[test]
    fn problem_matcher_hint_and_note() {
        let matcher = ProblemMatcher::standard("rustc", "rustc");
        let hint = matcher.parse_line("x.rs:1:1: hint: try this").unwrap();
        assert_eq!(hint.severity, ProblemSeverity::Hint);
        let note = matcher.parse_line("x.rs:2:1: note: see also").unwrap();
        assert_eq!(note.severity, ProblemSeverity::Info);
    }

    // -- DiagnosticCodeAction tests -----------------------------------------

    #[test]
    fn diagnostic_code_action_creation() {
        let action = DiagnosticCodeAction::new(
            "Add missing import",
            CodeActionKind::QuickFix,
            "src/main.rs",
            1,
            1,
            "use std::io;",
        );
        assert_eq!(action.title, "Add missing import");
        assert_eq!(action.kind, CodeActionKind::QuickFix);
        assert!(!action.is_preferred);

        let preferred = action.preferred();
        assert!(preferred.is_preferred);
    }

    #[test]
    fn code_action_kind_display() {
        assert_eq!(CodeActionKind::QuickFix.to_string(), "quickfix");
        assert_eq!(CodeActionKind::Refactor.to_string(), "refactor");
        assert_eq!(CodeActionKind::Extract.to_string(), "extract");
        assert_eq!(CodeActionKind::SourceOrganize.to_string(), "source.organize");
    }

    // -- ProblemHeatmap tests -----------------------------------------------

    #[test]
    fn heatmap_sorted_by_weighted_score() {
        let problems = vec![
            Problem::new(ProblemSeverity::Error, "e1", "rustc", "hot.rs", 1, 1),
            Problem::new(ProblemSeverity::Error, "e2", "rustc", "hot.rs", 2, 1),
            Problem::new(ProblemSeverity::Warning, "w1", "clippy", "warm.rs", 1, 1),
            Problem::new(ProblemSeverity::Hint, "h1", "clippy", "cool.rs", 1, 1),
        ];
        let heatmap = problem_heatmap(&problems);
        assert_eq!(heatmap.len(), 3);
        // hot.rs has 2 errors → weighted 8, warm.rs has 1 warning → weighted 2
        assert_eq!(heatmap[0].file_path, "hot.rs");
        assert_eq!(heatmap[0].weighted_score(), 8);
        assert_eq!(heatmap[1].file_path, "warm.rs");
        assert_eq!(heatmap[2].file_path, "cool.rs");
    }

    #[test]
    fn heatmap_entry_total() {
        let entry = HeatmapEntry {
            file_path: "a.rs".to_string(),
            error_count: 1,
            warning_count: 2,
            info_count: 3,
            hint_count: 4,
        };
        assert_eq!(entry.total(), 10);
        assert_eq!(entry.weighted_score(), 1 * 4 + 2 * 2 + 3 + 4);
    }

    #[test]
    fn heatmap_empty_input() {
        let heatmap = problem_heatmap(&[]);
        assert!(heatmap.is_empty());
    }

    // -- ProblemExporter tests ----------------------------------------------

    #[test]
    fn export_tsv_format() {
        let problems = vec![
            Problem::new(ProblemSeverity::Error, "bad thing", "rustc", "a.rs", 1, 2)
                .with_code("E0001"),
        ];
        let tsv = export_problems(&problems, ExportFormat::Tsv);
        assert!(tsv.starts_with("severity\tfile\t"));
        assert!(tsv.contains("Error\ta.rs\t1\t2\trustc\tE0001\tbad thing"));
    }

    #[test]
    fn export_plain_format() {
        let problems = vec![
            Problem::new(ProblemSeverity::Warning, "unused", "clippy", "b.rs", 5, 1),
        ];
        let plain = export_problems(&problems, ExportFormat::Plain);
        assert!(plain.contains("⚠"));
        assert!(plain.contains("b.rs"));
        assert!(plain.contains("unused"));
    }

    #[test]
    fn export_empty_problems() {
        let tsv = export_problems(&[], ExportFormat::Tsv);
        // Header only
        assert_eq!(tsv.lines().count(), 1);
        let plain = export_problems(&[], ExportFormat::Plain);
        assert!(plain.is_empty());
    }

    // -- ProblemBatch tests -------------------------------------------------

    #[test]
    fn batch_add_and_clear() {
        let mut panel = ProblemsPanel::new();
        panel.add_problem(Problem::new(ProblemSeverity::Error, "old", "rustc", "a.rs", 1, 1));

        let mut batch = ProblemBatch::new();
        batch.clear_source("rustc");
        batch.add(Problem::new(ProblemSeverity::Warning, "new", "rustc", "a.rs", 2, 1));
        batch.add(Problem::new(ProblemSeverity::Info, "info", "clippy", "b.rs", 3, 1));
        assert_eq!(batch.addition_count(), 2);

        let result = batch.apply(&mut panel);
        assert_eq!(result.removed, 1);
        assert_eq!(result.added, 2);
        assert_eq!(panel.total_count(), 2);
        assert_eq!(format!("{}", result), "Batch: +2 -1");
    }

    #[test]
    fn dedup_removes_duplicates() {
        let mut panel = ProblemsPanel::new();
        let p = Problem::new(ProblemSeverity::Error, "dup", "src", "a.rs", 1, 1);
        panel.add_problem(p.clone());
        panel.add_problem(p.clone());
        panel.add_problem(p);
        assert_eq!(panel.total_count(), 3);
        let removed = panel.dedup();
        assert_eq!(removed, 2);
        assert_eq!(panel.total_count(), 1);
    }

    #[test]
    fn search_finds_matching_problems() {
        let mut panel = ProblemsPanel::new();
        panel.add_problem(Problem::new(ProblemSeverity::Error, "unused variable", "rustc", "a.rs", 1, 1));
        panel.add_problem(Problem::new(ProblemSeverity::Warning, "dead code", "rustc", "b.rs", 2, 1));
        panel.add_problem(Problem::new(ProblemSeverity::Info, "UNUSED import", "clippy", "c.rs", 3, 1));

        let results = panel.search("unused");
        assert_eq!(results.len(), 2); // case-insensitive match
    }

    #[test]
    fn worst_severity_returns_most_severe() {
        let mut panel = ProblemsPanel::new();
        panel.add_problem(Problem::new(ProblemSeverity::Hint, "hint", "src", "a.rs", 1, 1));
        panel.add_problem(Problem::new(ProblemSeverity::Warning, "warn", "src", "a.rs", 2, 1));
        assert_eq!(panel.worst_severity(), Some(ProblemSeverity::Warning));

        panel.add_problem(Problem::new(ProblemSeverity::Error, "err", "src", "a.rs", 3, 1));
        assert_eq!(panel.worst_severity(), Some(ProblemSeverity::Error));

        let empty = ProblemsPanel::new();
        assert_eq!(empty.worst_severity(), None);
    }

    #[test]
    fn partition_splits_by_severity() {
        let mut panel = ProblemsPanel::new();
        panel.add_problem(Problem::new(ProblemSeverity::Error, "e1", "s", "a.rs", 1, 1));
        panel.add_problem(Problem::new(ProblemSeverity::Warning, "w1", "s", "a.rs", 2, 1));
        panel.add_problem(Problem::new(ProblemSeverity::Info, "i1", "s", "a.rs", 3, 1));
        panel.add_problem(Problem::new(ProblemSeverity::Hint, "h1", "s", "a.rs", 4, 1));
        panel.add_problem(Problem::new(ProblemSeverity::Error, "e2", "s", "b.rs", 5, 1));

        let (errors, warnings, rest) = panel.partition();
        assert_eq!(errors.len(), 2);
        assert_eq!(warnings.len(), 1);
        assert_eq!(rest.len(), 2);
    }

    #[test]
    fn count_for_file() {
        let mut panel = ProblemsPanel::new();
        panel.add_problem(Problem::new(ProblemSeverity::Error, "e1", "s", "a.rs", 1, 1));
        panel.add_problem(Problem::new(ProblemSeverity::Error, "e2", "s", "a.rs", 2, 1));
        panel.add_problem(Problem::new(ProblemSeverity::Warning, "w1", "s", "b.rs", 1, 1));
        assert_eq!(panel.count_for_file("a.rs"), 2);
        assert_eq!(panel.count_for_file("b.rs"), 1);
        assert_eq!(panel.count_for_file("c.rs"), 0);
    }

    // -----------------------------------------------------------------------
    // ProblemGrouper tests
    // -----------------------------------------------------------------------

    #[test]
    fn grouper_group_by_file() {
        let problems = sample_problems();
        let grouped = ProblemGrouper::group_by_file(&problems);
        assert_eq!(grouped.len(), 3);
        assert_eq!(grouped["src/main.rs"].len(), 2);
        assert_eq!(grouped["src/lib.rs"].len(), 2);
        assert_eq!(grouped["src/util.rs"].len(), 1);
    }

    #[test]
    fn grouper_group_by_severity() {
        let problems = sample_problems();
        let grouped = ProblemGrouper::group_by_severity(&problems);
        assert_eq!(grouped[&ProblemSeverity::Error].len(), 2);
        assert_eq!(grouped[&ProblemSeverity::Warning].len(), 1);
        assert_eq!(grouped[&ProblemSeverity::Info].len(), 1);
        assert_eq!(grouped[&ProblemSeverity::Hint].len(), 1);
    }

    #[test]
    fn grouper_group_by_source() {
        let problems = sample_problems();
        let grouped = ProblemGrouper::group_by_source(&problems);
        assert_eq!(grouped["rustc"].len(), 3);
        assert_eq!(grouped["clippy"].len(), 2);
    }

    #[test]
    fn grouper_group_by_file_and_severity() {
        let problems = sample_problems();
        let grouped = ProblemGrouper::group_by_file_and_severity(&problems);
        assert_eq!(grouped.len(), 3);
        let lib_groups = &grouped["src/lib.rs"];
        assert_eq!(lib_groups[&ProblemSeverity::Warning].len(), 1);
        assert_eq!(lib_groups[&ProblemSeverity::Info].len(), 1);
        let main_groups = &grouped["src/main.rs"];
        assert_eq!(main_groups[&ProblemSeverity::Error].len(), 2);
    }

    // -----------------------------------------------------------------------
    // ProblemAggregator tests
    // -----------------------------------------------------------------------

    #[test]
    fn aggregator_basic_counts() {
        let mut agg = ProblemAggregator::new();
        let p1 = vec![
            Problem::new(ProblemSeverity::Error, "e1", "rustc", "a.rs", 1, 1),
            Problem::new(ProblemSeverity::Warning, "w1", "clippy", "a.rs", 2, 1),
        ];
        let p2 = vec![
            Problem::new(ProblemSeverity::Error, "e2", "rustc", "b.rs", 1, 1),
        ];
        agg.add_problems("a.rs", &p1);
        agg.add_problems("b.rs", &p2);
        assert_eq!(agg.total_count(), 3);
        assert_eq!(agg.file_count(), 2);
    }

    #[test]
    fn aggregator_severity_counts() {
        let mut agg = ProblemAggregator::new();
        agg.add_problems("x.rs", &[
            Problem::new(ProblemSeverity::Error, "e", "s", "x.rs", 1, 1),
            Problem::new(ProblemSeverity::Error, "e2", "s", "x.rs", 2, 1),
            Problem::new(ProblemSeverity::Warning, "w", "s", "x.rs", 3, 1),
        ]);
        let counts = agg.severity_counts();
        assert_eq!(counts[&ProblemSeverity::Error], 2);
        assert_eq!(counts[&ProblemSeverity::Warning], 1);
    }

    #[test]
    fn aggregator_most_problematic_file() {
        let mut agg = ProblemAggregator::new();
        agg.add_problems("small.rs", &[
            Problem::new(ProblemSeverity::Info, "i", "s", "small.rs", 1, 1),
        ]);
        agg.add_problems("big.rs", &[
            Problem::new(ProblemSeverity::Error, "e1", "s", "big.rs", 1, 1),
            Problem::new(ProblemSeverity::Error, "e2", "s", "big.rs", 2, 1),
            Problem::new(ProblemSeverity::Warning, "w", "s", "big.rs", 3, 1),
        ]);
        let (file, count) = agg.most_problematic_file().unwrap();
        assert_eq!(file, "big.rs");
        assert_eq!(count, 3);
    }

    #[test]
    fn aggregator_source_summary() {
        let mut agg = ProblemAggregator::new();
        agg.add_problems("f.rs", &[
            Problem::new(ProblemSeverity::Error, "e", "rustc", "f.rs", 1, 1),
            Problem::new(ProblemSeverity::Warning, "w", "clippy", "f.rs", 2, 1),
            Problem::new(ProblemSeverity::Info, "i", "clippy", "f.rs", 3, 1),
        ]);
        let summary = agg.source_summary();
        assert_eq!(summary["rustc"], 1);
        assert_eq!(summary["clippy"], 2);
    }

    // -----------------------------------------------------------------------
    // Batch removal tests
    // -----------------------------------------------------------------------

    #[test]
    fn batch_remove_by_source_removes_matching() {
        let mut problems = vec![
            Problem::new(ProblemSeverity::Error, "e", "rustc", "a.rs", 1, 1),
            Problem::new(ProblemSeverity::Warning, "w", "clippy", "a.rs", 2, 1),
            Problem::new(ProblemSeverity::Info, "i", "rustc", "b.rs", 3, 1),
        ];
        let removed = batch_remove_by_source(&mut problems, "rustc");
        assert_eq!(removed, 2);
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].source, "clippy");
    }

    #[test]
    fn batch_remove_by_file_removes_matching() {
        let mut problems = vec![
            Problem::new(ProblemSeverity::Error, "e1", "s", "a.rs", 1, 1),
            Problem::new(ProblemSeverity::Error, "e2", "s", "a.rs", 5, 1),
            Problem::new(ProblemSeverity::Warning, "w", "s", "b.rs", 1, 1),
        ];
        let removed = batch_remove_by_file(&mut problems, "a.rs");
        assert_eq!(removed, 2);
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].file_path, "b.rs");
    }

    // -----------------------------------------------------------------------
    // ProblemDeduplicator tests
    // -----------------------------------------------------------------------

    #[test]
    fn deduplicator_removes_exact_duplicates() {
        let problems = vec![
            Problem::new(ProblemSeverity::Error, "unused var", "rustc", "a.rs", 10, 5),
            Problem::new(ProblemSeverity::Error, "unused var", "rustc", "a.rs", 10, 5),
            Problem::new(ProblemSeverity::Error, "unused var", "clippy", "a.rs", 10, 5),
        ];
        let deduped = ProblemDeduplicator::deduplicate(&problems);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].message, "unused var");
    }

    #[test]
    fn deduplicator_keeps_different_lines() {
        let problems = vec![
            Problem::new(ProblemSeverity::Error, "same msg", "s", "a.rs", 1, 1),
            Problem::new(ProblemSeverity::Error, "same msg", "s", "a.rs", 2, 1),
        ];
        let deduped = ProblemDeduplicator::deduplicate(&problems);
        assert_eq!(deduped.len(), 2);
    }

    // -----------------------------------------------------------------------
    // ProblemQuickFixList tests
    // -----------------------------------------------------------------------

    #[test]
    fn quickfix_add_and_query() {
        let mut list = ProblemQuickFixList::new();
        list.add_fixes(0, vec![
            QuickFixAction::new("Remove unused", "quickfix"),
            QuickFixAction::new("Rename", "refactor"),
        ]);
        assert_eq!(list.fixes_for(0).len(), 2);
        assert_eq!(list.fixes_for(1).len(), 0);
    }

    #[test]
    fn quickfix_preferred() {
        let mut list = ProblemQuickFixList::new();
        list.add_fixes(0, vec![
            QuickFixAction::new("Option A", "quickfix"),
            QuickFixAction::new("Option B", "quickfix").preferred(),
        ]);
        let pref = list.preferred_fix(0).unwrap();
        assert_eq!(pref.title, "Option B");
    }

    #[test]
    fn quickfix_total_count() {
        let mut list = ProblemQuickFixList::new();
        list.add_fixes(0, vec![QuickFixAction::new("a", "q")]);
        list.add_fixes(1, vec![QuickFixAction::new("b", "q"), QuickFixAction::new("c", "q")]);
        assert_eq!(list.total_fix_count(), 3);
        assert_eq!(list.problems_with_fixes(), 2);
    }

    // -----------------------------------------------------------------------
    // ProblemWorkspaceFilter tests
    // -----------------------------------------------------------------------

    #[test]
    fn workspace_filter_matches_root() {
        let filter = ProblemWorkspaceFilter::new(vec!["/project/src".into()]);
        let p = Problem::new(ProblemSeverity::Error, "err", "s", "/project/src/main.rs", 1, 1);
        assert!(filter.matches(&p));
    }

    #[test]
    fn workspace_filter_excludes() {
        let mut filter = ProblemWorkspaceFilter::new(vec!["/project".into()]);
        filter.exclude_file("/project/build.rs");
        let p = Problem::new(ProblemSeverity::Warning, "w", "s", "/project/build.rs", 1, 1);
        assert!(!filter.matches(&p));
    }

    #[test]
    fn workspace_filter_empty_roots_matches_all() {
        let filter = ProblemWorkspaceFilter::new(vec![]);
        let p = Problem::new(ProblemSeverity::Info, "i", "s", "any/path.rs", 1, 1);
        assert!(filter.matches(&p));
    }

    #[test]
    fn workspace_filter_batch() {
        let filter = ProblemWorkspaceFilter::new(vec!["/src".into()]);
        let problems = vec![
            Problem::new(ProblemSeverity::Error, "a", "s", "/src/a.rs", 1, 1),
            Problem::new(ProblemSeverity::Error, "b", "s", "/lib/b.rs", 1, 1),
        ];
        let matched = filter.filter(&problems);
        assert_eq!(matched.len(), 1);
    }

    // -----------------------------------------------------------------------
    // SeverityCounter tests
    // -----------------------------------------------------------------------

    #[test]
    fn severity_counter_from_problems() {
        let problems = sample_problems();
        let counter = SeverityCounter::from_problems(&problems);
        assert_eq!(counter.errors, 2);
        assert_eq!(counter.warnings, 1);
        assert_eq!(counter.infos, 1);
        assert_eq!(counter.hints, 1);
        assert_eq!(counter.total(), 5);
    }

    #[test]
    fn severity_counter_summary() {
        let counter = SeverityCounter { errors: 1, warnings: 2, infos: 0, hints: 3 };
        assert_eq!(counter.summary(), "1E 2W 0I 3H");
    }

    #[test]
    fn severity_counter_worst() {
        let c1 = SeverityCounter { errors: 0, warnings: 1, infos: 0, hints: 0 };
        assert_eq!(c1.worst_severity(), Some(ProblemSeverity::Warning));
        let c2 = SeverityCounter::default();
        assert_eq!(c2.worst_severity(), None);
    }

    #[test]
    fn severity_counter_merge() {
        let mut c1 = SeverityCounter { errors: 1, warnings: 0, infos: 2, hints: 0 };
        let c2 = SeverityCounter { errors: 0, warnings: 3, infos: 0, hints: 1 };
        c1.merge(&c2);
        assert_eq!(c1.errors, 1);
        assert_eq!(c1.warnings, 3);
        assert_eq!(c1.infos, 2);
        assert_eq!(c1.hints, 1);
    }

    // -----------------------------------------------------------------------
    // ProblemSourceAggregator tests
    // -----------------------------------------------------------------------

    #[test]
    fn source_aggregator_basic() {
        let problems = sample_problems();
        let agg = ProblemSourceAggregator::from_problems(&problems);
        let sources = agg.sources();
        assert!(sources.contains(&"rustc"));
        assert!(sources.contains(&"clippy"));
    }

    #[test]
    fn source_aggregator_counts() {
        let problems = sample_problems();
        let agg = ProblemSourceAggregator::from_problems(&problems);
        let rustc = agg.counts_for("rustc").unwrap();
        assert_eq!(rustc.errors, 2);
        assert_eq!(rustc.hints, 1);
    }

    #[test]
    fn source_aggregator_worst() {
        let problems = sample_problems();
        let agg = ProblemSourceAggregator::from_problems(&problems);
        assert_eq!(agg.worst_source(), Some("rustc"));
    }

    #[test]
    fn source_aggregator_total() {
        let problems = sample_problems();
        let agg = ProblemSourceAggregator::from_problems(&problems);
        assert_eq!(agg.total(), 5);
    }

    #[test]
    fn source_aggregator_table() {
        let problems = sample_problems();
        let agg = ProblemSourceAggregator::from_problems(&problems);
        let table = agg.summary_table();
        assert!(table.contains("rustc:"));
        assert!(table.contains("clippy:"));
    }

    #[test]
    fn problem_grouper_v2_by_file() {
        let probs = sample_problems();
        let grouped = ProblemGrouperV2::by_file(&probs);
        assert!(grouped.contains_key("src/main.rs"));
        assert!(grouped.contains_key("src/lib.rs"));
    }

    #[test]
    fn problem_grouper_v2_by_severity() {
        let probs = sample_problems();
        let grouped = ProblemGrouperV2::by_severity(&probs);
        assert!(grouped.contains_key("Error"));
    }

    #[test]
    fn problem_grouper_v2_by_source() {
        let probs = sample_problems();
        let grouped = ProblemGrouperV2::by_source(&probs);
        assert!(grouped.contains_key("rustc"));
        assert!(grouped.contains_key("clippy"));
    }

    #[test]
    fn problem_grouper_v2_largest_group() {
        let probs = sample_problems();
        let largest = ProblemGrouperV2::largest_group_file(&probs);
        assert!(largest.is_some());
    }

    #[test]
    fn problem_quick_fix_v2_apply() {
        let fix = ProblemQuickFixV2::new("rename", "bar", true);
        let result = fix.apply("let foo = 1;", 4, 7);
        assert_eq!(result, "let bar = 1;");
    }

    #[test]
    fn problem_quick_fix_v2_matches() {
        let fix = ProblemQuickFixV2::new("unused", "_ ", false);
        assert!(fix.matches_diagnostic("warning: unused variable"));
        assert!(!fix.matches_diagnostic("error: type mismatch"));
    }

    #[test]
    fn problem_quick_fix_v2_is_preferred() {
        let fix = ProblemQuickFixV2::new("fix", "x", true);
        assert!(fix.is_preferred);
    }

    #[test]
    fn problem_filter_v2_no_filters() {
        let f = ProblemFilterV2::new();
        let p = Problem::new(ProblemSeverity::Error, "msg", "rustc", "file.rs", 1, 1);
        assert!(f.matches(&p));
        assert_eq!(f.active_filter_count(), 0);
    }

    #[test]
    fn problem_filter_v2_by_source() {
        let f = ProblemFilterV2::new().with_source("clippy");
        let p1 = Problem::new(ProblemSeverity::Warning, "msg", "clippy", "f.rs", 1, 1);
        let p2 = Problem::new(ProblemSeverity::Warning, "msg", "rustc", "f.rs", 1, 1);
        assert!(f.matches(&p1));
        assert!(!f.matches(&p2));
    }

    #[test]
    fn problem_filter_v2_by_message() {
        let f = ProblemFilterV2::new().with_message("unused");
        let p = Problem::new(ProblemSeverity::Warning, "unused variable", "rustc", "f.rs", 1, 1);
        assert!(f.matches(&p));
    }

    #[test]
    fn problem_filter_v2_active_count() {
        let f = ProblemFilterV2::new()
            .with_source("rustc")
            .with_message("err");
        assert_eq!(f.active_filter_count(), 2);
    }

    #[test]
    fn problem_grouper_v2_by_code() {
        let probs = sample_problems();
        let grouped = ProblemGrouperV2::by_code(&probs);
        assert!(grouped.contains_key("E0001") || grouped.contains_key("(none)"));
    }



    #[test]
    fn problems_entry_creation() {
        let e = ProblemsEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn problems_entry_with_priority() {
        let e = ProblemsEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn problems_entry_metadata() {
        let e = ProblemsEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn problems_entry_remove_meta() {
        let mut e = ProblemsEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn problems_entry_activate_deactivate() {
        let mut e = ProblemsEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn problems_config_add_sorted() {
        let mut c = ProblemsConfig::new(10);
        c.add(ProblemsEntry::new("lo", "Lo").with_priority(1));
        c.add(ProblemsEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn problems_config_capacity() {
        let mut c = ProblemsConfig::new(1);
        assert!(c.add(ProblemsEntry::new("a", "A")));
        assert!(!c.add(ProblemsEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn problems_config_remove() {
        let mut c = ProblemsConfig::new(10);
        c.add(ProblemsEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn problems_config_get() {
        let mut c = ProblemsConfig::new(10);
        c.add(ProblemsEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn problems_config_active_entries() {
        let mut c = ProblemsConfig::new(10);
        c.add(ProblemsEntry::new("a", "A"));
        c.add(ProblemsEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn problems_config_enable_disable() {
        let mut c = ProblemsConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn problems_config_clear() {
        let mut c = ProblemsConfig::new(10);
        c.add(ProblemsEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn problems_config_find_by_label() {
        let mut c = ProblemsConfig::new(10);
        c.add(ProblemsEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn problems_config_top_n() {
        let mut c = ProblemsConfig::new(10);
        c.add(ProblemsEntry::new("a", "A").with_priority(1));
        c.add(ProblemsEntry::new("b", "B").with_priority(2));
        c.add(ProblemsEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn problems_config_deactivate_activate_all() {
        let mut c = ProblemsConfig::new(10);
        c.add(ProblemsEntry::new("a", "A"));
        c.add(ProblemsEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn problems_config_highest_priority() {
        let mut c = ProblemsConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(ProblemsEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn problems_config_contains() {
        let mut c = ProblemsConfig::new(10);
        c.add(ProblemsEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn problems_config_labels() {
        let mut c = ProblemsConfig::new(10);
        c.add(ProblemsEntry::new("a", "Alpha"));
        c.add(ProblemsEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn problems_config_drain_inactive() {
        let mut c = ProblemsConfig::new(10);
        c.add(ProblemsEntry::new("a", "A"));
        c.add(ProblemsEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn qt_metrics_empty() {
        let m = QtMetrics::new("problems");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qt_metrics_record_and_mean() {
        let mut m = QtMetrics::new("problems");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qt_metrics_min_max() {
        let mut m = QtMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qt_metrics_variance_and_std() {
        let mut m = QtMetrics::new("v");
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
    fn qt_metrics_percentile() {
        let mut m = QtMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qt_metrics_merge() {
        let mut a = QtMetrics::new("a");
        a.record(1.0);
        let mut b = QtMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qt_metrics_reset() {
        let mut m = QtMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qt_rate_window_empty() {
        let rw = QtRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qt_rate_window_tick_and_rate() {
        let mut rw = QtRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qt_lru_cache_basic() {
        let mut c = QtLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qt_lru_cache_contains_and_keys() {
        let mut c = QtLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qt_lru_cache_remove() {
        let mut c = QtLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qt_metrics_sum() {
        let mut m = QtMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qt_metrics_label() {
        let m = QtMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qt_lru_cache_clear() {
        let mut c = QtLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_9_push_and_len() {
        let mut rb = super::XbRingBuffer9::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_9_overwrite() {
        let mut rb = super::XbRingBuffer9::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_9_get_out_of_bounds() {
        let rb = super::XbRingBuffer9::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_9_drain_all() {
        let mut rb = super::XbRingBuffer9::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_9_peek_front_back() {
        let mut rb = super::XbRingBuffer9::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_9_clear() {
        let mut rb = super::XbRingBuffer9::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_9_capacity() {
        let rb = super::XbRingBuffer9::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_9_basic() {
        let h = super::xb_fnv1a_9(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_9(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_9_different_inputs() {
        let h1 = super::xb_fnv1a_9(b"abc");
        let h2 = super::xb_fnv1a_9(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_9_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_9(&data);
        let dec = super::xb_rle_decode_9(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_9_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_9(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_9(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_9_values() {
        assert!((super::xb_clamp_9(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_9(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_9(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_9_values() {
        assert!((super::xb_lerp_9(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_9(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_9(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_9_wrap_around_twice() {
        let mut rb = super::XbRingBuffer9::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 141 ----

    #[test]
    fn xc_141_pool_new_empty() {
        let pool: super::Xc141Pool<i32> = super::Xc141Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_141_pool_release_acquire() {
        let mut pool = super::Xc141Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_141_pool_acquire_empty() {
        let mut pool: super::Xc141Pool<i32> = super::Xc141Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_141_pool_full() {
        let mut pool = super::Xc141Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_141_pool_drain() {
        let mut pool = super::Xc141Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_141_pool_stats() {
        let mut pool = super::Xc141Pool::new(8);
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
    fn xc_141_pool_clear() {
        let mut pool = super::Xc141Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_141_pool_shrink() {
        let mut pool = super::Xc141Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_141_pool_default() {
        let pool: super::Xc141Pool<String> = super::Xc141Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_141_pool_extend() {
        let mut pool = super::Xc141Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_141_pool_retain() {
        let mut pool = super::Xc141Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_141_scheduler_round_robin() {
        let mut sched = super::Xc141Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_141_scheduler_empty() {
        let mut sched = super::Xc141Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_141_scheduler_reset() {
        let mut sched = super::Xc141Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_141_scheduler_add_remove() {
        let mut sched = super::Xc141Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_141_scheduler_targets() {
        let sched = super::Xc141Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_141_hash_empty() {
        assert_eq!(super::xc_141_hash(b""), 5381);
    }

    #[test]
    fn xc_141_hash_data() {
        let h = super::xc_141_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_141_hash(b"hello"), h);
    }

    #[test]
    fn xc_141_reverse_str() {
        assert_eq!(super::xc_141_reverse("abc"), "cba");
        assert_eq!(super::xc_141_reverse(""), "");
    }


    #[test]
    fn xe_21_pipeline_empty() {
        let p = super::Xe21Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_21_pipeline_parse_stage() {
        let p = super::Xe21Pipeline::new()
            .add_parse(super::xe_21_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_21_pipeline_transform_double() {
        let p = super::Xe21Pipeline::new()
            .add_transform(super::xe_21_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_21_pipeline_validate_reverse() {
        let p = super::Xe21Pipeline::new()
            .add_validate(super::xe_21_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_21_pipeline_emit_filter() {
        let p = super::Xe21Pipeline::new()
            .add_emit(super::xe_21_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_21_pipeline_multi_stage() {
        let p = super::Xe21Pipeline::new()
            .add_parse(super::xe_21_pipeline_identity)
            .add_transform(super::xe_21_pipeline_double)
            .add_validate(super::xe_21_pipeline_reverse)
            .add_emit(super::xe_21_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_21_pipeline_error_propagation() {
        let p = super::Xe21Pipeline::new()
            .add_parse(super::xe_21_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe21Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_21_pipeline_compose() {
        let p1 = super::Xe21Pipeline::new()
            .add_parse(super::xe_21_pipeline_identity);
        let p2 = super::Xe21Pipeline::new()
            .add_transform(super::xe_21_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_21_pipeline_error_display() {
        let e = super::Xe21PipelineError {
            stage: super::Xe21Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_21_cache_put_get() {
        let mut c = super::Xe21Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_21_cache_miss() {
        let mut c: super::Xe21Cache<&str, i32> = super::Xe21Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_21_cache_ttl_expiry() {
        let mut c = super::Xe21Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_21_cache_evict() {
        let mut c = super::Xe21Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_21_cache_capacity() {
        let mut c = super::Xe21Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_21_cache_stats() {
        let mut c = super::Xe21Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_21_cache_clear() {
        let mut c = super::Xe21Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #95 --

    #[test]
    fn xf95_trie_insert_search() {
        let mut t = Xf95Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf95_trie_starts_with() {
        let mut t = Xf95Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf95_trie_remove() {
        let mut t = Xf95Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf95_trie_word_count() {
        let mut t = Xf95Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf95_trie_longest_prefix() {
        let mut t = Xf95Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf95_trie_all_words() {
        let mut t = Xf95Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf95_trie_autocomplete() {
        let mut t = Xf95Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf95_trie_empty_search() {
        let t = Xf95Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf95_bloom_add_contains() {
        let mut bf = Xf95BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf95_bloom_probably_absent() {
        let bf = Xf95BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf95_bloom_false_positive_rate() {
        let mut bf = Xf95BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf95_bloom_clear() {
        let mut bf = Xf95BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf95_bloom_union() {
        let mut a = Xf95BloomFilter::xf_new(512, 2);
        let mut b = Xf95BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf95_bloom_intersection_estimate() {
        let mut a = Xf95BloomFilter::xf_new(512, 2);
        let mut b = Xf95BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf95_bloom_union_size_mismatch() {
        let a = Xf95BloomFilter::xf_new(256, 2);
        let b = Xf95BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh140_skip_insert_contains() {
        let mut sl = super::Xh140SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh140_skip_remove() {
        let mut sl = super::Xh140SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh140_skip_len() {
        let mut sl = super::Xh140SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh140_skip_range_query() {
        let mut sl = super::Xh140SkipList::xh_new(4);
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
    fn xh140_skip_floor_ceiling() {
        let mut sl = super::Xh140SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh140_skip_rank() {
        let mut sl = super::Xh140SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh140_skip_empty() {
        let sl = super::Xh140SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh140_skip_duplicates() {
        let mut sl = super::Xh140SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh140_bitset_set_test() {
        let mut bs = super::Xh140BitSet::xh_new(256);
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
    fn xh140_bitset_clear_count() {
        let mut bs = super::Xh140BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh140_bitset_and_or_xor() {
        let mut a = super::Xh140BitSet::xh_new(128);
        let mut b = super::Xh140BitSet::xh_new(128);
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
    fn xh140_bitset_iter_ones() {
        let mut bs = super::Xh140BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh140_bitset_first_last() {
        let mut bs = super::Xh140BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh140_bitset_empty() {
        let bs = super::Xh140BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi140_deque_push_pop_back() {
        let mut dq = super::Xi140Deque::xi_new(4);
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
    fn xi140_deque_push_pop_front() {
        let mut dq = super::Xi140Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi140_deque_mixed_ops() {
        let mut dq = super::Xi140Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi140_deque_get_and_split() {
        let mut dq = super::Xi140Deque::xi_new(8);
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
    fn xi140_deque_rotate_left() {
        let mut dq = super::Xi140Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi140_deque_rotate_right() {
        let mut dq = super::Xi140Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi140_deque_grow() {
        let mut dq = super::Xi140Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi140_deque_empty() {
        let dq = super::Xi140Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi140_interval_tree_insert_query() {
        let mut tree = super::Xi140IntervalTree::xi_new();
        tree.xi_insert(super::Xi140Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi140Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi140Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi140_interval_tree_overlap() {
        let mut tree = super::Xi140IntervalTree::xi_new();
        tree.xi_insert(super::Xi140Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi140Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi140Interval::xi_new(12, 20));
        let q = super::Xi140Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi140_interval_tree_remove() {
        let mut tree = super::Xi140IntervalTree::xi_new();
        tree.xi_insert(super::Xi140Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi140Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi140_interval_tree_gaps() {
        let mut tree = super::Xi140IntervalTree::xi_new();
        tree.xi_insert(super::Xi140Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi140Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi140Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi140Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi140Interval::xi_new(8, 10));
    }

    #[test]
    fn xi140_interval_tree_merge() {
        let mut tree = super::Xi140IntervalTree::xi_new();
        tree.xi_insert(super::Xi140Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi140Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi140Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi140Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi140Interval::xi_new(10, 15));
    }

    #[test]
    fn xi140_interval_tree_all() {
        let mut tree = super::Xi140IntervalTree::xi_new();
        tree.xi_insert(super::Xi140Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi140Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi140_interval_tree_empty() {
        let tree = super::Xi140IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi140_interval_tree_contains_point() {
        let iv = super::Xi140Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}
