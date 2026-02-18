//! Ext API: Progress.
//!
//! RPC bridge between the extension host and the main thread for progress reporting.

use std::fmt;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_progress";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ProgressMessage {
    Start {
        handle: u64,
        options: ProgressOptions,
    },
    Report {
        handle: u64,
        increment: Option<f64>,
        message: Option<String>,
    },
    End {
        handle: u64,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ProgressLocation {
    SourceControl,
    Window,
    Notification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProgressOptions {
    pub location: ProgressLocation,
    pub title: Option<String>,
    pub cancellable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProgressState {
    pub handle: u64,
    pub percentage: f64,
    pub message: Option<String>,
    pub is_done: bool,
}

// ── Bridge ──

pub struct ProgressBridge {
    active: Vec<ProgressState>,
}

impl ProgressBridge {
    pub fn new() -> Self {
        Self {
            active: Vec::new(),
        }
    }

    pub fn start(&mut self, handle: u64, options: &ProgressOptions) {
        self.active.push(ProgressState {
            handle,
            percentage: 0.0,
            message: options.title.clone(),
            is_done: false,
        });
    }

    pub fn report(&mut self, handle: u64, increment: Option<f64>, message: Option<String>) {
        if let Some(state) = self.active.iter_mut().find(|s| s.handle == handle) {
            if let Some(inc) = increment {
                state.percentage = (state.percentage + inc).min(100.0);
            }
            if message.is_some() {
                state.message = message;
            }
        }
    }

    pub fn end(&mut self, handle: u64) {
        if let Some(state) = self.active.iter_mut().find(|s| s.handle == handle) {
            state.is_done = true;
            state.percentage = 100.0;
        }
    }

    pub fn active_count(&self) -> usize {
        self.active.iter().filter(|s| !s.is_done).count()
    }

    pub fn get_state(&self, handle: u64) -> Option<&ProgressState> {
        self.active.iter().find(|s| s.handle == handle)
    }

    pub fn handle_message(&mut self, msg: &ProgressMessage) -> serde_json::Value {
        match msg {
            ProgressMessage::Start { handle, options } => {
                self.start(*handle, options);
                serde_json::json!({"started": handle})
            }
            ProgressMessage::Report {
                handle,
                increment,
                message,
            } => {
                self.report(*handle, *increment, message.clone());
                serde_json::json!({"reported": handle})
            }
            ProgressMessage::End { handle } => {
                self.end(*handle);
                serde_json::json!({"ended": handle})
            }
        }
    }
}

impl Default for ProgressBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ── Error Types ──

/// Errors that can occur during progress operations.
#[derive(Debug, Clone, PartialEq)]
pub enum ProgressError {
    /// The specified handle does not correspond to any active progress.
    HandleNotFound(u64),
    /// The increment value is invalid (negative or NaN).
    InvalidIncrement(String),
    /// A progress with this handle already exists.
    DuplicateHandle(u64),
    /// The title exceeds the maximum allowed length.
    TitleTooLong { max: usize, actual: usize },
    /// The percentage is out of the valid 0..=100 range.
    PercentageOutOfRange(f64),
}

impl std::fmt::Display for ProgressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProgressError::HandleNotFound(h) => write!(f, "progress handle {h} not found"),
            ProgressError::InvalidIncrement(reason) => {
                write!(f, "invalid increment: {reason}")
            }
            ProgressError::DuplicateHandle(h) => {
                write!(f, "progress handle {h} already exists")
            }
            ProgressError::TitleTooLong { max, actual } => {
                write!(f, "title length {actual} exceeds maximum {max}")
            }
            ProgressError::PercentageOutOfRange(v) => {
                write!(f, "percentage {v} is outside 0..=100")
            }
        }
    }
}

impl std::error::Error for ProgressError {}

// ── Display implementations ──

impl std::fmt::Display for ProgressLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProgressLocation::SourceControl => write!(f, "Source Control"),
            ProgressLocation::Window => write!(f, "Window"),
            ProgressLocation::Notification => write!(f, "Notification"),
        }
    }
}

impl std::fmt::Display for ProgressState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = if self.is_done { "done" } else { "active" };
        let msg = self.message.as_deref().unwrap_or("(no message)");
        write!(
            f,
            "[handle={}] {:.1}% — {} [{}]",
            self.handle, self.percentage, msg, status
        )
    }
}

impl std::fmt::Display for ProgressOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let title = self.title.as_deref().unwrap_or("(untitled)");
        let cancel = if self.cancellable { "cancellable" } else { "non-cancellable" };
        write!(f, "{} @ {} [{}]", title, self.location, cancel)
    }
}

// ── ProgressOptions builder ──

/// Maximum allowed title length for validation.
const MAX_TITLE_LEN: usize = 256;

/// Builder for constructing [`ProgressOptions`] with validation.
#[derive(Debug, Clone)]
pub struct ProgressOptionsBuilder {
    location: ProgressLocation,
    title: Option<String>,
    cancellable: bool,
}

impl ProgressOptionsBuilder {
    pub fn new(location: ProgressLocation) -> Self {
        Self {
            location,
            title: None,
            cancellable: false,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn cancellable(mut self, cancellable: bool) -> Self {
        self.cancellable = cancellable;
        self
    }

    /// Build the [`ProgressOptions`], validating constraints.
    pub fn build(self) -> Result<ProgressOptions, ProgressError> {
        if let Some(ref t) = self.title {
            if t.len() > MAX_TITLE_LEN {
                return Err(ProgressError::TitleTooLong {
                    max: MAX_TITLE_LEN,
                    actual: t.len(),
                });
            }
        }
        Ok(ProgressOptions {
            location: self.location,
            title: self.title,
            cancellable: self.cancellable,
        })
    }
}

// ── ProgressState helpers ──

impl ProgressState {
    /// Returns the remaining percentage until completion.
    pub fn remaining(&self) -> f64 {
        (100.0 - self.percentage).max(0.0)
    }

    /// Returns `true` if this progress has reached 100% or been marked done.
    pub fn is_complete(&self) -> bool {
        self.is_done || self.percentage >= 100.0
    }
}

// ── Extended ProgressBridge methods ──

impl ProgressBridge {
    /// Start a progress with duplicate-handle checking.
    pub fn try_start(
        &mut self,
        handle: u64,
        options: &ProgressOptions,
    ) -> Result<(), ProgressError> {
        if self.active.iter().any(|s| s.handle == handle) {
            return Err(ProgressError::DuplicateHandle(handle));
        }
        self.start(handle, options);
        Ok(())
    }

    /// Report progress with validation on the increment value.
    pub fn try_report(
        &mut self,
        handle: u64,
        increment: Option<f64>,
        message: Option<String>,
    ) -> Result<(), ProgressError> {
        if let Some(inc) = increment {
            if inc.is_nan() {
                return Err(ProgressError::InvalidIncrement("NaN".into()));
            }
            if inc < 0.0 {
                return Err(ProgressError::InvalidIncrement(format!(
                    "negative value {inc}"
                )));
            }
        }
        if !self.active.iter().any(|s| s.handle == handle) {
            return Err(ProgressError::HandleNotFound(handle));
        }
        self.report(handle, increment, message);
        Ok(())
    }

    /// End a progress, returning an error if the handle is unknown.
    pub fn try_end(&mut self, handle: u64) -> Result<(), ProgressError> {
        if !self.active.iter().any(|s| s.handle == handle) {
            return Err(ProgressError::HandleNotFound(handle));
        }
        self.end(handle);
        Ok(())
    }

    /// Remove all completed progress entries and return the count removed.
    pub fn gc_completed(&mut self) -> usize {
        let before = self.active.len();
        self.active.retain(|s| !s.is_done);
        before - self.active.len()
    }

    /// Returns an iterator over all active (non-done) progress states.
    pub fn active_states(&self) -> impl Iterator<Item = &ProgressState> {
        self.active.iter().filter(|s| !s.is_done)
    }

    /// Returns the total number of tracked progress entries (including done).
    pub fn total_count(&self) -> usize {
        self.active.len()
    }

    /// Compute the average percentage across all active (non-done) entries.
    pub fn average_progress(&self) -> Option<f64> {
        let active: Vec<_> = self.active.iter().filter(|s| !s.is_done).collect();
        if active.is_empty() {
            return None;
        }
        let sum: f64 = active.iter().map(|s| s.percentage).sum();
        Some(sum / active.len() as f64)
    }
}

/// Initialize the progress extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

/// Accumulated statistics for ext-progress operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtProgressStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtProgressStats {
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
    pub fn merge(&mut self, other: &ExtProgressStats) {
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

impl Default for ExtProgressStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtProgressStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtProgressStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-progress.
#[derive(Debug, Clone)]
pub struct ExtProgressValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtProgressValidator {
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

impl Default for ExtProgressValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ── Nested Progress Stack ──

/// An entry in a nested progress stack, representing one level of sub-task.
#[derive(Debug, Clone)]
pub struct ProgressStackEntry {
    pub label: String,
    pub percentage: f64,
    pub weight: f64,
}

/// Tracks nested progress for hierarchical sub-tasks.
///
/// Each level has a label, a weight (relative importance), and a current
/// percentage.  [`overall_percentage`](ProgressStack::overall_percentage)
/// computes the weighted composite progress across all levels.
#[derive(Debug)]
pub struct ProgressStack {
    stack: Vec<ProgressStackEntry>,
}

impl ProgressStack {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Push a new sub-task onto the stack with the given label and weight.
    pub fn push(&mut self, label: &str, weight: f64) {
        self.stack.push(ProgressStackEntry {
            label: label.to_string(),
            percentage: 0.0,
            weight,
        });
    }

    /// Pop (complete) the innermost sub-task, returning its entry.
    pub fn pop(&mut self) -> Option<ProgressStackEntry> {
        self.stack.pop()
    }

    /// Update the percentage of the innermost sub-task.
    pub fn update_current(&mut self, percentage: f64) {
        if let Some(entry) = self.stack.last_mut() {
            entry.percentage = percentage.clamp(0.0, 100.0);
        }
    }

    /// Compute the weighted overall percentage across all stack levels.
    ///
    /// Each level's contribution is scaled by its weight relative to the total
    /// weight of all entries.
    pub fn overall_percentage(&self) -> f64 {
        if self.stack.is_empty() {
            return 0.0;
        }
        let total_weight: f64 = self.stack.iter().map(|e| e.weight).sum();
        if total_weight <= 0.0 {
            return 0.0;
        }
        let weighted_sum: f64 = self
            .stack
            .iter()
            .map(|e| e.percentage * e.weight / total_weight)
            .sum();
        weighted_sum.clamp(0.0, 100.0)
    }

    /// The current nesting depth.
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// The label of the innermost sub-task, if any.
    pub fn current_label(&self) -> Option<&str> {
        self.stack.last().map(|e| e.label.as_str())
    }

    /// Whether the stack has no entries.
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// A breadcrumb trail of all labels, e.g. `"task1 > subtask > leaf"`.
    pub fn breadcrumb(&self) -> String {
        self.stack
            .iter()
            .map(|e| e.label.as_str())
            .collect::<Vec<_>>()
            .join(" > ")
    }
}

impl Default for ProgressStack {
    fn default() -> Self {
        Self::new()
    }
}

// ── Cancel Token ──

/// A token that can be used to signal cancellation of a progress operation.
#[derive(Debug)]
pub struct ProgressCancelToken {
    handle: u64,
    cancelled: bool,
    reason: Option<String>,
}

impl ProgressCancelToken {
    pub fn new(handle: u64) -> Self {
        Self {
            handle,
            cancelled: false,
            reason: None,
        }
    }

    /// Cancel without a specific reason.
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    /// Cancel with a human-readable reason.
    pub fn cancel_with_reason(&mut self, reason: &str) {
        self.cancelled = true;
        self.reason = Some(reason.to_string());
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }

    pub fn handle(&self) -> u64 {
        self.handle
    }
}

/// Convenience constructor for [`ProgressCancelToken`].
pub fn progress_cancel_token(handle: u64) -> ProgressCancelToken {
    ProgressCancelToken::new(handle)
}

// ── Format Helpers ──

/// Render a human-readable progress bar for a [`ProgressState`].
///
/// * With a percentage: `[##########----------] 50% message`
/// * Without meaningful percentage: `[...working...] message`
/// * When complete: `[####################] 100% message (done)`
pub fn progress_format_message(state: &ProgressState) -> String {
    const BAR_WIDTH: usize = 20;

    let msg = state.message.as_deref().unwrap_or("");

    if state.is_done {
        let bar = "#".repeat(BAR_WIDTH);
        return if msg.is_empty() {
            format!("[{}] 100% (done)", bar)
        } else {
            format!("[{}] 100% {} (done)", bar, msg)
        };
    }

    if state.percentage <= 0.0 {
        return if msg.is_empty() {
            "[...working...]".to_string()
        } else {
            format!("[...working...] {}", msg)
        };
    }

    let filled = ((state.percentage / 100.0) * BAR_WIDTH as f64)
        .round()
        .min(BAR_WIDTH as f64) as usize;
    let empty = BAR_WIDTH - filled;
    let bar = format!("{}{}", "#".repeat(filled), "-".repeat(empty));
    let pct = state.percentage.round() as u32;

    if msg.is_empty() {
        format!("[{}] {}%", bar, pct)
    } else {
        format!("[{}] {}% {}", bar, pct, msg)
    }
}

// ── Progress Summary ──

/// An aggregate summary of all progress states held by a [`ProgressBridge`].
#[derive(Debug)]
pub struct ProgressSummary {
    pub active: usize,
    pub completed: usize,
    pub overall_progress: f64,
}

impl ProgressSummary {
    /// Build a summary by inspecting all states in the bridge.
    pub fn from_bridge(bridge: &ProgressBridge) -> Self {
        let mut active: usize = 0;
        let mut completed: usize = 0;
        let mut pct_sum: f64 = 0.0;

        for state in &bridge.active {
            if state.is_done {
                completed += 1;
            } else {
                active += 1;
                pct_sum += state.percentage;
            }
        }

        let overall_progress = if active > 0 {
            pct_sum / active as f64
        } else {
            0.0
        };

        Self {
            active,
            completed,
            overall_progress,
        }
    }

    pub fn total_active(&self) -> usize {
        self.active
    }

    pub fn total_completed(&self) -> usize {
        self.completed
    }

    pub fn overall_progress(&self) -> f64 {
        self.overall_progress
    }

    /// A one-line human-readable summary string.
    pub fn display(&self) -> String {
        format!(
            "{} active, {} completed, overall {:.1}%",
            self.active, self.completed, self.overall_progress
        )
    }
}

// ── Progress Timeline (ETA estimation) ──

/// A single recorded data point in the progress timeline.
#[derive(Debug, Clone)]
pub struct TimelineEntry {
    pub percentage: f64,
    pub timestamp: Instant,
}

/// Tracks progress changes over time to provide ETA and rate estimation.
#[derive(Debug)]
pub struct ProgressTimeline {
    entries: Vec<TimelineEntry>,
    start: Instant,
}

impl ProgressTimeline {
    /// Create a new timeline starting at the current instant.
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            entries: vec![TimelineEntry {
                percentage: 0.0,
                timestamp: now,
            }],
            start: now,
        }
    }

    /// Create a timeline with a specific start instant (useful for testing).
    pub fn with_start(start: Instant) -> Self {
        Self {
            entries: vec![TimelineEntry {
                percentage: 0.0,
                timestamp: start,
            }],
            start,
        }
    }

    /// Record a progress percentage at the current instant.
    pub fn record(&mut self, percentage: f64) {
        self.entries.push(TimelineEntry {
            percentage: percentage.clamp(0.0, 100.0),
            timestamp: Instant::now(),
        });
    }

    /// Record a progress percentage at a specific instant.
    pub fn record_at(&mut self, percentage: f64, at: Instant) {
        self.entries.push(TimelineEntry {
            percentage: percentage.clamp(0.0, 100.0),
            timestamp: at,
        });
    }

    /// Total elapsed time since the timeline was created.
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// The most recently recorded percentage.
    pub fn current_percentage(&self) -> f64 {
        self.entries.last().map(|e| e.percentage).unwrap_or(0.0)
    }

    /// Compute the rate of progress in percentage-points per second,
    /// based on the first and last entries.
    pub fn rate_pct_per_sec(&self) -> Option<f64> {
        if self.entries.len() < 2 {
            return None;
        }
        let first = &self.entries[0];
        let last = self.entries.last().unwrap();
        let dt = last.timestamp.duration_since(first.timestamp).as_secs_f64();
        if dt <= 0.0 {
            return None;
        }
        Some((last.percentage - first.percentage) / dt)
    }

    /// Estimate the remaining duration until 100% based on the current rate.
    pub fn eta(&self) -> Option<Duration> {
        let rate = self.rate_pct_per_sec()?;
        if rate <= 0.0 {
            return None;
        }
        let remaining_pct = 100.0 - self.current_percentage();
        if remaining_pct <= 0.0 {
            return Some(Duration::ZERO);
        }
        Some(Duration::from_secs_f64(remaining_pct / rate))
    }

    /// Number of recorded data points.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the timeline has no entries beyond the initial point.
    pub fn is_empty(&self) -> bool {
        self.entries.len() <= 1
    }
}

impl Default for ProgressTimeline {
    fn default() -> Self {
        Self::new()
    }
}

// ── Progress Throttle ──

/// Limits the frequency of progress updates by dropping updates that arrive
/// too quickly.
#[derive(Debug)]
pub struct ProgressThrottle {
    interval: Duration,
    last_emit: Option<Instant>,
    last_value: Option<f64>,
}

impl ProgressThrottle {
    /// Create a throttle that allows at most one update per `interval`.
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_emit: None,
            last_value: None,
        }
    }

    /// Create a throttle from milliseconds.
    pub fn from_millis(ms: u64) -> Self {
        Self::new(Duration::from_millis(ms))
    }

    /// Attempt to emit a progress value. Returns `Some(value)` if enough time
    /// has elapsed since the last emit, or `None` if throttled.
    pub fn try_emit(&mut self, value: f64) -> Option<f64> {
        self.try_emit_at(value, Instant::now())
    }

    /// Attempt to emit at a specific instant (useful for testing).
    pub fn try_emit_at(&mut self, value: f64, now: Instant) -> Option<f64> {
        self.last_value = Some(value);
        match self.last_emit {
            None => {
                self.last_emit = Some(now);
                Some(value)
            }
            Some(prev) if now.duration_since(prev) >= self.interval => {
                self.last_emit = Some(now);
                Some(value)
            }
            _ => None,
        }
    }

    /// Force-emit the last recorded value regardless of the throttle interval.
    /// Useful for flushing a final update when an operation completes.
    pub fn flush(&mut self) -> Option<f64> {
        self.last_emit = Some(Instant::now());
        self.last_value
    }

    /// The configured throttle interval.
    pub fn interval(&self) -> Duration {
        self.interval
    }
}

// ── Multi-Progress Tracker ──

/// Entry within a [`MultiProgressTracker`].
#[derive(Debug)]
struct MultiEntry {
    label: String,
    percentage: f64,
    weight: f64,
    done: bool,
}

/// Manages multiple concurrent progress operations and provides a combined
/// overall view.
#[derive(Debug)]
pub struct MultiProgressTracker {
    entries: Vec<(u64, MultiEntry)>,
    next_id: u64,
}

impl MultiProgressTracker {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a new tracked operation, returning its unique id.
    pub fn add(&mut self, label: impl Into<String>, weight: f64) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push((
            id,
            MultiEntry {
                label: label.into(),
                percentage: 0.0,
                weight: weight.max(0.0),
                done: false,
            },
        ));
        id
    }

    /// Update the percentage for a tracked operation.
    pub fn update(&mut self, id: u64, percentage: f64) {
        if let Some((_, entry)) = self.entries.iter_mut().find(|(eid, _)| *eid == id) {
            entry.percentage = percentage.clamp(0.0, 100.0);
        }
    }

    /// Mark an operation as done (100%).
    pub fn finish(&mut self, id: u64) {
        if let Some((_, entry)) = self.entries.iter_mut().find(|(eid, _)| *eid == id) {
            entry.percentage = 100.0;
            entry.done = true;
        }
    }

    /// The weighted overall percentage across all tracked operations.
    pub fn overall_percentage(&self) -> f64 {
        let total_weight: f64 = self.entries.iter().map(|(_, e)| e.weight).sum();
        if total_weight <= 0.0 {
            return 0.0;
        }
        let weighted: f64 = self
            .entries
            .iter()
            .map(|(_, e)| e.percentage * e.weight / total_weight)
            .sum();
        weighted.clamp(0.0, 100.0)
    }

    /// Number of operations that are still active (not done).
    pub fn active_count(&self) -> usize {
        self.entries.iter().filter(|(_, e)| !e.done).count()
    }

    /// Number of completed operations.
    pub fn done_count(&self) -> usize {
        self.entries.iter().filter(|(_, e)| e.done).count()
    }

    /// Total number of tracked operations.
    pub fn total_count(&self) -> usize {
        self.entries.len()
    }

    /// Whether all tracked operations are done.
    pub fn all_done(&self) -> bool {
        !self.entries.is_empty() && self.entries.iter().all(|(_, e)| e.done)
    }

    /// Get the label for a tracked operation.
    pub fn label(&self, id: u64) -> Option<&str> {
        self.entries
            .iter()
            .find(|(eid, _)| *eid == id)
            .map(|(_, e)| e.label.as_str())
    }
}

impl Default for MultiProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MultiProgressTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MultiProgress({}/{} done, overall {:.1}%)",
            self.done_count(),
            self.total_count(),
            self.overall_percentage()
        )
    }
}

// ── Progress Formatter ──

/// Formats progress values for terminal display.
pub struct ProgressFormatter;

impl ProgressFormatter {
    /// Render a progress bar string of the given `width` (in characters).
    ///
    /// Example: `[##########----------]` for 50% with width 20.
    pub fn bar(percentage: f64, width: usize) -> String {
        let pct = percentage.clamp(0.0, 100.0);
        let filled = ((pct / 100.0) * width as f64).round() as usize;
        let empty = width.saturating_sub(filled);
        format!("[{}{}]", "#".repeat(filled), "-".repeat(empty))
    }

    /// Format a [`Duration`] as a human-readable ETA string.
    ///
    /// * Under 60 s → `"12s"`
    /// * Under 60 min → `"3m 12s"`
    /// * Otherwise → `"1h 03m"`
    pub fn eta_string(d: Duration) -> String {
        let secs = d.as_secs();
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m {:02}s", secs / 60, secs % 60)
        } else {
            format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60)
        }
    }

    /// Format a rate as items/sec or percentage-points/sec.
    pub fn rate_string(rate: f64, unit: &str) -> String {
        if rate < 0.01 {
            format!("<0.01 {unit}/s")
        } else if rate >= 1000.0 {
            format!("{:.0} {unit}/s", rate)
        } else {
            format!("{:.2} {unit}/s", rate)
        }
    }

    /// Produce a single-line summary combining bar, percentage, ETA, and message.
    pub fn summary_line(
        percentage: f64,
        eta: Option<Duration>,
        message: Option<&str>,
    ) -> String {
        let bar = Self::bar(percentage, 20);
        let pct = percentage.clamp(0.0, 100.0).round() as u32;
        let mut parts = vec![format!("{bar} {pct}%")];
        if let Some(d) = eta {
            parts.push(format!("ETA {}", Self::eta_string(d)));
        }
        if let Some(msg) = message {
            parts.push(msg.to_string());
        }
        parts.join(" ")
    }
}

// ── From impls ──

impl From<ProgressLocation> for String {
    fn from(loc: ProgressLocation) -> Self {
        loc.to_string()
    }
}

impl From<&ProgressState> for ProgressSummary {
    fn from(state: &ProgressState) -> Self {
        if state.is_done {
            ProgressSummary {
                active: 0,
                completed: 1,
                overall_progress: 100.0,
            }
        } else {
            ProgressSummary {
                active: 1,
                completed: 0,
                overall_progress: state.percentage,
            }
        }
    }
}

// ── Progress Chain ──

/// A single step within a [`ProgressChain`].
#[derive(Debug, Clone)]
pub struct ProgressChainStep {
    pub label: String,
    pub weight: f64,
    pub progress: f64,
    pub is_complete: bool,
}

/// Links sequential progress operations where each step has a weight.
///
/// Overall progress is computed as the weighted sum of individual step
/// percentages, making it easy to represent multi-phase workflows like
/// "download 30%, extract 20%, install 50%".
#[derive(Debug)]
pub struct ProgressChain {
    steps: Vec<ProgressChainStep>,
}

impl ProgressChain {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// Add a step with the given label and relative weight.
    pub fn add_step(&mut self, label: &str, weight: f64) -> usize {
        let idx = self.steps.len();
        self.steps.push(ProgressChainStep {
            label: label.to_string(),
            weight: weight.max(0.0),
            progress: 0.0,
            is_complete: false,
        });
        idx
    }

    /// Report progress for a specific step (0–100).
    pub fn report(&mut self, index: usize, progress: f64) {
        if let Some(step) = self.steps.get_mut(index) {
            step.progress = progress.clamp(0.0, 100.0);
            step.is_complete = step.progress >= 100.0;
        }
    }

    /// Mark a step as complete.
    pub fn complete_step(&mut self, index: usize) {
        self.report(index, 100.0);
    }

    /// The weighted overall progress across all steps (0–100).
    pub fn overall_progress(&self) -> f64 {
        let total_weight: f64 = self.steps.iter().map(|s| s.weight).sum();
        if total_weight <= 0.0 {
            return 0.0;
        }
        let weighted_sum: f64 = self
            .steps
            .iter()
            .map(|s| s.progress * s.weight)
            .sum();
        (weighted_sum / total_weight).clamp(0.0, 100.0)
    }

    /// True when every step is complete.
    pub fn is_finished(&self) -> bool {
        !self.steps.is_empty() && self.steps.iter().all(|s| s.is_complete)
    }

    /// The index of the first incomplete step, if any.
    pub fn current_step(&self) -> Option<usize> {
        self.steps.iter().position(|s| !s.is_complete)
    }

    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    pub fn get_step(&self, index: usize) -> Option<&ProgressChainStep> {
        self.steps.get(index)
    }
}

// ── Progress Estimator ──

/// Adaptive ETA estimation using an exponential moving average of recent
/// progress rates. More recent samples are weighted more heavily so the
/// estimate adapts quickly when throughput changes.
#[derive(Debug)]
pub struct ProgressEstimator {
    smoothed_rate: Option<f64>,
    alpha: f64,
    last_percentage: f64,
    last_time: Instant,
}

impl ProgressEstimator {
    /// Create a new estimator. `alpha` controls smoothing: values closer to
    /// 1.0 react faster to changes, values closer to 0.0 smooth more.
    pub fn new(alpha: f64) -> Self {
        Self {
            smoothed_rate: None,
            alpha: alpha.clamp(0.01, 1.0),
            last_percentage: 0.0,
            last_time: Instant::now(),
        }
    }

    /// Record a new percentage observation and update the rate estimate.
    pub fn record(&mut self, percentage: f64) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_time).as_secs_f64();
        if elapsed > 0.0 {
            let delta = percentage - self.last_percentage;
            if delta > 0.0 {
                let rate = delta / elapsed;
                self.smoothed_rate = Some(match self.smoothed_rate {
                    Some(prev) => self.alpha * rate + (1.0 - self.alpha) * prev,
                    None => rate,
                });
            }
        }
        self.last_percentage = percentage;
        self.last_time = now;
    }

    /// Record a percentage with an explicit timestamp (useful for testing).
    pub fn record_at(&mut self, percentage: f64, at: Instant) {
        let elapsed = at.duration_since(self.last_time).as_secs_f64();
        if elapsed > 0.0 {
            let delta = percentage - self.last_percentage;
            if delta > 0.0 {
                let rate = delta / elapsed;
                self.smoothed_rate = Some(match self.smoothed_rate {
                    Some(prev) => self.alpha * rate + (1.0 - self.alpha) * prev,
                    None => rate,
                });
            }
        }
        self.last_percentage = percentage;
        self.last_time = at;
    }

    /// Estimated time remaining to reach 100%, or `None` if no rate data.
    pub fn eta(&self) -> Option<Duration> {
        self.smoothed_rate.and_then(|r| {
            if r <= 0.0 {
                return None;
            }
            let remaining = 100.0 - self.last_percentage;
            if remaining <= 0.0 {
                return Some(Duration::ZERO);
            }
            Some(Duration::from_secs_f64(remaining / r))
        })
    }

    /// The current smoothed rate in percentage-points per second.
    pub fn rate(&self) -> Option<f64> {
        self.smoothed_rate
    }
}

// ── Progress Notification Link ──

/// Maps progress handles to notification IDs so that UI layers can pair a
/// running progress operation with the notification widget displaying it.
#[derive(Debug)]
pub struct ProgressNotificationLink {
    links: Vec<(u64, String)>,
}

impl ProgressNotificationLink {
    pub fn new() -> Self {
        Self { links: Vec::new() }
    }

    /// Associate a progress handle with a notification ID.
    pub fn link(&mut self, handle: u64, notification_id: &str) {
        if !self.links.iter().any(|(h, _)| *h == handle) {
            self.links.push((handle, notification_id.to_string()));
        }
    }

    /// Remove the link for a handle (e.g. when progress completes).
    pub fn unlink(&mut self, handle: u64) {
        self.links.retain(|(h, _)| *h != handle);
    }

    /// Look up the notification ID for a handle.
    pub fn notification_for(&self, handle: u64) -> Option<&str> {
        self.links
            .iter()
            .find(|(h, _)| *h == handle)
            .map(|(_, id)| id.as_str())
    }

    /// Look up the handle for a notification ID.
    pub fn handle_for(&self, notification_id: &str) -> Option<u64> {
        self.links
            .iter()
            .find(|(_, id)| id == notification_id)
            .map(|(h, _)| *h)
    }

    pub fn count(&self) -> usize {
        self.links.len()
    }
}

// ── Progress Cancellation Cascade ──

/// Tracks parent→child relationships between progress handles so that
/// cancelling a parent automatically cancels all descendants.
#[derive(Debug)]
pub struct ProgressCancellationCascade {
    edges: Vec<(u64, u64)>,
    cancelled: Vec<u64>,
}

impl ProgressCancellationCascade {
    pub fn new() -> Self {
        Self {
            edges: Vec::new(),
            cancelled: Vec::new(),
        }
    }

    /// Register `child` as a dependent of `parent`.
    pub fn add_child(&mut self, parent: u64, child: u64) {
        if !self.edges.iter().any(|&(p, c)| p == parent && c == child) {
            self.edges.push((parent, child));
        }
    }

    /// Cancel a handle and all of its transitive children.
    pub fn cancel(&mut self, handle: u64) {
        if self.cancelled.contains(&handle) {
            return;
        }
        self.cancelled.push(handle);
        let children: Vec<u64> = self
            .edges
            .iter()
            .filter(|(p, _)| *p == handle)
            .map(|(_, c)| *c)
            .collect();
        for child in children {
            self.cancel(child);
        }
    }

    pub fn is_cancelled(&self, handle: u64) -> bool {
        self.cancelled.contains(&handle)
    }

    /// All handles that are currently cancelled.
    pub fn cancelled_handles(&self) -> &[u64] {
        &self.cancelled
    }

    /// Direct children of a handle.
    pub fn children_of(&self, handle: u64) -> Vec<u64> {
        self.edges
            .iter()
            .filter(|(p, _)| *p == handle)
            .map(|(_, c)| *c)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// ProgressNotificationBridge - progress notification bridge
// ---------------------------------------------------------------------------

/// Severity level for progress notification bridge issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProgressNotificationBridgeSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for ProgressNotificationBridgeSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [ProgressNotificationBridge].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressNotificationBridgeEntry {
    pub id: String,
    pub label: String,
    pub severity: ProgressNotificationBridgeSeverity,
    pub detail: Option<String>,
    pub progress_pct: usize,
    enabled: bool,
}

impl ProgressNotificationBridgeEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: ProgressNotificationBridgeSeverity::Low,
            detail: None,
            progress_pct: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: ProgressNotificationBridgeSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_progress_pct(mut self, val: usize) -> Self {
        self.progress_pct = val;
        self
    }

    pub fn is_complete(&self) -> bool {
        self.enabled && self.severity >= ProgressNotificationBridgeSeverity::Medium
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
        format!("[{}] {} ({}): {}", self.severity, self.id, self.progress_pct, det)
    }
}

impl fmt::Display for ProgressNotificationBridgeEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [ProgressNotificationBridgeEntry] items.
#[derive(Debug, Clone)]
pub struct ProgressNotificationBridge {
    entries: Vec<ProgressNotificationBridgeEntry>,
    name: String,
    capacity: usize,
}

impl ProgressNotificationBridge {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: ProgressNotificationBridgeEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<ProgressNotificationBridgeEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&ProgressNotificationBridgeEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn progress_pct(&self) -> usize { self.entries.len() }

    pub fn is_complete(&self) -> bool {
        self.entries.iter().any(|e| e.is_complete())
    }

    pub fn entries_by_severity(&self, severity: ProgressNotificationBridgeSeverity) -> Vec<&ProgressNotificationBridgeEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= ProgressNotificationBridgeSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&ProgressNotificationBridgeEntry> {
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

    pub fn enabled_entries(&self) -> Vec<&ProgressNotificationBridgeEntry> {
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
// ProgressCancellationHandler - progress cancellation handler
// ---------------------------------------------------------------------------

/// Configuration for [ProgressCancellationHandler].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressCancellationHandlerConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub notification_count: usize,
}

impl ProgressCancellationHandlerConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, notification_count: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_notification_count(mut self, val: usize) -> Self { self.notification_count = val; self }
}

impl Default for ProgressCancellationHandlerConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [ProgressCancellationHandler].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressCancellationHandlerItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl ProgressCancellationHandlerItem {
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

    pub fn is_cancelled(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for ProgressCancellationHandlerItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [ProgressCancellationHandlerItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct ProgressCancellationHandler {
    config: ProgressCancellationHandlerConfig,
    items: Vec<ProgressCancellationHandlerItem>,
}

impl ProgressCancellationHandler {
    pub fn new(config: ProgressCancellationHandlerConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: ProgressCancellationHandlerItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<ProgressCancellationHandlerItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&ProgressCancellationHandlerItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn notification_count(&self) -> usize { self.items.len() }

    pub fn is_cancelled(&self) -> bool {
        self.items.iter().any(|i| i.is_cancelled())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&ProgressCancellationHandlerItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ProgressCancellationHandlerItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &ProgressCancellationHandlerConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



/// Extension progress configuration manager.
#[derive(Debug, Clone)]
pub struct ExtProgressConfig {
    entries: Vec<ExtProgressEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single extension progress entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtProgressEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl ExtProgressEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl ExtProgressConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: ExtProgressEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&ExtProgressEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut ExtProgressEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&ExtProgressEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&ExtProgressEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&ExtProgressEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<ExtProgressEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Extension progress reporting — extended utilities (qv)
// ---------------------------------------------------------------------------

/// Metric accumulator for ext_prog operations.
#[derive(Debug, Clone)]
pub struct QvMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QvMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for ext_prog.
#[derive(Debug, Clone)]
pub struct QvRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QvRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for ext_prog lookups.
#[derive(Debug, Clone)]
pub struct QvLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QvLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 11
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer11 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer11 {
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
pub fn xb_fnv1a_11(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_11<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_11<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_11(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_11(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 66
// ---------------------------------------------------------------------------

/// Generic object pool `Xc66Pool<T>`.
pub struct Xc66Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc66Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc66PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc66Pool<T> {
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
    pub fn stats(&self) -> Xc66PoolStats {
        Xc66PoolStats {
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

impl<T> Default for Xc66Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc66Scheduler`.
pub struct Xc66Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc66Scheduler {
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

impl Default for Xc66Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_66 hash for the given byte slice.
pub fn xc_66_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_66 convention.
pub fn xc_66_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe23 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe23Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe23PipelineError {
    pub stage: Xe23Stage,
    pub message: String,
}

impl std::fmt::Display for Xe23PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe23Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe23Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe23PipelineError>>>,
    stage_names: Vec<Xe23Stage>,
}

impl Xe23Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe23PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe23Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe23PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe23Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe23PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe23Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe23PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe23Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe23PipelineError> {
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

    pub fn compose(mut self, other: Xe23Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe23CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe23CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe23Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe23CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe23CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe23Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe23CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_23_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe23CacheEntry {
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

    fn xe_23_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe23CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_23_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe23PipelineError> {
    Ok(data)
}

pub fn xe_23_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe23PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_23_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe23PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_23_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe23PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_23_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe23PipelineError> {
    Err(Xe23PipelineError {
        stage: Xe23Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #103
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf103Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf103TrieNode {
    children: std::collections::HashMap<char, Xf103TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf103Trie {
    root: Xf103TrieNode,
    count: usize,
}

impl Xf103Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf103TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf103TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf103TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf103BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf103BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 65).
pub struct Xh65SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh65SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 107 as u64,
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

/// A compact bit set supporting boolean operations (variant 65).
pub struct Xh65BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh65BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 65).
pub struct Xi65Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi65Deque<T> {
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
pub struct Xi65Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi65Interval {
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

/// A simple interval tree (variant 65).
pub struct Xi65IntervalTree {
    xi_intervals: Vec<Xi65Interval>,
}

impl Xi65IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi65Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi65Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi65Interval) -> Vec<&Xi65Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi65Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi65Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi65Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi65Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi65Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi65Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 66) ---

/// Disjoint set / union-find for crate 66.
pub struct Xj66UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj66UnionFind {
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

const XJ66_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 66.
pub struct Xj66BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj66BTreeNode<K, V>>>,
    len: usize,
}

struct Xj66BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj66BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj66BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ66_BTREE_ORDER - 1
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
        let mid = XJ66_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj66BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj66BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj66BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj66BTreeNode::xj_new_leaf();
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


// --- xk_65 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk65SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk65SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk65DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk65DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_66).
#[derive(Debug, Clone)]
pub struct Xl66Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl66Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_66).
#[derive(Debug, Clone)]
pub struct Xl66SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl66SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm66MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm66MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm66Tokenizer {
    text: String,
}

impl Xm66Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 65.
pub struct Xn65Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn65Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 65 -----

#[derive(Debug, Clone)]
struct Xn65AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn65AvlNode<K, V>>>,
    right: Option<Box<Xn65AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 65.
#[derive(Debug, Clone)]
pub struct Xn65AVL<K, V> {
    root: Option<Box<Xn65AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn65AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn65AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn65AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn65AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn65AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn65AvlNode<K, V>>) -> Box<Xn65AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn65AvlNode<K, V>>) -> Box<Xn65AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn65AvlNode<K, V>>) -> Box<Xn65AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn65AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn65AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn65AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn65AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn65AvlNode<K, V>>) -> &Xn65AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn65AvlNode<K, V>>) -> (Box<Xn65AvlNode<K, V>>, Option<Box<Xn65AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn65AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn65AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn65AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn65AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn65AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn65AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn65AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo65RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo65Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo65RBNode<K, V> {
    key: K,
    value: V,
    color: Xo65Color,
    left: Option<Box<Xo65RBNode<K, V>>>,
    right: Option<Box<Xo65RBNode<K, V>>>,
}

/// A red-black tree map for crate 65.
#[derive(Debug, Clone)]
pub struct Xo65RedBlack<K, V> {
    root: Option<Box<Xo65RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo65RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo65Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo65RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo65RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo65RBNode {
                    key, value, color: Xo65Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo65RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo65Color::Red)
    }

    fn xo_balance(mut h: Box<Xo65RBNode<K, V>>) -> Box<Xo65RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo65Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo65RBNode<K, V>>) -> Box<Xo65RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo65Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo65RBNode<K, V>>) -> Box<Xo65RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo65Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo65RBNode<K, V>>) {
        h.color = Xo65Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo65Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo65Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo65Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo65RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo65RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo65RBNode<K, V>) -> (K, V, Option<Box<Xo65RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo65RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo65Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo65RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo65ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 65.
#[derive(Debug, Clone)]
pub struct Xo65ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo65ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo65#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo65#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 65).
#[derive(Debug)]
pub struct Xp65SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp65Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp65Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp65Node<K, V>>>,
    xp_right: Option<Box<Xp65Node<K, V>>>,
}

impl<K: Ord, V> Xp65Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp65SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp65SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp65Node<K, V>>>, key: &K) -> Option<Box<Xp65Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp65Node<K, V>>) -> Box<Xp65Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp65Node<K, V>>) -> Box<Xp65Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp65Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp65Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp65Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq65Treap ---------------

use std::cmp::Ordering as Xq65Ord;

struct Xq65TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq65TreapNode<K, V>>>,
    right: Option<Box<Xq65TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq65Treap<K, V> {
    root: Option<Box<Xq65TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq65TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_65_size<K, V>(node: &Option<Box<Xq65TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_65_update_size<K, V>(node: &mut Xq65TreapNode<K, V>) {
    node.size = 1 + xq_65_size(&node.left) + xq_65_size(&node.right);
}

fn xq_65_rotate_right<K, V>(mut node: Box<Xq65TreapNode<K, V>>) -> Box<Xq65TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_65_update_size(&mut node);
    left.right = Some(node);
    xq_65_update_size(&mut left);
    left
}

fn xq_65_rotate_left<K, V>(mut node: Box<Xq65TreapNode<K, V>>) -> Box<Xq65TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_65_update_size(&mut node);
    right.left = Some(node);
    xq_65_update_size(&mut right);
    right
}

fn xq_65_insert_node<K: Ord, V>(
    node: Option<Box<Xq65TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq65TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq65TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq65Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq65Ord::Less => {
                let (new_left, old) = xq_65_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_65_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_65_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq65Ord::Greater => {
                let (new_right, old) = xq_65_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_65_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_65_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_65_remove_node<K: Ord, V>(
    node: Option<Box<Xq65TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq65TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq65Ord::Less => {
                let (new_left, old) = xq_65_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_65_update_size(&mut n);
                (Some(n), old)
            }
            Xq65Ord::Greater => {
                let (new_right, old) = xq_65_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_65_update_size(&mut n);
                (Some(n), old)
            }
            Xq65Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_65_rotate_right(n);
                    let (new_right, old) = xq_65_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_65_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_65_rotate_left(n);
                    let (new_left, old) = xq_65_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_65_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_65_find_min<K, V>(node: &Option<Box<Xq65TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_65_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_65_find_max<K, V>(node: &Option<Box<Xq65TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_65_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_65_rank<K: Ord, V>(node: &Option<Box<Xq65TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq65Ord::Less => xq_65_rank(&n.left, key),
            Xq65Ord::Equal => xq_65_size(&n.left),
            Xq65Ord::Greater => 1 + xq_65_size(&n.left) + xq_65_rank(&n.right, key),
        },
    }
}

fn xq_65_kth<K, V>(node: &Option<Box<Xq65TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_65_size(&n.left);
        if k < left_size {
            xq_65_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_65_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_65_in_order<K: Clone, V>(node: &Option<Box<Xq65TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_65_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_65_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq65Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 65 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_65_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq65Ord::Equal => return Some(&n.value),
                Xq65Ord::Less => cur = &n.left,
                Xq65Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_65_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_65_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_65_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_65_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_65_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_65_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_65_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq65VEBTree ---------------

pub struct Xq65VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq65VEBTree>>,
    clusters: Vec<Option<Box<Xq65VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq65VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq65VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq65VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr65KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr65KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr65BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr65KDNode {
    xr_point: Xr65KDPoint,
    xr_left: Option<Box<Xr65KDNode>>,
    xr_right: Option<Box<Xr65KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr65KDTree {
    xr_root: Option<Box<Xr65KDNode>>,
    xr_size: usize,
}

impl Xr65KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr65KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr65KDNode>>,
        point: Xr65KDPoint,
        depth: usize,
    ) -> Box<Xr65KDNode> {
        match node {
            None => Box::new(Xr65KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr65KDPoint) -> Option<Xr65KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr65KDNode>,
        query: &Xr65KDPoint,
        depth: usize,
        best: &mut Xr65KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr65KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr65KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr65KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr65KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr65KDNode>>, pts: &mut Vec<Xr65KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr65KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr65BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr65BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = ProgressMessage::Start {
            handle: 1,
            options: ProgressOptions {
                location: ProgressLocation::Notification,
                title: Some("Loading".into()),
                cancellable: true,
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ProgressMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn progress_state_serialization() {
        let state = ProgressState {
            handle: 1,
            percentage: 50.0,
            message: Some("halfway".into()),
            is_done: false,
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: ProgressState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, back);
    }

    #[test]
    fn bridge_lifecycle() {
        let mut bridge = ProgressBridge::new();
        let opts = ProgressOptions {
            location: ProgressLocation::Window,
            title: Some("work".into()),
            cancellable: false,
        };
        bridge.start(1, &opts);
        assert_eq!(bridge.active_count(), 1);
        bridge.report(1, Some(50.0), None);
        assert_eq!(bridge.get_state(1).unwrap().percentage, 50.0);
        bridge.end(1);
        assert_eq!(bridge.active_count(), 0);
    }

    #[test]
    fn bridge_report_clamps() {
        let mut bridge = ProgressBridge::new();
        let opts = ProgressOptions {
            location: ProgressLocation::Notification,
            title: None,
            cancellable: false,
        };
        bridge.start(1, &opts);
        bridge.report(1, Some(80.0), None);
        bridge.report(1, Some(80.0), None);
        assert_eq!(bridge.get_state(1).unwrap().percentage, 100.0);
    }

    #[test]
    fn bridge_report_unknown_handle() {
        let mut bridge = ProgressBridge::new();
        bridge.report(999, Some(10.0), None);
        assert_eq!(bridge.active_count(), 0);
    }

    // ── Additional tests ──

    #[test]
    fn error_display_messages() {
        assert_eq!(
            ProgressError::HandleNotFound(42).to_string(),
            "progress handle 42 not found"
        );
        assert_eq!(
            ProgressError::DuplicateHandle(7).to_string(),
            "progress handle 7 already exists"
        );
        assert_eq!(
            ProgressError::InvalidIncrement("NaN".into()).to_string(),
            "invalid increment: NaN"
        );
        let err = ProgressError::TitleTooLong { max: 256, actual: 300 };
        assert_eq!(err.to_string(), "title length 300 exceeds maximum 256");
        assert_eq!(
            ProgressError::PercentageOutOfRange(120.0).to_string(),
            "percentage 120 is outside 0..=100"
        );
    }

    #[test]
    fn progress_location_display() {
        assert_eq!(ProgressLocation::SourceControl.to_string(), "Source Control");
        assert_eq!(ProgressLocation::Window.to_string(), "Window");
        assert_eq!(ProgressLocation::Notification.to_string(), "Notification");
    }

    #[test]
    fn progress_state_display() {
        let state = ProgressState {
            handle: 5,
            percentage: 33.3,
            message: Some("compiling".into()),
            is_done: false,
        };
        let display = state.to_string();
        assert!(display.contains("33.3%"));
        assert!(display.contains("compiling"));
        assert!(display.contains("active"));
    }

    #[test]
    fn progress_options_display() {
        let opts = ProgressOptions {
            location: ProgressLocation::Notification,
            title: Some("Installing".into()),
            cancellable: true,
        };
        let display = opts.to_string();
        assert!(display.contains("Installing"));
        assert!(display.contains("cancellable"));
    }

    #[test]
    fn builder_basic() {
        let opts = ProgressOptionsBuilder::new(ProgressLocation::Window)
            .title("Build")
            .cancellable(true)
            .build()
            .unwrap();
        assert_eq!(opts.location, ProgressLocation::Window);
        assert_eq!(opts.title.as_deref(), Some("Build"));
        assert!(opts.cancellable);
    }

    #[test]
    fn builder_title_too_long() {
        let long_title = "x".repeat(300);
        let result = ProgressOptionsBuilder::new(ProgressLocation::Notification)
            .title(long_title)
            .build();
        assert!(matches!(
            result,
            Err(ProgressError::TitleTooLong { max: 256, actual: 300 })
        ));
    }

    #[test]
    fn builder_no_title() {
        let opts = ProgressOptionsBuilder::new(ProgressLocation::SourceControl)
            .build()
            .unwrap();
        assert!(opts.title.is_none());
        assert!(!opts.cancellable);
    }

    #[test]
    fn try_start_duplicate() {
        let mut bridge = ProgressBridge::new();
        let opts = ProgressOptions {
            location: ProgressLocation::Window,
            title: None,
            cancellable: false,
        };
        bridge.try_start(1, &opts).unwrap();
        let err = bridge.try_start(1, &opts).unwrap_err();
        assert_eq!(err, ProgressError::DuplicateHandle(1));
    }

    #[test]
    fn try_report_validation() {
        let mut bridge = ProgressBridge::new();
        let opts = ProgressOptions {
            location: ProgressLocation::Window,
            title: None,
            cancellable: false,
        };
        bridge.try_start(1, &opts).unwrap();

        // NaN increment
        let err = bridge.try_report(1, Some(f64::NAN), None).unwrap_err();
        assert!(matches!(err, ProgressError::InvalidIncrement(_)));

        // Negative increment
        let err = bridge.try_report(1, Some(-5.0), None).unwrap_err();
        assert!(matches!(err, ProgressError::InvalidIncrement(_)));

        // Unknown handle
        let err = bridge.try_report(99, Some(10.0), None).unwrap_err();
        assert_eq!(err, ProgressError::HandleNotFound(99));

        // Valid report
        bridge.try_report(1, Some(25.0), Some("quarter".into())).unwrap();
        assert_eq!(bridge.get_state(1).unwrap().percentage, 25.0);
    }

    #[test]
    fn try_end_unknown() {
        let mut bridge = ProgressBridge::new();
        let err = bridge.try_end(42).unwrap_err();
        assert_eq!(err, ProgressError::HandleNotFound(42));
    }

    #[test]
    fn gc_completed_removes_done() {
        let mut bridge = ProgressBridge::new();
        let opts = ProgressOptions {
            location: ProgressLocation::Notification,
            title: None,
            cancellable: false,
        };
        bridge.start(1, &opts);
        bridge.start(2, &opts);
        bridge.start(3, &opts);
        bridge.end(1);
        bridge.end(3);

        assert_eq!(bridge.total_count(), 3);
        let removed = bridge.gc_completed();
        assert_eq!(removed, 2);
        assert_eq!(bridge.total_count(), 1);
        assert!(bridge.get_state(2).is_some());
    }

    #[test]
    fn average_progress() {
        let mut bridge = ProgressBridge::new();
        assert_eq!(bridge.average_progress(), None);

        let opts = ProgressOptions {
            location: ProgressLocation::Window,
            title: None,
            cancellable: false,
        };
        bridge.start(1, &opts);
        bridge.start(2, &opts);
        bridge.report(1, Some(40.0), None);
        bridge.report(2, Some(60.0), None);

        let avg = bridge.average_progress().unwrap();
        assert!((avg - 50.0).abs() < f64::EPSILON);

        // Done entries are excluded from the average
        bridge.end(2);
        let avg = bridge.average_progress().unwrap();
        assert!((avg - 40.0).abs() < f64::EPSILON);
    }

    #[test]
    fn progress_state_remaining_and_complete() {
        let mut state = ProgressState {
            handle: 1,
            percentage: 75.0,
            message: None,
            is_done: false,
        };
        assert!((state.remaining() - 25.0).abs() < f64::EPSILON);
        assert!(!state.is_complete());

        state.percentage = 100.0;
        assert!(state.is_complete());
        assert!((state.remaining()).abs() < f64::EPSILON);
    }

    #[test]
    fn handle_message_roundtrip() {
        let mut bridge = ProgressBridge::new();
        let start_msg = ProgressMessage::Start {
            handle: 10,
            options: ProgressOptions {
                location: ProgressLocation::Notification,
                title: Some("indexing".into()),
                cancellable: false,
            },
        };
        let result = bridge.handle_message(&start_msg);
        assert_eq!(result["started"], 10);

        let report_msg = ProgressMessage::Report {
            handle: 10,
            increment: Some(50.0),
            message: Some("halfway".into()),
        };
        let result = bridge.handle_message(&report_msg);
        assert_eq!(result["reported"], 10);
        assert_eq!(bridge.get_state(10).unwrap().percentage, 50.0);

        let end_msg = ProgressMessage::End { handle: 10 };
        let result = bridge.handle_message(&end_msg);
        assert_eq!(result["ended"], 10);
        assert!(bridge.get_state(10).unwrap().is_done);
    }

    #[test]
    fn error_is_std_error() {
        let err: Box<dyn std::error::Error> =
            Box::new(ProgressError::HandleNotFound(1));
        assert!(err.to_string().contains("handle 1"));
    }

    #[test]
    fn ext_progress_stats_new_defaults() {
        let stats = ExtProgressStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_progress_stats_record_success() {
        let mut stats = ExtProgressStats::new();
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
    fn ext_progress_stats_record_failure() {
        let mut stats = ExtProgressStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_progress_stats_reset() {
        let mut stats = ExtProgressStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_progress_stats_merge() {
        let mut a = ExtProgressStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtProgressStats::new();
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
    fn ext_progress_stats_display() {
        let mut stats = ExtProgressStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_progress_stats_default() {
        let stats = ExtProgressStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_progress_validator_accepts_valid_name() {
        let v = ExtProgressValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_progress_validator_rejects_empty() {
        let v = ExtProgressValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_progress_validator_rejects_too_long() {
        let v = ExtProgressValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_progress_validator_forbidden_prefix() {
        let v = ExtProgressValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_progress_validator_allowed_chars() {
        let v = ExtProgressValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_progress_validator_range() {
        let v = ExtProgressValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_progress_sanitize_removes_control() {
        let result = ExtProgressValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_progress_truncate_short_string() {
        assert_eq!(ExtProgressValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_progress_truncate_long_string() {
        let result = ExtProgressValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_progress_is_ascii_printable() {
        assert!(ExtProgressValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtProgressValidator::is_ascii_printable("Hello\x00World"));
    }

    // ── ProgressStack tests ──

    #[test]
    fn progress_stack_push_and_depth() {
        let mut stack = ProgressStack::new();
        assert!(stack.is_empty());
        assert_eq!(stack.depth(), 0);

        stack.push("task1", 1.0);
        assert_eq!(stack.depth(), 1);
        assert!(!stack.is_empty());

        stack.push("subtask", 2.0);
        assert_eq!(stack.depth(), 2);
        assert_eq!(stack.current_label(), Some("subtask"));
    }

    #[test]
    fn progress_stack_update_current_and_overall() {
        let mut stack = ProgressStack::new();
        stack.push("a", 1.0);
        stack.push("b", 1.0);

        // Both at 0% → overall 0%
        assert!((stack.overall_percentage() - 0.0).abs() < f64::EPSILON);

        stack.update_current(80.0);
        // a=0%, b=80%, equal weight → overall 40%
        let overall = stack.overall_percentage();
        assert!((overall - 40.0).abs() < 0.01, "expected ~40, got {overall}");
    }

    #[test]
    fn progress_stack_breadcrumb() {
        let mut stack = ProgressStack::new();
        stack.push("build", 1.0);
        stack.push("compile", 1.0);
        stack.push("link", 1.0);
        assert_eq!(stack.breadcrumb(), "build > compile > link");
    }

    #[test]
    fn progress_stack_pop_returns_entry() {
        let mut stack = ProgressStack::new();
        stack.push("first", 3.0);
        stack.push("second", 5.0);
        stack.update_current(60.0);

        let entry = stack.pop().unwrap();
        assert_eq!(entry.label, "second");
        assert!((entry.percentage - 60.0).abs() < f64::EPSILON);
        assert!((entry.weight - 5.0).abs() < f64::EPSILON);
        assert_eq!(stack.depth(), 1);
    }

    // ── ProgressCancelToken tests ──

    #[test]
    fn cancel_token_cancel() {
        let mut token = progress_cancel_token(42);
        assert!(!token.is_cancelled());
        assert_eq!(token.handle(), 42);

        token.cancel();
        assert!(token.is_cancelled());
        assert!(token.reason().is_none());
    }

    #[test]
    fn cancel_token_cancel_with_reason() {
        let mut token = ProgressCancelToken::new(99);
        token.cancel_with_reason("user pressed Escape");
        assert!(token.is_cancelled());
        assert_eq!(token.reason(), Some("user pressed Escape"));
    }

    // ── progress_format_message tests ──

    #[test]
    fn format_message_with_percentage() {
        let state = ProgressState {
            handle: 1,
            percentage: 50.0,
            message: Some("loading".into()),
            is_done: false,
        };
        let formatted = progress_format_message(&state);
        assert!(formatted.contains("50%"), "expected 50% in: {formatted}");
        assert!(formatted.contains("loading"));
        // 10 filled, 10 empty
        assert!(formatted.contains("##########----------"));
    }

    #[test]
    fn format_message_without_percentage() {
        let state = ProgressState {
            handle: 2,
            percentage: 0.0,
            message: Some("starting".into()),
            is_done: false,
        };
        let formatted = progress_format_message(&state);
        assert!(
            formatted.contains("...working..."),
            "expected spinner in: {formatted}"
        );
        assert!(formatted.contains("starting"));
    }

    #[test]
    fn format_message_done() {
        let state = ProgressState {
            handle: 3,
            percentage: 100.0,
            message: Some("finished".into()),
            is_done: true,
        };
        let formatted = progress_format_message(&state);
        assert!(formatted.contains("100%"));
        assert!(formatted.contains("(done)"));
    }

    // ── ProgressSummary tests ──

    #[test]
    fn progress_summary_from_bridge() {
        let mut bridge = ProgressBridge::new();
        let opts = ProgressOptions {
            location: ProgressLocation::Notification,
            title: Some("task1".into()),
            cancellable: false,
        };
        bridge.start(1, &opts);
        bridge.start(2, &opts);
        bridge.report(1, Some(40.0), None);
        bridge.report(2, Some(60.0), None);
        bridge.end(2);

        let summary = ProgressSummary::from_bridge(&bridge);
        assert_eq!(summary.total_active(), 1);
        assert_eq!(summary.total_completed(), 1);
        // Only handle 1 is active at 40%
        assert!(
            (summary.overall_progress() - 40.0).abs() < 0.01,
            "expected ~40, got {}",
            summary.overall_progress()
        );
        let display = summary.display();
        assert!(display.contains("1 active"));
        assert!(display.contains("1 completed"));
    }

    // ── ProgressTimeline tests ──

    #[test]
    fn timeline_eta_estimation() {
        let start = Instant::now();
        let mut tl = ProgressTimeline::with_start(start);
        // Simulate 50% done after 2 seconds
        tl.record_at(50.0, start + Duration::from_secs(2));

        let rate = tl.rate_pct_per_sec().unwrap();
        assert!((rate - 25.0).abs() < 0.01, "expected ~25 %/s, got {rate}");

        let eta = tl.eta().unwrap();
        // 50% remaining at 25%/s = 2s
        assert!(
            (eta.as_secs_f64() - 2.0).abs() < 0.1,
            "expected ~2s ETA, got {:.2}s",
            eta.as_secs_f64()
        );
        assert_eq!(tl.current_percentage(), 50.0);
        assert_eq!(tl.len(), 2);
        assert!(!tl.is_empty());
    }

    #[test]
    fn timeline_complete_eta_zero() {
        let start = Instant::now();
        let mut tl = ProgressTimeline::with_start(start);
        tl.record_at(100.0, start + Duration::from_secs(5));

        let eta = tl.eta().unwrap();
        assert_eq!(eta, Duration::ZERO);
    }

    // ── ProgressThrottle tests ──

    #[test]
    fn throttle_limits_frequency() {
        let mut throttle = ProgressThrottle::from_millis(100);
        let t0 = Instant::now();

        // First emit always succeeds
        assert_eq!(throttle.try_emit_at(10.0, t0), Some(10.0));
        // Too soon → throttled
        assert_eq!(throttle.try_emit_at(20.0, t0 + Duration::from_millis(50)), None);
        // After interval → succeeds
        assert_eq!(
            throttle.try_emit_at(30.0, t0 + Duration::from_millis(100)),
            Some(30.0)
        );
    }

    #[test]
    fn throttle_flush_emits_last() {
        let mut throttle = ProgressThrottle::from_millis(100);
        let t0 = Instant::now();
        throttle.try_emit_at(10.0, t0);
        throttle.try_emit_at(99.0, t0 + Duration::from_millis(10)); // throttled

        let flushed = throttle.flush();
        assert_eq!(flushed, Some(99.0));
    }

    // ── MultiProgressTracker tests ──

    #[test]
    fn multi_tracker_overall_and_lifecycle() {
        let mut mt = MultiProgressTracker::new();
        let a = mt.add("download", 1.0);
        let b = mt.add("extract", 1.0);

        mt.update(a, 50.0);
        mt.update(b, 0.0);
        // (50*1 + 0*1) / 2 = 25%
        assert!(
            (mt.overall_percentage() - 25.0).abs() < 0.01,
            "expected 25%, got {:.1}%",
            mt.overall_percentage()
        );
        assert_eq!(mt.active_count(), 2);
        assert!(!mt.all_done());

        mt.finish(a);
        mt.finish(b);
        assert!(mt.all_done());
        assert_eq!(mt.done_count(), 2);
        assert!((mt.overall_percentage() - 100.0).abs() < 0.01);
        assert_eq!(mt.label(a), Some("download"));

        let display = mt.to_string();
        assert!(display.contains("2/2 done"), "got: {display}");
    }

    // ── ProgressFormatter tests ──

    #[test]
    fn formatter_bar_and_eta() {
        let bar = ProgressFormatter::bar(50.0, 10);
        assert_eq!(bar, "[#####-----]");

        let bar_full = ProgressFormatter::bar(100.0, 10);
        assert_eq!(bar_full, "[##########]");

        let bar_empty = ProgressFormatter::bar(0.0, 10);
        assert_eq!(bar_empty, "[----------]");

        assert_eq!(ProgressFormatter::eta_string(Duration::from_secs(45)), "45s");
        assert_eq!(ProgressFormatter::eta_string(Duration::from_secs(125)), "2m 05s");
        assert_eq!(ProgressFormatter::eta_string(Duration::from_secs(3661)), "1h 01m");

        let line = ProgressFormatter::summary_line(50.0, Some(Duration::from_secs(30)), Some("compiling"));
        assert!(line.contains("50%"));
        assert!(line.contains("ETA 30s"));
        assert!(line.contains("compiling"));

        let rate = ProgressFormatter::rate_string(12.5, "%");
        assert_eq!(rate, "12.50 %/s");

        let slow_rate = ProgressFormatter::rate_string(0.001, "items");
        assert_eq!(slow_rate, "<0.01 items/s");
    }

    // ── From impls tests ──

    #[test]
    fn from_impls() {
        let s: String = ProgressLocation::Window.into();
        assert_eq!(s, "Window");

        let state = ProgressState {
            handle: 1,
            percentage: 60.0,
            message: None,
            is_done: false,
        };
        let summary = ProgressSummary::from(&state);
        assert_eq!(summary.active, 1);
        assert_eq!(summary.completed, 0);
        assert!((summary.overall_progress - 60.0).abs() < f64::EPSILON);

        let done_state = ProgressState {
            handle: 2,
            percentage: 100.0,
            message: None,
            is_done: true,
        };
        let summary = ProgressSummary::from(&done_state);
        assert_eq!(summary.active, 0);
        assert_eq!(summary.completed, 1);
    }

    // ── ProgressChain tests ──

    #[test]
    fn chain_empty_progress() {
        let chain = ProgressChain::new();
        assert!((chain.overall_progress()).abs() < f64::EPSILON);
        assert!(!chain.is_finished());
        assert_eq!(chain.step_count(), 0);
    }

    #[test]
    fn chain_weighted_progress() {
        let mut chain = ProgressChain::new();
        let dl = chain.add_step("download", 3.0);
        let inst = chain.add_step("install", 7.0);
        chain.report(dl, 50.0);
        chain.report(inst, 0.0);
        // weighted: (50*3 + 0*7) / 10 = 15
        assert!((chain.overall_progress() - 15.0).abs() < f64::EPSILON);
        assert_eq!(chain.current_step(), Some(0));
    }

    #[test]
    fn chain_complete_all_steps() {
        let mut chain = ProgressChain::new();
        let a = chain.add_step("a", 1.0);
        let b = chain.add_step("b", 1.0);
        chain.complete_step(a);
        assert!(!chain.is_finished());
        assert_eq!(chain.current_step(), Some(1));
        chain.complete_step(b);
        assert!(chain.is_finished());
        assert!((chain.overall_progress() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn chain_get_step() {
        let mut chain = ProgressChain::new();
        chain.add_step("fetch", 2.0);
        let step = chain.get_step(0).unwrap();
        assert_eq!(step.label, "fetch");
        assert!(!step.is_complete);
        assert!(chain.get_step(99).is_none());
    }

    // ── ProgressEstimator tests ──

    #[test]
    fn estimator_initial_state() {
        let est = ProgressEstimator::new(0.5);
        assert!(est.eta().is_none());
        assert!(est.rate().is_none());
    }

    #[test]
    fn estimator_eta_calculation() {
        let start = Instant::now();
        let mut est = ProgressEstimator::new(1.0);
        // record_at: 0% at t=0, 50% at t=5s → rate=10 %/s → ETA=5s
        est.record_at(0.0, start);
        est.record_at(50.0, start + Duration::from_secs(5));
        let eta = est.eta().unwrap();
        assert!((eta.as_secs_f64() - 5.0).abs() < 0.1);
    }

    #[test]
    fn estimator_smoothing() {
        let start = Instant::now();
        let mut est = ProgressEstimator::new(0.5);
        est.record_at(0.0, start);
        est.record_at(20.0, start + Duration::from_secs(2)); // rate = 10
        est.record_at(30.0, start + Duration::from_secs(4)); // rate = 5, smoothed = 0.5*5 + 0.5*10 = 7.5
        let rate = est.rate().unwrap();
        assert!((rate - 7.5).abs() < 0.01);
    }

    #[test]
    fn estimator_done_returns_zero_eta() {
        let start = Instant::now();
        let mut est = ProgressEstimator::new(1.0);
        est.record_at(0.0, start);
        est.record_at(100.0, start + Duration::from_secs(10));
        let eta = est.eta().unwrap();
        assert!(eta.as_secs_f64() < 0.01);
    }

    // ── ProgressNotificationLink tests ──

    #[test]
    fn notification_link_and_lookup() {
        let mut link = ProgressNotificationLink::new();
        link.link(1, "notif-abc");
        link.link(2, "notif-def");
        assert_eq!(link.notification_for(1), Some("notif-abc"));
        assert_eq!(link.handle_for("notif-def"), Some(2));
        assert_eq!(link.count(), 2);
    }

    #[test]
    fn notification_unlink() {
        let mut link = ProgressNotificationLink::new();
        link.link(10, "n1");
        link.unlink(10);
        assert!(link.notification_for(10).is_none());
        assert_eq!(link.count(), 0);
    }

    #[test]
    fn notification_no_duplicate_link() {
        let mut link = ProgressNotificationLink::new();
        link.link(5, "n");
        link.link(5, "n2"); // duplicate handle ignored
        assert_eq!(link.count(), 1);
        assert_eq!(link.notification_for(5), Some("n"));
    }

    // ── ProgressCancellationCascade tests ──

    #[test]
    fn cascade_cancel_parent_cancels_children() {
        let mut cascade = ProgressCancellationCascade::new();
        cascade.add_child(1, 2);
        cascade.add_child(1, 3);
        cascade.add_child(3, 4);
        cascade.cancel(1);
        assert!(cascade.is_cancelled(1));
        assert!(cascade.is_cancelled(2));
        assert!(cascade.is_cancelled(3));
        assert!(cascade.is_cancelled(4));
    }

    #[test]
    fn cascade_cancel_leaf_only() {
        let mut cascade = ProgressCancellationCascade::new();
        cascade.add_child(1, 2);
        cascade.cancel(2);
        assert!(cascade.is_cancelled(2));
        assert!(!cascade.is_cancelled(1));
    }

    #[test]
    fn cascade_children_of() {
        let mut cascade = ProgressCancellationCascade::new();
        cascade.add_child(10, 20);
        cascade.add_child(10, 30);
        let children = cascade.children_of(10);
        assert_eq!(children, vec![20, 30]);
        assert!(cascade.children_of(20).is_empty());
    }

    #[test]
    fn cascade_no_duplicate_edges() {
        let mut cascade = ProgressCancellationCascade::new();
        cascade.add_child(1, 2);
        cascade.add_child(1, 2);
        assert_eq!(cascade.children_of(1).len(), 1);
    }

#[test]
    fn progressnotificationbridge_severity_ordering() {
        assert!(ProgressNotificationBridgeSeverity::Critical > ProgressNotificationBridgeSeverity::High);
        assert!(ProgressNotificationBridgeSeverity::High > ProgressNotificationBridgeSeverity::Medium);
        assert!(ProgressNotificationBridgeSeverity::Medium > ProgressNotificationBridgeSeverity::Low);
    }

    #[test]
    fn progressnotificationbridge_severity_display() {
        assert_eq!(ProgressNotificationBridgeSeverity::Low.to_string(), "low");
        assert_eq!(ProgressNotificationBridgeSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn progressnotificationbridge_entry_creation() {
        let e = ProgressNotificationBridgeEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, ProgressNotificationBridgeSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn progressnotificationbridge_entry_builder() {
        let e = ProgressNotificationBridgeEntry::new("e2", "Entry 2")
            .with_severity(ProgressNotificationBridgeSeverity::High)
            .with_detail("some detail")
            .with_progress_pct(42);
        assert_eq!(e.severity, ProgressNotificationBridgeSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.progress_pct, 42);
    }

    #[test]
    fn progressnotificationbridge_entry_enable_disable() {
        let mut e = ProgressNotificationBridgeEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn progressnotificationbridge_add_and_count() {
        let mut mgr = ProgressNotificationBridge::new("test");
        mgr.add(ProgressNotificationBridgeEntry::new("a", "A"));
        mgr.add(ProgressNotificationBridgeEntry::new("b", "B").with_severity(ProgressNotificationBridgeSeverity::High));
        assert_eq!(mgr.progress_pct(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn progressnotificationbridge_remove() {
        let mut mgr = ProgressNotificationBridge::new("test");
        mgr.add(ProgressNotificationBridgeEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn progressnotificationbridge_capacity() {
        let mut mgr = ProgressNotificationBridge::new("test").with_capacity(1);
        assert!(mgr.add(ProgressNotificationBridgeEntry::new("a", "A")));
        assert!(!mgr.add(ProgressNotificationBridgeEntry::new("b", "B")));
    }

    #[test]
    fn progressnotificationbridge_sorted_by_severity() {
        let mut mgr = ProgressNotificationBridge::new("test");
        mgr.add(ProgressNotificationBridgeEntry::new("lo", "Low"));
        mgr.add(ProgressNotificationBridgeEntry::new("hi", "High").with_severity(ProgressNotificationBridgeSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, ProgressNotificationBridgeSeverity::Critical);
    }

    #[test]
    fn progressnotificationbridge_summary() {
        let mgr = ProgressNotificationBridge::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn progresscancellationhandler_config_defaults() {
        let cfg = ProgressCancellationHandlerConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn progresscancellationhandler_item_creation() {
        let item = ProgressCancellationHandlerItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn progresscancellationhandler_add_and_get() {
        let mut mgr = ProgressCancellationHandler::new(ProgressCancellationHandlerConfig::new("test"));
        mgr.add(ProgressCancellationHandlerItem::new("k1", "v1"));
        assert_eq!(mgr.notification_count(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn progresscancellationhandler_remove_item() {
        let mut mgr = ProgressCancellationHandler::new(ProgressCancellationHandlerConfig::new("test"));
        mgr.add(ProgressCancellationHandlerItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn progresscancellationhandler_sorted_by_priority() {
        let mut mgr = ProgressCancellationHandler::new(ProgressCancellationHandlerConfig::new("test"));
        mgr.add(ProgressCancellationHandlerItem::new("lo", "low").with_priority(1));
        mgr.add(ProgressCancellationHandlerItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn progresscancellationhandler_items_with_tag() {
        let mut mgr = ProgressCancellationHandler::new(ProgressCancellationHandlerConfig::new("test"));
        mgr.add(ProgressCancellationHandlerItem::new("a", "1").with_tag("x"));
        mgr.add(ProgressCancellationHandlerItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn progresscancellationhandler_report() {
        let mgr = ProgressCancellationHandler::new(ProgressCancellationHandlerConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn ext_progress_entry_creation() {
        let e = ExtProgressEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn ext_progress_entry_with_priority() {
        let e = ExtProgressEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn ext_progress_entry_metadata() {
        let e = ExtProgressEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn ext_progress_entry_remove_meta() {
        let mut e = ExtProgressEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn ext_progress_entry_activate_deactivate() {
        let mut e = ExtProgressEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn ext_progress_config_add_sorted() {
        let mut c = ExtProgressConfig::new(10);
        c.add(ExtProgressEntry::new("lo", "Lo").with_priority(1));
        c.add(ExtProgressEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn ext_progress_config_capacity() {
        let mut c = ExtProgressConfig::new(1);
        assert!(c.add(ExtProgressEntry::new("a", "A")));
        assert!(!c.add(ExtProgressEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn ext_progress_config_remove() {
        let mut c = ExtProgressConfig::new(10);
        c.add(ExtProgressEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn ext_progress_config_get() {
        let mut c = ExtProgressConfig::new(10);
        c.add(ExtProgressEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn ext_progress_config_active_entries() {
        let mut c = ExtProgressConfig::new(10);
        c.add(ExtProgressEntry::new("a", "A"));
        c.add(ExtProgressEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn ext_progress_config_enable_disable() {
        let mut c = ExtProgressConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn ext_progress_config_clear() {
        let mut c = ExtProgressConfig::new(10);
        c.add(ExtProgressEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn ext_progress_config_find_by_label() {
        let mut c = ExtProgressConfig::new(10);
        c.add(ExtProgressEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn ext_progress_config_top_n() {
        let mut c = ExtProgressConfig::new(10);
        c.add(ExtProgressEntry::new("a", "A").with_priority(1));
        c.add(ExtProgressEntry::new("b", "B").with_priority(2));
        c.add(ExtProgressEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn ext_progress_config_deactivate_activate_all() {
        let mut c = ExtProgressConfig::new(10);
        c.add(ExtProgressEntry::new("a", "A"));
        c.add(ExtProgressEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn ext_progress_config_highest_priority() {
        let mut c = ExtProgressConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(ExtProgressEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn ext_progress_config_contains() {
        let mut c = ExtProgressConfig::new(10);
        c.add(ExtProgressEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn ext_progress_config_labels() {
        let mut c = ExtProgressConfig::new(10);
        c.add(ExtProgressEntry::new("a", "Alpha"));
        c.add(ExtProgressEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn ext_progress_config_drain_inactive() {
        let mut c = ExtProgressConfig::new(10);
        c.add(ExtProgressEntry::new("a", "A"));
        c.add(ExtProgressEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn qv_metrics_empty() {
        let m = QvMetrics::new("ext_prog");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qv_metrics_record_and_mean() {
        let mut m = QvMetrics::new("ext_prog");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qv_metrics_min_max() {
        let mut m = QvMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qv_metrics_variance_and_std() {
        let mut m = QvMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn qv_metrics_percentile() {
        let mut m = QvMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qv_metrics_merge() {
        let mut a = QvMetrics::new("a");
        a.record(1.0);
        let mut b = QvMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qv_metrics_reset() {
        let mut m = QvMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qv_rate_window_empty() {
        let rw = QvRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qv_rate_window_tick_and_rate() {
        let mut rw = QvRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qv_lru_cache_basic() {
        let mut c = QvLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qv_lru_cache_contains_and_keys() {
        let mut c = QvLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qv_lru_cache_remove() {
        let mut c = QvLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qv_metrics_sum() {
        let mut m = QvMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qv_metrics_label() {
        let m = QvMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qv_lru_cache_clear() {
        let mut c = QvLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_11_push_and_len() {
        let mut rb = super::XbRingBuffer11::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_11_overwrite() {
        let mut rb = super::XbRingBuffer11::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_11_get_out_of_bounds() {
        let rb = super::XbRingBuffer11::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_11_drain_all() {
        let mut rb = super::XbRingBuffer11::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_11_peek_front_back() {
        let mut rb = super::XbRingBuffer11::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_11_clear() {
        let mut rb = super::XbRingBuffer11::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_11_capacity() {
        let rb = super::XbRingBuffer11::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_11_basic() {
        let h = super::xb_fnv1a_11(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_11(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_11_different_inputs() {
        let h1 = super::xb_fnv1a_11(b"abc");
        let h2 = super::xb_fnv1a_11(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_11_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_11(&data);
        let dec = super::xb_rle_decode_11(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_11_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_11(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_11(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_11_values() {
        assert!((super::xb_clamp_11(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_11(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_11(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_11_values() {
        assert!((super::xb_lerp_11(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_11(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_11(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_11_wrap_around_twice() {
        let mut rb = super::XbRingBuffer11::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 66 ----

    #[test]
    fn xc_66_pool_new_empty() {
        let pool: super::Xc66Pool<i32> = super::Xc66Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_66_pool_release_acquire() {
        let mut pool = super::Xc66Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_66_pool_acquire_empty() {
        let mut pool: super::Xc66Pool<i32> = super::Xc66Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_66_pool_full() {
        let mut pool = super::Xc66Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_66_pool_drain() {
        let mut pool = super::Xc66Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_66_pool_stats() {
        let mut pool = super::Xc66Pool::new(8);
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
    fn xc_66_pool_clear() {
        let mut pool = super::Xc66Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_66_pool_shrink() {
        let mut pool = super::Xc66Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_66_pool_default() {
        let pool: super::Xc66Pool<String> = super::Xc66Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_66_pool_extend() {
        let mut pool = super::Xc66Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_66_pool_retain() {
        let mut pool = super::Xc66Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_66_scheduler_round_robin() {
        let mut sched = super::Xc66Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_66_scheduler_empty() {
        let mut sched = super::Xc66Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_66_scheduler_reset() {
        let mut sched = super::Xc66Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_66_scheduler_add_remove() {
        let mut sched = super::Xc66Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_66_scheduler_targets() {
        let sched = super::Xc66Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_66_hash_empty() {
        assert_eq!(super::xc_66_hash(b""), 5381);
    }

    #[test]
    fn xc_66_hash_data() {
        let h = super::xc_66_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_66_hash(b"hello"), h);
    }

    #[test]
    fn xc_66_reverse_str() {
        assert_eq!(super::xc_66_reverse("abc"), "cba");
        assert_eq!(super::xc_66_reverse(""), "");
    }


    #[test]
    fn xe_23_pipeline_empty() {
        let p = super::Xe23Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_23_pipeline_parse_stage() {
        let p = super::Xe23Pipeline::new()
            .add_parse(super::xe_23_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_23_pipeline_transform_double() {
        let p = super::Xe23Pipeline::new()
            .add_transform(super::xe_23_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_23_pipeline_validate_reverse() {
        let p = super::Xe23Pipeline::new()
            .add_validate(super::xe_23_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_23_pipeline_emit_filter() {
        let p = super::Xe23Pipeline::new()
            .add_emit(super::xe_23_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_23_pipeline_multi_stage() {
        let p = super::Xe23Pipeline::new()
            .add_parse(super::xe_23_pipeline_identity)
            .add_transform(super::xe_23_pipeline_double)
            .add_validate(super::xe_23_pipeline_reverse)
            .add_emit(super::xe_23_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_23_pipeline_error_propagation() {
        let p = super::Xe23Pipeline::new()
            .add_parse(super::xe_23_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe23Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_23_pipeline_compose() {
        let p1 = super::Xe23Pipeline::new()
            .add_parse(super::xe_23_pipeline_identity);
        let p2 = super::Xe23Pipeline::new()
            .add_transform(super::xe_23_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_23_pipeline_error_display() {
        let e = super::Xe23PipelineError {
            stage: super::Xe23Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_23_cache_put_get() {
        let mut c = super::Xe23Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_23_cache_miss() {
        let mut c: super::Xe23Cache<&str, i32> = super::Xe23Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_23_cache_ttl_expiry() {
        let mut c = super::Xe23Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_23_cache_evict() {
        let mut c = super::Xe23Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_23_cache_capacity() {
        let mut c = super::Xe23Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_23_cache_stats() {
        let mut c = super::Xe23Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_23_cache_clear() {
        let mut c = super::Xe23Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #103 --

    #[test]
    fn xf103_trie_insert_search() {
        let mut t = Xf103Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf103_trie_starts_with() {
        let mut t = Xf103Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf103_trie_remove() {
        let mut t = Xf103Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf103_trie_word_count() {
        let mut t = Xf103Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf103_trie_longest_prefix() {
        let mut t = Xf103Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf103_trie_all_words() {
        let mut t = Xf103Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf103_trie_autocomplete() {
        let mut t = Xf103Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf103_trie_empty_search() {
        let t = Xf103Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf103_bloom_add_contains() {
        let mut bf = Xf103BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf103_bloom_probably_absent() {
        let bf = Xf103BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf103_bloom_false_positive_rate() {
        let mut bf = Xf103BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf103_bloom_clear() {
        let mut bf = Xf103BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf103_bloom_union() {
        let mut a = Xf103BloomFilter::xf_new(512, 2);
        let mut b = Xf103BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf103_bloom_intersection_estimate() {
        let mut a = Xf103BloomFilter::xf_new(512, 2);
        let mut b = Xf103BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf103_bloom_union_size_mismatch() {
        let a = Xf103BloomFilter::xf_new(256, 2);
        let b = Xf103BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh65_skip_insert_contains() {
        let mut sl = super::Xh65SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh65_skip_remove() {
        let mut sl = super::Xh65SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh65_skip_len() {
        let mut sl = super::Xh65SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh65_skip_range_query() {
        let mut sl = super::Xh65SkipList::xh_new(4);
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
    fn xh65_skip_floor_ceiling() {
        let mut sl = super::Xh65SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh65_skip_rank() {
        let mut sl = super::Xh65SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh65_skip_empty() {
        let sl = super::Xh65SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh65_skip_duplicates() {
        let mut sl = super::Xh65SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh65_bitset_set_test() {
        let mut bs = super::Xh65BitSet::xh_new(256);
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
    fn xh65_bitset_clear_count() {
        let mut bs = super::Xh65BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh65_bitset_and_or_xor() {
        let mut a = super::Xh65BitSet::xh_new(128);
        let mut b = super::Xh65BitSet::xh_new(128);
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
    fn xh65_bitset_iter_ones() {
        let mut bs = super::Xh65BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh65_bitset_first_last() {
        let mut bs = super::Xh65BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh65_bitset_empty() {
        let bs = super::Xh65BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi65_deque_push_pop_back() {
        let mut dq = super::Xi65Deque::xi_new(4);
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
    fn xi65_deque_push_pop_front() {
        let mut dq = super::Xi65Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi65_deque_mixed_ops() {
        let mut dq = super::Xi65Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi65_deque_get_and_split() {
        let mut dq = super::Xi65Deque::xi_new(8);
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
    fn xi65_deque_rotate_left() {
        let mut dq = super::Xi65Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi65_deque_rotate_right() {
        let mut dq = super::Xi65Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi65_deque_grow() {
        let mut dq = super::Xi65Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi65_deque_empty() {
        let dq = super::Xi65Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi65_interval_tree_insert_query() {
        let mut tree = super::Xi65IntervalTree::xi_new();
        tree.xi_insert(super::Xi65Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi65Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi65Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi65_interval_tree_overlap() {
        let mut tree = super::Xi65IntervalTree::xi_new();
        tree.xi_insert(super::Xi65Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi65Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi65Interval::xi_new(12, 20));
        let q = super::Xi65Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi65_interval_tree_remove() {
        let mut tree = super::Xi65IntervalTree::xi_new();
        tree.xi_insert(super::Xi65Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi65Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi65_interval_tree_gaps() {
        let mut tree = super::Xi65IntervalTree::xi_new();
        tree.xi_insert(super::Xi65Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi65Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi65Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi65Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi65Interval::xi_new(8, 10));
    }

    #[test]
    fn xi65_interval_tree_merge() {
        let mut tree = super::Xi65IntervalTree::xi_new();
        tree.xi_insert(super::Xi65Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi65Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi65Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi65Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi65Interval::xi_new(10, 15));
    }

    #[test]
    fn xi65_interval_tree_all() {
        let mut tree = super::Xi65IntervalTree::xi_new();
        tree.xi_insert(super::Xi65Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi65Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi65_interval_tree_empty() {
        let tree = super::Xi65IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi65_interval_tree_contains_point() {
        let iv = super::Xi65Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 66) ---

    #[test]
    fn xj_66_uf_make_and_find() {
        let mut uf = super::Xj66UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_66_uf_union_connected() {
        let mut uf = super::Xj66UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_66_uf_component_count() {
        let mut uf = super::Xj66UnionFind::xj_new();
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
    fn xj_66_uf_component_size() {
        let mut uf = super::Xj66UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_66_uf_largest_component() {
        let mut uf = super::Xj66UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_66_uf_many_elements() {
        let mut uf = super::Xj66UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_66_uf_separate_components() {
        let mut uf = super::Xj66UnionFind::xj_new();
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
    fn xj_66_uf_path_compression() {
        let mut uf = super::Xj66UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_66_bt_insert_get() {
        let mut bt = super::Xj66BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_66_bt_contains_len() {
        let mut bt = super::Xj66BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_66_bt_replace() {
        let mut bt = super::Xj66BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_66_bt_remove() {
        let mut bt = super::Xj66BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_66_bt_keys_values() {
        let mut bt = super::Xj66BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_66_bt_range() {
        let mut bt = super::Xj66BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_66_bt_min_max() {
        let mut bt = super::Xj66BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_66_bt_many_inserts() {
        let mut bt = super::Xj66BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_65 segment tree tests ---

    #[test]
    fn xk_65_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk65SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_65_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk65SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_65_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk65SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_65_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk65SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_65_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk65SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_65_st_single_element() {
        let data = vec![42];
        let st = super::Xk65SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_65_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk65SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_65_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk65SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_65 disjoint intervals tests ---

    #[test]
    fn xk_65_di_add_and_count() {
        let mut di = super::Xk65DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_65_di_merge_overlap() {
        let mut di = super::Xk65DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_65_di_contains() {
        let mut di = super::Xk65DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_65_di_remove() {
        let mut di = super::Xk65DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_65_di_covered_length() {
        let mut di = super::Xk65DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_65_di_gaps() {
        let mut di = super::Xk65DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_65_di_merge_adjacent() {
        let mut di = super::Xk65DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_65_di_empty() {
        let di = super::Xk65DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_66_rope_new_empty() {
        let rope = super::Xl66Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_66_rope_from_str() {
        let rope = super::Xl66Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_66_rope_insert_at() {
        let mut rope = super::Xl66Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_66_rope_delete_range() {
        let mut rope = super::Xl66Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_66_rope_char_at() {
        let rope = super::Xl66Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_66_rope_split_concat() {
        let rope = super::Xl66Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_66_rope_line_count() {
        let rope = super::Xl66Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_66_rope_line_at() {
        let rope = super::Xl66Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_66_sa_build_and_search() {
        let sa = super::Xl66SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_66_sa_count() {
        let sa = super::Xl66SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_66_sa_longest_repeated() {
        let sa = super::Xl66SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_66_sa_all_positions() {
        let sa = super::Xl66SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_66_sa_len() {
        let sa = super::Xl66SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_66_sa_empty() {
        let sa = super::Xl66SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_66_rope_slice() {
        let rope = super::Xl66Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_66_sa_search_start() {
        let sa = super::Xl66SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_66_sparse_set_get() {
        let mut m = super::Xm66MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_66_sparse_row_col() {
        let mut m = super::Xm66MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_66_sparse_transpose() {
        let mut m = super::Xm66MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_66_sparse_multiply_vec() {
        let mut m = super::Xm66MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_66_sparse_nnz_density() {
        let mut m = super::Xm66MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_66_sparse_clear() {
        let mut m = super::Xm66MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_66_sparse_overwrite_zero() {
        let mut m = super::Xm66MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_66_tokenizer_basic() {
        let t = super::Xm66Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_66_tokenizer_count() {
        let t = super::Xm66Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_66_tokenizer_unique() {
        let t = super::Xm66Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_66_tokenizer_frequency() {
        let t = super::Xm66Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_66_tokenizer_delimiter() {
        let t = super::Xm66Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_66_tokenizer_whitespace() {
        let t = super::Xm66Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_66_tokenizer_empty() {
        let t = super::Xm66Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 65 ----

    #[test]
    fn xn_65_fenwick_prefix_sum() {
        let mut ft = super::Xn65Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_65_fenwick_range_sum() {
        let mut ft = super::Xn65Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_65_fenwick_point_query() {
        let mut ft = super::Xn65Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_65_fenwick_len() {
        let ft = super::Xn65Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_65_fenwick_multiple_updates() {
        let mut ft = super::Xn65Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_65_fenwick_single_element() {
        let mut ft = super::Xn65Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_65_fenwick_find_kth() {
        let mut ft = super::Xn65Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_65_fenwick_negative_delta() {
        let mut ft = super::Xn65Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 65 ----

    #[test]
    fn xn_65_avl_insert_get() {
        let mut m = super::Xn65AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_65_avl_remove() {
        let mut m = super::Xn65AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_65_avl_in_order() {
        let mut m = super::Xn65AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_65_avl_min_max() {
        let mut m = super::Xn65AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_65_avl_floor_ceiling() {
        let mut m = super::Xn65AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_65_avl_height_balanced() {
        let mut m = super::Xn65AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_65_avl_overwrite() {
        let mut m = super::Xn65AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_65_avl_empty() {
        let m: super::Xn65AVL<i32, i32> = super::Xn65AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo65RedBlack tests ---

    #[test]
    fn xo_65_rb_insert_and_get() {
        let mut tree = super::Xo65RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_65_rb_len_and_empty() {
        let mut tree = super::Xo65RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_65_rb_min_max() {
        let mut tree = super::Xo65RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_65_rb_contains() {
        let mut tree = super::Xo65RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_65_rb_remove() {
        let mut tree = super::Xo65RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_65_rb_in_order() {
        let mut tree = super::Xo65RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_65_rb_black_height() {
        let mut tree = super::Xo65RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_65_rb_overwrite() {
        let mut tree = super::Xo65RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo65ConsistentHash tests ---

    #[test]
    fn xo_65_ch_add_and_count() {
        let mut ring = super::Xo65ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_65_ch_remove_node() {
        let mut ring = super::Xo65ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_65_ch_get_node() {
        let mut ring = super::Xo65ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_65_ch_empty_ring() {
        let ring = super::Xo65ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_65_ch_distribution() {
        let mut ring = super::Xo65ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_65_ch_rebalance() {
        let mut ring = super::Xo65ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_65_ch_virtual_nodes() {
        let mut ring = super::Xo65ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_65_ch_consistent_lookup() {
        let mut ring = super::Xo65ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_65_splay_insert_get() {
        let mut t = super::Xp65SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_65_splay_remove() {
        let mut t = super::Xp65SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_65_splay_count_increases() {
        let mut t = super::Xp65SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_65_splay_depth() {
        let mut t = super::Xp65SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_65_splay_len_empty() {
        let t = super::Xp65SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_65_splay_min_max() {
        let mut t = super::Xp65SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_65_splay_overwrite() {
        let mut t = super::Xp65SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_65_splay_remove_missing() {
        let mut t = super::Xp65SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_65 treap tests ----
    #[test]
    fn xq_65_treap_empty() {
        let t = super::Xq65Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_65_treap_insert_get() {
        let mut t = super::Xq65Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_65_treap_overwrite() {
        let mut t = super::Xq65Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_65_treap_remove() {
        let mut t = super::Xq65Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_65_treap_min_max() {
        let mut t = super::Xq65Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_65_treap_rank() {
        let mut t = super::Xq65Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_65_treap_kth() {
        let mut t = super::Xq65Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_65_treap_in_order() {
        let mut t = super::Xq65Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_65 VEB tree tests ----
    #[test]
    fn xq_65_veb_empty() {
        let v = super::Xq65VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_65_veb_insert_contains() {
        let mut v = super::Xq65VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_65_veb_min_max() {
        let mut v = super::Xq65VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_65_veb_delete() {
        let mut v = super::Xq65VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_65_veb_successor() {
        let mut v = super::Xq65VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_65_veb_predecessor() {
        let mut v = super::Xq65VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_65_veb_count() {
        let mut v = super::Xq65VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_65_veb_duplicate_insert() {
        let mut v = super::Xq65VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_65_kdtree_empty() {
        let tree = super::Xr65KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_65_kdtree_insert_one() {
        let mut tree = super::Xr65KDTree::xr_new();
        tree.xr_insert(super::Xr65KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_65_kdtree_insert_multiple() {
        let mut tree = super::Xr65KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr65KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_65_kdtree_nearest_neighbor() {
        let mut tree = super::Xr65KDTree::xr_new();
        tree.xr_insert(super::Xr65KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr65KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr65KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_65_kdtree_nn_empty() {
        let tree = super::Xr65KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr65KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_65_kdtree_range_search() {
        let mut tree = super::Xr65KDTree::xr_new();
        tree.xr_insert(super::Xr65KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr65KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr65KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_65_kdtree_range_empty() {
        let mut tree = super::Xr65KDTree::xr_new();
        tree.xr_insert(super::Xr65KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_65_kdtree_all_points() {
        let mut tree = super::Xr65KDTree::xr_new();
        tree.xr_insert(super::Xr65KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr65KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_65_kdtree_depth() {
        let mut tree = super::Xr65KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr65KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_65_kdtree_bounding_box() {
        let mut tree = super::Xr65KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr65KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr65KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

}
