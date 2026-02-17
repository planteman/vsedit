//! Ext API: Testing.
//!
//! RPC bridge between the extension host and the main thread for the test API.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_testing";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TestMessage {
    RegisterController {
        id: String,
        label: String,
    },
    UnregisterController {
        id: String,
    },
    AddTestItem {
        controller_id: String,
        item: TestItem,
    },
    StartRun {
        controller_id: String,
        test_ids: Vec<String>,
    },
    ReportResult {
        run_id: String,
        test_id: String,
        result: TestResult,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestItem {
    pub id: String,
    pub label: String,
    pub uri: Option<String>,
    pub range_start_line: Option<u32>,
    pub children: Vec<TestItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestRun {
    pub id: String,
    pub controller_id: String,
    pub is_running: bool,
    pub results: Vec<(String, TestResult)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum TestResult {
    Passed,
    Failed { message: String },
    Skipped,
    Errored { message: String },
}

// ── Errors ──

/// Errors that can occur during test bridge operations.
#[derive(Debug, Clone, PartialEq)]
pub enum TestError {
    ControllerNotFound(String),
    ControllerAlreadyExists(String),
    RunNotFound(String),
    InvalidTestItem(String),
    RunAlreadyFinished(String),
}

impl std::fmt::Display for TestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ControllerNotFound(id) => write!(f, "controller not found: {id}"),
            Self::ControllerAlreadyExists(id) => write!(f, "controller already exists: {id}"),
            Self::RunNotFound(id) => write!(f, "run not found: {id}"),
            Self::InvalidTestItem(reason) => write!(f, "invalid test item: {reason}"),
            Self::RunAlreadyFinished(id) => write!(f, "run already finished: {id}"),
        }
    }
}

impl std::error::Error for TestError {}

// ── TestItem helpers ──

impl TestItem {
    /// Create a new leaf test item with no children.
    pub fn leaf(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            uri: None,
            range_start_line: None,
            children: Vec::new(),
        }
    }

    /// Count this item plus all descendants.
    pub fn total_count(&self) -> usize {
        1 + self.children.iter().map(|c| c.total_count()).sum::<usize>()
    }

    /// Find a descendant (or self) by id.
    pub fn find_by_id(&self, target: &str) -> Option<&TestItem> {
        if self.id == target {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find_by_id(target))
    }

    /// Collect all leaf item ids (items with no children).
    pub fn leaf_ids(&self) -> Vec<&str> {
        if self.children.is_empty() {
            vec![&self.id]
        } else {
            self.children.iter().flat_map(|c| c.leaf_ids()).collect()
        }
    }

    /// Validate that the item has a non-empty id and label.
    pub fn validate(&self) -> Result<(), TestError> {
        if self.id.is_empty() {
            return Err(TestError::InvalidTestItem("id must not be empty".into()));
        }
        if self.label.is_empty() {
            return Err(TestError::InvalidTestItem("label must not be empty".into()));
        }
        for child in &self.children {
            child.validate()?;
        }
        Ok(())
    }
}

/// Builder for constructing a `TestItem` incrementally.
#[derive(Debug, Clone)]
pub struct TestItemBuilder {
    id: String,
    label: String,
    uri: Option<String>,
    range_start_line: Option<u32>,
    children: Vec<TestItem>,
}

impl TestItemBuilder {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            uri: None,
            range_start_line: None,
            children: Vec::new(),
        }
    }

    pub fn uri(mut self, uri: impl Into<String>) -> Self {
        self.uri = Some(uri.into());
        self
    }

    pub fn range_start_line(mut self, line: u32) -> Self {
        self.range_start_line = Some(line);
        self
    }

    pub fn child(mut self, child: TestItem) -> Self {
        self.children.push(child);
        self
    }

    pub fn build(self) -> Result<TestItem, TestError> {
        let item = TestItem {
            id: self.id,
            label: self.label,
            uri: self.uri,
            range_start_line: self.range_start_line,
            children: self.children,
        };
        item.validate()?;
        Ok(item)
    }
}

// ── TestRun helpers ──

impl TestRun {
    /// Count results with a given state.
    pub fn count_passed(&self) -> usize {
        self.results.iter().filter(|(_, r)| matches!(r, TestResult::Passed)).count()
    }

    pub fn count_failed(&self) -> usize {
        self.results
            .iter()
            .filter(|(_, r)| matches!(r, TestResult::Failed { .. }))
            .count()
    }

    pub fn count_skipped(&self) -> usize {
        self.results.iter().filter(|(_, r)| matches!(r, TestResult::Skipped)).count()
    }

    pub fn count_errored(&self) -> usize {
        self.results
            .iter()
            .filter(|(_, r)| matches!(r, TestResult::Errored { .. }))
            .count()
    }

    /// Returns `true` if any result is `Failed` or `Errored`.
    pub fn has_failures(&self) -> bool {
        self.results
            .iter()
            .any(|(_, r)| matches!(r, TestResult::Failed { .. } | TestResult::Errored { .. }))
    }

    /// A summary string, e.g. "3 passed, 1 failed, 0 skipped, 0 errored".
    pub fn summary(&self) -> String {
        format!(
            "{} passed, {} failed, {} skipped, {} errored",
            self.count_passed(),
            self.count_failed(),
            self.count_skipped(),
            self.count_errored(),
        )
    }
}

impl std::fmt::Display for TestRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TestRun({}, controller={}, running={}, {})",
            self.id,
            self.controller_id,
            self.is_running,
            self.summary(),
        )
    }
}

impl std::fmt::Display for TestResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Passed => write!(f, "Passed"),
            Self::Failed { message } => write!(f, "Failed: {message}"),
            Self::Skipped => write!(f, "Skipped"),
            Self::Errored { message } => write!(f, "Errored: {message}"),
        }
    }
}

impl TestResult {
    /// Returns `true` if the result represents a successful outcome.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Passed | Self::Skipped)
    }
}

// ── Bridge ──

pub struct TestBridge {
    controllers: Vec<(String, String)>,
    items: Vec<(String, TestItem)>,
    runs: Vec<TestRun>,
    next_run_id: u64,
}

impl TestBridge {
    pub fn new() -> Self {
        Self {
            controllers: Vec::new(),
            items: Vec::new(),
            runs: Vec::new(),
            next_run_id: 1,
        }
    }

    pub fn register_controller(&mut self, id: &str, label: &str) {
        if !self.controllers.iter().any(|(cid, _)| cid == id) {
            self.controllers.push((id.to_string(), label.to_string()));
        }
    }

    pub fn unregister_controller(&mut self, id: &str) {
        self.controllers.retain(|(cid, _)| cid != id);
        self.items.retain(|(cid, _)| cid != id);
    }

    pub fn add_item(&mut self, controller_id: &str, item: TestItem) {
        self.items.push((controller_id.to_string(), item));
    }

    pub fn start_run(&mut self, controller_id: &str) -> String {
        let id = format!("run-{}", self.next_run_id);
        self.next_run_id += 1;
        self.runs.push(TestRun {
            id: id.clone(),
            controller_id: controller_id.to_string(),
            is_running: true,
            results: Vec::new(),
        });
        id
    }

    pub fn get_run(&self, id: &str) -> Option<&TestRun> {
        self.runs.iter().find(|r| r.id == id)
    }

    /// Finish a run, marking it as no longer running.
    pub fn finish_run(&mut self, run_id: &str) -> Result<(), TestError> {
        let run = self
            .runs
            .iter_mut()
            .find(|r| r.id == run_id)
            .ok_or_else(|| TestError::RunNotFound(run_id.to_string()))?;
        if !run.is_running {
            return Err(TestError::RunAlreadyFinished(run_id.to_string()));
        }
        run.is_running = false;
        Ok(())
    }

    /// Report a result into a run, returning an error if the run is missing or finished.
    pub fn report_result(
        &mut self,
        run_id: &str,
        test_id: &str,
        result: TestResult,
    ) -> Result<(), TestError> {
        let run = self
            .runs
            .iter_mut()
            .find(|r| r.id == run_id)
            .ok_or_else(|| TestError::RunNotFound(run_id.to_string()))?;
        if !run.is_running {
            return Err(TestError::RunAlreadyFinished(run_id.to_string()));
        }
        run.results.push((test_id.to_string(), result));
        Ok(())
    }

    /// Get items belonging to a specific controller.
    pub fn items_for_controller(&self, controller_id: &str) -> Vec<&TestItem> {
        self.items
            .iter()
            .filter(|(cid, _)| cid == controller_id)
            .map(|(_, item)| item)
            .collect()
    }

    /// Returns the number of registered controllers.
    pub fn controller_count(&self) -> usize {
        self.controllers.len()
    }

    /// Look up a controller label by id.
    pub fn controller_label(&self, id: &str) -> Option<&str> {
        self.controllers
            .iter()
            .find(|(cid, _)| cid == id)
            .map(|(_, label)| label.as_str())
    }

    pub fn handle_message(&mut self, msg: &TestMessage) -> serde_json::Value {
        match msg {
            TestMessage::RegisterController { id, label } => {
                self.register_controller(id, label);
                serde_json::json!({"registered": true})
            }
            TestMessage::UnregisterController { id } => {
                self.unregister_controller(id);
                serde_json::json!({"unregistered": true})
            }
            TestMessage::AddTestItem {
                controller_id,
                item,
            } => {
                self.add_item(controller_id, item.clone());
                serde_json::json!({"added": true})
            }
            TestMessage::StartRun {
                controller_id,
                test_ids,
            } => {
                let run_id = self.start_run(controller_id);
                serde_json::json!({"runId": run_id, "testCount": test_ids.len()})
            }
            TestMessage::ReportResult {
                run_id,
                test_id,
                result,
            } => {
                if let Some(run) = self.runs.iter_mut().find(|r| r.id == *run_id) {
                    run.results.push((test_id.clone(), result.clone()));
                    serde_json::json!({"recorded": true})
                } else {
                    serde_json::json!({"error": "run not found"})
                }
            }
        }
    }
}

impl Default for TestBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the testing extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

// ── VS Code Testing API ──

/// A tag that can be associated with test items and run profiles.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TestTag {
    pub id: String,
}

impl TestTag {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

/// The execution state of a test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestState {
    Queued,
    Running,
    Passed,
    Failed,
    Skipped,
    Errored,
}

impl TestState {
    /// Returns a single-char icon for the state.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Queued => "○",
            Self::Running => "◉",
            Self::Passed => "✓",
            Self::Failed => "✗",
            Self::Skipped => "⊘",
            Self::Errored => "✗",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Running => "Running",
            Self::Passed => "Passed",
            Self::Failed => "Failed",
            Self::Skipped => "Skipped",
            Self::Errored => "Errored",
        }
    }
}

/// A VS Code-compatible test item with full metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VscTestItem {
    pub id: String,
    pub uri: Option<String>,
    pub label: String,
    pub description: Option<String>,
    pub range: Option<(u32, u32, u32, u32)>,
    pub children: Vec<VscTestItem>,
    pub tags: Vec<TestTag>,
    pub can_resolve_children: bool,
    pub busy: bool,
    pub error: Option<String>,
}

impl VscTestItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            uri: None,
            label: label.into(),
            description: None,
            range: None,
            children: Vec::new(),
            tags: Vec::new(),
            can_resolve_children: false,
            busy: false,
            error: None,
        }
    }

    /// Add a child item.
    pub fn add_child(&mut self, child: VscTestItem) {
        self.children.push(child);
    }

    /// Add a tag.
    pub fn add_tag(&mut self, tag: TestTag) {
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }

    /// Check if this item has a given tag.
    pub fn has_tag(&self, tag_id: &str) -> bool {
        self.tags.iter().any(|t| t.id == tag_id)
    }
}

/// A managed collection of test items, supporting add/delete/get/replace/forEach.
#[derive(Debug, Clone, Default)]
pub struct TestItemCollection {
    items: Vec<VscTestItem>,
}

impl TestItemCollection {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn add(&mut self, item: VscTestItem) {
        // Replace if same id exists
        self.items.retain(|i| i.id != item.id);
        self.items.push(item);
    }

    pub fn delete(&mut self, id: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|i| i.id != id);
        self.items.len() < before
    }

    pub fn get(&self, id: &str) -> Option<&VscTestItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn replace(&mut self, items: Vec<VscTestItem>) {
        self.items = items;
    }

    pub fn for_each(&self, mut callback: impl FnMut(&VscTestItem)) {
        for item in &self.items {
            callback(item);
        }
    }

    pub fn size(&self) -> usize {
        self.items.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &VscTestItem> {
        self.items.iter()
    }
}

/// The kind of test run profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestRunProfileKind {
    Run,
    Debug,
    Coverage,
}

/// A configuration profile for running tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRunProfile {
    pub label: String,
    pub kind: TestRunProfileKind,
    pub is_default: bool,
    pub tag: Option<TestTag>,
    pub supports_continuous_run: bool,
}

impl TestRunProfile {
    pub fn new(label: impl Into<String>, kind: TestRunProfileKind) -> Self {
        Self {
            label: label.into(),
            kind,
            is_default: false,
            tag: None,
            supports_continuous_run: false,
        }
    }
}

/// A request to execute tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRunRequest {
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
    pub profile: Option<TestRunProfile>,
}

impl TestRunRequest {
    pub fn new() -> Self {
        Self {
            include: None,
            exclude: None,
            profile: None,
        }
    }

    pub fn with_include(mut self, ids: Vec<String>) -> Self {
        self.include = Some(ids);
        self
    }

    pub fn with_exclude(mut self, ids: Vec<String>) -> Self {
        self.exclude = Some(ids);
        self
    }

    pub fn with_profile(mut self, profile: TestRunProfile) -> Self {
        self.profile = Some(profile);
        self
    }
}

impl Default for TestRunRequest {
    fn default() -> Self {
        Self::new()
    }
}

/// A message produced during test execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestOutputMessage {
    pub message: String,
    pub expected_output: Option<String>,
    pub actual_output: Option<String>,
    pub location: Option<TestLocation>,
}

/// A source location.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestLocation {
    pub uri: String,
    pub line: u32,
    pub column: Option<u32>,
}

/// The result of a single test item in a run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestRunResult {
    pub item_id: String,
    pub state: TestState,
    pub duration_ms: Option<f64>,
    pub messages: Vec<TestOutputMessage>,
}

/// A VS Code-style test run.
#[derive(Debug, Clone)]
pub struct VscTestRun {
    pub name: Option<String>,
    pub is_cancelled: bool,
    pub results: Vec<TestRunResult>,
}

impl VscTestRun {
    pub fn new(name: Option<String>) -> Self {
        Self {
            name,
            is_cancelled: false,
            results: Vec::new(),
        }
    }

    pub fn cancel(&mut self) {
        self.is_cancelled = true;
    }

    pub fn record(&mut self, result: TestRunResult) {
        self.results.push(result);
    }
}

// ── Test Controller ──

/// A test controller that manages test items and run profiles.
#[derive(Debug)]
pub struct TestController {
    pub id: String,
    pub label: String,
    pub profiles: Vec<TestRunProfile>,
    pub items: TestItemCollection,
    runs: Vec<VscTestRun>,
    next_run_id: u64,
}

impl TestController {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            profiles: Vec::new(),
            items: TestItemCollection::new(),
            runs: Vec::new(),
            next_run_id: 1,
        }
    }

    /// Register a run profile.
    pub fn create_run_profile(
        &mut self,
        label: impl Into<String>,
        kind: TestRunProfileKind,
    ) -> &TestRunProfile {
        let profile = TestRunProfile::new(label, kind);
        self.profiles.push(profile);
        self.profiles.last().unwrap()
    }

    /// Start a new test run.
    pub fn create_test_run(&mut self, request: &TestRunRequest) -> usize {
        let run = VscTestRun::new(request.profile.as_ref().map(|p| p.label.clone()));
        self.runs.push(run);
        let idx = self.runs.len() - 1;
        self.next_run_id += 1;
        idx
    }

    /// Get a test run by index.
    pub fn get_run(&self, idx: usize) -> Option<&VscTestRun> {
        self.runs.get(idx)
    }

    /// Get a mutable test run by index.
    pub fn get_run_mut(&mut self, idx: usize) -> Option<&mut VscTestRun> {
        self.runs.get_mut(idx)
    }

    /// Resolve children of a test item (sets busy flag, calls resolver).
    pub fn resolve_children(&mut self, item_id: &str) {
        if let Some(item) = self.items.items.iter_mut().find(|i| i.id == item_id) {
            item.busy = true;
            // In a real implementation, this would invoke the registered handler
            item.busy = false;
            item.can_resolve_children = false;
        }
    }

    pub fn run_count(&self) -> usize {
        self.runs.len()
    }
}

// ── Test Discovery ──

/// Supported test frameworks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestFramework {
    CargoTest,
    Jest,
    Pytest,
    GoTest,
    Unknown,
}

impl TestFramework {
    pub fn label(&self) -> &'static str {
        match self {
            Self::CargoTest => "cargo test",
            Self::Jest => "jest",
            Self::Pytest => "pytest",
            Self::GoTest => "go test",
            Self::Unknown => "unknown",
        }
    }
}

/// Detect the test framework for a workspace path.
pub fn detect_test_framework(workspace: &Path) -> TestFramework {
    if workspace.join("Cargo.toml").exists() {
        TestFramework::CargoTest
    } else if workspace.join("package.json").exists() {
        if workspace.join("jest.config.js").exists()
            || workspace.join("jest.config.ts").exists()
        {
            TestFramework::Jest
        } else {
            TestFramework::Unknown
        }
    } else if workspace.join("pytest.ini").exists()
        || workspace.join("setup.py").exists()
        || workspace.join("pyproject.toml").exists()
    {
        TestFramework::Pytest
    } else if workspace.join("go.mod").exists() {
        TestFramework::GoTest
    } else {
        TestFramework::Unknown
    }
}

/// Parse `cargo test --list` output into test items.
pub struct CargoTestDiscoverer;

impl CargoTestDiscoverer {
    /// Parse the output of `cargo test -- --list` into `VscTestItem` entries.
    pub fn parse_test_list(output: &str) -> Vec<VscTestItem> {
        let mut items: Vec<VscTestItem> = Vec::new();
        // Group tests by module path
        let mut modules: HashMap<String, Vec<VscTestItem>> = HashMap::new();

        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() || !line.ends_with(": test") {
                continue;
            }
            let name = line.trim_end_matches(": test").trim();
            if name.is_empty() {
                continue;
            }

            // Split into module path and test name
            if let Some(pos) = name.rfind("::") {
                let module = &name[..pos];
                let test_name = &name[pos + 2..];
                let item = VscTestItem::new(name, test_name);
                modules.entry(module.to_string()).or_default().push(item);
            } else {
                items.push(VscTestItem::new(name, name));
            }
        }

        // Build module hierarchy
        for (module_path, tests) in modules {
            let mut module_item = VscTestItem::new(&module_path, &module_path);
            for test in tests {
                module_item.add_child(test);
            }
            items.push(module_item);
        }

        items
    }
}

/// Discover tests for a given framework by running the appropriate command.
pub fn discover_tests(framework: TestFramework, path: &Path) -> Vec<VscTestItem> {
    match framework {
        TestFramework::CargoTest => {
            let output = std::process::Command::new("cargo")
                .args(["test", "--", "--list"])
                .current_dir(path)
                .output();
            match output {
                Ok(out) if out.status.success() => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    CargoTestDiscoverer::parse_test_list(&stdout)
                }
                _ => Vec::new(),
            }
        }
        TestFramework::Jest => {
            // TODO: Implement Jest discovery (run `npx jest --listTests`)
            Vec::new()
        }
        TestFramework::Pytest => {
            // TODO: Implement Pytest discovery (run `pytest --collect-only -q`)
            Vec::new()
        }
        TestFramework::GoTest => {
            // TODO: Implement Go test discovery (run `go test -list .`)
            Vec::new()
        }
        TestFramework::Unknown => Vec::new(),
    }
}

/// Parse pre-existing command output for the given framework without spawning a process.
pub fn discover_tests_from_output(framework: TestFramework, output: &str) -> Vec<VscTestItem> {
    match framework {
        TestFramework::CargoTest => CargoTestDiscoverer::parse_test_list(output),
        _ => Vec::new(),
    }
}

// ── Test Result Aggregation ──

/// Summary statistics for a set of test results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestResultSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub errored: usize,
    pub duration_ms: f64,
}

impl TestResultSummary {
    pub fn is_success(&self) -> bool {
        self.failed == 0 && self.errored == 0
    }
}

/// Compute a summary from a slice of test results.
pub fn compute_summary(results: &[TestRunResult]) -> TestResultSummary {
    let mut summary = TestResultSummary {
        total: results.len(),
        passed: 0,
        failed: 0,
        skipped: 0,
        errored: 0,
        duration_ms: 0.0,
    };
    for r in results {
        match r.state {
            TestState::Passed => summary.passed += 1,
            TestState::Failed => summary.failed += 1,
            TestState::Skipped => summary.skipped += 1,
            TestState::Errored => summary.errored += 1,
            TestState::Queued | TestState::Running => {}
        }
        if let Some(d) = r.duration_ms {
            summary.duration_ms += d;
        }
    }
    summary
}

/// Keeps the last N test runs for comparison.
#[derive(Debug, Clone)]
pub struct TestRunHistory {
    capacity: usize,
    summaries: Vec<TestResultSummary>,
}

impl TestRunHistory {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            summaries: Vec::new(),
        }
    }

    pub fn push(&mut self, summary: TestResultSummary) {
        if self.summaries.len() >= self.capacity {
            self.summaries.remove(0);
        }
        self.summaries.push(summary);
    }

    pub fn latest(&self) -> Option<&TestResultSummary> {
        self.summaries.last()
    }

    pub fn all(&self) -> &[TestResultSummary] {
        &self.summaries
    }

    pub fn len(&self) -> usize {
        self.summaries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.summaries.is_empty()
    }

    pub fn clear(&mut self) {
        self.summaries.clear();
    }
}

// ── Coverage Support ──

/// Coverage information for a single file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileCoverage {
    pub uri: String,
    pub statement_coverage: CoverageStats,
    pub branch_coverage: Option<CoverageStats>,
    pub function_coverage: Option<CoverageStats>,
}

/// Coverage statistics as covered/total.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoverageStats {
    pub covered: usize,
    pub total: usize,
}

impl CoverageStats {
    pub fn new(covered: usize, total: usize) -> Self {
        Self { covered, total }
    }

    pub fn percentage(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.covered as f64 / self.total as f64 * 100.0
        }
    }
}

/// Detailed per-line coverage info.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetailedCoverage {
    pub line_number: u32,
    pub executed_count: u32,
    pub is_covered: bool,
}

/// Trait for coverage providers.
pub trait CoverageProvider {
    fn provide_file_coverage(&self) -> Vec<FileCoverage>;
}

// ── Test View Rendering Helpers ──

/// Format a test item tree for terminal display.
pub fn render_test_tree(items: &[VscTestItem], indent: usize) -> String {
    let mut out = String::new();
    let prefix = "  ".repeat(indent);
    for item in items {
        out.push_str(&format!("{}{}\n", prefix, item.label));
        if !item.children.is_empty() {
            out.push_str(&render_test_tree(&item.children, indent + 1));
        }
    }
    out
}

/// Format a test result with state icon for display.
pub fn render_result_line(result: &TestRunResult) -> String {
    let icon = result.state.icon();
    let duration = result
        .duration_ms
        .map(|d| format!(" ({:.0}ms)", d))
        .unwrap_or_default();
    format!("{icon} {}{duration}", result.item_id)
}

/// Accumulated statistics for ext-testing operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtTestingStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtTestingStats {
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
    pub fn merge(&mut self, other: &ExtTestingStats) {
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

impl Default for ExtTestingStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtTestingStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtTestingStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-testing.
#[derive(Debug, Clone)]
pub struct ExtTestingValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtTestingValidator {
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

impl Default for ExtTestingValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// TestItemNameFilter
// ---------------------------------------------------------------------------

pub struct TestItemNameFilter {
    name_pattern: Option<String>,
    tags: Vec<String>,
}

impl TestItemNameFilter {
    pub fn new() -> Self { Self { name_pattern: None, tags: Vec::new() } }

    pub fn with_name(mut self, pattern: impl Into<String>) -> Self {
        self.name_pattern = Some(pattern.into()); self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into()); self
    }

    pub fn matches(&self, item: &TestItem) -> bool {
        if let Some(ref pat) = self.name_pattern {
            if !item.label.to_lowercase().contains(&pat.to_lowercase()) { return false; }
        }
        true
    }

    pub fn filter_items<'a>(&self, items: &'a [TestItem]) -> Vec<&'a TestItem> {
        items.iter().filter(|i| self.matches(i)).collect()
    }
}

impl Default for TestItemNameFilter { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// TestRunOutputCapture
// ---------------------------------------------------------------------------

pub struct TestRunOutputCapture {
    raw_output: String,
    stripped_output: String,
}

impl TestRunOutputCapture {
    pub fn new() -> Self { Self { raw_output: String::new(), stripped_output: String::new() } }

    pub fn append(&mut self, text: &str) {
        self.raw_output.push_str(text);
        self.stripped_output.push_str(&Self::strip_ansi(text));
    }

    pub fn raw(&self) -> &str { &self.raw_output }
    pub fn stripped(&self) -> &str { &self.stripped_output }

    pub fn strip_ansi(input: &str) -> String {
        let mut result = String::new();
        let mut in_escape = false;
        for ch in input.chars() {
            if ch == '\x1b' || ch == '\u{1b}' { in_escape = true; continue; }
            if in_escape {
                if ch.is_ascii_alphabetic() { in_escape = false; }
                continue;
            }
            result.push(ch);
        }
        result
    }

    pub fn line_count(&self) -> usize { self.stripped_output.lines().count() }
    pub fn clear(&mut self) { self.raw_output.clear(); self.stripped_output.clear(); }
}

impl Default for TestRunOutputCapture { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// TestCoverageMap
// ---------------------------------------------------------------------------

pub struct TestCoverageMap {
    entries: std::collections::HashMap<String, Vec<bool>>,
}

impl TestCoverageMap {
    pub fn new() -> Self { Self { entries: std::collections::HashMap::new() } }

    pub fn set_file_coverage(&mut self, file: impl Into<String>, line_coverage: Vec<bool>) {
        self.entries.insert(file.into(), line_coverage);
    }

    pub fn get_file_coverage(&self, file: &str) -> Option<&[bool]> {
        self.entries.get(file).map(|v| v.as_slice())
    }

    pub fn file_coverage_percent(&self, file: &str) -> Option<f64> {
        self.entries.get(file).map(|lines| {
            if lines.is_empty() { return 0.0; }
            let covered = lines.iter().filter(|&&b| b).count();
            covered as f64 / lines.len() as f64 * 100.0
        })
    }

    pub fn total_coverage_percent(&self) -> f64 {
        let mut total = 0usize;
        let mut covered = 0usize;
        for lines in self.entries.values() {
            total += lines.len();
            covered += lines.iter().filter(|&&b| b).count();
        }
        if total == 0 { 0.0 } else { covered as f64 / total as f64 * 100.0 }
    }

    pub fn file_count(&self) -> usize { self.entries.len() }
    pub fn uncovered_lines(&self, file: &str) -> Vec<usize> {
        self.entries.get(file).map(|lines| {
            lines.iter().enumerate().filter(|(_, b)| !**b).map(|(i, _)| i).collect()
        }).unwrap_or_default()
    }
}

impl Default for TestCoverageMap { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// TestDiffViewer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TestDiffLine {
    pub kind: TestDiffKind,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TestDiffKind { Same, Added, Removed }

pub struct TestDiffViewer;

impl TestDiffViewer {
    pub fn diff(expected: &str, actual: &str) -> Vec<TestDiffLine> {
        let exp_lines: Vec<&str> = expected.lines().collect();
        let act_lines: Vec<&str> = actual.lines().collect();
        let mut result = Vec::new();
        let max = exp_lines.len().max(act_lines.len());
        for i in 0..max {
            match (exp_lines.get(i), act_lines.get(i)) {
                (Some(e), Some(a)) if e == a => {
                    result.push(TestDiffLine { kind: TestDiffKind::Same, content: e.to_string() });
                }
                (Some(e), Some(a)) => {
                    result.push(TestDiffLine { kind: TestDiffKind::Removed, content: e.to_string() });
                    result.push(TestDiffLine { kind: TestDiffKind::Added, content: a.to_string() });
                }
                (Some(e), None) => {
                    result.push(TestDiffLine { kind: TestDiffKind::Removed, content: e.to_string() });
                }
                (None, Some(a)) => {
                    result.push(TestDiffLine { kind: TestDiffKind::Added, content: a.to_string() });
                }
                _ => {}
            }
        }
        result
    }

    pub fn has_differences(expected: &str, actual: &str) -> bool {
        Self::diff(expected, actual).iter().any(|l| l.kind != TestDiffKind::Same)
    }
}

// ---------------------------------------------------------------------------
// TestResultDiffViewer - test result diff viewer
// ---------------------------------------------------------------------------

/// Severity level for test result diff viewer issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TestResultDiffViewerSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for TestResultDiffViewerSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [TestResultDiffViewer].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestResultDiffViewerEntry {
    pub id: String,
    pub label: String,
    pub severity: TestResultDiffViewerSeverity,
    pub detail: Option<String>,
    pub result_count: usize,
    enabled: bool,
}

impl TestResultDiffViewerEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: TestResultDiffViewerSeverity::Low,
            detail: None,
            result_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: TestResultDiffViewerSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_result_count(mut self, val: usize) -> Self {
        self.result_count = val;
        self
    }

    pub fn has_failures(&self) -> bool {
        self.enabled && self.severity >= TestResultDiffViewerSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.result_count, det)
    }
}

impl fmt::Display for TestResultDiffViewerEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [TestResultDiffViewerEntry] items.
#[derive(Debug, Clone)]
pub struct TestResultDiffViewer {
    entries: Vec<TestResultDiffViewerEntry>,
    name: String,
    capacity: usize,
}

impl TestResultDiffViewer {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: TestResultDiffViewerEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<TestResultDiffViewerEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&TestResultDiffViewerEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn result_count(&self) -> usize { self.entries.len() }

    pub fn has_failures(&self) -> bool {
        self.entries.iter().any(|e| e.has_failures())
    }

    pub fn entries_by_severity(&self, severity: TestResultDiffViewerSeverity) -> Vec<&TestResultDiffViewerEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= TestResultDiffViewerSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&TestResultDiffViewerEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&TestResultDiffViewerEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// TestFilterExprParser - test filter expression parser
// ---------------------------------------------------------------------------

/// Configuration for [TestFilterExprParser].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestFilterExprParserConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub filter_depth: usize,
}

impl TestFilterExprParserConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, filter_depth: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_filter_depth(mut self, val: usize) -> Self { self.filter_depth = val; self }
}

impl Default for TestFilterExprParserConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [TestFilterExprParser].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestFilterExprParserItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl TestFilterExprParserItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn is_valid_filter(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for TestFilterExprParserItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [TestFilterExprParserItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct TestFilterExprParser {
    config: TestFilterExprParserConfig,
    items: Vec<TestFilterExprParserItem>,
}

impl TestFilterExprParser {
    pub fn new(config: TestFilterExprParserConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: TestFilterExprParserItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<TestFilterExprParserItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&TestFilterExprParserItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn filter_depth(&self) -> usize { self.items.len() }

    pub fn is_valid_filter(&self) -> bool {
        self.items.iter().any(|i| i.is_valid_filter())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&TestFilterExprParserItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&TestFilterExprParserItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &TestFilterExprParserConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



// ─── ExtTest Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for test results.
#[derive(Debug, Clone)]
pub struct ExtTestRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> ExtTestRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for ExtTestRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExtTestRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── ExtTest Builder & Validator ─────────────────────────────

/// Builder for constructing test runner configurations.
#[derive(Debug, Clone)]
pub struct ExtTestBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl ExtTestBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<ExtTestCfg, ExtTestBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(ExtTestBuildErr { errors }); }
        Ok(ExtTestCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated test runner configuration.
#[derive(Debug, Clone)]
pub struct ExtTestCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl ExtTestCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &ExtTestCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for ExtTestCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExtTestCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct ExtTestBuildErr { pub errors: Vec<String> }

impl fmt::Display for ExtTestBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExtTestBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for ExtTestBuildErr {}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_item() -> TestItem {
        TestItem {
            id: "t1".into(),
            label: "test_add".into(),
            uri: Some("file:///test.rs".into()),
            range_start_line: Some(10),
            children: vec![],
        }
    }

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = TestMessage::StartRun {
            controller_id: "rust".into(),
            test_ids: vec!["t1".into(), "t2".into()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: TestMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn test_result_serialization() {
        let r = TestResult::Failed {
            message: "assertion failed".into(),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: TestResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn bridge_controller_lifecycle() {
        let mut bridge = TestBridge::new();
        bridge.register_controller("rust", "Rust Tests");
        bridge.add_item("rust", test_item());
        assert_eq!(bridge.items.len(), 1);
        bridge.unregister_controller("rust");
        assert!(bridge.items.is_empty());
    }

    #[test]
    fn bridge_run_and_report() {
        let mut bridge = TestBridge::new();
        bridge.register_controller("rust", "Rust Tests");
        let run_id = bridge.start_run("rust");
        let msg = TestMessage::ReportResult {
            run_id: run_id.clone(),
            test_id: "t1".into(),
            result: TestResult::Passed,
        };
        bridge.handle_message(&msg);
        let run = bridge.get_run(&run_id).unwrap();
        assert_eq!(run.results.len(), 1);
    }

    #[test]
    fn bridge_report_unknown_run() {
        let mut bridge = TestBridge::new();
        let result = bridge.handle_message(&TestMessage::ReportResult {
            run_id: "nope".into(),
            test_id: "t1".into(),
            result: TestResult::Passed,
        });
        assert!(result.get("error").is_some());
    }

    #[test]
    fn test_item_leaf_constructor() {
        let item = TestItem::leaf("id1", "My Test");
        assert_eq!(item.id, "id1");
        assert_eq!(item.label, "My Test");
        assert!(item.uri.is_none());
        assert!(item.children.is_empty());
    }

    #[test]
    fn test_item_total_count() {
        let child1 = TestItem::leaf("c1", "child1");
        let child2 = TestItem::leaf("c2", "child2");
        let parent = TestItem {
            id: "p".into(),
            label: "parent".into(),
            uri: None,
            range_start_line: None,
            children: vec![child1, child2],
        };
        assert_eq!(parent.total_count(), 3);
    }

    #[test]
    fn test_item_find_by_id_nested() {
        let grandchild = TestItem::leaf("gc1", "grandchild");
        let child = TestItem {
            id: "c1".into(),
            label: "child".into(),
            uri: None,
            range_start_line: None,
            children: vec![grandchild],
        };
        let root = TestItem {
            id: "root".into(),
            label: "root".into(),
            uri: None,
            range_start_line: None,
            children: vec![child],
        };
        assert!(root.find_by_id("gc1").is_some());
        assert_eq!(root.find_by_id("gc1").unwrap().label, "grandchild");
        assert!(root.find_by_id("missing").is_none());
    }

    #[test]
    fn test_item_leaf_ids() {
        let c1 = TestItem::leaf("c1", "child1");
        let c2 = TestItem::leaf("c2", "child2");
        let parent = TestItem {
            id: "p".into(),
            label: "parent".into(),
            uri: None,
            range_start_line: None,
            children: vec![c1, c2],
        };
        let ids = parent.leaf_ids();
        assert_eq!(ids, vec!["c1", "c2"]);
    }

    #[test]
    fn test_item_validation() {
        let bad = TestItem::leaf("", "label");
        assert!(bad.validate().is_err());
        let bad2 = TestItem::leaf("id", "");
        assert!(bad2.validate().is_err());
        let good = TestItem::leaf("id", "label");
        assert!(good.validate().is_ok());
    }

    #[test]
    fn test_item_builder() {
        let item = TestItemBuilder::new("b1", "Built Test")
            .uri("file:///built.rs")
            .range_start_line(42)
            .child(TestItem::leaf("bc1", "built child"))
            .build()
            .unwrap();
        assert_eq!(item.id, "b1");
        assert_eq!(item.uri.as_deref(), Some("file:///built.rs"));
        assert_eq!(item.range_start_line, Some(42));
        assert_eq!(item.children.len(), 1);
    }

    #[test]
    fn test_item_builder_rejects_invalid() {
        let result = TestItemBuilder::new("", "label").build();
        assert!(result.is_err());
    }

    #[test]
    fn test_run_summary_and_display() {
        let run = TestRun {
            id: "run-99".into(),
            controller_id: "rust".into(),
            is_running: false,
            results: vec![
                ("t1".into(), TestResult::Passed),
                ("t2".into(), TestResult::Failed { message: "oops".into() }),
                ("t3".into(), TestResult::Skipped),
                ("t4".into(), TestResult::Errored { message: "boom".into() }),
            ],
        };
        assert_eq!(run.count_passed(), 1);
        assert_eq!(run.count_failed(), 1);
        assert_eq!(run.count_skipped(), 1);
        assert_eq!(run.count_errored(), 1);
        assert!(run.has_failures());
        assert_eq!(run.summary(), "1 passed, 1 failed, 1 skipped, 1 errored");
        let display = format!("{run}");
        assert!(display.contains("run-99"));
    }

    #[test]
    fn test_result_is_success() {
        assert!(TestResult::Passed.is_success());
        assert!(TestResult::Skipped.is_success());
        assert!(!TestResult::Failed { message: "x".into() }.is_success());
        assert!(!TestResult::Errored { message: "x".into() }.is_success());
    }

    #[test]
    fn bridge_finish_run() {
        let mut bridge = TestBridge::new();
        bridge.register_controller("r", "R");
        let run_id = bridge.start_run("r");
        bridge.report_result(&run_id, "t1", TestResult::Passed).unwrap();
        bridge.finish_run(&run_id).unwrap();
        assert!(!bridge.get_run(&run_id).unwrap().is_running);
        // Cannot report after finish
        assert!(bridge.report_result(&run_id, "t2", TestResult::Passed).is_err());
        // Cannot finish twice
        assert!(bridge.finish_run(&run_id).is_err());
    }

    #[test]
    fn bridge_items_for_controller() {
        let mut bridge = TestBridge::new();
        bridge.register_controller("a", "A");
        bridge.register_controller("b", "B");
        bridge.add_item("a", TestItem::leaf("a1", "item a1"));
        bridge.add_item("b", TestItem::leaf("b1", "item b1"));
        bridge.add_item("a", TestItem::leaf("a2", "item a2"));
        assert_eq!(bridge.items_for_controller("a").len(), 2);
        assert_eq!(bridge.items_for_controller("b").len(), 1);
        assert_eq!(bridge.items_for_controller("c").len(), 0);
    }

    #[test]
    fn bridge_controller_label_lookup() {
        let mut bridge = TestBridge::new();
        bridge.register_controller("go", "Go Tests");
        assert_eq!(bridge.controller_label("go"), Some("Go Tests"));
        assert_eq!(bridge.controller_label("missing"), None);
        assert_eq!(bridge.controller_count(), 1);
    }

    #[test]
    fn test_error_display() {
        let err = TestError::ControllerNotFound("x".into());
        assert_eq!(format!("{err}"), "controller not found: x");
        let err2 = TestError::InvalidTestItem("bad".into());
        assert!(format!("{err2}").contains("bad"));
    }

    #[test]
    fn ext_testing_stats_new_defaults() {
        let stats = ExtTestingStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_testing_stats_record_success() {
        let mut stats = ExtTestingStats::new();
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
    fn ext_testing_stats_record_failure() {
        let mut stats = ExtTestingStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_testing_stats_reset() {
        let mut stats = ExtTestingStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_testing_stats_merge() {
        let mut a = ExtTestingStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtTestingStats::new();
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
    fn ext_testing_stats_display() {
        let mut stats = ExtTestingStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_testing_stats_default() {
        let stats = ExtTestingStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_testing_validator_accepts_valid_name() {
        let v = ExtTestingValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_testing_validator_rejects_empty() {
        let v = ExtTestingValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_testing_validator_rejects_too_long() {
        let v = ExtTestingValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_testing_validator_forbidden_prefix() {
        let v = ExtTestingValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_testing_validator_allowed_chars() {
        let v = ExtTestingValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_testing_validator_range() {
        let v = ExtTestingValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_testing_sanitize_removes_control() {
        let result = ExtTestingValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_testing_truncate_short_string() {
        assert_eq!(ExtTestingValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_testing_truncate_long_string() {
        let result = ExtTestingValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_testing_is_ascii_printable() {
        assert!(ExtTestingValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtTestingValidator::is_ascii_printable("Hello\x00World"));
    }

    // ── VS Code Testing API Tests ──

    #[test]
    fn test_tag_new() {
        let tag = TestTag::new("slow");
        assert_eq!(tag.id, "slow");
    }

    #[test]
    fn test_tag_equality() {
        assert_eq!(TestTag::new("a"), TestTag::new("a"));
        assert_ne!(TestTag::new("a"), TestTag::new("b"));
    }

    #[test]
    fn test_state_icons() {
        assert_eq!(TestState::Passed.icon(), "✓");
        assert_eq!(TestState::Failed.icon(), "✗");
        assert_eq!(TestState::Skipped.icon(), "⊘");
        assert_eq!(TestState::Queued.icon(), "○");
        assert_eq!(TestState::Running.icon(), "◉");
        assert_eq!(TestState::Errored.icon(), "✗");
    }

    #[test]
    fn test_state_labels() {
        assert_eq!(TestState::Passed.label(), "Passed");
        assert_eq!(TestState::Failed.label(), "Failed");
        assert_eq!(TestState::Running.label(), "Running");
    }

    #[test]
    fn vsc_test_item_new() {
        let item = VscTestItem::new("t1", "my test");
        assert_eq!(item.id, "t1");
        assert_eq!(item.label, "my test");
        assert!(!item.busy);
        assert!(!item.can_resolve_children);
        assert!(item.children.is_empty());
        assert!(item.tags.is_empty());
    }

    #[test]
    fn vsc_test_item_add_child() {
        let mut parent = VscTestItem::new("p", "parent");
        parent.add_child(VscTestItem::new("c1", "child1"));
        parent.add_child(VscTestItem::new("c2", "child2"));
        assert_eq!(parent.children.len(), 2);
    }

    #[test]
    fn vsc_test_item_tags() {
        let mut item = VscTestItem::new("t1", "test");
        item.add_tag(TestTag::new("slow"));
        item.add_tag(TestTag::new("slow")); // duplicate ignored
        assert_eq!(item.tags.len(), 1);
        assert!(item.has_tag("slow"));
        assert!(!item.has_tag("fast"));
    }

    #[test]
    fn test_item_collection_add_get_delete() {
        let mut coll = TestItemCollection::new();
        coll.add(VscTestItem::new("a", "A"));
        coll.add(VscTestItem::new("b", "B"));
        assert_eq!(coll.size(), 2);
        assert!(coll.get("a").is_some());
        assert!(coll.get("c").is_none());
        assert!(coll.delete("a"));
        assert_eq!(coll.size(), 1);
        assert!(!coll.delete("nonexistent"));
    }

    #[test]
    fn test_item_collection_replace() {
        let mut coll = TestItemCollection::new();
        coll.add(VscTestItem::new("old", "Old"));
        coll.replace(vec![VscTestItem::new("new1", "New1"), VscTestItem::new("new2", "New2")]);
        assert_eq!(coll.size(), 2);
        assert!(coll.get("old").is_none());
        assert!(coll.get("new1").is_some());
    }

    #[test]
    fn test_item_collection_for_each() {
        let mut coll = TestItemCollection::new();
        coll.add(VscTestItem::new("a", "A"));
        coll.add(VscTestItem::new("b", "B"));
        let mut ids = Vec::new();
        coll.for_each(|item| ids.push(item.id.clone()));
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_item_collection_add_replaces_same_id() {
        let mut coll = TestItemCollection::new();
        coll.add(VscTestItem::new("a", "First"));
        coll.add(VscTestItem::new("a", "Second"));
        assert_eq!(coll.size(), 1);
        assert_eq!(coll.get("a").unwrap().label, "Second");
    }

    #[test]
    fn test_run_profile_new() {
        let profile = TestRunProfile::new("Run Tests", TestRunProfileKind::Run);
        assert_eq!(profile.label, "Run Tests");
        assert_eq!(profile.kind, TestRunProfileKind::Run);
        assert!(!profile.is_default);
        assert!(!profile.supports_continuous_run);
    }

    #[test]
    fn test_run_profile_kind_variants() {
        assert_ne!(TestRunProfileKind::Run, TestRunProfileKind::Debug);
        assert_ne!(TestRunProfileKind::Debug, TestRunProfileKind::Coverage);
        assert_eq!(TestRunProfileKind::Run, TestRunProfileKind::Run);
    }

    #[test]
    fn test_run_request_builder() {
        let req = TestRunRequest::new()
            .with_include(vec!["t1".into(), "t2".into()])
            .with_exclude(vec!["t3".into()])
            .with_profile(TestRunProfile::new("Run", TestRunProfileKind::Run));
        assert_eq!(req.include.as_ref().unwrap().len(), 2);
        assert_eq!(req.exclude.as_ref().unwrap().len(), 1);
        assert!(req.profile.is_some());
    }

    #[test]
    fn test_run_request_default() {
        let req = TestRunRequest::default();
        assert!(req.include.is_none());
        assert!(req.exclude.is_none());
        assert!(req.profile.is_none());
    }

    #[test]
    fn test_output_message() {
        let msg = TestOutputMessage {
            message: "assertion failed".into(),
            expected_output: Some("42".into()),
            actual_output: Some("43".into()),
            location: Some(TestLocation {
                uri: "file:///test.rs".into(),
                line: 10,
                column: Some(5),
            }),
        };
        assert_eq!(msg.expected_output.as_deref(), Some("42"));
        assert_eq!(msg.location.as_ref().unwrap().line, 10);
    }

    #[test]
    fn test_run_result() {
        let result = TestRunResult {
            item_id: "t1".into(),
            state: TestState::Passed,
            duration_ms: Some(12.5),
            messages: vec![],
        };
        assert_eq!(result.state, TestState::Passed);
        assert_eq!(result.duration_ms, Some(12.5));
    }

    #[test]
    fn vsc_test_run_lifecycle() {
        let mut run = VscTestRun::new(Some("Suite".into()));
        assert!(!run.is_cancelled);
        run.record(TestRunResult {
            item_id: "t1".into(),
            state: TestState::Passed,
            duration_ms: Some(5.0),
            messages: vec![],
        });
        assert_eq!(run.results.len(), 1);
        run.cancel();
        assert!(run.is_cancelled);
    }

    #[test]
    fn test_controller_lifecycle() {
        let mut ctrl = TestController::new("rust", "Rust Tests");
        assert_eq!(ctrl.id, "rust");
        ctrl.create_run_profile("Run", TestRunProfileKind::Run);
        ctrl.create_run_profile("Debug", TestRunProfileKind::Debug);
        assert_eq!(ctrl.profiles.len(), 2);

        ctrl.items.add(VscTestItem::new("t1", "test_one"));
        assert_eq!(ctrl.items.size(), 1);

        let req = TestRunRequest::new();
        let run_idx = ctrl.create_test_run(&req);
        assert_eq!(ctrl.run_count(), 1);
        assert!(ctrl.get_run(run_idx).is_some());
    }

    #[test]
    fn test_controller_resolve_children() {
        let mut ctrl = TestController::new("rust", "Rust Tests");
        let mut item = VscTestItem::new("t1", "test");
        item.can_resolve_children = true;
        ctrl.items.add(item);
        ctrl.resolve_children("t1");
        assert!(!ctrl.items.get("t1").unwrap().can_resolve_children);
    }

    #[test]
    fn test_framework_detection() {
        // Unknown for nonexistent path
        let fw = detect_test_framework(std::path::Path::new("/nonexistent/path"));
        assert_eq!(fw, TestFramework::Unknown);
    }

    #[test]
    fn test_framework_labels() {
        assert_eq!(TestFramework::CargoTest.label(), "cargo test");
        assert_eq!(TestFramework::Jest.label(), "jest");
        assert_eq!(TestFramework::Pytest.label(), "pytest");
        assert_eq!(TestFramework::GoTest.label(), "go test");
        assert_eq!(TestFramework::Unknown.label(), "unknown");
    }

    #[test]
    fn cargo_test_discoverer_parse() {
        let output = "\
tests::test_add: test
tests::test_sub: test
other_test: test
ignored_line
";
        let items = CargoTestDiscoverer::parse_test_list(output);
        // "tests" module has 2 children, plus "other_test" at root
        assert_eq!(items.len(), 2); // module "tests" + "other_test"
        let other = items.iter().find(|i| i.id == "other_test");
        assert!(other.is_some());
        let module = items.iter().find(|i| i.id == "tests");
        assert!(module.is_some());
        assert_eq!(module.unwrap().children.len(), 2);
    }

    #[test]
    fn cargo_test_discoverer_empty_output() {
        let items = CargoTestDiscoverer::parse_test_list("");
        assert!(items.is_empty());
    }

    #[test]
    fn compute_summary_basic() {
        let results = vec![
            TestRunResult { item_id: "t1".into(), state: TestState::Passed, duration_ms: Some(10.0), messages: vec![] },
            TestRunResult { item_id: "t2".into(), state: TestState::Failed, duration_ms: Some(20.0), messages: vec![] },
            TestRunResult { item_id: "t3".into(), state: TestState::Skipped, duration_ms: None, messages: vec![] },
            TestRunResult { item_id: "t4".into(), state: TestState::Errored, duration_ms: Some(5.0), messages: vec![] },
        ];
        let s = compute_summary(&results);
        assert_eq!(s.total, 4);
        assert_eq!(s.passed, 1);
        assert_eq!(s.failed, 1);
        assert_eq!(s.skipped, 1);
        assert_eq!(s.errored, 1);
        assert!((s.duration_ms - 35.0).abs() < f64::EPSILON);
        assert!(!s.is_success());
    }

    #[test]
    fn compute_summary_all_pass() {
        let results = vec![
            TestRunResult { item_id: "t1".into(), state: TestState::Passed, duration_ms: Some(1.0), messages: vec![] },
        ];
        let s = compute_summary(&results);
        assert!(s.is_success());
    }

    #[test]
    fn test_run_history() {
        let mut history = TestRunHistory::new(3);
        assert!(history.is_empty());
        history.push(TestResultSummary { total: 1, passed: 1, failed: 0, skipped: 0, errored: 0, duration_ms: 1.0 });
        history.push(TestResultSummary { total: 2, passed: 1, failed: 1, skipped: 0, errored: 0, duration_ms: 2.0 });
        history.push(TestResultSummary { total: 3, passed: 3, failed: 0, skipped: 0, errored: 0, duration_ms: 3.0 });
        assert_eq!(history.len(), 3);
        // Adding a 4th should drop the oldest
        history.push(TestResultSummary { total: 4, passed: 4, failed: 0, skipped: 0, errored: 0, duration_ms: 4.0 });
        assert_eq!(history.len(), 3);
        assert_eq!(history.all()[0].total, 2);
        assert_eq!(history.latest().unwrap().total, 4);
    }

    #[test]
    fn test_run_history_clear() {
        let mut history = TestRunHistory::new(5);
        history.push(TestResultSummary { total: 1, passed: 1, failed: 0, skipped: 0, errored: 0, duration_ms: 0.0 });
        history.clear();
        assert!(history.is_empty());
    }

    #[test]
    fn coverage_stats_percentage() {
        let stats = CoverageStats::new(75, 100);
        assert!((stats.percentage() - 75.0).abs() < f64::EPSILON);
        let zero = CoverageStats::new(0, 0);
        assert!((zero.percentage() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn file_coverage_creation() {
        let fc = FileCoverage {
            uri: "file:///src/main.rs".into(),
            statement_coverage: CoverageStats::new(80, 100),
            branch_coverage: Some(CoverageStats::new(40, 50)),
            function_coverage: None,
        };
        assert!((fc.statement_coverage.percentage() - 80.0).abs() < f64::EPSILON);
        assert!((fc.branch_coverage.as_ref().unwrap().percentage() - 80.0).abs() < f64::EPSILON);
    }

    #[test]
    fn detailed_coverage_fields() {
        let dc = DetailedCoverage { line_number: 42, executed_count: 3, is_covered: true };
        assert_eq!(dc.line_number, 42);
        assert!(dc.is_covered);
    }

    #[test]
    fn render_test_tree_output() {
        let mut parent = VscTestItem::new("p", "Parent");
        parent.add_child(VscTestItem::new("c1", "Child 1"));
        parent.add_child(VscTestItem::new("c2", "Child 2"));
        let output = render_test_tree(&[parent], 0);
        assert!(output.contains("Parent"));
        assert!(output.contains("  Child 1"));
        assert!(output.contains("  Child 2"));
    }

    #[test]
    fn render_result_line_output() {
        let result = TestRunResult {
            item_id: "my_test".into(),
            state: TestState::Passed,
            duration_ms: Some(42.0),
            messages: vec![],
        };
        let line = render_result_line(&result);
        assert!(line.contains("✓"));
        assert!(line.contains("my_test"));
        assert!(line.contains("42ms"));
    }

    #[test]
    fn render_result_line_no_duration() {
        let result = TestRunResult {
            item_id: "t1".into(),
            state: TestState::Failed,
            duration_ms: None,
            messages: vec![],
        };
        let line = render_result_line(&result);
        assert!(line.contains("✗"));
        assert!(!line.contains("ms"));
    }

    #[test]
    fn discover_tests_returns_empty_for_unknown() {
        let items = discover_tests(TestFramework::Unknown, std::path::Path::new("/tmp"));
        assert!(items.is_empty());
    }

    #[test]
    fn discover_tests_cargo_parses_output() {
        let output = "tests::test_one: test\ntests::test_two: test\nother_test: test\n";
        let items = discover_tests_from_output(TestFramework::CargoTest, output);
        // Should have module "tests" with 2 children, plus "other_test" at root
        assert!(items.len() >= 2); // at least module group + root test
        let module = items.iter().find(|i| i.id == "tests");
        assert!(module.is_some());
        assert_eq!(module.unwrap().children.len(), 2);
    }

    #[test]
    fn discover_tests_cargo_empty_output() {
        let items = discover_tests_from_output(TestFramework::CargoTest, "");
        assert!(items.is_empty());
    }

    #[test]
    fn discover_tests_cargo_skips_non_test_lines() {
        let output = "running 3 tests\ntests::my_test: test\n\n3 tests total\n";
        let items = discover_tests_from_output(TestFramework::CargoTest, output);
        assert_eq!(items.len(), 1); // only the module group
    }

    #[test]
    fn discover_tests_jest_stub() {
        let items = discover_tests_from_output(TestFramework::Jest, "some output");
        assert!(items.is_empty());
    }

    #[test]
    fn discover_tests_from_output_unknown() {
        let items = discover_tests_from_output(TestFramework::Unknown, "anything");
        assert!(items.is_empty());
    }


    #[test]
    fn name_filter_by_name() {
        let items = vec![
            TestItem::leaf("t1", "test_add"),
            TestItem::leaf("t2", "test_sub"),
            TestItem::leaf("t3", "bench_mul"),
        ];
        let f = TestItemNameFilter::new().with_name("test");
        assert_eq!(f.filter_items(&items).len(), 2);
    }

    #[test]
    fn name_filter_all() {
        let items = vec![TestItem::leaf("t1", "test_add")];
        let f = TestItemNameFilter::new();
        assert_eq!(f.filter_items(&items).len(), 1);
    }

    #[test]
    fn output_capture_basic() {
        let mut cap = TestRunOutputCapture::new();
        cap.append("hello\nworld");
        assert!(cap.stripped().contains("hello"));
        assert_eq!(cap.line_count(), 2);
    }

    #[test]
    fn output_capture_clear() {
        let mut cap = TestRunOutputCapture::new();
        cap.append("data");
        cap.clear();
        assert!(cap.stripped().is_empty());
    }

    #[test]
    fn coverage_map_basic() {
        let mut cm = TestCoverageMap::new();
        cm.set_file_coverage("main.rs", vec![true, true, false, true]);
        assert!((cm.file_coverage_percent("main.rs").unwrap() - 75.0).abs() < 0.01);
        assert_eq!(cm.uncovered_lines("main.rs"), vec![2]);
    }

    #[test]
    fn coverage_map_total() {
        let mut cm = TestCoverageMap::new();
        cm.set_file_coverage("a.rs", vec![true, false]);
        cm.set_file_coverage("b.rs", vec![true, true]);
        assert!((cm.total_coverage_percent() - 75.0).abs() < 0.01);
    }

    #[test]
    fn coverage_map_empty() {
        let cm = TestCoverageMap::new();
        assert!((cm.total_coverage_percent() - 0.0).abs() < 0.01);
    }

    #[test]
    fn diff_viewer_same() {
        let diff = TestDiffViewer::diff("a\nb", "a\nb");
        assert!(diff.iter().all(|l| l.kind == TestDiffKind::Same));
    }

    #[test]
    fn diff_viewer_different() {
        let diff = TestDiffViewer::diff("a\nb", "a\nc");
        assert!(TestDiffViewer::has_differences("a\nb", "a\nc"));
    }

    #[test]
    fn diff_viewer_added() {
        let diff = TestDiffViewer::diff("a", "a\nb");
        assert!(diff.iter().any(|l| l.kind == TestDiffKind::Added));
    }

    #[test]
    fn diff_viewer_removed() {
        let diff = TestDiffViewer::diff("a\nb", "a");
        assert!(diff.iter().any(|l| l.kind == TestDiffKind::Removed));
    }

    #[test]
    fn coverage_file_count() {
        let mut cm = TestCoverageMap::new();
        cm.set_file_coverage("a.rs", vec![true]);
        assert_eq!(cm.file_count(), 1);
    }


#[test]
    fn testresultdiffviewer_severity_ordering() {
        assert!(TestResultDiffViewerSeverity::Critical > TestResultDiffViewerSeverity::High);
        assert!(TestResultDiffViewerSeverity::High > TestResultDiffViewerSeverity::Medium);
        assert!(TestResultDiffViewerSeverity::Medium > TestResultDiffViewerSeverity::Low);
    }

    #[test]
    fn testresultdiffviewer_severity_display() {
        assert_eq!(TestResultDiffViewerSeverity::Low.to_string(), "low");
        assert_eq!(TestResultDiffViewerSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn testresultdiffviewer_entry_creation() {
        let e = TestResultDiffViewerEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, TestResultDiffViewerSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn testresultdiffviewer_entry_builder() {
        let e = TestResultDiffViewerEntry::new("e2", "Entry 2")
            .with_severity(TestResultDiffViewerSeverity::High)
            .with_detail("some detail")
            .with_result_count(42);
        assert_eq!(e.severity, TestResultDiffViewerSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.result_count, 42);
    }

    #[test]
    fn testresultdiffviewer_entry_enable_disable() {
        let mut e = TestResultDiffViewerEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn testresultdiffviewer_add_and_count() {
        let mut mgr = TestResultDiffViewer::new("test");
        mgr.add(TestResultDiffViewerEntry::new("a", "A"));
        mgr.add(TestResultDiffViewerEntry::new("b", "B").with_severity(TestResultDiffViewerSeverity::High));
        assert_eq!(mgr.result_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn testresultdiffviewer_remove() {
        let mut mgr = TestResultDiffViewer::new("test");
        mgr.add(TestResultDiffViewerEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn testresultdiffviewer_capacity() {
        let mut mgr = TestResultDiffViewer::new("test").with_capacity(1);
        assert!(mgr.add(TestResultDiffViewerEntry::new("a", "A")));
        assert!(!mgr.add(TestResultDiffViewerEntry::new("b", "B")));
    }

    #[test]
    fn testresultdiffviewer_sorted_by_severity() {
        let mut mgr = TestResultDiffViewer::new("test");
        mgr.add(TestResultDiffViewerEntry::new("lo", "Low"));
        mgr.add(TestResultDiffViewerEntry::new("hi", "High").with_severity(TestResultDiffViewerSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, TestResultDiffViewerSeverity::Critical);
    }

    #[test]
    fn testresultdiffviewer_summary() {
        let mgr = TestResultDiffViewer::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn testfilterexprparser_config_defaults() {
        let cfg = TestFilterExprParserConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn testfilterexprparser_item_creation() {
        let item = TestFilterExprParserItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn testfilterexprparser_add_and_get() {
        let mut mgr = TestFilterExprParser::new(TestFilterExprParserConfig::new("test"));
        mgr.add(TestFilterExprParserItem::new("k1", "v1"));
        assert_eq!(mgr.filter_depth(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn testfilterexprparser_remove_item() {
        let mut mgr = TestFilterExprParser::new(TestFilterExprParserConfig::new("test"));
        mgr.add(TestFilterExprParserItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn testfilterexprparser_sorted_by_priority() {
        let mut mgr = TestFilterExprParser::new(TestFilterExprParserConfig::new("test"));
        mgr.add(TestFilterExprParserItem::new("lo", "low").with_priority(1));
        mgr.add(TestFilterExprParserItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn testfilterexprparser_items_with_tag() {
        let mut mgr = TestFilterExprParser::new(TestFilterExprParserConfig::new("test"));
        mgr.add(TestFilterExprParserItem::new("a", "1").with_tag("x"));
        mgr.add(TestFilterExprParserItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn testfilterexprparser_report() {
        let mgr = TestFilterExprParser::new(TestFilterExprParserConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn exttest_ringbuf_push_get() {
        let mut rb = ExtTestRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn exttest_ringbuf_overflow() {
        let mut rb = ExtTestRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn exttest_ringbuf_clear() {
        let mut rb = ExtTestRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn exttest_ringbuf_newest_oldest() {
        let mut rb = ExtTestRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn exttest_ringbuf_to_vec() {
        let mut rb = ExtTestRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn exttest_ringbuf_is_full() {
        let mut rb = ExtTestRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn exttest_builder_valid() {
        let cfg = ExtTestBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn exttest_builder_empty_name() {
        let r = ExtTestBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn exttest_builder_bad_priority() {
        assert!(ExtTestBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn exttest_builder_zero_max() {
        assert!(ExtTestBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn exttest_cfg_merge() {
        let mut a = ExtTestBuilder::new("a").property("x", "1").build().unwrap();
        let b = ExtTestBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn exttest_cfg_display() {
        let cfg = ExtTestBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }

}