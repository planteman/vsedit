//! Test explorer view.

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
}

impl Default for TestService {
    fn default() -> Self {
        Self::new()
    }
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
}
