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
// xa_ extended helpers for lifecycle_svc
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaLifecycleSvcRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaLifecycleSvcRingBuf {
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
pub struct XaLifecycleSvcCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaLifecycleSvcCounter {
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

impl Default for XaLifecycleSvcCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 111
// ---------------------------------------------------------------------------

/// Generic object pool `Xc111Pool<T>`.
pub struct Xc111Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc111Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc111PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc111Pool<T> {
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
    pub fn stats(&self) -> Xc111PoolStats {
        Xc111PoolStats {
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

impl<T> Default for Xc111Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc111Scheduler`.
pub struct Xc111Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc111Scheduler {
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

impl Default for Xc111Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_111 hash for the given byte slice.
pub fn xc_111_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_111 convention.
pub fn xc_111_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_69 deepening: state machine + event bus ---

/// States for the Xd69 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd69State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd69State {
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
pub struct Xd69Transition {
    pub from: Xd69State,
    pub to: Xd69State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd69StateMachine {
    current: Xd69State,
    history: Vec<Xd69Transition>,
    step_counter: usize,
}

impl Xd69StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd69State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd69State {
        self.current
    }

    pub fn history(&self) -> &[Xd69Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd69State) -> Result<Xd69State, String> {
        let allowed = match (self.current, target) {
            (Xd69State::Idle, Xd69State::Running) => true,
            (Xd69State::Running, Xd69State::Paused) => true,
            (Xd69State::Running, Xd69State::Done) => true,
            (Xd69State::Paused, Xd69State::Running) => true,
            (Xd69State::Paused, Xd69State::Done) => true,
            (Xd69State::Done, Xd69State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_69: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd69Transition {
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
            "Xd69SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd69State> {
        let prefix = "Xd69SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd69State::Idle),
            "Running" => Some(Xd69State::Running),
            "Paused" => Some(Xd69State::Paused),
            "Done" => Some(Xd69State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd69State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd69 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd69Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd69Event {
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

type Xd69HandlerFn = Box<dyn Fn(&Xd69Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd69EventBus {
    handlers: Vec<(usize, Option<String>, Xd69HandlerFn)>,
    next_id: usize,
    published: Vec<Xd69Event>,
}

impl Xd69EventBus {
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
        F: Fn(&Xd69Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd69Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd69Event) {
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

    pub fn published_events(&self) -> &[Xd69Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #78
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf78Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf78TrieNode {
    children: std::collections::HashMap<char, Xf78TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf78Trie {
    root: Xf78TrieNode,
    count: usize,
}

impl Xf78Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf78TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf78TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf78TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf78BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf78BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 110).
pub struct Xh110SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh110SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 152 as u64,
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

/// A compact bit set supporting boolean operations (variant 110).
pub struct Xh110BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh110BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 110).
pub struct Xi110Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi110Deque<T> {
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
pub struct Xi110Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi110Interval {
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

/// A simple interval tree (variant 110).
pub struct Xi110IntervalTree {
    xi_intervals: Vec<Xi110Interval>,
}

impl Xi110IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi110Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi110Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi110Interval) -> Vec<&Xi110Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi110Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi110Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi110Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi110Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi110Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi110Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 111) ---

/// Disjoint set / union-find for crate 111.
pub struct Xj111UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj111UnionFind {
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

const XJ111_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 111.
pub struct Xj111BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj111BTreeNode<K, V>>>,
    len: usize,
}

struct Xj111BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj111BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj111BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ111_BTREE_ORDER - 1
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
        let mid = XJ111_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj111BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj111BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj111BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj111BTreeNode::xj_new_leaf();
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


// --- xk_111 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk111SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk111SegmentTree {
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
pub struct Xk111DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk111DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_111).
#[derive(Debug, Clone)]
pub struct Xl111Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl111Rope {
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

/// Suffix array for efficient string searching (xl_111).
#[derive(Debug, Clone)]
pub struct Xl111SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl111SuffixArray {
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
pub struct Xm111MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm111MatrixSparse {
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
pub struct Xm111Tokenizer {
    text: String,
}

impl Xm111Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 110.
pub struct Xn110Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn110Fenwick {
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

// ----- AVL tree map — crate 110 -----

#[derive(Debug, Clone)]
struct Xn110AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn110AvlNode<K, V>>>,
    right: Option<Box<Xn110AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 110.
#[derive(Debug, Clone)]
pub struct Xn110AVL<K, V> {
    root: Option<Box<Xn110AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn110AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn110AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn110AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn110AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn110AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn110AvlNode<K, V>>) -> Box<Xn110AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn110AvlNode<K, V>>) -> Box<Xn110AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn110AvlNode<K, V>>) -> Box<Xn110AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn110AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn110AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn110AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn110AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn110AvlNode<K, V>>) -> &Xn110AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn110AvlNode<K, V>>) -> (Box<Xn110AvlNode<K, V>>, Option<Box<Xn110AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn110AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn110AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn110AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn110AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn110AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn110AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn110AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo110RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo110Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo110RBNode<K, V> {
    key: K,
    value: V,
    color: Xo110Color,
    left: Option<Box<Xo110RBNode<K, V>>>,
    right: Option<Box<Xo110RBNode<K, V>>>,
}

/// A red-black tree map for crate 110.
#[derive(Debug, Clone)]
pub struct Xo110RedBlack<K, V> {
    root: Option<Box<Xo110RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo110RedBlack<K, V> {
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
            r.color = Xo110Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo110RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo110RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo110RBNode {
                    key, value, color: Xo110Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo110RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo110Color::Red)
    }

    fn xo_balance(mut h: Box<Xo110RBNode<K, V>>) -> Box<Xo110RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo110Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo110RBNode<K, V>>) -> Box<Xo110RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo110Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo110RBNode<K, V>>) -> Box<Xo110RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo110Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo110RBNode<K, V>>) {
        h.color = Xo110Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo110Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo110Color::Black; }
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
            r.color = Xo110Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo110RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo110RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo110RBNode<K, V>) -> (K, V, Option<Box<Xo110RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo110RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo110Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo110RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo110ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 110.
#[derive(Debug, Clone)]
pub struct Xo110ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo110ConsistentHash {
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
            let vkey = format!("{}#xo110#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo110#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 111).
#[derive(Debug)]
pub struct Xp111SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp111Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp111Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp111Node<K, V>>>,
    xp_right: Option<Box<Xp111Node<K, V>>>,
}

impl<K: Ord, V> Xp111Node<K, V> {
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

impl<K: Ord, V> Default for Xp111SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp111SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp111Node<K, V>>>, key: &K) -> Option<Box<Xp111Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp111Node<K, V>>) -> Box<Xp111Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp111Node<K, V>>) -> Box<Xp111Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp111Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp111Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp111Node::xp_new(key, val));
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


    // xa_ extended tests for lifecycle_svc
    #[test]
    fn xa_lifecycle_svc_ring_new() {
        let rb = super::XaLifecycleSvcRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_lifecycle_svc_ring_push_len() {
        let mut rb = super::XaLifecycleSvcRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_lifecycle_svc_ring_wrap() {
        let mut rb = super::XaLifecycleSvcRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_lifecycle_svc_ring_mean_empty() {
        let rb = super::XaLifecycleSvcRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_lifecycle_svc_ring_mean_values() {
        let mut rb = super::XaLifecycleSvcRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_lifecycle_svc_ring_min_max() {
        let mut rb = super::XaLifecycleSvcRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_lifecycle_svc_ring_iter() {
        let mut rb = super::XaLifecycleSvcRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_lifecycle_svc_counter_new() {
        let c = super::XaLifecycleSvcCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_lifecycle_svc_counter_inc() {
        let mut c = super::XaLifecycleSvcCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_lifecycle_svc_counter_inc_by() {
        let mut c = super::XaLifecycleSvcCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_lifecycle_svc_counter_reset() {
        let mut c = super::XaLifecycleSvcCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_lifecycle_svc_counter_clear() {
        let mut c = super::XaLifecycleSvcCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_lifecycle_svc_counter_default() {
        let c = super::XaLifecycleSvcCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 111 ----

    #[test]
    fn xc_111_pool_new_empty() {
        let pool: super::Xc111Pool<i32> = super::Xc111Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_111_pool_release_acquire() {
        let mut pool = super::Xc111Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_111_pool_acquire_empty() {
        let mut pool: super::Xc111Pool<i32> = super::Xc111Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_111_pool_full() {
        let mut pool = super::Xc111Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_111_pool_drain() {
        let mut pool = super::Xc111Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_111_pool_stats() {
        let mut pool = super::Xc111Pool::new(8);
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
    fn xc_111_pool_clear() {
        let mut pool = super::Xc111Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_111_pool_shrink() {
        let mut pool = super::Xc111Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_111_pool_default() {
        let pool: super::Xc111Pool<String> = super::Xc111Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_111_pool_extend() {
        let mut pool = super::Xc111Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_111_pool_retain() {
        let mut pool = super::Xc111Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_111_scheduler_round_robin() {
        let mut sched = super::Xc111Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_111_scheduler_empty() {
        let mut sched = super::Xc111Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_111_scheduler_reset() {
        let mut sched = super::Xc111Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_111_scheduler_add_remove() {
        let mut sched = super::Xc111Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_111_scheduler_targets() {
        let sched = super::Xc111Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_111_hash_empty() {
        assert_eq!(super::xc_111_hash(b""), 5381);
    }

    #[test]
    fn xc_111_hash_data() {
        let h = super::xc_111_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_111_hash(b"hello"), h);
    }

    #[test]
    fn xc_111_reverse_str() {
        assert_eq!(super::xc_111_reverse("abc"), "cba");
        assert_eq!(super::xc_111_reverse(""), "");
    }


    // --- xd_69 deepening tests ---

    #[test]
    fn xd_69_sm_initial_state() {
        let sm = Xd69StateMachine::new();
        assert_eq!(sm.current_state(), Xd69State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_69_sm_valid_idle_to_running() {
        let mut sm = Xd69StateMachine::new();
        assert!(sm.transition(Xd69State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd69State::Running);
    }

    #[test]
    fn xd_69_sm_valid_running_to_paused() {
        let mut sm = Xd69StateMachine::new();
        sm.transition(Xd69State::Running).unwrap();
        assert!(sm.transition(Xd69State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd69State::Paused);
    }

    #[test]
    fn xd_69_sm_valid_running_to_done() {
        let mut sm = Xd69StateMachine::new();
        sm.transition(Xd69State::Running).unwrap();
        assert!(sm.transition(Xd69State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd69State::Done);
    }

    #[test]
    fn xd_69_sm_valid_paused_to_running() {
        let mut sm = Xd69StateMachine::new();
        sm.transition(Xd69State::Running).unwrap();
        sm.transition(Xd69State::Paused).unwrap();
        assert!(sm.transition(Xd69State::Running).is_ok());
    }

    #[test]
    fn xd_69_sm_valid_done_to_idle() {
        let mut sm = Xd69StateMachine::new();
        sm.transition(Xd69State::Running).unwrap();
        sm.transition(Xd69State::Done).unwrap();
        assert!(sm.transition(Xd69State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd69State::Idle);
    }

    #[test]
    fn xd_69_sm_invalid_idle_to_done() {
        let mut sm = Xd69StateMachine::new();
        assert!(sm.transition(Xd69State::Done).is_err());
    }

    #[test]
    fn xd_69_sm_invalid_idle_to_paused() {
        let mut sm = Xd69StateMachine::new();
        assert!(sm.transition(Xd69State::Paused).is_err());
    }

    #[test]
    fn xd_69_sm_history_tracking() {
        let mut sm = Xd69StateMachine::new();
        sm.transition(Xd69State::Running).unwrap();
        sm.transition(Xd69State::Paused).unwrap();
        sm.transition(Xd69State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd69State::Idle);
        assert_eq!(sm.history()[0].to, Xd69State::Running);
        assert_eq!(sm.history()[1].from, Xd69State::Running);
        assert_eq!(sm.history()[2].to, Xd69State::Done);
    }

    #[test]
    fn xd_69_sm_serialize_deserialize() {
        let mut sm = Xd69StateMachine::new();
        sm.transition(Xd69State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd69StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd69State::Running));
    }

    #[test]
    fn xd_69_sm_deserialize_invalid() {
        assert_eq!(Xd69StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_69_sm_reset() {
        let mut sm = Xd69StateMachine::new();
        sm.transition(Xd69State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd69State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_69_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd69EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd69Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_69_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd69EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd69Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd69Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_69_bus_unsubscribe() {
        let mut bus = Xd69EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_69_event_kind_and_payload() {
        let e = Xd69Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd69Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_69_bus_clear_history() {
        let mut bus = Xd69EventBus::new();
        bus.publish(Xd69Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_69_sm_step_counter_increments() {
        let mut sm = Xd69StateMachine::new();
        sm.transition(Xd69State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd69State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #78 --

    #[test]
    fn xf78_trie_insert_search() {
        let mut t = Xf78Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf78_trie_starts_with() {
        let mut t = Xf78Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf78_trie_remove() {
        let mut t = Xf78Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf78_trie_word_count() {
        let mut t = Xf78Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf78_trie_longest_prefix() {
        let mut t = Xf78Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf78_trie_all_words() {
        let mut t = Xf78Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf78_trie_autocomplete() {
        let mut t = Xf78Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf78_trie_empty_search() {
        let t = Xf78Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf78_bloom_add_contains() {
        let mut bf = Xf78BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf78_bloom_probably_absent() {
        let bf = Xf78BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf78_bloom_false_positive_rate() {
        let mut bf = Xf78BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf78_bloom_clear() {
        let mut bf = Xf78BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf78_bloom_union() {
        let mut a = Xf78BloomFilter::xf_new(512, 2);
        let mut b = Xf78BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf78_bloom_intersection_estimate() {
        let mut a = Xf78BloomFilter::xf_new(512, 2);
        let mut b = Xf78BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf78_bloom_union_size_mismatch() {
        let a = Xf78BloomFilter::xf_new(256, 2);
        let b = Xf78BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh110_skip_insert_contains() {
        let mut sl = super::Xh110SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh110_skip_remove() {
        let mut sl = super::Xh110SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh110_skip_len() {
        let mut sl = super::Xh110SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh110_skip_range_query() {
        let mut sl = super::Xh110SkipList::xh_new(4);
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
    fn xh110_skip_floor_ceiling() {
        let mut sl = super::Xh110SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh110_skip_rank() {
        let mut sl = super::Xh110SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh110_skip_empty() {
        let sl = super::Xh110SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh110_skip_duplicates() {
        let mut sl = super::Xh110SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh110_bitset_set_test() {
        let mut bs = super::Xh110BitSet::xh_new(256);
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
    fn xh110_bitset_clear_count() {
        let mut bs = super::Xh110BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh110_bitset_and_or_xor() {
        let mut a = super::Xh110BitSet::xh_new(128);
        let mut b = super::Xh110BitSet::xh_new(128);
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
    fn xh110_bitset_iter_ones() {
        let mut bs = super::Xh110BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh110_bitset_first_last() {
        let mut bs = super::Xh110BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh110_bitset_empty() {
        let bs = super::Xh110BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi110_deque_push_pop_back() {
        let mut dq = super::Xi110Deque::xi_new(4);
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
    fn xi110_deque_push_pop_front() {
        let mut dq = super::Xi110Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi110_deque_mixed_ops() {
        let mut dq = super::Xi110Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi110_deque_get_and_split() {
        let mut dq = super::Xi110Deque::xi_new(8);
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
    fn xi110_deque_rotate_left() {
        let mut dq = super::Xi110Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi110_deque_rotate_right() {
        let mut dq = super::Xi110Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi110_deque_grow() {
        let mut dq = super::Xi110Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi110_deque_empty() {
        let dq = super::Xi110Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi110_interval_tree_insert_query() {
        let mut tree = super::Xi110IntervalTree::xi_new();
        tree.xi_insert(super::Xi110Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi110Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi110Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi110_interval_tree_overlap() {
        let mut tree = super::Xi110IntervalTree::xi_new();
        tree.xi_insert(super::Xi110Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi110Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi110Interval::xi_new(12, 20));
        let q = super::Xi110Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi110_interval_tree_remove() {
        let mut tree = super::Xi110IntervalTree::xi_new();
        tree.xi_insert(super::Xi110Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi110Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi110_interval_tree_gaps() {
        let mut tree = super::Xi110IntervalTree::xi_new();
        tree.xi_insert(super::Xi110Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi110Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi110Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi110Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi110Interval::xi_new(8, 10));
    }

    #[test]
    fn xi110_interval_tree_merge() {
        let mut tree = super::Xi110IntervalTree::xi_new();
        tree.xi_insert(super::Xi110Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi110Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi110Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi110Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi110Interval::xi_new(10, 15));
    }

    #[test]
    fn xi110_interval_tree_all() {
        let mut tree = super::Xi110IntervalTree::xi_new();
        tree.xi_insert(super::Xi110Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi110Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi110_interval_tree_empty() {
        let tree = super::Xi110IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi110_interval_tree_contains_point() {
        let iv = super::Xi110Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 111) ---

    #[test]
    fn xj_111_uf_make_and_find() {
        let mut uf = super::Xj111UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_111_uf_union_connected() {
        let mut uf = super::Xj111UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_111_uf_component_count() {
        let mut uf = super::Xj111UnionFind::xj_new();
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
    fn xj_111_uf_component_size() {
        let mut uf = super::Xj111UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_111_uf_largest_component() {
        let mut uf = super::Xj111UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_111_uf_many_elements() {
        let mut uf = super::Xj111UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_111_uf_separate_components() {
        let mut uf = super::Xj111UnionFind::xj_new();
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
    fn xj_111_uf_path_compression() {
        let mut uf = super::Xj111UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_111_bt_insert_get() {
        let mut bt = super::Xj111BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_111_bt_contains_len() {
        let mut bt = super::Xj111BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_111_bt_replace() {
        let mut bt = super::Xj111BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_111_bt_remove() {
        let mut bt = super::Xj111BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_111_bt_keys_values() {
        let mut bt = super::Xj111BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_111_bt_range() {
        let mut bt = super::Xj111BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_111_bt_min_max() {
        let mut bt = super::Xj111BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_111_bt_many_inserts() {
        let mut bt = super::Xj111BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_111 segment tree tests ---

    #[test]
    fn xk_111_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk111SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_111_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk111SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_111_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk111SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_111_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk111SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_111_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk111SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_111_st_single_element() {
        let data = vec![42];
        let st = super::Xk111SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_111_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk111SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_111_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk111SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_111 disjoint intervals tests ---

    #[test]
    fn xk_111_di_add_and_count() {
        let mut di = super::Xk111DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_111_di_merge_overlap() {
        let mut di = super::Xk111DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_111_di_contains() {
        let mut di = super::Xk111DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_111_di_remove() {
        let mut di = super::Xk111DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_111_di_covered_length() {
        let mut di = super::Xk111DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_111_di_gaps() {
        let mut di = super::Xk111DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_111_di_merge_adjacent() {
        let mut di = super::Xk111DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_111_di_empty() {
        let di = super::Xk111DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_111_rope_new_empty() {
        let rope = super::Xl111Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_111_rope_from_str() {
        let rope = super::Xl111Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_111_rope_insert_at() {
        let mut rope = super::Xl111Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_111_rope_delete_range() {
        let mut rope = super::Xl111Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_111_rope_char_at() {
        let rope = super::Xl111Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_111_rope_split_concat() {
        let rope = super::Xl111Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_111_rope_line_count() {
        let rope = super::Xl111Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_111_rope_line_at() {
        let rope = super::Xl111Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_111_sa_build_and_search() {
        let sa = super::Xl111SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_111_sa_count() {
        let sa = super::Xl111SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_111_sa_longest_repeated() {
        let sa = super::Xl111SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_111_sa_all_positions() {
        let sa = super::Xl111SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_111_sa_len() {
        let sa = super::Xl111SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_111_sa_empty() {
        let sa = super::Xl111SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_111_rope_slice() {
        let rope = super::Xl111Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_111_sa_search_start() {
        let sa = super::Xl111SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_111_sparse_set_get() {
        let mut m = super::Xm111MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_111_sparse_row_col() {
        let mut m = super::Xm111MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_111_sparse_transpose() {
        let mut m = super::Xm111MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_111_sparse_multiply_vec() {
        let mut m = super::Xm111MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_111_sparse_nnz_density() {
        let mut m = super::Xm111MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_111_sparse_clear() {
        let mut m = super::Xm111MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_111_sparse_overwrite_zero() {
        let mut m = super::Xm111MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_111_tokenizer_basic() {
        let t = super::Xm111Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_111_tokenizer_count() {
        let t = super::Xm111Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_111_tokenizer_unique() {
        let t = super::Xm111Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_111_tokenizer_frequency() {
        let t = super::Xm111Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_111_tokenizer_delimiter() {
        let t = super::Xm111Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_111_tokenizer_whitespace() {
        let t = super::Xm111Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_111_tokenizer_empty() {
        let t = super::Xm111Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 110 ----

    #[test]
    fn xn_110_fenwick_prefix_sum() {
        let mut ft = super::Xn110Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_110_fenwick_range_sum() {
        let mut ft = super::Xn110Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_110_fenwick_point_query() {
        let mut ft = super::Xn110Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_110_fenwick_len() {
        let ft = super::Xn110Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_110_fenwick_multiple_updates() {
        let mut ft = super::Xn110Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_110_fenwick_single_element() {
        let mut ft = super::Xn110Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_110_fenwick_find_kth() {
        let mut ft = super::Xn110Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_110_fenwick_negative_delta() {
        let mut ft = super::Xn110Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 110 ----

    #[test]
    fn xn_110_avl_insert_get() {
        let mut m = super::Xn110AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_110_avl_remove() {
        let mut m = super::Xn110AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_110_avl_in_order() {
        let mut m = super::Xn110AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_110_avl_min_max() {
        let mut m = super::Xn110AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_110_avl_floor_ceiling() {
        let mut m = super::Xn110AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_110_avl_height_balanced() {
        let mut m = super::Xn110AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_110_avl_overwrite() {
        let mut m = super::Xn110AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_110_avl_empty() {
        let m: super::Xn110AVL<i32, i32> = super::Xn110AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo110RedBlack tests ---

    #[test]
    fn xo_110_rb_insert_and_get() {
        let mut tree = super::Xo110RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_110_rb_len_and_empty() {
        let mut tree = super::Xo110RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_110_rb_min_max() {
        let mut tree = super::Xo110RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_110_rb_contains() {
        let mut tree = super::Xo110RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_110_rb_remove() {
        let mut tree = super::Xo110RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_110_rb_in_order() {
        let mut tree = super::Xo110RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_110_rb_black_height() {
        let mut tree = super::Xo110RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_110_rb_overwrite() {
        let mut tree = super::Xo110RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo110ConsistentHash tests ---

    #[test]
    fn xo_110_ch_add_and_count() {
        let mut ring = super::Xo110ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_110_ch_remove_node() {
        let mut ring = super::Xo110ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_110_ch_get_node() {
        let mut ring = super::Xo110ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_110_ch_empty_ring() {
        let ring = super::Xo110ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_110_ch_distribution() {
        let mut ring = super::Xo110ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_110_ch_rebalance() {
        let mut ring = super::Xo110ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_110_ch_virtual_nodes() {
        let mut ring = super::Xo110ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_110_ch_consistent_lookup() {
        let mut ring = super::Xo110ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_111_splay_insert_get() {
        let mut t = super::Xp111SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_111_splay_remove() {
        let mut t = super::Xp111SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_111_splay_count_increases() {
        let mut t = super::Xp111SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_111_splay_depth() {
        let mut t = super::Xp111SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_111_splay_len_empty() {
        let t = super::Xp111SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_111_splay_min_max() {
        let mut t = super::Xp111SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_111_splay_overwrite() {
        let mut t = super::Xp111SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_111_splay_remove_missing() {
        let mut t = super::Xp111SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }

}