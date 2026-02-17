//! Ext API: Tasks.
//!
//! RPC bridge between the extension host and the main thread for task providers.

use std::collections::HashMap;
use std::fmt;
use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_tasks";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TaskMessage {
    RegisterProvider {
        provider_type: String,
    },
    UnregisterProvider {
        provider_type: String,
    },
    ExecuteTask {
        task: Task,
    },
    TerminateTask {
        execution_id: String,
    },
    FetchTasks {
        filter_type: Option<String>,
    },
}

// ── Error Types ──

/// Errors that can occur during task operations.
#[derive(Debug, Clone, PartialEq)]
pub enum TaskError {
    /// Task name is empty or blank.
    EmptyName,
    /// Task type is empty or blank.
    EmptyType,
    /// The referenced execution was not found.
    ExecutionNotFound(String),
    /// Provider type is already registered.
    DuplicateProvider(String),
    /// Provider type was not found.
    ProviderNotFound(String),
    /// A validation error with a custom message.
    ValidationError(String),
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskError::EmptyName => write!(f, "task name must not be empty"),
            TaskError::EmptyType => write!(f, "task type must not be empty"),
            TaskError::ExecutionNotFound(id) => write!(f, "execution not found: {id}"),
            TaskError::DuplicateProvider(t) => write!(f, "provider already registered: {t}"),
            TaskError::ProviderNotFound(t) => write!(f, "provider not found: {t}"),
            TaskError::ValidationError(msg) => write!(f, "validation error: {msg}"),
        }
    }
}

impl std::error::Error for TaskError {}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskDefinition {
    #[serde(rename = "type")]
    pub task_type: String,
    pub attributes: serde_json::Value,
}

impl TaskDefinition {
    /// Create a new task definition with the given type and no attributes.
    pub fn new(task_type: impl Into<String>) -> Self {
        Self {
            task_type: task_type.into(),
            attributes: serde_json::Value::Object(Default::default()),
        }
    }

    /// Create a shell-type task definition with the given command.
    pub fn shell(command: impl Into<String>) -> Self {
        Self {
            task_type: "shell".into(),
            attributes: serde_json::json!({ "command": command.into() }),
        }
    }

    /// Create a process-type task definition.
    pub fn process(program: impl Into<String>, args: &[&str]) -> Self {
        Self {
            task_type: "process".into(),
            attributes: serde_json::json!({
                "program": program.into(),
                "args": args,
            }),
        }
    }

    /// Validate that the definition has a non-empty type.
    pub fn validate(&self) -> Result<(), TaskError> {
        if self.task_type.trim().is_empty() {
            return Err(TaskError::EmptyType);
        }
        Ok(())
    }

    /// Return the command string if this is a shell-type definition.
    pub fn shell_command(&self) -> Option<&str> {
        if self.task_type == "shell" {
            self.attributes.get("command").and_then(|v| v.as_str())
        } else {
            None
        }
    }
}

impl std::fmt::Display for TaskDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}]", self.task_type)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub name: String,
    pub definition: TaskDefinition,
    pub source: String,
    pub group: Option<String>,
    pub detail: Option<String>,
}

impl Task {
    /// Validate the task name and definition.
    pub fn validate(&self) -> Result<(), TaskError> {
        if self.name.trim().is_empty() {
            return Err(TaskError::EmptyName);
        }
        self.definition.validate()
    }

    /// Returns true if this task belongs to the given group.
    pub fn is_in_group(&self, group: &str) -> bool {
        self.group.as_deref() == Some(group)
    }

    /// Returns a display label combining name, source, and optional detail.
    pub fn label(&self) -> String {
        match &self.detail {
            Some(d) => format!("{} ({}) - {}", self.name, self.source, d),
            None => format!("{} ({})", self.name, self.source),
        }
    }
}

impl std::fmt::Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Builder for constructing [`Task`] instances.
#[derive(Debug, Clone)]
pub struct TaskBuilder {
    name: String,
    definition: TaskDefinition,
    source: String,
    group: Option<String>,
    detail: Option<String>,
}

impl TaskBuilder {
    pub fn new(name: impl Into<String>, definition: TaskDefinition) -> Self {
        Self {
            name: name.into(),
            definition,
            source: "workspace".into(),
            group: None,
            detail: None,
        }
    }

    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Build and validate the task, returning an error if invalid.
    pub fn build(self) -> Result<Task, TaskError> {
        let task = Task {
            name: self.name,
            definition: self.definition,
            source: self.source,
            group: self.group,
            detail: self.detail,
        };
        task.validate()?;
        Ok(task)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskExecution {
    pub id: String,
    pub task: Task,
    pub is_running: bool,
}

impl TaskExecution {
    /// Returns the elapsed display name for this execution.
    pub fn display_name(&self) -> String {
        let status = if self.is_running { "running" } else { "stopped" };
        format!("[{}] {} ({})", self.id, self.task.name, status)
    }
}

impl std::fmt::Display for TaskExecution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

// ── Bridge ──

pub struct TaskBridge {
    providers: Vec<String>,
    executions: Vec<TaskExecution>,
    next_id: u64,
}

impl TaskBridge {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            executions: Vec::new(),
            next_id: 1,
        }
    }

    pub fn register_provider(&mut self, task_type: &str) {
        if !self.providers.contains(&task_type.to_string()) {
            self.providers.push(task_type.to_string());
        }
    }

    pub fn unregister_provider(&mut self, task_type: &str) {
        self.providers.retain(|p| p != task_type);
    }

    pub fn execute_task(&mut self, task: Task) -> String {
        let id = format!("exec-{}", self.next_id);
        self.next_id += 1;
        self.executions.push(TaskExecution {
            id: id.clone(),
            task,
            is_running: true,
        });
        id
    }

    pub fn terminate_task(&mut self, execution_id: &str) -> bool {
        if let Some(exec) = self.executions.iter_mut().find(|e| e.id == execution_id) {
            exec.is_running = false;
            true
        } else {
            false
        }
    }

    pub fn running_tasks(&self) -> Vec<&TaskExecution> {
        self.executions.iter().filter(|e| e.is_running).collect()
    }

    /// Return a list of all registered provider types.
    pub fn providers(&self) -> &[String] {
        &self.providers
    }

    /// Returns true if a provider of the given type is registered.
    pub fn has_provider(&self, task_type: &str) -> bool {
        self.providers.iter().any(|p| p == task_type)
    }

    /// Return the total number of executions (running + stopped).
    pub fn execution_count(&self) -> usize {
        self.executions.len()
    }

    /// Look up an execution by id.
    pub fn get_execution(&self, execution_id: &str) -> Option<&TaskExecution> {
        self.executions.iter().find(|e| e.id == execution_id)
    }

    /// Remove all stopped executions, returning how many were removed.
    pub fn gc_stopped(&mut self) -> usize {
        let before = self.executions.len();
        self.executions.retain(|e| e.is_running);
        before - self.executions.len()
    }

    /// Execute a task with validation, returning an error if the task is invalid.
    pub fn execute_validated(&mut self, task: Task) -> Result<String, TaskError> {
        task.validate()?;
        Ok(self.execute_task(task))
    }

    /// Filter executions by task group.
    pub fn executions_in_group(&self, group: &str) -> Vec<&TaskExecution> {
        self.executions
            .iter()
            .filter(|e| e.task.is_in_group(group))
            .collect()
    }

    /// Terminate all running tasks, returning the count terminated.
    pub fn terminate_all(&mut self) -> usize {
        let mut count = 0;
        for exec in &mut self.executions {
            if exec.is_running {
                exec.is_running = false;
                count += 1;
            }
        }
        count
    }

    pub fn handle_message(&mut self, msg: &TaskMessage) -> serde_json::Value {
        match msg {
            TaskMessage::RegisterProvider { provider_type } => {
                self.register_provider(provider_type);
                serde_json::json!({"registered": true})
            }
            TaskMessage::UnregisterProvider { provider_type } => {
                self.unregister_provider(provider_type);
                serde_json::json!({"unregistered": true})
            }
            TaskMessage::ExecuteTask { task } => {
                let id = self.execute_task(task.clone());
                serde_json::json!({"executionId": id})
            }
            TaskMessage::TerminateTask { execution_id } => {
                let ok = self.terminate_task(execution_id);
                serde_json::json!({"terminated": ok})
            }
            TaskMessage::FetchTasks { filter_type } => {
                serde_json::json!({"filter": filter_type, "tasks": []})
            }
        }
    }
}

impl Default for TaskBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TaskBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TaskBridge(providers={}, executions={}, running={})",
            self.providers.len(),
            self.executions.len(),
            self.running_tasks().len(),
        )
    }
}

/// Initialize the tasks extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

/// Accumulated statistics for ext-tasks operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtTasksStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtTasksStats {
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
    pub fn merge(&mut self, other: &ExtTasksStats) {
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

impl Default for ExtTasksStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtTasksStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtTasksStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-tasks.
#[derive(Debug, Clone)]
pub struct ExtTasksValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtTasksValidator {
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

impl Default for ExtTasksValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ── Task Execution Records ──

/// Record of a completed task execution with timing and exit status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskExecutionRecord {
    pub task_name: String,
    pub task_type: String,
    pub start_time_ms: u64,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub success: bool,
}

impl TaskExecutionRecord {
    pub fn new(task_name: impl Into<String>, task_type: impl Into<String>) -> Self {
        Self {
            task_name: task_name.into(),
            task_type: task_type.into(),
            start_time_ms: 0,
            duration_ms: 0,
            exit_code: None,
            success: false,
        }
    }

    pub fn with_timing(mut self, start_ms: u64, duration_ms: u64) -> Self {
        self.start_time_ms = start_ms;
        self.duration_ms = duration_ms;
        self
    }

    pub fn with_result(mut self, exit_code: i32) -> Self {
        self.exit_code = Some(exit_code);
        self.success = exit_code == 0;
        self
    }

    pub fn mark_success(mut self) -> Self {
        self.success = true;
        self.exit_code = Some(0);
        self
    }

    pub fn mark_failure(mut self, exit_code: i32) -> Self {
        self.success = false;
        self.exit_code = Some(exit_code);
        self
    }
}

impl fmt::Display for TaskExecutionRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.success { "OK" } else { "FAIL" };
        write!(
            f,
            "[{}] {} ({}) {}ms",
            status, self.task_name, self.task_type, self.duration_ms
        )
    }
}

/// History of task execution records.
#[derive(Debug, Clone, Default)]
pub struct TaskExecutionHistory {
    records: Vec<TaskExecutionRecord>,
    max_records: usize,
}

impl TaskExecutionHistory {
    pub fn new(max_records: usize) -> Self {
        Self {
            records: Vec::new(),
            max_records,
        }
    }

    pub fn add(&mut self, record: TaskExecutionRecord) {
        self.records.push(record);
        if self.records.len() > self.max_records {
            self.records.remove(0);
        }
    }

    pub fn records(&self) -> &[TaskExecutionRecord] {
        &self.records
    }

    pub fn successful_count(&self) -> usize {
        self.records.iter().filter(|r| r.success).count()
    }

    pub fn failed_count(&self) -> usize {
        self.records.iter().filter(|r| !r.success).count()
    }

    pub fn last_record(&self) -> Option<&TaskExecutionRecord> {
        self.records.last()
    }

    pub fn by_task_name(&self, name: &str) -> Vec<&TaskExecutionRecord> {
        self.records.iter().filter(|r| r.task_name == name).collect()
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

// ── Problem Matcher ──

/// A problem pattern that matches compiler-style error output.
#[derive(Debug, Clone, PartialEq)]
pub struct ProblemPattern {
    pub regexp: String,
    pub file_group: usize,
    pub line_group: usize,
    pub message_group: usize,
    pub severity_group: Option<usize>,
}

/// A matched problem from task output.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchedProblem {
    pub file: String,
    pub line: u32,
    pub message: String,
    pub severity: ProblemSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemSeverity {
    Error,
    Warning,
    Info,
}

impl fmt::Display for ProblemSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProblemSeverity::Error => write!(f, "error"),
            ProblemSeverity::Warning => write!(f, "warning"),
            ProblemSeverity::Info => write!(f, "info"),
        }
    }
}

impl fmt::Display for MatchedProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {}: {}",
            self.file, self.line, self.severity, self.message
        )
    }
}

/// Parse task output lines against a problem pattern, returning matched problems.
pub fn task_problem_matcher(output: &str, pattern: &ProblemPattern) -> Vec<MatchedProblem> {
    let re = match regex::Regex::new(&pattern.regexp) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    output
        .lines()
        .filter_map(|line| {
            let caps = re.captures(line)?;
            let file = caps.get(pattern.file_group)?.as_str().to_string();
            let line_num: u32 = caps.get(pattern.line_group)?.as_str().parse().ok()?;
            let message = caps.get(pattern.message_group)?.as_str().to_string();
            let severity = if let Some(sg) = pattern.severity_group {
                match caps
                    .get(sg)
                    .map(|m| m.as_str().to_lowercase())
                    .as_deref()
                {
                    Some("warning") | Some("warn") => ProblemSeverity::Warning,
                    Some("info") | Some("note") => ProblemSeverity::Info,
                    _ => ProblemSeverity::Error,
                }
            } else {
                ProblemSeverity::Error
            };
            Some(MatchedProblem {
                file,
                line: line_num,
                message,
                severity,
            })
        })
        .collect()
}

/// Built-in GCC/rustc-style problem pattern.
pub fn gcc_problem_pattern() -> ProblemPattern {
    ProblemPattern {
        regexp: r"^(.+):(\d+):\d+: (error|warning|note): (.+)$".to_string(),
        file_group: 1,
        line_group: 2,
        message_group: 4,
        severity_group: Some(3),
    }
}

// ── Task Dependency Chain ──

/// Manages task dependencies (preLaunchTask chains).
#[derive(Debug, Clone, Default)]
pub struct TaskDependencyChain {
    /// task_name -> list of tasks it depends on (preLaunchTasks)
    dependencies: HashMap<String, Vec<String>>,
}

impl TaskDependencyChain {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a dependency: `task` depends on `depends_on` (preLaunchTask).
    pub fn add_dependency(&mut self, task: impl Into<String>, depends_on: impl Into<String>) {
        self.dependencies
            .entry(task.into())
            .or_default()
            .push(depends_on.into());
    }

    /// Get direct dependencies of a task.
    pub fn dependencies_of(&self, task: &str) -> Vec<&str> {
        self.dependencies
            .get(task)
            .map(|deps| deps.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Compute the full execution order for a task (topological sort, dependencies first).
    /// Returns None if a cycle is detected.
    pub fn execution_order(&self, task: &str) -> Option<Vec<String>> {
        let mut order = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut in_stack = std::collections::HashSet::new();
        if self.topo_visit(task, &mut order, &mut visited, &mut in_stack) {
            Some(order)
        } else {
            None
        }
    }

    fn topo_visit(
        &self,
        task: &str,
        order: &mut Vec<String>,
        visited: &mut std::collections::HashSet<String>,
        in_stack: &mut std::collections::HashSet<String>,
    ) -> bool {
        if in_stack.contains(task) {
            return false;
        } // cycle
        if visited.contains(task) {
            return true;
        }
        in_stack.insert(task.to_string());
        if let Some(deps) = self.dependencies.get(task) {
            for dep in deps {
                if !self.topo_visit(dep, order, visited, in_stack) {
                    return false;
                }
            }
        }
        in_stack.remove(task);
        visited.insert(task.to_string());
        order.push(task.to_string());
        true
    }

    /// Check if a task has any dependencies.
    pub fn has_dependencies(&self, task: &str) -> bool {
        self.dependencies.get(task).is_some_and(|d| !d.is_empty())
    }

    /// Get all tasks that have dependencies registered.
    pub fn all_tasks(&self) -> Vec<&str> {
        self.dependencies.keys().map(|s| s.as_str()).collect()
    }
}

// ── Task Filter ──

/// Filter for querying tasks by type, group, source, and running state.
///
/// Uses a builder pattern: construct with `TaskFilter::new()`, chain
/// predicates, then call `matches()` or `apply()`.
#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    task_type: Option<String>,
    group: Option<String>,
    source: Option<String>,
    name_contains: Option<String>,
    running_only: Option<bool>,
}

impl TaskFilter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Only match tasks whose definition type equals `task_type`.
    pub fn task_type(mut self, task_type: impl Into<String>) -> Self {
        self.task_type = Some(task_type.into());
        self
    }

    /// Only match tasks belonging to the given group.
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Only match tasks from the given source.
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Only match tasks whose name contains the given substring.
    pub fn name_contains(mut self, needle: impl Into<String>) -> Self {
        self.name_contains = Some(needle.into());
        self
    }

    /// When applied to executions, only match running (or stopped) ones.
    pub fn running_only(mut self, running: bool) -> Self {
        self.running_only = Some(running);
        self
    }

    /// Test whether a [`Task`] satisfies this filter.
    pub fn matches_task(&self, task: &Task) -> bool {
        if let Some(ref tt) = self.task_type {
            if task.definition.task_type != *tt {
                return false;
            }
        }
        if let Some(ref g) = self.group {
            if task.group.as_deref() != Some(g.as_str()) {
                return false;
            }
        }
        if let Some(ref s) = self.source {
            if task.source != *s {
                return false;
            }
        }
        if let Some(ref nc) = self.name_contains {
            if !task.name.contains(nc.as_str()) {
                return false;
            }
        }
        true
    }

    /// Test whether a [`TaskExecution`] satisfies this filter.
    pub fn matches_execution(&self, exec: &TaskExecution) -> bool {
        if !self.matches_task(&exec.task) {
            return false;
        }
        if let Some(running) = self.running_only {
            if exec.is_running != running {
                return false;
            }
        }
        true
    }

    /// Filter a slice of tasks, returning those that match.
    pub fn apply<'a>(&self, tasks: &'a [Task]) -> Vec<&'a Task> {
        tasks.iter().filter(|t| self.matches_task(t)).collect()
    }

    /// Filter a slice of executions, returning those that match.
    pub fn apply_executions<'a>(&self, execs: &'a [TaskExecution]) -> Vec<&'a TaskExecution> {
        execs.iter().filter(|e| self.matches_execution(e)).collect()
    }
}

impl fmt::Display for TaskFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if let Some(ref tt) = self.task_type {
            parts.push(format!("type={tt}"));
        }
        if let Some(ref g) = self.group {
            parts.push(format!("group={g}"));
        }
        if let Some(ref s) = self.source {
            parts.push(format!("source={s}"));
        }
        if let Some(ref nc) = self.name_contains {
            parts.push(format!("name~={nc}"));
        }
        if let Some(r) = self.running_only {
            parts.push(format!("running={r}"));
        }
        if parts.is_empty() {
            write!(f, "TaskFilter(*)")
        } else {
            write!(f, "TaskFilter({})", parts.join(", "))
        }
    }
}

// ── Task Group Summary ──

/// Per-group statistics computed from a collection of tasks.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupStats {
    pub group: String,
    pub count: usize,
    pub task_types: Vec<String>,
}

/// Summary of tasks grouped by their `group` field.
#[derive(Debug, Clone)]
pub struct TaskGroupSummary {
    groups: HashMap<String, Vec<String>>,
}

impl TaskGroupSummary {
    /// Build a summary from a slice of tasks.
    pub fn from_tasks(tasks: &[Task]) -> Self {
        let mut groups: HashMap<String, Vec<String>> = HashMap::new();
        for task in tasks {
            let key = task.group.clone().unwrap_or_else(|| "(ungrouped)".into());
            groups.entry(key).or_default().push(task.definition.task_type.clone());
        }
        Self { groups }
    }

    /// Return the number of distinct groups (including ungrouped).
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Return a sorted list of group names.
    pub fn group_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.groups.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    /// Return statistics for a single group, or `None` if the group has no tasks.
    pub fn stats_for(&self, group: &str) -> Option<GroupStats> {
        self.groups.get(group).map(|types| {
            let mut unique: Vec<String> = types.clone();
            unique.sort();
            unique.dedup();
            GroupStats {
                group: group.to_string(),
                count: types.len(),
                task_types: unique,
            }
        })
    }

    /// Total number of tasks across all groups.
    pub fn total_tasks(&self) -> usize {
        self.groups.values().map(|v| v.len()).sum()
    }
}

impl fmt::Display for TaskGroupSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TaskGroupSummary(groups={}, tasks={})",
            self.group_count(),
            self.total_tasks()
        )
    }
}

// ── Task Environment ──

/// Environment variable set for task execution.
///
/// Supports merging multiple layers (system → workspace → task) and resolving
/// `${VAR}` references within values.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TaskEnvironment {
    vars: HashMap<String, String>,
}

impl TaskEnvironment {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create from an iterator of key-value pairs.
    pub fn from_iter(iter: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            vars: iter.into_iter().collect(),
        }
    }

    /// Set a single variable, overwriting any previous value.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.vars.insert(key.into(), value.into());
    }

    /// Get the value of a variable.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(|s| s.as_str())
    }

    /// Remove a variable, returning its former value.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.vars.remove(key)
    }

    /// Return the number of variables.
    pub fn len(&self) -> usize {
        self.vars.len()
    }

    /// Return whether the environment is empty.
    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }

    /// Merge another environment on top of this one.  Values from `other`
    /// overwrite existing keys.
    pub fn merge(&mut self, other: &TaskEnvironment) {
        for (k, v) in &other.vars {
            self.vars.insert(k.clone(), v.clone());
        }
    }

    /// Resolve `${VAR}` references in all values using the variables defined
    /// in this environment.  Unknown references are left as-is.
    /// Returns the number of substitutions made.
    pub fn resolve_references(&mut self) -> usize {
        let snapshot: HashMap<String, String> = self.vars.clone();
        let mut count = 0usize;
        for value in self.vars.values_mut() {
            let mut resolved = value.clone();
            for (k, v) in &snapshot {
                let pattern = format!("${{{}}}", k);
                if resolved.contains(&pattern) {
                    resolved = resolved.replace(&pattern, v);
                    count += 1;
                }
            }
            *value = resolved;
        }
        count
    }

    /// Return a sorted list of variable names.
    pub fn keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.vars.keys().map(|s| s.as_str()).collect();
        keys.sort();
        keys
    }

    /// Convert into a sorted `Vec` of `(key, value)` pairs, suitable for
    /// passing to a process builder.
    pub fn into_sorted_vec(self) -> Vec<(String, String)> {
        let mut pairs: Vec<(String, String)> = self.vars.into_iter().collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        pairs
    }
}

impl fmt::Display for TaskEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TaskEnvironment({} vars)", self.vars.len())
    }
}

impl From<HashMap<String, String>> for TaskEnvironment {
    fn from(map: HashMap<String, String>) -> Self {
        Self { vars: map }
    }
}

impl From<TaskEnvironment> for HashMap<String, String> {
    fn from(env: TaskEnvironment) -> Self {
        env.vars
    }
}

// ---------------------------------------------------------------------------
// TaskDependencyGraph – execution ordering
// ---------------------------------------------------------------------------

/// A directed acyclic graph for task dependencies and execution ordering.
#[derive(Debug, Clone)]
pub struct TaskDependencyGraph {
    /// Map from task name to list of task names it depends on.
    dependencies: HashMap<String, Vec<String>>,
}

impl TaskDependencyGraph {
    pub fn new() -> Self {
        Self { dependencies: HashMap::new() }
    }

    /// Add a task with no dependencies.
    pub fn add_task(&mut self, name: impl Into<String>) {
        let name = name.into();
        self.dependencies.entry(name).or_default();
    }

    /// Add a dependency: `task` depends on `depends_on`.
    pub fn add_dependency(&mut self, task: impl Into<String>, depends_on: impl Into<String>) {
        let task = task.into();
        let dep = depends_on.into();
        self.dependencies.entry(dep.clone()).or_default();
        self.dependencies.entry(task).or_default().push(dep);
    }

    /// Return tasks in topological order (dependencies first).
    pub fn execution_order(&self) -> Result<Vec<String>, String> {
        let mut visited = HashMap::new();
        let mut order = Vec::new();

        for task in self.dependencies.keys() {
            if !visited.contains_key(task.as_str()) {
                self.topo_visit(task, &mut visited, &mut order)?;
            }
        }

        Ok(order)
    }

    fn topo_visit(
        &self,
        task: &str,
        visited: &mut HashMap<String, bool>,
        order: &mut Vec<String>,
    ) -> Result<(), String> {
        if let Some(&in_progress) = visited.get(task) {
            if in_progress {
                return Err(format!("circular dependency detected at: {task}"));
            }
            return Ok(());
        }
        visited.insert(task.to_string(), true);

        if let Some(deps) = self.dependencies.get(task) {
            for dep in deps {
                self.topo_visit(dep, visited, order)?;
            }
        }

        visited.insert(task.to_string(), false);
        order.push(task.to_string());
        Ok(())
    }

    /// Get direct dependencies of a task.
    pub fn dependencies_of(&self, task: &str) -> Vec<&str> {
        self.dependencies
            .get(task)
            .map(|deps| deps.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Tasks with no dependencies (roots).
    pub fn root_tasks(&self) -> Vec<&str> {
        self.dependencies
            .iter()
            .filter(|(_, deps)| deps.is_empty())
            .map(|(name, _)| name.as_str())
            .collect()
    }

    pub fn task_count(&self) -> usize {
        self.dependencies.len()
    }
}

impl Default for TaskDependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TaskDependencyGraph {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TaskDependencyGraph({} tasks)", self.dependencies.len())
    }
}

// ---------------------------------------------------------------------------
// TaskVariableSubstitution – ${workspaceFolder} etc
// ---------------------------------------------------------------------------

/// Substitutes predefined variables in task command strings.
#[derive(Debug, Clone)]
pub struct TaskVariableSubstitution {
    vars: HashMap<String, String>,
}

impl TaskVariableSubstitution {
    pub fn new() -> Self {
        Self { vars: HashMap::new() }
    }

    /// Set a variable value.
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.vars.insert(name.into(), value.into());
    }

    /// Set common VS Code task variables.
    pub fn set_workspace(&mut self, folder: &str, name: &str) {
        self.set("workspaceFolder", folder);
        self.set("workspaceFolderBasename", name);
    }

    /// Set file-related variables.
    pub fn set_file(&mut self, path: &str, dir: &str, basename: &str, ext: &str) {
        self.set("file", path);
        self.set("fileDirname", dir);
        self.set("fileBasename", basename);
        self.set("fileExtname", ext);
    }

    /// Substitute all `${variable}` references in the input string.
    pub fn substitute(&self, input: &str) -> String {
        let mut result = input.to_string();
        for (name, value) in &self.vars {
            let placeholder = format!("${{{name}}}");
            result = result.replace(&placeholder, value);
        }
        result
    }

    /// Count unresolved variables in the string.
    pub fn unresolved_count(&self, input: &str) -> usize {
        let substituted = self.substitute(input);
        substituted.matches("${").count()
    }

    pub fn variable_count(&self) -> usize {
        self.vars.len()
    }
}

impl Default for TaskVariableSubstitution {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TaskVariableSubstitution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TaskVariableSubstitution({} vars)", self.vars.len())
    }
}

// ---------------------------------------------------------------------------
// TaskProblemMatcher – regex-based output parsing
// ---------------------------------------------------------------------------

/// Matches task output lines to extract problem (error/warning) information.
#[derive(Debug, Clone)]
pub struct TaskProblemMatcher {
    pub name: String,
    pub pattern: String,
    pub file_group: usize,
    pub line_group: usize,
    pub message_group: usize,
}

impl TaskProblemMatcher {
    pub fn new(name: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            pattern: pattern.into(),
            file_group: 1,
            line_group: 2,
            message_group: 3,
        }
    }

    /// Create a default problem matcher for Rust compiler output.
    pub fn rust_default() -> Self {
        Self::new("rustc", r"^error\[?\w*\]?:?\s*(.+)")
    }

    /// Create a default problem matcher for GCC-style output.
    pub fn gcc_default() -> Self {
        Self::new("gcc", r"^(.+):(\d+):\d+:\s*(error|warning):\s*(.+)")
    }

    /// Try to match a line and extract a problem.
    pub fn match_line(&self, line: &str) -> Option<TaskProblem> {
        let re = regex::Regex::new(&self.pattern).ok()?;
        let caps = re.captures(line)?;
        Some(TaskProblem {
            file: caps.get(self.file_group).map(|m| m.as_str().to_string()),
            line: caps.get(self.line_group).and_then(|m| m.as_str().parse().ok()),
            message: caps.get(self.message_group).map(|m| m.as_str().to_string())
                .unwrap_or_else(|| caps.get(0).map(|m| m.as_str().to_string()).unwrap_or_default()),
        })
    }
}

/// A problem extracted from task output.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskProblem {
    pub file: Option<String>,
    pub line: Option<u32>,
    pub message: String,
}

impl fmt::Display for TaskProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.file, self.line) {
            (Some(file), Some(line)) => write!(f, "{}:{}: {}", file, line, self.message),
            (Some(file), None) => write!(f, "{}: {}", file, self.message),
            _ => write!(f, "{}", self.message),
        }
    }
}

// ---------------------------------------------------------------------------
// Task terminal assignment
// ---------------------------------------------------------------------------

/// Determines how a task uses terminal panels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskTerminalKind {
    /// Reuse an existing integrated terminal.
    Integrated,
    /// Create a new dedicated terminal.
    Dedicated,
    /// Run in the background without a visible terminal.
    Background,
}

impl fmt::Display for TaskTerminalKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integrated => write!(f, "Integrated"),
            Self::Dedicated => write!(f, "Dedicated"),
            Self::Background => write!(f, "Background"),
        }
    }
}

/// Assignment of a task to a terminal panel.
#[derive(Debug, Clone)]
pub struct TaskTerminalAssignment {
    pub task_name: String,
    pub terminal_kind: TaskTerminalKind,
    pub panel_name: Option<String>,
    pub clear_before_run: bool,
}

impl TaskTerminalAssignment {
    pub fn new(task_name: impl Into<String>, kind: TaskTerminalKind) -> Self {
        Self {
            task_name: task_name.into(),
            terminal_kind: kind,
            panel_name: None,
            clear_before_run: false,
        }
    }

    pub fn with_panel(mut self, panel: impl Into<String>) -> Self {
        self.panel_name = Some(panel.into());
        self
    }

    pub fn with_clear(mut self) -> Self {
        self.clear_before_run = true;
        self
    }

    /// Generate a display label for the terminal tab.
    pub fn terminal_label(&self) -> String {
        self.panel_name.clone().unwrap_or_else(|| format!("Task - {}", self.task_name))
    }
}

impl fmt::Display for TaskTerminalAssignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Terminal({}, {})", self.task_name, self.terminal_kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_task() -> Task {
        Task {
            name: "build".into(),
            definition: TaskDefinition {
                task_type: "shell".into(),
                attributes: serde_json::json!({"command": "cargo build"}),
            },
            source: "workspace".into(),
            group: Some("build".into()),
            detail: None,
        }
    }

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = TaskMessage::ExecuteTask {
            task: test_task(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: TaskMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn task_serialization() {
        let t = test_task();
        let json = serde_json::to_string(&t).unwrap();
        let back: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn bridge_execute_and_terminate() {
        let mut bridge = TaskBridge::new();
        let id = bridge.execute_task(test_task());
        assert_eq!(bridge.running_tasks().len(), 1);
        bridge.terminate_task(&id);
        assert_eq!(bridge.running_tasks().len(), 0);
    }

    #[test]
    fn bridge_register_provider() {
        let mut bridge = TaskBridge::new();
        bridge.register_provider("shell");
        bridge.register_provider("shell");
        assert_eq!(bridge.providers.len(), 1);
    }

    #[test]
    fn bridge_terminate_unknown() {
        let mut bridge = TaskBridge::new();
        assert!(!bridge.terminate_task("nope"));
    }

    #[test]
    fn task_definition_shell_constructor() {
        let def = TaskDefinition::shell("cargo test");
        assert_eq!(def.task_type, "shell");
        assert_eq!(def.shell_command(), Some("cargo test"));
    }

    #[test]
    fn task_definition_process_constructor() {
        let def = TaskDefinition::process("rustc", &["--edition", "2021", "main.rs"]);
        assert_eq!(def.task_type, "process");
        assert_eq!(def.attributes["program"], "rustc");
        assert_eq!(def.attributes["args"][0], "--edition");
    }

    #[test]
    fn task_definition_validate_empty_type() {
        let def = TaskDefinition::new("  ");
        assert_eq!(def.validate(), Err(TaskError::EmptyType));
    }

    #[test]
    fn task_definition_display() {
        let def = TaskDefinition::new("npm");
        assert_eq!(format!("{def}"), "[npm]");
    }

    #[test]
    fn task_validate_empty_name() {
        let task = Task {
            name: "".into(),
            definition: TaskDefinition::new("shell"),
            source: "workspace".into(),
            group: None,
            detail: None,
        };
        assert_eq!(task.validate(), Err(TaskError::EmptyName));
    }

    #[test]
    fn task_label_with_detail() {
        let mut t = test_task();
        t.detail = Some("production".into());
        assert_eq!(t.label(), "build (workspace) - production");
    }

    #[test]
    fn task_is_in_group() {
        let t = test_task();
        assert!(t.is_in_group("build"));
        assert!(!t.is_in_group("test"));
    }

    #[test]
    fn task_builder_success() {
        let task = TaskBuilder::new("lint", TaskDefinition::shell("cargo clippy"))
            .source("user")
            .group("check")
            .detail("runs clippy")
            .build()
            .unwrap();
        assert_eq!(task.name, "lint");
        assert_eq!(task.source, "user");
        assert_eq!(task.group.as_deref(), Some("check"));
        assert_eq!(task.detail.as_deref(), Some("runs clippy"));
    }

    #[test]
    fn task_builder_validation_error() {
        let result = TaskBuilder::new("", TaskDefinition::shell("echo")).build();
        assert!(result.is_err());
    }

    #[test]
    fn bridge_execute_validated_rejects_invalid() {
        let mut bridge = TaskBridge::new();
        let bad_task = Task {
            name: "".into(),
            definition: TaskDefinition::new("shell"),
            source: "ws".into(),
            group: None,
            detail: None,
        };
        assert!(bridge.execute_validated(bad_task).is_err());
        assert_eq!(bridge.execution_count(), 0);
    }

    #[test]
    fn bridge_gc_stopped() {
        let mut bridge = TaskBridge::new();
        let id1 = bridge.execute_task(test_task());
        let _id2 = bridge.execute_task(test_task());
        bridge.terminate_task(&id1);
        assert_eq!(bridge.gc_stopped(), 1);
        assert_eq!(bridge.execution_count(), 1);
    }

    #[test]
    fn bridge_terminate_all() {
        let mut bridge = TaskBridge::new();
        bridge.execute_task(test_task());
        bridge.execute_task(test_task());
        assert_eq!(bridge.terminate_all(), 2);
        assert_eq!(bridge.running_tasks().len(), 0);
    }

    #[test]
    fn bridge_get_execution() {
        let mut bridge = TaskBridge::new();
        let id = bridge.execute_task(test_task());
        assert!(bridge.get_execution(&id).is_some());
        assert!(bridge.get_execution("no-such").is_none());
    }

    #[test]
    fn bridge_has_provider() {
        let mut bridge = TaskBridge::new();
        bridge.register_provider("npm");
        assert!(bridge.has_provider("npm"));
        assert!(!bridge.has_provider("cargo"));
    }

    #[test]
    fn bridge_executions_in_group() {
        let mut bridge = TaskBridge::new();
        bridge.execute_task(test_task()); // group = "build"
        let t2 = TaskBuilder::new("test-all", TaskDefinition::shell("cargo test"))
            .group("test")
            .build()
            .unwrap();
        bridge.execute_task(t2);
        assert_eq!(bridge.executions_in_group("build").len(), 1);
        assert_eq!(bridge.executions_in_group("test").len(), 1);
        assert_eq!(bridge.executions_in_group("deploy").len(), 0);
    }

    #[test]
    fn bridge_display() {
        let bridge = TaskBridge::new();
        let s = format!("{bridge}");
        assert!(s.contains("TaskBridge"));
        assert!(s.contains("providers=0"));
    }

    #[test]
    fn task_error_display() {
        let err = TaskError::EmptyName;
        assert_eq!(format!("{err}"), "task name must not be empty");
        let err2 = TaskError::ExecutionNotFound("x".into());
        assert!(format!("{err2}").contains("x"));
    }

    #[test]
    fn task_execution_display_name() {
        let mut bridge = TaskBridge::new();
        let id = bridge.execute_task(test_task());
        let exec = bridge.get_execution(&id).unwrap();
        let name = exec.display_name();
        assert!(name.contains("running"));
        assert!(name.contains("build"));
    }

    #[test]
    fn handle_message_register_and_unregister() {
        let mut bridge = TaskBridge::new();
        bridge.handle_message(&TaskMessage::RegisterProvider {
            provider_type: "cargo".into(),
        });
        assert!(bridge.has_provider("cargo"));
        bridge.handle_message(&TaskMessage::UnregisterProvider {
            provider_type: "cargo".into(),
        });
        assert!(!bridge.has_provider("cargo"));
    }

    #[test]
    fn ext_tasks_stats_new_defaults() {
        let stats = ExtTasksStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_tasks_stats_record_success() {
        let mut stats = ExtTasksStats::new();
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
    fn ext_tasks_stats_record_failure() {
        let mut stats = ExtTasksStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_tasks_stats_reset() {
        let mut stats = ExtTasksStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_tasks_stats_merge() {
        let mut a = ExtTasksStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtTasksStats::new();
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
    fn ext_tasks_stats_display() {
        let mut stats = ExtTasksStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_tasks_stats_default() {
        let stats = ExtTasksStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_tasks_validator_accepts_valid_name() {
        let v = ExtTasksValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_tasks_validator_rejects_empty() {
        let v = ExtTasksValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_tasks_validator_rejects_too_long() {
        let v = ExtTasksValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_tasks_validator_forbidden_prefix() {
        let v = ExtTasksValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_tasks_validator_allowed_chars() {
        let v = ExtTasksValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_tasks_validator_range() {
        let v = ExtTasksValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_tasks_sanitize_removes_control() {
        let result = ExtTasksValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_tasks_truncate_short_string() {
        assert_eq!(ExtTasksValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_tasks_truncate_long_string() {
        let result = ExtTasksValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_tasks_is_ascii_printable() {
        assert!(ExtTasksValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtTasksValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn execution_record_basic() {
        let r = TaskExecutionRecord::new("build", "shell")
            .with_timing(1000, 500)
            .mark_success();
        assert!(r.success);
        assert_eq!(r.exit_code, Some(0));
        assert_eq!(r.duration_ms, 500);
    }

    #[test]
    fn execution_record_failure() {
        let r = TaskExecutionRecord::new("test", "shell").mark_failure(1);
        assert!(!r.success);
        assert_eq!(r.exit_code, Some(1));
    }

    #[test]
    fn execution_record_display() {
        let r = TaskExecutionRecord::new("build", "shell")
            .with_timing(0, 1234)
            .mark_success();
        let s = format!("{}", r);
        assert!(s.contains("OK"));
        assert!(s.contains("1234ms"));
    }

    #[test]
    fn execution_history_basic() {
        let mut h = TaskExecutionHistory::new(10);
        h.add(TaskExecutionRecord::new("a", "shell").mark_success());
        h.add(TaskExecutionRecord::new("b", "shell").mark_failure(1));
        assert_eq!(h.len(), 2);
        assert_eq!(h.successful_count(), 1);
        assert_eq!(h.failed_count(), 1);
    }

    #[test]
    fn execution_history_max_records() {
        let mut h = TaskExecutionHistory::new(2);
        h.add(TaskExecutionRecord::new("a", "s").mark_success());
        h.add(TaskExecutionRecord::new("b", "s").mark_success());
        h.add(TaskExecutionRecord::new("c", "s").mark_success());
        assert_eq!(h.len(), 2);
        assert_eq!(h.records()[0].task_name, "b");
    }

    #[test]
    fn execution_history_by_name() {
        let mut h = TaskExecutionHistory::new(10);
        h.add(TaskExecutionRecord::new("build", "shell").mark_success());
        h.add(TaskExecutionRecord::new("test", "shell").mark_success());
        h.add(TaskExecutionRecord::new("build", "shell").mark_failure(1));
        assert_eq!(h.by_task_name("build").len(), 2);
    }

    #[test]
    fn problem_matcher_gcc_style() {
        let output =
            "src/main.rs:10:5: error: expected `;`\nsrc/lib.rs:20:1: warning: unused variable";
        let pattern = gcc_problem_pattern();
        let problems = task_problem_matcher(output, &pattern);
        assert_eq!(problems.len(), 2);
        assert_eq!(problems[0].file, "src/main.rs");
        assert_eq!(problems[0].line, 10);
        assert_eq!(problems[0].severity, ProblemSeverity::Error);
        assert_eq!(problems[1].severity, ProblemSeverity::Warning);
    }

    #[test]
    fn problem_matcher_no_match() {
        let output = "Everything is fine\nNo errors here";
        let pattern = gcc_problem_pattern();
        let problems = task_problem_matcher(output, &pattern);
        assert!(problems.is_empty());
    }

    #[test]
    fn dependency_chain_execution_order() {
        let mut chain = TaskDependencyChain::new();
        chain.add_dependency("test", "build");
        chain.add_dependency("build", "compile");
        let order = chain.execution_order("test").unwrap();
        assert_eq!(order, vec!["compile", "build", "test"]);
    }

    #[test]
    fn dependency_chain_cycle_detection() {
        let mut chain = TaskDependencyChain::new();
        chain.add_dependency("a", "b");
        chain.add_dependency("b", "a");
        assert!(chain.execution_order("a").is_none());
    }

    #[test]
    fn dependency_chain_no_deps() {
        let chain = TaskDependencyChain::new();
        let order = chain.execution_order("standalone").unwrap();
        assert_eq!(order, vec!["standalone"]);
    }

    #[test]
    fn dependency_chain_has_dependencies() {
        let mut chain = TaskDependencyChain::new();
        chain.add_dependency("test", "build");
        assert!(chain.has_dependencies("test"));
        assert!(!chain.has_dependencies("build"));
    }

    #[test]
    fn matched_problem_display() {
        let p = MatchedProblem {
            file: "main.rs".into(),
            line: 5,
            message: "oops".into(),
            severity: ProblemSeverity::Error,
        };
        let s = format!("{}", p);
        assert!(s.contains("main.rs:5"));
        assert!(s.contains("error"));
    }

    // ── TaskFilter tests ──

    #[test]
    fn task_filter_matches_all_by_default() {
        let filter = TaskFilter::new();
        let t = test_task();
        assert!(filter.matches_task(&t));
        assert_eq!(format!("{filter}"), "TaskFilter(*)");
    }

    #[test]
    fn task_filter_by_type_and_group() {
        let filter = TaskFilter::new()
            .task_type("shell")
            .group("build");
        let t = test_task(); // type=shell, group=build
        assert!(filter.matches_task(&t));

        let no_match = TaskFilter::new().task_type("process");
        assert!(!no_match.matches_task(&t));
    }

    #[test]
    fn task_filter_by_source_and_name() {
        let filter = TaskFilter::new()
            .source("workspace")
            .name_contains("bui");
        assert!(filter.matches_task(&test_task()));

        let miss = TaskFilter::new().name_contains("deploy");
        assert!(!miss.matches_task(&test_task()));
    }

    #[test]
    fn task_filter_apply_slice() {
        let tasks = vec![
            test_task(),
            TaskBuilder::new("test-all", TaskDefinition::shell("cargo test"))
                .group("test")
                .build()
                .unwrap(),
            TaskBuilder::new("lint", TaskDefinition::new("npm"))
                .group("build")
                .build()
                .unwrap(),
        ];
        let filter = TaskFilter::new().group("build");
        let matched = filter.apply(&tasks);
        assert_eq!(matched.len(), 2);
        assert!(matched.iter().all(|t| t.is_in_group("build")));
    }

    #[test]
    fn task_filter_running_only() {
        let mut bridge = TaskBridge::new();
        let id = bridge.execute_task(test_task());
        bridge.execute_task(test_task());
        bridge.terminate_task(&id);

        let running = TaskFilter::new().running_only(true);
        let stopped = TaskFilter::new().running_only(false);
        assert_eq!(running.apply_executions(&bridge.executions).len(), 1);
        assert_eq!(stopped.apply_executions(&bridge.executions).len(), 1);
    }

    #[test]
    fn task_filter_display_with_predicates() {
        let f = TaskFilter::new().task_type("shell").source("user");
        let s = format!("{f}");
        assert!(s.contains("type=shell"));
        assert!(s.contains("source=user"));
    }

    // ── TaskGroupSummary tests ──

    #[test]
    fn task_group_summary_basic() {
        let tasks = vec![
            test_task(), // group=build, type=shell
            TaskBuilder::new("test-all", TaskDefinition::shell("cargo test"))
                .group("test")
                .build()
                .unwrap(),
            TaskBuilder::new("lint", TaskDefinition::new("npm"))
                .group("build")
                .build()
                .unwrap(),
        ];
        let summary = TaskGroupSummary::from_tasks(&tasks);
        assert_eq!(summary.group_count(), 2);
        assert_eq!(summary.total_tasks(), 3);

        let build_stats = summary.stats_for("build").unwrap();
        assert_eq!(build_stats.count, 2);
        assert!(build_stats.task_types.contains(&"shell".to_string()));
        assert!(build_stats.task_types.contains(&"npm".to_string()));

        let test_stats = summary.stats_for("test").unwrap();
        assert_eq!(test_stats.count, 1);

        assert!(summary.stats_for("deploy").is_none());
    }

    #[test]
    fn task_group_summary_ungrouped() {
        let tasks = vec![Task {
            name: "orphan".into(),
            definition: TaskDefinition::new("shell"),
            source: "ws".into(),
            group: None,
            detail: None,
        }];
        let summary = TaskGroupSummary::from_tasks(&tasks);
        assert!(summary.group_names().contains(&"(ungrouped)"));
        assert_eq!(summary.stats_for("(ungrouped)").unwrap().count, 1);
    }

    #[test]
    fn task_group_summary_display() {
        let summary = TaskGroupSummary::from_tasks(&[test_task()]);
        let s = format!("{summary}");
        assert!(s.contains("groups=1"));
        assert!(s.contains("tasks=1"));
    }

    // ── TaskEnvironment tests ──

    #[test]
    fn task_environment_set_get_remove() {
        let mut env = TaskEnvironment::new();
        assert!(env.is_empty());
        env.set("PATH", "/usr/bin");
        env.set("HOME", "/home/user");
        assert_eq!(env.len(), 2);
        assert_eq!(env.get("PATH"), Some("/usr/bin"));
        assert_eq!(env.remove("PATH"), Some("/usr/bin".into()));
        assert_eq!(env.len(), 1);
        assert_eq!(env.get("PATH"), None);
    }

    #[test]
    fn task_environment_merge_overwrites() {
        let mut base = TaskEnvironment::new();
        base.set("A", "1");
        base.set("B", "2");

        let mut overlay = TaskEnvironment::new();
        overlay.set("B", "overridden");
        overlay.set("C", "3");

        base.merge(&overlay);
        assert_eq!(base.get("A"), Some("1"));
        assert_eq!(base.get("B"), Some("overridden"));
        assert_eq!(base.get("C"), Some("3"));
        assert_eq!(base.len(), 3);
    }

    #[test]
    fn task_environment_resolve_references() {
        let mut env = TaskEnvironment::new();
        env.set("BASE", "/opt");
        env.set("BIN", "${BASE}/bin");
        env.set("LIB", "${BASE}/lib");

        let subs = env.resolve_references();
        assert!(subs >= 2);
        assert_eq!(env.get("BIN"), Some("/opt/bin"));
        assert_eq!(env.get("LIB"), Some("/opt/lib"));
    }

    #[test]
    fn task_environment_unknown_ref_left_as_is() {
        let mut env = TaskEnvironment::new();
        env.set("X", "${UNKNOWN}/path");
        env.resolve_references();
        assert_eq!(env.get("X"), Some("${UNKNOWN}/path"));
    }

    #[test]
    fn task_environment_from_hashmap_and_back() {
        let mut map = HashMap::new();
        map.insert("K".into(), "V".into());
        let env = TaskEnvironment::from(map);
        assert_eq!(env.get("K"), Some("V"));

        let back: HashMap<String, String> = env.into();
        assert_eq!(back.get("K").unwrap(), "V");
    }

    #[test]
    fn task_environment_sorted_output() {
        let mut env = TaskEnvironment::new();
        env.set("Z", "last");
        env.set("A", "first");
        env.set("M", "middle");

        assert_eq!(env.keys(), vec!["A", "M", "Z"]);

        let pairs = env.into_sorted_vec();
        assert_eq!(pairs[0], ("A".into(), "first".into()));
        assert_eq!(pairs[2], ("Z".into(), "last".into()));
    }

    #[test]
    fn task_environment_display() {
        let mut env = TaskEnvironment::new();
        env.set("A", "1");
        env.set("B", "2");
        assert_eq!(format!("{env}"), "TaskEnvironment(2 vars)");
    }

    // -- TaskDependencyGraph -----------------------------------------------

    #[test]
    fn dependency_graph_execution_order() {
        let mut g = TaskDependencyGraph::new();
        g.add_dependency("test", "build");
        g.add_dependency("build", "compile");
        g.add_task("compile");
        let order = g.execution_order().unwrap();
        let compile_pos = order.iter().position(|x| x == "compile").unwrap();
        let build_pos = order.iter().position(|x| x == "build").unwrap();
        let test_pos = order.iter().position(|x| x == "test").unwrap();
        assert!(compile_pos < build_pos);
        assert!(build_pos < test_pos);
    }

    #[test]
    fn dependency_graph_circular_detection() {
        let mut g = TaskDependencyGraph::new();
        g.add_dependency("a", "b");
        g.add_dependency("b", "a");
        assert!(g.execution_order().is_err());
    }

    #[test]
    fn dependency_graph_root_tasks() {
        let mut g = TaskDependencyGraph::new();
        g.add_task("root1");
        g.add_dependency("child", "root1");
        let roots = g.root_tasks();
        assert!(roots.contains(&"root1"));
    }

    #[test]
    fn dependency_graph_display() {
        let g = TaskDependencyGraph::new();
        assert!(format!("{g}").contains("0 tasks"));
    }

    // -- TaskVariableSubstitution ------------------------------------------

    #[test]
    fn variable_substitution_basic() {
        let mut sub = TaskVariableSubstitution::new();
        sub.set("workspaceFolder", "/home/user/project");
        let result = sub.substitute("cd ${workspaceFolder} && make");
        assert_eq!(result, "cd /home/user/project && make");
    }

    #[test]
    fn variable_substitution_workspace() {
        let mut sub = TaskVariableSubstitution::new();
        sub.set_workspace("/home/user/proj", "proj");
        assert_eq!(sub.substitute("${workspaceFolder}"), "/home/user/proj");
        assert_eq!(sub.substitute("${workspaceFolderBasename}"), "proj");
    }

    #[test]
    fn variable_substitution_unresolved() {
        let sub = TaskVariableSubstitution::new();
        assert_eq!(sub.unresolved_count("${a} ${b}"), 2);
    }

    #[test]
    fn variable_substitution_display() {
        let sub = TaskVariableSubstitution::default();
        assert!(format!("{sub}").contains("0 vars"));
    }

    // -- TaskProblemMatcher ------------------------------------------------

    #[test]
    fn problem_matcher_rustc() {
        let m = TaskProblemMatcher::rust_default();
        let problem = m.match_line("error[E0308]: mismatched types");
        assert!(problem.is_some());
    }

    #[test]
    fn problem_display_with_file_and_line() {
        let p = TaskProblem {
            file: Some("main.rs".into()),
            line: Some(42),
            message: "type mismatch".into(),
        };
        let s = format!("{p}");
        assert!(s.contains("main.rs:42"));
    }

    // -- TaskTerminalAssignment --------------------------------------------

    #[test]
    fn terminal_assignment_label() {
        let a = TaskTerminalAssignment::new("build", TaskTerminalKind::Integrated);
        assert_eq!(a.terminal_label(), "Task - build");
    }

    #[test]
    fn terminal_assignment_with_panel() {
        let a = TaskTerminalAssignment::new("test", TaskTerminalKind::Dedicated)
            .with_panel("My Panel");
        assert_eq!(a.terminal_label(), "My Panel");
    }

    #[test]
    fn terminal_kind_display() {
        assert_eq!(format!("{}", TaskTerminalKind::Background), "Background");
    }

    #[test]
    fn terminal_assignment_display() {
        let a = TaskTerminalAssignment::new("lint", TaskTerminalKind::Integrated);
        let s = format!("{a}");
        assert!(s.contains("lint"));
    }
}
