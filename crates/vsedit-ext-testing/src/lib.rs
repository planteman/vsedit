//! Ext API: Testing.
//!
//! RPC bridge between the extension host and the main thread for the test API.

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
}
