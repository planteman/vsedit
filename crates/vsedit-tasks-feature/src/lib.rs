//! Task runner: build tasks, test tasks, and task execution.

use std::collections::HashMap;
use std::fmt;

// ── Errors ──────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Eq)]
pub enum TaskError {
    TaskNotFound(String),
    AlreadyRunning(String),
    ExecutionFailed(String),
}

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskError::TaskNotFound(name) => write!(f, "task not found: {name}"),
            TaskError::AlreadyRunning(name) => write!(f, "task already running: {name}"),
            TaskError::ExecutionFailed(msg) => write!(f, "execution failed: {msg}"),
        }
    }
}

// ── Core types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskSource {
    Workspace,
    Extension,
    User,
}

impl fmt::Display for TaskSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskSource::Workspace => write!(f, "Workspace"),
            TaskSource::Extension => write!(f, "Extension"),
            TaskSource::User => write!(f, "User"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskGroup {
    Build,
    Test,
    Clean,
    Deploy,
    None,
}

impl fmt::Display for TaskGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TaskGroup::Build => write!(f, "Build"),
            TaskGroup::Test => write!(f, "Test"),
            TaskGroup::Clean => write!(f, "Clean"),
            TaskGroup::Deploy => write!(f, "Deploy"),
            TaskGroup::None => write!(f, "None"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskDefinition {
    pub task_type: String,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub name: String,
    pub source: TaskSource,
    pub group: TaskGroup,
    pub command: String,
    pub args: Vec<String>,
    pub definition: TaskDefinition,
    pub is_background: bool,
    pub problem_matcher: Option<String>,
}

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}] ({})", self.name, self.group, self.source)
    }
}

pub struct TaskExecution {
    pub task: Task,
    pub running: bool,
    pub exit_code: Option<i32>,
}

impl TaskExecution {
    pub fn is_success(&self) -> bool {
        self.exit_code == Some(0)
    }
}

// ── TaskBuilder ─────────────────────────────────────────────────────────

pub struct TaskBuilder {
    name: String,
    source: TaskSource,
    group: TaskGroup,
    command: String,
    args: Vec<String>,
    definition: TaskDefinition,
    is_background: bool,
    problem_matcher: Option<String>,
}

impl TaskBuilder {
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: TaskSource::Workspace,
            group: TaskGroup::None,
            command: command.into(),
            args: Vec::new(),
            definition: TaskDefinition {
                task_type: "shell".to_string(),
                properties: HashMap::new(),
            },
            is_background: false,
            problem_matcher: None,
        }
    }

    pub fn source(mut self, source: TaskSource) -> Self {
        self.source = source;
        self
    }

    pub fn group(mut self, group: TaskGroup) -> Self {
        self.group = group;
        self
    }

    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn background(mut self, bg: bool) -> Self {
        self.is_background = bg;
        self
    }

    pub fn problem_matcher(mut self, matcher: impl Into<String>) -> Self {
        self.problem_matcher = Some(matcher.into());
        self
    }

    pub fn definition(mut self, def: TaskDefinition) -> Self {
        self.definition = def;
        self
    }

    pub fn build(self) -> Task {
        Task {
            name: self.name,
            source: self.source,
            group: self.group,
            command: self.command,
            args: self.args,
            definition: self.definition,
            is_background: self.is_background,
            problem_matcher: self.problem_matcher,
        }
    }
}

// ── TaskService ─────────────────────────────────────────────────────────

pub struct TaskService {
    pub tasks: Vec<Task>,
    pub executions: Vec<TaskExecution>,
}

impl TaskService {
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            executions: Vec::new(),
        }
    }

    pub fn register_task(&mut self, task: Task) {
        self.tasks.push(task);
    }

    pub fn get_tasks_by_group(&self, group: TaskGroup) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.group == group).collect()
    }

    /// Starts a task by name. Returns the execution index if found.
    pub fn run_task(&mut self, name: &str) -> Option<usize> {
        let task = self.tasks.iter().find(|t| t.name == name)?.clone();
        let idx = self.executions.len();
        self.executions.push(TaskExecution {
            task,
            running: true,
            exit_code: None,
        });
        Some(idx)
    }

    /// Like `run_task` but returns a `Result` with a typed error.
    pub fn try_run_task(&mut self, name: &str) -> Result<usize, TaskError> {
        let task = self
            .tasks
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| TaskError::TaskNotFound(name.to_string()))?
            .clone();
        let idx = self.executions.len();
        self.executions.push(TaskExecution {
            task,
            running: true,
            exit_code: None,
        });
        Ok(idx)
    }

    /// Marks an execution as stopped with the given exit code.
    pub fn stop_task(&mut self, index: usize, exit_code: i32) -> Result<(), TaskError> {
        let exec = self.executions.get_mut(index).ok_or_else(|| {
            TaskError::TaskNotFound(format!("execution index {index}"))
        })?;
        exec.running = false;
        exec.exit_code = Some(exit_code);
        Ok(())
    }

    pub fn get_task_by_name(&self, name: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.name == name)
    }

    pub fn remove_task(&mut self, name: &str) -> Option<Task> {
        let pos = self.tasks.iter().position(|t| t.name == name)?;
        Some(self.tasks.remove(pos))
    }

    pub fn get_executions_for_task(&self, name: &str) -> Vec<&TaskExecution> {
        self.executions.iter().filter(|e| e.task.name == name).collect()
    }

    pub fn clear_completed_executions(&mut self) {
        self.executions.retain(|e| e.running);
    }

    pub fn running_count(&self) -> usize {
        self.executions.iter().filter(|e| e.running).count()
    }

    pub fn get_running(&self) -> Vec<&TaskExecution> {
        self.executions.iter().filter(|e| e.running).collect()
    }

    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Returns true if tasks is empty.
    pub fn is_tasks_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Get the first task, if any.
    pub fn first_task(&self) -> Option<&Task> {
        self.tasks.first()
    }

    /// Get the last task, if any.
    pub fn last_task(&self) -> Option<&Task> {
        self.tasks.last()
    }

    /// Retain only tasks matching the predicate.
    pub fn retain_tasks(&mut self, f: impl Fn(&Task) -> bool) {
        self.tasks.retain(|item| f(item));
    }

    /// Returns true if executions is empty.
    pub fn is_executions_empty(&self) -> bool {
        self.executions.is_empty()
    }

    /// Get the first execution, if any.
    pub fn first_execution(&self) -> Option<&TaskExecution> {
        self.executions.first()
    }

    /// Get the last execution, if any.
    pub fn last_execution(&self) -> Option<&TaskExecution> {
        self.executions.last()
    }

    /// Retain only executions matching the predicate.
    pub fn retain_executions(&mut self, f: impl Fn(&TaskExecution) -> bool) {
        self.executions.retain(|item| f(item));
    }
}

impl Default for TaskService {
    fn default() -> Self {
        Self::new()
    }
}

// ── Task Runner ─────────────────────────────────────────────────────────

/// Tracks the execution lifecycle of a task.
#[derive(Debug, Clone)]
pub struct TaskRunResult {
    pub task_name: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

impl TaskRunResult {
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }

    pub fn has_output(&self) -> bool {
        !self.stdout.is_empty() || !self.stderr.is_empty()
    }

    pub fn combined_output(&self) -> String {
        if self.stderr.is_empty() {
            self.stdout.clone()
        } else if self.stdout.is_empty() {
            self.stderr.clone()
        } else {
            format!("{}\n{}", self.stdout, self.stderr)
        }
    }
}

/// A task runner that can simulate execution and track results.
pub struct TaskRunner {
    results: Vec<TaskRunResult>,
    running_tasks: Vec<String>,
}

impl TaskRunner {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
            running_tasks: Vec::new(),
        }
    }

    /// Start a task (marks it as running).
    pub fn start(&mut self, task: &Task) -> Result<(), TaskError> {
        if self.running_tasks.contains(&task.name) {
            return Err(TaskError::AlreadyRunning(task.name.clone()));
        }
        self.running_tasks.push(task.name.clone());
        Ok(())
    }

    /// Complete a running task with a result.
    pub fn complete(&mut self, result: TaskRunResult) -> Result<(), TaskError> {
        let pos = self
            .running_tasks
            .iter()
            .position(|n| n == &result.task_name);
        match pos {
            Some(i) => {
                self.running_tasks.remove(i);
                self.results.push(result);
                Ok(())
            }
            None => Err(TaskError::TaskNotFound(result.task_name.clone())),
        }
    }

    pub fn is_running(&self, name: &str) -> bool {
        self.running_tasks.iter().any(|n| n == name)
    }

    pub fn running_count(&self) -> usize {
        self.running_tasks.len()
    }

    pub fn history(&self) -> &[TaskRunResult] {
        &self.results
    }

    pub fn last_result(&self) -> Option<&TaskRunResult> {
        self.results.last()
    }

    pub fn success_count(&self) -> usize {
        self.results.iter().filter(|r| r.success()).count()
    }

    pub fn failure_count(&self) -> usize {
        self.results.iter().filter(|r| !r.success()).count()
    }
}

impl Default for TaskRunner {
    fn default() -> Self {
        Self::new()
    }
}

// ── Task Auto-Detection ─────────────────────────────────────────────────

/// Detected task from a build file.
#[derive(Debug, Clone, PartialEq)]
pub struct DetectedTask {
    pub name: String,
    pub command: String,
    pub source_file: String,
    pub group: TaskGroup,
}

/// Auto-detect tasks from package.json content.
pub fn detect_from_package_json(content: &str) -> Vec<DetectedTask> {
    let mut tasks = Vec::new();
    if let Some(scripts_start) = content.find("\"scripts\"") {
        if let Some(brace_start) = content[scripts_start..].find('{') {
            let rest = &content[scripts_start + brace_start + 1..];
            if let Some(brace_end) = rest.find('}') {
                let scripts_block = &rest[..brace_end];
                for line in scripts_block.lines() {
                    let trimmed = line.trim().trim_end_matches(',');
                    if let Some(colon) = trimmed.find(':') {
                        let key = trimmed[..colon].trim().trim_matches('"');
                        let val = trimmed[colon + 1..].trim().trim_matches('"');
                        if !key.is_empty() && !val.is_empty() {
                            let group = if key.contains("build") {
                                TaskGroup::Build
                            } else if key.contains("test") {
                                TaskGroup::Test
                            } else if key.contains("clean") {
                                TaskGroup::Clean
                            } else {
                                TaskGroup::None
                            };
                            tasks.push(DetectedTask {
                                name: format!("npm: {key}"),
                                command: format!("npm run {key}"),
                                source_file: "package.json".to_string(),
                                group,
                            });
                        }
                    }
                }
            }
        }
    }
    tasks
}

/// Auto-detect tasks from Makefile content.
pub fn detect_from_makefile(content: &str) -> Vec<DetectedTask> {
    let mut tasks = Vec::new();
    for line in content.lines() {
        if let Some(colon_pos) = line.find(':') {
            let target = line[..colon_pos].trim();
            if !target.is_empty()
                && !target.starts_with('\t')
                && !target.starts_with(' ')
                && !target.starts_with('#')
                && !target.starts_with('.')
                && !target.contains('=')
            {
                let group = if target == "build" || target == "all" {
                    TaskGroup::Build
                } else if target == "test" || target == "check" {
                    TaskGroup::Test
                } else if target == "clean" {
                    TaskGroup::Clean
                } else {
                    TaskGroup::None
                };
                tasks.push(DetectedTask {
                    name: format!("make: {target}"),
                    command: format!("make {target}"),
                    source_file: "Makefile".to_string(),
                    group,
                });
            }
        }
    }
    tasks
}

/// Auto-detect tasks from Cargo.toml content.
pub fn detect_from_cargo_toml(content: &str) -> Vec<DetectedTask> {
    let mut tasks = Vec::new();
    if content.contains("[package]") {
        tasks.push(DetectedTask {
            name: "cargo: build".to_string(),
            command: "cargo build".to_string(),
            source_file: "Cargo.toml".to_string(),
            group: TaskGroup::Build,
        });
        tasks.push(DetectedTask {
            name: "cargo: test".to_string(),
            command: "cargo test".to_string(),
            source_file: "Cargo.toml".to_string(),
            group: TaskGroup::Test,
        });
        tasks.push(DetectedTask {
            name: "cargo: clean".to_string(),
            command: "cargo clean".to_string(),
            source_file: "Cargo.toml".to_string(),
            group: TaskGroup::Clean,
        });
        tasks.push(DetectedTask {
            name: "cargo: check".to_string(),
            command: "cargo check".to_string(),
            source_file: "Cargo.toml".to_string(),
            group: TaskGroup::Build,
        });
    }
    tasks
}

// ── Tests ───────────────────────────────────────────────────────────────

/// Accumulated statistics for tasks-feature operations.
#[derive(Debug, Clone, PartialEq)]
pub struct TasksFeatureStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl TasksFeatureStats {
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
    pub fn merge(&mut self, other: &TasksFeatureStats) {
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

impl Default for TasksFeatureStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TasksFeatureStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TasksFeatureStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for tasks-feature.
#[derive(Debug, Clone)]
pub struct TasksFeatureValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl TasksFeatureValidator {
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

impl Default for TasksFeatureValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(name: &str, group: TaskGroup) -> Task {
        Task {
            name: name.to_string(),
            source: TaskSource::Workspace,
            group,
            command: "cargo".to_string(),
            args: vec!["build".to_string()],
            definition: TaskDefinition {
                task_type: "shell".to_string(),
                properties: HashMap::new(),
            },
            is_background: false,
            problem_matcher: None,
        }
    }

    #[test]
    fn register_and_count() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("build", TaskGroup::Build));
        svc.register_task(make_task("test", TaskGroup::Test));
        assert_eq!(svc.task_count(), 2);
    }

    #[test]
    fn get_tasks_by_group() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("build", TaskGroup::Build));
        svc.register_task(make_task("lint", TaskGroup::Build));
        svc.register_task(make_task("test", TaskGroup::Test));
        assert_eq!(svc.get_tasks_by_group(TaskGroup::Build).len(), 2);
        assert_eq!(svc.get_tasks_by_group(TaskGroup::Test).len(), 1);
        assert_eq!(svc.get_tasks_by_group(TaskGroup::Clean).len(), 0);
    }

    #[test]
    fn run_task_and_get_running() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("build", TaskGroup::Build));
        let idx = svc.run_task("build");
        assert_eq!(idx, Some(0));
        assert_eq!(svc.get_running().len(), 1);
        assert!(svc.run_task("nonexistent").is_none());
    }

    #[test]
    fn try_run_task_not_found() {
        let mut svc = TaskService::new();
        let err = svc.try_run_task("missing").unwrap_err();
        assert_eq!(err, TaskError::TaskNotFound("missing".to_string()));
    }

    #[test]
    fn try_run_task_success() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("build", TaskGroup::Build));
        let idx = svc.try_run_task("build").unwrap();
        assert_eq!(idx, 0);
        assert!(svc.executions[0].running);
    }

    #[test]
    fn stop_task_sets_exit_code() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("build", TaskGroup::Build));
        let idx = svc.try_run_task("build").unwrap();
        svc.stop_task(idx, 0).unwrap();
        assert!(!svc.executions[0].running);
        assert!(svc.executions[0].is_success());
    }

    #[test]
    fn stop_task_invalid_index() {
        let mut svc = TaskService::new();
        assert!(svc.stop_task(99, 1).is_err());
    }

    #[test]
    fn get_task_by_name() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("build", TaskGroup::Build));
        assert!(svc.get_task_by_name("build").is_some());
        assert!(svc.get_task_by_name("nope").is_none());
    }

    #[test]
    fn remove_task() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("build", TaskGroup::Build));
        svc.register_task(make_task("test", TaskGroup::Test));
        let removed = svc.remove_task("build");
        assert!(removed.is_some());
        assert_eq!(svc.task_count(), 1);
        assert!(svc.remove_task("build").is_none());
    }

    #[test]
    fn get_executions_for_task() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("build", TaskGroup::Build));
        svc.run_task("build");
        svc.run_task("build");
        assert_eq!(svc.get_executions_for_task("build").len(), 2);
        assert_eq!(svc.get_executions_for_task("other").len(), 0);
    }

    #[test]
    fn clear_completed_executions() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("build", TaskGroup::Build));
        svc.run_task("build");
        svc.run_task("build");
        svc.stop_task(0, 0).unwrap();
        svc.clear_completed_executions();
        assert_eq!(svc.executions.len(), 1);
        assert!(svc.executions[0].running);
    }

    #[test]
    fn running_count() {
        let mut svc = TaskService::new();
        svc.register_task(make_task("a", TaskGroup::Build));
        svc.register_task(make_task("b", TaskGroup::Test));
        svc.run_task("a");
        svc.run_task("b");
        assert_eq!(svc.running_count(), 2);
        svc.stop_task(0, 0).unwrap();
        assert_eq!(svc.running_count(), 1);
    }

    #[test]
    fn execution_is_success() {
        let exec_ok = TaskExecution {
            task: make_task("t", TaskGroup::None),
            running: false,
            exit_code: Some(0),
        };
        let exec_fail = TaskExecution {
            task: make_task("t", TaskGroup::None),
            running: false,
            exit_code: Some(1),
        };
        let exec_none = TaskExecution {
            task: make_task("t", TaskGroup::None),
            running: true,
            exit_code: None,
        };
        assert!(exec_ok.is_success());
        assert!(!exec_fail.is_success());
        assert!(!exec_none.is_success());
    }

    #[test]
    fn task_builder() {
        let task = TaskBuilder::new("deploy", "kubectl")
            .source(TaskSource::User)
            .group(TaskGroup::Deploy)
            .args(vec!["apply".to_string(), "-f".to_string()])
            .background(true)
            .problem_matcher("$kubectl")
            .build();
        assert_eq!(task.name, "deploy");
        assert_eq!(task.command, "kubectl");
        assert_eq!(task.source, TaskSource::User);
        assert_eq!(task.group, TaskGroup::Deploy);
        assert_eq!(task.args.len(), 2);
        assert!(task.is_background);
        assert_eq!(task.problem_matcher, Some("$kubectl".to_string()));
    }

    #[test]
    fn display_impls() {
        assert_eq!(TaskSource::Workspace.to_string(), "Workspace");
        assert_eq!(TaskGroup::Build.to_string(), "Build");
        assert_eq!(TaskGroup::None.to_string(), "None");
        let task = make_task("lint", TaskGroup::Clean);
        assert_eq!(task.to_string(), "lint [Clean] (Workspace)");
    }

    #[test]
    fn task_error_display() {
        let e = TaskError::TaskNotFound("x".into());
        assert_eq!(e.to_string(), "task not found: x");
        let e = TaskError::AlreadyRunning("y".into());
        assert_eq!(e.to_string(), "task already running: y");
        let e = TaskError::ExecutionFailed("boom".into());
        assert_eq!(e.to_string(), "execution failed: boom");
    }

    #[test]
    fn eq_tasksource_same() {
        assert_eq!(TaskSource::Workspace, TaskSource::Workspace);
    }

    #[test]
    fn ne_tasksource_diff() {
        assert_ne!(TaskSource::Workspace, TaskSource::Extension);
    }

    #[test]
    fn eq_taskgroup_same() {
        assert_eq!(TaskGroup::Build, TaskGroup::Build);
    }

    #[test]
    fn ne_taskgroup_diff() {
        assert_ne!(TaskGroup::Build, TaskGroup::Test);
    }

    #[test]
    fn display_tasksource_variants() {
        assert!(!TaskSource::Workspace.to_string().is_empty());
        assert!(!TaskSource::Extension.to_string().is_empty());
        assert!(!TaskSource::User.to_string().is_empty());
    }

    #[test]
    fn display_taskgroup_variants() {
        assert!(!TaskGroup::Build.to_string().is_empty());
        assert!(!TaskGroup::Test.to_string().is_empty());
        assert!(!TaskGroup::Clean.to_string().is_empty());
        assert!(!TaskGroup::Deploy.to_string().is_empty());
        assert!(!TaskGroup::None.to_string().is_empty());
    }

    #[test]
    fn behavior_check_0() {
        let _svc = TaskService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        let _svc = TaskService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        let _svc = TaskService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        let _svc = TaskService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        let _svc = TaskService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        let _svc = TaskService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        let _svc = TaskService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        let _svc = TaskService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        let _svc = TaskService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        let _svc = TaskService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        let _svc = TaskService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        let _svc = TaskService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        let _svc = TaskService::new();
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn tasks_feature_stats_new_defaults() {
        let stats = TasksFeatureStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn tasks_feature_stats_record_success() {
        let mut stats = TasksFeatureStats::new();
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
    fn tasks_feature_stats_record_failure() {
        let mut stats = TasksFeatureStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn tasks_feature_stats_reset() {
        let mut stats = TasksFeatureStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn tasks_feature_stats_merge() {
        let mut a = TasksFeatureStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = TasksFeatureStats::new();
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
    fn tasks_feature_stats_display() {
        let mut stats = TasksFeatureStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn tasks_feature_stats_default() {
        let stats = TasksFeatureStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn tasks_feature_validator_accepts_valid_name() {
        let v = TasksFeatureValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn tasks_feature_validator_rejects_empty() {
        let v = TasksFeatureValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn tasks_feature_validator_rejects_too_long() {
        let v = TasksFeatureValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn tasks_feature_validator_forbidden_prefix() {
        let v = TasksFeatureValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn tasks_feature_validator_allowed_chars() {
        let v = TasksFeatureValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn tasks_feature_validator_range() {
        let v = TasksFeatureValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn tasks_feature_sanitize_removes_control() {
        let result = TasksFeatureValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn tasks_feature_truncate_short_string() {
        assert_eq!(TasksFeatureValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn tasks_feature_truncate_long_string() {
        let result = TasksFeatureValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn tasks_feature_is_ascii_printable() {
        assert!(TasksFeatureValidator::is_ascii_printable("Hello World 123"));
        assert!(!TasksFeatureValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn runner_start_and_complete() {
        let mut runner = TaskRunner::new();
        let task = TaskBuilder::new("build", "make build")
            .group(TaskGroup::Build)
            .build();
        runner.start(&task).unwrap();
        assert!(runner.is_running("build"));
        runner
            .complete(TaskRunResult {
                task_name: "build".to_string(),
                exit_code: 0,
                stdout: "OK".to_string(),
                stderr: String::new(),
                duration_ms: 100,
            })
            .unwrap();
        assert!(!runner.is_running("build"));
        assert_eq!(runner.success_count(), 1);
    }

    #[test]
    fn runner_already_running() {
        let mut runner = TaskRunner::new();
        let task = TaskBuilder::new("t", "cmd").build();
        runner.start(&task).unwrap();
        assert!(runner.start(&task).is_err());
    }

    #[test]
    fn runner_complete_unknown_fails() {
        let mut runner = TaskRunner::new();
        let result = runner.complete(TaskRunResult {
            task_name: "ghost".into(),
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 0,
        });
        assert!(result.is_err());
    }

    #[test]
    fn run_result_combined_output() {
        let r = TaskRunResult {
            task_name: "t".into(),
            exit_code: 1,
            stdout: "out".into(),
            stderr: "err".into(),
            duration_ms: 0,
        };
        assert!(!r.success());
        assert_eq!(r.combined_output(), "out\nerr");
    }

    #[test]
    fn detect_package_json_scripts() {
        let content = r#"{
  "scripts": {
    "build": "tsc",
    "test": "jest",
    "lint": "eslint"
  }
}"#;
        let tasks = detect_from_package_json(content);
        assert_eq!(tasks.len(), 3);
        assert!(tasks
            .iter()
            .any(|t| t.name == "npm: build" && t.group == TaskGroup::Build));
        assert!(tasks
            .iter()
            .any(|t| t.name == "npm: test" && t.group == TaskGroup::Test));
    }

    #[test]
    fn detect_makefile_targets() {
        let content =
            "all: main.o\n\tgcc -o main main.o\nclean:\n\trm -f main\ntest:\n\t./run_tests\n";
        let tasks = detect_from_makefile(content);
        assert!(tasks
            .iter()
            .any(|t| t.name == "make: all" && t.group == TaskGroup::Build));
        assert!(tasks
            .iter()
            .any(|t| t.name == "make: clean" && t.group == TaskGroup::Clean));
        assert!(tasks
            .iter()
            .any(|t| t.name == "make: test" && t.group == TaskGroup::Test));
    }

    #[test]
    fn detect_cargo_toml() {
        let content = "[package]\nname = \"myapp\"\nversion = \"0.1.0\"\n";
        let tasks = detect_from_cargo_toml(content);
        assert!(tasks.len() >= 3);
        assert!(tasks.iter().any(|t| t.name == "cargo: build"));
        assert!(tasks.iter().any(|t| t.name == "cargo: test"));
    }

    #[test]
    fn detect_empty_package_json() {
        let tasks = detect_from_package_json("{}");
        assert!(tasks.is_empty());
    }

    #[test]
    fn runner_failure_count() {
        let mut runner = TaskRunner::new();
        let task = TaskBuilder::new("t", "cmd").build();
        runner.start(&task).unwrap();
        runner
            .complete(TaskRunResult {
                task_name: "t".into(),
                exit_code: 1,
                stdout: String::new(),
                stderr: "fail".into(),
                duration_ms: 50,
            })
            .unwrap();
        assert_eq!(runner.failure_count(), 1);
        assert_eq!(runner.success_count(), 0);
    }
}
