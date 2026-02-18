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



// ---------------------------------------------------------------------------
// ext_testing – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for extension test runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YExtTestingTestRunState {
    Queued,
    Running,
    Passed,
    Failed,
}

impl YExtTestingTestRunState {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Queued => 0,
            Self::Running => 1,
            Self::Passed => 2,
            Self::Failed => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Running => "Running",
            Self::Passed => "Passed",
            Self::Failed => "Failed",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YExtTestingTestRunState] {
        &[
            YExtTestingTestRunState::Queued,
            YExtTestingTestRunState::Running,
            YExtTestingTestRunState::Passed,
            YExtTestingTestRunState::Failed,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YExtTestingTestRunState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks test result summary data.
#[derive(Debug, Clone)]
pub struct YExtTestingTestResultSummary {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
}

impl YExtTestingTestResultSummary {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            passed: 0,
            failed: 0,
            skipped: 0,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YExtTestingTestResultSummary({}: {:?})", "passed", self.passed)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_ext_testing_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_ext_testing_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_ext_testing_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_ext_testing_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_ext_testing_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_ext_testing_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_ext_testing_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_ext_testing_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// ext_testing – Extended test coverage map helpers
// ---------------------------------------------------------------------------

/// Priority levels for test coverage map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZExtTestingPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZExtTestingPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZExtTestingPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZExtTestingPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks test coverage map data.
#[derive(Debug, Clone)]
pub struct ZExtTestingTestCoverageMap {
    pub covered_lines: Vec<(String, Vec<u32>)>,
    pub total_lines: usize,
    pub pct_covered: f64,
}

impl ZExtTestingTestCoverageMap {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            covered_lines: Vec::new(),
            total_lines: 0,
            pct_covered: 0.0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.covered_lines.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.covered_lines.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.covered_lines.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZExtTestingTestCoverageMap[total_lines={:?}, pct_covered={:?}]", self.total_lines, self.pct_covered)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for test coverage map.
pub fn z_ext_testing_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_ext_testing_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_ext_testing_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_ext_testing_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_ext_testing_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_ext_testing_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_ext_testing_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 50
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer50 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer50 {
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
pub fn xb_fnv1a_50(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_50<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_50<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_50(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_50(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 73
// ---------------------------------------------------------------------------

/// Generic object pool `Xc73Pool<T>`.
pub struct Xc73Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc73Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc73PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc73Pool<T> {
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
    pub fn stats(&self) -> Xc73PoolStats {
        Xc73PoolStats {
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

impl<T> Default for Xc73Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc73Scheduler`.
pub struct Xc73Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc73Scheduler {
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

impl Default for Xc73Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_73 hash for the given byte slice.
pub fn xc_73_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_73 convention.
pub fn xc_73_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe63 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe63Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe63PipelineError {
    pub stage: Xe63Stage,
    pub message: String,
}

impl std::fmt::Display for Xe63PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe63Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe63Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe63PipelineError>>>,
    stage_names: Vec<Xe63Stage>,
}

impl Xe63Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe63PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe63Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe63PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe63Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe63PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe63Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe63PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe63Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe63PipelineError> {
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

    pub fn compose(mut self, other: Xe63Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe63CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe63CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe63Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe63CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe63CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe63Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe63CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_63_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe63CacheEntry {
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

    fn xe_63_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe63CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_63_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe63PipelineError> {
    Ok(data)
}

pub fn xe_63_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe63PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_63_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe63PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_63_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe63PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_63_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe63PipelineError> {
    Err(Xe63PipelineError {
        stage: Xe63Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_61: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg61Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg61Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg61Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_61: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg61Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg61Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg61Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg61Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 72).
pub struct Xh72SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh72SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 114 as u64,
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

/// A compact bit set supporting boolean operations (variant 72).
pub struct Xh72BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh72BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 72).
pub struct Xi72Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi72Deque<T> {
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
pub struct Xi72Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi72Interval {
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

/// A simple interval tree (variant 72).
pub struct Xi72IntervalTree {
    xi_intervals: Vec<Xi72Interval>,
}

impl Xi72IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi72Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi72Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi72Interval) -> Vec<&Xi72Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi72Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi72Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi72Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi72Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi72Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi72Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 73) ---

/// Disjoint set / union-find for crate 73.
pub struct Xj73UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj73UnionFind {
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

const XJ73_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 73.
pub struct Xj73BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj73BTreeNode<K, V>>>,
    len: usize,
}

struct Xj73BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj73BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj73BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ73_BTREE_ORDER - 1
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
        let mid = XJ73_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj73BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj73BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj73BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj73BTreeNode::xj_new_leaf();
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


    // -- ext_testing extended domain tests ----------------------------------------

    #[test]
    fn y_ext_testing_enum_index() {
        assert_eq!(YExtTestingTestRunState::Queued.index(), 0);
        assert_eq!(YExtTestingTestRunState::Running.index(), 1);
        assert_eq!(YExtTestingTestRunState::Passed.index(), 2);
        assert_eq!(YExtTestingTestRunState::Failed.index(), 3);
    }

    #[test]
    fn y_ext_testing_enum_label() {
        assert_eq!(YExtTestingTestRunState::Queued.label(), "Queued");
        assert_eq!(YExtTestingTestRunState::Running.label(), "Running");
        assert_eq!(YExtTestingTestRunState::Passed.label(), "Passed");
        assert_eq!(YExtTestingTestRunState::Failed.label(), "Failed");
    }

    #[test]
    fn y_ext_testing_enum_all() {
        let all = YExtTestingTestRunState::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_ext_testing_enum_is_default() {
        assert!(YExtTestingTestRunState::Queued.is_default());
        assert!(!YExtTestingTestRunState::Failed.is_default());
    }

    #[test]
    fn y_ext_testing_enum_display() {
        assert_eq!(format!("{}", YExtTestingTestRunState::Queued), "Queued");
    }

    #[test]
    fn y_ext_testing_struct_new() {
        let s = YExtTestingTestResultSummary::new();
        let _ = s.summary();
    }

    #[test]
    fn y_ext_testing_fingerprint_deterministic() {
        let h1 = y_ext_testing_fingerprint("hello");
        let h2 = y_ext_testing_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_ext_testing_fingerprint("a"), y_ext_testing_fingerprint("b"));
    }

    #[test]
    fn y_ext_testing_truncate_short() {
        assert_eq!(y_ext_testing_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_ext_testing_truncate_long() {
        let r = y_ext_testing_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_ext_testing_normalize_key_basic() {
        assert_eq!(y_ext_testing_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_ext_testing_split_path_basic() {
        let parts = y_ext_testing_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_ext_testing_count_occurrences_basic() {
        assert_eq!(y_ext_testing_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_ext_testing_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_ext_testing_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_ext_testing_in_range_basic() {
        assert!(y_ext_testing_in_range(5, 1, 10));
        assert!(y_ext_testing_in_range(1, 1, 10));
        assert!(y_ext_testing_in_range(10, 1, 10));
        assert!(!y_ext_testing_in_range(0, 1, 10));
        assert!(!y_ext_testing_in_range(11, 1, 10));
    }

    #[test]
    fn y_ext_testing_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_ext_testing_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_ext_testing_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_ext_testing_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- ext_testing Z-extended tests -----------------------------------------------

    #[test]
    fn z_ext_testing_priority_weight() {
        assert_eq!(ZExtTestingPriority::Idle.weight(), 0);
        assert_eq!(ZExtTestingPriority::Normal.weight(), 2);
        assert_eq!(ZExtTestingPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_ext_testing_priority_label() {
        assert_eq!(ZExtTestingPriority::Low.label(), "low");
        assert_eq!(ZExtTestingPriority::High.label(), "high");
    }

    #[test]
    fn z_ext_testing_priority_is_elevated() {
        assert!(!ZExtTestingPriority::Normal.is_elevated());
        assert!(ZExtTestingPriority::High.is_elevated());
        assert!(ZExtTestingPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_ext_testing_priority_display() {
        assert_eq!(format!("{}", ZExtTestingPriority::Idle), "idle");
    }

    #[test]
    fn z_ext_testing_priority_all_asc() {
        let all = ZExtTestingPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZExtTestingPriority::Idle);
        assert_eq!(all[4], ZExtTestingPriority::Realtime);
    }

    #[test]
    fn z_ext_testing_struct_new() {
        let s = ZExtTestingTestCoverageMap::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_ext_testing_struct_toggled_clone() {
        let s = ZExtTestingTestCoverageMap::new();
        let t = s.toggled_clone();
        let _ = t.pct_covered;
    }

    #[test]
    fn z_ext_testing_rolling_hash_deterministic() {
        let h1 = z_ext_testing_rolling_hash(b"test");
        let h2 = z_ext_testing_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_ext_testing_rolling_hash(b"a"), z_ext_testing_rolling_hash(b"b"));
    }

    #[test]
    fn z_ext_testing_pad_to_basic() {
        assert_eq!(z_ext_testing_pad_to("hi", 5), "hi   ");
        assert_eq!(z_ext_testing_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_ext_testing_is_identifier_basic() {
        assert!(z_ext_testing_is_identifier("foo_bar"));
        assert!(z_ext_testing_is_identifier("abc123"));
        assert!(!z_ext_testing_is_identifier(""));
        assert!(!z_ext_testing_is_identifier("has space"));
    }

    #[test]
    fn z_ext_testing_levenshtein_basic() {
        assert_eq!(z_ext_testing_levenshtein("", ""), 0);
        assert_eq!(z_ext_testing_levenshtein("abc", "abc"), 0);
        assert_eq!(z_ext_testing_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_ext_testing_unique_words_basic() {
        let w = z_ext_testing_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_ext_testing_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_ext_testing_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_ext_testing_common_prefix_basic() {
        assert_eq!(z_ext_testing_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_ext_testing_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_ext_testing_struct_clear() {
        let mut s = ZExtTestingTestCoverageMap::new();
        s.covered_lines.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_ext_testing_rolling_hash_empty() {
        let h = z_ext_testing_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_50_push_and_len() {
        let mut rb = super::XbRingBuffer50::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_50_overwrite() {
        let mut rb = super::XbRingBuffer50::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_50_get_out_of_bounds() {
        let rb = super::XbRingBuffer50::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_50_drain_all() {
        let mut rb = super::XbRingBuffer50::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_50_peek_front_back() {
        let mut rb = super::XbRingBuffer50::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_50_clear() {
        let mut rb = super::XbRingBuffer50::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_50_capacity() {
        let rb = super::XbRingBuffer50::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_50_basic() {
        let h = super::xb_fnv1a_50(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_50(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_50_different_inputs() {
        let h1 = super::xb_fnv1a_50(b"abc");
        let h2 = super::xb_fnv1a_50(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_50_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_50(&data);
        let dec = super::xb_rle_decode_50(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_50_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_50(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_50(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_50_values() {
        assert!((super::xb_clamp_50(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_50(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_50(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_50_values() {
        assert!((super::xb_lerp_50(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_50(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_50(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_50_wrap_around_twice() {
        let mut rb = super::XbRingBuffer50::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 73 ----

    #[test]
    fn xc_73_pool_new_empty() {
        let pool: super::Xc73Pool<i32> = super::Xc73Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_73_pool_release_acquire() {
        let mut pool = super::Xc73Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_73_pool_acquire_empty() {
        let mut pool: super::Xc73Pool<i32> = super::Xc73Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_73_pool_full() {
        let mut pool = super::Xc73Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_73_pool_drain() {
        let mut pool = super::Xc73Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_73_pool_stats() {
        let mut pool = super::Xc73Pool::new(8);
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
    fn xc_73_pool_clear() {
        let mut pool = super::Xc73Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_73_pool_shrink() {
        let mut pool = super::Xc73Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_73_pool_default() {
        let pool: super::Xc73Pool<String> = super::Xc73Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_73_pool_extend() {
        let mut pool = super::Xc73Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_73_pool_retain() {
        let mut pool = super::Xc73Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_73_scheduler_round_robin() {
        let mut sched = super::Xc73Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_73_scheduler_empty() {
        let mut sched = super::Xc73Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_73_scheduler_reset() {
        let mut sched = super::Xc73Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_73_scheduler_add_remove() {
        let mut sched = super::Xc73Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_73_scheduler_targets() {
        let sched = super::Xc73Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_73_hash_empty() {
        assert_eq!(super::xc_73_hash(b""), 5381);
    }

    #[test]
    fn xc_73_hash_data() {
        let h = super::xc_73_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_73_hash(b"hello"), h);
    }

    #[test]
    fn xc_73_reverse_str() {
        assert_eq!(super::xc_73_reverse("abc"), "cba");
        assert_eq!(super::xc_73_reverse(""), "");
    }


    #[test]
    fn xe_63_pipeline_empty() {
        let p = super::Xe63Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_63_pipeline_parse_stage() {
        let p = super::Xe63Pipeline::new()
            .add_parse(super::xe_63_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_63_pipeline_transform_double() {
        let p = super::Xe63Pipeline::new()
            .add_transform(super::xe_63_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_63_pipeline_validate_reverse() {
        let p = super::Xe63Pipeline::new()
            .add_validate(super::xe_63_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_63_pipeline_emit_filter() {
        let p = super::Xe63Pipeline::new()
            .add_emit(super::xe_63_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_63_pipeline_multi_stage() {
        let p = super::Xe63Pipeline::new()
            .add_parse(super::xe_63_pipeline_identity)
            .add_transform(super::xe_63_pipeline_double)
            .add_validate(super::xe_63_pipeline_reverse)
            .add_emit(super::xe_63_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_63_pipeline_error_propagation() {
        let p = super::Xe63Pipeline::new()
            .add_parse(super::xe_63_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe63Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_63_pipeline_compose() {
        let p1 = super::Xe63Pipeline::new()
            .add_parse(super::xe_63_pipeline_identity);
        let p2 = super::Xe63Pipeline::new()
            .add_transform(super::xe_63_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_63_pipeline_error_display() {
        let e = super::Xe63PipelineError {
            stage: super::Xe63Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_63_cache_put_get() {
        let mut c = super::Xe63Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_63_cache_miss() {
        let mut c: super::Xe63Cache<&str, i32> = super::Xe63Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_63_cache_ttl_expiry() {
        let mut c = super::Xe63Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_63_cache_evict() {
        let mut c = super::Xe63Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_63_cache_capacity() {
        let mut c = super::Xe63Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_63_cache_stats() {
        let mut c = super::Xe63Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_63_cache_clear() {
        let mut c = super::Xe63Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_61 graph tests ------------------------------------------------

    #[test]
    fn xg_61_graph_empty() {
        let g = super::Xg61Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_61_graph_add_node() {
        let mut g = super::Xg61Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_61_graph_add_edge() {
        let mut g = super::Xg61Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_61_graph_neighbors() {
        let mut g = super::Xg61Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_61_graph_has_path() {
        let mut g = super::Xg61Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_61_graph_self_path() {
        let g = super::Xg61Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_61_graph_topo_sort() {
        let mut g = super::Xg61Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_61_graph_cycle_detect_false() {
        let mut g = super::Xg61Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_61_graph_cycle_detect_true() {
        let mut g = super::Xg61Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_61 heap tests -------------------------------------------------

    #[test]
    fn xg_61_heap_empty() {
        let h: super::Xg61Heap<i32> = super::Xg61Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_61_heap_push_pop() {
        let mut h = super::Xg61Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_61_heap_peek() {
        let mut h = super::Xg61Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_61_heap_drain_sorted() {
        let mut h = super::Xg61Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_61_heap_merge() {
        let mut a = super::Xg61Heap::new();
        let mut b = super::Xg61Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_61_heap_default() {
        let h: super::Xg61Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_61_graph_default() {
        let g: super::Xg61Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh72_skip_insert_contains() {
        let mut sl = super::Xh72SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh72_skip_remove() {
        let mut sl = super::Xh72SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh72_skip_len() {
        let mut sl = super::Xh72SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh72_skip_range_query() {
        let mut sl = super::Xh72SkipList::xh_new(4);
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
    fn xh72_skip_floor_ceiling() {
        let mut sl = super::Xh72SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh72_skip_rank() {
        let mut sl = super::Xh72SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh72_skip_empty() {
        let sl = super::Xh72SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh72_skip_duplicates() {
        let mut sl = super::Xh72SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh72_bitset_set_test() {
        let mut bs = super::Xh72BitSet::xh_new(256);
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
    fn xh72_bitset_clear_count() {
        let mut bs = super::Xh72BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh72_bitset_and_or_xor() {
        let mut a = super::Xh72BitSet::xh_new(128);
        let mut b = super::Xh72BitSet::xh_new(128);
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
    fn xh72_bitset_iter_ones() {
        let mut bs = super::Xh72BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh72_bitset_first_last() {
        let mut bs = super::Xh72BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh72_bitset_empty() {
        let bs = super::Xh72BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi72_deque_push_pop_back() {
        let mut dq = super::Xi72Deque::xi_new(4);
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
    fn xi72_deque_push_pop_front() {
        let mut dq = super::Xi72Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi72_deque_mixed_ops() {
        let mut dq = super::Xi72Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi72_deque_get_and_split() {
        let mut dq = super::Xi72Deque::xi_new(8);
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
    fn xi72_deque_rotate_left() {
        let mut dq = super::Xi72Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi72_deque_rotate_right() {
        let mut dq = super::Xi72Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi72_deque_grow() {
        let mut dq = super::Xi72Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi72_deque_empty() {
        let dq = super::Xi72Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi72_interval_tree_insert_query() {
        let mut tree = super::Xi72IntervalTree::xi_new();
        tree.xi_insert(super::Xi72Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi72Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi72Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi72_interval_tree_overlap() {
        let mut tree = super::Xi72IntervalTree::xi_new();
        tree.xi_insert(super::Xi72Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi72Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi72Interval::xi_new(12, 20));
        let q = super::Xi72Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi72_interval_tree_remove() {
        let mut tree = super::Xi72IntervalTree::xi_new();
        tree.xi_insert(super::Xi72Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi72Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi72_interval_tree_gaps() {
        let mut tree = super::Xi72IntervalTree::xi_new();
        tree.xi_insert(super::Xi72Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi72Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi72Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi72Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi72Interval::xi_new(8, 10));
    }

    #[test]
    fn xi72_interval_tree_merge() {
        let mut tree = super::Xi72IntervalTree::xi_new();
        tree.xi_insert(super::Xi72Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi72Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi72Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi72Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi72Interval::xi_new(10, 15));
    }

    #[test]
    fn xi72_interval_tree_all() {
        let mut tree = super::Xi72IntervalTree::xi_new();
        tree.xi_insert(super::Xi72Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi72Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi72_interval_tree_empty() {
        let tree = super::Xi72IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi72_interval_tree_contains_point() {
        let iv = super::Xi72Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 73) ---

    #[test]
    fn xj_73_uf_make_and_find() {
        let mut uf = super::Xj73UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_73_uf_union_connected() {
        let mut uf = super::Xj73UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_73_uf_component_count() {
        let mut uf = super::Xj73UnionFind::xj_new();
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
    fn xj_73_uf_component_size() {
        let mut uf = super::Xj73UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_73_uf_largest_component() {
        let mut uf = super::Xj73UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_73_uf_many_elements() {
        let mut uf = super::Xj73UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_73_uf_separate_components() {
        let mut uf = super::Xj73UnionFind::xj_new();
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
    fn xj_73_uf_path_compression() {
        let mut uf = super::Xj73UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_73_bt_insert_get() {
        let mut bt = super::Xj73BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_73_bt_contains_len() {
        let mut bt = super::Xj73BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_73_bt_replace() {
        let mut bt = super::Xj73BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_73_bt_remove() {
        let mut bt = super::Xj73BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_73_bt_keys_values() {
        let mut bt = super::Xj73BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_73_bt_range() {
        let mut bt = super::Xj73BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_73_bt_min_max() {
        let mut bt = super::Xj73BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_73_bt_many_inserts() {
        let mut bt = super::Xj73BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }

}