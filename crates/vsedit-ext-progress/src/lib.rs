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

}
