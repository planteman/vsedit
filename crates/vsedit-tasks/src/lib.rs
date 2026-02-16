//! VS Code tasks.json parsing, task execution, problem matching, and auto-detection.
//!
//! This crate provides a complete implementation for working with VS Code-style
//! `tasks.json` files: parsing task definitions, executing tasks, matching
//! compiler diagnostics, auto-detecting build systems, and managing task
//! queues and history.

pub mod definition;
pub mod detect;
pub mod execution;
pub mod problem_matcher;
pub mod runner;
pub mod variables;

pub use definition::*;
pub use detect::detect_tasks;
pub use execution::*;
pub use problem_matcher::*;
pub use runner::*;
pub use variables::*;

use std::collections::BTreeMap;
use std::fmt;

use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------------------
// TaskError
// ---------------------------------------------------------------------------

/// Unified error type for task operations beyond basic I/O and parse errors
/// already covered by [`TasksError`].
#[derive(Debug, Error)]
pub enum TaskError {
    #[error("task not found: {0}")]
    NotFound(String),

    #[error("invalid task configuration: {0}")]
    InvalidConfig(String),

    #[error("task execution failed (exit {exit_code}): {message}")]
    ExecutionFailed { exit_code: i32, message: String },

    #[error("failed to parse task definition: {0}")]
    ParseError(String),

    #[error("task timed out after {0} ms")]
    Timeout(u64),

    #[error("task was cancelled: {0}")]
    Cancelled(String),

    #[error("duplicate task label: {0}")]
    DuplicateLabel(String),

    #[error("dependency cycle detected involving task: {0}")]
    DependencyCycle(String),

    #[error("queue is empty")]
    QueueEmpty,

    #[error("invalid task name: {reason}")]
    InvalidName { reason: String },
}

// ---------------------------------------------------------------------------
// TaskRunStatus
// ---------------------------------------------------------------------------

/// Lifecycle status for a managed task (distinct from the lower-level
/// [`execution::TaskStatus`] which tracks a single process).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskRunStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl fmt::Display for TaskRunStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Running => write!(f, "Running"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

impl TaskRunStatus {
    /// Returns `true` for terminal states (Completed, Failed, Cancelled).
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Returns `true` when the task finished successfully.
    pub fn is_success(self) -> bool {
        self == Self::Completed
    }
}

// ---------------------------------------------------------------------------
// TaskPriority
// ---------------------------------------------------------------------------

/// Priority level used for ordering tasks in a [`TaskQueue`].
///
/// Variants are ordered from lowest to highest; `Critical` is executed first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl TaskPriority {
    fn ordinal(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Normal => 1,
            Self::High => 2,
            Self::Critical => 3,
        }
    }
}

impl PartialOrd for TaskPriority {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TaskPriority {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ordinal().cmp(&other.ordinal())
    }
}

impl fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "Low"),
            Self::Normal => write!(f, "Normal"),
            Self::High => write!(f, "High"),
            Self::Critical => write!(f, "Critical"),
        }
    }
}

impl Default for TaskPriority {
    fn default() -> Self {
        Self::Normal
    }
}

// ---------------------------------------------------------------------------
// TaskResult
// ---------------------------------------------------------------------------

/// Outcome of a completed task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Process exit code (`0` typically means success).
    pub exit_code: i32,
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// High-level status derived from the execution.
    pub status: TaskRunStatus,
}

impl TaskResult {
    /// Create a new `TaskResult`.
    pub fn new(
        exit_code: i32,
        stdout: String,
        stderr: String,
        duration_ms: u64,
    ) -> Self {
        let status = if exit_code == 0 {
            TaskRunStatus::Completed
        } else {
            TaskRunStatus::Failed
        };
        Self {
            exit_code,
            stdout,
            stderr,
            duration_ms,
            status,
        }
    }

    /// Convenience constructor for a cancelled task.
    pub fn cancelled(duration_ms: u64) -> Self {
        Self {
            exit_code: -1,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms,
            status: TaskRunStatus::Cancelled,
        }
    }

    /// Returns `true` when the task finished successfully.
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }

    /// Combined stdout + stderr output, separated by a newline when both
    /// are non-empty.
    pub fn combined_output(&self) -> String {
        if self.stdout.is_empty() {
            return self.stderr.clone();
        }
        if self.stderr.is_empty() {
            return self.stdout.clone();
        }
        format!("{}\n{}", self.stdout, self.stderr)
    }
}

// ---------------------------------------------------------------------------
// TaskQueue
// ---------------------------------------------------------------------------

/// A prioritised entry inside a [`TaskQueue`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedTask {
    /// Unique identifier (usually the task label).
    pub id: String,
    /// Display label.
    pub label: String,
    /// Priority governs dequeue order.
    pub priority: TaskPriority,
    /// Current lifecycle status.
    pub status: TaskRunStatus,
    /// Monotonically increasing sequence number to preserve FIFO within a
    /// priority level.
    sequence: u64,
}

/// A simple priority queue for scheduling tasks.
///
/// Tasks with higher [`TaskPriority`] are dequeued first. Within the same
/// priority, tasks are dequeued in FIFO order.
#[derive(Debug, Default)]
pub struct TaskQueue {
    tasks: Vec<QueuedTask>,
    next_seq: u64,
}

impl TaskQueue {
    /// Create an empty queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of tasks in the queue (all statuses).
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// Returns `true` if the queue contains no tasks.
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Add a task to the queue with the given priority.
    pub fn enqueue(&mut self, id: impl Into<String>, label: impl Into<String>, priority: TaskPriority) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.tasks.push(QueuedTask {
            id: id.into(),
            label: label.into(),
            priority,
            status: TaskRunStatus::Pending,
            sequence: seq,
        });
    }

    /// Remove and return the highest-priority pending task.
    ///
    /// Returns `None` when no pending tasks remain.
    pub fn dequeue(&mut self) -> Option<QueuedTask> {
        let idx = self.best_pending_index()?;
        Some(self.tasks.remove(idx))
    }

    /// Peek at the highest-priority pending task without removing it.
    pub fn peek(&self) -> Option<&QueuedTask> {
        self.best_pending_index().map(|i| &self.tasks[i])
    }

    /// Cancel a queued task by id. Returns an error if the task is not found
    /// or is not in a cancellable state.
    pub fn cancel(&mut self, id: &str) -> Result<(), TaskError> {
        let entry = self
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| TaskError::NotFound(id.to_string()))?;

        if entry.status.is_terminal() {
            return Err(TaskError::InvalidConfig(format!(
                "task '{}' is already in terminal state {}",
                id, entry.status
            )));
        }
        entry.status = TaskRunStatus::Cancelled;
        Ok(())
    }

    /// Change the priority of an existing task. The task must still be
    /// pending.
    pub fn reorder(&mut self, id: &str, new_priority: TaskPriority) -> Result<(), TaskError> {
        let entry = self
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| TaskError::NotFound(id.to_string()))?;

        if entry.status != TaskRunStatus::Pending {
            return Err(TaskError::InvalidConfig(format!(
                "cannot reorder task '{}' in state {}",
                id, entry.status
            )));
        }
        entry.priority = new_priority;
        Ok(())
    }

    /// Return all tasks currently in the queue, ordered by priority
    /// (highest first), then by insertion order.
    pub fn pending_tasks(&self) -> Vec<&QueuedTask> {
        let mut out: Vec<&QueuedTask> = self
            .tasks
            .iter()
            .filter(|t| t.status == TaskRunStatus::Pending)
            .collect();
        out.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.sequence.cmp(&b.sequence)));
        out
    }

    /// Remove all tasks that are in a terminal state.
    pub fn drain_completed(&mut self) -> Vec<QueuedTask> {
        let (done, remaining): (Vec<_>, Vec<_>) =
            self.tasks.drain(..).partition(|t| t.status.is_terminal());
        self.tasks = remaining;
        done
    }

    // -- internal helpers ---------------------------------------------------

    fn best_pending_index(&self) -> Option<usize> {
        self.tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| t.status == TaskRunStatus::Pending)
            .max_by(|(_, a), (_, b)| {
                a.priority.cmp(&b.priority).then(b.sequence.cmp(&a.sequence))
            })
            .map(|(i, _)| i)
    }
}

// ---------------------------------------------------------------------------
// TaskHistory
// ---------------------------------------------------------------------------

/// A record of a single completed task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub label: String,
    pub result: TaskResult,
    /// Unix timestamp in seconds when the task started.
    pub started_at: u64,
}

/// Collects completed task results and exposes aggregate statistics.
#[derive(Debug, Default)]
pub struct TaskHistory {
    entries: Vec<HistoryEntry>,
    /// Per-label statistics kept up-to-date on every `record`.
    stats: BTreeMap<String, LabelStats>,
}

/// Accumulated statistics for a single task label.
#[derive(Debug, Clone, Default)]
struct LabelStats {
    total_runs: u64,
    successes: u64,
    total_duration_ms: u64,
}

impl TaskHistory {
    /// Create an empty history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a completed task.
    pub fn record(&mut self, label: impl Into<String>, result: TaskResult, started_at: u64) {
        let label = label.into();
        let stats = self.stats.entry(label.clone()).or_default();
        stats.total_runs += 1;
        if result.is_success() {
            stats.successes += 1;
        }
        stats.total_duration_ms += result.duration_ms;

        self.entries.push(HistoryEntry {
            label,
            result,
            started_at,
        });
    }

    /// Total number of recorded executions.
    pub fn total_runs(&self) -> usize {
        self.entries.len()
    }

    /// Average wall-clock duration across all recorded runs (in ms).
    /// Returns `None` when there are no entries.
    pub fn average_duration_ms(&self) -> Option<u64> {
        if self.entries.is_empty() {
            return None;
        }
        let total: u64 = self.entries.iter().map(|e| e.result.duration_ms).sum();
        Some(total / self.entries.len() as u64)
    }

    /// Overall success rate as a value between 0.0 and 1.0.
    /// Returns `None` when there are no entries.
    pub fn success_rate(&self) -> Option<f64> {
        if self.entries.is_empty() {
            return None;
        }
        let successes = self.entries.iter().filter(|e| e.result.is_success()).count();
        Some(successes as f64 / self.entries.len() as f64)
    }

    /// Average duration for a specific task label (in ms).
    pub fn average_duration_for(&self, label: &str) -> Option<u64> {
        let stats = self.stats.get(label)?;
        if stats.total_runs == 0 {
            return None;
        }
        Some(stats.total_duration_ms / stats.total_runs)
    }

    /// Success rate for a specific task label (0.0–1.0).
    pub fn success_rate_for(&self, label: &str) -> Option<f64> {
        let stats = self.stats.get(label)?;
        if stats.total_runs == 0 {
            return None;
        }
        Some(stats.successes as f64 / stats.total_runs as f64)
    }

    /// Total runs for a specific task label.
    pub fn runs_for(&self, label: &str) -> u64 {
        self.stats.get(label).map_or(0, |s| s.total_runs)
    }

    /// Immutable access to all entries.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// The most recent `n` entries (newest first).
    pub fn recent(&self, n: usize) -> Vec<&HistoryEntry> {
        self.entries.iter().rev().take(n).collect()
    }

    /// Clear all history and statistics.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.stats.clear();
    }

    /// Distinct task labels that have been recorded.
    pub fn labels(&self) -> Vec<&str> {
        self.stats.keys().map(String::as_str).collect()
    }
}

// ---------------------------------------------------------------------------
// TaskFilter
// ---------------------------------------------------------------------------

/// Filter predicate for selecting tasks from collections.
///
/// All fields are optional; a task must match **all** set criteria.
#[derive(Debug, Default)]
pub struct TaskFilter {
    /// If set, only tasks with this status pass.
    pub status: Option<TaskRunStatus>,
    /// If set, only tasks whose label matches this regex pass.
    pub name_pattern: Option<Regex>,
    /// If set, only tasks with this priority pass.
    pub priority: Option<TaskPriority>,
}

impl TaskFilter {
    /// Create a filter with no criteria (matches everything).
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder: set the status criterion.
    pub fn with_status(mut self, status: TaskRunStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Builder: set the name pattern criterion.
    pub fn with_name_pattern(mut self, pattern: Regex) -> Self {
        self.name_pattern = Some(pattern);
        self
    }

    /// Builder: set the priority criterion.
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = Some(priority);
        self
    }

    /// Test whether a [`QueuedTask`] satisfies all set criteria.
    pub fn matches_queued(&self, task: &QueuedTask) -> bool {
        if let Some(ref status) = self.status {
            if task.status != *status {
                return false;
            }
        }
        if let Some(ref pat) = self.name_pattern {
            if !pat.is_match(&task.label) {
                return false;
            }
        }
        if let Some(ref prio) = self.priority {
            if task.priority != *prio {
                return false;
            }
        }
        true
    }

    /// Test whether a [`HistoryEntry`] satisfies the status and name
    /// criteria (priority is not applicable to history entries).
    pub fn matches_history(&self, entry: &HistoryEntry) -> bool {
        if let Some(ref status) = self.status {
            if entry.result.status != *status {
                return false;
            }
        }
        if let Some(ref pat) = self.name_pattern {
            if !pat.is_match(&entry.label) {
                return false;
            }
        }
        true
    }

    /// Filter a slice of [`QueuedTask`]s, returning only those that match.
    pub fn filter_queued<'a>(&self, tasks: &'a [QueuedTask]) -> Vec<&'a QueuedTask> {
        tasks.iter().filter(|t| self.matches_queued(t)).collect()
    }

    /// Filter a slice of [`HistoryEntry`]s, returning only those that match.
    pub fn filter_history<'a>(&self, entries: &'a [HistoryEntry]) -> Vec<&'a HistoryEntry> {
        entries.iter().filter(|e| self.matches_history(e)).collect()
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Parse a task label into a (group, name) pair.
///
/// Labels may be written as `"group: name"` or just `"name"`. The separator
/// is the first `: ` (colon + space) sequence.
///
/// ```
/// # use vsedit_tasks::parse_task_label;
/// assert_eq!(parse_task_label("build: release"), (Some("build"), "release"));
/// assert_eq!(parse_task_label("lint"), (None, "lint"));
/// ```
pub fn parse_task_label(label: &str) -> (Option<&str>, &str) {
    match label.find(": ") {
        Some(pos) => {
            let group = label[..pos].trim();
            let name = label[pos + 2..].trim();
            if group.is_empty() {
                (None, name)
            } else {
                (Some(group), name)
            }
        }
        None => (None, label.trim()),
    }
}

/// Validate a task name according to common conventions.
///
/// A valid name:
/// - Is non-empty and at most 128 characters.
/// - Contains only alphanumeric characters, spaces, hyphens, underscores,
///   dots, colons, and forward slashes.
/// - Does not start or end with whitespace.
pub fn validate_task_name(name: &str) -> Result<(), TaskError> {
    if name.is_empty() {
        return Err(TaskError::InvalidName {
            reason: "task name must not be empty".into(),
        });
    }
    if name.len() > 128 {
        return Err(TaskError::InvalidName {
            reason: format!("task name exceeds 128 characters (got {})", name.len()),
        });
    }
    if name != name.trim() {
        return Err(TaskError::InvalidName {
            reason: "task name must not have leading or trailing whitespace".into(),
        });
    }
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || " -_.:/'".contains(c))
    {
        return Err(TaskError::InvalidName {
            reason: "task name contains invalid characters; allowed: alphanumeric, space, - _ . : / '".into(),
        });
    }
    Ok(())
}

/// Format a duration given in milliseconds into a human-readable string.
///
/// ```
/// # use vsedit_tasks::format_duration;
/// assert_eq!(format_duration(500), "500ms");
/// assert_eq!(format_duration(2_500), "2.50s");
/// assert_eq!(format_duration(90_000), "1m 30s");
/// assert_eq!(format_duration(3_723_000), "1h 2m 3s");
/// ```
pub fn format_duration(ms: u64) -> String {
    if ms < 1_000 {
        return format!("{ms}ms");
    }
    let total_secs = ms / 1_000;
    let remaining_ms = ms % 1_000;

    if total_secs < 60 {
        let frac = remaining_ms as f64 / 1_000.0;
        return format!("{:.2}s", total_secs as f64 + frac);
    }

    let hours = total_secs / 3_600;
    let minutes = (total_secs % 3_600) / 60;
    let seconds = total_secs % 60;

    let mut parts = Vec::new();
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes}m"));
    }
    if seconds > 0 {
        parts.push(format!("{seconds}s"));
    }
    if parts.is_empty() {
        parts.push("0s".to_string());
    }
    parts.join(" ")
}

/// Merge two JSON task configurations with `overlay` taking precedence.
///
/// Both inputs must be JSON objects (maps). Keys present in `overlay`
/// overwrite those in `base`; keys only in `base` are preserved.
/// Nested objects are merged recursively.
pub fn merge_task_configs(
    base: &serde_json::Value,
    overlay: &serde_json::Value,
) -> Result<serde_json::Value, TaskError> {
    match (base, overlay) {
        (serde_json::Value::Object(b), serde_json::Value::Object(o)) => {
            let mut merged = b.clone();
            for (key, oval) in o {
                let new_val = if let Some(bval) = b.get(key) {
                    if bval.is_object() && oval.is_object() {
                        merge_task_configs(bval, oval)?
                    } else {
                        oval.clone()
                    }
                } else {
                    oval.clone()
                };
                merged.insert(key.clone(), new_val);
            }
            Ok(serde_json::Value::Object(merged))
        }
        _ => Err(TaskError::InvalidConfig(
            "both base and overlay must be JSON objects".into(),
        )),
    }
}

// ---------------------------------------------------------------------------
// task_presentation — terminal display options
// ---------------------------------------------------------------------------

/// How the task terminal panel is revealed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskRevealKind {
    Always,
    Silent,
    Never,
}

impl Default for TaskRevealKind {
    fn default() -> Self {
        Self::Always
    }
}

/// Where the task terminal panel appears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskPanelKind {
    Shared,
    Dedicated,
    New,
}

impl Default for TaskPanelKind {
    fn default() -> Self {
        Self::Shared
    }
}

/// Presentation options controlling how a task's terminal behaves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskPresentation {
    pub reveal: TaskRevealKind,
    pub focus: bool,
    pub echo: bool,
    pub show_reuse_message: bool,
    pub panel: TaskPanelKind,
    pub clear: bool,
    pub close: bool,
}

impl Default for TaskPresentation {
    fn default() -> Self {
        Self {
            reveal: TaskRevealKind::Always,
            focus: false,
            echo: true,
            show_reuse_message: true,
            panel: TaskPanelKind::Shared,
            clear: false,
            close: false,
        }
    }
}

impl fmt::Display for TaskPresentation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "reveal={:?}, focus={}, echo={}, panel={:?}, clear={}, close={}",
            self.reveal, self.focus, self.echo, self.panel, self.clear, self.close,
        )
    }
}

/// Parse presentation options from a JSON value (as found in tasks.json).
pub fn task_presentation(json: &serde_json::Value) -> TaskPresentation {
    let mut pres = TaskPresentation::default();
    if let Some(obj) = json.as_object() {
        if let Some(r) = obj.get("reveal").and_then(|v| v.as_str()) {
            pres.reveal = match r {
                "silent" => TaskRevealKind::Silent,
                "never" => TaskRevealKind::Never,
                _ => TaskRevealKind::Always,
            };
        }
        if let Some(f) = obj.get("focus").and_then(|v| v.as_bool()) {
            pres.focus = f;
        }
        if let Some(e) = obj.get("echo").and_then(|v| v.as_bool()) {
            pres.echo = e;
        }
        if let Some(s) = obj.get("showReuseMessage").and_then(|v| v.as_bool()) {
            pres.show_reuse_message = s;
        }
        if let Some(p) = obj.get("panel").and_then(|v| v.as_str()) {
            pres.panel = match p {
                "dedicated" => TaskPanelKind::Dedicated,
                "new" => TaskPanelKind::New,
                _ => TaskPanelKind::Shared,
            };
        }
        if let Some(c) = obj.get("clear").and_then(|v| v.as_bool()) {
            pres.clear = c;
        }
        if let Some(c) = obj.get("close").and_then(|v| v.as_bool()) {
            pres.close = c;
        }
    }
    pres
}

// ---------------------------------------------------------------------------
// TaskTemplate — reusable task templates
// ---------------------------------------------------------------------------

/// A reusable task template that can be instantiated with variable overrides.
///
/// Templates store a base JSON configuration and a set of placeholder names.
/// Calling [`instantiate`](TaskTemplate::instantiate) substitutes placeholders
/// of the form `${key}` in string values throughout the JSON tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTemplate {
    /// Human-readable name for this template.
    pub name: String,
    /// Base configuration (must be a JSON object).
    pub config: serde_json::Value,
    /// Ordered list of placeholder keys expected in the config.
    pub placeholders: Vec<String>,
}

impl TaskTemplate {
    /// Create a new template.
    pub fn new(
        name: impl Into<String>,
        config: serde_json::Value,
        placeholders: Vec<String>,
    ) -> Result<Self, TaskError> {
        if !config.is_object() {
            return Err(TaskError::InvalidConfig(
                "template config must be a JSON object".into(),
            ));
        }
        Ok(Self {
            name: name.into(),
            config,
            placeholders,
        })
    }

    /// Instantiate this template by replacing `${key}` with the supplied values.
    ///
    /// Keys not present in `vars` are left as-is.
    pub fn instantiate(&self, vars: &BTreeMap<String, String>) -> serde_json::Value {
        Self::substitute_value(&self.config, vars)
    }

    /// Returns `true` if every declared placeholder has a corresponding key
    /// in `vars`.
    pub fn is_fully_bound(&self, vars: &BTreeMap<String, String>) -> bool {
        self.placeholders.iter().all(|p| vars.contains_key(p))
    }

    /// Return placeholder keys that are missing from `vars`.
    pub fn missing_placeholders(&self, vars: &BTreeMap<String, String>) -> Vec<&str> {
        self.placeholders
            .iter()
            .filter(|p| !vars.contains_key(p.as_str()))
            .map(String::as_str)
            .collect()
    }

    fn substitute_value(
        value: &serde_json::Value,
        vars: &BTreeMap<String, String>,
    ) -> serde_json::Value {
        match value {
            serde_json::Value::String(s) => {
                let mut result = s.clone();
                for (key, val) in vars {
                    result = result.replace(&format!("${{{key}}}"), val);
                }
                serde_json::Value::String(result)
            }
            serde_json::Value::Array(arr) => serde_json::Value::Array(
                arr.iter().map(|v| Self::substitute_value(v, vars)).collect(),
            ),
            serde_json::Value::Object(obj) => {
                let mut map = serde_json::Map::new();
                for (k, v) in obj {
                    map.insert(k.clone(), Self::substitute_value(v, vars));
                }
                serde_json::Value::Object(map)
            }
            other => other.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// TaskScheduler — dependency-aware task ordering
// ---------------------------------------------------------------------------

/// A dependency-aware scheduler that determines a valid execution order
/// for a set of named tasks with declared dependencies.
///
/// Uses topological sorting (Kahn's algorithm) to produce an ordering
/// that respects all dependency edges, or reports a cycle.
#[derive(Debug, Default)]
pub struct TaskScheduler {
    /// Map from task id to its list of dependency ids.
    deps: BTreeMap<String, Vec<String>>,
}

impl TaskScheduler {
    /// Create an empty scheduler.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a task with no dependencies.
    pub fn add_task(&mut self, id: impl Into<String>) {
        self.deps.entry(id.into()).or_default();
    }

    /// Register a task that depends on another task.
    ///
    /// Both `id` and `dependency` are implicitly registered if not already
    /// present.
    pub fn add_dependency(&mut self, id: impl Into<String>, dependency: impl Into<String>) {
        let dep = dependency.into();
        let id = id.into();
        self.deps.entry(dep.clone()).or_default();
        self.deps.entry(id.clone()).or_default().push(dep);
    }

    /// Return all direct dependencies for a task.
    pub fn dependencies_of(&self, id: &str) -> Option<&[String]> {
        self.deps.get(id).map(Vec::as_slice)
    }

    /// Number of registered tasks.
    pub fn task_count(&self) -> usize {
        self.deps.len()
    }

    /// Compute a topological ordering of the tasks.
    ///
    /// Returns tasks in an order such that every task appears after all of
    /// its dependencies.  Returns [`TaskError::DependencyCycle`] if the
    /// graph contains a cycle.
    pub fn schedule(&self) -> Result<Vec<String>, TaskError> {
        // in-degree map
        let mut in_degree: BTreeMap<&str, usize> = BTreeMap::new();
        for id in self.deps.keys() {
            in_degree.entry(id.as_str()).or_insert(0);
        }
        for (id, deps_list) in &self.deps {
            in_degree.insert(id.as_str(), deps_list.len());
        }

        let mut queue: std::collections::VecDeque<&str> = in_degree
            .iter()
            .filter(|entry| *entry.1 == 0)
            .map(|entry| *entry.0)
            .collect();

        let mut order: Vec<String> = Vec::with_capacity(self.deps.len());

        while let Some(current) = queue.pop_front() {
            order.push(current.to_string());
            // For every task that lists `current` as a dependency, reduce
            // its in-degree.
            for (id, deps_list) in &self.deps {
                if deps_list.iter().any(|d| d == current) {
                    if let Some(deg) = in_degree.get_mut(id.as_str()) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(id.as_str());
                        }
                    }
                }
            }
        }

        if order.len() != self.deps.len() {
            // Find a task still with nonzero in-degree to report.
            let stuck = in_degree
                .iter()
                .find(|entry| *entry.1 > 0)
                .map(|entry| entry.0.to_string())
                .unwrap_or_default();
            return Err(TaskError::DependencyCycle(stuck));
        }

        Ok(order)
    }
}

// ---------------------------------------------------------------------------
// TaskExporter — export tasks to JSON
// ---------------------------------------------------------------------------

/// Serialise a collection of [`QueuedTask`]s or [`HistoryEntry`]s into a
/// VS Code-style `tasks.json` fragment.
pub struct TaskExporter;

impl TaskExporter {
    /// Export queued tasks as a JSON array.
    pub fn export_queue(tasks: &[QueuedTask]) -> serde_json::Value {
        serde_json::json!(tasks)
    }

    /// Export history entries as a JSON array.
    pub fn export_history(entries: &[HistoryEntry]) -> serde_json::Value {
        serde_json::json!(entries)
    }

    /// Export queued tasks into a VS Code compatible `tasks.json` wrapper.
    pub fn export_tasks_json(tasks: &[QueuedTask]) -> serde_json::Value {
        serde_json::json!({
            "version": "2.0.0",
            "tasks": tasks.iter().map(|t| {
                serde_json::json!({
                    "label": t.label,
                    "group": {
                        "kind": "build",
                        "isDefault": false
                    }
                })
            }).collect::<Vec<_>>()
        })
    }

    /// Produce a human-readable summary string for a set of history entries.
    pub fn summary(entries: &[HistoryEntry]) -> String {
        if entries.is_empty() {
            return "No task history.".to_string();
        }
        let total = entries.len();
        let successes = entries.iter().filter(|e| e.result.is_success()).count();
        let total_ms: u64 = entries.iter().map(|e| e.result.duration_ms).sum();
        format!(
            "{total} run(s): {successes} succeeded, {} failed, total time {}",
            total - successes,
            format_duration(total_ms),
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Extended TaskQueue and TaskResult methods
// ---------------------------------------------------------------------------

impl TaskQueue {
    /// Remove and return all tasks (regardless of status), draining the queue.
    pub fn drain(&mut self) -> Vec<QueuedTask> {
        self.tasks.drain(..).collect()
    }

    /// Find a task by its id without removing it.
    pub fn find(&self, id: &str) -> Option<&QueuedTask> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// Cancel all pending tasks. Returns the number of tasks cancelled.
    pub fn cancel_all(&mut self) -> usize {
        let mut count = 0;
        for task in &mut self.tasks {
            if task.status == TaskRunStatus::Pending {
                task.status = TaskRunStatus::Cancelled;
                count += 1;
            }
        }
        count
    }

    /// Return the set of distinct priorities present among pending tasks,
    /// ordered highest to lowest.
    pub fn priorities(&self) -> Vec<TaskPriority> {
        let mut prios: Vec<TaskPriority> = self
            .tasks
            .iter()
            .filter(|t| t.status == TaskRunStatus::Pending)
            .map(|t| t.priority)
            .collect();
        prios.sort_by(|a, b| b.cmp(a));
        prios.dedup();
        prios
    }

    /// Re-queue a terminal task by resetting its status to Pending.
    ///
    /// Returns an error if the task is not found or is not in a terminal state.
    pub fn requeue(&mut self, id: &str) -> Result<(), TaskError> {
        let entry = self
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| TaskError::NotFound(id.to_string()))?;
        if !entry.status.is_terminal() {
            return Err(TaskError::InvalidConfig(format!(
                "task '{}' is not in a terminal state ({})",
                id, entry.status
            )));
        }
        entry.status = TaskRunStatus::Pending;
        entry.sequence = self.next_seq;
        self.next_seq += 1;
        Ok(())
    }

    /// Sort internal tasks by priority (highest first), preserving FIFO
    /// within each priority level.
    pub fn sort_by_priority(&mut self) {
        self.tasks
            .sort_by(|a, b| b.priority.cmp(&a.priority).then(a.sequence.cmp(&b.sequence)));
    }

    /// Count the number of tasks with a given status.
    pub fn count_by_status(&self, status: TaskRunStatus) -> usize {
        self.tasks.iter().filter(|t| t.status == status).count()
    }
}

impl TaskResult {
    /// Merge two results, combining stdout/stderr and summing duration.
    ///
    /// The exit code is the first non-zero exit code, or 0 if both succeeded.
    /// Status is `Completed` only if both are successful.
    pub fn merge(&self, other: &TaskResult) -> TaskResult {
        let exit_code = if self.exit_code != 0 {
            self.exit_code
        } else {
            other.exit_code
        };
        let stdout = if self.stdout.is_empty() {
            other.stdout.clone()
        } else if other.stdout.is_empty() {
            self.stdout.clone()
        } else {
            format!("{}\n{}", self.stdout, other.stdout)
        };
        let stderr = if self.stderr.is_empty() {
            other.stderr.clone()
        } else if other.stderr.is_empty() {
            self.stderr.clone()
        } else {
            format!("{}\n{}", self.stderr, other.stderr)
        };
        let duration_ms = self.duration_ms + other.duration_ms;
        TaskResult::new(exit_code, stdout, stderr, duration_ms)
    }
}

// ---------------------------------------------------------------------------
// TaskSource — identifies where a task definition comes from
// ---------------------------------------------------------------------------

/// Identifies the origin of a task definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskSource {
    /// Defined in the workspace `.vscode/tasks.json`.
    Workspace,
    /// Defined in user-level settings.
    User,
    /// Auto-detected by an extension or built-in provider.
    AutoDetected { provider: String },
    /// Contributed by a specific extension.
    Extension { id: String },
}

impl fmt::Display for TaskSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workspace => write!(f, "workspace"),
            Self::User => write!(f, "user"),
            Self::AutoDetected { provider } => write!(f, "auto-detected ({provider})"),
            Self::Extension { id } => write!(f, "extension ({id})"),
        }
    }
}

// ---------------------------------------------------------------------------
// TaskRunOptions — controls re-evaluation and instance behaviour
// ---------------------------------------------------------------------------

/// Controls how a task behaves when re-run or when multiple instances exist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRunOptions {
    /// When true, variables in the command are re-evaluated on every run.
    #[serde(default)]
    pub reevaluate_on_rerun: bool,
    /// Behaviour when the same task is launched while still running.
    #[serde(default)]
    pub instance_limit: TaskInstancePolicy,
}

impl Default for TaskRunOptions {
    fn default() -> Self {
        Self {
            reevaluate_on_rerun: true,
            instance_limit: TaskInstancePolicy::default(),
        }
    }
}

/// Policy for handling multiple simultaneous instances of the same task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskInstancePolicy {
    /// Allow parallel instances.
    Parallel,
    /// Terminate the running instance and start a new one.
    Terminate,
    /// Discard the new launch request.
    Ignore,
}

impl Default for TaskInstancePolicy {
    fn default() -> Self {
        Self::Terminate
    }
}

/// Parse [`TaskRunOptions`] from a JSON value (as found in tasks.json).
pub fn parse_run_options(json: &serde_json::Value) -> TaskRunOptions {
    let mut opts = TaskRunOptions::default();
    if let Some(obj) = json.as_object() {
        if let Some(r) = obj.get("reevaluateOnRerun").and_then(|v| v.as_bool()) {
            opts.reevaluate_on_rerun = r;
        }
        if let Some(il) = obj.get("instanceLimit").and_then(|v| v.as_str()) {
            opts.instance_limit = match il {
                "parallel" => TaskInstancePolicy::Parallel,
                "ignore" => TaskInstancePolicy::Ignore,
                _ => TaskInstancePolicy::Terminate,
            };
        }
    }
    opts
}

// ---------------------------------------------------------------------------
// Shell quoting helpers
// ---------------------------------------------------------------------------

/// Escape a string for safe inclusion in a POSIX shell command.
///
/// Wraps the value in single quotes, escaping any embedded single quotes.
pub fn shell_quote_posix(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    // If the string contains no special chars, return as-is
    if s.chars()
        .all(|c| c.is_alphanumeric() || "-_./=:@,".contains(c))
    {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Escape a string for safe inclusion in a Windows `cmd.exe` command.
///
/// Wraps in double quotes and escapes internal special characters.
pub fn shell_quote_cmd(s: &str) -> String {
    if s.is_empty() {
        return "\"\"".to_string();
    }
    if s.chars()
        .all(|c| c.is_alphanumeric() || "-_./=:@,".contains(c))
    {
        return s.to_string();
    }
    let escaped = s
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    format!("\"{escaped}\"")
}

/// Build a command string from a program and argument list, quoting each
/// argument for POSIX shells.
pub fn build_shell_command(program: &str, args: &[&str]) -> String {
    let mut parts = vec![shell_quote_posix(program)];
    for arg in args {
        parts.push(shell_quote_posix(arg));
    }
    parts.join(" ")
}

// ---------------------------------------------------------------------------
// Task label formatting helpers
// ---------------------------------------------------------------------------

/// Format a task label from optional group and name components.
///
/// If a group is provided the label is `"group: name"`, otherwise just `"name"`.
pub fn format_task_label(group: Option<&str>, name: &str) -> String {
    match group {
        Some(g) if !g.is_empty() => format!("{g}: {name}"),
        _ => name.to_string(),
    }
}

/// Normalize a task label by trimming whitespace and collapsing multiple
/// spaces into one.
pub fn normalize_task_label(label: &str) -> String {
    label.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// TaskDependencyGraph — richer graph with parallel-group support
// ---------------------------------------------------------------------------

/// A dependency graph that can compute execution levels for maximum
/// parallelism.
///
/// Each level in the result contains tasks that can run in parallel,
/// provided all tasks in previous levels have completed.
#[derive(Debug, Default)]
pub struct TaskDependencyGraph {
    deps: BTreeMap<String, Vec<String>>,
}

impl TaskDependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a task (with no dependencies if not already present).
    pub fn add_task(&mut self, id: impl Into<String>) {
        self.deps.entry(id.into()).or_default();
    }

    /// Add a dependency edge: `id` depends on `dependency`.
    pub fn add_dependency(&mut self, id: impl Into<String>, dependency: impl Into<String>) {
        let dep = dependency.into();
        let id = id.into();
        self.deps.entry(dep.clone()).or_default();
        self.deps.entry(id.clone()).or_default().push(dep);
    }

    /// Return the set of tasks that have no dependents (i.e., nothing
    /// depends on them). These are the "leaf" / final tasks.
    pub fn leaf_tasks(&self) -> Vec<&str> {
        let mut depended_on: std::collections::HashSet<&str> =
            std::collections::HashSet::new();
        for deps_list in self.deps.values() {
            for d in deps_list {
                depended_on.insert(d.as_str());
            }
        }
        self.deps
            .keys()
            .filter(|k| !depended_on.contains(k.as_str()))
            .map(String::as_str)
            .collect()
    }

    /// Return the set of tasks that have no dependencies (roots).
    pub fn root_tasks(&self) -> Vec<&str> {
        self.deps
            .iter()
            .filter(|(_, deps)| deps.is_empty())
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// Compute execution levels using topological layering.
    ///
    /// Returns `Ok(levels)` where each inner `Vec` contains tasks that may
    /// execute in parallel. Returns an error on cycles.
    pub fn execution_levels(&self) -> Result<Vec<Vec<String>>, TaskError> {
        let mut in_degree: BTreeMap<&str, usize> = BTreeMap::new();
        for (id, deps_list) in &self.deps {
            in_degree.entry(id.as_str()).or_insert(0);
            // Ensure deps_list length is reflected
            *in_degree.entry(id.as_str()).or_insert(0) = deps_list.len();
        }

        let mut levels: Vec<Vec<String>> = Vec::new();
        let mut remaining = in_degree.clone();

        loop {
            let current_level: Vec<String> = remaining
                .iter()
                .filter(|(_, deg)| **deg == 0)
                .map(|(id, _)| id.to_string())
                .collect();

            if current_level.is_empty() {
                break;
            }

            for id in &current_level {
                remaining.remove(id.as_str());
            }

            // Reduce in-degree for dependents
            for (id, deg) in remaining.iter_mut() {
                if let Some(deps_list) = self.deps.get(*id) {
                    let resolved = deps_list
                        .iter()
                        .filter(|d| current_level.contains(d))
                        .count();
                    *deg = deg.saturating_sub(resolved);
                }
            }

            levels.push(current_level);
        }

        if !remaining.is_empty() {
            let stuck = remaining
                .keys()
                .next()
                .map(|s| s.to_string())
                .unwrap_or_default();
            return Err(TaskError::DependencyCycle(stuck));
        }

        Ok(levels)
    }

    /// Total number of registered tasks.
    pub fn task_count(&self) -> usize {
        self.deps.len()
    }

    /// Check if `id` transitively depends on `target`.
    pub fn depends_on(&self, id: &str, target: &str) -> bool {
        let mut visited: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut stack: Vec<&str> = vec![id];
        while let Some(current) = stack.pop() {
            if current == target && current != id {
                return true;
            }
            if !visited.insert(current) {
                continue;
            }
            if let Some(deps_list) = self.deps.get(current) {
                for d in deps_list {
                    stack.push(d.as_str());
                }
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Problem matcher pattern compilation helper
// ---------------------------------------------------------------------------

/// A compiled problem matcher pattern ready for matching against output lines.
#[derive(Debug)]
pub struct CompiledPattern {
    pub regex: Regex,
    pub file_group: Option<usize>,
    pub line_group: Option<usize>,
    pub column_group: Option<usize>,
    pub message_group: Option<usize>,
    pub severity_group: Option<usize>,
}

impl CompiledPattern {
    /// Compile a problem matcher pattern from its regex string and group
    /// indices.
    pub fn new(
        pattern: &str,
        file_group: Option<usize>,
        line_group: Option<usize>,
        column_group: Option<usize>,
        message_group: Option<usize>,
        severity_group: Option<usize>,
    ) -> Result<Self, TaskError> {
        let regex = Regex::new(pattern).map_err(|e| {
            TaskError::ParseError(format!("invalid problem matcher regex: {e}"))
        })?;
        Ok(Self {
            regex,
            file_group,
            line_group,
            column_group,
            message_group,
            severity_group,
        })
    }

    /// Extract a diagnostic match from an output line.
    ///
    /// Returns `None` if the line does not match.
    pub fn match_line(&self, line: &str) -> Option<DiagnosticMatch> {
        let caps = self.regex.captures(line)?;
        let get = |group: Option<usize>| -> Option<String> {
            group.and_then(|g| caps.get(g).map(|m| m.as_str().to_string()))
        };
        Some(DiagnosticMatch {
            file: get(self.file_group),
            line: get(self.line_group).and_then(|s| s.parse().ok()),
            column: get(self.column_group).and_then(|s| s.parse().ok()),
            message: get(self.message_group),
            severity: get(self.severity_group),
        })
    }
}

/// A single diagnostic extracted from a compiler output line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticMatch {
    pub file: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: Option<String>,
    pub severity: Option<String>,
}

// ---------------------------------------------------------------------------
// Task definition JSON parsing helpers
// ---------------------------------------------------------------------------

/// Extract the list of task labels from a parsed tasks.json `Value`.
///
/// Expects the standard `{ "tasks": [ { "label": "..." }, ... ] }` format.
pub fn extract_task_labels(tasks_json: &serde_json::Value) -> Vec<String> {
    tasks_json
        .get("tasks")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.get("label").and_then(|l| l.as_str()))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

/// Extract task `dependsOn` entries for a given label.
///
/// Returns the list of dependency labels, or an empty vec if the task has
/// none or is not found.
pub fn extract_depends_on(
    tasks_json: &serde_json::Value,
    label: &str,
) -> Vec<String> {
    let tasks = match tasks_json.get("tasks").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return Vec::new(),
    };
    for task in tasks {
        let task_label = task.get("label").and_then(|l| l.as_str()).unwrap_or("");
        if task_label != label {
            continue;
        }
        if let Some(deps) = task.get("dependsOn") {
            if let Some(arr) = deps.as_array() {
                return arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
            }
            if let Some(s) = deps.as_str() {
                return vec![s.to_string()];
            }
        }
    }
    Vec::new()
}

/// Build a [`TaskDependencyGraph`] from a full tasks.json `Value`.
pub fn build_dependency_graph(
    tasks_json: &serde_json::Value,
) -> Result<TaskDependencyGraph, TaskError> {
    let labels = extract_task_labels(tasks_json);
    let mut graph = TaskDependencyGraph::new();
    for label in &labels {
        graph.add_task(label);
    }
    for label in &labels {
        for dep in extract_depends_on(tasks_json, label) {
            graph.add_dependency(label, dep);
        }
    }
    Ok(graph)
}

/// Filter tasks from a tasks.json `Value` by group kind.
///
/// Returns the JSON objects for tasks whose `group` matches `kind`.
pub fn filter_tasks_by_group(
    tasks_json: &serde_json::Value,
    kind: &str,
) -> Vec<serde_json::Value> {
    let tasks = match tasks_json.get("tasks").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return Vec::new(),
    };
    tasks
        .iter()
        .filter(|t| {
            if let Some(g) = t.get("group") {
                if let Some(s) = g.as_str() {
                    return s == kind;
                }
                if let Some(obj) = g.as_object() {
                    if let Some(k) = obj.get("kind").and_then(|v| v.as_str()) {
                        return k == kind;
                    }
                }
            }
            false
        })
        .cloned()
        .collect()
}

/// Determine the default task for a given group kind, if one is marked.
///
/// Looks for `{ "group": { "kind": "<kind>", "isDefault": true } }`.
pub fn find_default_task(
    tasks_json: &serde_json::Value,
    kind: &str,
) -> Option<String> {
    let tasks = tasks_json.get("tasks")?.as_array()?;
    for task in tasks {
        if let Some(g) = task.get("group").and_then(|v| v.as_object()) {
            let is_kind = g.get("kind").and_then(|v| v.as_str()) == Some(kind);
            let is_default = g.get("isDefault").and_then(|v| v.as_bool()) == Some(true);
            if is_kind && is_default {
                return task.get("label").and_then(|l| l.as_str()).map(String::from);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- TaskError ----------------------------------------------------------

    #[test]
    fn task_error_display() {
        let err = TaskError::NotFound("build".into());
        assert_eq!(err.to_string(), "task not found: build");

        let err = TaskError::Timeout(5000);
        assert_eq!(err.to_string(), "task timed out after 5000 ms");
    }

    #[test]
    fn task_error_execution_failed_display() {
        let err = TaskError::ExecutionFailed {
            exit_code: 1,
            message: "segfault".into(),
        };
        assert!(err.to_string().contains("exit 1"));
        assert!(err.to_string().contains("segfault"));
    }

    // -- TaskRunStatus ------------------------------------------------------

    #[test]
    fn task_run_status_display_and_terminal() {
        assert_eq!(TaskRunStatus::Pending.to_string(), "Pending");
        assert!(!TaskRunStatus::Pending.is_terminal());
        assert!(!TaskRunStatus::Running.is_terminal());
        assert!(TaskRunStatus::Completed.is_terminal());
        assert!(TaskRunStatus::Failed.is_terminal());
        assert!(TaskRunStatus::Cancelled.is_terminal());
    }

    #[test]
    fn task_run_status_is_success() {
        assert!(TaskRunStatus::Completed.is_success());
        assert!(!TaskRunStatus::Failed.is_success());
    }

    // -- TaskPriority -------------------------------------------------------

    #[test]
    fn priority_ordering() {
        assert!(TaskPriority::Critical > TaskPriority::High);
        assert!(TaskPriority::High > TaskPriority::Normal);
        assert!(TaskPriority::Normal > TaskPriority::Low);
    }

    #[test]
    fn priority_default_is_normal() {
        assert_eq!(TaskPriority::default(), TaskPriority::Normal);
    }

    #[test]
    fn priority_display() {
        assert_eq!(TaskPriority::Low.to_string(), "Low");
        assert_eq!(TaskPriority::Critical.to_string(), "Critical");
    }

    // -- TaskResult ---------------------------------------------------------

    #[test]
    fn task_result_success() {
        let r = TaskResult::new(0, "ok".into(), String::new(), 120);
        assert!(r.is_success());
        assert_eq!(r.status, TaskRunStatus::Completed);
        assert_eq!(r.combined_output(), "ok");
    }

    #[test]
    fn task_result_failure() {
        let r = TaskResult::new(1, String::new(), "err".into(), 50);
        assert!(!r.is_success());
        assert_eq!(r.status, TaskRunStatus::Failed);
        assert_eq!(r.combined_output(), "err");
    }

    #[test]
    fn task_result_combined_output() {
        let r = TaskResult::new(0, "out".into(), "err".into(), 10);
        assert_eq!(r.combined_output(), "out\nerr");
    }

    #[test]
    fn task_result_cancelled() {
        let r = TaskResult::cancelled(300);
        assert_eq!(r.status, TaskRunStatus::Cancelled);
        assert_eq!(r.exit_code, -1);
        assert!(!r.is_success());
    }

    // -- TaskQueue ----------------------------------------------------------

    #[test]
    fn queue_enqueue_dequeue_by_priority() {
        let mut q = TaskQueue::new();
        q.enqueue("a", "low task", TaskPriority::Low);
        q.enqueue("b", "critical task", TaskPriority::Critical);
        q.enqueue("c", "normal task", TaskPriority::Normal);

        let first = q.dequeue().unwrap();
        assert_eq!(first.id, "b");
        let second = q.dequeue().unwrap();
        assert_eq!(second.id, "c");
        let third = q.dequeue().unwrap();
        assert_eq!(third.id, "a");
        assert!(q.dequeue().is_none());
    }

    #[test]
    fn queue_fifo_within_same_priority() {
        let mut q = TaskQueue::new();
        q.enqueue("x", "first", TaskPriority::Normal);
        q.enqueue("y", "second", TaskPriority::Normal);
        q.enqueue("z", "third", TaskPriority::Normal);

        assert_eq!(q.dequeue().unwrap().id, "x");
        assert_eq!(q.dequeue().unwrap().id, "y");
        assert_eq!(q.dequeue().unwrap().id, "z");
    }

    #[test]
    fn queue_peek_does_not_remove() {
        let mut q = TaskQueue::new();
        q.enqueue("a", "task", TaskPriority::Normal);
        assert_eq!(q.peek().unwrap().id, "a");
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn queue_cancel() {
        let mut q = TaskQueue::new();
        q.enqueue("a", "task", TaskPriority::Normal);
        q.cancel("a").unwrap();

        assert!(q.dequeue().is_none());
    }

    #[test]
    fn queue_cancel_nonexistent_returns_error() {
        let mut q = TaskQueue::new();
        assert!(matches!(q.cancel("ghost"), Err(TaskError::NotFound(_))));
    }

    #[test]
    fn queue_reorder() {
        let mut q = TaskQueue::new();
        q.enqueue("a", "task a", TaskPriority::Low);
        q.enqueue("b", "task b", TaskPriority::High);

        q.reorder("a", TaskPriority::Critical).unwrap();
        assert_eq!(q.peek().unwrap().id, "a");
    }

    #[test]
    fn queue_drain_completed() {
        let mut q = TaskQueue::new();
        q.enqueue("a", "task a", TaskPriority::Normal);
        q.enqueue("b", "task b", TaskPriority::Normal);
        q.cancel("a").unwrap();

        let drained = q.drain_completed();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn queue_pending_tasks_returns_sorted() {
        let mut q = TaskQueue::new();
        q.enqueue("lo", "low", TaskPriority::Low);
        q.enqueue("hi", "high", TaskPriority::High);
        q.enqueue("no", "normal", TaskPriority::Normal);

        let pending = q.pending_tasks();
        assert_eq!(pending[0].id, "hi");
        assert_eq!(pending[1].id, "no");
        assert_eq!(pending[2].id, "lo");
    }

    // -- TaskHistory --------------------------------------------------------

    #[test]
    fn history_basic_stats() {
        let mut h = TaskHistory::new();
        h.record("build", TaskResult::new(0, String::new(), String::new(), 100), 1000);
        h.record("build", TaskResult::new(1, String::new(), String::new(), 200), 1001);
        h.record("test", TaskResult::new(0, String::new(), String::new(), 300), 1002);

        assert_eq!(h.total_runs(), 3);
        assert_eq!(h.average_duration_ms(), Some(200));
        assert!((h.success_rate().unwrap() - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn history_per_label_stats() {
        let mut h = TaskHistory::new();
        h.record("build", TaskResult::new(0, String::new(), String::new(), 100), 1);
        h.record("build", TaskResult::new(1, String::new(), String::new(), 300), 2);

        assert_eq!(h.runs_for("build"), 2);
        assert_eq!(h.average_duration_for("build"), Some(200));
        assert!((h.success_rate_for("build").unwrap() - 0.5).abs() < 1e-9);
        assert_eq!(h.runs_for("unknown"), 0);
    }

    #[test]
    fn history_recent_and_labels() {
        let mut h = TaskHistory::new();
        h.record("a", TaskResult::new(0, String::new(), String::new(), 10), 1);
        h.record("b", TaskResult::new(0, String::new(), String::new(), 20), 2);
        h.record("c", TaskResult::new(0, String::new(), String::new(), 30), 3);

        let recent = h.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].label, "c");
        assert_eq!(recent[1].label, "b");

        let mut labels = h.labels();
        labels.sort();
        assert_eq!(labels, vec!["a", "b", "c"]);
    }

    #[test]
    fn history_empty_returns_none() {
        let h = TaskHistory::new();
        assert_eq!(h.average_duration_ms(), None);
        assert_eq!(h.success_rate(), None);
    }

    #[test]
    fn history_clear() {
        let mut h = TaskHistory::new();
        h.record("build", TaskResult::new(0, String::new(), String::new(), 100), 1);
        h.clear();
        assert_eq!(h.total_runs(), 0);
        assert!(h.labels().is_empty());
    }

    // -- TaskFilter ---------------------------------------------------------

    #[test]
    fn filter_matches_queued() {
        let task = QueuedTask {
            id: "t1".into(),
            label: "build: release".into(),
            priority: TaskPriority::High,
            status: TaskRunStatus::Pending,
            sequence: 0,
        };

        let f = TaskFilter::new()
            .with_status(TaskRunStatus::Pending)
            .with_priority(TaskPriority::High)
            .with_name_pattern(Regex::new("build").unwrap());

        assert!(f.matches_queued(&task));

        let f2 = TaskFilter::new().with_status(TaskRunStatus::Running);
        assert!(!f2.matches_queued(&task));
    }

    #[test]
    fn filter_matches_history() {
        let entry = HistoryEntry {
            label: "cargo test".into(),
            result: TaskResult::new(0, String::new(), String::new(), 100),
            started_at: 1000,
        };

        let f = TaskFilter::new()
            .with_name_pattern(Regex::new("cargo").unwrap())
            .with_status(TaskRunStatus::Completed);
        assert!(f.matches_history(&entry));

        let f2 = TaskFilter::new().with_name_pattern(Regex::new("npm").unwrap());
        assert!(!f2.matches_history(&entry));
    }

    // -- Utility functions --------------------------------------------------

    #[test]
    fn parse_task_label_with_group() {
        let (group, name) = parse_task_label("build: release");
        assert_eq!(group, Some("build"));
        assert_eq!(name, "release");
    }

    #[test]
    fn parse_task_label_without_group() {
        let (group, name) = parse_task_label("lint");
        assert_eq!(group, None);
        assert_eq!(name, "lint");
    }

    #[test]
    fn validate_task_name_valid() {
        assert!(validate_task_name("build: release").is_ok());
        assert!(validate_task_name("my-task_v2.0").is_ok());
    }

    #[test]
    fn validate_task_name_invalid() {
        assert!(validate_task_name("").is_err());
        assert!(validate_task_name(" leading").is_err());
        assert!(validate_task_name("trailing ").is_err());
        assert!(validate_task_name("bad\tchar").is_err());
        let long_name = "a".repeat(129);
        assert!(validate_task_name(&long_name).is_err());
    }

    #[test]
    fn format_duration_various() {
        assert_eq!(format_duration(0), "0ms");
        assert_eq!(format_duration(500), "500ms");
        assert_eq!(format_duration(1_000), "1.00s");
        assert_eq!(format_duration(2_500), "2.50s");
        assert_eq!(format_duration(90_000), "1m 30s");
        assert_eq!(format_duration(3_723_000), "1h 2m 3s");
        assert_eq!(format_duration(3_600_000), "1h");
    }

    #[test]
    fn merge_task_configs_simple() {
        let base: serde_json::Value = serde_json::json!({
            "label": "build",
            "command": "cargo",
            "args": ["build"]
        });
        let overlay: serde_json::Value = serde_json::json!({
            "args": ["build", "--release"],
            "group": "build"
        });

        let merged = merge_task_configs(&base, &overlay).unwrap();
        assert_eq!(merged["label"], "build");
        assert_eq!(merged["args"], serde_json::json!(["build", "--release"]));
        assert_eq!(merged["group"], "build");
    }

    #[test]
    fn merge_task_configs_nested() {
        let base: serde_json::Value = serde_json::json!({
            "presentation": { "reveal": "always", "echo": true }
        });
        let overlay: serde_json::Value = serde_json::json!({
            "presentation": { "reveal": "silent" }
        });

        let merged = merge_task_configs(&base, &overlay).unwrap();
        assert_eq!(merged["presentation"]["reveal"], "silent");
        assert_eq!(merged["presentation"]["echo"], true);
    }

    #[test]
    fn merge_task_configs_rejects_non_objects() {
        let base = serde_json::json!("not an object");
        let overlay = serde_json::json!({});
        assert!(merge_task_configs(&base, &overlay).is_err());
    }

    // -- task_presentation tests --------------------------------------------

    #[test]
    fn task_presentation_default_values() {
        let pres = TaskPresentation::default();
        assert_eq!(pres.reveal, TaskRevealKind::Always);
        assert!(!pres.focus);
        assert!(pres.echo);
        assert!(pres.show_reuse_message);
        assert_eq!(pres.panel, TaskPanelKind::Shared);
        assert!(!pres.clear);
        assert!(!pres.close);
    }

    #[test]
    fn task_presentation_from_json_full() {
        let json = serde_json::json!({
            "reveal": "silent",
            "focus": true,
            "echo": false,
            "showReuseMessage": false,
            "panel": "dedicated",
            "clear": true,
            "close": true,
        });
        let pres = task_presentation(&json);
        assert_eq!(pres.reveal, TaskRevealKind::Silent);
        assert!(pres.focus);
        assert!(!pres.echo);
        assert!(!pres.show_reuse_message);
        assert_eq!(pres.panel, TaskPanelKind::Dedicated);
        assert!(pres.clear);
        assert!(pres.close);
    }

    #[test]
    fn task_presentation_from_json_partial() {
        let json = serde_json::json!({ "reveal": "never" });
        let pres = task_presentation(&json);
        assert_eq!(pres.reveal, TaskRevealKind::Never);
        assert!(pres.echo); // default preserved
    }

    #[test]
    fn task_presentation_from_empty_json() {
        let pres = task_presentation(&serde_json::json!({}));
        assert_eq!(pres, TaskPresentation::default());
    }

    #[test]
    fn task_presentation_from_non_object() {
        let pres = task_presentation(&serde_json::json!("not an object"));
        assert_eq!(pres, TaskPresentation::default());
    }

    #[test]
    fn task_presentation_display() {
        let pres = TaskPresentation::default();
        let s = format!("{pres}");
        assert!(s.contains("reveal=Always"));
        assert!(s.contains("panel=Shared"));
    }

    #[test]
    fn task_presentation_serde_roundtrip() {
        let pres = TaskPresentation {
            reveal: TaskRevealKind::Silent,
            focus: true,
            echo: false,
            show_reuse_message: false,
            panel: TaskPanelKind::New,
            clear: true,
            close: true,
        };
        let json = serde_json::to_string(&pres).unwrap();
        let restored: TaskPresentation = serde_json::from_str(&json).unwrap();
        assert_eq!(pres, restored);
    }

    // -- TaskTemplate -------------------------------------------------------

    #[test]
    fn template_instantiate_replaces_placeholders() {
        let tpl = TaskTemplate::new(
            "cargo build",
            serde_json::json!({
                "label": "build ${profile}",
                "command": "cargo",
                "args": ["build", "--profile", "${profile}"]
            }),
            vec!["profile".into()],
        )
        .unwrap();

        let mut vars = BTreeMap::new();
        vars.insert("profile".into(), "release".into());

        let result = tpl.instantiate(&vars);
        assert_eq!(result["label"], "build release");
        assert_eq!(result["args"][2], "release");
    }

    #[test]
    fn template_missing_placeholders() {
        let tpl = TaskTemplate::new(
            "test",
            serde_json::json!({"a": "${x}", "b": "${y}"}),
            vec!["x".into(), "y".into()],
        )
        .unwrap();

        let mut vars = BTreeMap::new();
        vars.insert("x".into(), "1".into());

        assert!(!tpl.is_fully_bound(&vars));
        assert_eq!(tpl.missing_placeholders(&vars), vec!["y"]);

        vars.insert("y".into(), "2".into());
        assert!(tpl.is_fully_bound(&vars));
        assert!(tpl.missing_placeholders(&vars).is_empty());
    }

    #[test]
    fn template_rejects_non_object_config() {
        let result = TaskTemplate::new("bad", serde_json::json!([1, 2]), vec![]);
        assert!(result.is_err());
    }

    // -- TaskScheduler ------------------------------------------------------

    #[test]
    fn scheduler_linear_chain() {
        let mut sched = TaskScheduler::new();
        sched.add_task("compile");
        sched.add_dependency("link", "compile");
        sched.add_dependency("test", "link");

        let order = sched.schedule().unwrap();
        let pos = |name: &str| order.iter().position(|s| s == name).unwrap();
        assert!(pos("compile") < pos("link"));
        assert!(pos("link") < pos("test"));
    }

    #[test]
    fn scheduler_detects_cycle() {
        let mut sched = TaskScheduler::new();
        sched.add_dependency("a", "b");
        sched.add_dependency("b", "a");

        let result = sched.schedule();
        assert!(matches!(result, Err(TaskError::DependencyCycle(_))));
    }

    #[test]
    fn scheduler_independent_tasks() {
        let mut sched = TaskScheduler::new();
        sched.add_task("lint");
        sched.add_task("format");
        sched.add_task("docs");

        let order = sched.schedule().unwrap();
        assert_eq!(order.len(), 3);
    }

    // -- TaskExporter -------------------------------------------------------

    #[test]
    fn exporter_summary_counts() {
        let entries = vec![
            HistoryEntry {
                label: "build".into(),
                result: TaskResult::new(0, String::new(), String::new(), 100),
                started_at: 1,
            },
            HistoryEntry {
                label: "test".into(),
                result: TaskResult::new(1, String::new(), String::new(), 200),
                started_at: 2,
            },
        ];

        let summary = TaskExporter::summary(&entries);
        assert!(summary.contains("2 run(s)"));
        assert!(summary.contains("1 succeeded"));
        assert!(summary.contains("1 failed"));
    }

    #[test]
    fn exporter_empty_summary() {
        assert_eq!(TaskExporter::summary(&[]), "No task history.");
    }

    #[test]
    fn exporter_tasks_json_format() {
        let mut q = TaskQueue::new();
        q.enqueue("a", "build release", TaskPriority::High);

        let pending: Vec<QueuedTask> = {
            let refs = q.pending_tasks();
            refs.into_iter().cloned().collect()
        };
        let json = TaskExporter::export_tasks_json(&pending);
        assert_eq!(json["version"], "2.0.0");
        assert!(json["tasks"].is_array());
        assert_eq!(json["tasks"][0]["label"], "build release");
    }

    // -- New functionality tests --

    #[test]
    fn task_queue_drain() {
        let mut q = TaskQueue::new();
        q.enqueue("a", "Task A", TaskPriority::Normal);
        q.enqueue("b", "Task B", TaskPriority::High);
        let all = q.drain();
        assert_eq!(all.len(), 2);
        assert!(q.is_empty());
    }

    #[test]
    fn task_queue_find() {
        let mut q = TaskQueue::new();
        q.enqueue("build", "Build Project", TaskPriority::Normal);
        assert!(q.find("build").is_some());
        assert_eq!(q.find("build").unwrap().label, "Build Project");
        assert!(q.find("missing").is_none());
    }

    #[test]
    fn task_queue_cancel_all() {
        let mut q = TaskQueue::new();
        q.enqueue("a", "A", TaskPriority::Low);
        q.enqueue("b", "B", TaskPriority::High);
        q.enqueue("c", "C", TaskPriority::Normal);
        let count = q.cancel_all();
        assert_eq!(count, 3);
        assert_eq!(q.pending_tasks().len(), 0);
    }

    #[test]
    fn task_queue_priorities() {
        let mut q = TaskQueue::new();
        q.enqueue("a", "A", TaskPriority::Low);
        q.enqueue("b", "B", TaskPriority::Critical);
        q.enqueue("c", "C", TaskPriority::Low);
        let prios = q.priorities();
        assert_eq!(prios, vec![TaskPriority::Critical, TaskPriority::Low]);
    }

    #[test]
    fn task_result_merge_both_success() {
        let r1 = TaskResult::new(0, "out1".into(), String::new(), 100);
        let r2 = TaskResult::new(0, "out2".into(), String::new(), 200);
        let merged = r1.merge(&r2);
        assert_eq!(merged.exit_code, 0);
        assert!(merged.is_success());
        assert_eq!(merged.duration_ms, 300);
        assert!(merged.stdout.contains("out1"));
        assert!(merged.stdout.contains("out2"));
    }

    #[test]
    fn task_result_merge_one_failed() {
        let r1 = TaskResult::new(0, "ok".into(), String::new(), 50);
        let r2 = TaskResult::new(1, String::new(), "error".into(), 50);
        let merged = r1.merge(&r2);
        assert_eq!(merged.exit_code, 1);
        assert!(!merged.is_success());
    }

    #[test]
    fn task_queue_requeue() {
        let mut q = TaskQueue::new();
        q.enqueue("a", "A", TaskPriority::Normal);
        q.cancel("a").unwrap();
        assert!(q.pending_tasks().is_empty());
        q.requeue("a").unwrap();
        assert_eq!(q.pending_tasks().len(), 1);
    }

    #[test]
    fn task_queue_requeue_not_terminal_fails() {
        let mut q = TaskQueue::new();
        q.enqueue("a", "A", TaskPriority::Normal);
        assert!(q.requeue("a").is_err());
    }

    #[test]
    fn task_queue_sort_by_priority() {
        let mut q = TaskQueue::new();
        q.enqueue("a", "A", TaskPriority::Low);
        q.enqueue("b", "B", TaskPriority::Critical);
        q.enqueue("c", "C", TaskPriority::Normal);
        q.sort_by_priority();
        let all = q.drain();
        assert_eq!(all[0].priority, TaskPriority::Critical);
        assert_eq!(all[1].priority, TaskPriority::Normal);
        assert_eq!(all[2].priority, TaskPriority::Low);
    }

    #[test]
    fn task_queue_count_by_status() {
        let mut q = TaskQueue::new();
        q.enqueue("a", "A", TaskPriority::Normal);
        q.enqueue("b", "B", TaskPriority::Normal);
        q.enqueue("c", "C", TaskPriority::Normal);
        q.cancel("b").unwrap();
        assert_eq!(q.count_by_status(TaskRunStatus::Pending), 2);
        assert_eq!(q.count_by_status(TaskRunStatus::Cancelled), 1);
    }

    // -- TaskSource ---------------------------------------------------------

    #[test]
    fn task_source_display() {
        assert_eq!(TaskSource::Workspace.to_string(), "workspace");
        assert_eq!(TaskSource::User.to_string(), "user");
        assert_eq!(
            TaskSource::AutoDetected {
                provider: "npm".into()
            }
            .to_string(),
            "auto-detected (npm)"
        );
        assert_eq!(
            TaskSource::Extension {
                id: "rust-analyzer".into()
            }
            .to_string(),
            "extension (rust-analyzer)"
        );
    }

    #[test]
    fn task_source_serde_roundtrip() {
        let src = TaskSource::AutoDetected {
            provider: "cargo".into(),
        };
        let json = serde_json::to_string(&src).unwrap();
        let restored: TaskSource = serde_json::from_str(&json).unwrap();
        assert_eq!(src, restored);
    }

    // -- TaskRunOptions -----------------------------------------------------

    #[test]
    fn run_options_defaults() {
        let opts = TaskRunOptions::default();
        assert!(opts.reevaluate_on_rerun);
        assert_eq!(opts.instance_limit, TaskInstancePolicy::Terminate);
    }

    #[test]
    fn parse_run_options_from_json() {
        let json = serde_json::json!({
            "reevaluateOnRerun": false,
            "instanceLimit": "parallel"
        });
        let opts = parse_run_options(&json);
        assert!(!opts.reevaluate_on_rerun);
        assert_eq!(opts.instance_limit, TaskInstancePolicy::Parallel);
    }

    #[test]
    fn parse_run_options_partial() {
        let json = serde_json::json!({ "instanceLimit": "ignore" });
        let opts = parse_run_options(&json);
        assert!(opts.reevaluate_on_rerun); // default
        assert_eq!(opts.instance_limit, TaskInstancePolicy::Ignore);
    }

    #[test]
    fn parse_run_options_empty() {
        let opts = parse_run_options(&serde_json::json!({}));
        assert_eq!(opts, TaskRunOptions::default());
    }

    // -- Shell quoting ------------------------------------------------------

    #[test]
    fn shell_quote_posix_simple() {
        assert_eq!(shell_quote_posix("hello"), "hello");
        assert_eq!(shell_quote_posix(""), "''");
    }

    #[test]
    fn shell_quote_posix_special_chars() {
        assert_eq!(shell_quote_posix("hello world"), "'hello world'");
        assert_eq!(
            shell_quote_posix("it's"),
            "'it'\\''s'"
        );
    }

    #[test]
    fn shell_quote_cmd_simple() {
        assert_eq!(shell_quote_cmd("hello"), "hello");
        assert_eq!(shell_quote_cmd(""), "\"\"");
    }

    #[test]
    fn shell_quote_cmd_special_chars() {
        assert_eq!(shell_quote_cmd("hello world"), "\"hello world\"");
        assert_eq!(shell_quote_cmd("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(shell_quote_cmd("100%"), "\"100%%\"");
    }

    #[test]
    fn build_shell_command_basic() {
        let cmd = build_shell_command("cargo", &["build", "--release"]);
        assert_eq!(cmd, "cargo build --release");
    }

    #[test]
    fn build_shell_command_with_spaces() {
        let cmd = build_shell_command("my program", &["arg one", "arg2"]);
        assert_eq!(cmd, "'my program' 'arg one' arg2");
    }

    // -- Label formatting ---------------------------------------------------

    #[test]
    fn format_task_label_with_group() {
        assert_eq!(format_task_label(Some("build"), "release"), "build: release");
    }

    #[test]
    fn format_task_label_without_group() {
        assert_eq!(format_task_label(None, "lint"), "lint");
        assert_eq!(format_task_label(Some(""), "lint"), "lint");
    }

    #[test]
    fn normalize_task_label_collapses_spaces() {
        assert_eq!(normalize_task_label("  build   release  "), "build release");
        assert_eq!(normalize_task_label("already fine"), "already fine");
    }

    // -- TaskDependencyGraph ------------------------------------------------

    #[test]
    fn dep_graph_execution_levels() {
        let mut g = TaskDependencyGraph::new();
        g.add_task("compile");
        g.add_dependency("link", "compile");
        g.add_dependency("test", "link");

        let levels = g.execution_levels().unwrap();
        assert_eq!(levels.len(), 3);
        assert!(levels[0].contains(&"compile".to_string()));
        assert!(levels[1].contains(&"link".to_string()));
        assert!(levels[2].contains(&"test".to_string()));
    }

    #[test]
    fn dep_graph_parallel_roots() {
        let mut g = TaskDependencyGraph::new();
        g.add_task("lint");
        g.add_task("format");
        g.add_dependency("check", "lint");
        g.add_dependency("check", "format");

        let levels = g.execution_levels().unwrap();
        assert_eq!(levels.len(), 2);
        // lint and format should be in level 0
        assert!(levels[0].contains(&"lint".to_string()));
        assert!(levels[0].contains(&"format".to_string()));
        assert!(levels[1].contains(&"check".to_string()));
    }

    #[test]
    fn dep_graph_cycle_detected() {
        let mut g = TaskDependencyGraph::new();
        g.add_dependency("a", "b");
        g.add_dependency("b", "a");
        assert!(matches!(
            g.execution_levels(),
            Err(TaskError::DependencyCycle(_))
        ));
    }

    #[test]
    fn dep_graph_root_and_leaf_tasks() {
        let mut g = TaskDependencyGraph::new();
        g.add_task("compile");
        g.add_dependency("link", "compile");
        g.add_dependency("test", "link");

        let roots = g.root_tasks();
        assert_eq!(roots, vec!["compile"]);

        let leaves = g.leaf_tasks();
        assert_eq!(leaves, vec!["test"]);
    }

    #[test]
    fn dep_graph_depends_on_transitive() {
        let mut g = TaskDependencyGraph::new();
        g.add_task("a");
        g.add_dependency("b", "a");
        g.add_dependency("c", "b");

        assert!(g.depends_on("c", "a"));
        assert!(g.depends_on("c", "b"));
        assert!(!g.depends_on("a", "c"));
        assert!(!g.depends_on("a", "a")); // self
    }

    // -- CompiledPattern / DiagnosticMatch ----------------------------------

    #[test]
    fn compiled_pattern_matches_gcc_style() {
        let pat = CompiledPattern::new(
            r"^(.+):(\d+):(\d+):\s+(warning|error):\s+(.+)$",
            Some(1),
            Some(2),
            Some(3),
            Some(5),
            Some(4),
        )
        .unwrap();

        let diag = pat
            .match_line("main.c:10:5: error: undeclared identifier")
            .unwrap();
        assert_eq!(diag.file.as_deref(), Some("main.c"));
        assert_eq!(diag.line, Some(10));
        assert_eq!(diag.column, Some(5));
        assert_eq!(diag.severity.as_deref(), Some("error"));
        assert_eq!(diag.message.as_deref(), Some("undeclared identifier"));
    }

    #[test]
    fn compiled_pattern_no_match() {
        let pat = CompiledPattern::new(
            r"^ERROR:\s+(.+)$",
            None,
            None,
            None,
            Some(1),
            None,
        )
        .unwrap();
        assert!(pat.match_line("all good").is_none());
    }

    #[test]
    fn compiled_pattern_invalid_regex() {
        let result = CompiledPattern::new(
            r"[invalid",
            None,
            None,
            None,
            None,
            None,
        );
        assert!(result.is_err());
    }

    // -- Task JSON parsing helpers ------------------------------------------

    #[test]
    fn extract_task_labels_basic() {
        let json = serde_json::json!({
            "version": "2.0.0",
            "tasks": [
                { "label": "build", "command": "cargo build" },
                { "label": "test", "command": "cargo test" },
                { "command": "no-label" }
            ]
        });
        let labels = extract_task_labels(&json);
        assert_eq!(labels, vec!["build", "test"]);
    }

    #[test]
    fn extract_task_labels_empty() {
        assert!(extract_task_labels(&serde_json::json!({})).is_empty());
        assert!(extract_task_labels(&serde_json::json!({ "tasks": [] })).is_empty());
    }

    #[test]
    fn extract_depends_on_array() {
        let json = serde_json::json!({
            "tasks": [
                {
                    "label": "all",
                    "dependsOn": ["build", "lint"]
                }
            ]
        });
        let deps = extract_depends_on(&json, "all");
        assert_eq!(deps, vec!["build", "lint"]);
    }

    #[test]
    fn extract_depends_on_string() {
        let json = serde_json::json!({
            "tasks": [
                { "label": "test", "dependsOn": "build" }
            ]
        });
        let deps = extract_depends_on(&json, "test");
        assert_eq!(deps, vec!["build"]);
    }

    #[test]
    fn extract_depends_on_missing() {
        let json = serde_json::json!({ "tasks": [{ "label": "build" }] });
        assert!(extract_depends_on(&json, "build").is_empty());
        assert!(extract_depends_on(&json, "nonexistent").is_empty());
    }

    #[test]
    fn build_dependency_graph_from_json() {
        let json = serde_json::json!({
            "tasks": [
                { "label": "compile" },
                { "label": "link", "dependsOn": ["compile"] },
                { "label": "test", "dependsOn": ["link"] }
            ]
        });
        let graph = build_dependency_graph(&json).unwrap();
        assert_eq!(graph.task_count(), 3);
        let levels = graph.execution_levels().unwrap();
        assert_eq!(levels.len(), 3);
    }

    #[test]
    fn filter_tasks_by_group_simple() {
        let json = serde_json::json!({
            "tasks": [
                { "label": "build", "group": "build" },
                { "label": "test", "group": "test" },
                { "label": "lint", "group": "build" }
            ]
        });
        let build_tasks = filter_tasks_by_group(&json, "build");
        assert_eq!(build_tasks.len(), 2);
    }

    #[test]
    fn filter_tasks_by_group_detailed() {
        let json = serde_json::json!({
            "tasks": [
                {
                    "label": "build-release",
                    "group": { "kind": "build", "isDefault": true }
                },
                { "label": "test", "group": "test" }
            ]
        });
        let build_tasks = filter_tasks_by_group(&json, "build");
        assert_eq!(build_tasks.len(), 1);
        assert_eq!(build_tasks[0]["label"], "build-release");
    }

    #[test]
    fn find_default_task_found() {
        let json = serde_json::json!({
            "tasks": [
                { "label": "build-debug", "group": { "kind": "build", "isDefault": false } },
                { "label": "build-release", "group": { "kind": "build", "isDefault": true } }
            ]
        });
        assert_eq!(find_default_task(&json, "build"), Some("build-release".into()));
    }

    #[test]
    fn find_default_task_none() {
        let json = serde_json::json!({
            "tasks": [
                { "label": "build", "group": "build" }
            ]
        });
        assert_eq!(find_default_task(&json, "build"), None);
    }
}
