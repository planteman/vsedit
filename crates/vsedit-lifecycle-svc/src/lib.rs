//! App startup and shutdown lifecycle.
//!
//! Equivalent to VS Code's `vs/platform/lifecycle/common/lifecycle.ts`.
//! Manages application phases and shutdown confirmation.

use std::collections::HashSet;
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

}
