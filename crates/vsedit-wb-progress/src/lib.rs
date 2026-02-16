//! Progress indicators.

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
}
