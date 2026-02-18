//! Progress indicators.

use std::collections::HashMap;
use std::fmt;

/// Errors returned by fallible progress operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressError {
    TaskNotFound(u64),
    AlreadyComplete(u64),
    NotCancellable(u64),
}

impl fmt::Display for ProgressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TaskNotFound(id) => write!(f, "task {id} not found"),
            Self::AlreadyComplete(id) => write!(f, "task {id} is already complete"),
            Self::NotCancellable(id) => write!(f, "task {id} is not cancellable"),
        }
    }
}

/// Where a progress indicator is displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressLocation {
    Notification,
    Window,
    SourceControl,
    StatusBar,
    Panel,
    Editor,
}

impl fmt::Display for ProgressLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Notification => write!(f, "Notification"),
            Self::Window => write!(f, "Window"),
            Self::SourceControl => write!(f, "Source Control"),
            Self::StatusBar => write!(f, "Status Bar"),
            Self::Panel => write!(f, "Panel"),
            Self::Editor => write!(f, "Editor"),
        }
    }
}

/// Options for starting a progress task.
#[derive(Debug, Clone)]
pub struct ProgressOptions {
    pub location: ProgressLocation,
    pub title: Option<String>,
    pub cancellable: bool,
}

/// Builder for constructing [`ProgressOptions`].
#[derive(Debug)]
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

    pub fn build(self) -> ProgressOptions {
        ProgressOptions {
            location: self.location,
            title: self.title,
            cancellable: self.cancellable,
        }
    }
}

/// Current progress report.
#[derive(Debug, Clone)]
pub struct ProgressReport {
    pub message: Option<String>,
    pub increment: Option<f64>,
    pub total: f64,
}

/// A running progress task.
#[derive(Debug, Clone)]
pub struct ProgressTask {
    pub id: u64,
    pub options: ProgressOptions,
    pub report: ProgressReport,
    pub cancelled: bool,
    pub done: bool,
}

impl ProgressTask {
    /// Returns `true` if no increments have been reported yet.
    pub fn is_indeterminate(&self) -> bool {
        self.report.increment.is_none()
    }
}

impl fmt::Display for ProgressTask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let title = self.options.title.as_deref().unwrap_or("Untitled");
        let pct = self.report.total.min(100.0).max(0.0);
        write!(f, "[{title}] {pct:.0}%")
    }
}

/// Service for managing progress tasks.
pub struct ProgressService {
    tasks: Vec<ProgressTask>,
    next_id: u64,
}

impl ProgressService {
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            next_id: 1,
        }
    }

    pub fn start(&mut self, options: ProgressOptions) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.tasks.push(ProgressTask {
            id,
            options,
            report: ProgressReport {
                message: None,
                increment: None,
                total: 0.0,
            },
            cancelled: false,
            done: false,
        });
        id
    }

    pub fn report(&mut self, id: u64, message: Option<String>, increment: Option<f64>) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.report.message = message;
            if let Some(inc) = increment {
                task.report.total += inc;
                task.report.increment = Some(inc);
            }
        }
    }

    pub fn cancel(&mut self, id: u64) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.cancelled = true;
            task.done = true;
        }
    }

    pub fn complete(&mut self, id: u64) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.done = true;
        }
    }

    pub fn get_task(&self, id: u64) -> Option<&ProgressTask> {
        self.tasks.iter().find(|t| t.id == id)
    }

    pub fn active_count(&self) -> usize {
        self.tasks.iter().filter(|t| !t.done).count()
    }

    pub fn is_cancelled(&self, id: u64) -> bool {
        self.tasks
            .iter()
            .find(|t| t.id == id)
            .map_or(false, |t| t.cancelled)
    }

    /// Cancel a task, returning an error if the task is not found,
    /// already complete, or not cancellable.
    pub fn try_cancel(&mut self, id: u64) -> Result<(), ProgressError> {
        let task = self
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or(ProgressError::TaskNotFound(id))?;
        if task.done {
            return Err(ProgressError::AlreadyComplete(id));
        }
        if !task.options.cancellable {
            return Err(ProgressError::NotCancellable(id));
        }
        task.cancelled = true;
        task.done = true;
        Ok(())
    }

    /// Returns all tasks that are not yet done.
    pub fn get_active_tasks(&self) -> Vec<&ProgressTask> {
        self.tasks.iter().filter(|t| !t.done).collect()
    }

    /// Removes all completed (done) tasks and returns how many were removed.
    pub fn remove_completed(&mut self) -> usize {
        let before = self.tasks.len();
        self.tasks.retain(|t| !t.done);
        before - self.tasks.len()
    }

    /// Returns the percentage (0.0–100.0) for a task, clamped.
    /// Returns `None` if the task is not found.
    pub fn percentage(&self, id: u64) -> Option<f64> {
        self.get_task(id)
            .map(|t| t.report.total.min(100.0).max(0.0))
    }

    /// Returns the total number of tasks (active and done).
    pub fn total_task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Returns true if tasks is empty.
    pub fn is_tasks_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Get the first task, if any.
    pub fn first_task(&self) -> Option<&ProgressTask> {
        self.tasks.first()
    }

    /// Get the last task, if any.
    pub fn last_task(&self) -> Option<&ProgressTask> {
        self.tasks.last()
    }

    /// Retain only tasks matching the predicate.
    pub fn retain_tasks(&mut self, f: impl Fn(&ProgressTask) -> bool) {
        self.tasks.retain(|item| f(item));
    }
}

impl Default for ProgressService {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for wb-progress operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbProgressStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbProgressStats {
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
    pub fn merge(&mut self, other: &WbProgressStats) {
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

impl Default for WbProgressStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbProgressStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbProgressStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-progress.
#[derive(Debug, Clone)]
pub struct WbProgressValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbProgressValidator {
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

impl Default for WbProgressValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ProgressReportBatch — batch multiple reports
// ---------------------------------------------------------------------------

/// A queued report to be applied in batch.
struct QueuedReport {
    task_id: u64,
    message: Option<String>,
    increment: Option<f64>,
}

/// Batch multiple progress reports for grouped application.
pub struct ProgressReportBatch {
    reports: Vec<QueuedReport>,
}

impl ProgressReportBatch {
    pub fn new() -> Self {
        Self {
            reports: Vec::new(),
        }
    }

    pub fn add(&mut self, task_id: u64, message: Option<String>, increment: Option<f64>) {
        self.reports.push(QueuedReport {
            task_id,
            message,
            increment,
        });
    }

    pub fn apply(&self, service: &mut ProgressService) {
        for r in &self.reports {
            service.report(r.task_id, r.message.clone(), r.increment);
        }
    }

    pub fn len(&self) -> usize {
        self.reports.len()
    }

    pub fn is_empty(&self) -> bool {
        self.reports.is_empty()
    }

    pub fn clear(&mut self) {
        self.reports.clear();
    }
}

impl Default for ProgressReportBatch {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// progress_estimate_remaining
// ---------------------------------------------------------------------------

/// Estimate remaining time in milliseconds based on completed fraction and elapsed time.
/// Returns `None` if fraction is <= 0 or >= 1.0.
pub fn estimate_remaining_ms(completed_fraction: f64, elapsed_ms: u64) -> Option<u64> {
    if completed_fraction <= 0.0 || completed_fraction >= 1.0 {
        return None;
    }
    let remaining = elapsed_ms as f64 * (1.0 - completed_fraction) / completed_fraction;
    Some(remaining as u64)
}

// ---------------------------------------------------------------------------
// ProgressService extensions
// ---------------------------------------------------------------------------

impl ProgressService {
    /// Get all tasks at a specific location.
    pub fn get_tasks_by_location(&self, location: ProgressLocation) -> Vec<&ProgressTask> {
        self.tasks
            .iter()
            .filter(|t| !t.done && t.options.location == location)
            .collect()
    }

    /// Cancel all active tasks, returning the count cancelled.
    pub fn cancel_all(&mut self) -> usize {
        let mut count = 0;
        for task in &mut self.tasks {
            if !task.done {
                task.cancelled = true;
                task.done = true;
                count += 1;
            }
        }
        count
    }

    /// Average percentage across all active tasks.
    pub fn overall_percentage(&self) -> f64 {
        let active: Vec<&ProgressTask> = self.tasks.iter().filter(|t| !t.done).collect();
        if active.is_empty() {
            return 0.0;
        }
        let sum: f64 = active.iter().map(|t| t.report.total.min(100.0).max(0.0)).sum();
        sum / active.len() as f64
    }
}

// ---------------------------------------------------------------------------
// ProgressTimer – elapsed time tracking for progress bars
// ---------------------------------------------------------------------------

/// Tracks elapsed time for a progress operation using monotonic instants.
#[derive(Debug, Clone)]
pub struct ProgressTimer {
    start: std::time::Instant,
    label: String,
    paused_elapsed: Option<std::time::Duration>,
}

impl ProgressTimer {
    /// Start a new timer with the given label.
    pub fn start(label: impl Into<String>) -> Self {
        Self {
            start: std::time::Instant::now(),
            label: label.into(),
            paused_elapsed: None,
        }
    }

    /// Elapsed time since the timer was started (excluding paused time).
    pub fn elapsed(&self) -> std::time::Duration {
        match self.paused_elapsed {
            Some(d) => d,
            None => self.start.elapsed(),
        }
    }

    /// Pause the timer, freezing the elapsed duration.
    pub fn pause(&mut self) {
        if self.paused_elapsed.is_none() {
            self.paused_elapsed = Some(self.start.elapsed());
        }
    }

    /// Resume the timer after a pause.
    pub fn resume(&mut self) {
        if let Some(paused) = self.paused_elapsed.take() {
            self.start = std::time::Instant::now() - paused;
        }
    }

    /// Whether the timer is currently paused.
    pub fn is_paused(&self) -> bool {
        self.paused_elapsed.is_some()
    }

    /// The label associated with this timer.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Format elapsed time as a human-readable string (e.g., "1m 23s" or "45s").
    pub fn elapsed_display(&self) -> String {
        format_duration(self.elapsed())
    }
}

impl fmt::Display for ProgressTimer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.label, self.elapsed_display())
    }
}

/// Format a duration as a human-readable string.
pub fn format_duration(d: std::time::Duration) -> String {
    let total_secs = d.as_secs();
    if total_secs >= 3600 {
        let h = total_secs / 3600;
        let m = (total_secs % 3600) / 60;
        let s = total_secs % 60;
        format!("{h}h {m}m {s}s")
    } else if total_secs >= 60 {
        let m = total_secs / 60;
        let s = total_secs % 60;
        format!("{m}m {s}s")
    } else if total_secs > 0 {
        format!("{total_secs}s")
    } else {
        let ms = d.as_millis();
        format!("{ms}ms")
    }
}

/// Estimate remaining time given current progress (0.0..=1.0) and elapsed duration.
pub fn estimate_remaining(progress: f64, elapsed: std::time::Duration) -> Option<std::time::Duration> {
    if progress <= 0.0 || progress > 1.0 {
        return None;
    }
    let total_estimate = elapsed.as_secs_f64() / progress;
    let remaining = total_estimate - elapsed.as_secs_f64();
    if remaining < 0.0 {
        None
    } else {
        Some(std::time::Duration::from_secs_f64(remaining))
    }
}

// ---------------------------------------------------------------------------
// ProgressLocation extensions
// ---------------------------------------------------------------------------

impl ProgressLocation {
    pub fn is_notification(&self) -> bool {
        matches!(self, Self::Notification)
    }

    pub fn is_status_bar(&self) -> bool {
        matches!(self, Self::StatusBar)
    }

    pub fn is_window(&self) -> bool {
        matches!(self, Self::Window)
    }

    pub fn is_overlay(&self) -> bool {
        matches!(self, Self::Notification | Self::Window)
    }
}

// ---------------------------------------------------------------------------
// ProgressOptions extensions
// ---------------------------------------------------------------------------

impl ProgressOptions {
    pub fn is_cancellable(&self) -> bool {
        self.cancellable
    }

    pub fn has_title(&self) -> bool {
        self.title.is_some()
    }

    pub fn summary(&self) -> String {
        let title = self.title.as_deref().unwrap_or("(no title)");
        let cancel = if self.cancellable { "cancellable" } else { "non-cancellable" };
        format!("{title} @ {location} [{cancel}]", location = self.location)
    }
}

// ---------------------------------------------------------------------------
// ProgressReport extensions
// ---------------------------------------------------------------------------

impl ProgressReport {
    pub fn is_empty(&self) -> bool {
        self.message.is_none() && self.increment.is_none() && self.total == 0.0
    }

    pub fn has_message(&self) -> bool {
        self.message.is_some()
    }

    pub fn clamped_total(&self) -> f64 {
        self.total.clamp(0.0, 100.0)
    }
}

impl fmt::Display for ProgressReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pct = self.clamped_total();
        match &self.message {
            Some(msg) => write!(f, "{pct:.0}% - {msg}"),
            None => write!(f, "{pct:.0}%"),
        }
    }
}

// ---------------------------------------------------------------------------
// ProgressTask extensions
// ---------------------------------------------------------------------------

impl ProgressTask {
    pub fn is_complete(&self) -> bool {
        self.done
    }

    pub fn is_active(&self) -> bool {
        !self.done && !self.cancelled
    }

    pub fn percentage(&self) -> f64 {
        self.report.clamped_total()
    }
}

// ---------------------------------------------------------------------------
// ProgressService extensions — completed_count, find_by_title, iter
// ---------------------------------------------------------------------------

impl ProgressService {
    pub fn completed_count(&self) -> usize {
        self.tasks.iter().filter(|t| t.done).count()
    }

    pub fn find_by_title(&self, title: &str) -> Option<&ProgressTask> {
        self.tasks
            .iter()
            .find(|t| t.options.title.as_deref() == Some(title))
    }

    pub fn iter(&self) -> std::slice::Iter<'_, ProgressTask> {
        self.tasks.iter()
    }

    pub fn cancelled_count(&self) -> usize {
        self.tasks.iter().filter(|t| t.cancelled).count()
    }
}

impl<'a> IntoIterator for &'a ProgressService {
    type Item = &'a ProgressTask;
    type IntoIter = std::slice::Iter<'a, ProgressTask>;

    fn into_iter(self) -> Self::IntoIter {
        self.tasks.iter()
    }
}

// ---------------------------------------------------------------------------
// ProgressReportBatch extensions
// ---------------------------------------------------------------------------

impl ProgressReportBatch {
    pub fn total_increment(&self) -> f64 {
        self.reports
            .iter()
            .filter_map(|r| r.increment)
            .sum()
    }

    pub fn message_count(&self) -> usize {
        self.reports.iter().filter(|r| r.message.is_some()).count()
    }

    pub fn merge(&mut self, other: &mut ProgressReportBatch) {
        self.reports.append(&mut other.reports);
    }

    pub fn task_ids(&self) -> Vec<u64> {
        let mut ids: Vec<u64> = self.reports.iter().map(|r| r.task_id).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}

// ---------------------------------------------------------------------------
// WbProgressStats extensions
// ---------------------------------------------------------------------------

impl WbProgressStats {
    pub fn summary(&self) -> String {
        format!(
            "{total} ops ({ok} ok, {err} err) avg {avg}ns",
            total = self.total_operations,
            ok = self.successful_operations,
            err = self.failed_operations,
            avg = self.average_time_ns(),
        )
    }

    pub fn has_failures(&self) -> bool {
        self.failed_operations > 0
    }

    pub fn is_empty(&self) -> bool {
        self.total_operations == 0
    }
}

// ---------------------------------------------------------------------------
// ProgressTimer extensions
// ---------------------------------------------------------------------------

impl ProgressTimer {
    pub fn is_running(&self) -> bool {
        !self.is_paused()
    }

    pub fn elapsed_secs(&self) -> f64 {
        self.elapsed().as_secs_f64()
    }

    pub fn restart(&mut self) {
        self.start = std::time::Instant::now();
        self.paused_elapsed = None;
    }
}

// ---------------------------------------------------------------------------
// ThroughputTracker — items-per-second calculation
// ---------------------------------------------------------------------------

/// Tracks throughput (items processed per unit time) for progress reporting.
#[derive(Debug, Clone)]
pub struct ThroughputTracker {
    items_processed: u64,
    start: std::time::Instant,
    window: Vec<(std::time::Instant, u64)>,
    window_duration: std::time::Duration,
}

impl ThroughputTracker {
    /// Create a new tracker with a sliding window duration for rate calculation.
    pub fn new(window_duration: std::time::Duration) -> Self {
        Self {
            items_processed: 0,
            start: std::time::Instant::now(),
            window: Vec::new(),
            window_duration,
        }
    }

    /// Record that `count` items were processed at this instant.
    pub fn record(&mut self, count: u64) {
        let now = std::time::Instant::now();
        self.items_processed += count;
        self.window.push((now, count));
        self.prune(now);
    }

    /// Total items processed since creation.
    pub fn total_items(&self) -> u64 {
        self.items_processed
    }

    /// Overall throughput (items/second) since creation.
    pub fn overall_rate(&self) -> f64 {
        let elapsed = self.start.elapsed().as_secs_f64();
        if elapsed <= 0.0 {
            return 0.0;
        }
        self.items_processed as f64 / elapsed
    }

    /// Windowed throughput (items/second) over the configured window.
    pub fn windowed_rate(&mut self) -> f64 {
        let now = std::time::Instant::now();
        self.prune(now);
        let window_items: u64 = self.window.iter().map(|(_, c)| c).sum();
        let secs = self.window_duration.as_secs_f64();
        if secs <= 0.0 {
            return 0.0;
        }
        window_items as f64 / secs
    }

    fn prune(&mut self, now: std::time::Instant) {
        let cutoff = now - self.window_duration;
        self.window.retain(|(t, _)| *t >= cutoff);
    }

    /// Estimate time remaining given `remaining_items` based on overall rate.
    pub fn estimate_remaining(&self, remaining_items: u64) -> Option<std::time::Duration> {
        let rate = self.overall_rate();
        if rate <= 0.0 {
            return None;
        }
        Some(std::time::Duration::from_secs_f64(
            remaining_items as f64 / rate,
        ))
    }
}

impl fmt::Display for ThroughputTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} items ({:.1} items/s)",
            self.items_processed,
            self.overall_rate()
        )
    }
}

// ---------------------------------------------------------------------------
// WeightedStepProgress — multi-step progress with per-step weights
// ---------------------------------------------------------------------------

/// A single step in a weighted multi-step progress tracker.
#[derive(Debug, Clone)]
pub struct WeightedStep {
    pub label: String,
    pub weight: f64,
    pub progress: f64,
}

/// Tracks progress across multiple weighted steps, producing a single
/// aggregate percentage.
#[derive(Debug, Clone)]
pub struct WeightedStepProgress {
    steps: Vec<WeightedStep>,
}

impl WeightedStepProgress {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// Add a step with the given label and relative weight.
    pub fn add_step(&mut self, label: impl Into<String>, weight: f64) -> usize {
        let idx = self.steps.len();
        self.steps.push(WeightedStep {
            label: label.into(),
            weight,
            progress: 0.0,
        });
        idx
    }

    /// Update progress (0.0–100.0) for a step by index.
    pub fn set_step_progress(&mut self, index: usize, progress: f64) {
        if let Some(step) = self.steps.get_mut(index) {
            step.progress = progress.clamp(0.0, 100.0);
        }
    }

    /// Overall weighted percentage (0.0–100.0).
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

    /// Number of steps.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Get a step by index.
    pub fn get_step(&self, index: usize) -> Option<&WeightedStep> {
        self.steps.get(index)
    }

    /// Returns true when every step is at 100%.
    pub fn is_complete(&self) -> bool {
        !self.steps.is_empty() && self.steps.iter().all(|s| (s.progress - 100.0).abs() < f64::EPSILON)
    }

    /// Returns the index of the current (first non-100%) step, if any.
    pub fn current_step_index(&self) -> Option<usize> {
        self.steps.iter().position(|s| s.progress < 100.0)
    }

    /// Human-readable summary: "Step 2/3: Linking (45%)"
    pub fn summary(&self) -> String {
        let total = self.steps.len();
        match self.current_step_index() {
            Some(idx) => {
                let step = &self.steps[idx];
                format!(
                    "Step {}/{}: {} ({:.0}%)",
                    idx + 1,
                    total,
                    step.label,
                    step.progress
                )
            }
            None if total > 0 => "All steps complete".to_string(),
            None => "No steps".to_string(),
        }
    }
}

impl Default for WeightedStepProgress {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WeightedStepProgress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.0}% — {}", self.overall_progress(), self.summary())
    }
}

// ---------------------------------------------------------------------------
// ProgressAggregator
// ---------------------------------------------------------------------------

/// A single source contributing to an aggregated progress value.
#[derive(Debug, Clone)]
struct AggregatorSource {
    weight: f64,
    progress: f64,
}

/// Merges multiple weighted progress sources into a single overall progress.
#[derive(Debug, Clone)]
pub struct ProgressAggregator {
    sources: Vec<(String, AggregatorSource)>,
}

impl ProgressAggregator {
    /// Creates an empty aggregator with no sources.
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    /// Adds a named source with the given weight.
    ///
    /// The weight is relative; it does not need to sum to any particular value.
    pub fn add_source(&mut self, id: &str, weight: f64) {
        let w = if weight < 0.0 { 0.0 } else { weight };
        self.sources.push((
            id.to_string(),
            AggregatorSource {
                weight: w,
                progress: 0.0,
            },
        ));
    }

    /// Updates the progress (0–100) of the source identified by `id`.
    ///
    /// If the source does not exist the call is silently ignored.
    pub fn update_source(&mut self, id: &str, progress: f64) {
        let clamped = progress.clamp(0.0, 100.0);
        if let Some((_name, src)) = self.sources.iter_mut().find(|(n, _)| n == id) {
            src.progress = clamped;
        }
    }

    /// Returns the weighted overall progress in the range 0–100.
    ///
    /// Returns 0.0 when there are no sources or when total weight is zero.
    pub fn overall_progress(&self) -> f64 {
        let total_weight: f64 = self.sources.iter().map(|(_, s)| s.weight).sum();
        if total_weight <= 0.0 {
            return 0.0;
        }
        let weighted_sum: f64 = self
            .sources
            .iter()
            .map(|(_, s)| s.weight * s.progress)
            .sum();
        weighted_sum / total_weight
    }

    /// Returns `true` when every source has reached 100 %.
    pub fn all_complete(&self) -> bool {
        !self.sources.is_empty()
            && self
                .sources
                .iter()
                .all(|(_, s)| (s.progress - 100.0).abs() < f64::EPSILON)
    }

    /// Returns the number of sources whose progress is below 100 %.
    pub fn active_source_count(&self) -> usize {
        self.sources
            .iter()
            .filter(|(_, s)| (s.progress - 100.0).abs() >= f64::EPSILON)
            .count()
    }

    /// Returns a human-readable summary of the aggregated progress.
    pub fn summary(&self) -> String {
        if self.sources.is_empty() {
            return "No sources".to_string();
        }
        format!(
            "{:.1}% ({} source(s), {} active)",
            self.overall_progress(),
            self.sources.len(),
            self.active_source_count(),
        )
    }
}

impl Default for ProgressAggregator {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ProgressAggregator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

// ---------------------------------------------------------------------------
// ProgressRateEstimator
// ---------------------------------------------------------------------------

/// A single sample recording progress at a point in time.
#[derive(Debug, Clone, Copy)]
struct RateSample {
    progress: f64,
    elapsed_ms: u64,
}

/// Estimates the rate of progress and projected time to completion.
#[derive(Debug, Clone)]
pub struct ProgressRateEstimator {
    samples: Vec<RateSample>,
}

impl ProgressRateEstimator {
    /// Creates an estimator with no recorded samples.
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
        }
    }

    /// Records a progress sample at the given elapsed time (in milliseconds).
    ///
    /// Progress values are clamped to 0–100 and elapsed time must be
    /// monotonically increasing; samples that violate this are ignored.
    pub fn record_sample(&mut self, progress: f64, elapsed_ms: u64) {
        let clamped = progress.clamp(0.0, 100.0);
        if let Some(last) = self.samples.last() {
            if elapsed_ms <= last.elapsed_ms {
                return;
            }
        }
        self.samples.push(RateSample {
            progress: clamped,
            elapsed_ms,
        });
    }

    /// Returns the average rate of progress per second (progress‑units / s).
    ///
    /// Returns 0.0 when fewer than two samples are available.
    pub fn rate_per_second(&self) -> f64 {
        if self.samples.len() < 2 {
            return 0.0;
        }
        let first = &self.samples[0];
        let last = &self.samples[self.samples.len() - 1];
        let dt_ms = last.elapsed_ms.saturating_sub(first.elapsed_ms);
        if dt_ms == 0 {
            return 0.0;
        }
        let dp = last.progress - first.progress;
        (dp / dt_ms as f64) * 1000.0
    }

    /// Estimates the number of milliseconds remaining until progress reaches
    /// 100 %, based on the current rate.  Returns `None` when an estimate
    /// cannot be computed (e.g., rate is zero or progress already complete).
    pub fn estimated_remaining_ms(&self) -> Option<u64> {
        let rate = self.rate_per_second();
        if rate <= 0.0 {
            return None;
        }
        let last = self.samples.last()?;
        let remaining = 100.0 - last.progress;
        if remaining <= 0.0 {
            return Some(0);
        }
        let secs = remaining / rate;
        Some((secs * 1000.0) as u64)
    }

    /// Returns the number of samples recorded so far.
    pub fn samples_count(&self) -> usize {
        self.samples.len()
    }
}

impl Default for ProgressRateEstimator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ProgressHistory
// ---------------------------------------------------------------------------

/// Record of a single completed progress task.
#[derive(Debug, Clone)]
pub struct ProgressCompletionRecord {
    /// Identifier of the completed task.
    pub task_id: u64,
    /// Human‑readable label describing the task.
    pub label: String,
    /// Wall‑clock duration in milliseconds.
    pub duration_ms: u64,
}

/// Maintains a log of completed progress tasks for statistical queries.
#[derive(Debug, Clone)]
pub struct ProgressHistory {
    records: Vec<ProgressCompletionRecord>,
}

impl ProgressHistory {
    /// Creates an empty history.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Records the completion of a task.
    pub fn record_completion(&mut self, task_id: u64, label: &str, duration_ms: u64) {
        self.records.push(ProgressCompletionRecord {
            task_id,
            label: label.to_string(),
            duration_ms,
        });
    }

    /// Returns the total number of completed tasks.
    pub fn total_completed(&self) -> usize {
        self.records.len()
    }

    /// Returns the average duration across all recorded tasks, or `None` if
    /// no tasks have been recorded.
    pub fn average_duration_ms(&self) -> Option<u64> {
        if self.records.is_empty() {
            return None;
        }
        let total: u64 = self.records.iter().map(|r| r.duration_ms).sum();
        Some(total / self.records.len() as u64)
    }

    /// Returns the label and duration of the longest‑running task, or `None`
    /// if the history is empty.
    pub fn longest_task(&self) -> Option<(&str, u64)> {
        self.records
            .iter()
            .max_by_key(|r| r.duration_ms)
            .map(|r| (r.label.as_str(), r.duration_ms))
    }

    /// Returns references to the `n` most recent completion records.
    pub fn recent(&self, n: usize) -> Vec<&ProgressCompletionRecord> {
        let start = self.records.len().saturating_sub(n);
        self.records[start..].iter().collect()
    }
}

impl Default for ProgressHistory {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CancellationChain
// ---------------------------------------------------------------------------

/// Propagates cancellation from a parent context to a set of child task ids.
#[derive(Debug, Clone)]
pub struct CancellationChain {
    cancelled: bool,
    children: Vec<u64>,
}

impl CancellationChain {
    /// Creates a new, non‑cancelled chain with no children.
    pub fn new() -> Self {
        Self {
            cancelled: false,
            children: Vec::new(),
        }
    }

    /// Registers a child task id to be notified on cancellation.
    pub fn add_child(&mut self, child_id: u64) {
        if !self.children.contains(&child_id) {
            self.children.push(child_id);
        }
    }

    /// Marks this chain (and all children) as cancelled.
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    /// Returns `true` if `cancel` has been called.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    /// Returns the list of child task ids that are part of this chain.
    pub fn cancelled_children(&self) -> &[u64] {
        if self.cancelled {
            &self.children
        } else {
            &[]
        }
    }
}

impl Default for CancellationChain {
    fn default() -> Self {
        Self::new()
    }
}


// === Progress Estimation Algorithm ===

/// Progress Estimation Algorithm implementation.
#[derive(Debug, Clone)]
pub struct ProgressEstimationAlgorithm {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: ProgressEstimationAlgorithmStats,
}

/// Statistics for ProgressEstimationAlgorithm.
#[derive(Debug, Clone, Default)]
pub struct ProgressEstimationAlgorithmStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl ProgressEstimationAlgorithmStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl ProgressEstimationAlgorithm {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: ProgressEstimationAlgorithmStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &ProgressEstimationAlgorithmStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for ProgressEstimationAlgorithm {
    fn default() -> Self {
        Self::new()
    }
}

// === Progress Widget Layout ===

/// Priority level for ProgressWidgetLayout items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProgressWidgetLayoutPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl ProgressWidgetLayoutPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for ProgressWidgetLayoutPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Progress Widget Layout implementation.
#[derive(Debug, Clone)]
pub struct ProgressWidgetLayout {
    items: Vec<ProgressWidgetLayoutItem>,
    max_items: usize,
    default_priority: ProgressWidgetLayoutPriority,
}

/// A single item in ProgressWidgetLayout.
#[derive(Debug, Clone)]
pub struct ProgressWidgetLayoutItem {
    pub id: String,
    pub label: String,
    pub priority: ProgressWidgetLayoutPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl ProgressWidgetLayoutItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: ProgressWidgetLayoutPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: ProgressWidgetLayoutPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl ProgressWidgetLayout {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: ProgressWidgetLayoutPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: ProgressWidgetLayoutItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<ProgressWidgetLayoutItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&ProgressWidgetLayoutItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: ProgressWidgetLayoutPriority) -> Vec<&ProgressWidgetLayoutItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ProgressWidgetLayoutItem> {
        let mut sorted: Vec<&ProgressWidgetLayoutItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&ProgressWidgetLayoutItem> {
        let mut sorted: Vec<&ProgressWidgetLayoutItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&ProgressWidgetLayoutItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: ProgressWidgetLayoutPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> ProgressWidgetLayoutPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &ProgressWidgetLayoutItem> {
        self.items.iter()
    }
}

impl Default for ProgressWidgetLayout {
    fn default() -> Self {
        Self::new()
    }
}


/// Workbench progress configuration manager.
#[derive(Debug, Clone)]
pub struct WbProgressConfig {
    entries: Vec<WbProgressEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single workbench progress entry.
#[derive(Debug, Clone, PartialEq)]
pub struct WbProgressEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl WbProgressEntry {
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

impl WbProgressConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: WbProgressEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&WbProgressEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut WbProgressEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&WbProgressEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&WbProgressEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&WbProgressEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<WbProgressEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Progress notification handling — extended utilities (yu)
// ---------------------------------------------------------------------------

/// Metric accumulator for wb_prog operations.
#[derive(Debug, Clone)]
pub struct YuMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YuMetrics {
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

/// Sliding-window rate counter for wb_prog.
#[derive(Debug, Clone)]
pub struct YuRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YuRateWindow {
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

/// A small LRU-style cache for wb_prog lookups.
#[derive(Debug, Clone)]
pub struct YuLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YuLruCache {
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
// xa_ extended helpers for wb_progress
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWbProgressRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWbProgressRingBuf {
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
pub struct XaWbProgressCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWbProgressCounter {
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

impl Default for XaWbProgressCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 222
// ---------------------------------------------------------------------------

/// Generic object pool `Xc222Pool<T>`.
pub struct Xc222Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc222Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc222PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc222Pool<T> {
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
    pub fn stats(&self) -> Xc222PoolStats {
        Xc222PoolStats {
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

impl<T> Default for Xc222Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc222Scheduler`.
pub struct Xc222Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc222Scheduler {
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

impl Default for Xc222Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_222 hash for the given byte slice.
pub fn xc_222_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_222 convention.
pub fn xc_222_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_106 deepening: state machine + event bus ---

/// States for the Xd106 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd106State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd106State {
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
pub struct Xd106Transition {
    pub from: Xd106State,
    pub to: Xd106State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd106StateMachine {
    current: Xd106State,
    history: Vec<Xd106Transition>,
    step_counter: usize,
}

impl Xd106StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd106State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd106State {
        self.current
    }

    pub fn history(&self) -> &[Xd106Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd106State) -> Result<Xd106State, String> {
        let allowed = match (self.current, target) {
            (Xd106State::Idle, Xd106State::Running) => true,
            (Xd106State::Running, Xd106State::Paused) => true,
            (Xd106State::Running, Xd106State::Done) => true,
            (Xd106State::Paused, Xd106State::Running) => true,
            (Xd106State::Paused, Xd106State::Done) => true,
            (Xd106State::Done, Xd106State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_106: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd106Transition {
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
            "Xd106SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd106State> {
        let prefix = "Xd106SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd106State::Idle),
            "Running" => Some(Xd106State::Running),
            "Paused" => Some(Xd106State::Paused),
            "Done" => Some(Xd106State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd106State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd106 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd106Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd106Event {
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

type Xd106HandlerFn = Box<dyn Fn(&Xd106Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd106EventBus {
    handlers: Vec<(usize, Option<String>, Xd106HandlerFn)>,
    next_id: usize,
    published: Vec<Xd106Event>,
}

impl Xd106EventBus {
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
        F: Fn(&Xd106Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd106Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd106Event) {
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

    pub fn published_events(&self) -> &[Xd106Event] {
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
// xg_30: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg30Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg30Graph {
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

impl Default for Xg30Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_30: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg30Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg30Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg30Heap<T>) {
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

impl<T: Ord> Default for Xg30Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 221).
pub struct Xh221SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh221SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 263 as u64,
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

/// A compact bit set supporting boolean operations (variant 221).
pub struct Xh221BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh221BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 221).
pub struct Xi221Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi221Deque<T> {
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
pub struct Xi221Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi221Interval {
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

/// A simple interval tree (variant 221).
pub struct Xi221IntervalTree {
    xi_intervals: Vec<Xi221Interval>,
}

impl Xi221IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi221Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi221Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi221Interval) -> Vec<&Xi221Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi221Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi221Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi221Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi221Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi221Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi221Interval> = Vec::new();
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

    fn opts(loc: ProgressLocation) -> ProgressOptions {
        ProgressOptions {
            location: loc,
            title: Some("Test".to_string()),
            cancellable: true,
        }
    }

    #[test]
    fn start_and_complete() {
        let mut svc = ProgressService::new();
        let id = svc.start(opts(ProgressLocation::Notification));
        assert_eq!(svc.active_count(), 1);
        svc.complete(id);
        assert_eq!(svc.active_count(), 0);
        assert!(svc.get_task(id).unwrap().done);
    }

    #[test]
    fn report_progress() {
        let mut svc = ProgressService::new();
        let id = svc.start(opts(ProgressLocation::StatusBar));
        svc.report(id, Some("step 1".to_string()), Some(25.0));
        svc.report(id, Some("step 2".to_string()), Some(25.0));
        let task = svc.get_task(id).unwrap();
        assert_eq!(task.report.total, 50.0);
        assert_eq!(task.report.message.as_deref(), Some("step 2"));
    }

    #[test]
    fn cancel_task() {
        let mut svc = ProgressService::new();
        let id = svc.start(opts(ProgressLocation::Window));
        assert!(!svc.is_cancelled(id));
        svc.cancel(id);
        assert!(svc.is_cancelled(id));
        assert_eq!(svc.active_count(), 0);
    }

    #[test]
    fn try_cancel_success() {
        let mut svc = ProgressService::new();
        let id = svc.start(opts(ProgressLocation::Notification));
        assert!(svc.try_cancel(id).is_ok());
        assert!(svc.is_cancelled(id));
    }

    #[test]
    fn try_cancel_not_cancellable() {
        let mut svc = ProgressService::new();
        let id = svc.start(ProgressOptions {
            location: ProgressLocation::Window,
            title: Some("NC".to_string()),
            cancellable: false,
        });
        assert_eq!(svc.try_cancel(id), Err(ProgressError::NotCancellable(id)));
    }

    #[test]
    fn try_cancel_already_complete() {
        let mut svc = ProgressService::new();
        let id = svc.start(opts(ProgressLocation::StatusBar));
        svc.complete(id);
        assert_eq!(svc.try_cancel(id), Err(ProgressError::AlreadyComplete(id)));
    }

    #[test]
    fn try_cancel_task_not_found() {
        let mut svc = ProgressService::new();
        assert_eq!(svc.try_cancel(999), Err(ProgressError::TaskNotFound(999)));
    }

    #[test]
    fn get_active_tasks() {
        let mut svc = ProgressService::new();
        let id1 = svc.start(opts(ProgressLocation::Notification));
        let _id2 = svc.start(opts(ProgressLocation::Window));
        svc.complete(id1);
        let active = svc.get_active_tasks();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, _id2);
    }

    #[test]
    fn remove_completed_tasks() {
        let mut svc = ProgressService::new();
        let id1 = svc.start(opts(ProgressLocation::Notification));
        let _id2 = svc.start(opts(ProgressLocation::Window));
        svc.complete(id1);
        let removed = svc.remove_completed();
        assert_eq!(removed, 1);
        assert_eq!(svc.total_task_count(), 1);
    }

    #[test]
    fn percentage_calculation() {
        let mut svc = ProgressService::new();
        let id = svc.start(opts(ProgressLocation::StatusBar));
        assert_eq!(svc.percentage(id), Some(0.0));
        svc.report(id, None, Some(40.0));
        assert_eq!(svc.percentage(id), Some(40.0));
        svc.report(id, None, Some(80.0));
        // Clamped to 100
        assert_eq!(svc.percentage(id), Some(100.0));
    }

    #[test]
    fn percentage_not_found() {
        let svc = ProgressService::new();
        assert_eq!(svc.percentage(42), None);
    }

    #[test]
    fn is_indeterminate() {
        let mut svc = ProgressService::new();
        let id = svc.start(opts(ProgressLocation::Notification));
        assert!(svc.get_task(id).unwrap().is_indeterminate());
        svc.report(id, Some("working".to_string()), Some(10.0));
        assert!(!svc.get_task(id).unwrap().is_indeterminate());
    }

    #[test]
    fn progress_options_builder() {
        let opts = ProgressOptionsBuilder::new(ProgressLocation::SourceControl)
            .title("Building")
            .cancellable(true)
            .build();
        assert_eq!(opts.location, ProgressLocation::SourceControl);
        assert_eq!(opts.title.as_deref(), Some("Building"));
        assert!(opts.cancellable);
    }

    #[test]
    fn progress_options_builder_defaults() {
        let opts = ProgressOptionsBuilder::new(ProgressLocation::Notification).build();
        assert_eq!(opts.location, ProgressLocation::Notification);
        assert!(opts.title.is_none());
        assert!(!opts.cancellable);
    }

    #[test]
    fn display_progress_location() {
        assert_eq!(ProgressLocation::Notification.to_string(), "Notification");
        assert_eq!(ProgressLocation::SourceControl.to_string(), "Source Control");
        assert_eq!(ProgressLocation::StatusBar.to_string(), "Status Bar");
        assert_eq!(ProgressLocation::Window.to_string(), "Window");
    }

    #[test]
    fn display_progress_task() {
        let mut svc = ProgressService::new();
        let id = svc.start(opts(ProgressLocation::Window));
        assert_eq!(svc.get_task(id).unwrap().to_string(), "[Test] 0%");
        svc.report(id, None, Some(75.0));
        assert_eq!(svc.get_task(id).unwrap().to_string(), "[Test] 75%");
    }

    #[test]
    fn display_progress_error() {
        assert_eq!(ProgressError::TaskNotFound(1).to_string(), "task 1 not found");
        assert_eq!(
            ProgressError::AlreadyComplete(2).to_string(),
            "task 2 is already complete"
        );
        assert_eq!(
            ProgressError::NotCancellable(3).to_string(),
            "task 3 is not cancellable"
        );
    }

    #[test]
    fn total_task_count() {
        let mut svc = ProgressService::new();
        assert_eq!(svc.total_task_count(), 0);
        let id1 = svc.start(opts(ProgressLocation::Notification));
        let _id2 = svc.start(opts(ProgressLocation::Window));
        assert_eq!(svc.total_task_count(), 2);
        svc.complete(id1);
        assert_eq!(svc.total_task_count(), 2);
    }

    #[test]
    fn eq_progresslocation_same() {
        assert_eq!(ProgressLocation::Notification, ProgressLocation::Notification);
    }

    #[test]
    fn ne_progresslocation_diff() {
        assert_ne!(ProgressLocation::Notification, ProgressLocation::Window);
    }

    #[test]
    fn display_progresslocation_variants() {
        assert!(!ProgressLocation::Notification.to_string().is_empty());
        assert!(!ProgressLocation::Window.to_string().is_empty());
        assert!(!ProgressLocation::SourceControl.to_string().is_empty());
        assert!(!ProgressLocation::StatusBar.to_string().is_empty());
    }

    // -- ProgressLocation Panel/Editor tests --

    #[test]
    fn display_progress_location_panel_editor() {
        assert_eq!(ProgressLocation::Panel.to_string(), "Panel");
        assert_eq!(ProgressLocation::Editor.to_string(), "Editor");
    }

    #[test]
    fn eq_progresslocation_panel() {
        assert_eq!(ProgressLocation::Panel, ProgressLocation::Panel);
        assert_ne!(ProgressLocation::Panel, ProgressLocation::Editor);
    }

    // -- ProgressReportBatch tests --

    #[test]
    fn batch_empty() {
        let batch = ProgressReportBatch::new();
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);
    }

    #[test]
    fn batch_add_and_apply() {
        let mut svc = ProgressService::new();
        let id = svc.start(opts(ProgressLocation::Notification));
        let mut batch = ProgressReportBatch::new();
        batch.add(id, Some("step 1".into()), Some(25.0));
        batch.add(id, Some("step 2".into()), Some(25.0));
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
        batch.apply(&mut svc);
        let task = svc.get_task(id).unwrap();
        assert_eq!(task.report.total, 50.0);
        assert_eq!(task.report.message.as_deref(), Some("step 2"));
    }

    #[test]
    fn batch_clear() {
        let mut batch = ProgressReportBatch::new();
        batch.add(1, None, Some(10.0));
        batch.clear();
        assert!(batch.is_empty());
    }

    // -- estimate_remaining_ms tests --

    #[test]
    fn estimate_remaining_at_half() {
        let result = estimate_remaining_ms(0.5, 1000);
        assert_eq!(result, Some(1000));
    }

    #[test]
    fn estimate_remaining_edge_cases() {
        assert_eq!(estimate_remaining_ms(0.0, 1000), None);
        assert_eq!(estimate_remaining_ms(1.0, 1000), None);
        assert_eq!(estimate_remaining_ms(-0.5, 1000), None);
    }

    // -- ProgressService extension tests --

    #[test]
    fn get_tasks_by_location() {
        let mut svc = ProgressService::new();
        svc.start(opts(ProgressLocation::Notification));
        svc.start(opts(ProgressLocation::Window));
        svc.start(opts(ProgressLocation::Notification));
        let notif_tasks = svc.get_tasks_by_location(ProgressLocation::Notification);
        assert_eq!(notif_tasks.len(), 2);
        let window_tasks = svc.get_tasks_by_location(ProgressLocation::Window);
        assert_eq!(window_tasks.len(), 1);
    }

    #[test]
    fn cancel_all_tasks() {
        let mut svc = ProgressService::new();
        svc.start(opts(ProgressLocation::Notification));
        svc.start(opts(ProgressLocation::Window));
        let id3 = svc.start(opts(ProgressLocation::StatusBar));
        svc.complete(id3);
        let cancelled = svc.cancel_all();
        assert_eq!(cancelled, 2);
        assert_eq!(svc.active_count(), 0);
    }

    #[test]
    fn overall_percentage_average() {
        let mut svc = ProgressService::new();
        let id1 = svc.start(opts(ProgressLocation::Notification));
        let id2 = svc.start(opts(ProgressLocation::Window));
        svc.report(id1, None, Some(50.0));
        svc.report(id2, None, Some(100.0));
        assert!((svc.overall_percentage() - 75.0).abs() < f64::EPSILON);
    }

    #[test]
    fn overall_percentage_no_active() {
        let svc = ProgressService::new();
        assert!((svc.overall_percentage() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_progress_stats_new_defaults() {
        let stats = WbProgressStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_progress_stats_record_success() {
        let mut stats = WbProgressStats::new();
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
    fn wb_progress_stats_record_failure() {
        let mut stats = WbProgressStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_progress_stats_reset() {
        let mut stats = WbProgressStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_progress_stats_merge() {
        let mut a = WbProgressStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbProgressStats::new();
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
    fn wb_progress_stats_display() {
        let mut stats = WbProgressStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_progress_stats_default() {
        let stats = WbProgressStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_progress_validator_accepts_valid_name() {
        let v = WbProgressValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_progress_validator_rejects_empty() {
        let v = WbProgressValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_progress_validator_rejects_too_long() {
        let v = WbProgressValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_progress_validator_forbidden_prefix() {
        let v = WbProgressValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_progress_validator_allowed_chars() {
        let v = WbProgressValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_progress_validator_range() {
        let v = WbProgressValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_progress_sanitize_removes_control() {
        let result = WbProgressValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_progress_truncate_short_string() {
        assert_eq!(WbProgressValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_progress_truncate_long_string() {
        let result = WbProgressValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_progress_is_ascii_printable() {
        assert!(WbProgressValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbProgressValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn progress_timer_label() {
        let timer = ProgressTimer::start("Building");
        assert_eq!(timer.label(), "Building");
    }

    #[test]
    fn progress_timer_elapsed_increases() {
        let timer = ProgressTimer::start("test");
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(timer.elapsed().as_millis() >= 5);
    }

    #[test]
    fn progress_timer_pause_resume() {
        let mut timer = ProgressTimer::start("test");
        std::thread::sleep(std::time::Duration::from_millis(10));
        timer.pause();
        assert!(timer.is_paused());
        let paused_elapsed = timer.elapsed();
        std::thread::sleep(std::time::Duration::from_millis(20));
        // Elapsed should not change while paused
        assert_eq!(timer.elapsed(), paused_elapsed);
        timer.resume();
        assert!(!timer.is_paused());
    }

    #[test]
    fn format_duration_ms() {
        let d = std::time::Duration::from_millis(500);
        assert_eq!(format_duration(d), "500ms");
    }

    #[test]
    fn format_duration_minutes() {
        let d = std::time::Duration::from_secs(125);
        assert_eq!(format_duration(d), "2m 5s");
    }

    #[test]
    fn format_duration_hours() {
        let d = std::time::Duration::from_secs(3723);
        assert_eq!(format_duration(d), "1h 2m 3s");
    }

    #[test]
    fn estimate_remaining_half() {
        let elapsed = std::time::Duration::from_secs(10);
        let remaining = estimate_remaining(0.5, elapsed).unwrap();
        assert!((remaining.as_secs_f64() - 10.0).abs() < 0.1);
    }

    #[test]
    fn estimate_remaining_invalid() {
        let elapsed = std::time::Duration::from_secs(10);
        assert!(estimate_remaining(0.0, elapsed).is_none());
        assert!(estimate_remaining(-0.5, elapsed).is_none());
    }

    // -- ProgressLocation extension tests --

    #[test]
    fn progress_location_predicates() {
        assert!(ProgressLocation::Notification.is_notification());
        assert!(!ProgressLocation::Window.is_notification());
        assert!(ProgressLocation::StatusBar.is_status_bar());
        assert!(!ProgressLocation::Panel.is_status_bar());
        assert!(ProgressLocation::Window.is_window());
        assert!(ProgressLocation::Notification.is_overlay());
        assert!(ProgressLocation::Window.is_overlay());
        assert!(!ProgressLocation::Panel.is_overlay());
    }

    // -- ProgressOptions extension tests --

    #[test]
    fn progress_options_extensions() {
        let o = ProgressOptions {
            location: ProgressLocation::Notification,
            title: Some("Build".to_string()),
            cancellable: true,
        };
        assert!(o.is_cancellable());
        assert!(o.has_title());
        let s = o.summary();
        assert!(s.contains("Build"));
        assert!(s.contains("cancellable"));

        let o2 = ProgressOptions {
            location: ProgressLocation::Window,
            title: None,
            cancellable: false,
        };
        assert!(!o2.is_cancellable());
        assert!(!o2.has_title());
        assert!(o2.summary().contains("(no title)"));
        assert!(o2.summary().contains("non-cancellable"));
    }

    // -- ProgressReport extension tests --

    #[test]
    fn progress_report_extensions() {
        let empty = ProgressReport {
            message: None,
            increment: None,
            total: 0.0,
        };
        assert!(empty.is_empty());
        assert!(!empty.has_message());
        assert_eq!(empty.clamped_total(), 0.0);
        assert_eq!(empty.to_string(), "0%");

        let with_msg = ProgressReport {
            message: Some("building".to_string()),
            increment: Some(50.0),
            total: 150.0,
        };
        assert!(!with_msg.is_empty());
        assert!(with_msg.has_message());
        assert_eq!(with_msg.clamped_total(), 100.0);
        assert_eq!(with_msg.to_string(), "100% - building");
    }

    // -- ProgressTask extension tests --

    #[test]
    fn progress_task_extensions() {
        let mut svc = ProgressService::new();
        let id = svc.start(opts(ProgressLocation::Notification));
        assert!(!svc.get_task(id).unwrap().is_complete());
        assert!(svc.get_task(id).unwrap().is_active());
        assert_eq!(svc.get_task(id).unwrap().percentage(), 0.0);

        svc.report(id, None, Some(60.0));
        assert_eq!(svc.get_task(id).unwrap().percentage(), 60.0);

        svc.complete(id);
        assert!(svc.get_task(id).unwrap().is_complete());
        assert!(!svc.get_task(id).unwrap().is_active());
    }

    // -- ProgressService extension tests --

    #[test]
    fn progress_service_completed_and_cancelled_count() {
        let mut svc = ProgressService::new();
        let id1 = svc.start(opts(ProgressLocation::Notification));
        let id2 = svc.start(opts(ProgressLocation::Window));
        let _id3 = svc.start(opts(ProgressLocation::StatusBar));
        svc.complete(id1);
        svc.cancel(id2);
        assert_eq!(svc.completed_count(), 2);
        assert_eq!(svc.cancelled_count(), 1);
    }

    #[test]
    fn progress_service_find_by_title() {
        let mut svc = ProgressService::new();
        svc.start(ProgressOptions {
            location: ProgressLocation::Notification,
            title: Some("Alpha".to_string()),
            cancellable: false,
        });
        svc.start(ProgressOptions {
            location: ProgressLocation::Window,
            title: Some("Beta".to_string()),
            cancellable: false,
        });
        assert_eq!(svc.find_by_title("Alpha").unwrap().options.title.as_deref(), Some("Alpha"));
        assert_eq!(svc.find_by_title("Beta").unwrap().options.location, ProgressLocation::Window);
        assert!(svc.find_by_title("Gamma").is_none());
    }

    #[test]
    fn progress_service_iter_and_into_iter() {
        let mut svc = ProgressService::new();
        svc.start(opts(ProgressLocation::Notification));
        svc.start(opts(ProgressLocation::Window));
        assert_eq!(svc.iter().count(), 2);
        let count = (&svc).into_iter().count();
        assert_eq!(count, 2);
    }

    // -- ProgressReportBatch extension tests --

    #[test]
    fn batch_extensions() {
        let mut batch = ProgressReportBatch::new();
        batch.add(1, Some("a".to_string()), Some(10.0));
        batch.add(2, None, Some(20.0));
        batch.add(1, Some("b".to_string()), None);
        assert_eq!(batch.total_increment(), 30.0);
        assert_eq!(batch.message_count(), 2);
        assert_eq!(batch.task_ids(), vec![1, 2]);

        let mut batch2 = ProgressReportBatch::new();
        batch2.add(3, None, Some(5.0));
        batch.merge(&mut batch2);
        assert_eq!(batch.len(), 4);
        assert!(batch2.is_empty());
    }

    // -- WbProgressStats extension tests --

    #[test]
    fn wb_progress_stats_extensions() {
        let empty = WbProgressStats::new();
        assert!(empty.is_empty());
        assert!(!empty.has_failures());
        assert!(empty.summary().contains("0 ops"));

        let mut stats = WbProgressStats::new();
        stats.record_success(100);
        stats.record_failure(200);
        assert!(!stats.is_empty());
        assert!(stats.has_failures());
        let s = stats.summary();
        assert!(s.contains("2 ops"));
        assert!(s.contains("1 ok"));
        assert!(s.contains("1 err"));
    }

    // -- ProgressTimer extension tests --

    #[test]
    fn progress_timer_extensions() {
        let mut timer = ProgressTimer::start("test");
        assert!(timer.is_running());
        assert!(timer.elapsed_secs() >= 0.0);
        timer.pause();
        assert!(!timer.is_running());
        timer.restart();
        assert!(timer.is_running());
        assert!(timer.elapsed_secs() < 1.0);
    }

    // -- ThroughputTracker tests --

    #[test]
    fn throughput_tracker_basic() {
        let mut tracker = ThroughputTracker::new(std::time::Duration::from_secs(60));
        assert_eq!(tracker.total_items(), 0);
        tracker.record(10);
        tracker.record(5);
        assert_eq!(tracker.total_items(), 15);
    }

    #[test]
    fn throughput_tracker_estimate_remaining() {
        let mut tracker = ThroughputTracker::new(std::time::Duration::from_secs(60));
        // With zero items, rate is 0 so estimate should be None
        assert!(tracker.estimate_remaining(100).is_none());
        tracker.record(50);
        std::thread::sleep(std::time::Duration::from_millis(10));
        // Now we have some rate, so estimate should be Some
        let est = tracker.estimate_remaining(50);
        assert!(est.is_some());
    }

    #[test]
    fn throughput_tracker_display() {
        let mut tracker = ThroughputTracker::new(std::time::Duration::from_secs(60));
        tracker.record(42);
        let s = format!("{tracker}");
        assert!(s.contains("42 items"));
        assert!(s.contains("items/s"));
    }

    // -- WeightedStepProgress tests --

    #[test]
    fn weighted_step_progress_overall() {
        let mut wsp = WeightedStepProgress::new();
        let compile = wsp.add_step("Compile", 3.0);
        let link = wsp.add_step("Link", 1.0);
        assert_eq!(wsp.step_count(), 2);
        assert!(!wsp.is_complete());

        // Compile 100%, Link 0% => overall = (100*3 + 0*1)/4 = 75%
        wsp.set_step_progress(compile, 100.0);
        assert!((wsp.overall_progress() - 75.0).abs() < f64::EPSILON);

        wsp.set_step_progress(link, 100.0);
        assert!(wsp.is_complete());
        assert!((wsp.overall_progress() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn weighted_step_progress_summary() {
        let mut wsp = WeightedStepProgress::new();
        wsp.add_step("Parse", 1.0);
        wsp.add_step("Codegen", 2.0);
        let s = wsp.summary();
        assert!(s.contains("Step 1/2"));
        assert!(s.contains("Parse"));

        wsp.set_step_progress(0, 100.0);
        let s2 = wsp.summary();
        assert!(s2.contains("Step 2/2"));
        assert!(s2.contains("Codegen"));

        wsp.set_step_progress(1, 100.0);
        assert_eq!(wsp.summary(), "All steps complete");
    }

    #[test]
    fn weighted_step_progress_empty() {
        let wsp = WeightedStepProgress::new();
        assert_eq!(wsp.overall_progress(), 0.0);
        assert_eq!(wsp.summary(), "No steps");
        assert!(!wsp.is_complete());
        assert!(wsp.current_step_index().is_none());
    }

    #[test]
    fn weighted_step_progress_clamping() {
        let mut wsp = WeightedStepProgress::new();
        let idx = wsp.add_step("Test", 1.0);
        wsp.set_step_progress(idx, 200.0);
        assert!((wsp.get_step(idx).unwrap().progress - 100.0).abs() < f64::EPSILON);
        wsp.set_step_progress(idx, -50.0);
        assert!((wsp.get_step(idx).unwrap().progress - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn weighted_step_progress_display() {
        let mut wsp = WeightedStepProgress::new();
        wsp.add_step("A", 1.0);
        wsp.add_step("B", 1.0);
        wsp.set_step_progress(0, 50.0);
        let s = format!("{wsp}");
        assert!(s.contains("25%"));
        assert!(s.contains("Step 1/2"));
    }

    // -- ProgressAggregator tests --

    #[test]
    fn aggregator_empty() {
        let agg = ProgressAggregator::new();
        assert!((agg.overall_progress() - 0.0).abs() < f64::EPSILON);
        assert!(!agg.all_complete());
        assert_eq!(agg.active_source_count(), 0);
        assert_eq!(agg.summary(), "No sources");
    }

    #[test]
    fn aggregator_equal_weights() {
        let mut agg = ProgressAggregator::new();
        agg.add_source("a", 1.0);
        agg.add_source("b", 1.0);
        agg.update_source("a", 50.0);
        agg.update_source("b", 100.0);
        assert!((agg.overall_progress() - 75.0).abs() < f64::EPSILON);
        assert!(!agg.all_complete());
        assert_eq!(agg.active_source_count(), 1);
    }

    #[test]
    fn aggregator_all_complete() {
        let mut agg = ProgressAggregator::new();
        agg.add_source("x", 2.0);
        agg.add_source("y", 3.0);
        agg.update_source("x", 100.0);
        agg.update_source("y", 100.0);
        assert!(agg.all_complete());
        assert_eq!(agg.active_source_count(), 0);
    }

    #[test]
    fn aggregator_display() {
        let mut agg = ProgressAggregator::new();
        agg.add_source("s", 1.0);
        agg.update_source("s", 40.0);
        let s = format!("{agg}");
        assert!(s.contains("40.0%"));
        assert!(s.contains("1 active"));
    }

    // -- ProgressRateEstimator tests --

    #[test]
    fn rate_estimator_empty() {
        let est = ProgressRateEstimator::new();
        assert!((est.rate_per_second() - 0.0).abs() < f64::EPSILON);
        assert!(est.estimated_remaining_ms().is_none());
        assert_eq!(est.samples_count(), 0);
    }

    #[test]
    fn rate_estimator_basic() {
        let mut est = ProgressRateEstimator::new();
        est.record_sample(0.0, 0);
        est.record_sample(50.0, 1000);
        // 50 units in 1 second = 50 units/s
        assert!((est.rate_per_second() - 50.0).abs() < f64::EPSILON);
        // 50 units remaining at 50/s = 1 second = 1000 ms
        assert_eq!(est.estimated_remaining_ms(), Some(1000));
        assert_eq!(est.samples_count(), 2);
    }

    #[test]
    fn rate_estimator_ignores_non_monotonic() {
        let mut est = ProgressRateEstimator::new();
        est.record_sample(0.0, 100);
        est.record_sample(10.0, 50); // earlier timestamp, should be ignored
        assert_eq!(est.samples_count(), 1);
    }

    #[test]
    fn rate_estimator_complete() {
        let mut est = ProgressRateEstimator::new();
        est.record_sample(0.0, 0);
        est.record_sample(100.0, 2000);
        assert_eq!(est.estimated_remaining_ms(), Some(0));
    }

    // -- ProgressHistory tests --

    #[test]
    fn history_empty() {
        let hist = ProgressHistory::new();
        assert_eq!(hist.total_completed(), 0);
        assert!(hist.average_duration_ms().is_none());
        assert!(hist.longest_task().is_none());
        assert!(hist.recent(5).is_empty());
    }

    #[test]
    fn history_records_and_queries() {
        let mut hist = ProgressHistory::new();
        hist.record_completion(1, "build", 300);
        hist.record_completion(2, "test", 500);
        hist.record_completion(3, "lint", 200);
        assert_eq!(hist.total_completed(), 3);
        assert_eq!(hist.average_duration_ms(), Some(333));
        assert_eq!(hist.longest_task(), Some(("test", 500)));
        let recent = hist.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].label, "test");
        assert_eq!(recent[1].label, "lint");
    }

    // -- CancellationChain tests --

    #[test]
    fn cancellation_chain_not_cancelled() {
        let mut chain = CancellationChain::new();
        chain.add_child(10);
        chain.add_child(20);
        assert!(!chain.is_cancelled());
        assert!(chain.cancelled_children().is_empty());
    }

    #[test]
    fn cancellation_chain_cancel_propagates() {
        let mut chain = CancellationChain::new();
        chain.add_child(10);
        chain.add_child(20);
        chain.cancel();
        assert!(chain.is_cancelled());
        assert_eq!(chain.cancelled_children(), &[10, 20]);
    }

    #[test]
    fn cancellation_chain_dedup_children() {
        let mut chain = CancellationChain::new();
        chain.add_child(5);
        chain.add_child(5);
        chain.cancel();
        assert_eq!(chain.cancelled_children().len(), 1);
    }

    #[test]
    fn progressEstimationAlgorithm_new() {
        let s = ProgressEstimationAlgorithm::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn progressEstimationAlgorithm_add_contains() {
        let mut s = ProgressEstimationAlgorithm::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn progressEstimationAlgorithm_add_duplicate() {
        let mut s = ProgressEstimationAlgorithm::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn progressEstimationAlgorithm_remove() {
        let mut s = ProgressEstimationAlgorithm::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn progressEstimationAlgorithm_capacity() {
        let s = ProgressEstimationAlgorithm::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn progressEstimationAlgorithm_search() {
        let mut s = ProgressEstimationAlgorithm::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn progressEstimationAlgorithm_stats() {
        let mut s = ProgressEstimationAlgorithm::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn progressWidgetLayout_new() {
        let m = ProgressWidgetLayout::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn progressWidgetLayout_add_find() {
        let mut m = ProgressWidgetLayout::new();
        m.add(ProgressWidgetLayoutItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn progressWidgetLayout_priority_filter() {
        let mut m = ProgressWidgetLayout::new();
        m.add(ProgressWidgetLayoutItem::new("a", "A").with_priority(ProgressWidgetLayoutPriority::High));
        m.add(ProgressWidgetLayoutItem::new("b", "B").with_priority(ProgressWidgetLayoutPriority::Low));
        m.add(ProgressWidgetLayoutItem::new("c", "C").with_priority(ProgressWidgetLayoutPriority::High));
        assert_eq!(m.by_priority(ProgressWidgetLayoutPriority::High).len(), 2);
    }

    #[test]
    fn progressWidgetLayout_remove() {
        let mut m = ProgressWidgetLayout::new();
        m.add(ProgressWidgetLayoutItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn progressWidgetLayout_search() {
        let mut m = ProgressWidgetLayout::new();
        m.add(ProgressWidgetLayoutItem::new("id1", "Hello World"));
        m.add(ProgressWidgetLayoutItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn progressWidgetLayout_total_weight() {
        let mut m = ProgressWidgetLayout::new();
        m.add(ProgressWidgetLayoutItem::new("a", "A").with_priority(ProgressWidgetLayoutPriority::Critical));
        m.add(ProgressWidgetLayoutItem::new("b", "B").with_priority(ProgressWidgetLayoutPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn progressWidgetLayout_capacity_limit() {
        let mut m = ProgressWidgetLayout::new().with_max_items(2);
        m.add(ProgressWidgetLayoutItem::new("1", "one"));
        m.add(ProgressWidgetLayoutItem::new("2", "two"));
        assert!(!m.add(ProgressWidgetLayoutItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn progressWidgetLayout_sorted_by_priority() {
        let mut m = ProgressWidgetLayout::new();
        m.add(ProgressWidgetLayoutItem::new("lo", "Low").with_priority(ProgressWidgetLayoutPriority::Low));
        m.add(ProgressWidgetLayoutItem::new("hi", "High").with_priority(ProgressWidgetLayoutPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn progressWidgetLayout_item_metadata() {
        let mut item = ProgressWidgetLayoutItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn progressEstimationAlgorithm_enabled_toggle() {
        let mut s = ProgressEstimationAlgorithm::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn progressWidgetLayout_priority_display() {
        assert_eq!(format!("{}", ProgressWidgetLayoutPriority::High), "high");
        assert_eq!(format!("{}", ProgressWidgetLayoutPriority::Low), "low");
    }


    #[test]
    fn wb_progress_entry_creation() {
        let e = WbProgressEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn wb_progress_entry_with_priority() {
        let e = WbProgressEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn wb_progress_entry_metadata() {
        let e = WbProgressEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn wb_progress_entry_remove_meta() {
        let mut e = WbProgressEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn wb_progress_entry_activate_deactivate() {
        let mut e = WbProgressEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn wb_progress_config_add_sorted() {
        let mut c = WbProgressConfig::new(10);
        c.add(WbProgressEntry::new("lo", "Lo").with_priority(1));
        c.add(WbProgressEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn wb_progress_config_capacity() {
        let mut c = WbProgressConfig::new(1);
        assert!(c.add(WbProgressEntry::new("a", "A")));
        assert!(!c.add(WbProgressEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn wb_progress_config_remove() {
        let mut c = WbProgressConfig::new(10);
        c.add(WbProgressEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn wb_progress_config_get() {
        let mut c = WbProgressConfig::new(10);
        c.add(WbProgressEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn wb_progress_config_active_entries() {
        let mut c = WbProgressConfig::new(10);
        c.add(WbProgressEntry::new("a", "A"));
        c.add(WbProgressEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn wb_progress_config_enable_disable() {
        let mut c = WbProgressConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn wb_progress_config_clear() {
        let mut c = WbProgressConfig::new(10);
        c.add(WbProgressEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn wb_progress_config_find_by_label() {
        let mut c = WbProgressConfig::new(10);
        c.add(WbProgressEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn wb_progress_config_top_n() {
        let mut c = WbProgressConfig::new(10);
        c.add(WbProgressEntry::new("a", "A").with_priority(1));
        c.add(WbProgressEntry::new("b", "B").with_priority(2));
        c.add(WbProgressEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn wb_progress_config_deactivate_activate_all() {
        let mut c = WbProgressConfig::new(10);
        c.add(WbProgressEntry::new("a", "A"));
        c.add(WbProgressEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn wb_progress_config_highest_priority() {
        let mut c = WbProgressConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(WbProgressEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn wb_progress_config_contains() {
        let mut c = WbProgressConfig::new(10);
        c.add(WbProgressEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn wb_progress_config_labels() {
        let mut c = WbProgressConfig::new(10);
        c.add(WbProgressEntry::new("a", "Alpha"));
        c.add(WbProgressEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn wb_progress_config_drain_inactive() {
        let mut c = WbProgressConfig::new(10);
        c.add(WbProgressEntry::new("a", "A"));
        c.add(WbProgressEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn yu_metrics_empty() {
        let m = YuMetrics::new("wb_prog");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yu_metrics_record_and_mean() {
        let mut m = YuMetrics::new("wb_prog");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yu_metrics_min_max() {
        let mut m = YuMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yu_metrics_variance_and_std() {
        let mut m = YuMetrics::new("v");
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
    fn yu_metrics_percentile() {
        let mut m = YuMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yu_metrics_merge() {
        let mut a = YuMetrics::new("a");
        a.record(1.0);
        let mut b = YuMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yu_metrics_reset() {
        let mut m = YuMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yu_rate_window_empty() {
        let rw = YuRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yu_rate_window_tick_and_rate() {
        let mut rw = YuRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yu_lru_cache_basic() {
        let mut c = YuLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yu_lru_cache_contains_and_keys() {
        let mut c = YuLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yu_lru_cache_remove() {
        let mut c = YuLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yu_metrics_sum() {
        let mut m = YuMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yu_metrics_label() {
        let m = YuMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yu_lru_cache_clear() {
        let mut c = YuLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for wb_progress
    #[test]
    fn xa_wb_progress_ring_new() {
        let rb = super::XaWbProgressRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_wb_progress_ring_push_len() {
        let mut rb = super::XaWbProgressRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_wb_progress_ring_wrap() {
        let mut rb = super::XaWbProgressRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_wb_progress_ring_mean_empty() {
        let rb = super::XaWbProgressRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_wb_progress_ring_mean_values() {
        let mut rb = super::XaWbProgressRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_wb_progress_ring_min_max() {
        let mut rb = super::XaWbProgressRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_wb_progress_ring_iter() {
        let mut rb = super::XaWbProgressRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_wb_progress_counter_new() {
        let c = super::XaWbProgressCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_progress_counter_inc() {
        let mut c = super::XaWbProgressCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_wb_progress_counter_inc_by() {
        let mut c = super::XaWbProgressCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_wb_progress_counter_reset() {
        let mut c = super::XaWbProgressCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_wb_progress_counter_clear() {
        let mut c = super::XaWbProgressCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_progress_counter_default() {
        let c = super::XaWbProgressCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 222 ----

    #[test]
    fn xc_222_pool_new_empty() {
        let pool: super::Xc222Pool<i32> = super::Xc222Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_222_pool_release_acquire() {
        let mut pool = super::Xc222Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_222_pool_acquire_empty() {
        let mut pool: super::Xc222Pool<i32> = super::Xc222Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_222_pool_full() {
        let mut pool = super::Xc222Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_222_pool_drain() {
        let mut pool = super::Xc222Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_222_pool_stats() {
        let mut pool = super::Xc222Pool::new(8);
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
    fn xc_222_pool_clear() {
        let mut pool = super::Xc222Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_222_pool_shrink() {
        let mut pool = super::Xc222Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_222_pool_default() {
        let pool: super::Xc222Pool<String> = super::Xc222Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_222_pool_extend() {
        let mut pool = super::Xc222Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_222_pool_retain() {
        let mut pool = super::Xc222Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_222_scheduler_round_robin() {
        let mut sched = super::Xc222Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_222_scheduler_empty() {
        let mut sched = super::Xc222Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_222_scheduler_reset() {
        let mut sched = super::Xc222Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_222_scheduler_add_remove() {
        let mut sched = super::Xc222Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_222_scheduler_targets() {
        let sched = super::Xc222Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_222_hash_empty() {
        assert_eq!(super::xc_222_hash(b""), 5381);
    }

    #[test]
    fn xc_222_hash_data() {
        let h = super::xc_222_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_222_hash(b"hello"), h);
    }

    #[test]
    fn xc_222_reverse_str() {
        assert_eq!(super::xc_222_reverse("abc"), "cba");
        assert_eq!(super::xc_222_reverse(""), "");
    }


    // --- xd_106 deepening tests ---

    #[test]
    fn xd_106_sm_initial_state() {
        let sm = Xd106StateMachine::new();
        assert_eq!(sm.current_state(), Xd106State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_106_sm_valid_idle_to_running() {
        let mut sm = Xd106StateMachine::new();
        assert!(sm.transition(Xd106State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd106State::Running);
    }

    #[test]
    fn xd_106_sm_valid_running_to_paused() {
        let mut sm = Xd106StateMachine::new();
        sm.transition(Xd106State::Running).unwrap();
        assert!(sm.transition(Xd106State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd106State::Paused);
    }

    #[test]
    fn xd_106_sm_valid_running_to_done() {
        let mut sm = Xd106StateMachine::new();
        sm.transition(Xd106State::Running).unwrap();
        assert!(sm.transition(Xd106State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd106State::Done);
    }

    #[test]
    fn xd_106_sm_valid_paused_to_running() {
        let mut sm = Xd106StateMachine::new();
        sm.transition(Xd106State::Running).unwrap();
        sm.transition(Xd106State::Paused).unwrap();
        assert!(sm.transition(Xd106State::Running).is_ok());
    }

    #[test]
    fn xd_106_sm_valid_done_to_idle() {
        let mut sm = Xd106StateMachine::new();
        sm.transition(Xd106State::Running).unwrap();
        sm.transition(Xd106State::Done).unwrap();
        assert!(sm.transition(Xd106State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd106State::Idle);
    }

    #[test]
    fn xd_106_sm_invalid_idle_to_done() {
        let mut sm = Xd106StateMachine::new();
        assert!(sm.transition(Xd106State::Done).is_err());
    }

    #[test]
    fn xd_106_sm_invalid_idle_to_paused() {
        let mut sm = Xd106StateMachine::new();
        assert!(sm.transition(Xd106State::Paused).is_err());
    }

    #[test]
    fn xd_106_sm_history_tracking() {
        let mut sm = Xd106StateMachine::new();
        sm.transition(Xd106State::Running).unwrap();
        sm.transition(Xd106State::Paused).unwrap();
        sm.transition(Xd106State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd106State::Idle);
        assert_eq!(sm.history()[0].to, Xd106State::Running);
        assert_eq!(sm.history()[1].from, Xd106State::Running);
        assert_eq!(sm.history()[2].to, Xd106State::Done);
    }

    #[test]
    fn xd_106_sm_serialize_deserialize() {
        let mut sm = Xd106StateMachine::new();
        sm.transition(Xd106State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd106StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd106State::Running));
    }

    #[test]
    fn xd_106_sm_deserialize_invalid() {
        assert_eq!(Xd106StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_106_sm_reset() {
        let mut sm = Xd106StateMachine::new();
        sm.transition(Xd106State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd106State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_106_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd106EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd106Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_106_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd106EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd106Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd106Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_106_bus_unsubscribe() {
        let mut bus = Xd106EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_106_event_kind_and_payload() {
        let e = Xd106Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd106Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_106_bus_clear_history() {
        let mut bus = Xd106EventBus::new();
        bus.publish(Xd106Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_106_sm_step_counter_increments() {
        let mut sm = Xd106StateMachine::new();
        sm.transition(Xd106State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd106State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_30 graph tests ------------------------------------------------

    #[test]
    fn xg_30_graph_empty() {
        let g = super::Xg30Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_30_graph_add_node() {
        let mut g = super::Xg30Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_30_graph_add_edge() {
        let mut g = super::Xg30Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_30_graph_neighbors() {
        let mut g = super::Xg30Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_30_graph_has_path() {
        let mut g = super::Xg30Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_30_graph_self_path() {
        let g = super::Xg30Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_30_graph_topo_sort() {
        let mut g = super::Xg30Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_30_graph_cycle_detect_false() {
        let mut g = super::Xg30Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_30_graph_cycle_detect_true() {
        let mut g = super::Xg30Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_30 heap tests -------------------------------------------------

    #[test]
    fn xg_30_heap_empty() {
        let h: super::Xg30Heap<i32> = super::Xg30Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_30_heap_push_pop() {
        let mut h = super::Xg30Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_30_heap_peek() {
        let mut h = super::Xg30Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_30_heap_drain_sorted() {
        let mut h = super::Xg30Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_30_heap_merge() {
        let mut a = super::Xg30Heap::new();
        let mut b = super::Xg30Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_30_heap_default() {
        let h: super::Xg30Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_30_graph_default() {
        let g: super::Xg30Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh221_skip_insert_contains() {
        let mut sl = super::Xh221SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh221_skip_remove() {
        let mut sl = super::Xh221SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh221_skip_len() {
        let mut sl = super::Xh221SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh221_skip_range_query() {
        let mut sl = super::Xh221SkipList::xh_new(4);
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
    fn xh221_skip_floor_ceiling() {
        let mut sl = super::Xh221SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh221_skip_rank() {
        let mut sl = super::Xh221SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh221_skip_empty() {
        let sl = super::Xh221SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh221_skip_duplicates() {
        let mut sl = super::Xh221SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh221_bitset_set_test() {
        let mut bs = super::Xh221BitSet::xh_new(256);
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
    fn xh221_bitset_clear_count() {
        let mut bs = super::Xh221BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh221_bitset_and_or_xor() {
        let mut a = super::Xh221BitSet::xh_new(128);
        let mut b = super::Xh221BitSet::xh_new(128);
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
    fn xh221_bitset_iter_ones() {
        let mut bs = super::Xh221BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh221_bitset_first_last() {
        let mut bs = super::Xh221BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh221_bitset_empty() {
        let bs = super::Xh221BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi221_deque_push_pop_back() {
        let mut dq = super::Xi221Deque::xi_new(4);
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
    fn xi221_deque_push_pop_front() {
        let mut dq = super::Xi221Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi221_deque_mixed_ops() {
        let mut dq = super::Xi221Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi221_deque_get_and_split() {
        let mut dq = super::Xi221Deque::xi_new(8);
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
    fn xi221_deque_rotate_left() {
        let mut dq = super::Xi221Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi221_deque_rotate_right() {
        let mut dq = super::Xi221Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi221_deque_grow() {
        let mut dq = super::Xi221Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi221_deque_empty() {
        let dq = super::Xi221Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi221_interval_tree_insert_query() {
        let mut tree = super::Xi221IntervalTree::xi_new();
        tree.xi_insert(super::Xi221Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi221Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi221Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi221_interval_tree_overlap() {
        let mut tree = super::Xi221IntervalTree::xi_new();
        tree.xi_insert(super::Xi221Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi221Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi221Interval::xi_new(12, 20));
        let q = super::Xi221Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi221_interval_tree_remove() {
        let mut tree = super::Xi221IntervalTree::xi_new();
        tree.xi_insert(super::Xi221Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi221Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi221_interval_tree_gaps() {
        let mut tree = super::Xi221IntervalTree::xi_new();
        tree.xi_insert(super::Xi221Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi221Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi221Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi221Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi221Interval::xi_new(8, 10));
    }

    #[test]
    fn xi221_interval_tree_merge() {
        let mut tree = super::Xi221IntervalTree::xi_new();
        tree.xi_insert(super::Xi221Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi221Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi221Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi221Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi221Interval::xi_new(10, 15));
    }

    #[test]
    fn xi221_interval_tree_all() {
        let mut tree = super::Xi221IntervalTree::xi_new();
        tree.xi_insert(super::Xi221Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi221Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi221_interval_tree_empty() {
        let tree = super::Xi221IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi221_interval_tree_contains_point() {
        let iv = super::Xi221Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}
