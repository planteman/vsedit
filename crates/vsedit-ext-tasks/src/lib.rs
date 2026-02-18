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

// ---------------------------------------------------------------------------
// TaskShellExecutor - task shell executor
// ---------------------------------------------------------------------------

/// Severity level for task shell executor issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskShellExecutorSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for TaskShellExecutorSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [TaskShellExecutor].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskShellExecutorEntry {
    pub id: String,
    pub label: String,
    pub severity: TaskShellExecutorSeverity,
    pub detail: Option<String>,
    pub task_count: usize,
    enabled: bool,
}

impl TaskShellExecutorEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: TaskShellExecutorSeverity::Low,
            detail: None,
            task_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: TaskShellExecutorSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_task_count(mut self, val: usize) -> Self {
        self.task_count = val;
        self
    }

    pub fn is_running(&self) -> bool {
        self.enabled && self.severity >= TaskShellExecutorSeverity::Medium
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
        format!("[{}] {} ({}): {}", self.severity, self.id, self.task_count, det)
    }
}

impl fmt::Display for TaskShellExecutorEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [TaskShellExecutorEntry] items.
#[derive(Debug, Clone)]
pub struct TaskShellExecutor {
    entries: Vec<TaskShellExecutorEntry>,
    name: String,
    capacity: usize,
}

impl TaskShellExecutor {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: TaskShellExecutorEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<TaskShellExecutorEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&TaskShellExecutorEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn task_count(&self) -> usize { self.entries.len() }

    pub fn is_running(&self) -> bool {
        self.entries.iter().any(|e| e.is_running())
    }

    pub fn entries_by_severity(&self, severity: TaskShellExecutorSeverity) -> Vec<&TaskShellExecutorEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= TaskShellExecutorSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&TaskShellExecutorEntry> {
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

    pub fn enabled_entries(&self) -> Vec<&TaskShellExecutorEntry> {
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
// TaskOutputParser - task output parser
// ---------------------------------------------------------------------------

/// Configuration for [TaskOutputParser].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskOutputParserConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub output_lines: usize,
}

impl TaskOutputParserConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, output_lines: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_output_lines(mut self, val: usize) -> Self { self.output_lines = val; self }
}

impl Default for TaskOutputParserConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [TaskOutputParser].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskOutputParserItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl TaskOutputParserItem {
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

    pub fn has_output(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for TaskOutputParserItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [TaskOutputParserItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct TaskOutputParser {
    config: TaskOutputParserConfig,
    items: Vec<TaskOutputParserItem>,
}

impl TaskOutputParser {
    pub fn new(config: TaskOutputParserConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: TaskOutputParserItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<TaskOutputParserItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&TaskOutputParserItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn output_lines(&self) -> usize { self.items.len() }

    pub fn has_output(&self) -> bool {
        self.items.iter().any(|i| i.has_output())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&TaskOutputParserItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&TaskOutputParserItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &TaskOutputParserConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



// ---------------------------------------------------------------------------
// ext_tasks – Extension protocol helpers
// ---------------------------------------------------------------------------

/// Activation event kinds for extension lifecycle management.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum XExtTasksActivationKind {
    /// Activate on a specific language.
    Language(String),
    /// Activate on a command.
    Command(String),
    /// Activate on a workspace-contains glob.
    WorkspaceContains(String),
    /// Activate on a custom URI scheme.
    UriScheme(String),
    /// Activate on startup.
    Star,
}

impl XExtTasksActivationKind {
    /// Parse an activation event string like `"onLanguage:rust"`.
    pub fn parse(raw: &str) -> Option<Self> {
        if raw == "*" {
            return Some(Self::Star);
        }
        let (kind, value) = raw.split_once(':')?;
        match kind {
            "onLanguage" => Some(Self::Language(value.to_string())),
            "onCommand" => Some(Self::Command(value.to_string())),
            "workspaceContains" => Some(Self::WorkspaceContains(value.to_string())),
            "onUri" => Some(Self::UriScheme(value.to_string())),
            _ => None,
        }
    }

    /// Returns true if this activation kind targets a specific language.
    pub fn is_language(&self) -> bool {
        matches!(self, Self::Language(_))
    }
}

/// Message envelope for extension host RPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XExtTasksRpcEnvelope {
    pub seq: u64,
    pub method: String,
    pub payload: String,
}

impl XExtTasksRpcEnvelope {
    /// Create a new RPC envelope.
    pub fn new(seq: u64, method: impl Into<String>, payload: impl Into<String>) -> Self {
        Self { seq, method: method.into(), payload: payload.into() }
    }

    /// Returns true when the envelope carries a response (method starts with `$/`).
    pub fn is_response(&self) -> bool {
        self.method.starts_with("$/")
    }

    /// Compute a simple checksum of the payload (sum of bytes mod 2^32).
    pub fn payload_checksum(&self) -> u32 {
        self.payload.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32))
    }
}

/// Batch multiple RPC envelopes and return their sequence numbers.
pub fn x_ext_tasks_collect_sequences(envelopes: &[XExtTasksRpcEnvelope]) -> Vec<u64> {
    envelopes.iter().map(|e| e.seq).collect()
}

/// Filter envelopes by method prefix.
pub fn x_ext_tasks_filter_by_method<'a>(
    envelopes: &'a [XExtTasksRpcEnvelope],
    method_prefix: &str,
) -> Vec<&'a XExtTasksRpcEnvelope> {
    envelopes.iter().filter(|e| e.method.starts_with(method_prefix)).collect()
}

/// Deduplicate envelopes by sequence number, keeping the first occurrence.
pub fn x_ext_tasks_dedup_by_seq(envelopes: Vec<XExtTasksRpcEnvelope>) -> Vec<XExtTasksRpcEnvelope> {
    let mut seen = std::collections::HashSet::new();
    envelopes.into_iter().filter(|e| seen.insert(e.seq)).collect()
}

/// Simple capability negotiation: given requested and available feature sets,
/// return the intersection.
pub fn x_ext_tasks_negotiate_capabilities(
    requested: &[&str],
    available: &[&str],
) -> Vec<String> {
    requested.iter()
        .filter(|r| available.contains(r))
        .map(|s| s.to_string())
        .collect()
}

/// Version tuple for extension API compatibility checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct XExtTasksApiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl XExtTasksApiVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
    /// Check if this version satisfies a minimum requirement.
    pub fn satisfies(&self, min: &Self) -> bool {
        (self.major, self.minor, self.patch) >= (min.major, min.minor, min.patch)
    }
}

impl std::fmt::Display for XExtTasksApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}



// ---------------------------------------------------------------------------
// ext_tasks – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for extension task management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YExtTasksTaskGroup {
    Build,
    Test,
    Clean,
    Deploy,
}

impl YExtTasksTaskGroup {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Build => 0,
            Self::Test => 1,
            Self::Clean => 2,
            Self::Deploy => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Build => "Build",
            Self::Test => "Test",
            Self::Clean => "Clean",
            Self::Deploy => "Deploy",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YExtTasksTaskGroup] {
        &[
            YExtTasksTaskGroup::Build,
            YExtTasksTaskGroup::Test,
            YExtTasksTaskGroup::Clean,
            YExtTasksTaskGroup::Deploy,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YExtTasksTaskGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks task execution data.
#[derive(Debug, Clone)]
pub struct YExtTasksTaskExecution {
    pub task_id: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
}

impl YExtTasksTaskExecution {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            task_id: String::new(),
            exit_code: None,
            duration_ms: 0,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YExtTasksTaskExecution({}: {:?})", "task_id", self.task_id)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_ext_tasks_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_ext_tasks_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_ext_tasks_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_ext_tasks_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_ext_tasks_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_ext_tasks_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_ext_tasks_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_ext_tasks_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// ext_tasks – Extended task dependency graph helpers
// ---------------------------------------------------------------------------

/// Priority levels for task dependency graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZExtTasksPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZExtTasksPriority {
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
    pub fn all_asc() -> [ZExtTasksPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZExtTasksPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks task dependency graph data.
#[derive(Debug, Clone)]
pub struct ZExtTasksTaskDependencyGraph {
    pub edges: Vec<(String, String)>,
    pub node_count: usize,
    pub has_cycle: bool,
}

impl ZExtTasksTaskDependencyGraph {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            edges: Vec::new(),
            node_count: 0,
            has_cycle: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.edges.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZExtTasksTaskDependencyGraph[node_count={:?}, has_cycle={:?}]", self.node_count, self.has_cycle)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.has_cycle = !c.has_cycle;
        c
    }
}

/// Compute a simple rolling hash for task dependency graph.
pub fn z_ext_tasks_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_ext_tasks_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_ext_tasks_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_ext_tasks_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_ext_tasks_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_ext_tasks_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_ext_tasks_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 66
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer66 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer66 {
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
pub fn xb_fnv1a_66(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_66<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_66<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_66(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_66(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 71
// ---------------------------------------------------------------------------

/// Generic object pool `Xc71Pool<T>`.
pub struct Xc71Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc71Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc71PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc71Pool<T> {
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
    pub fn stats(&self) -> Xc71PoolStats {
        Xc71PoolStats {
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

impl<T> Default for Xc71Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc71Scheduler`.
pub struct Xc71Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc71Scheduler {
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

impl Default for Xc71Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_71 hash for the given byte slice.
pub fn xc_71_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_71 convention.
pub fn xc_71_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe79 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe79Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe79PipelineError {
    pub stage: Xe79Stage,
    pub message: String,
}

impl std::fmt::Display for Xe79PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe79Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe79Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe79PipelineError>>>,
    stage_names: Vec<Xe79Stage>,
}

impl Xe79Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe79PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe79Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe79PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe79Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe79PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe79Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe79PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe79Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe79PipelineError> {
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

    pub fn compose(mut self, other: Xe79Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe79CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe79CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe79Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe79CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe79CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe79Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe79CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_79_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe79CacheEntry {
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

    fn xe_79_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe79CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_79_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe79PipelineError> {
    Ok(data)
}

pub fn xe_79_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe79PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_79_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe79PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_79_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe79PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_79_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe79PipelineError> {
    Err(Xe79PipelineError {
        stage: Xe79Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_77: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg77Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg77Graph {
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

impl Default for Xg77Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_77: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg77Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg77Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg77Heap<T>) {
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

impl<T: Ord> Default for Xg77Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 70).
pub struct Xh70SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh70SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 112 as u64,
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

/// A compact bit set supporting boolean operations (variant 70).
pub struct Xh70BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh70BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 70).
pub struct Xi70Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi70Deque<T> {
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
pub struct Xi70Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi70Interval {
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

/// A simple interval tree (variant 70).
pub struct Xi70IntervalTree {
    xi_intervals: Vec<Xi70Interval>,
}

impl Xi70IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi70Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi70Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi70Interval) -> Vec<&Xi70Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi70Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi70Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi70Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi70Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi70Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi70Interval> = Vec::new();
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

#[test]
    fn taskshellexecutor_severity_ordering() {
        assert!(TaskShellExecutorSeverity::Critical > TaskShellExecutorSeverity::High);
        assert!(TaskShellExecutorSeverity::High > TaskShellExecutorSeverity::Medium);
        assert!(TaskShellExecutorSeverity::Medium > TaskShellExecutorSeverity::Low);
    }

    #[test]
    fn taskshellexecutor_severity_display() {
        assert_eq!(TaskShellExecutorSeverity::Low.to_string(), "low");
        assert_eq!(TaskShellExecutorSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn taskshellexecutor_entry_creation() {
        let e = TaskShellExecutorEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, TaskShellExecutorSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn taskshellexecutor_entry_builder() {
        let e = TaskShellExecutorEntry::new("e2", "Entry 2")
            .with_severity(TaskShellExecutorSeverity::High)
            .with_detail("some detail")
            .with_task_count(42);
        assert_eq!(e.severity, TaskShellExecutorSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.task_count, 42);
    }

    #[test]
    fn taskshellexecutor_entry_enable_disable() {
        let mut e = TaskShellExecutorEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn taskshellexecutor_add_and_count() {
        let mut mgr = TaskShellExecutor::new("test");
        mgr.add(TaskShellExecutorEntry::new("a", "A"));
        mgr.add(TaskShellExecutorEntry::new("b", "B").with_severity(TaskShellExecutorSeverity::High));
        assert_eq!(mgr.task_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn taskshellexecutor_remove() {
        let mut mgr = TaskShellExecutor::new("test");
        mgr.add(TaskShellExecutorEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn taskshellexecutor_capacity() {
        let mut mgr = TaskShellExecutor::new("test").with_capacity(1);
        assert!(mgr.add(TaskShellExecutorEntry::new("a", "A")));
        assert!(!mgr.add(TaskShellExecutorEntry::new("b", "B")));
    }

    #[test]
    fn taskshellexecutor_sorted_by_severity() {
        let mut mgr = TaskShellExecutor::new("test");
        mgr.add(TaskShellExecutorEntry::new("lo", "Low"));
        mgr.add(TaskShellExecutorEntry::new("hi", "High").with_severity(TaskShellExecutorSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, TaskShellExecutorSeverity::Critical);
    }

    #[test]
    fn taskshellexecutor_summary() {
        let mgr = TaskShellExecutor::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn taskoutputparser_config_defaults() {
        let cfg = TaskOutputParserConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn taskoutputparser_item_creation() {
        let item = TaskOutputParserItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn taskoutputparser_add_and_get() {
        let mut mgr = TaskOutputParser::new(TaskOutputParserConfig::new("test"));
        mgr.add(TaskOutputParserItem::new("k1", "v1"));
        assert_eq!(mgr.output_lines(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn taskoutputparser_remove_item() {
        let mut mgr = TaskOutputParser::new(TaskOutputParserConfig::new("test"));
        mgr.add(TaskOutputParserItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn taskoutputparser_sorted_by_priority() {
        let mut mgr = TaskOutputParser::new(TaskOutputParserConfig::new("test"));
        mgr.add(TaskOutputParserItem::new("lo", "low").with_priority(1));
        mgr.add(TaskOutputParserItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn taskoutputparser_items_with_tag() {
        let mut mgr = TaskOutputParser::new(TaskOutputParserConfig::new("test"));
        mgr.add(TaskOutputParserItem::new("a", "1").with_tag("x"));
        mgr.add(TaskOutputParserItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn taskoutputparser_report() {
        let mgr = TaskOutputParser::new(TaskOutputParserConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    // -- ext_tasks additional tests -------------------------------------------

    #[test]
    fn x_ext_tasks_activation_parse_language() {
        let ak = XExtTasksActivationKind::parse("onLanguage:rust").unwrap();
        assert_eq!(ak, XExtTasksActivationKind::Language("rust".into()));
        assert!(ak.is_language());
    }

    #[test]
    fn x_ext_tasks_activation_parse_command() {
        let ak = XExtTasksActivationKind::parse("onCommand:editor.action.format").unwrap();
        assert_eq!(ak, XExtTasksActivationKind::Command("editor.action.format".into()));
        assert!(!ak.is_language());
    }

    #[test]
    fn x_ext_tasks_activation_parse_star() {
        assert_eq!(XExtTasksActivationKind::parse("*"), Some(XExtTasksActivationKind::Star));
    }

    #[test]
    fn x_ext_tasks_activation_parse_unknown() {
        assert!(XExtTasksActivationKind::parse("badKind:thing").is_none());
    }

    #[test]
    fn x_ext_tasks_activation_parse_workspace() {
        let ak = XExtTasksActivationKind::parse("workspaceContains:**/Cargo.toml").unwrap();
        assert_eq!(ak, XExtTasksActivationKind::WorkspaceContains("**/" .to_owned() + "Cargo.toml"));
    }

    #[test]
    fn x_ext_tasks_rpc_envelope_basic() {
        let env = XExtTasksRpcEnvelope::new(1, "textDocument/didOpen", "{}" );
        assert_eq!(env.seq, 1);
        assert!(!env.is_response());
    }

    #[test]
    fn x_ext_tasks_rpc_envelope_response() {
        let env = XExtTasksRpcEnvelope::new(2, "$/cancelRequest", "");
        assert!(env.is_response());
    }

    #[test]
    fn x_ext_tasks_rpc_payload_checksum() {
        let env = XExtTasksRpcEnvelope::new(1, "m", "AB");
        assert_eq!(env.payload_checksum(), 65 + 66);
    }

    #[test]
    fn x_ext_tasks_collect_sequences_works() {
        let envs = vec![
            XExtTasksRpcEnvelope::new(10, "a", ""),
            XExtTasksRpcEnvelope::new(20, "b", ""),
        ];
        assert_eq!(x_ext_tasks_collect_sequences(&envs), vec![10, 20]);
    }

    #[test]
    fn x_ext_tasks_filter_by_method_works() {
        let envs = vec![
            XExtTasksRpcEnvelope::new(1, "textDocument/open", ""),
            XExtTasksRpcEnvelope::new(2, "workspace/config", ""),
            XExtTasksRpcEnvelope::new(3, "textDocument/close", ""),
        ];
        let filtered = x_ext_tasks_filter_by_method(&envs, "textDocument/");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn x_ext_tasks_dedup_by_seq_works() {
        let envs = vec![
            XExtTasksRpcEnvelope::new(1, "a", "first"),
            XExtTasksRpcEnvelope::new(1, "a", "second"),
            XExtTasksRpcEnvelope::new(2, "b", "third"),
        ];
        let deduped = x_ext_tasks_dedup_by_seq(envs);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].payload, "first");
    }

    #[test]
    fn x_ext_tasks_negotiate_capabilities_basic() {
        let result = x_ext_tasks_negotiate_capabilities(
            &["hover", "completion", "rename"],
            &["hover", "rename", "format"],
        );
        assert_eq!(result, vec!["hover", "rename"]);
    }

    #[test]
    fn x_ext_tasks_api_version_satisfies() {
        let v1 = XExtTasksApiVersion::new(1, 80, 0);
        let min = XExtTasksApiVersion::new(1, 70, 0);
        assert!(v1.satisfies(&min));
        assert!(!min.satisfies(&v1));
    }

    #[test]
    fn x_ext_tasks_api_version_display() {
        let v = XExtTasksApiVersion::new(2, 3, 4);
        assert_eq!(v.to_string(), "2.3.4");
    }

    #[test]
    fn x_ext_tasks_api_version_ord() {
        let v1 = XExtTasksApiVersion::new(1, 0, 0);
        let v2 = XExtTasksApiVersion::new(1, 1, 0);
        assert!(v1 < v2);
    }


    // -- ext_tasks extended domain tests ----------------------------------------

    #[test]
    fn y_ext_tasks_enum_index() {
        assert_eq!(YExtTasksTaskGroup::Build.index(), 0);
        assert_eq!(YExtTasksTaskGroup::Test.index(), 1);
        assert_eq!(YExtTasksTaskGroup::Clean.index(), 2);
        assert_eq!(YExtTasksTaskGroup::Deploy.index(), 3);
    }

    #[test]
    fn y_ext_tasks_enum_label() {
        assert_eq!(YExtTasksTaskGroup::Build.label(), "Build");
        assert_eq!(YExtTasksTaskGroup::Test.label(), "Test");
        assert_eq!(YExtTasksTaskGroup::Clean.label(), "Clean");
        assert_eq!(YExtTasksTaskGroup::Deploy.label(), "Deploy");
    }

    #[test]
    fn y_ext_tasks_enum_all() {
        let all = YExtTasksTaskGroup::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_ext_tasks_enum_is_default() {
        assert!(YExtTasksTaskGroup::Build.is_default());
        assert!(!YExtTasksTaskGroup::Deploy.is_default());
    }

    #[test]
    fn y_ext_tasks_enum_display() {
        assert_eq!(format!("{}", YExtTasksTaskGroup::Build), "Build");
    }

    #[test]
    fn y_ext_tasks_struct_new() {
        let s = YExtTasksTaskExecution::new();
        let _ = s.summary();
    }

    #[test]
    fn y_ext_tasks_fingerprint_deterministic() {
        let h1 = y_ext_tasks_fingerprint("hello");
        let h2 = y_ext_tasks_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_ext_tasks_fingerprint("a"), y_ext_tasks_fingerprint("b"));
    }

    #[test]
    fn y_ext_tasks_truncate_short() {
        assert_eq!(y_ext_tasks_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_ext_tasks_truncate_long() {
        let r = y_ext_tasks_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_ext_tasks_normalize_key_basic() {
        assert_eq!(y_ext_tasks_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_ext_tasks_split_path_basic() {
        let parts = y_ext_tasks_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_ext_tasks_count_occurrences_basic() {
        assert_eq!(y_ext_tasks_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_ext_tasks_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_ext_tasks_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_ext_tasks_in_range_basic() {
        assert!(y_ext_tasks_in_range(5, 1, 10));
        assert!(y_ext_tasks_in_range(1, 1, 10));
        assert!(y_ext_tasks_in_range(10, 1, 10));
        assert!(!y_ext_tasks_in_range(0, 1, 10));
        assert!(!y_ext_tasks_in_range(11, 1, 10));
    }

    #[test]
    fn y_ext_tasks_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_ext_tasks_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_ext_tasks_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_ext_tasks_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- ext_tasks Z-extended tests -----------------------------------------------

    #[test]
    fn z_ext_tasks_priority_weight() {
        assert_eq!(ZExtTasksPriority::Idle.weight(), 0);
        assert_eq!(ZExtTasksPriority::Normal.weight(), 2);
        assert_eq!(ZExtTasksPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_ext_tasks_priority_label() {
        assert_eq!(ZExtTasksPriority::Low.label(), "low");
        assert_eq!(ZExtTasksPriority::High.label(), "high");
    }

    #[test]
    fn z_ext_tasks_priority_is_elevated() {
        assert!(!ZExtTasksPriority::Normal.is_elevated());
        assert!(ZExtTasksPriority::High.is_elevated());
        assert!(ZExtTasksPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_ext_tasks_priority_display() {
        assert_eq!(format!("{}", ZExtTasksPriority::Idle), "idle");
    }

    #[test]
    fn z_ext_tasks_priority_all_asc() {
        let all = ZExtTasksPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZExtTasksPriority::Idle);
        assert_eq!(all[4], ZExtTasksPriority::Realtime);
    }

    #[test]
    fn z_ext_tasks_struct_new() {
        let s = ZExtTasksTaskDependencyGraph::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_ext_tasks_struct_toggled_clone() {
        let s = ZExtTasksTaskDependencyGraph::new();
        let t = s.toggled_clone();
        assert_ne!(s.has_cycle, t.has_cycle);
    }

    #[test]
    fn z_ext_tasks_rolling_hash_deterministic() {
        let h1 = z_ext_tasks_rolling_hash(b"test");
        let h2 = z_ext_tasks_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_ext_tasks_rolling_hash(b"a"), z_ext_tasks_rolling_hash(b"b"));
    }

    #[test]
    fn z_ext_tasks_pad_to_basic() {
        assert_eq!(z_ext_tasks_pad_to("hi", 5), "hi   ");
        assert_eq!(z_ext_tasks_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_ext_tasks_is_identifier_basic() {
        assert!(z_ext_tasks_is_identifier("foo_bar"));
        assert!(z_ext_tasks_is_identifier("abc123"));
        assert!(!z_ext_tasks_is_identifier(""));
        assert!(!z_ext_tasks_is_identifier("has space"));
    }

    #[test]
    fn z_ext_tasks_levenshtein_basic() {
        assert_eq!(z_ext_tasks_levenshtein("", ""), 0);
        assert_eq!(z_ext_tasks_levenshtein("abc", "abc"), 0);
        assert_eq!(z_ext_tasks_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_ext_tasks_unique_words_basic() {
        let w = z_ext_tasks_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_ext_tasks_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_ext_tasks_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_ext_tasks_common_prefix_basic() {
        assert_eq!(z_ext_tasks_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_ext_tasks_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_ext_tasks_struct_clear() {
        let mut s = ZExtTasksTaskDependencyGraph::new();
        s.edges.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_ext_tasks_rolling_hash_empty() {
        let h = z_ext_tasks_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_66_push_and_len() {
        let mut rb = super::XbRingBuffer66::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_66_overwrite() {
        let mut rb = super::XbRingBuffer66::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_66_get_out_of_bounds() {
        let rb = super::XbRingBuffer66::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_66_drain_all() {
        let mut rb = super::XbRingBuffer66::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_66_peek_front_back() {
        let mut rb = super::XbRingBuffer66::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_66_clear() {
        let mut rb = super::XbRingBuffer66::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_66_capacity() {
        let rb = super::XbRingBuffer66::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_66_basic() {
        let h = super::xb_fnv1a_66(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_66(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_66_different_inputs() {
        let h1 = super::xb_fnv1a_66(b"abc");
        let h2 = super::xb_fnv1a_66(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_66_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_66(&data);
        let dec = super::xb_rle_decode_66(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_66_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_66(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_66(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_66_values() {
        assert!((super::xb_clamp_66(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_66(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_66(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_66_values() {
        assert!((super::xb_lerp_66(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_66(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_66(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_66_wrap_around_twice() {
        let mut rb = super::XbRingBuffer66::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 71 ----

    #[test]
    fn xc_71_pool_new_empty() {
        let pool: super::Xc71Pool<i32> = super::Xc71Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_71_pool_release_acquire() {
        let mut pool = super::Xc71Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_71_pool_acquire_empty() {
        let mut pool: super::Xc71Pool<i32> = super::Xc71Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_71_pool_full() {
        let mut pool = super::Xc71Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_71_pool_drain() {
        let mut pool = super::Xc71Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_71_pool_stats() {
        let mut pool = super::Xc71Pool::new(8);
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
    fn xc_71_pool_clear() {
        let mut pool = super::Xc71Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_71_pool_shrink() {
        let mut pool = super::Xc71Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_71_pool_default() {
        let pool: super::Xc71Pool<String> = super::Xc71Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_71_pool_extend() {
        let mut pool = super::Xc71Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_71_pool_retain() {
        let mut pool = super::Xc71Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_71_scheduler_round_robin() {
        let mut sched = super::Xc71Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_71_scheduler_empty() {
        let mut sched = super::Xc71Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_71_scheduler_reset() {
        let mut sched = super::Xc71Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_71_scheduler_add_remove() {
        let mut sched = super::Xc71Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_71_scheduler_targets() {
        let sched = super::Xc71Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_71_hash_empty() {
        assert_eq!(super::xc_71_hash(b""), 5381);
    }

    #[test]
    fn xc_71_hash_data() {
        let h = super::xc_71_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_71_hash(b"hello"), h);
    }

    #[test]
    fn xc_71_reverse_str() {
        assert_eq!(super::xc_71_reverse("abc"), "cba");
        assert_eq!(super::xc_71_reverse(""), "");
    }


    #[test]
    fn xe_79_pipeline_empty() {
        let p = super::Xe79Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_79_pipeline_parse_stage() {
        let p = super::Xe79Pipeline::new()
            .add_parse(super::xe_79_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_79_pipeline_transform_double() {
        let p = super::Xe79Pipeline::new()
            .add_transform(super::xe_79_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_79_pipeline_validate_reverse() {
        let p = super::Xe79Pipeline::new()
            .add_validate(super::xe_79_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_79_pipeline_emit_filter() {
        let p = super::Xe79Pipeline::new()
            .add_emit(super::xe_79_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_79_pipeline_multi_stage() {
        let p = super::Xe79Pipeline::new()
            .add_parse(super::xe_79_pipeline_identity)
            .add_transform(super::xe_79_pipeline_double)
            .add_validate(super::xe_79_pipeline_reverse)
            .add_emit(super::xe_79_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_79_pipeline_error_propagation() {
        let p = super::Xe79Pipeline::new()
            .add_parse(super::xe_79_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe79Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_79_pipeline_compose() {
        let p1 = super::Xe79Pipeline::new()
            .add_parse(super::xe_79_pipeline_identity);
        let p2 = super::Xe79Pipeline::new()
            .add_transform(super::xe_79_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_79_pipeline_error_display() {
        let e = super::Xe79PipelineError {
            stage: super::Xe79Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_79_cache_put_get() {
        let mut c = super::Xe79Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_79_cache_miss() {
        let mut c: super::Xe79Cache<&str, i32> = super::Xe79Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_79_cache_ttl_expiry() {
        let mut c = super::Xe79Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_79_cache_evict() {
        let mut c = super::Xe79Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_79_cache_capacity() {
        let mut c = super::Xe79Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_79_cache_stats() {
        let mut c = super::Xe79Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_79_cache_clear() {
        let mut c = super::Xe79Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_77 graph tests ------------------------------------------------

    #[test]
    fn xg_77_graph_empty() {
        let g = super::Xg77Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_77_graph_add_node() {
        let mut g = super::Xg77Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_77_graph_add_edge() {
        let mut g = super::Xg77Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_77_graph_neighbors() {
        let mut g = super::Xg77Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_77_graph_has_path() {
        let mut g = super::Xg77Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_77_graph_self_path() {
        let g = super::Xg77Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_77_graph_topo_sort() {
        let mut g = super::Xg77Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_77_graph_cycle_detect_false() {
        let mut g = super::Xg77Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_77_graph_cycle_detect_true() {
        let mut g = super::Xg77Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_77 heap tests -------------------------------------------------

    #[test]
    fn xg_77_heap_empty() {
        let h: super::Xg77Heap<i32> = super::Xg77Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_77_heap_push_pop() {
        let mut h = super::Xg77Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_77_heap_peek() {
        let mut h = super::Xg77Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_77_heap_drain_sorted() {
        let mut h = super::Xg77Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_77_heap_merge() {
        let mut a = super::Xg77Heap::new();
        let mut b = super::Xg77Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_77_heap_default() {
        let h: super::Xg77Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_77_graph_default() {
        let g: super::Xg77Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh70_skip_insert_contains() {
        let mut sl = super::Xh70SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh70_skip_remove() {
        let mut sl = super::Xh70SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh70_skip_len() {
        let mut sl = super::Xh70SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh70_skip_range_query() {
        let mut sl = super::Xh70SkipList::xh_new(4);
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
    fn xh70_skip_floor_ceiling() {
        let mut sl = super::Xh70SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh70_skip_rank() {
        let mut sl = super::Xh70SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh70_skip_empty() {
        let sl = super::Xh70SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh70_skip_duplicates() {
        let mut sl = super::Xh70SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh70_bitset_set_test() {
        let mut bs = super::Xh70BitSet::xh_new(256);
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
    fn xh70_bitset_clear_count() {
        let mut bs = super::Xh70BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh70_bitset_and_or_xor() {
        let mut a = super::Xh70BitSet::xh_new(128);
        let mut b = super::Xh70BitSet::xh_new(128);
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
    fn xh70_bitset_iter_ones() {
        let mut bs = super::Xh70BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh70_bitset_first_last() {
        let mut bs = super::Xh70BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh70_bitset_empty() {
        let bs = super::Xh70BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi70_deque_push_pop_back() {
        let mut dq = super::Xi70Deque::xi_new(4);
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
    fn xi70_deque_push_pop_front() {
        let mut dq = super::Xi70Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi70_deque_mixed_ops() {
        let mut dq = super::Xi70Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi70_deque_get_and_split() {
        let mut dq = super::Xi70Deque::xi_new(8);
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
    fn xi70_deque_rotate_left() {
        let mut dq = super::Xi70Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi70_deque_rotate_right() {
        let mut dq = super::Xi70Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi70_deque_grow() {
        let mut dq = super::Xi70Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi70_deque_empty() {
        let dq = super::Xi70Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi70_interval_tree_insert_query() {
        let mut tree = super::Xi70IntervalTree::xi_new();
        tree.xi_insert(super::Xi70Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi70Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi70Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi70_interval_tree_overlap() {
        let mut tree = super::Xi70IntervalTree::xi_new();
        tree.xi_insert(super::Xi70Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi70Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi70Interval::xi_new(12, 20));
        let q = super::Xi70Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi70_interval_tree_remove() {
        let mut tree = super::Xi70IntervalTree::xi_new();
        tree.xi_insert(super::Xi70Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi70Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi70_interval_tree_gaps() {
        let mut tree = super::Xi70IntervalTree::xi_new();
        tree.xi_insert(super::Xi70Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi70Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi70Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi70Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi70Interval::xi_new(8, 10));
    }

    #[test]
    fn xi70_interval_tree_merge() {
        let mut tree = super::Xi70IntervalTree::xi_new();
        tree.xi_insert(super::Xi70Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi70Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi70Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi70Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi70Interval::xi_new(10, 15));
    }

    #[test]
    fn xi70_interval_tree_all() {
        let mut tree = super::Xi70IntervalTree::xi_new();
        tree.xi_insert(super::Xi70Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi70Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi70_interval_tree_empty() {
        let tree = super::Xi70IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi70_interval_tree_contains_point() {
        let iv = super::Xi70Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}
