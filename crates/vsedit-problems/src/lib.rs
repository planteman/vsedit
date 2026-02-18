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

}
