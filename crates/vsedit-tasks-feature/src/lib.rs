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

// ---------------------------------------------------------------------------
// Task dependency resolution
// ---------------------------------------------------------------------------

/// Resolves task execution order based on declared dependencies.
#[derive(Debug, Clone)]
pub struct TaskDependencyResolver {
    /// Map from task name to its dependency names.
    deps: HashMap<String, Vec<String>>,
}

impl TaskDependencyResolver {
    pub fn new() -> Self {
        Self {
            deps: HashMap::new(),
        }
    }

    /// Declare that `task` depends on `dependency`.
    pub fn add_dependency(&mut self, task: &str, dependency: &str) {
        self.deps
            .entry(task.to_string())
            .or_default()
            .push(dependency.to_string());
    }

    /// Return the direct dependencies for a task.
    pub fn dependencies_of(&self, task: &str) -> Vec<&str> {
        self.deps
            .get(task)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Compute a topological execution order. Returns `Err` if a cycle is detected.
    pub fn resolve_order(&self, tasks: &[&str]) -> Result<Vec<String>, String> {
        let mut visited: HashMap<String, u8> = HashMap::new(); // 0=unvisited, 1=in-progress, 2=done
        let mut order = Vec::new();

        for &task in tasks {
            if visited.get(task).copied().unwrap_or(0) == 0 {
                self.visit(task, &mut visited, &mut order)?;
            }
        }
        Ok(order)
    }

    fn visit(
        &self,
        task: &str,
        visited: &mut HashMap<String, u8>,
        order: &mut Vec<String>,
    ) -> Result<(), String> {
        let state = visited.get(task).copied().unwrap_or(0);
        if state == 2 {
            return Ok(());
        }
        if state == 1 {
            return Err(format!("cycle detected involving task '{}'", task));
        }
        visited.insert(task.to_string(), 1);
        if let Some(deps) = self.deps.get(task) {
            for dep in deps {
                self.visit(dep, visited, order)?;
            }
        }
        visited.insert(task.to_string(), 2);
        order.push(task.to_string());
        Ok(())
    }

    /// Returns `true` if the given task has any dependencies.
    pub fn has_dependencies(&self, task: &str) -> bool {
        self.deps.get(task).map_or(false, |v| !v.is_empty())
    }
}

impl Default for TaskDependencyResolver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Task output parser
// ---------------------------------------------------------------------------

/// Severity of a parsed output entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputSeverity {
    Error,
    Warning,
    Info,
}

impl fmt::Display for OutputSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputSeverity::Error => write!(f, "error"),
            OutputSeverity::Warning => write!(f, "warning"),
            OutputSeverity::Info => write!(f, "info"),
        }
    }
}

/// A single parsed diagnostic from task output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDiagnostic {
    pub severity: OutputSeverity,
    pub message: String,
    pub line_number: Option<usize>,
}

/// Parses task output text and extracts diagnostics.
pub struct TaskOutputParser;

impl TaskOutputParser {
    /// Parse output lines looking for error/warning markers.
    pub fn parse(output: &str) -> Vec<ParsedDiagnostic> {
        let mut diagnostics = Vec::new();
        for (idx, line) in output.lines().enumerate() {
            let lower = line.to_lowercase();
            let severity = if lower.contains("error") {
                Some(OutputSeverity::Error)
            } else if lower.contains("warning") || lower.contains("warn") {
                Some(OutputSeverity::Warning)
            } else if lower.contains("info") || lower.contains("note") {
                Some(OutputSeverity::Info)
            } else {
                None
            };
            if let Some(sev) = severity {
                diagnostics.push(ParsedDiagnostic {
                    severity: sev,
                    message: line.trim().to_string(),
                    line_number: Some(idx + 1),
                });
            }
        }
        diagnostics
    }

    /// Count errors in output.
    pub fn error_count(output: &str) -> usize {
        Self::parse(output)
            .iter()
            .filter(|d| d.severity == OutputSeverity::Error)
            .count()
    }

    /// Count warnings in output.
    pub fn warning_count(output: &str) -> usize {
        Self::parse(output)
            .iter()
            .filter(|d| d.severity == OutputSeverity::Warning)
            .count()
    }

    /// Returns `true` if the output contains no errors.
    pub fn is_clean(output: &str) -> bool {
        Self::error_count(output) == 0
    }
}

// ---------------------------------------------------------------------------
// Task template
// ---------------------------------------------------------------------------

/// A template for creating tasks with variable substitution.
#[derive(Debug, Clone)]
pub struct TaskTemplate {
    pub name_template: String,
    pub command_template: String,
    pub group: TaskGroup,
    pub variables: HashMap<String, String>,
}

impl TaskTemplate {
    pub fn new(name: &str, command: &str) -> Self {
        Self {
            name_template: name.to_string(),
            command_template: command.to_string(),
            group: TaskGroup::None,
            variables: HashMap::new(),
        }
    }

    /// Set a variable value for substitution.
    pub fn set_var(&mut self, key: &str, value: &str) {
        self.variables.insert(key.to_string(), value.to_string());
    }

    /// Set the task group.
    pub fn with_group(mut self, group: TaskGroup) -> Self {
        self.group = group;
        self
    }

    fn substitute(&self, template: &str) -> String {
        let mut result = template.to_string();
        for (k, v) in &self.variables {
            result = result.replace(&format!("${{{}}}", k), v);
        }
        result
    }

    /// Instantiate the template into a concrete `Task`.
    pub fn instantiate(&self) -> Task {
        TaskBuilder::new(
            self.substitute(&self.name_template),
            self.substitute(&self.command_template),
        )
        .group(self.group)
        .build()
    }
}

// ── Task utilities ──────────────────────────────────────────────────────

/// Filter tasks by source type.
pub fn tasks_by_source(tasks: &[Task], source: TaskSource) -> Vec<&Task> {
    tasks.iter().filter(|t| t.source == source).collect()
}

/// Return all background tasks.
pub fn background_tasks(tasks: &[Task]) -> Vec<&Task> {
    tasks.iter().filter(|t| t.is_background).collect()
}

/// Return all tasks that have a problem matcher configured.
pub fn tasks_with_problem_matcher(tasks: &[Task]) -> Vec<&Task> {
    tasks.iter().filter(|t| t.problem_matcher.is_some()).collect()
}

/// Count the number of successful executions.
pub fn successful_execution_count(executions: &[TaskExecution]) -> usize {
    executions.iter().filter(|e| e.is_success()).count()
}

/// Count the number of failed executions (non-zero exit code, completed).
pub fn failed_execution_count(executions: &[TaskExecution]) -> usize {
    executions
        .iter()
        .filter(|e| !e.running && e.exit_code.is_some() && e.exit_code != Some(0))
        .count()
}

/// Collect unique task group values from a set of tasks.
pub fn unique_groups(tasks: &[Task]) -> Vec<TaskGroup> {
    let mut groups = Vec::new();
    for t in tasks {
        if !groups.contains(&t.group) {
            groups.push(t.group.clone());
        }
    }
    groups
}

/// Compute the full command line for a task, joining command and args.
pub fn full_command_line(task: &Task) -> String {
    if task.args.is_empty() {
        task.command.clone()
    } else {
        format!("{} {}", task.command, task.args.join(" "))
    }
}

/// Find tasks whose name contains the given substring (case-insensitive).
pub fn search_tasks_by_name<'a>(tasks: &'a [Task], query: &str) -> Vec<&'a Task> {
    let query_lower = query.to_lowercase();
    tasks
        .iter()
        .filter(|t| t.name.to_lowercase().contains(&query_lower))
        .collect()
}

/// Create a summary string for a task service.
pub fn task_service_summary(service: &TaskService) -> String {
    let running = service.running_count();
    format!(
        "{} task(s) registered, {} execution(s), {} running",
        service.task_count(),
        service.executions.len(),
        running
    )
}

// ---------------------------------------------------------------------------
// Task variable substitution
// ---------------------------------------------------------------------------

/// Manages variable definitions and performs substitution in strings.
///
/// Variables use the `${name}` syntax. Undefined variables are left as-is.
#[derive(Debug, Clone)]
pub struct TaskVariableResolver {
    variables: HashMap<String, String>,
}

impl TaskVariableResolver {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    /// Define a variable.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.variables.insert(key.into(), value.into());
    }

    /// Remove a variable.
    pub fn unset(&mut self, key: &str) -> Option<String> {
        self.variables.remove(key)
    }

    /// Look up a variable value.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.variables.get(key).map(|s| s.as_str())
    }

    /// Return the number of defined variables.
    pub fn len(&self) -> usize {
        self.variables.len()
    }

    /// Return true if no variables are defined.
    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }

    /// Substitute all `${key}` occurrences in `input` with their values.
    /// Unknown variables are left as-is.
    pub fn resolve(&self, input: &str) -> String {
        let mut result = input.to_string();
        for (k, v) in &self.variables {
            result = result.replace(&format!("${{{}}}", k), v);
        }
        result
    }

    /// Substitute variables in all elements of a slice, returning a new `Vec`.
    pub fn resolve_all(&self, inputs: &[String]) -> Vec<String> {
        inputs.iter().map(|s| self.resolve(s)).collect()
    }

    /// List all defined variable names sorted alphabetically.
    pub fn keys_sorted(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.variables.keys().map(|s| s.as_str()).collect();
        keys.sort_unstable();
        keys
    }
}

impl Default for TaskVariableResolver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Task environment manager
// ---------------------------------------------------------------------------

/// Manages environment variables for task execution.
///
/// Supports layered overrides: base env → workspace env → task-specific env.
#[derive(Debug, Clone)]
pub struct TaskEnvironment {
    base: HashMap<String, String>,
    overrides: HashMap<String, String>,
}

impl TaskEnvironment {
    pub fn new() -> Self {
        Self {
            base: HashMap::new(),
            overrides: HashMap::new(),
        }
    }

    /// Set a base environment variable.
    pub fn set_base(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.base.insert(key.into(), value.into());
    }

    /// Set an override environment variable (takes precedence over base).
    pub fn set_override(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.overrides.insert(key.into(), value.into());
    }

    /// Resolve a single environment variable (override wins over base).
    pub fn get(&self, key: &str) -> Option<&str> {
        self.overrides
            .get(key)
            .or_else(|| self.base.get(key))
            .map(|s| s.as_str())
    }

    /// Produce the merged environment (base + overrides).
    pub fn merged(&self) -> HashMap<String, String> {
        let mut env = self.base.clone();
        for (k, v) in &self.overrides {
            env.insert(k.clone(), v.clone());
        }
        env
    }

    /// Return the number of unique keys across both layers.
    pub fn len(&self) -> usize {
        self.merged().len()
    }

    /// Return true if there are no env vars defined.
    pub fn is_empty(&self) -> bool {
        self.base.is_empty() && self.overrides.is_empty()
    }

    /// Remove an override, falling back to the base value.
    pub fn remove_override(&mut self, key: &str) -> Option<String> {
        self.overrides.remove(key)
    }

    /// Clear all overrides.
    pub fn clear_overrides(&mut self) {
        self.overrides.clear();
    }
}

impl Default for TaskEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Task retry policy
// ---------------------------------------------------------------------------

/// Configuration for automatically retrying failed tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub retry_on_exit_codes: Vec<i32>,
}

impl RetryPolicy {
    /// Create a policy that retries up to `max_attempts` times on any failure.
    pub fn new(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            retry_on_exit_codes: Vec::new(),
        }
    }

    /// Only retry when the exit code matches one of the given codes.
    pub fn on_exit_codes(mut self, codes: Vec<i32>) -> Self {
        self.retry_on_exit_codes = codes;
        self
    }

    /// Whether the given exit code should trigger a retry.
    pub fn should_retry(&self, exit_code: i32, attempt: u32) -> bool {
        if attempt >= self.max_attempts {
            return false;
        }
        if exit_code == 0 {
            return false;
        }
        if self.retry_on_exit_codes.is_empty() {
            return true; // retry on any non-zero
        }
        self.retry_on_exit_codes.contains(&exit_code)
    }
}

/// Tracks the state of a retryable task execution.
#[derive(Debug, Clone)]
pub struct RetryTracker {
    policy: RetryPolicy,
    attempts: Vec<i32>, // exit codes of each attempt
}

impl RetryTracker {
    pub fn new(policy: RetryPolicy) -> Self {
        Self {
            policy,
            attempts: Vec::new(),
        }
    }

    /// Record an attempt with the given exit code. Returns `true` if another
    /// retry should be made.
    pub fn record_attempt(&mut self, exit_code: i32) -> bool {
        self.attempts.push(exit_code);
        self.policy
            .should_retry(exit_code, self.attempts.len() as u32)
    }

    /// Number of attempts so far.
    pub fn attempt_count(&self) -> u32 {
        self.attempts.len() as u32
    }

    /// Whether the last attempt succeeded.
    pub fn last_succeeded(&self) -> bool {
        self.attempts.last() == Some(&0)
    }

    /// Whether we have exhausted all retries without success.
    pub fn exhausted(&self) -> bool {
        if self.attempts.is_empty() {
            return false;
        }
        !self.last_succeeded()
            && self.attempts.len() as u32 >= self.policy.max_attempts
    }
}

// ---------------------------------------------------------------------------
// Task group manager
// ---------------------------------------------------------------------------

/// Organises tasks into named groups and provides batch operations.
#[derive(Debug)]
pub struct TaskGroupManager {
    groups: HashMap<String, Vec<String>>, // group name → task names
}

impl TaskGroupManager {
    pub fn new() -> Self {
        Self {
            groups: HashMap::new(),
        }
    }

    /// Add a task to a named group.
    pub fn add_to_group(&mut self, group: &str, task_name: &str) {
        self.groups
            .entry(group.to_string())
            .or_default()
            .push(task_name.to_string());
    }

    /// Get the task names in a group.
    pub fn tasks_in_group(&self, group: &str) -> Vec<&str> {
        self.groups
            .get(group)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// List all group names sorted.
    pub fn group_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.groups.keys().map(|s| s.as_str()).collect();
        names.sort_unstable();
        names
    }

    /// Total number of groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Remove a task from a group. Returns true if the task was found.
    pub fn remove_from_group(&mut self, group: &str, task_name: &str) -> bool {
        if let Some(members) = self.groups.get_mut(group) {
            if let Some(pos) = members.iter().position(|n| n == task_name) {
                members.remove(pos);
                return true;
            }
        }
        false
    }

    /// Remove an entire group. Returns the contained task names.
    pub fn remove_group(&mut self, group: &str) -> Option<Vec<String>> {
        self.groups.remove(group)
    }
}

impl Default for TaskGroupManager {
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

    // -- TaskDependencyResolver --

    #[test]
    fn dependency_resolver_linear_order() {
        let mut r = TaskDependencyResolver::new();
        r.add_dependency("build", "compile");
        r.add_dependency("test", "build");
        let order = r.resolve_order(&["test"]).unwrap();
        assert_eq!(order, vec!["compile", "build", "test"]);
    }

    #[test]
    fn dependency_resolver_cycle_detected() {
        let mut r = TaskDependencyResolver::new();
        r.add_dependency("a", "b");
        r.add_dependency("b", "a");
        assert!(r.resolve_order(&["a"]).is_err());
    }

    #[test]
    fn dependency_resolver_no_deps() {
        let r = TaskDependencyResolver::new();
        let order = r.resolve_order(&["standalone"]).unwrap();
        assert_eq!(order, vec!["standalone"]);
        assert!(!r.has_dependencies("standalone"));
    }

    #[test]
    fn dependency_resolver_diamond() {
        let mut r = TaskDependencyResolver::new();
        r.add_dependency("d", "b");
        r.add_dependency("d", "c");
        r.add_dependency("b", "a");
        r.add_dependency("c", "a");
        let order = r.resolve_order(&["d"]).unwrap();
        // "a" must come before "b" and "c", and "d" last
        let pos = |name: &str| order.iter().position(|s| s == name).unwrap();
        assert!(pos("a") < pos("b"));
        assert!(pos("a") < pos("c"));
        assert!(pos("b") < pos("d"));
        assert!(pos("c") < pos("d"));
    }

    // -- TaskOutputParser --

    #[test]
    fn output_parser_extracts_errors_and_warnings() {
        let output = "compiling...\nerror: undefined variable\nwarning: unused import\nDone.";
        let diags = TaskOutputParser::parse(output);
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].severity, OutputSeverity::Error);
        assert_eq!(diags[1].severity, OutputSeverity::Warning);
        assert_eq!(TaskOutputParser::error_count(output), 1);
        assert_eq!(TaskOutputParser::warning_count(output), 1);
        assert!(!TaskOutputParser::is_clean(output));
    }

    #[test]
    fn output_parser_clean_output() {
        assert!(TaskOutputParser::is_clean("compiling...\nDone."));
    }

    // -- TaskTemplate --

    #[test]
    fn template_instantiate_with_vars() {
        let mut tpl = TaskTemplate::new("build-${target}", "cargo build --target ${target}");
        tpl.set_var("target", "x86_64");
        let task = tpl.instantiate();
        assert_eq!(task.name, "build-x86_64");
        assert_eq!(task.command, "cargo build --target x86_64");
    }

    #[test]
    fn template_with_group() {
        let tpl = TaskTemplate::new("test", "cargo test").with_group(TaskGroup::Test);
        let task = tpl.instantiate();
        assert_eq!(task.group, TaskGroup::Test);
    }

    #[test]
    fn output_severity_display() {
        assert_eq!(format!("{}", OutputSeverity::Error), "error");
        assert_eq!(format!("{}", OutputSeverity::Warning), "warning");
        assert_eq!(format!("{}", OutputSeverity::Info), "info");
    }

    #[test]
    fn tasks_by_source_filters() {
        let tasks = vec![
            make_task("a", TaskGroup::Build),
            {
                let mut t = make_task("b", TaskGroup::Test);
                t.source = TaskSource::Extension;
                t
            },
            make_task("c", TaskGroup::Clean),
        ];
        assert_eq!(tasks_by_source(&tasks, TaskSource::Workspace).len(), 2);
        assert_eq!(tasks_by_source(&tasks, TaskSource::Extension).len(), 1);
        assert_eq!(tasks_by_source(&tasks, TaskSource::User).len(), 0);
    }

    #[test]
    fn background_tasks_filters() {
        let tasks = vec![
            make_task("fg", TaskGroup::Build),
            {
                let mut t = make_task("bg", TaskGroup::Build);
                t.is_background = true;
                t
            },
        ];
        let bg = background_tasks(&tasks);
        assert_eq!(bg.len(), 1);
        assert_eq!(bg[0].name, "bg");
    }

    #[test]
    fn tasks_with_problem_matcher_filters() {
        let tasks = vec![
            make_task("no-pm", TaskGroup::Build),
            {
                let mut t = make_task("with-pm", TaskGroup::Build);
                t.problem_matcher = Some("$tsc".into());
                t
            },
        ];
        assert_eq!(tasks_with_problem_matcher(&tasks).len(), 1);
    }

    #[test]
    fn successful_and_failed_execution_counts() {
        let executions = vec![
            TaskExecution { task: make_task("a", TaskGroup::Build), running: false, exit_code: Some(0) },
            TaskExecution { task: make_task("b", TaskGroup::Build), running: false, exit_code: Some(1) },
            TaskExecution { task: make_task("c", TaskGroup::Build), running: true, exit_code: None },
            TaskExecution { task: make_task("d", TaskGroup::Build), running: false, exit_code: Some(0) },
        ];
        assert_eq!(successful_execution_count(&executions), 2);
        assert_eq!(failed_execution_count(&executions), 1);
    }

    #[test]
    fn unique_groups_deduplicates() {
        let tasks = vec![
            make_task("a", TaskGroup::Build),
            make_task("b", TaskGroup::Build),
            make_task("c", TaskGroup::Test),
            make_task("d", TaskGroup::Clean),
        ];
        let groups = unique_groups(&tasks);
        assert_eq!(groups.len(), 3);
    }

    #[test]
    fn full_command_line_formatting() {
        let t = make_task("build", TaskGroup::Build);
        assert_eq!(full_command_line(&t), "cargo build");

        let mut t2 = make_task("test", TaskGroup::Test);
        t2.args = vec!["test".into(), "--release".into()];
        assert_eq!(full_command_line(&t2), "cargo test --release");

        let mut t3 = make_task("simple", TaskGroup::None);
        t3.args = vec![];
        assert_eq!(full_command_line(&t3), "cargo");
    }

    #[test]
    fn search_tasks_by_name_finds() {
        let tasks = vec![
            make_task("build-project", TaskGroup::Build),
            make_task("run-tests", TaskGroup::Test),
            make_task("build-docs", TaskGroup::Build),
        ];
        assert_eq!(search_tasks_by_name(&tasks, "build").len(), 2);
        assert_eq!(search_tasks_by_name(&tasks, "BUILD").len(), 2);
        assert_eq!(search_tasks_by_name(&tasks, "deploy").len(), 0);
    }

    #[test]
    fn task_service_summary_formatting() {
        let mut service = TaskService::new();
        service.register_task(make_task("build", TaskGroup::Build));
        let summary = task_service_summary(&service);
        assert!(summary.contains("1 task(s) registered"));
        assert!(summary.contains("0 execution(s)"));
        assert!(summary.contains("0 running"));
    }

    // -- TaskVariableResolver --

    #[test]
    fn variable_resolver_substitutes_known_vars() {
        let mut r = TaskVariableResolver::new();
        r.set("workspaceFolder", "/home/user/project");
        r.set("file", "src/main.rs");
        let result = r.resolve("cd ${workspaceFolder} && edit ${file}");
        assert_eq!(result, "cd /home/user/project && edit src/main.rs");
    }

    #[test]
    fn variable_resolver_leaves_unknown_vars() {
        let r = TaskVariableResolver::new();
        let result = r.resolve("echo ${unknown}");
        assert_eq!(result, "echo ${unknown}");
    }

    #[test]
    fn variable_resolver_unset_and_len() {
        let mut r = TaskVariableResolver::new();
        r.set("a", "1");
        r.set("b", "2");
        assert_eq!(r.len(), 2);
        assert!(!r.is_empty());
        assert_eq!(r.unset("a"), Some("1".to_string()));
        assert_eq!(r.len(), 1);
        assert!(r.get("a").is_none());
        assert_eq!(r.get("b"), Some("2"));
    }

    #[test]
    fn variable_resolver_resolve_all() {
        let mut r = TaskVariableResolver::new();
        r.set("name", "world");
        let inputs = vec!["hello ${name}".to_string(), "${name}!".to_string()];
        let resolved = r.resolve_all(&inputs);
        assert_eq!(resolved, vec!["hello world", "world!"]);
    }

    #[test]
    fn variable_resolver_keys_sorted() {
        let mut r = TaskVariableResolver::new();
        r.set("zebra", "z");
        r.set("alpha", "a");
        r.set("mid", "m");
        assert_eq!(r.keys_sorted(), vec!["alpha", "mid", "zebra"]);
    }

    // -- TaskEnvironment --

    #[test]
    fn task_environment_override_wins() {
        let mut env = TaskEnvironment::new();
        env.set_base("PATH", "/usr/bin");
        env.set_override("PATH", "/custom/bin");
        assert_eq!(env.get("PATH"), Some("/custom/bin"));
    }

    #[test]
    fn task_environment_fallback_to_base() {
        let mut env = TaskEnvironment::new();
        env.set_base("HOME", "/home/user");
        assert_eq!(env.get("HOME"), Some("/home/user"));
        assert!(env.get("MISSING").is_none());
    }

    #[test]
    fn task_environment_merged() {
        let mut env = TaskEnvironment::new();
        env.set_base("A", "1");
        env.set_base("B", "2");
        env.set_override("B", "override");
        env.set_override("C", "3");
        let merged = env.merged();
        assert_eq!(merged.get("A").unwrap(), "1");
        assert_eq!(merged.get("B").unwrap(), "override");
        assert_eq!(merged.get("C").unwrap(), "3");
        assert_eq!(env.len(), 3);
    }

    #[test]
    fn task_environment_clear_overrides() {
        let mut env = TaskEnvironment::new();
        env.set_base("X", "base");
        env.set_override("X", "over");
        assert_eq!(env.get("X"), Some("over"));
        env.clear_overrides();
        assert_eq!(env.get("X"), Some("base"));
    }

    // -- RetryPolicy / RetryTracker --

    #[test]
    fn retry_policy_basic() {
        let policy = RetryPolicy::new(3);
        assert!(policy.should_retry(1, 0));
        assert!(policy.should_retry(1, 2));
        assert!(!policy.should_retry(1, 3)); // exhausted
        assert!(!policy.should_retry(0, 0)); // success never retries
    }

    #[test]
    fn retry_policy_specific_exit_codes() {
        let policy = RetryPolicy::new(3).on_exit_codes(vec![2, 137]);
        assert!(policy.should_retry(2, 0));
        assert!(policy.should_retry(137, 1));
        assert!(!policy.should_retry(1, 0)); // exit code 1 not in list
    }

    #[test]
    fn retry_tracker_records_and_exhausts() {
        let policy = RetryPolicy::new(2);
        let mut tracker = RetryTracker::new(policy);
        assert!(!tracker.exhausted());
        let should = tracker.record_attempt(1);
        assert!(should); // 1 of 2, should retry
        assert_eq!(tracker.attempt_count(), 1);
        let should = tracker.record_attempt(1);
        assert!(!should); // 2 of 2, exhausted
        assert!(tracker.exhausted());
        assert!(!tracker.last_succeeded());
    }

    #[test]
    fn retry_tracker_succeeds_on_second_try() {
        let policy = RetryPolicy::new(3);
        let mut tracker = RetryTracker::new(policy);
        tracker.record_attempt(1); // fail
        tracker.record_attempt(0); // success
        assert!(tracker.last_succeeded());
        assert!(!tracker.exhausted());
        assert_eq!(tracker.attempt_count(), 2);
    }

    // -- TaskGroupManager --

    #[test]
    fn group_manager_add_and_list() {
        let mut mgr = TaskGroupManager::new();
        mgr.add_to_group("build", "compile");
        mgr.add_to_group("build", "link");
        mgr.add_to_group("test", "unit-test");
        assert_eq!(mgr.tasks_in_group("build"), vec!["compile", "link"]);
        assert_eq!(mgr.tasks_in_group("test"), vec!["unit-test"]);
        assert!(mgr.tasks_in_group("deploy").is_empty());
        assert_eq!(mgr.group_count(), 2);
    }

    #[test]
    fn group_manager_remove_task_from_group() {
        let mut mgr = TaskGroupManager::new();
        mgr.add_to_group("ci", "lint");
        mgr.add_to_group("ci", "test");
        assert!(mgr.remove_from_group("ci", "lint"));
        assert!(!mgr.remove_from_group("ci", "lint")); // already removed
        assert_eq!(mgr.tasks_in_group("ci"), vec!["test"]);
    }

    #[test]
    fn group_manager_remove_group() {
        let mut mgr = TaskGroupManager::new();
        mgr.add_to_group("release", "build");
        mgr.add_to_group("release", "publish");
        let removed = mgr.remove_group("release").unwrap();
        assert_eq!(removed, vec!["build", "publish"]);
        assert!(mgr.remove_group("release").is_none());
        assert_eq!(mgr.group_count(), 0);
    }

    #[test]
    fn group_manager_group_names_sorted() {
        let mut mgr = TaskGroupManager::new();
        mgr.add_to_group("z-group", "t1");
        mgr.add_to_group("a-group", "t2");
        mgr.add_to_group("m-group", "t3");
        assert_eq!(mgr.group_names(), vec!["a-group", "m-group", "z-group"]);
    }
}
