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
use std::collections::HashMap;
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


// ---------------------------------------------------------------------------
// tasks – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XTasksLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XTasksPanelState {
    pub region: XTasksLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XTasksPanelState {
    pub fn new(region: XTasksLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_tasks_total_visible_area(panels: &[XTasksPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_tasks_count_in_region(
    panels: &[XTasksPanelState],
    region: XTasksLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_tasks_widest_panel(panels: &[XTasksPanelState]) -> Option<&XTasksPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_tasks_collapse_region(
    panels: &mut [XTasksPanelState],
    region: XTasksLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XTasksLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XTasksLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}


/// Configuration manager for tasks functionality.
pub struct TasksConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl TasksConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &TasksConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for tasks operations.
pub struct TasksRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl TasksRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for tasks.
pub struct TasksValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl TasksValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &TasksValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
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
// xa_ extended helpers for tasks
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaTasksRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaTasksRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaTasksCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaTasksCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaTasksCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 174
// ---------------------------------------------------------------------------

/// Generic object pool `Xc174Pool<T>`.
pub struct Xc174Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc174Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc174PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc174Pool<T> {
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
    pub fn stats(&self) -> Xc174PoolStats {
        Xc174PoolStats {
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

impl<T> Default for Xc174Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc174Scheduler`.
pub struct Xc174Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc174Scheduler {
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

impl Default for Xc174Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_174 hash for the given byte slice.
pub fn xc_174_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_174 convention.
pub fn xc_174_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_64 deepening: state machine + event bus ---

/// States for the Xd64 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd64State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd64State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd64Transition {
    pub from: Xd64State,
    pub to: Xd64State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd64StateMachine {
    current: Xd64State,
    history: Vec<Xd64Transition>,
    step_counter: usize,
}

impl Xd64StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd64State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd64State {
        self.current
    }

    pub fn history(&self) -> &[Xd64Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd64State) -> Result<Xd64State, String> {
        let allowed = match (self.current, target) {
            (Xd64State::Idle, Xd64State::Running) => true,
            (Xd64State::Running, Xd64State::Paused) => true,
            (Xd64State::Running, Xd64State::Done) => true,
            (Xd64State::Paused, Xd64State::Running) => true,
            (Xd64State::Paused, Xd64State::Done) => true,
            (Xd64State::Done, Xd64State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_64: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd64Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd64SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd64State> {
        let prefix = "Xd64SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd64State::Idle),
            "Running" => Some(Xd64State::Running),
            "Paused" => Some(Xd64State::Paused),
            "Done" => Some(Xd64State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd64State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd64 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd64Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd64Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd64HandlerFn = Box<dyn Fn(&Xd64Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd64EventBus {
    handlers: Vec<(usize, Option<String>, Xd64HandlerFn)>,
    next_id: usize,
    published: Vec<Xd64Event>,
}

impl Xd64EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd64Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd64Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd64Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd64Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #65
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf65Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf65TrieNode {
    children: std::collections::HashMap<char, Xf65TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf65Trie {
    root: Xf65TrieNode,
    count: usize,
}

impl Xf65Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf65TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf65TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf65TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf65BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf65BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 173).
pub struct Xh173SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh173SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 215 as u64,
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

/// A compact bit set supporting boolean operations (variant 173).
pub struct Xh173BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh173BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 173).
pub struct Xi173Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi173Deque<T> {
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
pub struct Xi173Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi173Interval {
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

/// A simple interval tree (variant 173).
pub struct Xi173IntervalTree {
    xi_intervals: Vec<Xi173Interval>,
}

impl Xi173IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi173Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi173Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi173Interval) -> Vec<&Xi173Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi173Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi173Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi173Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi173Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi173Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi173Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 173) ---

/// Disjoint set / union-find for crate 173.
pub struct Xj173UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj173UnionFind {
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

const XJ173_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 173.
pub struct Xj173BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj173BTreeNode<K, V>>>,
    len: usize,
}

struct Xj173BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj173BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj173BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ173_BTREE_ORDER - 1
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
        let mid = XJ173_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj173BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj173BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj173BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj173BTreeNode::xj_new_leaf();
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

    // -- tasks additional tests -------------------------------------------

    #[test]
    fn x_tasks_panel_state_new() {
        let p = XTasksPanelState::new(XTasksLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XTasksLayoutRegion::Sidebar);
    }

    #[test]
    fn x_tasks_panel_area() {
        let p = XTasksPanelState::new(XTasksLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_tasks_panel_toggle() {
        let mut p = XTasksPanelState::new(XTasksLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_tasks_panel_resize() {
        let mut p = XTasksPanelState::new(XTasksLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_tasks_panel_is_narrow() {
        let mut p = XTasksPanelState::new(XTasksLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_tasks_total_visible_area_basic() {
        let panels = vec![
            XTasksPanelState::new(XTasksLayoutRegion::Sidebar, "a"),
            XTasksPanelState::new(XTasksLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_tasks_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_tasks_total_visible_area_hidden() {
        let mut panels = vec![
            XTasksPanelState::new(XTasksLayoutRegion::Sidebar, "a"),
            XTasksPanelState::new(XTasksLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_tasks_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_tasks_count_in_region_basic() {
        let panels = vec![
            XTasksPanelState::new(XTasksLayoutRegion::Sidebar, "a"),
            XTasksPanelState::new(XTasksLayoutRegion::Sidebar, "b"),
            XTasksPanelState::new(XTasksLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_tasks_count_in_region(&panels, XTasksLayoutRegion::Sidebar), 2);
        assert_eq!(x_tasks_count_in_region(&panels, XTasksLayoutRegion::Editor), 1);
        assert_eq!(x_tasks_count_in_region(&panels, XTasksLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_tasks_widest_panel_basic() {
        let mut panels = vec![
            XTasksPanelState::new(XTasksLayoutRegion::Sidebar, "narrow"),
            XTasksPanelState::new(XTasksLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_tasks_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_tasks_collapse_region_basic() {
        let mut panels = vec![
            XTasksPanelState::new(XTasksLayoutRegion::Sidebar, "a"),
            XTasksPanelState::new(XTasksLayoutRegion::Sidebar, "b"),
            XTasksPanelState::new(XTasksLayoutRegion::Editor, "c"),
        ];
        x_tasks_collapse_region(&mut panels, XTasksLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_tasks_layout_constraint_clamp() {
        let lc = XTasksLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_tasks_layout_constraint_satisfied() {
        let lc = XTasksLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_tasks_widest_panel_empty() {
        let panels: Vec<XTasksPanelState> = vec![];
        assert!(x_tasks_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_tasks_layout_region_eq() {
        assert_eq!(XTasksLayoutRegion::Sidebar, XTasksLayoutRegion::Sidebar);
        assert_ne!(XTasksLayoutRegion::Sidebar, XTasksLayoutRegion::Panel);
    }


    #[test]
    fn tasks_config_new() {
        let cfg = TasksConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn tasks_config_set_get() {
        let mut cfg = TasksConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn tasks_config_remove() {
        let mut cfg = TasksConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn tasks_config_keys_sorted() {
        let mut cfg = TasksConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn tasks_config_bump_version() {
        let mut cfg = TasksConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn tasks_config_clear() {
        let mut cfg = TasksConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn tasks_config_merge() {
        let mut cfg1 = TasksConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = TasksConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn tasks_config_disable() {
        let mut cfg = TasksConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn tasks_rate_tracker_empty() {
        let rt = TasksRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn tasks_rate_tracker_record() {
        let mut rt = TasksRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn tasks_rate_tracker_prune() {
        let mut rt = TasksRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn tasks_validator_valid() {
        let v = TasksValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn tasks_validator_errors() {
        let mut v = TasksValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn tasks_validator_clear() {
        let mut v = TasksValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn tasks_validator_merge() {
        let mut v1 = TasksValidator::new();
        v1.add_error("e1");
        let mut v2 = TasksValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn tasks_rate_tracker_clear() {
        let mut rt = TasksRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
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


    // xa_ extended tests for tasks
    #[test]
    fn xa_tasks_ring_new() {
        let rb = super::XaTasksRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_tasks_ring_push_len() {
        let mut rb = super::XaTasksRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_tasks_ring_wrap() {
        let mut rb = super::XaTasksRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_tasks_ring_mean_empty() {
        let rb = super::XaTasksRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_tasks_ring_mean_values() {
        let mut rb = super::XaTasksRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_tasks_ring_min_max() {
        let mut rb = super::XaTasksRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_tasks_ring_iter() {
        let mut rb = super::XaTasksRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_tasks_counter_new() {
        let c = super::XaTasksCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_tasks_counter_inc() {
        let mut c = super::XaTasksCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_tasks_counter_inc_by() {
        let mut c = super::XaTasksCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_tasks_counter_reset() {
        let mut c = super::XaTasksCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_tasks_counter_clear() {
        let mut c = super::XaTasksCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_tasks_counter_default() {
        let c = super::XaTasksCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 174 ----

    #[test]
    fn xc_174_pool_new_empty() {
        let pool: super::Xc174Pool<i32> = super::Xc174Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_174_pool_release_acquire() {
        let mut pool = super::Xc174Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_174_pool_acquire_empty() {
        let mut pool: super::Xc174Pool<i32> = super::Xc174Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_174_pool_full() {
        let mut pool = super::Xc174Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_174_pool_drain() {
        let mut pool = super::Xc174Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_174_pool_stats() {
        let mut pool = super::Xc174Pool::new(8);
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
    fn xc_174_pool_clear() {
        let mut pool = super::Xc174Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_174_pool_shrink() {
        let mut pool = super::Xc174Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_174_pool_default() {
        let pool: super::Xc174Pool<String> = super::Xc174Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_174_pool_extend() {
        let mut pool = super::Xc174Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_174_pool_retain() {
        let mut pool = super::Xc174Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_174_scheduler_round_robin() {
        let mut sched = super::Xc174Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_174_scheduler_empty() {
        let mut sched = super::Xc174Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_174_scheduler_reset() {
        let mut sched = super::Xc174Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_174_scheduler_add_remove() {
        let mut sched = super::Xc174Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_174_scheduler_targets() {
        let sched = super::Xc174Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_174_hash_empty() {
        assert_eq!(super::xc_174_hash(b""), 5381);
    }

    #[test]
    fn xc_174_hash_data() {
        let h = super::xc_174_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_174_hash(b"hello"), h);
    }

    #[test]
    fn xc_174_reverse_str() {
        assert_eq!(super::xc_174_reverse("abc"), "cba");
        assert_eq!(super::xc_174_reverse(""), "");
    }


    // --- xd_64 deepening tests ---

    #[test]
    fn xd_64_sm_initial_state() {
        let sm = Xd64StateMachine::new();
        assert_eq!(sm.current_state(), Xd64State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_64_sm_valid_idle_to_running() {
        let mut sm = Xd64StateMachine::new();
        assert!(sm.transition(Xd64State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd64State::Running);
    }

    #[test]
    fn xd_64_sm_valid_running_to_paused() {
        let mut sm = Xd64StateMachine::new();
        sm.transition(Xd64State::Running).unwrap();
        assert!(sm.transition(Xd64State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd64State::Paused);
    }

    #[test]
    fn xd_64_sm_valid_running_to_done() {
        let mut sm = Xd64StateMachine::new();
        sm.transition(Xd64State::Running).unwrap();
        assert!(sm.transition(Xd64State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd64State::Done);
    }

    #[test]
    fn xd_64_sm_valid_paused_to_running() {
        let mut sm = Xd64StateMachine::new();
        sm.transition(Xd64State::Running).unwrap();
        sm.transition(Xd64State::Paused).unwrap();
        assert!(sm.transition(Xd64State::Running).is_ok());
    }

    #[test]
    fn xd_64_sm_valid_done_to_idle() {
        let mut sm = Xd64StateMachine::new();
        sm.transition(Xd64State::Running).unwrap();
        sm.transition(Xd64State::Done).unwrap();
        assert!(sm.transition(Xd64State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd64State::Idle);
    }

    #[test]
    fn xd_64_sm_invalid_idle_to_done() {
        let mut sm = Xd64StateMachine::new();
        assert!(sm.transition(Xd64State::Done).is_err());
    }

    #[test]
    fn xd_64_sm_invalid_idle_to_paused() {
        let mut sm = Xd64StateMachine::new();
        assert!(sm.transition(Xd64State::Paused).is_err());
    }

    #[test]
    fn xd_64_sm_history_tracking() {
        let mut sm = Xd64StateMachine::new();
        sm.transition(Xd64State::Running).unwrap();
        sm.transition(Xd64State::Paused).unwrap();
        sm.transition(Xd64State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd64State::Idle);
        assert_eq!(sm.history()[0].to, Xd64State::Running);
        assert_eq!(sm.history()[1].from, Xd64State::Running);
        assert_eq!(sm.history()[2].to, Xd64State::Done);
    }

    #[test]
    fn xd_64_sm_serialize_deserialize() {
        let mut sm = Xd64StateMachine::new();
        sm.transition(Xd64State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd64StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd64State::Running));
    }

    #[test]
    fn xd_64_sm_deserialize_invalid() {
        assert_eq!(Xd64StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_64_sm_reset() {
        let mut sm = Xd64StateMachine::new();
        sm.transition(Xd64State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd64State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_64_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd64EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd64Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_64_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd64EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd64Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd64Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_64_bus_unsubscribe() {
        let mut bus = Xd64EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_64_event_kind_and_payload() {
        let e = Xd64Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd64Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_64_bus_clear_history() {
        let mut bus = Xd64EventBus::new();
        bus.publish(Xd64Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_64_sm_step_counter_increments() {
        let mut sm = Xd64StateMachine::new();
        sm.transition(Xd64State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd64State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #65 --

    #[test]
    fn xf65_trie_insert_search() {
        let mut t = Xf65Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf65_trie_starts_with() {
        let mut t = Xf65Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf65_trie_remove() {
        let mut t = Xf65Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf65_trie_word_count() {
        let mut t = Xf65Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf65_trie_longest_prefix() {
        let mut t = Xf65Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf65_trie_all_words() {
        let mut t = Xf65Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf65_trie_autocomplete() {
        let mut t = Xf65Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf65_trie_empty_search() {
        let t = Xf65Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf65_bloom_add_contains() {
        let mut bf = Xf65BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf65_bloom_probably_absent() {
        let bf = Xf65BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf65_bloom_false_positive_rate() {
        let mut bf = Xf65BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf65_bloom_clear() {
        let mut bf = Xf65BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf65_bloom_union() {
        let mut a = Xf65BloomFilter::xf_new(512, 2);
        let mut b = Xf65BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf65_bloom_intersection_estimate() {
        let mut a = Xf65BloomFilter::xf_new(512, 2);
        let mut b = Xf65BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf65_bloom_union_size_mismatch() {
        let a = Xf65BloomFilter::xf_new(256, 2);
        let b = Xf65BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh173_skip_insert_contains() {
        let mut sl = super::Xh173SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh173_skip_remove() {
        let mut sl = super::Xh173SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh173_skip_len() {
        let mut sl = super::Xh173SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh173_skip_range_query() {
        let mut sl = super::Xh173SkipList::xh_new(4);
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
    fn xh173_skip_floor_ceiling() {
        let mut sl = super::Xh173SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh173_skip_rank() {
        let mut sl = super::Xh173SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh173_skip_empty() {
        let sl = super::Xh173SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh173_skip_duplicates() {
        let mut sl = super::Xh173SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh173_bitset_set_test() {
        let mut bs = super::Xh173BitSet::xh_new(256);
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
    fn xh173_bitset_clear_count() {
        let mut bs = super::Xh173BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh173_bitset_and_or_xor() {
        let mut a = super::Xh173BitSet::xh_new(128);
        let mut b = super::Xh173BitSet::xh_new(128);
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
    fn xh173_bitset_iter_ones() {
        let mut bs = super::Xh173BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh173_bitset_first_last() {
        let mut bs = super::Xh173BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh173_bitset_empty() {
        let bs = super::Xh173BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi173_deque_push_pop_back() {
        let mut dq = super::Xi173Deque::xi_new(4);
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
    fn xi173_deque_push_pop_front() {
        let mut dq = super::Xi173Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi173_deque_mixed_ops() {
        let mut dq = super::Xi173Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi173_deque_get_and_split() {
        let mut dq = super::Xi173Deque::xi_new(8);
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
    fn xi173_deque_rotate_left() {
        let mut dq = super::Xi173Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi173_deque_rotate_right() {
        let mut dq = super::Xi173Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi173_deque_grow() {
        let mut dq = super::Xi173Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi173_deque_empty() {
        let dq = super::Xi173Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi173_interval_tree_insert_query() {
        let mut tree = super::Xi173IntervalTree::xi_new();
        tree.xi_insert(super::Xi173Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi173Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi173Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi173_interval_tree_overlap() {
        let mut tree = super::Xi173IntervalTree::xi_new();
        tree.xi_insert(super::Xi173Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi173Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi173Interval::xi_new(12, 20));
        let q = super::Xi173Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi173_interval_tree_remove() {
        let mut tree = super::Xi173IntervalTree::xi_new();
        tree.xi_insert(super::Xi173Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi173Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi173_interval_tree_gaps() {
        let mut tree = super::Xi173IntervalTree::xi_new();
        tree.xi_insert(super::Xi173Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi173Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi173Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi173Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi173Interval::xi_new(8, 10));
    }

    #[test]
    fn xi173_interval_tree_merge() {
        let mut tree = super::Xi173IntervalTree::xi_new();
        tree.xi_insert(super::Xi173Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi173Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi173Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi173Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi173Interval::xi_new(10, 15));
    }

    #[test]
    fn xi173_interval_tree_all() {
        let mut tree = super::Xi173IntervalTree::xi_new();
        tree.xi_insert(super::Xi173Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi173Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi173_interval_tree_empty() {
        let tree = super::Xi173IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi173_interval_tree_contains_point() {
        let iv = super::Xi173Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 173) ---

    #[test]
    fn xj_173_uf_make_and_find() {
        let mut uf = super::Xj173UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_173_uf_union_connected() {
        let mut uf = super::Xj173UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_173_uf_component_count() {
        let mut uf = super::Xj173UnionFind::xj_new();
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
    fn xj_173_uf_component_size() {
        let mut uf = super::Xj173UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_173_uf_largest_component() {
        let mut uf = super::Xj173UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_173_uf_many_elements() {
        let mut uf = super::Xj173UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_173_uf_separate_components() {
        let mut uf = super::Xj173UnionFind::xj_new();
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
    fn xj_173_uf_path_compression() {
        let mut uf = super::Xj173UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_173_bt_insert_get() {
        let mut bt = super::Xj173BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_173_bt_contains_len() {
        let mut bt = super::Xj173BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_173_bt_replace() {
        let mut bt = super::Xj173BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_173_bt_remove() {
        let mut bt = super::Xj173BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_173_bt_keys_values() {
        let mut bt = super::Xj173BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_173_bt_range() {
        let mut bt = super::Xj173BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_173_bt_min_max() {
        let mut bt = super::Xj173BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_173_bt_many_inserts() {
        let mut bt = super::Xj173BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }

}
