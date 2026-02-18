//! App startup and shutdown lifecycle.
//!
//! Equivalent to VS Code's `vs/platform/lifecycle/common/lifecycle.ts`.
//! Manages application phases and shutdown confirmation.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use vsedit_events::{Emitter, Event};

/// Application lifecycle phases (in order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LifecyclePhase {
    /// Services are being created.
    Starting = 0,
    /// Workspace storage is ready.
    Ready = 1,
    /// The workbench layout has been restored.
    Restored = 2,
    /// The workbench is fully interactive.
    Eventually = 3,
}

impl LifecyclePhase {
    /// Returns true if this is the final phase (`Eventually`).
    pub fn is_terminal(&self) -> bool {
        *self == LifecyclePhase::Eventually
    }

    /// Returns the next phase, or `None` if already terminal.
    pub fn next(&self) -> Option<LifecyclePhase> {
        match self {
            LifecyclePhase::Starting => Some(LifecyclePhase::Ready),
            LifecyclePhase::Ready => Some(LifecyclePhase::Restored),
            LifecyclePhase::Restored => Some(LifecyclePhase::Eventually),
            LifecyclePhase::Eventually => None,
        }
    }
}

/// Returns a human-readable name for a lifecycle phase.
pub fn phase_name(phase: LifecyclePhase) -> &'static str {
    match phase {
        LifecyclePhase::Starting => "Starting",
        LifecyclePhase::Ready => "Ready",
        LifecyclePhase::Restored => "Restored",
        LifecyclePhase::Eventually => "Eventually",
    }
}

/// Shutdown reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownReason {
    /// User requested quit.
    Quit,
    /// Window was closed.
    Close,
    /// Reload window.
    Reload,
    /// Process was killed.
    Kill,
}

impl ShutdownReason {
    /// Returns `true` if the reason is user-initiated (`Quit` or `Close`).
    pub fn is_user_initiated(&self) -> bool {
        matches!(self, ShutdownReason::Quit | ShutdownReason::Close)
    }

    /// Returns `true` if the shutdown is destructive and non-recoverable.
    pub fn is_destructive(&self) -> bool {
        matches!(self, ShutdownReason::Kill)
    }

    /// Returns `true` if the shutdown allows the window to reopen (i.e. `Reload`).
    pub fn is_recoverable(&self) -> bool {
        matches!(self, ShutdownReason::Reload)
    }

    /// Returns a human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            ShutdownReason::Quit => "Quit",
            ShutdownReason::Close => "Close",
            ShutdownReason::Reload => "Reload",
            ShutdownReason::Kill => "Kill",
        }
    }

    /// Returns all shutdown reason variants.
    pub fn all() -> &'static [ShutdownReason] {
        &[
            ShutdownReason::Quit,
            ShutdownReason::Close,
            ShutdownReason::Reload,
            ShutdownReason::Kill,
        ]
    }
}

impl fmt::Display for ShutdownReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Veto that can prevent shutdown.
#[derive(Clone)]
pub struct WillShutdownEvent {
    pub reason: ShutdownReason,
    vetoed: Arc<Mutex<bool>>,
}

impl WillShutdownEvent {
    pub fn new(reason: ShutdownReason) -> Self {
        Self {
            reason,
            vetoed: Arc::new(Mutex::new(false)),
        }
    }

    /// Veto the shutdown (e.g., unsaved changes).
    pub fn veto(&self) {
        *self.vetoed.lock().unwrap() = true;
    }

    pub fn is_vetoed(&self) -> bool {
        *self.vetoed.lock().unwrap()
    }

    /// Returns `true` if the shutdown reason is user-initiated.
    pub fn is_user_initiated(&self) -> bool {
        self.reason.is_user_initiated()
    }

    /// Returns `true` if the shutdown can be safely vetoed (non-kill reasons).
    pub fn is_vetoable(&self) -> bool {
        !self.reason.is_destructive()
    }

    /// Conditionally veto: only vetoes if the reason is vetoable.
    /// Returns `true` if the veto was applied.
    pub fn try_veto(&self) -> bool {
        if self.is_vetoable() {
            self.veto();
            true
        } else {
            false
        }
    }
}

/// Snapshot of a completed shutdown.
#[derive(Debug, Clone)]
pub struct ShutdownEvent {
    pub reason: ShutdownReason,
    pub phase_at_shutdown: LifecyclePhase,
    pub timestamp: Instant,
}

/// Statistics about the lifecycle service.
#[derive(Debug, Clone)]
pub struct LifecycleStats {
    pub phase_transition_count: u64,
    pub shutdown_attempts: u64,
    pub vetoed_shutdowns: u64,
    pub current_phase: LifecyclePhase,
}

impl LifecycleStats {
    /// Returns the ratio of vetoed shutdowns to total attempts, or 0 if none attempted.
    pub fn veto_rate(&self) -> f64 {
        if self.shutdown_attempts == 0 {
            return 0.0;
        }
        self.vetoed_shutdowns as f64 / self.shutdown_attempts as f64
    }

    /// Returns `true` if the service has progressed past the initial phase.
    pub fn has_progressed(&self) -> bool {
        self.phase_transition_count > 0
    }

    /// Returns `true` if any shutdown was ever attempted.
    pub fn has_shutdown_history(&self) -> bool {
        self.shutdown_attempts > 0
    }

    /// Returns the number of successful (non-vetoed) shutdown attempts.
    pub fn successful_shutdowns(&self) -> u64 {
        self.shutdown_attempts.saturating_sub(self.vetoed_shutdowns)
    }
}

/// The lifecycle service.
pub struct LifecycleService {
    phase: AtomicU8,
    phase_transition_count: AtomicU64,
    shutdown_attempt_count: AtomicU64,
    vetoed_count: AtomicU64,
    is_shut_down: AtomicBool,
    on_will_shutdown: Emitter<WillShutdownEvent>,
    on_did_shutdown: Emitter<ShutdownReason>,
    on_phase_change: Emitter<LifecyclePhase>,
    barriers: Mutex<Vec<ShutdownBarrier>>,
    timeline: Mutex<LifecycleTimeline>,
}

impl LifecycleService {
    pub fn new() -> Self {
        Self {
            phase: AtomicU8::new(LifecyclePhase::Starting as u8),
            phase_transition_count: AtomicU64::new(0),
            shutdown_attempt_count: AtomicU64::new(0),
            vetoed_count: AtomicU64::new(0),
            is_shut_down: AtomicBool::new(false),
            on_will_shutdown: Emitter::new(),
            on_did_shutdown: Emitter::new(),
            on_phase_change: Emitter::new(),
            barriers: Mutex::new(Vec::new()),
            timeline: Mutex::new(LifecycleTimeline::new()),
        }
    }

    pub fn phase(&self) -> LifecyclePhase {
        match self.phase.load(Ordering::Relaxed) {
            0 => LifecyclePhase::Starting,
            1 => LifecyclePhase::Ready,
            2 => LifecyclePhase::Restored,
            _ => LifecyclePhase::Eventually,
        }
    }

    /// Advance to the next phase. Fires phase change event.
    pub fn set_phase(&self, phase: LifecyclePhase) {
        let current = self.phase.load(Ordering::Relaxed);
        if (phase as u8) > current {
            self.phase.store(phase as u8, Ordering::Relaxed);
            self.phase_transition_count.fetch_add(1, Ordering::Relaxed);
            self.on_phase_change.fire(&phase);
        }
    }

    /// Request shutdown. Returns false if vetoed.
    pub fn request_shutdown(&self, reason: ShutdownReason) -> bool {
        self.shutdown_attempt_count.fetch_add(1, Ordering::Relaxed);
        let event = WillShutdownEvent::new(reason);
        self.on_will_shutdown.fire(&event);

        if event.is_vetoed() {
            self.vetoed_count.fetch_add(1, Ordering::Relaxed);
            return false;
        }

        self.is_shut_down.store(true, Ordering::Relaxed);
        self.on_did_shutdown.fire(&reason);
        true
    }

    /// Force shutdown without checking for vetoes.
    pub fn force_shutdown(&self, reason: ShutdownReason) -> bool {
        self.shutdown_attempt_count.fetch_add(1, Ordering::Relaxed);
        self.is_shut_down.store(true, Ordering::Relaxed);
        self.on_did_shutdown.fire(&reason);
        true
    }

    /// Returns true if the current phase is at least `phase`.
    pub fn is_phase_at_least(&self, phase: LifecyclePhase) -> bool {
        self.phase.load(Ordering::Relaxed) >= phase as u8
    }

    /// Register a callback to run when a specific phase is reached.
    /// If the phase has already been reached, the callback runs immediately.
    pub fn when_phase<F>(&self, phase: LifecyclePhase, callback: F)
    where
        F: FnOnce() + Send + 'static,
    {
        if self.is_phase_at_least(phase) {
            callback();
            return;
        }

        let callback = Arc::new(Mutex::new(Some(callback)));
        let _sub = self.on_phase_change().on(move |current: &LifecyclePhase| {
            if *current >= phase {
                if let Some(cb) = callback.lock().unwrap().take() {
                    cb();
                }
            }
        });
    }

    /// Get a snapshot of lifecycle statistics.
    pub fn get_stats(&self) -> LifecycleStats {
        LifecycleStats {
            phase_transition_count: self.phase_transition_count.load(Ordering::Relaxed),
            shutdown_attempts: self.shutdown_attempt_count.load(Ordering::Relaxed),
            vetoed_shutdowns: self.vetoed_count.load(Ordering::Relaxed),
            current_phase: self.phase(),
        }
    }

    /// Total number of shutdown attempts.
    pub fn shutdown_attempt_count(&self) -> u64 {
        self.shutdown_attempt_count.load(Ordering::Relaxed)
    }

    /// Number of shutdowns that were vetoed.
    pub fn vetoed_count(&self) -> u64 {
        self.vetoed_count.load(Ordering::Relaxed)
    }

    pub fn on_will_shutdown(&self) -> Event<WillShutdownEvent> {
        self.on_will_shutdown.event()
    }

    pub fn on_did_shutdown(&self) -> Event<ShutdownReason> {
        self.on_did_shutdown.event()
    }

    pub fn on_phase_change(&self) -> Event<LifecyclePhase> {
        self.on_phase_change.event()
    }

    /// Register a shutdown barrier.
    pub fn register_barrier(&self, barrier: ShutdownBarrier) {
        self.barriers.lock().unwrap().push(barrier);
    }

    /// Number of registered barriers.
    pub fn barrier_count(&self) -> usize {
        self.barriers.lock().unwrap().len()
    }

    /// Access the lifecycle timeline.
    pub fn timeline(&self) -> std::sync::MutexGuard<'_, LifecycleTimeline> {
        self.timeline.lock().unwrap()
    }

    /// Returns true if the service has been shut down.
    pub fn is_shut_down(&self) -> bool {
        self.is_shut_down.load(Ordering::Relaxed)
    }

    /// Reset the service from ShutDown back to Starting.
    /// Only succeeds if the service is currently shut down.
    pub fn service_restart(&self) -> Result<(), String> {
        if !self.is_shut_down.load(Ordering::Relaxed) {
            return Err("service is not shut down".to_string());
        }
        self.phase.store(LifecyclePhase::Starting as u8, Ordering::Relaxed);
        self.is_shut_down.store(false, Ordering::Relaxed);
        self.timeline.lock().unwrap().clear();
        Ok(())
    }
}

impl Default for LifecycleService {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for lifecycle-svc operations.
#[derive(Debug, Clone, PartialEq)]
pub struct LifecycleSvcStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl LifecycleSvcStats {
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
    pub fn merge(&mut self, other: &LifecycleSvcStats) {
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

impl Default for LifecycleSvcStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LifecycleSvcStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LifecycleSvcStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for lifecycle-svc.
#[derive(Debug, Clone)]
pub struct LifecycleSvcValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl LifecycleSvcValidator {
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

impl Default for LifecycleSvcValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ShutdownBarrier
// ---------------------------------------------------------------------------

/// Tracks registered task names and blocks until all complete.
#[derive(Debug, Clone)]
pub struct ShutdownBarrier {
    pending: HashSet<String>,
}

impl ShutdownBarrier {
    /// Create a new empty barrier.
    pub fn new() -> Self {
        Self {
            pending: HashSet::new(),
        }
    }

    /// Register a task by name.
    pub fn register(&mut self, name: &str) {
        self.pending.insert(name.to_string());
    }

    /// Mark a task as complete. Returns `true` if the task was found and removed.
    pub fn complete(&mut self, name: &str) -> bool {
        self.pending.remove(name)
    }

    /// Number of tasks still pending.
    pub fn remaining(&self) -> usize {
        self.pending.len()
    }

    /// Returns `true` if no tasks are pending.
    pub fn is_clear(&self) -> bool {
        self.pending.is_empty()
    }

    /// Names of pending tasks.
    pub fn pending_names(&self) -> Vec<&str> {
        self.pending.iter().map(|s| s.as_str()).collect()
    }

    /// Returns `true` if a task with the given name is currently pending.
    pub fn contains(&self, name: &str) -> bool {
        self.pending.contains(name)
    }

    /// Register multiple tasks at once.
    pub fn register_all(&mut self, names: &[&str]) {
        for name in names {
            self.pending.insert((*name).to_string());
        }
    }

    /// Complete all pending tasks, returning how many were cleared.
    pub fn complete_all(&mut self) -> usize {
        let count = self.pending.len();
        self.pending.clear();
        count
    }
}

impl Default for ShutdownBarrier {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// StartupPhase
// ---------------------------------------------------------------------------

/// Phases of application startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum StartupPhase {
    /// Before services are created.
    EarlyInit = 0,
    /// Services are being created.
    ServiceInit = 1,
    /// Extensions are loading.
    ExtensionLoad = 2,
    /// Fully ready.
    Ready = 3,
}

impl StartupPhase {
    /// Human-readable label for this phase.
    pub fn phase_label(&self) -> &'static str {
        match self {
            StartupPhase::EarlyInit => "Early Init",
            StartupPhase::ServiceInit => "Service Init",
            StartupPhase::ExtensionLoad => "Extension Load",
            StartupPhase::Ready => "Ready",
        }
    }

    /// Returns `true` if this phase is `Ready`.
    pub fn is_complete(&self) -> bool {
        *self == StartupPhase::Ready
    }
}

impl fmt::Display for StartupPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.phase_label())
    }
}

// ---------------------------------------------------------------------------
// TimelineEntry
// ---------------------------------------------------------------------------

/// A single entry in the startup timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEntry {
    pub phase: StartupPhase,
    pub label: String,
    pub duration_ms: u64,
}

impl TimelineEntry {
    /// Returns `true` if this entry took longer than the given threshold.
    pub fn is_slow(&self, threshold_ms: u64) -> bool {
        self.duration_ms > threshold_ms
    }

    /// Returns the duration formatted as a human-readable string.
    pub fn formatted_duration(&self) -> String {
        format_duration_ms(self.duration_ms)
    }

    /// Returns a one-line summary of this entry.
    pub fn one_line_summary(&self) -> String {
        format!("[{}] {} ({})", self.phase, self.label, self.formatted_duration())
    }
}

// ---------------------------------------------------------------------------
// LifecycleTimeline
// ---------------------------------------------------------------------------

/// Records startup profiling entries grouped by phase.
#[derive(Debug, Clone)]
pub struct LifecycleTimeline {
    entries: Vec<TimelineEntry>,
}

impl LifecycleTimeline {
    /// Create a new empty timeline.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Record a timing entry.
    pub fn record(&mut self, phase: StartupPhase, label: &str, duration_ms: u64) {
        self.entries.push(TimelineEntry {
            phase,
            label: label.to_string(),
            duration_ms,
        });
    }

    /// Sum of all recorded durations.
    pub fn total_duration_ms(&self) -> u64 {
        self.entries.iter().map(|e| e.duration_ms).sum()
    }

    /// Filter entries by phase.
    pub fn entries_for_phase(&self, phase: StartupPhase) -> Vec<&TimelineEntry> {
        self.entries.iter().filter(|e| e.phase == phase).collect()
    }

    /// Entry with the longest duration.
    pub fn slowest_entry(&self) -> Option<&TimelineEntry> {
        self.entries.iter().max_by_key(|e| e.duration_ms)
    }

    /// Number of recorded entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Formatted summary string.
    pub fn to_summary(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "Timeline: {} entries, total={}ms",
            self.entries.len(),
            self.total_duration_ms()
        ));
        for entry in &self.entries {
            lines.push(format!(
                "  [{}] {} ({}ms)",
                entry.phase, entry.label, entry.duration_ms
            ));
        }
        lines.join("\n")
    }
}

impl Default for LifecycleTimeline {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// HealthStatus
// ---------------------------------------------------------------------------

/// Health status snapshot from a lifecycle service.
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub phase_name: String,
    pub is_healthy: bool,
    pub uptime_events: usize,
    pub barrier_count: usize,
}

impl HealthStatus {
    /// Returns `true` if the service has barriers registered.
    pub fn has_barriers(&self) -> bool {
        self.barrier_count > 0
    }

    /// Returns a one-line diagnostic summary.
    pub fn summary(&self) -> String {
        let health = if self.is_healthy { "healthy" } else { "unhealthy" };
        format!(
            "phase={}, status={}, events={}, barriers={}",
            self.phase_name, health, self.uptime_events, self.barrier_count
        )
    }
}

impl fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

/// Check the health of a lifecycle service.
pub fn lifecycle_health_check(svc: &LifecycleService) -> HealthStatus {
    let phase = svc.phase();
    let is_shut_down = svc.is_shut_down();
    HealthStatus {
        phase_name: phase_name(phase).to_string(),
        is_healthy: !is_shut_down,
        uptime_events: svc.timeline().entry_count(),
        barrier_count: svc.barrier_count(),
    }
}

// ---------------------------------------------------------------------------
// Phase iteration
// ---------------------------------------------------------------------------

/// Iterator over lifecycle phases in order.
pub struct PhaseIter {
    current: Option<LifecyclePhase>,
}

impl PhaseIter {
    pub fn new() -> Self {
        Self { current: Some(LifecyclePhase::Starting) }
    }
}

impl Default for PhaseIter {
    fn default() -> Self {
        Self::new()
    }
}

impl Iterator for PhaseIter {
    type Item = LifecyclePhase;

    fn next(&mut self) -> Option<Self::Item> {
        let phase = self.current?;
        self.current = phase.next();
        Some(phase)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = match self.current {
            Some(LifecyclePhase::Starting) => 4,
            Some(LifecyclePhase::Ready) => 3,
            Some(LifecyclePhase::Restored) => 2,
            Some(LifecyclePhase::Eventually) => 1,
            None => 0,
        };
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for PhaseIter {}

// ---------------------------------------------------------------------------
// LifecyclePhase helpers
// ---------------------------------------------------------------------------

impl LifecyclePhase {
    /// Returns the zero-based index of this phase.
    pub fn index(&self) -> usize {
        *self as usize
    }

    /// Returns all phases in order.
    pub fn all() -> &'static [LifecyclePhase] {
        &[
            LifecyclePhase::Starting,
            LifecyclePhase::Ready,
            LifecyclePhase::Restored,
            LifecyclePhase::Eventually,
        ]
    }

    /// Parse from a string name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "starting" => Some(Self::Starting),
            "ready" => Some(Self::Ready),
            "restored" => Some(Self::Restored),
            "eventually" => Some(Self::Eventually),
            _ => None,
        }
    }

    /// Returns the phase name as a static string.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Restored => "restored",
            Self::Eventually => "eventually",
        }
    }
}

// ---------------------------------------------------------------------------
// StartupPhase helpers
// ---------------------------------------------------------------------------

impl StartupPhase {
    /// Returns all startup phase variants.
    pub fn all() -> Vec<Self> {
        vec![
            Self::EarlyInit,
            Self::ServiceInit,
            Self::ExtensionLoad,
            Self::Ready,
        ]
    }

    /// Returns the ordinal position of this phase.
    pub fn ordinal(&self) -> usize {
        match self {
            Self::EarlyInit => 0,
            Self::ServiceInit => 1,
            Self::ExtensionLoad => 2,
            Self::Ready => 3,
        }
    }

    /// Returns the next startup phase, or `None` if already `Ready`.
    pub fn next(&self) -> Option<Self> {
        match self {
            Self::EarlyInit => Some(Self::ServiceInit),
            Self::ServiceInit => Some(Self::ExtensionLoad),
            Self::ExtensionLoad => Some(Self::Ready),
            Self::Ready => None,
        }
    }

    /// Returns the previous startup phase, or `None` if already `EarlyInit`.
    pub fn previous(&self) -> Option<Self> {
        match self {
            Self::EarlyInit => None,
            Self::ServiceInit => Some(Self::EarlyInit),
            Self::ExtensionLoad => Some(Self::ServiceInit),
            Self::Ready => Some(Self::ExtensionLoad),
        }
    }

    /// Parse from a string label (case-insensitive).
    pub fn from_label(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "early init" | "earlyinit" | "early_init" => Some(Self::EarlyInit),
            "service init" | "serviceinit" | "service_init" => Some(Self::ServiceInit),
            "extension load" | "extensionload" | "extension_load" => Some(Self::ExtensionLoad),
            "ready" => Some(Self::Ready),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Timeline helpers
// ---------------------------------------------------------------------------

impl LifecycleTimeline {
    /// Returns a slice of all recorded entries.
    pub fn entries(&self) -> &[TimelineEntry] {
        &self.entries
    }

    /// Returns the total span from first to last entry duration.
    pub fn total_span_ms(&self) -> Option<u64> {
        let entries = self.entries();
        if entries.len() < 2 {
            return None;
        }
        let first = entries.first().unwrap().duration_ms;
        let last = entries.last().unwrap().duration_ms;
        Some(last.saturating_sub(first))
    }

    /// Returns entry labels as a comma-separated string.
    pub fn summary(&self) -> String {
        self.entries()
            .iter()
            .map(|e| e.label.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Returns entries whose duration exceeds the threshold.
    pub fn slow_entries(&self, threshold_ms: u64) -> Vec<&TimelineEntry> {
        self.entries.iter().filter(|e| e.is_slow(threshold_ms)).collect()
    }

    /// Returns the entry with the shortest duration, or `None` if empty.
    pub fn fastest_entry(&self) -> Option<&TimelineEntry> {
        self.entries.iter().min_by_key(|e| e.duration_ms)
    }

    /// Returns a count of entries per phase.
    pub fn count_by_phase(&self) -> Vec<(StartupPhase, usize)> {
        StartupPhase::all()
            .into_iter()
            .map(|p| (p, self.entries.iter().filter(|e| e.phase == p).count()))
            .filter(|(_, count)| *count > 0)
            .collect()
    }

    /// Returns the average duration across all entries, or `None` if empty.
    pub fn average_duration_ms(&self) -> Option<u64> {
        if self.entries.is_empty() {
            return None;
        }
        Some(self.total_duration_ms() / self.entries.len() as u64)
    }
}

/// Format a duration in milliseconds as a human-readable string.
pub fn format_duration_ms(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        let mins = ms / 60_000;
        let secs = (ms % 60_000) / 1000;
        format!("{mins}m {secs}s")
    }
}


// ---------------------------------------------------------------------------
// LifecycleHook
// ---------------------------------------------------------------------------

/// A named hook that fires during a specific lifecycle phase.
#[derive(Debug, Clone)]
pub struct LifecycleHook {
    /// Unique name for this hook.
    pub name: String,
    /// The phase during which this hook should fire.
    pub phase: LifecyclePhase,
    /// Human-readable description of what the hook callback does.
    pub callback_description: String,
}

impl LifecycleHook {
    pub fn new(name: impl Into<String>, phase: LifecyclePhase, desc: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            phase,
            callback_description: desc.into(),
        }
    }
}

impl fmt::Display for LifecycleHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} - {}", phase_name(self.phase), self.name, self.callback_description)
    }
}

// ---------------------------------------------------------------------------
// HookRegistry
// ---------------------------------------------------------------------------

/// Registry that collects lifecycle hooks and retrieves them by phase.
#[derive(Debug, Default)]
pub struct HookRegistry {
    hooks: Vec<LifecycleHook>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// Register a new hook. Returns `false` if a hook with the same name
    /// already exists.
    pub fn register(&mut self, hook: LifecycleHook) -> bool {
        if self.hooks.iter().any(|h| h.name == hook.name) {
            return false;
        }
        self.hooks.push(hook);
        true
    }

    /// Remove a hook by name. Returns `true` if found and removed.
    pub fn unregister(&mut self, name: &str) -> bool {
        let before = self.hooks.len();
        self.hooks.retain(|h| h.name != name);
        self.hooks.len() < before
    }

    /// Return all hooks registered for a given phase.
    pub fn hooks_for_phase(&self, phase: LifecyclePhase) -> Vec<&LifecycleHook> {
        self.hooks.iter().filter(|h| h.phase == phase).collect()
    }

    /// Total number of registered hooks.
    pub fn len(&self) -> usize {
        self.hooks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }
}

// ---------------------------------------------------------------------------
// LifecycleMetrics
// ---------------------------------------------------------------------------

/// Tracks phase-transition durations for performance monitoring.
#[derive(Debug, Default)]
pub struct LifecycleMetrics {
    transitions: Vec<(LifecyclePhase, LifecyclePhase, u64)>,
}

impl LifecycleMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the time (in ms) taken to move from `from` to `to`.
    pub fn record(&mut self, from: LifecyclePhase, to: LifecyclePhase, ms: u64) {
        self.transitions.push((from, to, ms));
    }

    /// Average transition time across all recorded transitions.
    pub fn average_transition_ms(&self) -> Option<f64> {
        if self.transitions.is_empty() {
            return None;
        }
        let total: u64 = self.transitions.iter().map(|t| t.2).sum();
        Some(total as f64 / self.transitions.len() as f64)
    }

    /// Sum of all transition durations (approximates total startup time).
    pub fn total_startup_ms(&self) -> u64 {
        self.transitions.iter().map(|t| t.2).sum()
    }

    /// Number of recorded transitions.
    pub fn len(&self) -> usize {
        self.transitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transitions.is_empty()
    }
}

impl fmt::Display for LifecycleMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LifecycleMetrics({} transitions, total {}ms", self.len(), self.total_startup_ms())?;
        if let Some(avg) = self.average_transition_ms() {
            write!(f, ", avg {avg:.1}ms")?;
        }
        write!(f, ")")
    }
}

// ---------------------------------------------------------------------------
// ShutdownGuard
// ---------------------------------------------------------------------------

/// Token-based shutdown veto system. Each guard holds a token; shutdown is
/// blocked as long as at least one token is outstanding.
#[derive(Debug, Default)]
pub struct ShutdownGuard {
    tokens: HashSet<String>,
}

impl ShutdownGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Acquire a veto token. Returns `false` if the token name is already held.
    pub fn acquire(&mut self, token: impl Into<String>) -> bool {
        self.tokens.insert(token.into())
    }

    /// Release a veto token. Returns `false` if the token was not held.
    pub fn release(&mut self, token: &str) -> bool {
        self.tokens.remove(token)
    }

    /// Returns `true` if shutdown is currently allowed (no outstanding tokens).
    pub fn can_shutdown(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Number of outstanding veto tokens.
    pub fn outstanding(&self) -> usize {
        self.tokens.len()
    }

    /// List all outstanding token names.
    pub fn token_names(&self) -> Vec<&str> {
        self.tokens.iter().map(|s| s.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// LifecycleHookRegistry
// ---------------------------------------------------------------------------

/// Result of running a single lifecycle hook.
#[derive(Debug, Clone)]
pub struct HookResult {
    /// Name of the hook that was run.
    pub name: String,
    /// Whether the hook completed successfully.
    pub success: bool,
    /// Error message if the hook failed.
    pub error: Option<String>,
}

impl fmt::Display for HookResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.success {
            write!(f, "hook '{}': ok", self.name)
        } else {
            write!(f, "hook '{}': FAILED - {}", self.name, self.error.as_deref().unwrap_or("unknown"))
        }
    }
}

/// Entry in the hook registry: a named, prioritised callback for a phase.
struct PrioritisedHook {
    phase: String,
    priority: i32,
    name: String,
    hook: Box<dyn Fn() -> Result<(), String>>,
}

/// Registry of hooks with priorities, keyed by phase name.
pub struct LifecycleHookRegistry {
    hooks: Vec<PrioritisedHook>,
}

impl Default for LifecycleHookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl LifecycleHookRegistry {
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// Register a hook for a given phase with a numeric priority (lower = earlier).
    pub fn register(
        &mut self,
        phase: &str,
        priority: i32,
        name: &str,
        hook: Box<dyn Fn() -> Result<(), String>>,
    ) {
        self.hooks.push(PrioritisedHook {
            phase: phase.to_string(),
            priority,
            name: name.to_string(),
            hook,
        });
    }

    /// Run all hooks registered for `phase` in priority order (lower first).
    pub fn run_hooks(&self, phase: &str) -> Vec<HookResult> {
        let mut indices: Vec<usize> = self
            .hooks
            .iter()
            .enumerate()
            .filter(|(_, h)| h.phase == phase)
            .map(|(i, _)| i)
            .collect();
        indices.sort_by_key(|&i| self.hooks[i].priority);

        indices
            .into_iter()
            .map(|i| {
                let h = &self.hooks[i];
                match (h.hook)() {
                    Ok(()) => HookResult { name: h.name.clone(), success: true, error: None },
                    Err(e) => HookResult { name: h.name.clone(), success: false, error: Some(e) },
                }
            })
            .collect()
    }

    /// Total number of registered hooks across all phases.
    pub fn hook_count(&self) -> usize {
        self.hooks.len()
    }

    /// Number of hooks registered for a specific phase.
    pub fn hooks_for_phase(&self, phase: &str) -> usize {
        self.hooks.iter().filter(|h| h.phase == phase).count()
    }
}

// ---------------------------------------------------------------------------
// LifecycleHealthCheck
// ---------------------------------------------------------------------------

/// Health status of a single check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckHealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

impl fmt::Display for CheckHealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckHealthStatus::Healthy => write!(f, "healthy"),
            CheckHealthStatus::Degraded(msg) => write!(f, "degraded: {msg}"),
            CheckHealthStatus::Unhealthy(msg) => write!(f, "unhealthy: {msg}"),
        }
    }
}

impl Default for CheckHealthStatus {
    fn default() -> Self {
        CheckHealthStatus::Healthy
    }
}

/// Aggregated health report from all registered checks.
#[derive(Debug, Clone)]
pub struct HealthReport {
    /// Individual check results.
    pub checks: Vec<(String, CheckHealthStatus)>,
    /// `true` only when every check is `Healthy`.
    pub overall_healthy: bool,
}

impl fmt::Display for HealthReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = if self.overall_healthy { "HEALTHY" } else { "UNHEALTHY" };
        write!(f, "HealthReport({label}, {} checks)", self.checks.len())
    }
}

/// Collects named health-check callbacks and runs them on demand.
pub struct LifecycleHealthChecker {
    checks: Vec<(String, Box<dyn Fn() -> CheckHealthStatus>)>,
}

impl Default for LifecycleHealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl LifecycleHealthChecker {
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    /// Register a named health check.
    pub fn register_check(&mut self, name: &str, check: Box<dyn Fn() -> CheckHealthStatus>) {
        self.checks.push((name.to_string(), check));
    }

    /// Execute every registered check and return an aggregated report.
    pub fn run_all(&self) -> HealthReport {
        let results: Vec<(String, CheckHealthStatus)> = self
            .checks
            .iter()
            .map(|(name, check)| (name.clone(), check()))
            .collect();
        let overall_healthy = results.iter().all(|(_, s)| matches!(s, CheckHealthStatus::Healthy));
        HealthReport { checks: results, overall_healthy }
    }

    /// Convenience: returns `true` when every check is healthy.
    pub fn is_healthy(&self) -> bool {
        self.run_all().overall_healthy
    }
}

// ---------------------------------------------------------------------------
// LifecycleRecovery
// ---------------------------------------------------------------------------

/// Stores key-value recovery data for crash recovery.
#[derive(Debug, Clone, Default)]
pub struct LifecycleRecovery {
    state: HashMap<String, String>,
}

impl LifecycleRecovery {
    pub fn new() -> Self {
        Self::default()
    }

    /// Persist a piece of recovery state.
    pub fn save_state(&mut self, key: &str, data: &str) {
        self.state.insert(key.to_string(), data.to_string());
    }

    /// Retrieve previously saved state.
    pub fn get_state(&self, key: &str) -> Option<&str> {
        self.state.get(key).map(|s| s.as_str())
    }

    /// Remove a single key.
    pub fn remove_state(&mut self, key: &str) {
        self.state.remove(key);
    }

    /// Returns `true` if any recovery data has been saved.
    pub fn has_recovery_data(&self) -> bool {
        !self.state.is_empty()
    }

    /// Returns all stored keys.
    pub fn state_keys(&self) -> Vec<&str> {
        self.state.keys().map(|k| k.as_str()).collect()
    }

    /// Remove all recovery data.
    pub fn clear_all(&mut self) {
        self.state.clear();
    }
}

impl fmt::Display for LifecycleRecovery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LifecycleRecovery({} keys)", self.state.len())
    }
}

// ---------------------------------------------------------------------------
// LifecycleEventBus
// ---------------------------------------------------------------------------

/// A typed lifecycle event with a kind, timestamp, and optional payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleEvent {
    /// The kind/type of event (e.g. "phase_change", "shutdown").
    pub kind: String,
    /// Millisecond timestamp (relative or absolute).
    pub timestamp_ms: u64,
    /// Optional free-form data payload.
    pub data: Option<String>,
}

impl fmt::Display for LifecycleEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.data {
            Some(d) => write!(f, "[{}ms] {}: {}", self.timestamp_ms, self.kind, d),
            None => write!(f, "[{}ms] {}", self.timestamp_ms, self.kind),
        }
    }
}

impl Default for LifecycleEvent {
    fn default() -> Self {
        Self { kind: String::new(), timestamp_ms: 0, data: None }
    }
}

/// Simple typed event bus that records lifecycle events and allows querying.
#[derive(Debug, Default)]
pub struct LifecycleEventBus {
    events: Vec<LifecycleEvent>,
}

impl LifecycleEventBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Emit (record) a lifecycle event.
    pub fn emit(&mut self, event: LifecycleEvent) {
        self.events.push(event);
    }

    /// Full history of emitted events.
    pub fn history(&self) -> &[LifecycleEvent] {
        &self.events
    }

    /// The most recently emitted event, if any.
    pub fn last_event(&self) -> Option<&LifecycleEvent> {
        self.events.last()
    }

    /// Return all events matching a given kind string.
    pub fn events_of_kind(&self, kind: &str) -> Vec<&LifecycleEvent> {
        self.events.iter().filter(|e| e.kind == kind).collect()
    }
}

impl fmt::Display for LifecycleEventBus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LifecycleEventBus({} events)", self.events.len())
    }
}



// ─── LcBuf Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for lifecycle events.
#[derive(Debug, Clone)]
pub struct LcBufRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> LcBufRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for LcBufRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LcBufRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── LcBld Builder & Validator ─────────────────────────────

/// Builder for constructing lifecycle configurations.
#[derive(Debug, Clone)]
pub struct LcBldBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl LcBldBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<LcBldCfg, LcBldBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(LcBldBuildErr { errors }); }
        Ok(LcBldCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated lifecycle configuration.
#[derive(Debug, Clone)]
pub struct LcBldCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl LcBldCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &LcBldCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for LcBldCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LcBldCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct LcBldBuildErr { pub errors: Vec<String> }

impl fmt::Display for LcBldBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LcBldBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for LcBldBuildErr {}


/// Lifecycle service configuration manager.
#[derive(Debug, Clone)]
pub struct LifecycleSvcConfig {
    entries: Vec<LifecycleSvcEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single lifecycle service entry.
#[derive(Debug, Clone, PartialEq)]
pub struct LifecycleSvcEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl LifecycleSvcEntry {
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

impl LifecycleSvcConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: LifecycleSvcEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&LifecycleSvcEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut LifecycleSvcEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&LifecycleSvcEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&LifecycleSvcEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&LifecycleSvcEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<LifecycleSvcEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::time::Instant;

    #[test]
    fn initial_phase() {
        let svc = LifecycleService::new();
        assert_eq!(svc.phase(), LifecyclePhase::Starting);
    }

    #[test]
    fn phase_progression() {
        let svc = LifecycleService::new();
        svc.set_phase(LifecyclePhase::Ready);
        assert_eq!(svc.phase(), LifecyclePhase::Ready);
        svc.set_phase(LifecyclePhase::Restored);
        assert_eq!(svc.phase(), LifecyclePhase::Restored);
    }

    #[test]
    fn phase_cannot_go_backwards() {
        let svc = LifecycleService::new();
        svc.set_phase(LifecyclePhase::Restored);
        svc.set_phase(LifecyclePhase::Ready); // should be ignored
        assert_eq!(svc.phase(), LifecyclePhase::Restored);
    }

    #[test]
    fn shutdown_not_vetoed() {
        let svc = LifecycleService::new();
        let did_shutdown = Arc::new(AtomicBool::new(false));
        let did_shutdown2 = did_shutdown.clone();
        let _sub = svc.on_did_shutdown().on(move |_: &ShutdownReason| {
            did_shutdown2.store(true, Ordering::Relaxed);
        });
        assert!(svc.request_shutdown(ShutdownReason::Quit));
        assert!(did_shutdown.load(Ordering::Relaxed));
    }

    #[test]
    fn shutdown_vetoed() {
        let svc = LifecycleService::new();
        let _sub = svc.on_will_shutdown().on(move |evt: &WillShutdownEvent| {
            evt.veto();
        });
        assert!(!svc.request_shutdown(ShutdownReason::Close));
    }

    #[test]
    fn phase_name_returns_correct_strings() {
        assert_eq!(phase_name(LifecyclePhase::Starting), "Starting");
        assert_eq!(phase_name(LifecyclePhase::Ready), "Ready");
        assert_eq!(phase_name(LifecyclePhase::Restored), "Restored");
        assert_eq!(phase_name(LifecyclePhase::Eventually), "Eventually");
    }

    #[test]
    fn phase_is_terminal() {
        assert!(!LifecyclePhase::Starting.is_terminal());
        assert!(!LifecyclePhase::Ready.is_terminal());
        assert!(!LifecyclePhase::Restored.is_terminal());
        assert!(LifecyclePhase::Eventually.is_terminal());
    }

    #[test]
    fn phase_next() {
        assert_eq!(LifecyclePhase::Starting.next(), Some(LifecyclePhase::Ready));
        assert_eq!(LifecyclePhase::Ready.next(), Some(LifecyclePhase::Restored));
        assert_eq!(LifecyclePhase::Restored.next(), Some(LifecyclePhase::Eventually));
        assert_eq!(LifecyclePhase::Eventually.next(), None);
    }

    #[test]
    fn is_phase_at_least() {
        let svc = LifecycleService::new();
        assert!(svc.is_phase_at_least(LifecyclePhase::Starting));
        assert!(!svc.is_phase_at_least(LifecyclePhase::Ready));
        svc.set_phase(LifecyclePhase::Restored);
        assert!(svc.is_phase_at_least(LifecyclePhase::Starting));
        assert!(svc.is_phase_at_least(LifecyclePhase::Ready));
        assert!(svc.is_phase_at_least(LifecyclePhase::Restored));
        assert!(!svc.is_phase_at_least(LifecyclePhase::Eventually));
    }

    #[test]
    fn force_shutdown_skips_veto() {
        let svc = LifecycleService::new();
        let _sub = svc.on_will_shutdown().on(move |evt: &WillShutdownEvent| {
            evt.veto();
        });
        // Normal shutdown is vetoed
        assert!(!svc.request_shutdown(ShutdownReason::Quit));
        // Force shutdown ignores veto listeners entirely
        let did_shutdown = Arc::new(AtomicBool::new(false));
        let did_shutdown2 = did_shutdown.clone();
        let _sub2 = svc.on_did_shutdown().on(move |_: &ShutdownReason| {
            did_shutdown2.store(true, Ordering::Relaxed);
        });
        assert!(svc.force_shutdown(ShutdownReason::Kill));
        assert!(did_shutdown.load(Ordering::Relaxed));
    }

    #[test]
    fn stats_initial_values() {
        let svc = LifecycleService::new();
        let stats = svc.get_stats();
        assert_eq!(stats.phase_transition_count, 0);
        assert_eq!(stats.shutdown_attempts, 0);
        assert_eq!(stats.vetoed_shutdowns, 0);
        assert_eq!(stats.current_phase, LifecyclePhase::Starting);
    }

    #[test]
    fn stats_after_transitions() {
        let svc = LifecycleService::new();
        svc.set_phase(LifecyclePhase::Ready);
        svc.set_phase(LifecyclePhase::Restored);
        svc.set_phase(LifecyclePhase::Eventually);
        let stats = svc.get_stats();
        assert_eq!(stats.phase_transition_count, 3);
        assert_eq!(stats.current_phase, LifecyclePhase::Eventually);
    }

    #[test]
    fn stats_after_shutdown_attempts() {
        let svc = LifecycleService::new();
        let _sub = svc.on_will_shutdown().on(move |evt: &WillShutdownEvent| {
            evt.veto();
        });
        svc.request_shutdown(ShutdownReason::Quit);
        svc.request_shutdown(ShutdownReason::Close);
        let stats = svc.get_stats();
        assert_eq!(stats.shutdown_attempts, 2);
        assert_eq!(stats.vetoed_shutdowns, 2);
    }

    #[test]
    fn shutdown_attempt_count_accessor() {
        let svc = LifecycleService::new();
        assert_eq!(svc.shutdown_attempt_count(), 0);
        svc.request_shutdown(ShutdownReason::Quit);
        assert_eq!(svc.shutdown_attempt_count(), 1);
        svc.force_shutdown(ShutdownReason::Kill);
        assert_eq!(svc.shutdown_attempt_count(), 2);
    }

    #[test]
    fn vetoed_count_accessor() {
        let svc = LifecycleService::new();
        assert_eq!(svc.vetoed_count(), 0);
        let _sub = svc.on_will_shutdown().on(move |evt: &WillShutdownEvent| {
            evt.veto();
        });
        svc.request_shutdown(ShutdownReason::Quit);
        assert_eq!(svc.vetoed_count(), 1);
    }

    #[test]
    fn backward_phase_does_not_increment_count() {
        let svc = LifecycleService::new();
        svc.set_phase(LifecyclePhase::Restored);
        svc.set_phase(LifecyclePhase::Ready); // ignored
        assert_eq!(svc.get_stats().phase_transition_count, 1);
    }

    #[test]
    fn shutdown_event_struct() {
        let evt = ShutdownEvent {
            reason: ShutdownReason::Reload,
            phase_at_shutdown: LifecyclePhase::Restored,
            timestamp: Instant::now(),
        };
        assert_eq!(evt.reason, ShutdownReason::Reload);
        assert_eq!(evt.phase_at_shutdown, LifecyclePhase::Restored);
    }

    #[test]
    fn when_phase_already_reached() {
        let svc = LifecycleService::new();
        svc.set_phase(LifecyclePhase::Ready);
        let called = Arc::new(AtomicBool::new(false));
        let called2 = called.clone();
        svc.when_phase(LifecyclePhase::Starting, move || {
            called2.store(true, Ordering::Relaxed);
        });
        assert!(called.load(Ordering::Relaxed));
    }

    #[test]
    fn eq_lifecyclephase_same() {
        assert_eq!(LifecyclePhase::Starting, LifecyclePhase::Starting);
    }

    #[test]
    fn ne_lifecyclephase_diff() {
        assert_ne!(LifecyclePhase::Starting, LifecyclePhase::Ready);
    }

    #[test]
    fn eq_shutdownreason_same() {
        assert_eq!(ShutdownReason::Quit, ShutdownReason::Quit);
    }

    #[test]
    fn ne_shutdownreason_diff() {
        assert_ne!(ShutdownReason::Quit, ShutdownReason::Close);
    }

    #[test]
    fn shutdown_barrier_register_and_complete() {
        let mut barrier = ShutdownBarrier::new();
        barrier.register("save_files");
        barrier.register("flush_logs");
        assert_eq!(barrier.remaining(), 2);
        assert!(!barrier.is_clear());
        assert!(barrier.complete("save_files"));
        assert_eq!(barrier.remaining(), 1);
        assert!(!barrier.complete("nonexistent"));
        assert!(barrier.complete("flush_logs"));
        assert!(barrier.is_clear());
    }

    #[test]
    fn shutdown_barrier_pending_names() {
        let mut barrier = ShutdownBarrier::new();
        barrier.register("task_a");
        barrier.register("task_b");
        let mut names = barrier.pending_names();
        names.sort();
        assert_eq!(names, vec!["task_a", "task_b"]);
    }

    #[test]
    fn shutdown_barrier_duplicate_register() {
        let mut barrier = ShutdownBarrier::new();
        barrier.register("task");
        barrier.register("task");
        assert_eq!(barrier.remaining(), 1);
    }

    #[test]
    fn startup_phase_ordering() {
        assert!(StartupPhase::EarlyInit < StartupPhase::ServiceInit);
        assert!(StartupPhase::ServiceInit < StartupPhase::ExtensionLoad);
        assert!(StartupPhase::ExtensionLoad < StartupPhase::Ready);
    }

    #[test]
    fn startup_phase_display_and_label() {
        assert_eq!(StartupPhase::EarlyInit.phase_label(), "Early Init");
        assert_eq!(format!("{}", StartupPhase::Ready), "Ready");
        assert_eq!(format!("{}", StartupPhase::ExtensionLoad), "Extension Load");
    }

    #[test]
    fn startup_phase_is_complete() {
        assert!(!StartupPhase::EarlyInit.is_complete());
        assert!(!StartupPhase::ServiceInit.is_complete());
        assert!(!StartupPhase::ExtensionLoad.is_complete());
        assert!(StartupPhase::Ready.is_complete());
    }

    #[test]
    fn lifecycle_timeline_record_and_total() {
        let mut timeline = LifecycleTimeline::new();
        timeline.record(StartupPhase::EarlyInit, "config", 50);
        timeline.record(StartupPhase::ServiceInit, "db_connect", 200);
        timeline.record(StartupPhase::ExtensionLoad, "ext_a", 100);
        assert_eq!(timeline.entry_count(), 3);
        assert_eq!(timeline.total_duration_ms(), 350);
    }

    #[test]
    fn lifecycle_timeline_filter_by_phase() {
        let mut timeline = LifecycleTimeline::new();
        timeline.record(StartupPhase::EarlyInit, "a", 10);
        timeline.record(StartupPhase::EarlyInit, "b", 20);
        timeline.record(StartupPhase::ServiceInit, "c", 30);
        let early = timeline.entries_for_phase(StartupPhase::EarlyInit);
        assert_eq!(early.len(), 2);
        let svc = timeline.entries_for_phase(StartupPhase::ServiceInit);
        assert_eq!(svc.len(), 1);
        assert_eq!(svc[0].label, "c");
    }

    #[test]
    fn lifecycle_timeline_slowest_entry() {
        let mut timeline = LifecycleTimeline::new();
        timeline.record(StartupPhase::EarlyInit, "fast", 10);
        timeline.record(StartupPhase::ServiceInit, "slow", 500);
        timeline.record(StartupPhase::ExtensionLoad, "medium", 100);
        let slowest = timeline.slowest_entry().unwrap();
        assert_eq!(slowest.label, "slow");
        assert_eq!(slowest.duration_ms, 500);
    }

    #[test]
    fn lifecycle_timeline_summary() {
        let mut timeline = LifecycleTimeline::new();
        timeline.record(StartupPhase::EarlyInit, "config", 50);
        let summary = timeline.to_summary();
        assert!(summary.contains("1 entries"));
        assert!(summary.contains("total=50ms"));
        assert!(summary.contains("[Early Init] config (50ms)"));
    }

    #[test]
    fn lifecycle_timeline_empty() {
        let timeline = LifecycleTimeline::new();
        assert_eq!(timeline.entry_count(), 0);
        assert_eq!(timeline.total_duration_ms(), 0);
        assert!(timeline.slowest_entry().is_none());
    }

    #[test]
    fn lifecycle_svc_stats_new_defaults() {
        let stats = LifecycleSvcStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn lifecycle_svc_stats_record_success() {
        let mut stats = LifecycleSvcStats::new();
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
    fn lifecycle_svc_stats_record_failure() {
        let mut stats = LifecycleSvcStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn lifecycle_svc_stats_reset() {
        let mut stats = LifecycleSvcStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn lifecycle_svc_stats_merge() {
        let mut a = LifecycleSvcStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = LifecycleSvcStats::new();
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
    fn lifecycle_svc_stats_display() {
        let mut stats = LifecycleSvcStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn lifecycle_svc_stats_default() {
        let stats = LifecycleSvcStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn lifecycle_svc_validator_accepts_valid_name() {
        let v = LifecycleSvcValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn lifecycle_svc_validator_rejects_empty() {
        let v = LifecycleSvcValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn lifecycle_svc_validator_rejects_too_long() {
        let v = LifecycleSvcValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn lifecycle_svc_validator_forbidden_prefix() {
        let v = LifecycleSvcValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn lifecycle_svc_validator_allowed_chars() {
        let v = LifecycleSvcValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn lifecycle_svc_validator_range() {
        let v = LifecycleSvcValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn lifecycle_svc_sanitize_removes_control() {
        let result = LifecycleSvcValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn lifecycle_svc_truncate_short_string() {
        assert_eq!(LifecycleSvcValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn lifecycle_svc_truncate_long_string() {
        let result = LifecycleSvcValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn lifecycle_svc_is_ascii_printable() {
        assert!(LifecycleSvcValidator::is_ascii_printable("Hello World 123"));
        assert!(!LifecycleSvcValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn test_service_restart_from_shutdown() {
        let svc = LifecycleService::new();
        svc.set_phase(LifecyclePhase::Eventually);
        assert!(svc.request_shutdown(ShutdownReason::Quit));
        assert!(svc.is_shut_down());
        assert!(svc.service_restart().is_ok());
        assert_eq!(svc.phase(), LifecyclePhase::Starting);
        assert!(!svc.is_shut_down());
    }

    #[test]
    fn test_service_restart_not_shutdown_fails() {
        let svc = LifecycleService::new();
        let result = svc.service_restart();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "service is not shut down");
    }

    #[test]
    fn test_health_check_healthy() {
        let svc = LifecycleService::new();
        svc.set_phase(LifecyclePhase::Ready);
        let status = lifecycle_health_check(&svc);
        assert_eq!(status.phase_name, "Ready");
        assert!(status.is_healthy);
        assert_eq!(status.uptime_events, 0);
        assert_eq!(status.barrier_count, 0);
    }

    #[test]
    fn test_health_check_shutdown() {
        let svc = LifecycleService::new();
        assert!(svc.request_shutdown(ShutdownReason::Quit));
        let status = lifecycle_health_check(&svc);
        assert!(!status.is_healthy);
    }

    #[test]
    fn test_barrier_count() {
        let svc = LifecycleService::new();
        assert_eq!(svc.barrier_count(), 0);
        svc.register_barrier(ShutdownBarrier::new());
        assert_eq!(svc.barrier_count(), 1);
        svc.register_barrier(ShutdownBarrier::new());
        assert_eq!(svc.barrier_count(), 2);
    }

    #[test]
    fn test_health_check_with_barriers() {
        let svc = LifecycleService::new();
        svc.register_barrier(ShutdownBarrier::new());
        svc.register_barrier(ShutdownBarrier::new());
        svc.timeline().record(StartupPhase::EarlyInit, "init", 10);
        let status = lifecycle_health_check(&svc);
        assert!(status.is_healthy);
        assert_eq!(status.barrier_count, 2);
        assert_eq!(status.uptime_events, 1);
    }

    #[test]
    fn test_phase_iter() {
        let phases: Vec<LifecyclePhase> = PhaseIter::new().collect();
        assert_eq!(phases.len(), 4);
        assert_eq!(phases[0], LifecyclePhase::Starting);
        assert_eq!(phases[3], LifecyclePhase::Eventually);
    }

    #[test]
    fn test_phase_iter_exact_size() {
        let iter = PhaseIter::new();
        assert_eq!(iter.len(), 4);
    }

    #[test]
    fn test_lifecycle_phase_index() {
        assert_eq!(LifecyclePhase::Starting.index(), 0);
        assert_eq!(LifecyclePhase::Eventually.index(), 3);
    }

    #[test]
    fn test_lifecycle_phase_all() {
        assert_eq!(LifecyclePhase::all().len(), 4);
    }

    #[test]
    fn test_lifecycle_phase_from_name() {
        assert_eq!(LifecyclePhase::from_name("ready"), Some(LifecyclePhase::Ready));
        assert_eq!(LifecyclePhase::from_name("STARTING"), Some(LifecyclePhase::Starting));
        assert_eq!(LifecyclePhase::from_name("bogus"), None);
    }

    #[test]
    fn test_lifecycle_phase_name() {
        assert_eq!(LifecyclePhase::Ready.name(), "ready");
        assert_eq!(LifecyclePhase::Eventually.name(), "eventually");
    }

    #[test]
    fn test_startup_phase_all() {
        assert_eq!(StartupPhase::all().len(), 4);
    }

    #[test]
    fn test_startup_phase_ordinal() {
        assert_eq!(StartupPhase::EarlyInit.ordinal(), 0);
        assert_eq!(StartupPhase::Ready.ordinal(), 3);
    }

    #[test]
    fn test_format_duration_ms() {
        assert_eq!(format_duration_ms(500), "500ms");
        assert_eq!(format_duration_ms(2500), "2.5s");
        assert_eq!(format_duration_ms(90000), "1m 30s");
    }

    #[test]
    fn test_lifecycle_timeline_total_duration() {
        let mut tl = LifecycleTimeline::default();
        tl.record(StartupPhase::EarlyInit, "start", 100);
        tl.record(StartupPhase::EarlyInit, "end", 350);
        assert_eq!(tl.total_span_ms(), Some(250));
    }

    #[test]
    fn test_lifecycle_timeline_summary() {
        let mut tl = LifecycleTimeline::default();
        tl.record(StartupPhase::EarlyInit, "alpha", 0);
        tl.record(StartupPhase::EarlyInit, "beta", 100);
        let s = tl.summary();
        assert!(s.contains("alpha"));
        assert!(s.contains("beta"));
    }

    // --- new tests ---

    #[test]
    fn hook_registry_register_and_query() {
        let mut reg = HookRegistry::new();
        assert!(reg.register(LifecycleHook::new("init-db", LifecyclePhase::Ready, "open database")));
        assert!(reg.register(LifecycleHook::new("load-ui", LifecyclePhase::Restored, "render UI")));
        assert!(!reg.register(LifecycleHook::new("init-db", LifecyclePhase::Ready, "dup")));
        assert_eq!(reg.len(), 2);
        assert_eq!(reg.hooks_for_phase(LifecyclePhase::Ready).len(), 1);
        assert_eq!(reg.hooks_for_phase(LifecyclePhase::Starting).len(), 0);
    }

    #[test]
    fn hook_registry_unregister() {
        let mut reg = HookRegistry::new();
        reg.register(LifecycleHook::new("a", LifecyclePhase::Starting, "desc"));
        assert!(reg.unregister("a"));
        assert!(!reg.unregister("a"));
        assert!(reg.is_empty());
    }

    #[test]
    fn lifecycle_metrics_record_and_stats() {
        let mut m = LifecycleMetrics::new();
        m.record(LifecyclePhase::Starting, LifecyclePhase::Ready, 100);
        m.record(LifecyclePhase::Ready, LifecyclePhase::Restored, 200);
        assert_eq!(m.total_startup_ms(), 300);
        assert!((m.average_transition_ms().unwrap() - 150.0).abs() < f64::EPSILON);
        assert_eq!(m.len(), 2);
        let display = format!("{m}");
        assert!(display.contains("300ms"));
    }

    #[test]
    fn lifecycle_metrics_empty() {
        let m = LifecycleMetrics::new();
        assert!(m.average_transition_ms().is_none());
        assert_eq!(m.total_startup_ms(), 0);
        assert!(m.is_empty());
    }

    #[test]
    fn shutdown_guard_acquire_release() {
        let mut guard = ShutdownGuard::new();
        assert!(guard.can_shutdown());
        assert!(guard.acquire("save-file"));
        assert!(!guard.can_shutdown());
        assert_eq!(guard.outstanding(), 1);
        assert!(!guard.acquire("save-file")); // duplicate
        assert!(guard.release("save-file"));
        assert!(guard.can_shutdown());
    }

    #[test]
    fn lifecycle_hook_display() {
        let h = LifecycleHook::new("db", LifecyclePhase::Ready, "open db");
        let s = format!("{h}");
        assert!(s.contains("Ready"));
        assert!(s.contains("db"));
    }

    // -----------------------------------------------------------------------
    // New tests for deepened functionality
    // -----------------------------------------------------------------------

    #[test]
    fn shutdown_reason_is_user_initiated() {
        assert!(ShutdownReason::Quit.is_user_initiated());
        assert!(ShutdownReason::Close.is_user_initiated());
        assert!(!ShutdownReason::Reload.is_user_initiated());
        assert!(!ShutdownReason::Kill.is_user_initiated());
    }

    #[test]
    fn shutdown_reason_is_destructive() {
        assert!(ShutdownReason::Kill.is_destructive());
        assert!(!ShutdownReason::Quit.is_destructive());
        assert!(!ShutdownReason::Close.is_destructive());
        assert!(!ShutdownReason::Reload.is_destructive());
    }

    #[test]
    fn shutdown_reason_is_recoverable() {
        assert!(ShutdownReason::Reload.is_recoverable());
        assert!(!ShutdownReason::Quit.is_recoverable());
        assert!(!ShutdownReason::Kill.is_recoverable());
    }

    #[test]
    fn shutdown_reason_label_and_display() {
        assert_eq!(ShutdownReason::Quit.label(), "Quit");
        assert_eq!(ShutdownReason::Close.label(), "Close");
        assert_eq!(ShutdownReason::Reload.label(), "Reload");
        assert_eq!(ShutdownReason::Kill.label(), "Kill");
        assert_eq!(format!("{}", ShutdownReason::Quit), "Quit");
    }

    #[test]
    fn shutdown_reason_all() {
        let all = ShutdownReason::all();
        assert_eq!(all.len(), 4);
        assert!(all.contains(&ShutdownReason::Quit));
        assert!(all.contains(&ShutdownReason::Kill));
    }

    #[test]
    fn will_shutdown_event_is_vetoable() {
        let evt_quit = WillShutdownEvent::new(ShutdownReason::Quit);
        assert!(evt_quit.is_vetoable());
        let evt_kill = WillShutdownEvent::new(ShutdownReason::Kill);
        assert!(!evt_kill.is_vetoable());
    }

    #[test]
    fn will_shutdown_event_try_veto() {
        let evt = WillShutdownEvent::new(ShutdownReason::Close);
        assert!(evt.try_veto());
        assert!(evt.is_vetoed());

        let evt_kill = WillShutdownEvent::new(ShutdownReason::Kill);
        assert!(!evt_kill.try_veto());
        assert!(!evt_kill.is_vetoed());
    }

    #[test]
    fn will_shutdown_event_is_user_initiated() {
        let evt = WillShutdownEvent::new(ShutdownReason::Quit);
        assert!(evt.is_user_initiated());
        let evt2 = WillShutdownEvent::new(ShutdownReason::Reload);
        assert!(!evt2.is_user_initiated());
    }

    #[test]
    fn shutdown_barrier_contains() {
        let mut barrier = ShutdownBarrier::new();
        barrier.register("alpha");
        assert!(barrier.contains("alpha"));
        assert!(!barrier.contains("beta"));
    }

    #[test]
    fn shutdown_barrier_register_all() {
        let mut barrier = ShutdownBarrier::new();
        barrier.register_all(&["x", "y", "z"]);
        assert_eq!(barrier.remaining(), 3);
        assert!(barrier.contains("x"));
        assert!(barrier.contains("y"));
        assert!(barrier.contains("z"));
    }

    #[test]
    fn shutdown_barrier_complete_all() {
        let mut barrier = ShutdownBarrier::new();
        barrier.register_all(&["a", "b", "c"]);
        let cleared = barrier.complete_all();
        assert_eq!(cleared, 3);
        assert!(barrier.is_clear());
    }

    #[test]
    fn timeline_entry_is_slow() {
        let entry = TimelineEntry {
            phase: StartupPhase::EarlyInit,
            label: "test".to_string(),
            duration_ms: 500,
        };
        assert!(entry.is_slow(100));
        assert!(!entry.is_slow(500));
        assert!(!entry.is_slow(1000));
    }

    #[test]
    fn timeline_entry_formatted_duration() {
        let entry = TimelineEntry {
            phase: StartupPhase::EarlyInit,
            label: "test".to_string(),
            duration_ms: 2500,
        };
        assert_eq!(entry.formatted_duration(), "2.5s");
    }

    #[test]
    fn timeline_entry_one_line_summary() {
        let entry = TimelineEntry {
            phase: StartupPhase::ServiceInit,
            label: "connect_db".to_string(),
            duration_ms: 150,
        };
        let s = entry.one_line_summary();
        assert!(s.contains("Service Init"));
        assert!(s.contains("connect_db"));
        assert!(s.contains("150ms"));
    }

    #[test]
    fn lifecycle_timeline_slow_entries() {
        let mut tl = LifecycleTimeline::new();
        tl.record(StartupPhase::EarlyInit, "fast", 10);
        tl.record(StartupPhase::ServiceInit, "slow", 500);
        tl.record(StartupPhase::ExtensionLoad, "medium", 100);
        let slow = tl.slow_entries(200);
        assert_eq!(slow.len(), 1);
        assert_eq!(slow[0].label, "slow");
    }

    #[test]
    fn lifecycle_timeline_fastest_entry() {
        let mut tl = LifecycleTimeline::new();
        tl.record(StartupPhase::EarlyInit, "a", 50);
        tl.record(StartupPhase::ServiceInit, "b", 10);
        tl.record(StartupPhase::ExtensionLoad, "c", 200);
        let fastest = tl.fastest_entry().unwrap();
        assert_eq!(fastest.label, "b");
        assert_eq!(fastest.duration_ms, 10);
    }

    #[test]
    fn lifecycle_timeline_count_by_phase() {
        let mut tl = LifecycleTimeline::new();
        tl.record(StartupPhase::EarlyInit, "a", 10);
        tl.record(StartupPhase::EarlyInit, "b", 20);
        tl.record(StartupPhase::ServiceInit, "c", 30);
        let counts = tl.count_by_phase();
        assert_eq!(counts.len(), 2);
        let early = counts.iter().find(|(p, _)| *p == StartupPhase::EarlyInit).unwrap();
        assert_eq!(early.1, 2);
        let svc = counts.iter().find(|(p, _)| *p == StartupPhase::ServiceInit).unwrap();
        assert_eq!(svc.1, 1);
    }

    #[test]
    fn lifecycle_timeline_average_duration() {
        let mut tl = LifecycleTimeline::new();
        assert!(tl.average_duration_ms().is_none());
        tl.record(StartupPhase::EarlyInit, "a", 100);
        tl.record(StartupPhase::ServiceInit, "b", 300);
        assert_eq!(tl.average_duration_ms(), Some(200));
    }

    #[test]
    fn lifecycle_stats_veto_rate() {
        let svc = LifecycleService::new();
        let _sub = svc.on_will_shutdown().on(move |evt: &WillShutdownEvent| {
            evt.veto();
        });
        svc.request_shutdown(ShutdownReason::Quit);
        svc.request_shutdown(ShutdownReason::Close);
        let stats = svc.get_stats();
        assert!((stats.veto_rate() - 1.0).abs() < f64::EPSILON);
        assert!(stats.has_shutdown_history());
        assert_eq!(stats.successful_shutdowns(), 0);
    }

    #[test]
    fn lifecycle_stats_has_progressed() {
        let svc = LifecycleService::new();
        let stats = svc.get_stats();
        assert!(!stats.has_progressed());
        svc.set_phase(LifecyclePhase::Ready);
        let stats = svc.get_stats();
        assert!(stats.has_progressed());
    }

    #[test]
    fn lifecycle_stats_successful_shutdowns() {
        let svc = LifecycleService::new();
        let stats = svc.get_stats();
        assert_eq!(stats.successful_shutdowns(), 0);
        assert!(!stats.has_shutdown_history());
        assert!((stats.veto_rate() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn health_status_has_barriers() {
        let svc = LifecycleService::new();
        let status = lifecycle_health_check(&svc);
        assert!(!status.has_barriers());

        svc.register_barrier(ShutdownBarrier::new());
        let status = lifecycle_health_check(&svc);
        assert!(status.has_barriers());
    }

    #[test]
    fn health_status_summary_and_display() {
        let svc = LifecycleService::new();
        svc.set_phase(LifecyclePhase::Ready);
        let status = lifecycle_health_check(&svc);
        let summary = status.summary();
        assert!(summary.contains("phase=Ready"));
        assert!(summary.contains("healthy"));
        let display = format!("{status}");
        assert_eq!(summary, display);
    }

    #[test]
    fn startup_phase_next() {
        assert_eq!(StartupPhase::EarlyInit.next(), Some(StartupPhase::ServiceInit));
        assert_eq!(StartupPhase::ServiceInit.next(), Some(StartupPhase::ExtensionLoad));
        assert_eq!(StartupPhase::ExtensionLoad.next(), Some(StartupPhase::Ready));
        assert_eq!(StartupPhase::Ready.next(), None);
    }

    #[test]
    fn startup_phase_previous() {
        assert_eq!(StartupPhase::EarlyInit.previous(), None);
        assert_eq!(StartupPhase::ServiceInit.previous(), Some(StartupPhase::EarlyInit));
        assert_eq!(StartupPhase::ExtensionLoad.previous(), Some(StartupPhase::ServiceInit));
        assert_eq!(StartupPhase::Ready.previous(), Some(StartupPhase::ExtensionLoad));
    }

    // --- LifecycleHookRegistry tests ---

    #[test]
    fn hook_registry_register_and_run() {
        let mut reg = LifecycleHookRegistry::new();
        reg.register("startup", 2, "second", Box::new(|| Ok(())));
        reg.register("startup", 1, "first", Box::new(|| Ok(())));
        reg.register("shutdown", 0, "cleanup", Box::new(|| Err("disk full".into())));
        assert_eq!(reg.hook_count(), 3);
        assert_eq!(reg.hooks_for_phase("startup"), 2);
        assert_eq!(reg.hooks_for_phase("shutdown"), 1);
        assert_eq!(reg.hooks_for_phase("missing"), 0);

        let results = reg.run_hooks("startup");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "first");
        assert!(results[0].success);
        assert_eq!(results[1].name, "second");
        assert!(results[1].success);
    }

    #[test]
    fn hook_registry_priority_ordering() {
        let mut reg = LifecycleHookRegistry::new();
        reg.register("init", 10, "late", Box::new(|| Ok(())));
        reg.register("init", -5, "early", Box::new(|| Ok(())));
        reg.register("init", 0, "middle", Box::new(|| Ok(())));
        let results = reg.run_hooks("init");
        let names: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["early", "middle", "late"]);
    }

    #[test]
    fn hook_registry_failure_captured() {
        let mut reg = LifecycleHookRegistry::new();
        reg.register("phase", 0, "bad", Box::new(|| Err("boom".into())));
        let results = reg.run_hooks("phase");
        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
        assert_eq!(results[0].error.as_deref(), Some("boom"));
        let display = format!("{}", results[0]);
        assert!(display.contains("FAILED"));
    }

    #[test]
    fn hook_registry_default() {
        let reg = LifecycleHookRegistry::default();
        assert_eq!(reg.hook_count(), 0);
    }

    // --- LifecycleHealthChecker tests ---

    #[test]
    fn health_checker_all_healthy() {
        let mut hc = LifecycleHealthChecker::new();
        hc.register_check("cpu", Box::new(|| CheckHealthStatus::Healthy));
        hc.register_check("mem", Box::new(|| CheckHealthStatus::Healthy));
        assert!(hc.is_healthy());
        let report = hc.run_all();
        assert!(report.overall_healthy);
        assert_eq!(report.checks.len(), 2);
    }

    #[test]
    fn health_checker_degraded_not_healthy() {
        let mut hc = LifecycleHealthChecker::new();
        hc.register_check("disk", Box::new(|| CheckHealthStatus::Degraded("90% full".into())));
        assert!(!hc.is_healthy());
        let report = hc.run_all();
        assert!(!report.overall_healthy);
        assert_eq!(report.checks[0].1, CheckHealthStatus::Degraded("90% full".into()));
    }

    #[test]
    fn health_checker_display() {
        let mut hc = LifecycleHealthChecker::new();
        hc.register_check("net", Box::new(|| CheckHealthStatus::Unhealthy("timeout".into())));
        let report = hc.run_all();
        let s = format!("{report}");
        assert!(s.contains("UNHEALTHY"));
        let status_s = format!("{}", CheckHealthStatus::Unhealthy("timeout".into()));
        assert!(status_s.contains("unhealthy"));
    }

    #[test]
    fn health_status_default() {
        assert_eq!(CheckHealthStatus::default(), CheckHealthStatus::Healthy);
    }

    // --- LifecycleRecovery tests ---

    #[test]
    fn recovery_save_get_remove() {
        let mut rec = LifecycleRecovery::new();
        assert!(!rec.has_recovery_data());
        rec.save_state("cursor", "line=42,col=10");
        rec.save_state("scroll", "top=100");
        assert!(rec.has_recovery_data());
        assert_eq!(rec.get_state("cursor"), Some("line=42,col=10"));
        assert_eq!(rec.get_state("missing"), None);
        assert_eq!(rec.state_keys().len(), 2);

        rec.remove_state("cursor");
        assert_eq!(rec.get_state("cursor"), None);
        assert!(rec.has_recovery_data());

        rec.clear_all();
        assert!(!rec.has_recovery_data());
    }

    #[test]
    fn recovery_display() {
        let mut rec = LifecycleRecovery::new();
        rec.save_state("a", "1");
        let s = format!("{rec}");
        assert!(s.contains("1 keys"));
    }

    // --- LifecycleEventBus tests ---

    #[test]
    fn event_bus_emit_and_query() {
        let mut bus = LifecycleEventBus::new();
        assert!(bus.history().is_empty());
        assert!(bus.last_event().is_none());

        bus.emit(LifecycleEvent { kind: "phase_change".into(), timestamp_ms: 100, data: Some("Ready".into()) });
        bus.emit(LifecycleEvent { kind: "shutdown".into(), timestamp_ms: 200, data: None });
        bus.emit(LifecycleEvent { kind: "phase_change".into(), timestamp_ms: 300, data: Some("Restored".into()) });

        assert_eq!(bus.history().len(), 3);
        assert_eq!(bus.last_event().unwrap().kind, "phase_change");
        assert_eq!(bus.events_of_kind("phase_change").len(), 2);
        assert_eq!(bus.events_of_kind("shutdown").len(), 1);
        assert_eq!(bus.events_of_kind("other").len(), 0);
    }

    #[test]
    fn event_bus_display() {
        let mut bus = LifecycleEventBus::new();
        bus.emit(LifecycleEvent::default());
        let s = format!("{bus}");
        assert!(s.contains("1 events"));
    }

    #[test]
    fn lifecycle_event_display() {
        let with_data = LifecycleEvent { kind: "init".into(), timestamp_ms: 42, data: Some("ok".into()) };
        assert!(format!("{with_data}").contains("[42ms] init: ok"));
        let without = LifecycleEvent { kind: "done".into(), timestamp_ms: 99, data: None };
        assert!(format!("{without}").contains("[99ms] done"));
    }

    #[test]
    fn startup_phase_from_label() {
        assert_eq!(StartupPhase::from_label("early init"), Some(StartupPhase::EarlyInit));
        assert_eq!(StartupPhase::from_label("Early_Init"), Some(StartupPhase::EarlyInit));
        assert_eq!(StartupPhase::from_label("service_init"), Some(StartupPhase::ServiceInit));
        assert_eq!(StartupPhase::from_label("Extension Load"), Some(StartupPhase::ExtensionLoad));
        assert_eq!(StartupPhase::from_label("Ready"), Some(StartupPhase::Ready));
        assert_eq!(StartupPhase::from_label("bogus"), None);
    }


    #[test]
    fn lcbuf_ringbuf_push_get() {
        let mut rb = LcBufRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn lcbuf_ringbuf_overflow() {
        let mut rb = LcBufRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn lcbuf_ringbuf_clear() {
        let mut rb = LcBufRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn lcbuf_ringbuf_newest_oldest() {
        let mut rb = LcBufRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn lcbuf_ringbuf_to_vec() {
        let mut rb = LcBufRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn lcbuf_ringbuf_is_full() {
        let mut rb = LcBufRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn lcbld_builder_valid() {
        let cfg = LcBldBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn lcbld_builder_empty_name() {
        let r = LcBldBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn lcbld_builder_bad_priority() {
        assert!(LcBldBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn lcbld_builder_zero_max() {
        assert!(LcBldBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn lcbld_cfg_merge() {
        let mut a = LcBldBuilder::new("a").property("x", "1").build().unwrap();
        let b = LcBldBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn lcbld_cfg_display() {
        let cfg = LcBldBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }


    #[test]
    fn lifecycle_svc_entry_creation() {
        let e = LifecycleSvcEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn lifecycle_svc_entry_with_priority() {
        let e = LifecycleSvcEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn lifecycle_svc_entry_metadata() {
        let e = LifecycleSvcEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn lifecycle_svc_entry_remove_meta() {
        let mut e = LifecycleSvcEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn lifecycle_svc_entry_activate_deactivate() {
        let mut e = LifecycleSvcEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn lifecycle_svc_config_add_sorted() {
        let mut c = LifecycleSvcConfig::new(10);
        c.add(LifecycleSvcEntry::new("lo", "Lo").with_priority(1));
        c.add(LifecycleSvcEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn lifecycle_svc_config_capacity() {
        let mut c = LifecycleSvcConfig::new(1);
        assert!(c.add(LifecycleSvcEntry::new("a", "A")));
        assert!(!c.add(LifecycleSvcEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn lifecycle_svc_config_remove() {
        let mut c = LifecycleSvcConfig::new(10);
        c.add(LifecycleSvcEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn lifecycle_svc_config_get() {
        let mut c = LifecycleSvcConfig::new(10);
        c.add(LifecycleSvcEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn lifecycle_svc_config_active_entries() {
        let mut c = LifecycleSvcConfig::new(10);
        c.add(LifecycleSvcEntry::new("a", "A"));
        c.add(LifecycleSvcEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn lifecycle_svc_config_enable_disable() {
        let mut c = LifecycleSvcConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn lifecycle_svc_config_clear() {
        let mut c = LifecycleSvcConfig::new(10);
        c.add(LifecycleSvcEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn lifecycle_svc_config_find_by_label() {
        let mut c = LifecycleSvcConfig::new(10);
        c.add(LifecycleSvcEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn lifecycle_svc_config_top_n() {
        let mut c = LifecycleSvcConfig::new(10);
        c.add(LifecycleSvcEntry::new("a", "A").with_priority(1));
        c.add(LifecycleSvcEntry::new("b", "B").with_priority(2));
        c.add(LifecycleSvcEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn lifecycle_svc_config_deactivate_activate_all() {
        let mut c = LifecycleSvcConfig::new(10);
        c.add(LifecycleSvcEntry::new("a", "A"));
        c.add(LifecycleSvcEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn lifecycle_svc_config_highest_priority() {
        let mut c = LifecycleSvcConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(LifecycleSvcEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn lifecycle_svc_config_contains() {
        let mut c = LifecycleSvcConfig::new(10);
        c.add(LifecycleSvcEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn lifecycle_svc_config_labels() {
        let mut c = LifecycleSvcConfig::new(10);
        c.add(LifecycleSvcEntry::new("a", "Alpha"));
        c.add(LifecycleSvcEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn lifecycle_svc_config_drain_inactive() {
        let mut c = LifecycleSvcConfig::new(10);
        c.add(LifecycleSvcEntry::new("a", "A"));
        c.add(LifecycleSvcEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }

}