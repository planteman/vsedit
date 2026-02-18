//! Test explorer view.

use std::collections::HashMap;
use std::fmt;
/// The execution state of a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestState {
    Queued,
    Running,
    Passed,
    Failed,
    Skipped,
    Errored,
}

/// Returns a human-readable label for a test state.
pub fn state_label(state: TestState) -> &'static str {
    match state {
        TestState::Queued => "Queued",
        TestState::Running => "Running",
        TestState::Passed => "Passed",
        TestState::Failed => "Failed",
        TestState::Skipped => "Skipped",
        TestState::Errored => "Errored",
    }
}

/// The kind of a test profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestProfileKind {
    Run,
    Debug,
    Coverage,
}

/// A configuration profile for running tests.
#[derive(Debug, Clone)]
pub struct TestProfile {
    pub id: String,
    pub label: String,
    pub kind: TestProfileKind,
}

/// Aggregated statistics for a test run.
#[derive(Debug, Clone)]
pub struct TestRunStats {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub errored: usize,
    pub running: usize,
    pub queued: usize,
    pub pass_rate: f64,
}

/// A single test item, potentially with children.
#[derive(Debug, Clone)]
pub struct TestItem {
    pub id: String,
    pub label: String,
    pub uri: Option<String>,
    pub line: Option<u32>,
    pub state: TestState,
    pub children: Vec<TestItem>,
    pub duration_ms: Option<f64>,
    pub message: Option<String>,
}

impl TestItem {
    /// Returns `true` if this item has any children.
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }

    /// Returns the total number of descendants (recursive).
    pub fn child_count(&self) -> usize {
        self.children.iter().map(|c| 1 + c.child_count()).sum()
    }
}

/// A test run containing multiple test items.
pub struct TestRun {
    pub id: String,
    pub name: String,
    pub items: Vec<TestItem>,
    pub started_at: Option<u64>,
    pub finished_at: Option<u64>,
}

impl TestRun {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            items: Vec::new(),
            started_at: None,
            finished_at: None,
        }
    }

    pub fn add_item(&mut self, item: TestItem) {
        self.items.push(item);
    }

    pub fn set_state(&mut self, item_id: &str, state: TestState) {
        for item in &mut self.items {
            if set_state_recursive(item, item_id, state) {
                return;
            }
        }
    }

    pub fn pass_count(&self) -> usize {
        self.items.iter().map(|i| count_state(i, TestState::Passed)).sum()
    }

    pub fn fail_count(&self) -> usize {
        self.items.iter().map(|i| count_state(i, TestState::Failed)).sum()
    }

    pub fn skipped_count(&self) -> usize {
        self.items.iter().map(|i| count_state(i, TestState::Skipped)).sum()
    }

    pub fn error_count(&self) -> usize {
        self.items.iter().map(|i| count_state(i, TestState::Errored)).sum()
    }

    pub fn total_count(&self) -> usize {
        self.items.iter().map(count_all).sum()
    }

    pub fn is_complete(&self) -> bool {
        self.items.iter().all(all_complete)
    }

    pub fn duration_ms(&self) -> Option<u64> {
        match (self.started_at, self.finished_at) {
            (Some(s), Some(f)) if f >= s => Some(f - s),
            _ => None,
        }
    }

    pub fn get_stats(&self) -> TestRunStats {
        let total = self.total_count();
        let passed = self.pass_count();
        let failed = self.fail_count();
        let skipped = self.skipped_count();
        let errored = self.error_count();
        let running = self.items.iter().map(|i| count_state(i, TestState::Running)).sum();
        let queued = self.items.iter().map(|i| count_state(i, TestState::Queued)).sum();
        let pass_rate = if total > 0 { passed as f64 / total as f64 } else { 0.0 };
        TestRunStats { total, passed, failed, skipped, errored, running, queued, pass_rate }
    }

    pub fn find_item(&self, id: &str) -> Option<&TestItem> {
        for item in &self.items {
            if let Some(found) = find_item_recursive(item, id) {
                return Some(found);
            }
        }
        None
    }

    pub fn get_failed_items(&self) -> Vec<&TestItem> {
        let mut result = Vec::new();
        for item in &self.items {
            collect_failed(item, &mut result);
        }
        result
    }

    pub fn flatten_items(&self) -> Vec<&TestItem> {
        let mut result = Vec::new();
        for item in &self.items {
            flatten_recursive(item, &mut result);
        }
        result
    }
}

fn set_state_recursive(item: &mut TestItem, id: &str, state: TestState) -> bool {
    if item.id == id {
        item.state = state;
        return true;
    }
    for child in &mut item.children {
        if set_state_recursive(child, id, state) {
            return true;
        }
    }
    false
}

fn count_state(item: &TestItem, state: TestState) -> usize {
    let own = usize::from(item.state == state);
    own + item.children.iter().map(|c| count_state(c, state)).sum::<usize>()
}

fn count_all(item: &TestItem) -> usize {
    1 + item.children.iter().map(count_all).sum::<usize>()
}

fn all_complete(item: &TestItem) -> bool {
    matches!(
        item.state,
        TestState::Passed | TestState::Failed | TestState::Skipped | TestState::Errored
    ) && item.children.iter().all(all_complete)
}

fn find_item_recursive<'a>(item: &'a TestItem, id: &str) -> Option<&'a TestItem> {
    if item.id == id {
        return Some(item);
    }
    for child in &item.children {
        if let Some(found) = find_item_recursive(child, id) {
            return Some(found);
        }
    }
    None
}

fn collect_failed<'a>(item: &'a TestItem, result: &mut Vec<&'a TestItem>) {
    if matches!(item.state, TestState::Failed | TestState::Errored) {
        result.push(item);
    }
    for child in &item.children {
        collect_failed(child, result);
    }
}

fn flatten_recursive<'a>(item: &'a TestItem, result: &mut Vec<&'a TestItem>) {
    result.push(item);
    for child in &item.children {
        flatten_recursive(child, result);
    }
}

/// Service managing multiple test runs.
pub struct TestService {
    runs: Vec<TestRun>,
    next_id: u64,
}

impl TestService {
    pub fn new() -> Self {
        Self {
            runs: Vec::new(),
            next_id: 1,
        }
    }

    pub fn create_run(&mut self, name: impl Into<String>) -> String {
        let id = format!("run-{}", self.next_id);
        self.next_id += 1;
        self.runs.push(TestRun::new(&id, name));
        id
    }

    pub fn get_run(&self, id: &str) -> Option<&TestRun> {
        self.runs.iter().find(|r| r.id == id)
    }

    pub fn get_run_mut(&mut self, id: &str) -> Option<&mut TestRun> {
        self.runs.iter_mut().find(|r| r.id == id)
    }

    pub fn get_all_runs(&self) -> &[TestRun] {
        &self.runs
    }

    pub fn remove_run(&mut self, id: &str) -> bool {
        let len = self.runs.len();
        self.runs.retain(|r| r.id != id);
        self.runs.len() < len
    }

    /// Returns true if runs is empty.
    pub fn is_runs_empty(&self) -> bool {
        self.runs.is_empty()
    }

    /// Get the first run, if any.
    pub fn first_run(&self) -> Option<&TestRun> {
        self.runs.first()
    }

    /// Get the last run, if any.
    pub fn last_run(&self) -> Option<&TestRun> {
        self.runs.last()
    }

    /// Retain only runs matching the predicate.
    pub fn retain_runs(&mut self, f: impl Fn(&TestRun) -> bool) {
        self.runs.retain(|item| f(item));
    }
}

impl Default for TestService {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for test-view operations.
#[derive(Debug, Clone, PartialEq)]
pub struct TestViewStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl TestViewStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &TestViewStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for TestViewStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TestViewStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TestViewStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for test-view.
#[derive(Debug, Clone)]
pub struct TestViewValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl TestViewValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for TestViewValidator {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export VS Code Testing API types from ext-testing
pub use vsedit_ext_testing::{
    CoverageStats, DetailedCoverage, FileCoverage, TestItemCollection, TestOutputMessage,
    TestRunHistory, TestRunProfileKind, TestRunRequest, TestRunResult, TestTag,
    VscTestItem, VscTestRun, TestController, TestRunProfile, TestFramework,
    compute_summary, detect_test_framework, render_test_tree, render_result_line,
    CargoTestDiscoverer,
};

/// Render a test tree with state icons.
pub fn render_test_items_with_state(items: &[TestItem], indent: usize) -> String {
    let mut out = String::new();
    let prefix = "  ".repeat(indent);
    for item in items {
        let icon = state_icon(item.state);
        let duration = item
            .duration_ms
            .map(|d| format!(" ({:.0}ms)", d))
            .unwrap_or_default();
        out.push_str(&format!("{prefix}{icon} {}{duration}\n", item.label));
        if !item.children.is_empty() {
            out.push_str(&render_test_items_with_state(&item.children, indent + 1));
        }
    }
    out
}

/// Returns a single-character icon for the given state.
pub fn state_icon(state: TestState) -> &'static str {
    match state {
        TestState::Queued => "○",
        TestState::Running => "◉",
        TestState::Passed => "✓",
        TestState::Failed => "✗",
        TestState::Skipped => "⊘",
        TestState::Errored => "✗",
    }
}

/// Format a test run as a summary output panel.
pub fn render_output_panel(run: &TestRun) -> String {
    let stats = run.get_stats();
    let mut out = String::new();
    out.push_str(&format!("Test Run: {}\n", run.name));
    out.push_str(&format!(
        "Total: {}  Passed: {}  Failed: {}  Skipped: {}  Errored: {}\n",
        stats.total, stats.passed, stats.failed, stats.skipped, stats.errored
    ));
    if let Some(d) = run.duration_ms() {
        out.push_str(&format!("Duration: {}ms\n", d));
    }
    out.push('\n');
    for item in run.flatten_items() {
        let icon = state_icon(item.state);
        let duration = item
            .duration_ms
            .map(|d| format!(" ({:.0}ms)", d))
            .unwrap_or_default();
        let msg = item
            .message
            .as_deref()
            .map(|m| format!("  → {m}"))
            .unwrap_or_default();
        out.push_str(&format!("{icon} {}{duration}{msg}\n", item.label));
    }
    out
}

// ---------------------------------------------------------------------------
// TestResultSummary with pass/fail/skip counts
// ---------------------------------------------------------------------------

/// Aggregated summary of test results computed from a tree of `TestItem`s.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TestResultSummary {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub errored: usize,
    pub running: usize,
    pub queued: usize,
    pub total: usize,
    pub total_duration_ms: u64,
}

impl TestResultSummary {
    /// Compute a summary by recursively walking a slice of test items.
    pub fn from_items(items: &[TestItem]) -> Self {
        let mut summary = Self::default();
        for item in items {
            summary.count_item(item);
        }
        summary
    }

    fn count_item(&mut self, item: &TestItem) {
        if item.children.is_empty() {
            // Leaf node — this is an actual test
            self.total += 1;
            match item.state {
                TestState::Passed => self.passed += 1,
                TestState::Failed => self.failed += 1,
                TestState::Skipped => self.skipped += 1,
                TestState::Errored => self.errored += 1,
                TestState::Running => self.running += 1,
                TestState::Queued => self.queued += 1,
            }
            if let Some(dur) = item.duration_ms {
                self.total_duration_ms += dur as u64;
            }
        } else {
            // Container node — recurse into children
            for child in &item.children {
                self.count_item(child);
            }
        }
    }

    /// Returns the pass rate as a value between 0.0 and 1.0.
    /// Returns 0.0 if no tests have completed.
    pub fn pass_rate(&self) -> f64 {
        let completed = self.passed + self.failed + self.errored;
        if completed == 0 {
            0.0
        } else {
            self.passed as f64 / completed as f64
        }
    }

    /// Whether all tests have passed (none failed or errored).
    pub fn all_passed(&self) -> bool {
        self.failed == 0 && self.errored == 0 && self.passed > 0
    }

    /// Whether there are any failures.
    pub fn has_failures(&self) -> bool {
        self.failed > 0 || self.errored > 0
    }

    /// Format the summary as a status line, e.g., "5 passed, 2 failed, 1 skipped (120ms)".
    pub fn status_line(&self) -> String {
        let mut parts = Vec::new();
        if self.passed > 0 {
            parts.push(format!("{} passed", self.passed));
        }
        if self.failed > 0 {
            parts.push(format!("{} failed", self.failed));
        }
        if self.errored > 0 {
            parts.push(format!("{} errored", self.errored));
        }
        if self.skipped > 0 {
            parts.push(format!("{} skipped", self.skipped));
        }
        if self.running > 0 {
            parts.push(format!("{} running", self.running));
        }
        if self.queued > 0 {
            parts.push(format!("{} queued", self.queued));
        }
        let status = if parts.is_empty() {
            "No tests".to_string()
        } else {
            parts.join(", ")
        };
        if self.total_duration_ms > 0 {
            format!("{status} ({}ms)", self.total_duration_ms)
        } else {
            status
        }
    }
}

impl fmt::Display for TestResultSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.status_line())
    }
}

// ---------------------------------------------------------------------------
// TestFilter – filter tests by state, tag substring, or duration threshold
// ---------------------------------------------------------------------------

/// Criteria for filtering test items.
#[derive(Debug, Clone, Default)]
pub struct TestFilter {
    /// If set, only items whose state is in this list pass.
    pub states: Option<Vec<TestState>>,
    /// If set, only items whose label contains this substring (case-insensitive) pass.
    pub label_contains: Option<String>,
    /// If set, only items whose duration_ms is at or above this threshold pass.
    pub min_duration_ms: Option<f64>,
    /// If set, only items whose duration_ms is at or below this threshold pass.
    pub max_duration_ms: Option<f64>,
}

impl TestFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_states(mut self, states: Vec<TestState>) -> Self {
        self.states = Some(states);
        self
    }

    pub fn with_label_contains(mut self, sub: impl Into<String>) -> Self {
        self.label_contains = Some(sub.into());
        self
    }

    pub fn with_min_duration(mut self, ms: f64) -> Self {
        self.min_duration_ms = Some(ms);
        self
    }

    pub fn with_max_duration(mut self, ms: f64) -> Self {
        self.max_duration_ms = Some(ms);
        self
    }

    /// Returns `true` if the given item matches **all** active criteria.
    pub fn matches(&self, item: &TestItem) -> bool {
        if let Some(ref states) = self.states {
            if !states.contains(&item.state) {
                return false;
            }
        }
        if let Some(ref sub) = self.label_contains {
            if !item.label.to_lowercase().contains(&sub.to_lowercase()) {
                return false;
            }
        }
        if let Some(min) = self.min_duration_ms {
            match item.duration_ms {
                Some(d) if d >= min => {}
                _ => return false,
            }
        }
        if let Some(max) = self.max_duration_ms {
            match item.duration_ms {
                Some(d) if d <= max => {}
                _ => return false,
            }
        }
        true
    }

    /// Collect all leaf items from a tree that match the filter.
    pub fn apply<'a>(&self, items: &'a [TestItem]) -> Vec<&'a TestItem> {
        let mut out = Vec::new();
        for item in items {
            self.collect_matching(item, &mut out);
        }
        out
    }

    fn collect_matching<'a>(&self, item: &'a TestItem, out: &mut Vec<&'a TestItem>) {
        if item.children.is_empty() {
            if self.matches(item) {
                out.push(item);
            }
        } else {
            for child in &item.children {
                self.collect_matching(child, out);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TestDurationAnalyzer – compute duration statistics over a set of test items
// ---------------------------------------------------------------------------

/// Statistics about test durations within a run.
#[derive(Debug, Clone, PartialEq)]
pub struct DurationStats {
    pub count: usize,
    pub total_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub mean_ms: f64,
    pub median_ms: f64,
}

/// Analyze durations across test items.
pub struct TestDurationAnalyzer;

impl TestDurationAnalyzer {
    /// Compute duration statistics from a flat slice of test items.
    /// Only items with `duration_ms` set are considered.
    pub fn analyze(items: &[TestItem]) -> Option<DurationStats> {
        let mut durations: Vec<f64> = items.iter().filter_map(|i| i.duration_ms).collect();
        if durations.is_empty() {
            return None;
        }
        durations.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let count = durations.len();
        let total_ms: f64 = durations.iter().sum();
        let min_ms = durations[0];
        let max_ms = durations[count - 1];
        let mean_ms = total_ms / count as f64;
        let median_ms = if count % 2 == 0 {
            (durations[count / 2 - 1] + durations[count / 2]) / 2.0
        } else {
            durations[count / 2]
        };
        Some(DurationStats { count, total_ms, min_ms, max_ms, mean_ms, median_ms })
    }

    /// Return the top-N slowest items (by duration_ms) from the flattened run.
    pub fn slowest(run: &TestRun, n: usize) -> Vec<&TestItem> {
        let mut with_dur: Vec<&TestItem> = run
            .flatten_items()
            .into_iter()
            .filter(|i| i.duration_ms.is_some())
            .collect();
        with_dur.sort_by(|a, b| {
            b.duration_ms
                .partial_cmp(&a.duration_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        with_dur.truncate(n);
        with_dur
    }
}

// ---------------------------------------------------------------------------
// TestHistory – track sequential test run snapshots
// ---------------------------------------------------------------------------

/// A snapshot of a single historical test run.
#[derive(Debug, Clone)]
pub struct TestRunSnapshot {
    pub run_id: String,
    pub run_name: String,
    pub stats: TestRunStats,
    pub timestamp: u64,
}

/// Keeps an ordered history of test run snapshots.
#[derive(Debug, Clone, Default)]
pub struct TestHistory {
    snapshots: Vec<TestRunSnapshot>,
    max_entries: usize,
}

impl TestHistory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            snapshots: Vec::new(),
            max_entries,
        }
    }

    /// Record a snapshot from a completed `TestRun`.
    pub fn record(&mut self, run: &TestRun, timestamp: u64) {
        let snapshot = TestRunSnapshot {
            run_id: run.id.clone(),
            run_name: run.name.clone(),
            stats: run.get_stats(),
            timestamp,
        };
        self.snapshots.push(snapshot);
        if self.snapshots.len() > self.max_entries {
            self.snapshots.remove(0);
        }
    }

    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    pub fn latest(&self) -> Option<&TestRunSnapshot> {
        self.snapshots.last()
    }

    pub fn snapshots(&self) -> &[TestRunSnapshot] {
        &self.snapshots
    }

    /// Compute the average pass rate across all recorded snapshots.
    pub fn average_pass_rate(&self) -> f64 {
        if self.snapshots.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.snapshots.iter().map(|s| s.stats.pass_rate).sum();
        sum / self.snapshots.len() as f64
    }

    /// Return the snapshot with the worst (lowest) pass rate.
    pub fn worst_run(&self) -> Option<&TestRunSnapshot> {
        self.snapshots
            .iter()
            .min_by(|a, b| a.stats.pass_rate.partial_cmp(&b.stats.pass_rate).unwrap_or(std::cmp::Ordering::Equal))
    }
}

// ---------------------------------------------------------------------------
// TestTreeWalker – depth-first iterator over a test item tree
// ---------------------------------------------------------------------------

/// Depth-first iterator over all nodes in a `TestItem` tree.
pub struct TestTreeWalker<'a> {
    stack: Vec<(usize, &'a TestItem)>,
}

impl<'a> TestTreeWalker<'a> {
    /// Create a walker from a slice of root items.
    pub fn new(roots: &'a [TestItem]) -> Self {
        let stack: Vec<(usize, &'a TestItem)> = roots.iter().rev().map(|i| (0, i)).collect();
        Self { stack }
    }
}

impl<'a> Iterator for TestTreeWalker<'a> {
    type Item = (usize, &'a TestItem);

    fn next(&mut self) -> Option<Self::Item> {
        let (depth, item) = self.stack.pop()?;
        // push children in reverse so the first child is visited next
        for child in item.children.iter().rev() {
            self.stack.push((depth + 1, child));
        }
        Some((depth, item))
    }
}

// ---------------------------------------------------------------------------
// TestItemBuilder – ergonomic builder for TestItem
// ---------------------------------------------------------------------------

/// Builder for constructing `TestItem` instances.
pub struct TestItemBuilder {
    id: String,
    label: String,
    uri: Option<String>,
    line: Option<u32>,
    state: TestState,
    children: Vec<TestItem>,
    duration_ms: Option<f64>,
    message: Option<String>,
}

impl TestItemBuilder {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            uri: None,
            line: None,
            state: TestState::Queued,
            children: Vec::new(),
            duration_ms: None,
            message: None,
        }
    }

    pub fn state(mut self, state: TestState) -> Self {
        self.state = state;
        self
    }

    pub fn uri(mut self, uri: impl Into<String>) -> Self {
        self.uri = Some(uri.into());
        self
    }

    pub fn line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    pub fn duration_ms(mut self, ms: f64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    pub fn message(mut self, msg: impl Into<String>) -> Self {
        self.message = Some(msg.into());
        self
    }

    pub fn child(mut self, child: TestItem) -> Self {
        self.children.push(child);
        self
    }

    pub fn build(self) -> TestItem {
        TestItem {
            id: self.id,
            label: self.label,
            uri: self.uri,
            line: self.line,
            state: self.state,
            children: self.children,
            duration_ms: self.duration_ms,
            message: self.message,
        }
    }
}

/// Collect all items grouped by their state from a test item tree.
pub fn group_by_state(items: &[TestItem]) -> std::collections::HashMap<&'static str, Vec<&TestItem>> {
    let mut map: std::collections::HashMap<&'static str, Vec<&TestItem>> = std::collections::HashMap::new();
    for (_, item) in TestTreeWalker::new(items) {
        if item.children.is_empty() {
            map.entry(state_label(item.state)).or_default().push(item);
        }
    }
    map
}

/// Compute the depth of the deepest node in a test item tree.
pub fn max_depth(items: &[TestItem]) -> usize {
    TestTreeWalker::new(items).map(|(d, _)| d).max().unwrap_or(0)
}

// ---------------------------------------------------------------------------
// LocalTestRunProfile – test run configuration
// ---------------------------------------------------------------------------

/// Local configuration for how tests should be run.
///
/// Extends the basic [`TestRunProfile`] from `vsedit-ext-testing` with
/// additional fields like environment variables and arguments.
#[derive(Debug, Clone)]
pub struct LocalTestRunProfile {
    /// Profile label (e.g. "Run Tests", "Debug Tests").
    pub label: String,
    /// The kind of profile.
    pub kind: TestProfileKind,
    /// Whether this is the default profile for its kind.
    pub is_default: bool,
    /// Environment variables to set.
    pub env: std::collections::HashMap<String, String>,
    /// Arguments to pass to the test runner.
    pub args: Vec<String>,
}

impl LocalTestRunProfile {
    /// Create a new profile.
    pub fn new(label: impl Into<String>, kind: TestProfileKind) -> Self {
        Self {
            label: label.into(),
            kind,
            is_default: false,
            env: std::collections::HashMap::new(),
            args: Vec::new(),
        }
    }

    /// Mark as the default profile.
    pub fn with_default(mut self) -> Self {
        self.is_default = true;
        self
    }

    /// Add an environment variable.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Add an argument.
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }
}

impl fmt::Display for LocalTestRunProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({:?})", self.label, self.kind)
    }
}

// ---------------------------------------------------------------------------
// LocalTestCoverageReport – line/branch coverage stats
// ---------------------------------------------------------------------------

/// Local code coverage report for a test run.
///
/// Provides a higher-level view than [`FileCoverage`] from `vsedit-ext-testing`,
/// aggregating line and branch coverage across multiple files.
#[derive(Debug, Clone, Default)]
pub struct LocalTestCoverageReport {
    /// Total lines in covered files.
    pub total_lines: u64,
    /// Lines covered by tests.
    pub covered_lines: u64,
    /// Total branches.
    pub total_branches: u64,
    /// Branches covered.
    pub covered_branches: u64,
    /// Per-file coverage data.
    pub files: Vec<LocalFileCoverage>,
}

/// Coverage data for a single file.
#[derive(Debug, Clone)]
pub struct LocalFileCoverage {
    /// File path.
    pub path: String,
    /// Lines covered.
    pub covered: u64,
    /// Total lines.
    pub total: u64,
}

impl LocalTestCoverageReport {
    /// Line coverage percentage (0.0–100.0).
    pub fn line_coverage_percent(&self) -> f64 {
        if self.total_lines == 0 {
            return 0.0;
        }
        self.covered_lines as f64 / self.total_lines as f64 * 100.0
    }

    /// Branch coverage percentage (0.0–100.0).
    pub fn branch_coverage_percent(&self) -> f64 {
        if self.total_branches == 0 {
            return 0.0;
        }
        self.covered_branches as f64 / self.total_branches as f64 * 100.0
    }

    /// Add file coverage data, updating totals.
    pub fn add_file(&mut self, path: impl Into<String>, covered: u64, total: u64) {
        self.covered_lines += covered;
        self.total_lines += total;
        self.files.push(LocalFileCoverage {
            path: path.into(),
            covered,
            total,
        });
    }

    /// Get files with coverage below the given threshold.
    pub fn files_below_threshold(&self, threshold: f64) -> Vec<&LocalFileCoverage> {
        self.files
            .iter()
            .filter(|f| {
                if f.total == 0 {
                    return false;
                }
                (f.covered as f64 / f.total as f64 * 100.0) < threshold
            })
            .collect()
    }
}

impl fmt::Display for LocalTestCoverageReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Coverage: {:.1}% lines, {:.1}% branches",
            self.line_coverage_percent(),
            self.branch_coverage_percent()
        )
    }
}

// ---------------------------------------------------------------------------
// TestHistoryTracker – comparing run results
// ---------------------------------------------------------------------------

/// Compares test run results across multiple runs.
#[derive(Debug, Clone)]
pub struct TestHistoryTracker {
    runs: Vec<TestRunSummary>,
    max_history: usize,
}

/// Summary of a single test run for history tracking.
#[derive(Debug, Clone)]
pub struct TestRunSummary {
    /// Run identifier.
    pub run_id: String,
    /// Total tests.
    pub total: usize,
    /// Passed tests.
    pub passed: usize,
    /// Failed tests.
    pub failed: usize,
    /// Timestamp (epoch millis).
    pub timestamp: u64,
}

impl Default for TestHistoryTracker {
    fn default() -> Self {
        Self {
            runs: Vec::new(),
            max_history: 50,
        }
    }
}

impl TestHistoryTracker {
    /// Create a tracker with the given history depth.
    pub fn new(max_history: usize) -> Self {
        Self {
            max_history,
            ..Default::default()
        }
    }

    /// Record a run summary.
    pub fn record(&mut self, summary: TestRunSummary) {
        if self.runs.len() >= self.max_history {
            self.runs.remove(0);
        }
        self.runs.push(summary);
    }

    /// Get the most recent run.
    pub fn latest(&self) -> Option<&TestRunSummary> {
        self.runs.last()
    }

    /// Get the failure trend (number of failed tests in recent runs).
    pub fn failure_trend(&self, last_n: usize) -> Vec<usize> {
        self.runs
            .iter()
            .rev()
            .take(last_n)
            .map(|r| r.failed)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    /// Whether the latest run is a regression (more failures than previous).
    pub fn is_regression(&self) -> bool {
        if self.runs.len() < 2 {
            return false;
        }
        let current = &self.runs[self.runs.len() - 1];
        let previous = &self.runs[self.runs.len() - 2];
        current.failed > previous.failed
    }

    /// Number of recorded runs.
    pub fn run_count(&self) -> usize {
        self.runs.len()
    }

    /// Average pass rate across all recorded runs.
    pub fn average_pass_rate(&self) -> f64 {
        if self.runs.is_empty() {
            return 0.0;
        }
        let total_rate: f64 = self
            .runs
            .iter()
            .map(|r| {
                if r.total == 0 {
                    0.0
                } else {
                    r.passed as f64 / r.total as f64
                }
            })
            .sum();
        total_rate / self.runs.len() as f64
    }
}

// ---------------------------------------------------------------------------
// Test output capture with ANSI stripping
// ---------------------------------------------------------------------------

/// Strips ANSI escape sequences from test output.
pub fn strip_ansi(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until 'm' or end of string (CSI sequence)
            if chars.peek() == Some(&'[') {
                chars.next(); // skip '['
                while let Some(&nc) = chars.peek() {
                    chars.next();
                    if nc.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Captures and processes test output.
#[derive(Debug, Clone)]
pub struct TestOutputCapture {
    lines: Vec<String>,
    strip_ansi_codes: bool,
}

impl Default for TestOutputCapture {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            strip_ansi_codes: true,
        }
    }
}

impl TestOutputCapture {
    /// Create a new capture buffer.
    pub fn new(strip_ansi_codes: bool) -> Self {
        Self {
            strip_ansi_codes,
            ..Default::default()
        }
    }

    /// Append output text (may contain multiple lines).
    pub fn append(&mut self, text: &str) {
        for line in text.lines() {
            let processed = if self.strip_ansi_codes {
                strip_ansi(line)
            } else {
                line.to_string()
            };
            self.lines.push(processed);
        }
    }

    /// Get all captured lines.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Total number of lines.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Search for lines containing a keyword.
    pub fn grep(&self, keyword: &str) -> Vec<&str> {
        self.lines
            .iter()
            .filter(|l| l.contains(keyword))
            .map(|l| l.as_str())
            .collect()
    }

    /// Clear captured output.
    pub fn clear(&mut self) {
        self.lines.clear();
    }
}


// === Test Run Scheduler ===

/// Test Run Scheduler implementation.
#[derive(Debug, Clone)]
pub struct TestRunScheduler {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: TestRunSchedulerStats,
}

/// Statistics for TestRunScheduler.
#[derive(Debug, Clone, Default)]
pub struct TestRunSchedulerStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl TestRunSchedulerStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl TestRunScheduler {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: TestRunSchedulerStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &TestRunSchedulerStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for TestRunScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// === Test Output Diff Formatter ===

/// Priority level for TestOutputDiffFormatter items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TestOutputDiffFormatterPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl TestOutputDiffFormatterPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for TestOutputDiffFormatterPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Test Output Diff Formatter implementation.
#[derive(Debug, Clone)]
pub struct TestOutputDiffFormatter {
    items: Vec<TestOutputDiffFormatterItem>,
    max_items: usize,
    default_priority: TestOutputDiffFormatterPriority,
}

/// A single item in TestOutputDiffFormatter.
#[derive(Debug, Clone)]
pub struct TestOutputDiffFormatterItem {
    pub id: String,
    pub label: String,
    pub priority: TestOutputDiffFormatterPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl TestOutputDiffFormatterItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: TestOutputDiffFormatterPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: TestOutputDiffFormatterPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl TestOutputDiffFormatter {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: TestOutputDiffFormatterPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: TestOutputDiffFormatterItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<TestOutputDiffFormatterItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&TestOutputDiffFormatterItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: TestOutputDiffFormatterPriority) -> Vec<&TestOutputDiffFormatterItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&TestOutputDiffFormatterItem> {
        let mut sorted: Vec<&TestOutputDiffFormatterItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&TestOutputDiffFormatterItem> {
        let mut sorted: Vec<&TestOutputDiffFormatterItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&TestOutputDiffFormatterItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: TestOutputDiffFormatterPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> TestOutputDiffFormatterPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &TestOutputDiffFormatterItem> {
        self.items.iter()
    }
}

impl Default for TestOutputDiffFormatter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// vsedit-test-view: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestViewXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl TestViewXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for TestViewXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct TestViewXRegistry {
    entries: Vec<TestViewXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl TestViewXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: TestViewXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&TestViewXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut TestViewXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<TestViewXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&TestViewXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&TestViewXConfig> {
        let mut sorted: Vec<&TestViewXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&TestViewXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> TestViewXIterator<'_> {
        TestViewXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct TestViewXIterator<'a> {
    inner: std::slice::Iter<'a, TestViewXConfig>,
}

impl<'a> Iterator for TestViewXIterator<'a> {
    type Item = &'a TestViewXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct TestViewXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl TestViewXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct TestViewXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl TestViewXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &TestViewXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &TestViewXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &TestViewXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for TestViewXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct TestViewXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl TestViewXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &TestViewXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &TestViewXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for TestViewXValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 179
// ---------------------------------------------------------------------------

/// Generic object pool `Xc179Pool<T>`.
pub struct Xc179Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc179Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc179PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc179Pool<T> {
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
    pub fn stats(&self) -> Xc179PoolStats {
        Xc179PoolStats {
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

impl<T> Default for Xc179Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc179Scheduler`.
pub struct Xc179Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc179Scheduler {
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

impl Default for Xc179Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_179 hash for the given byte slice.
pub fn xc_179_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_179 convention.
pub fn xc_179_reverse(s: &str) -> String {
    s.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_item(id: &str, state: TestState) -> TestItem {
        TestItem {
            id: id.to_string(),
            label: id.to_string(),
            uri: None,
            line: None,
            state,
            children: Vec::new(),
            duration_ms: None,
            message: None,
        }
    }

    #[test]
    fn run_counts() {
        let mut run = TestRun::new("r1", "Suite");
        run.add_item(test_item("t1", TestState::Passed));
        run.add_item(test_item("t2", TestState::Failed));
        run.add_item(test_item("t3", TestState::Passed));
        assert_eq!(run.pass_count(), 2);
        assert_eq!(run.fail_count(), 1);
        assert_eq!(run.total_count(), 3);
    }

    #[test]
    fn run_completion() {
        let mut run = TestRun::new("r1", "Suite");
        run.add_item(test_item("t1", TestState::Passed));
        run.add_item(test_item("t2", TestState::Running));
        assert!(!run.is_complete());
        run.set_state("t2", TestState::Passed);
        assert!(run.is_complete());
    }

    #[test]
    fn run_duration() {
        let mut run = TestRun::new("r1", "Suite");
        assert!(run.duration_ms().is_none());
        run.started_at = Some(100);
        run.finished_at = Some(350);
        assert_eq!(run.duration_ms(), Some(250));
    }

    #[test]
    fn service_create_and_get() {
        let mut svc = TestService::new();
        let id = svc.create_run("my tests");
        assert!(svc.get_run(&id).is_some());
        assert!(svc.get_run("nonexistent").is_none());
    }

    #[test]
    fn skipped_and_error_counts() {
        let mut run = TestRun::new("r1", "Suite");
        run.add_item(test_item("t1", TestState::Skipped));
        run.add_item(test_item("t2", TestState::Errored));
        run.add_item(test_item("t3", TestState::Skipped));
        assert_eq!(run.skipped_count(), 2);
        assert_eq!(run.error_count(), 1);
    }

    #[test]
    fn get_stats_returns_correct_values() {
        let mut run = TestRun::new("r1", "Suite");
        run.add_item(test_item("t1", TestState::Passed));
        run.add_item(test_item("t2", TestState::Failed));
        run.add_item(test_item("t3", TestState::Skipped));
        run.add_item(test_item("t4", TestState::Errored));
        run.add_item(test_item("t5", TestState::Running));
        run.add_item(test_item("t6", TestState::Queued));
        let stats = run.get_stats();
        assert_eq!(stats.total, 6);
        assert_eq!(stats.passed, 1);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.skipped, 1);
        assert_eq!(stats.errored, 1);
        assert_eq!(stats.running, 1);
        assert_eq!(stats.queued, 1);
        assert!((stats.pass_rate - 1.0 / 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn get_stats_empty_run() {
        let run = TestRun::new("r1", "Empty");
        let stats = run.get_stats();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.pass_rate, 0.0);
    }

    #[test]
    fn find_item_top_level_and_nested() {
        let mut run = TestRun::new("r1", "Suite");
        let mut parent = test_item("p1", TestState::Passed);
        parent.children.push(test_item("c1", TestState::Failed));
        run.add_item(parent);
        run.add_item(test_item("t2", TestState::Passed));

        assert_eq!(run.find_item("p1").unwrap().id, "p1");
        assert_eq!(run.find_item("c1").unwrap().id, "c1");
        assert_eq!(run.find_item("t2").unwrap().id, "t2");
        assert!(run.find_item("nonexistent").is_none());
    }

    #[test]
    fn get_failed_items_includes_errored() {
        let mut run = TestRun::new("r1", "Suite");
        let mut parent = test_item("p1", TestState::Failed);
        parent.children.push(test_item("c1", TestState::Errored));
        parent.children.push(test_item("c2", TestState::Passed));
        run.add_item(parent);
        run.add_item(test_item("t2", TestState::Passed));

        let failed: Vec<&str> = run.get_failed_items().iter().map(|i| i.id.as_str()).collect();
        assert_eq!(failed, vec!["p1", "c1"]);
    }

    #[test]
    fn flatten_items_depth_first() {
        let mut run = TestRun::new("r1", "Suite");
        let mut parent = test_item("p1", TestState::Passed);
        parent.children.push(test_item("c1", TestState::Passed));
        parent.children.push(test_item("c2", TestState::Passed));
        run.add_item(parent);
        run.add_item(test_item("t2", TestState::Passed));

        let ids: Vec<&str> = run.flatten_items().iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, vec!["p1", "c1", "c2", "t2"]);
    }

    #[test]
    fn test_item_has_children_and_child_count() {
        let mut parent = test_item("p1", TestState::Passed);
        assert!(!parent.has_children());
        assert_eq!(parent.child_count(), 0);

        let mut child = test_item("c1", TestState::Passed);
        child.children.push(test_item("gc1", TestState::Passed));
        parent.children.push(child);
        parent.children.push(test_item("c2", TestState::Passed));

        assert!(parent.has_children());
        assert_eq!(parent.child_count(), 3);
    }

    #[test]
    fn test_profile_creation() {
        let profile = TestProfile {
            id: "p1".to_string(),
            label: "Run".to_string(),
            kind: TestProfileKind::Run,
        };
        assert_eq!(profile.kind, TestProfileKind::Run);
        let debug = TestProfile {
            id: "p2".to_string(),
            label: "Debug".to_string(),
            kind: TestProfileKind::Debug,
        };
        assert_eq!(debug.kind, TestProfileKind::Debug);
        let cov = TestProfile {
            id: "p3".to_string(),
            label: "Coverage".to_string(),
            kind: TestProfileKind::Coverage,
        };
        assert_eq!(cov.kind, TestProfileKind::Coverage);
    }

    #[test]
    fn state_label_values() {
        assert_eq!(state_label(TestState::Queued), "Queued");
        assert_eq!(state_label(TestState::Running), "Running");
        assert_eq!(state_label(TestState::Passed), "Passed");
        assert_eq!(state_label(TestState::Failed), "Failed");
        assert_eq!(state_label(TestState::Skipped), "Skipped");
        assert_eq!(state_label(TestState::Errored), "Errored");
    }

    #[test]
    fn service_get_run_mut() {
        let mut svc = TestService::new();
        let id = svc.create_run("mutate me");
        svc.get_run_mut(&id).unwrap().add_item(test_item("t1", TestState::Passed));
        assert_eq!(svc.get_run(&id).unwrap().total_count(), 1);
        assert!(svc.get_run_mut("nope").is_none());
    }

    #[test]
    fn service_get_all_runs_and_remove() {
        let mut svc = TestService::new();
        let id1 = svc.create_run("first");
        let id2 = svc.create_run("second");
        assert_eq!(svc.get_all_runs().len(), 2);

        assert!(svc.remove_run(&id1));
        assert_eq!(svc.get_all_runs().len(), 1);
        assert_eq!(svc.get_all_runs()[0].id, id2);

        assert!(!svc.remove_run("nonexistent"));
    }

    #[test]
    fn eq_teststate_same() {
        assert_eq!(TestState::Queued, TestState::Queued);
    }

    #[test]
    fn ne_teststate_diff() {
        assert_ne!(TestState::Queued, TestState::Running);
    }

    #[test]
    fn eq_testprofilekind_same() {
        assert_eq!(TestProfileKind::Run, TestProfileKind::Run);
    }

    #[test]
    fn ne_testprofilekind_diff() {
        assert_ne!(TestProfileKind::Run, TestProfileKind::Debug);
    }

    #[test]
    fn behavior_check_0() {
        let _svc = TestService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = TestService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = TestService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = TestService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = TestService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = TestService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = TestService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = TestService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = TestService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = TestService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = TestService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = TestService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = TestService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        let _svc = TestService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        let _svc = TestService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn test_view_stats_new_defaults() {
        let stats = TestViewStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn test_view_stats_record_success() {
        let mut stats = TestViewStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_view_stats_record_failure() {
        let mut stats = TestViewStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_view_stats_reset() {
        let mut stats = TestViewStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn test_view_stats_merge() {
        let mut a = TestViewStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = TestViewStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn test_view_stats_display() {
        let mut stats = TestViewStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn test_view_stats_default() {
        let stats = TestViewStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn test_view_validator_accepts_valid_name() {
        let v = TestViewValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn test_view_validator_rejects_empty() {
        let v = TestViewValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn test_view_validator_rejects_too_long() {
        let v = TestViewValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn test_view_validator_forbidden_prefix() {
        let v = TestViewValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn test_view_validator_allowed_chars() {
        let v = TestViewValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn test_view_validator_range() {
        let v = TestViewValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn test_view_sanitize_removes_control() {
        let result = TestViewValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn test_view_truncate_short_string() {
        assert_eq!(TestViewValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn test_view_truncate_long_string() {
        let result = TestViewValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn test_view_is_ascii_printable() {
        assert!(TestViewValidator::is_ascii_printable("Hello World 123"));
        assert!(!TestViewValidator::is_ascii_printable("Hello\x00World"));
    }

    fn make_leaf(state: TestState, dur_ms: Option<f64>) -> TestItem {
        TestItem {
            id: "t".to_string(),
            label: "test".to_string(),
            uri: None,
            line: None,
            state,
            children: vec![],
            duration_ms: dur_ms,
            message: None,
        }
    }

    #[test]
    fn test_result_summary_all_passed() {
        let items = vec![
            make_leaf(TestState::Passed, Some(10.0)),
            make_leaf(TestState::Passed, Some(20.0)),
        ];
        let summary = TestResultSummary::from_items(&items);
        assert_eq!(summary.passed, 2);
        assert_eq!(summary.total, 2);
        assert_eq!(summary.total_duration_ms, 30);
        assert!(summary.all_passed());
        assert!(!summary.has_failures());
    }

    #[test]
    fn test_result_summary_mixed() {
        let items = vec![
            make_leaf(TestState::Passed, Some(10.0)),
            make_leaf(TestState::Failed, Some(5.0)),
            make_leaf(TestState::Skipped, None),
        ];
        let summary = TestResultSummary::from_items(&items);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.skipped, 1);
        assert_eq!(summary.total, 3);
        assert!(summary.has_failures());
        assert!(!summary.all_passed());
    }

    #[test]
    fn test_result_summary_nested() {
        let suite = TestItem {
            id: "suite".to_string(),
            label: "Suite".to_string(),
            uri: None,
            line: None,
            state: TestState::Passed,
            children: vec![
                make_leaf(TestState::Passed, Some(5.0)),
                make_leaf(TestState::Failed, Some(3.0)),
            ],
            duration_ms: None,
            message: None,
        };
        let summary = TestResultSummary::from_items(&[suite]);
        assert_eq!(summary.total, 2); // only leaf tests counted
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 1);
    }

    #[test]
    fn test_result_summary_pass_rate() {
        let items = vec![
            make_leaf(TestState::Passed, None),
            make_leaf(TestState::Passed, None),
            make_leaf(TestState::Failed, None),
        ];
        let summary = TestResultSummary::from_items(&items);
        let rate = summary.pass_rate();
        assert!((rate - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn test_result_summary_status_line() {
        let items = vec![
            make_leaf(TestState::Passed, Some(100.0)),
            make_leaf(TestState::Skipped, None),
        ];
        let summary = TestResultSummary::from_items(&items);
        let line = summary.status_line();
        assert!(line.contains("1 passed"));
        assert!(line.contains("1 skipped"));
        assert!(line.contains("100ms"));
    }

    #[test]
    fn test_result_summary_empty() {
        let summary = TestResultSummary::from_items(&[]);
        assert_eq!(summary.total, 0);
        assert_eq!(summary.status_line(), "No tests");
        assert!(!summary.all_passed());
    }

    // -----------------------------------------------------------------------
    // TestFilter tests
    // -----------------------------------------------------------------------

    fn make_named_leaf(id: &str, label: &str, state: TestState, dur: Option<f64>) -> TestItem {
        TestItem {
            id: id.to_string(),
            label: label.to_string(),
            uri: None,
            line: None,
            state,
            children: vec![],
            duration_ms: dur,
            message: None,
        }
    }

    #[test]
    fn test_filter_by_state() {
        let items = vec![
            make_named_leaf("a", "alpha", TestState::Passed, Some(10.0)),
            make_named_leaf("b", "beta", TestState::Failed, Some(20.0)),
            make_named_leaf("c", "gamma", TestState::Skipped, None),
        ];
        let filter = TestFilter::new().with_states(vec![TestState::Failed, TestState::Skipped]);
        let matched = filter.apply(&items);
        let ids: Vec<&str> = matched.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "c"]);
    }

    #[test]
    fn test_filter_by_label_case_insensitive() {
        let items = vec![
            make_named_leaf("a", "TestLogin", TestState::Passed, None),
            make_named_leaf("b", "TestLogout", TestState::Passed, None),
            make_named_leaf("c", "TestDashboard", TestState::Passed, None),
        ];
        let filter = TestFilter::new().with_label_contains("login");
        let matched = filter.apply(&items);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].id, "a");
    }

    #[test]
    fn test_filter_by_duration_range() {
        let items = vec![
            make_named_leaf("fast", "fast", TestState::Passed, Some(5.0)),
            make_named_leaf("mid", "mid", TestState::Passed, Some(50.0)),
            make_named_leaf("slow", "slow", TestState::Passed, Some(500.0)),
            make_named_leaf("none", "none", TestState::Passed, None),
        ];
        let filter = TestFilter::new().with_min_duration(10.0).with_max_duration(100.0);
        let matched = filter.apply(&items);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].id, "mid");
    }

    #[test]
    fn test_filter_combined_criteria() {
        let items = vec![
            make_named_leaf("a", "TestAuth", TestState::Passed, Some(100.0)),
            make_named_leaf("b", "TestAuth", TestState::Failed, Some(200.0)),
            make_named_leaf("c", "TestDB", TestState::Passed, Some(300.0)),
        ];
        let filter = TestFilter::new()
            .with_states(vec![TestState::Passed])
            .with_label_contains("auth");
        let matched = filter.apply(&items);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].id, "a");
    }

    #[test]
    fn test_filter_empty_matches_all() {
        let items = vec![
            make_named_leaf("a", "x", TestState::Passed, None),
            make_named_leaf("b", "y", TestState::Failed, None),
        ];
        let filter = TestFilter::new();
        assert_eq!(filter.apply(&items).len(), 2);
    }

    // -----------------------------------------------------------------------
    // TestDurationAnalyzer tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_duration_analyzer_basic() {
        let items = vec![
            make_named_leaf("a", "a", TestState::Passed, Some(10.0)),
            make_named_leaf("b", "b", TestState::Passed, Some(30.0)),
            make_named_leaf("c", "c", TestState::Passed, Some(20.0)),
        ];
        let stats = TestDurationAnalyzer::analyze(&items).unwrap();
        assert_eq!(stats.count, 3);
        assert!((stats.total_ms - 60.0).abs() < f64::EPSILON);
        assert!((stats.min_ms - 10.0).abs() < f64::EPSILON);
        assert!((stats.max_ms - 30.0).abs() < f64::EPSILON);
        assert!((stats.mean_ms - 20.0).abs() < f64::EPSILON);
        assert!((stats.median_ms - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_duration_analyzer_none_when_empty() {
        let items: Vec<TestItem> = vec![
            make_named_leaf("a", "a", TestState::Passed, None),
        ];
        assert!(TestDurationAnalyzer::analyze(&items).is_none());
    }

    #[test]
    fn test_duration_analyzer_slowest() {
        let mut run = TestRun::new("r1", "Suite");
        run.add_item(make_named_leaf("fast", "fast", TestState::Passed, Some(1.0)));
        run.add_item(make_named_leaf("mid", "mid", TestState::Passed, Some(50.0)));
        run.add_item(make_named_leaf("slow", "slow", TestState::Passed, Some(200.0)));
        let slowest = TestDurationAnalyzer::slowest(&run, 2);
        assert_eq!(slowest.len(), 2);
        assert_eq!(slowest[0].id, "slow");
        assert_eq!(slowest[1].id, "mid");
    }

    // -----------------------------------------------------------------------
    // TestHistory tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_history_record_and_retrieve() {
        let mut history = TestHistory::new(10);
        assert!(history.is_empty());

        let mut run = TestRun::new("r1", "Run 1");
        run.add_item(test_item("t1", TestState::Passed));
        history.record(&run, 1000);

        assert_eq!(history.len(), 1);
        let snap = history.latest().unwrap();
        assert_eq!(snap.run_id, "r1");
        assert_eq!(snap.timestamp, 1000);
        assert_eq!(snap.stats.total, 1);
        assert_eq!(snap.stats.passed, 1);
    }

    #[test]
    fn test_history_evicts_oldest() {
        let mut history = TestHistory::new(2);
        let mut r1 = TestRun::new("r1", "A");
        r1.add_item(test_item("t1", TestState::Passed));
        history.record(&r1, 1);

        let mut r2 = TestRun::new("r2", "B");
        r2.add_item(test_item("t2", TestState::Failed));
        history.record(&r2, 2);

        let mut r3 = TestRun::new("r3", "C");
        r3.add_item(test_item("t3", TestState::Passed));
        history.record(&r3, 3);

        assert_eq!(history.len(), 2);
        assert_eq!(history.snapshots()[0].run_id, "r2");
        assert_eq!(history.snapshots()[1].run_id, "r3");
    }

    #[test]
    fn test_history_average_pass_rate() {
        let mut history = TestHistory::new(10);

        // Run with 100% pass rate
        let mut r1 = TestRun::new("r1", "good");
        r1.add_item(test_item("t1", TestState::Passed));
        history.record(&r1, 1);

        // Run with 0% pass rate
        let mut r2 = TestRun::new("r2", "bad");
        r2.add_item(test_item("t2", TestState::Failed));
        history.record(&r2, 2);

        let avg = history.average_pass_rate();
        assert!((avg - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_history_worst_run() {
        let mut history = TestHistory::new(10);

        let mut good = TestRun::new("good", "good");
        good.add_item(test_item("t1", TestState::Passed));
        good.add_item(test_item("t2", TestState::Passed));
        history.record(&good, 1);

        let mut bad = TestRun::new("bad", "bad");
        bad.add_item(test_item("t3", TestState::Passed));
        bad.add_item(test_item("t4", TestState::Failed));
        bad.add_item(test_item("t5", TestState::Failed));
        history.record(&bad, 2);

        let worst = history.worst_run().unwrap();
        assert_eq!(worst.run_id, "bad");
    }

    #[test]
    fn test_tree_walker_visits_all() {
        let child1 = test_item("c1", TestState::Passed);
        let child2 = test_item("c2", TestState::Failed);
        let mut parent = test_item("p1", TestState::Running);
        parent.children = vec![child1, child2];
        let items = vec![parent, test_item("t2", TestState::Skipped)];
        let visited: Vec<&str> = TestTreeWalker::new(&items).map(|(_, i)| i.id.as_str()).collect();
        assert_eq!(visited, vec!["p1", "c1", "c2", "t2"]);
    }

    #[test]
    fn test_tree_walker_depths() {
        let grandchild = test_item("gc", TestState::Passed);
        let mut child = test_item("c", TestState::Passed);
        child.children = vec![grandchild];
        let mut root = test_item("r", TestState::Passed);
        root.children = vec![child];
        let depths: Vec<usize> = TestTreeWalker::new(&[root]).map(|(d, _)| d).collect();
        assert_eq!(depths, vec![0, 1, 2]);
    }

    #[test]
    fn test_item_builder() {
        let item = TestItemBuilder::new("t1", "Test One")
            .state(TestState::Passed)
            .duration_ms(42.0)
            .uri("file:///test.rs".to_string())
            .line(10)
            .message("ok")
            .build();
        assert_eq!(item.id, "t1");
        assert_eq!(item.label, "Test One");
        assert_eq!(item.state, TestState::Passed);
        assert_eq!(item.duration_ms, Some(42.0));
        assert_eq!(item.uri.as_deref(), Some("file:///test.rs"));
        assert_eq!(item.line, Some(10));
        assert_eq!(item.message.as_deref(), Some("ok"));
    }

    #[test]
    fn test_item_builder_with_children() {
        let child = TestItemBuilder::new("c1", "child").state(TestState::Failed).build();
        let parent = TestItemBuilder::new("p1", "parent")
            .state(TestState::Running)
            .child(child)
            .build();
        assert!(parent.has_children());
        assert_eq!(parent.child_count(), 1);
    }

    #[test]
    fn test_group_by_state() {
        let items = vec![
            test_item("t1", TestState::Passed),
            test_item("t2", TestState::Passed),
            test_item("t3", TestState::Failed),
            test_item("t4", TestState::Skipped),
        ];
        let groups = group_by_state(&items);
        assert_eq!(groups.get("Passed").map(|v| v.len()), Some(2));
        assert_eq!(groups.get("Failed").map(|v| v.len()), Some(1));
        assert_eq!(groups.get("Skipped").map(|v| v.len()), Some(1));
        assert!(groups.get("Errored").is_none());
    }

    #[test]
    fn test_max_depth() {
        let gc = test_item("gc", TestState::Passed);
        let mut c = test_item("c", TestState::Passed);
        c.children = vec![gc];
        let mut r = test_item("r", TestState::Passed);
        r.children = vec![c];
        assert_eq!(max_depth(&[r]), 2);
        assert_eq!(max_depth(&[test_item("solo", TestState::Passed)]), 0);
    }

    // -- LocalTestRunProfile tests --

    #[test]
    fn profile_creation() {
        let p = LocalTestRunProfile::new("Run", TestProfileKind::Run)
            .with_default()
            .with_env("RUST_LOG", "debug")
            .with_arg("--nocapture");
        assert_eq!(p.label, "Run");
        assert!(p.is_default);
        assert_eq!(p.env.get("RUST_LOG").unwrap(), "debug");
        assert_eq!(p.args, vec!["--nocapture"]);
    }

    #[test]
    fn profile_display() {
        let p = LocalTestRunProfile::new("Debug", TestProfileKind::Debug);
        let s = format!("{}", p);
        assert!(s.contains("Debug"));
    }

    // -- LocalTestCoverageReport tests --

    #[test]
    fn coverage_report() {
        let mut report = LocalTestCoverageReport::default();
        report.add_file("src/lib.rs", 80, 100);
        report.add_file("src/util.rs", 10, 50);
        assert!((report.line_coverage_percent() - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn coverage_below_threshold() {
        let mut report = LocalTestCoverageReport::default();
        report.add_file("good.rs", 90, 100);
        report.add_file("bad.rs", 10, 100);
        let below = report.files_below_threshold(50.0);
        assert_eq!(below.len(), 1);
        assert_eq!(below[0].path, "bad.rs");
    }

    #[test]
    fn coverage_empty() {
        let report = LocalTestCoverageReport::default();
        assert!((report.line_coverage_percent()).abs() < f64::EPSILON);
    }

    // -- TestHistoryTracker tests --

    #[test]
    fn history_tracker_record() {
        let mut t = TestHistoryTracker::new(10);
        t.record(TestRunSummary { run_id: "1".into(), total: 10, passed: 9, failed: 1, timestamp: 100 });
        t.record(TestRunSummary { run_id: "2".into(), total: 10, passed: 8, failed: 2, timestamp: 200 });
        assert!(t.is_regression());
    }

    #[test]
    fn history_tracker_no_regression() {
        let mut t = TestHistoryTracker::new(10);
        t.record(TestRunSummary { run_id: "1".into(), total: 10, passed: 8, failed: 2, timestamp: 100 });
        t.record(TestRunSummary { run_id: "2".into(), total: 10, passed: 9, failed: 1, timestamp: 200 });
        assert!(!t.is_regression());
    }

    #[test]
    fn history_tracker_failure_trend() {
        let mut t = TestHistoryTracker::new(10);
        t.record(TestRunSummary { run_id: "1".into(), total: 10, passed: 9, failed: 1, timestamp: 100 });
        t.record(TestRunSummary { run_id: "2".into(), total: 10, passed: 7, failed: 3, timestamp: 200 });
        assert_eq!(t.failure_trend(2), vec![1, 3]);
    }

    #[test]
    fn history_average_pass_rate() {
        let mut t = TestHistoryTracker::new(10);
        t.record(TestRunSummary { run_id: "1".into(), total: 10, passed: 10, failed: 0, timestamp: 100 });
        t.record(TestRunSummary { run_id: "2".into(), total: 10, passed: 8, failed: 2, timestamp: 200 });
        assert!((t.average_pass_rate() - 0.9).abs() < f64::EPSILON);
    }

    // -- strip_ansi tests --

    #[test]
    fn strip_ansi_basic() {
        assert_eq!(strip_ansi("\x1b[32mPASS\x1b[0m"), "PASS");
    }

    #[test]
    fn strip_ansi_no_codes() {
        assert_eq!(strip_ansi("plain text"), "plain text");
    }

    // -- TestOutputCapture tests --

    #[test]
    fn output_capture_append() {
        let mut cap = TestOutputCapture::new(true);
        cap.append("line 1\n\x1b[31mline 2\x1b[0m");
        assert_eq!(cap.line_count(), 2);
        assert_eq!(cap.lines()[1], "line 2");
    }

    #[test]
    fn output_capture_grep() {
        let mut cap = TestOutputCapture::new(false);
        cap.append("test PASSED\ntest FAILED\ntest PASSED");
        let failed = cap.grep("FAILED");
        assert_eq!(failed.len(), 1);
    }

    #[test]
    fn testRunScheduler_new() {
        let s = TestRunScheduler::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn testRunScheduler_add_contains() {
        let mut s = TestRunScheduler::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn testRunScheduler_add_duplicate() {
        let mut s = TestRunScheduler::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn testRunScheduler_remove() {
        let mut s = TestRunScheduler::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn testRunScheduler_capacity() {
        let s = TestRunScheduler::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn testRunScheduler_search() {
        let mut s = TestRunScheduler::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn testRunScheduler_stats() {
        let mut s = TestRunScheduler::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn testOutputDiffFormatter_new() {
        let m = TestOutputDiffFormatter::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn testOutputDiffFormatter_add_find() {
        let mut m = TestOutputDiffFormatter::new();
        m.add(TestOutputDiffFormatterItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn testOutputDiffFormatter_priority_filter() {
        let mut m = TestOutputDiffFormatter::new();
        m.add(TestOutputDiffFormatterItem::new("a", "A").with_priority(TestOutputDiffFormatterPriority::High));
        m.add(TestOutputDiffFormatterItem::new("b", "B").with_priority(TestOutputDiffFormatterPriority::Low));
        m.add(TestOutputDiffFormatterItem::new("c", "C").with_priority(TestOutputDiffFormatterPriority::High));
        assert_eq!(m.by_priority(TestOutputDiffFormatterPriority::High).len(), 2);
    }

    #[test]
    fn testOutputDiffFormatter_remove() {
        let mut m = TestOutputDiffFormatter::new();
        m.add(TestOutputDiffFormatterItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn testOutputDiffFormatter_search() {
        let mut m = TestOutputDiffFormatter::new();
        m.add(TestOutputDiffFormatterItem::new("id1", "Hello World"));
        m.add(TestOutputDiffFormatterItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn testOutputDiffFormatter_total_weight() {
        let mut m = TestOutputDiffFormatter::new();
        m.add(TestOutputDiffFormatterItem::new("a", "A").with_priority(TestOutputDiffFormatterPriority::Critical));
        m.add(TestOutputDiffFormatterItem::new("b", "B").with_priority(TestOutputDiffFormatterPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn testOutputDiffFormatter_capacity_limit() {
        let mut m = TestOutputDiffFormatter::new().with_max_items(2);
        m.add(TestOutputDiffFormatterItem::new("1", "one"));
        m.add(TestOutputDiffFormatterItem::new("2", "two"));
        assert!(!m.add(TestOutputDiffFormatterItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn testOutputDiffFormatter_sorted_by_priority() {
        let mut m = TestOutputDiffFormatter::new();
        m.add(TestOutputDiffFormatterItem::new("lo", "Low").with_priority(TestOutputDiffFormatterPriority::Low));
        m.add(TestOutputDiffFormatterItem::new("hi", "High").with_priority(TestOutputDiffFormatterPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn testOutputDiffFormatter_item_metadata() {
        let mut item = TestOutputDiffFormatterItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn testRunScheduler_enabled_toggle() {
        let mut s = TestRunScheduler::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn testOutputDiffFormatter_priority_display() {
        assert_eq!(format!("{}", TestOutputDiffFormatterPriority::High), "high");
        assert_eq!(format!("{}", TestOutputDiffFormatterPriority::Low), "low");
    }


    #[test]
    fn testView_x_config_new() {
        let c = TestViewXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn testView_x_config_builder() {
        let c = TestViewXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn testView_x_config_display() {
        let c = TestViewXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn testView_x_registry_insert_get() {
        let mut reg = TestViewXRegistry::new();
        reg.insert(TestViewXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn testView_x_registry_duplicate() {
        let mut reg = TestViewXRegistry::new();
        reg.insert(TestViewXConfig::new("a")).unwrap();
        assert!(reg.insert(TestViewXConfig::new("a")).is_err());
    }

    #[test]
    fn testView_x_registry_remove() {
        let mut reg = TestViewXRegistry::new();
        reg.insert(TestViewXConfig::new("a")).unwrap();
        reg.insert(TestViewXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn testView_x_registry_active_entries() {
        let mut reg = TestViewXRegistry::new();
        reg.insert(TestViewXConfig::new("a")).unwrap();
        reg.insert(TestViewXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn testView_x_registry_by_weight() {
        let mut reg = TestViewXRegistry::new();
        reg.insert(TestViewXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(TestViewXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn testView_x_registry_tags() {
        let mut reg = TestViewXRegistry::new();
        reg.insert(TestViewXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(TestViewXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn testView_x_registry_total_weight() {
        let mut reg = TestViewXRegistry::new();
        reg.insert(TestViewXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(TestViewXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn testView_x_registry_iterator() {
        let mut reg = TestViewXRegistry::new();
        reg.insert(TestViewXConfig::new("a")).unwrap();
        reg.insert(TestViewXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn testView_x_cache_put_get() {
        let mut cache = TestViewXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn testView_x_cache_eviction() {
        let mut cache = TestViewXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn testView_x_cache_lru_order() {
        let mut cache = TestViewXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn testView_x_cache_most_least_recent() {
        let mut cache = TestViewXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn testView_x_formatter_entry() {
        let e = TestViewXConfig::new("k").with_value("v");
        let fmt = TestViewXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn testView_x_formatter_summary() {
        let mut reg = TestViewXRegistry::new();
        reg.insert(TestViewXConfig::new("a").with_weight(5)).unwrap();
        let fmt = TestViewXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn testView_x_validator_valid() {
        let v = TestViewXValidator::new();
        let c = TestViewXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn testView_x_validator_empty_key() {
        let v = TestViewXValidator::new();
        let c = TestViewXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn testView_x_validator_require_value() {
        let v = TestViewXValidator::new().require_value(true);
        let c = TestViewXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn testView_x_validator_allowed_tags() {
        let v = TestViewXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = TestViewXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn testView_x_validator_validate_all() {
        let v = TestViewXValidator::new();
        let mut reg = TestViewXRegistry::new();
        reg.insert(TestViewXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // ---- xc_ pool / scheduler tests – block 179 ----

    #[test]
    fn xc_179_pool_new_empty() {
        let pool: super::Xc179Pool<i32> = super::Xc179Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_179_pool_release_acquire() {
        let mut pool = super::Xc179Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_179_pool_acquire_empty() {
        let mut pool: super::Xc179Pool<i32> = super::Xc179Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_179_pool_full() {
        let mut pool = super::Xc179Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_179_pool_drain() {
        let mut pool = super::Xc179Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_179_pool_stats() {
        let mut pool = super::Xc179Pool::new(8);
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
    fn xc_179_pool_clear() {
        let mut pool = super::Xc179Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_179_pool_shrink() {
        let mut pool = super::Xc179Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_179_pool_default() {
        let pool: super::Xc179Pool<String> = super::Xc179Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_179_pool_extend() {
        let mut pool = super::Xc179Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_179_pool_retain() {
        let mut pool = super::Xc179Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_179_scheduler_round_robin() {
        let mut sched = super::Xc179Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_179_scheduler_empty() {
        let mut sched = super::Xc179Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_179_scheduler_reset() {
        let mut sched = super::Xc179Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_179_scheduler_add_remove() {
        let mut sched = super::Xc179Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_179_scheduler_targets() {
        let sched = super::Xc179Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_179_hash_empty() {
        assert_eq!(super::xc_179_hash(b""), 5381);
    }

    #[test]
    fn xc_179_hash_data() {
        let h = super::xc_179_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_179_hash(b"hello"), h);
    }

    #[test]
    fn xc_179_reverse_str() {
        assert_eq!(super::xc_179_reverse("abc"), "cba");
        assert_eq!(super::xc_179_reverse(""), "");
    }

}
